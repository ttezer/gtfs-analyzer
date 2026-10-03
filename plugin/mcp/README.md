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
GTFS_MAX_DOWNLOAD_BYTES       (default 512 MiB)
GTFS_TOTAL_TIMEOUT_SECONDS    (default 105)
GTFS_ANALYZER_TIMEOUT_SECONDS (default 90)
MCP_ALLOWED_HOST              (temporary tunnel hostname, if used)
```

This is the first integration slice. Cloud deployment, production DNS rebinding
pinning, rate limiting, resource ceilings, and container packaging remain separate
release gates.

