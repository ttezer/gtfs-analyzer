# Issue #118 — feed expiry horizon audit

Date: 2026-08-09  
Decision: keep `FIN_019` as one operational Quality rule with a configurable
feed-info horizon; keep calendar expiry configuration separate.

## Current rule separation

- `FIN_010` reports an already expired `feed_info.feed_end_date`.
- `FIN_019` reports a future `feed_info.feed_end_date` within
  `feed_info_expiry_warning_days` (default **7**, valid range **1–60**).
- `CAL_008` reports service `end_date` values within
  `feed_expiry_warning_days` (default **30**).
- `FIN_016`, `FIN_017`, and `FIN_020` retain their independent future-date and
  validity-window semantics.

The two configuration keys are deliberately separate. A user may want a
30-day calendar-service freshness warning while keeping the feed publication
metadata warning at the current 7-day horizon, or the reverse. FIN_019 remains
the same rule ID and reports `warning_days` and `days_left` in `details`.

## Regression matrix

For `today = 2026-05-14`:

| feed_end_date distance | default horizon 7 | horizon 30 |
|---:|:---:|:---:|
| 3 days | FIN_019 | FIN_019 |
| 10 days | no FIN_019 | FIN_019 |
| 29 days | no FIN_019 | FIN_019 |
| 31 days | no FIN_019 | no FIN_019 |
| already expired | FIN_010 only | FIN_010 only |
| missing feed_end_date | no FIN_019 | no FIN_019 |

All rows above are covered by pipeline tests. Config merge tests cover the
default, independent override, and 1-day lower-bound rejection.

## Corpus comparison

The issue's 1000-feed audit reports **105 feeds / 106 MobilityData warnings**
for the 30-day upstream signal. That full 1000-feed artifact is not present in
the checked-in workspace. The available local report/ZIP subset contains 24
feeds with the old `feed_expiration_date7_days` notice (24 warnings). A CLI
rerun of those exact local ZIP/report pairs with
`{"feed_info_expiry_warning_days":30}` and each report's validation date
produced **1 FIN_019** and **23 no-match** results; most pairs contain stale
feed metadata whose `feed_end_date` was already years in the past by the
recorded validation date, so they correctly produce FIN_010 rather than
FIN_019. This is evidence of correct expired-feed separation, not a substitute
for the missing 105-feed rerun.

The implementation is therefore validated against the complete fixture matrix
and the available corpus subset, while the 105/106 comparison remains an
explicit follow-up when the original 1000-feed ZIP/report artifact is restored.

## Classification

This is an operational freshness preference, not a GTFS Schedule Reference
validity constraint. `FIN_019` remains `Quality`/Low and does not affect the
Spec publication gate.
