import asyncio
from http.server import BaseHTTPRequestHandler, HTTPServer
import os
import tempfile
import threading
import unittest
from unittest.mock import AsyncMock, patch

import httpx
import server


class _Response:
    async def aiter_bytes(self):
        yield b"feed"


class _LargeResponse:
    async def aiter_bytes(self):
        yield b"feed"


class _Stdin:
    def is_closing(self):
        return False

    def write(self, _chunk):
        return None

    async def drain(self):
        return None

    def close(self):
        return None


class _Pipe:
    async def read(self, _limit=-1):
        return b"{}"


class _SlowProcess:
    def __init__(self):
        self.stdin = _Stdin()
        self.stdout = _Pipe()
        self.stderr = _Pipe()
        self.returncode = None
        self.killed = False
        self.wait_calls = 0

    async def wait(self):
        self.wait_calls += 1
        if not self.killed:
            await asyncio.sleep(0.05)
        self.returncode = -9 if self.killed else 0
        return self.returncode

    def kill(self):
        self.killed = True


class ServerTests(unittest.IsolatedAsyncioTestCase):
    def test_private_and_unsupported_urls_are_rejected(self):
        for url in (
            "http://127.0.0.1/feed.zip",
            "http://10.0.0.1/feed.zip",
            "http://172.16.0.1/feed.zip",
            "http://192.168.0.1/feed.zip",
            "http://169.254.169.254/latest/meta-data/",
            "http://[::1]/feed.zip",
            "http://[fd00::1]/feed.zip",
            "http://100.64.0.1/feed.zip",
            "http://0.0.0.0/feed.zip",
            "http://224.0.0.1/feed.zip",
            "http://192.0.0.1/feed.zip",
        ):
            with self.subTest(url=url), self.assertRaises(server.ToolError) as private:
                server._allowed_url(url)
            self.assertEqual(private.exception.error_type, "PRIVATE_NETWORK_URL")

        with self.assertRaises(server.ToolError) as scheme:
            server._allowed_url("ftp://example.com/feed.zip")
        self.assertEqual(scheme.exception.error_type, "UNSUPPORTED_SCHEME")

    def test_public_hostname_is_checked_against_resolved_addresses(self):
        with patch(
            "server.socket.getaddrinfo",
            return_value=[(None, None, None, None, ("8.8.8.8", 443))],
        ):
            server._allowed_url("https://feeds.example.test/feed.zip")

    async def test_download_transport_uses_checked_ip_and_original_host(self):
        seen_hosts = []

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                seen_hosts.append(self.headers["Host"])
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b"ok")

            def log_message(self, *_args):
                return None

        httpd = HTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=httpd.serve_forever, daemon=True)
        thread.start()
        try:
            transport = server._PinnedIPTransport("127.0.0.1", "feeds.example.test")
            async with httpx.AsyncClient(transport=transport) as client:
                response = await client.get(
                    f"http://feeds.example.test:{httpd.server_port}/feed.zip"
                )
            self.assertEqual(response.status_code, 200)
            self.assertEqual(response.content, b"ok")
            self.assertEqual(seen_hosts, [f"feeds.example.test:{httpd.server_port}"])
        finally:
            httpd.shutdown()
            httpd.server_close()
            thread.join(timeout=2)

    async def test_analyzer_timeout_kills_subprocess(self):
        process = _SlowProcess()
        with patch.object(server, "ANALYZER_TIMEOUT_SECONDS", 0.001), patch(
            "server.asyncio.create_subprocess_exec", new_callable=AsyncMock, return_value=process
        ):
            with self.assertRaises(server.ToolError) as error:
                await server._validate_stream(_Response(), "en", None)

        self.assertEqual(error.exception.error_type, "ANALYZER_TIMEOUT")
        self.assertTrue(process.killed)
        self.assertGreaterEqual(process.wait_calls, 2)

    async def test_total_timeout_kills_process_and_deletes_url_config(self):
        process = _SlowProcess()
        created_paths = []
        real_named_temporary_file = tempfile.NamedTemporaryFile

        def capture_tempfile(*args, **kwargs):
            handle = real_named_temporary_file(*args, **kwargs)
            created_paths.append(handle.name)
            return handle

        with patch.object(server, "TOTAL_TIMEOUT_SECONDS", 0.001), patch(
            "server.asyncio.create_subprocess_exec", new_callable=AsyncMock, return_value=process
        ), patch.object(server.tempfile, "NamedTemporaryFile", side_effect=capture_tempfile):
            result = await server._run_bounded(
                lambda: server._validate_stream(
                    _Response(), "en", "https://feeds.example.test/gtfs.zip?token=secret"
                )
            )

        self.assertEqual(result["error"]["type"], "ANALYSIS_TIMEOUT")
        self.assertTrue(process.killed)
        self.assertGreaterEqual(process.wait_calls, 1)
        self.assertTrue(created_paths)
        self.assertTrue(all(not os.path.exists(path) for path in created_paths))

    async def test_download_limit_kills_subprocess(self):
        process = _SlowProcess()
        with patch.object(server, "MAX_DOWNLOAD_BYTES", 3), patch(
            "server.asyncio.create_subprocess_exec", new_callable=AsyncMock, return_value=process
        ):
            with self.assertRaises(server.ToolError) as error:
                await server._validate_stream(_LargeResponse(), "en", None)

        self.assertEqual(error.exception.error_type, "FILE_TOO_LARGE")
        self.assertEqual(error.exception.details["analyzer_web_url"], server.ANALYZER_WEB_URL)
        self.assertTrue(process.killed)

    async def test_analysis_slot_rejects_concurrent_work(self):
        await server._ANALYSIS_SLOTS.acquire()
        try:
            async def operation():
                return {"status": "ok"}

            result = await server._run_bounded(lambda: operation())
        finally:
            server._ANALYSIS_SLOTS.release()

        self.assertEqual(result["error"]["type"], "RESOURCE_LIMIT")

    def test_tool_error_is_json_safe(self):
        result = server._tool_error(server.ToolError("FILE_TOO_LARGE", "too big", limit=10))
        self.assertEqual(result, {"error": {"type": "FILE_TOO_LARGE", "message": "too big", "limit": 10}})

    async def test_subject_rate_limit_and_rule_id_normalization(self):
        class Meta:
            def model_dump(self, by_alias=False):
                return {"openai/subject": "subject-1"}

        class RequestContext:
            meta = Meta()

        class Context:
            request_context = RequestContext()

        with patch.object(server, "SUBJECT_RATE_LIMIT", 1), patch.object(
            server, "SUBJECT_RATE_WINDOW_SECONDS", 60
        ):
            server._SUBJECT_REQUESTS.clear()
            self.assertIsNone(await server._check_subject_rate_limit(Context()))
            error = await server._check_subject_rate_limit(Context())
        self.assertEqual(error.error_type, "RATE_LIMIT")

        with patch.dict(server._rules_cache, {"en": [{"id": "TRP_002", "title": "route_id missing"}]}, clear=True):
            rule = await server.get_gtfs_rule(" trp_002 ")
            unknown = await server.get_gtfs_rule("NOPE_999")
        self.assertEqual(rule["id"], "TRP_002")
        self.assertEqual(unknown["error"]["type"], "UNKNOWN_RULE")


if __name__ == "__main__":
    unittest.main()
