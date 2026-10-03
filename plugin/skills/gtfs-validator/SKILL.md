---
name: gtfs-validator
description: Validate GTFS Schedule ZIP files or public GTFS feed URLs with GTFS Validator and explain prioritized findings.
---

# GTFS Validator

Use the connected GTFS Validator MCP server for GTFS Schedule validation.

## Tool selection

- Use `analyze_gtfs_file` for a user-uploaded GTFS Schedule ZIP.
- Use `analyze_gtfs_url` for a public, directly downloadable GTFS ZIP URL.
- Use `get_gtfs_rule` when the user asks what a rule ID means.

Preserve the returned `status`, `analysis.input_mode`,
`analysis.source_url_provided`, publishability, coverage, scores, severity
counts, feed metrics, and R9 ordering. Do not invent findings or claim that a
feed is publishable when the returned report says otherwise.

Treat the Analyzer's scores, publishability verdict, severity counts, and R9
ordering as canonical. Do not recalculate or reinterpret them in the client.
Quality- or Analytics-only findings do not make a feed invalid by themselves;
follow the returned publishability and blocker fields. Explain `partial`
results as incomplete coverage, and explain `fatal` results as an analysis
failure rather than a feed verdict.

If the server returns `FILE_TOO_LARGE`, explain that synchronous ChatGPT
validation is bounded and provide the returned `analyzer_web_url` when present.
