# Issue #121 — STM_014 shape-distance lower bound audit

## Decision

The implementation now has two independent protections. Shape-projected arc distance
remains protected against inflated projections by the existing `<= 4 × Haversine` guard
and is clamped to the Haversine lower bound, preventing self-near/crossing shapes from
hiding `STM_014`. In addition, K6 uses a shared recoverable stop-coordinate helper: K2
continues to report strict lexical whitespace as `DQ_016`, while numeric whitespace
payloads remain usable for speed and geometry analytics.

The issue remains open for final parity adjudication. Same-input replay recovered the
previously missing speed segments, but analyzer/MD counts still differ and the full
acceptance decision is not claimed here.

## Regression evidence

`crates/pipeline/src/k6_analytics.rs` covers:

- an ambiguous crossing shape whose projected arc is shorter than Haversine: the physical
  lower bound restores `STM_014` and does not create `STM_012`;
- a legitimate rail segment at approximately 175 km/h: the 300 km/h rail threshold remains
  silent;
- the existing inflated-projection fixture: the 4× Haversine fallback still suppresses the
  pathological shape false-positive;
- the existing moderate-detour fixture: a valid shape distance below the 4× ceiling remains
  in use.
- a bus segment whose both stop coordinates are strict-K2 `None` because of leading
  whitespace: trimmed numeric payloads still produce `STM_014`.

## Same-input replay

| Feed | Input SHA-256 | Analyzer `STM_014` | MD fast-travel count |
|---|---|---:|---:|
| `mdb-510` | `94933c1af1f182b3cf20a7e11881f14b9afd4a3c75ea310ee5f368b55a62c636` | 4 | 32 |
| `mdb-2712` | `5306e0acea7337ee95f9ab477edcd6707d03435ae444a67aa5a5314aebb7f698` | 139 | 51 |

The pinned `mdb-2712` segment is now traceable and reaches STM_014:

| Field | Value |
|---|---|
| trip / stops | `801515`, `1049 → 253` |
| coordinates | `(40.247612, -4.848185) → (40.317659, -4.694104)` |
| departure / arrival | `11:25:00 → 11:30:00` |
| effective route type / threshold | `3` / `120 km/h` |
| Haversine / projected / used distance | `15.215 km` / unavailable / `15.215 km` |
| speed / skip reason | `182.6 km/h` / none |

The raw stop coordinates carry lexical whitespace; the helper recovers the numeric
payload without removing `DQ_016`. The remaining count differences need threshold,
route-identity and analyzer segment-aggregation adjudication; they are not presented as
exact parity.

## Validation

- `cargo test --workspace --all-features` — green;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — green;
- `python3 -m unittest discover -s spec-audit -p 'test_*.py'` — green, no unexplained
  parity rows.

## Remaining acceptance evidence

- verify the intentional `route_type=101` rail behavior on `mdb-2785`;
- record MobilityData parity counts and any intentional aggregation difference.
