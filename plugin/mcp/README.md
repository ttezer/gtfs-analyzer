# GTFS Validator MCP

Python MCP boundary for the native `gtfs-analyzer` CLI.

The server exposes:

- `analyze_gtfs_file`
- `analyze_gtfs_url`
- `get_gtfs_rule`

The analyzer binary is selected with `GTFS_ANALYZER_BIN`. The default transport
endpoint is `http://127.0.0.1:8787/mcp`.

Important environment settings:

```text
GTFS_ANALYZER_BIN
GTFS_MAX_DOWNLOAD_BYTES       (default 20 MiB; provisional Phase 4 cap)
GTFS_TOTAL_TIMEOUT_SECONDS    (default 105)
GTFS_ANALYZER_TIMEOUT_SECONDS (default 90)
GTFS_ANALYZER_WEB_URL         (default https://ttezer.github.io/gtfs-analyzer/)
MCP_ALLOWED_HOST              (temporary tunnel hostname, if used)
```

This is the first integration slice. Cloud deployment, production DNS rebinding
pinning, rate limiting, resource ceilings, and container packaging remain separate
release gates.

Operational logging is URL-free: HTTP client request logs are suppressed at INFO
level and download errors do not echo source URLs or query strings.

The total request deadline and the native Analyzer subprocess deadline are
independent: `GTFS_TOTAL_TIMEOUT_SECONDS` bounds download plus validation, while
`GTFS_ANALYZER_TIMEOUT_SECONDS` bounds the native process after the feed has been
streamed to stdin.

Run the local MCP boundary tests with:

```bash
python -m unittest test_server.py
```
