# GTFS Validator MCP

Python MCP boundary for the native `gtfs-analyzer` CLI.

The server exposes:

- `analyze_gtfs_file`
- `analyze_gtfs_url`
- `get_gtfs_rule`

## Routing contract

The MCP server does not register a native ChatGPT slash command. The connected
model can nevertheless treat `/gtfs` in a user message as routing intent using
the server instructions and tool descriptions:

| User request | Existing tool |
|---|---|
| `/gtfs` + attached GTFS ZIP | `analyze_gtfs_file` |
| `/gtfs` + public GTFS URL | `analyze_gtfs_url` |
| `/gtfs TRP_002` | `get_gtfs_rule` |
| bare `/gtfs` | Ask for a ZIP upload or public URL |
| `Bu GTFS dosyasını analiz et` + attached ZIP | `analyze_gtfs_file` |

This routing does not add a fourth tool or change any existing tool schema. The
Analyzer remains the source of validation results; MCP transports them and does
not recalculate scores, publishability, severity, coverage, or R9 ordering.

The analyzer binary is selected with `GTFS_ANALYZER_BIN`. The default transport
endpoint is `http://127.0.0.1:8787/mcp`.

Important environment settings:

```text
GTFS_ANALYZER_BIN
GTFS_MAX_DOWNLOAD_BYTES       (default 20 MiB; provisional Phase 4 cap)
GTFS_TOTAL_TIMEOUT_SECONDS    (default 105)
GTFS_ANALYZER_TIMEOUT_SECONDS (default 90)
GTFS_ANALYZER_WEB_URL         (default https://ttezer.github.io/gtfs-analyzer/)
GTFS_MAX_CONCURRENT_ANALYSES  (default 1)
GTFS_SUBJECT_RATE_LIMIT       (default 12 requests)
GTFS_SUBJECT_RATE_WINDOW_SECONDS (default 60)
GTFS_MAX_TRACKED_SUBJECTS     (default 10000)
MCP_ALLOWED_HOST              (required public Cloud Run/tunnel host)
```

The boundary pins each resolved public download host for the actual HTTP
connection, rejects every non-global address (including CGNAT, multicast, and
unspecified ranges), kills the native process on either timeout path, and
removes temporary URL context files. MCP requests are rate-limited by
`openai/subject` when supplied; requests without that metadata share an
anonymous bucket.

The rate-limit bucket is process-local in this release: multiple Cloud Run
instances have separate buckets, anonymous requests share one bucket, expired
entries are pruned when a subject is seen again, and the oldest keys are evicted
when `GTFS_MAX_TRACKED_SUBJECTS` is reached. Requests rejected because the
single analysis slot is busy do not consume a subject rate-limit token.

The portable package manifest is at `../plugin.json`, its bundled usage skill is
at `../skills/gtfs-validator/SKILL.md`, and the deployed MCP connection is
declared at `../mcp.json`.

## Container

Build from the repository root after Docker or Cloud Build is available:

```bash
docker build -f plugin/mcp/Dockerfile -t gtfs-validator-mcp .
```

The image listens on `PORT`/`MCP_PORT` and exposes `/mcp`. The native Analyzer
binary is built in the first stage and copied into the small Python runtime
image.

Cloud Build/Run commands are documented in `CLOUD_RUN.md`; deployment requires
an authenticated Google Cloud environment and is not performed by local tests.

Operational logging is URL-free: HTTP client request logs are suppressed at INFO
level and download errors do not echo source URLs or query strings.

The total request deadline and the native Analyzer subprocess deadline are
independent: `GTFS_TOTAL_TIMEOUT_SECONDS` bounds download plus validation, while
`GTFS_ANALYZER_TIMEOUT_SECONDS` bounds the native process after the feed has been
streamed to stdin.

Set `MCP_ALLOWED_HOST` to the exact public hostname used by the MCP endpoint.
The service intentionally does not accept arbitrary Host headers.

Run the local MCP boundary tests with:

```bash
python -m unittest test_server.py
```
