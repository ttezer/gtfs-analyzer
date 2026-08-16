# Full MobilityDatabase GTFS Schedule audit — run 31934698855

This directory pins the successful full-corpus audit so another reviewer/AI can retrieve and inspect the raw outputs without relying on chat context.

## Source run

- Repository: `ttezer/gtfs-analyzer`
- Pull request: `#143`
- Workflow run: `31934698855`
- Commit: `ca551b29cd73f9487826edc35ef8b48a25ad3396`
- Scope: MobilityDatabase rows with `data_type=gtfs` only (GTFS Schedule); GTFS Realtime excluded.
- Final artifact name: `validator-audit-all-mdb-final`
- Artifact ID: `9261023945`
- Artifact size: `38,428,908` bytes
- Artifact expires: `2026-09-15T08:47:21Z`
- Workflow URL: `https://github.com/ttezer/gtfs-analyzer/actions/runs/31934698855`
- Artifact API URL: `https://api.github.com/repos/ttezer/gtfs-analyzer/actions/artifacts/9261023945/zip`

## Raw package integrity

The final artifact contains `validator-audit-all-mdb-results.tar.gz` with recorded SHA-256:

`275602e5f4b68af891769f58c7224c170167143c4d9b4e109ccdcb01dbecb0c7`

## Files inside the final artifact

- `final/all-results.json.gz` — 14,996,556 bytes; per-feed raw audit result rows.
- `final/divergence-candidates-full.json` — 27,635,584 bytes; complete divergence candidate set.
- `final/divergence-candidates.csv` — 4,358,105 bytes.
- `final/feed-summary.csv` — 1,054,138 bytes.
- `final/analyzer-rules-full.json` — 3,244,342 bytes.
- `final/analyzer-rules.csv` — 1,636,342 bytes.
- `final/md-codes-full.json` — 667,109 bytes.
- `final/raw-evidence.json` — 271,733 bytes; pinned/re-downloaded evidence for high-priority representatives.
- `final/triage-representatives.json` — 130,744 bytes.
- `final/corpus-manifest.json` — 2,592,588 bytes.
- `final/summary.json`
- `final/completeness.json`
- `final/AUDIT_SUMMARY.md`

The two very large raw files are intentionally not duplicated as normal Git blobs. Use the pinned workflow artifact above as the canonical raw dataset. The small machine-readable summary and completeness files are committed beside this README.

## Review warning

Automated divergence candidates are not correctness verdicts. Generic MobilityData codes such as `duplicate_key` and `foreign_key_violation` can map to multiple GTFS tables/fields and require field-context-aware adjudication before being called an Analyzer miss. Likewise, Analyzer SPEC-only candidates may be cascades from a more fundamental structural error. Review raw evidence and the applicable normative GTFS rule before opening/fixing an issue.
