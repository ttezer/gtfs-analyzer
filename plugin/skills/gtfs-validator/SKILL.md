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
- Treat `/gtfs` as routing intent, not as a fourth MCP tool:
  - `/gtfs` plus an attached ZIP → `analyze_gtfs_file`.
  - `/gtfs` plus a public URL → `analyze_gtfs_url`.
  - `/gtfs TRP_002` (or another rule ID) → `get_gtfs_rule`.
  - Bare `/gtfs` → ask for a GTFS ZIP upload or public feed URL.
- A natural-language request such as `Bu GTFS dosyasını analiz et` routes to
  `analyze_gtfs_file` when the file is attached.

MCP tool metadata can guide this selection, but it does not register a native
ChatGPT slash command. If the client does not interpret `/gtfs` as prompt text,
the user must provide the same intent in ordinary language.

Do not invoke the validator just to list or extract files from a generic ZIP.

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
