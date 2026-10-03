"""Production MCP boundary for GTFS Analyzer.

The validator remains the native Rust CLI. This process owns transport,
download, timeout, and error translation only.
"""

from __future__ import annotations

import asyncio
import ipaddress
import json
import logging
import os
import socket
import time
import tempfile
from collections.abc import Awaitable, Callable
from typing import Any, NotRequired, Required, TypedDict
from urllib.parse import urljoin, urlparse

import httpx
from mcp.server.fastmcp import FastMCP
from mcp.server.transport_security import TransportSecuritySettings
from mcp.types import ToolAnnotations


# HTTPX INFO request logs include the complete URL, which may contain a signed
# ChatGPT download token. Keep operational logs URL-free.
logging.getLogger("httpx").setLevel(logging.WARNING)
logging.getLogger("httpcore").setLevel(logging.WARNING)


class OpenAIFile(TypedDict):
    download_url: Required[str]
    file_id: Required[str]
    mime_type: NotRequired[str]
    file_name: NotRequired[str]


class ToolError(RuntimeError):
    def __init__(self, error_type: str, message: str, **details: Any) -> None:
        super().__init__(message)
        self.error_type = error_type
        self.details = details


MAX_DOWNLOAD_BYTES = int(os.environ.get("GTFS_MAX_DOWNLOAD_BYTES", str(20 * 1024 * 1024)))
TOTAL_TIMEOUT_SECONDS = float(os.environ.get("GTFS_TOTAL_TIMEOUT_SECONDS", "105"))
ANALYZER_TIMEOUT_SECONDS = float(os.environ.get("GTFS_ANALYZER_TIMEOUT_SECONDS", "90"))
MAX_REDIRECTS = int(os.environ.get("GTFS_MAX_REDIRECTS", "5"))
STDERR_LIMIT = 64 * 1024
MAX_CONCURRENT_ANALYSES = int(os.environ.get("GTFS_MAX_CONCURRENT_ANALYSES", "1"))
ANALYZER_WEB_URL = os.environ.get(
    "GTFS_ANALYZER_WEB_URL", "https://ttezer.github.io/gtfs-analyzer/"
)
_ANALYSIS_SLOTS = asyncio.Semaphore(MAX_CONCURRENT_ANALYSES)


def _analyzer_bin() -> str:
    return os.environ.get("GTFS_ANALYZER_BIN", "gtfs-analyzer")


def _file_too_large_error() -> ToolError:
    return ToolError(
        "FILE_TOO_LARGE",
        "The GTFS feed is too large for synchronous validation. "
        f"Use GTFS Analyzer Web instead: {ANALYZER_WEB_URL}",
        analyzer_web_url=ANALYZER_WEB_URL,
    )


def _allowed_url(url: str) -> None:
    parsed = urlparse(url)
    if parsed.scheme not in {"http", "https"} or not parsed.hostname:
        raise ToolError("UNSUPPORTED_SCHEME", "Only HTTP and HTTPS URLs are supported.")
    host = parsed.hostname
    try:
        addresses = {ipaddress.ip_address(host)}
    except ValueError:
        try:
            addresses = {
                ipaddress.ip_address(info[4][0])
                for info in socket.getaddrinfo(host, parsed.port or (443 if parsed.scheme == "https" else 80))
            }
        except socket.gaierror as exc:
            raise ToolError("INVALID_DOWNLOAD", f"Could not resolve download host: {host}") from exc
    if any(address.is_private or address.is_loopback or address.is_link_local or address.is_reserved for address in addresses):
        raise ToolError("PRIVATE_NETWORK_URL", "Private and local network URLs are not allowed.")


async def _write_stderr(stream: asyncio.StreamReader) -> bytes:
    data = await stream.read(STDERR_LIMIT + 1)
    return data[:STDERR_LIMIT]


