import asyncio
import unittest
from unittest.mock import AsyncMock, patch

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
        with self.assertRaises(server.ToolError) as private:
            server._allowed_url("http://127.0.0.1/feed.zip")
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

    async def test_download_limit_kills_subprocess(self):
        process = _SlowProcess()
        with patch.object(server, "MAX_DOWNLOAD_BYTES", 3), patch(
            "server.asyncio.create_subprocess_exec", new_callable=AsyncMock, return_value=process
        ):
            with self.assertRaises(server.ToolError) as error:
                await server._validate_stream(_LargeResponse(), "en", None)

        self.assertEqual(error.exception.error_type, "FILE_TOO_LARGE")
        self.assertTrue(process.killed)

    def test_tool_error_is_json_safe(self):
        result = server._tool_error(server.ToolError("FILE_TOO_LARGE", "too big", limit=10))
        self.assertEqual(result, {"error": {"type": "FILE_TOO_LARGE", "message": "too big", "limit": 10}})


if __name__ == "__main__":
    unittest.main()
