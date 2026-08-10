# Issue #121 — STM_014 shape-distance lower bound audit

## Decision

The implementation portion is complete. Shape-projected arc distance remains protected
against inflated projections by the existing `<= 4 × Haversine` guard. It is now also
clamped to the Haversine distance, because a route path cannot be physically shorter than
the straight-line stop-to-stop distance. This prevents self-near/crossing shapes from
underestimating speed and hiding `STM_014`.

The issue remains open for the requested SAME_INPUT corpus reruns. The `mdb-2712` and
`mdb-510` feed artifacts are not available in this workspace, so recovery counts are not
claimed here.

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

## Validation

- `cargo test --workspace --all-features` — green;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — green;
- `python3 -m unittest discover -s spec-audit -p 'test_*.py'` — green, no unexplained
  parity rows.

## Remaining acceptance evidence

- rerun SAME_INPUT `mdb-2712` and compare consecutive-stop speed findings;
- rerun SAME_INPUT `mdb-510` and confirm the Q19-style anomaly;
- verify the intentional `route_type=101` rail behavior on `mdb-2785`;
- record MobilityData parity counts and any intentional aggregation difference.