async def _validate_stream(response: httpx.Response, language: str, source_url: str | None) -> dict[str, Any]:
    config_path: str | None = None
    command = [_analyzer_bin(), "validate", "-", "--compact-json", "--lang", language]
    if source_url:
        config = tempfile.NamedTemporaryFile(mode="w", suffix=".json", prefix="gtfs-validator-", delete=False)
        try:
            json.dump({"source_url": source_url}, config)
            config.flush()
            config_path = config.name
        finally:
            config.close()
        command.extend(["--config", config_path])

    try:
        process = await asyncio.create_subprocess_exec(
            *command,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
    except Exception:
        if config_path:
            try:
                os.unlink(config_path)
            except OSError:
                pass
        raise
    assert process.stdin and process.stdout and process.stderr
    stderr_task = asyncio.create_task(_write_stderr(process.stderr))
    stdout_task = asyncio.create_task(process.stdout.read())
    try:
        bytes_seen = 0
        async for chunk in response.aiter_bytes():
            if not chunk:
                continue
            bytes_seen += len(chunk)
            if bytes_seen > MAX_DOWNLOAD_BYTES:
                raise _file_too_large_error()
            if process.stdin.is_closing():
                raise ToolError("INVALID_DOWNLOAD", "Analyzer stdin closed before the download completed.")
            process.stdin.write(chunk)
            await process.stdin.drain()
        process.stdin.close()
        try:
            await asyncio.wait_for(process.wait(), ANALYZER_TIMEOUT_SECONDS)
        except asyncio.TimeoutError as exc:
            raise ToolError(
                "ANALYZER_TIMEOUT",
                "The GTFS Analyzer process exceeded its time limit.",
            ) from exc
        stdout = await stdout_task
        stderr = await stderr_task
    except Exception:
        if process.returncode is None:
            process.kill()
        await process.wait()
        await asyncio.gather(stdout_task, stderr_task, return_exceptions=True)
        if config_path:
            try:
                os.unlink(config_path)
            except OSError:
                pass
        raise

    if config_path:
        try:
            os.unlink(config_path)
        except OSError:
            pass
    if not stdout:
        detail = stderr.decode("utf-8", errors="replace").strip()
        raise ToolError("INTERNAL_ERROR", "Analyzer returned no JSON output.", stderr=detail)
    try:
        result = json.loads(stdout)
    except json.JSONDecodeError as exc:
        raise ToolError("INTERNAL_ERROR", "Analyzer returned invalid JSON.") from exc
    if source_url and isinstance(result, dict):
        # The CLI currently receives URL context through a config file in the
        # production adapter; never echo the raw URL into the tool response.
        result.setdefault("analysis", {}).setdefault("source_url_provided", True)
    return result


async def _download_and_validate(
    url: str,
    language: str,
    source_url_for_analyzer: str | None = None,
) -> dict[str, Any]:
    _allowed_url(url)
    started = time.monotonic()
    timeout = httpx.Timeout(connect=10.0, read=30.0, write=30.0, pool=10.0)
    async with httpx.AsyncClient(follow_redirects=False, timeout=timeout) as client:
        current = url
        for _ in range(MAX_REDIRECTS + 1):
            _allowed_url(current)
            try:
                async with client.stream("GET", current) as response:
                    if response.is_redirect:
                        location = response.headers.get("location")
                        if not location:
                            raise ToolError("INVALID_DOWNLOAD", "Redirect response has no Location header.")
                        current = urljoin(current, location)
                        continue
                    response.raise_for_status()
                    length = response.headers.get("content-length")
                    if length:
                        try:
                            declared_length = int(length)
                        except ValueError as exc:
                            raise ToolError(
                                "INVALID_DOWNLOAD",
                                "The GTFS download returned an invalid Content-Length header.",
                            ) from exc
                        if declared_length < 0:
                            raise ToolError(
                                "INVALID_DOWNLOAD",
                                "The GTFS download returned a negative Content-Length header.",
                            )
                        if declared_length > MAX_DOWNLOAD_BYTES:
                            raise _file_too_large_error()
                    result = await _validate_stream(response, language, source_url_for_analyzer)
                    if isinstance(result, dict):
                        result.setdefault("transport", {})["download_elapsed_ms"] = round(
                            (time.monotonic() - started) * 1000
                        )
                    return result
            except ToolError:
                raise
            except httpx.TimeoutException as exc:
                raise ToolError("DOWNLOAD_TIMEOUT", "The GTFS download timed out.") from exc
            except httpx.HTTPStatusError as exc:
                raise ToolError(
                    "INVALID_DOWNLOAD",
                    f"The GTFS download returned HTTP {exc.response.status_code}.",
                ) from exc
            except httpx.HTTPError as exc:
                raise ToolError("INVALID_DOWNLOAD", "The GTFS download failed.") from exc
        raise ToolError("INVALID_DOWNLOAD", "Too many HTTP redirects.")


def _tool_error(error: ToolError) -> dict[str, Any]:
    return {"error": {"type": error.error_type, "message": str(error), **error.details}}


async def _run_bounded(operation: Callable[[], Awaitable[dict[str, Any]]]) -> dict[str, Any]:
    if _ANALYSIS_SLOTS.locked():
        return _tool_error(
            ToolError(
                "RESOURCE_LIMIT",
                "Another GTFS analysis is already running. Please retry shortly.",
            )
        )

    async def run() -> dict[str, Any]:
        async with _ANALYSIS_SLOTS:
            return await operation()

    try:
        return await asyncio.wait_for(run(), TOTAL_TIMEOUT_SECONDS)
    except asyncio.TimeoutError:
        return _tool_error(ToolError("ANALYSIS_TIMEOUT", "The GTFS analysis exceeded the total time limit."))
    except ToolError as error:
        return _tool_error(error)


allowed_hosts = ["127.0.0.1:*", "localhost:*", "[::1]:*"]
if tunnel_host := os.environ.get("MCP_ALLOWED_HOST"):
    allowed_hosts.append(tunnel_host)

mcp = FastMCP(
    "GTFS Validator",
    instructions=(
        "Validate GTFS Schedule feeds using the GTFS Analyzer engine. Preserve "
        "the returned status, scores, severity counts, partial coverage, and R9 order."
    ),
    host=os.environ.get("MCP_HOST", "127.0.0.1"),
    port=int(os.environ.get("MCP_PORT", os.environ.get("PORT", "8787"))),
    streamable_http_path="/mcp",
    json_response=True,
    stateless_http=True,
    transport_security=TransportSecuritySettings(allowed_hosts=allowed_hosts),
)


@mcp.tool(
    name="analyze_gtfs_file",
    title="Analyze GTFS file",
    description=(
        "Validate an uploaded GTFS Schedule ZIP using the GTFS Analyzer engine. "
        "Returns publishability, scores, severity counts, feed metrics, and prioritized findings."
    ),
    annotations=ToolAnnotations(readOnlyHint=True, destructiveHint=False, openWorldHint=False, idempotentHint=True),
    meta={"openai/fileParams": ["file"]},
)
async def analyze_gtfs_file(file: OpenAIFile, language: str = "en") -> dict[str, Any]:
    if language not in {"tr", "en", "ja", "fr"}:
        language = "en"
    def operation() -> Awaitable[dict[str, Any]]:
        _allowed_url(file["download_url"])
        return _download_and_validate(file["download_url"], language)

    return await _run_bounded(operation)


@mcp.tool(
    name="analyze_gtfs_url",
    title="Analyze GTFS URL",
    description=(
        "Validate a public GTFS Schedule ZIP URL using the GTFS Analyzer engine. "
        "Only public HTTP/HTTPS URLs are accepted."
    ),
    annotations=ToolAnnotations(readOnlyHint=True, destructiveHint=False, openWorldHint=True, idempotentHint=True),
)
async def analyze_gtfs_url(url: str, language: str = "en") -> dict[str, Any]:
    if language not in {"tr", "en", "ja", "fr"}:
        language = "en"
    def operation() -> Awaitable[dict[str, Any]]:
        return _download_and_validate(url, language, url)

    return await _run_bounded(operation)


_rules_cache: dict[str, list[dict[str, Any]]] = {}


@mcp.tool(
    name="get_gtfs_rule",
    title="Get GTFS rule",
    description="Return the canonical GTFS Analyzer rule metadata for a rule ID.",
    annotations=ToolAnnotations(readOnlyHint=True, destructiveHint=False, openWorldHint=False, idempotentHint=True),
)
async def get_gtfs_rule(rule_id: str, language: str = "en") -> dict[str, Any]:
    if language not in {"tr", "en", "ja", "fr"}:
        language = "en"
    try:
        if language not in _rules_cache:
            process = await asyncio.create_subprocess_exec(
                _analyzer_bin(), "rules", "--json", "--lang", language,
                stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE,
            )
            stdout, stderr = await asyncio.wait_for(process.communicate(), ANALYZER_TIMEOUT_SECONDS)
            if process.returncode != 0:
                raise ToolError("INTERNAL_ERROR", "Analyzer rule registry failed to load.", stderr=stderr.decode(errors="replace"))
            _rules_cache[language] = json.loads(stdout)
        for rule in _rules_cache[language]:
            if rule.get("id") == rule_id:
                return rule
        return _tool_error(ToolError("ANALYZER_FATAL", f"Unknown GTFS rule ID: {rule_id}"))
    except asyncio.TimeoutError:
        return _tool_error(ToolError("ANALYSIS_TIMEOUT", "The Analyzer rule registry timed out."))
    except (ToolError, json.JSONDecodeError) as error:
        return _tool_error(error if isinstance(error, ToolError) else ToolError("INTERNAL_ERROR", str(error)))


def main() -> None:
    mcp.run("streamable-http")


if __name__ == "__main__":
    main()
