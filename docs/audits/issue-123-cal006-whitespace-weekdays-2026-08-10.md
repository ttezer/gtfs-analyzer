# Issue #123 — `CAL_006` whitespace weekday recovery

Date: 2026-08-10
Status: implementation verified; final corpus acceptance remains pending

## Finding and root cause

The `mdb-2830` regression feed contains `calendar.txt` weekday values such as
`" 0"`. K2's lexical parser correctly records the leading whitespace as `DQ_016`,
but previously left the weekday value unavailable to the all-zero weekly-pattern
check. K7 then removed the dependent `CAL_002` whitespace derivatives, leaving no
`CAL_006` finding for the 12 affected services.

The fix keeps the two concerns separate:

- trim only numeric `0/1` payloads for the weekly-pattern decision;
- retain `DQ_016` as the raw lexical finding;
- retain `CAL_002` when the trimmed weekday value is outside `0/1` or non-numeric;
- allow `CAL_006` to remain informational for dates-only services, including rows
  with `calendar_dates.txt` additions.

## Same-input replay

Feed: `mdb-2830.zip`
SHA-256: `158fe2dff61c65c8cba71b0e75170712f08904c8f09aadc05c390ca9b08847ae`

| Check | Before | After |
|---|---:|---:|
| `CAL_006` | 0 | 12 |
| `CAL_002` | suppressed derivative | 0 |
| `DQ_016` | 1 | 1 |

Recovered service IDs:

`3308`, `3317`, `3318`, `3338`, `3350`, `3352`, `3354`, `3356`, `3360`, `3362`, `3366`, `3367`.

The feed contains `calendar_dates.txt` additions; those do not suppress the
informational weekly all-zero finding. The reduced emit-proof fixture mirrors the
same whitespace shape and locks this behavior.

## Regression coverage

- Whitespace-wrapped all-zero weekdays emit `CAL_006` without a duplicate
  whitespace-derived `CAL_002`.
- Whitespace-wrapped weekday `1` keeps `CAL_006` silent.
- A whitespace-wrapped invalid weekday keeps `CAL_002`; parseable remaining zeros
  may independently produce `CAL_006`.
- `calendar_dates.txt` additions do not hide `CAL_006`.

The issue remains open until the planned corpus rerun and DELFI comparison are
completed; this replay proves the pinned regression and does not claim full-corpus
parity.
