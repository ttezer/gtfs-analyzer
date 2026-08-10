# Issue #122 — SHP_024 user-distance false-negative audit

## Decision

The implementation portion is complete. The confirmed `tdg-83134` miss was caused by
leading whitespace in the numeric `stops.txt` coordinates. K2 intentionally retains the
lexical error for `DQ_016`/STP evidence, but SHP_024 now recovers the trimmed numeric
payload for its geometry comparison.

The issue remains open for final corpus adjudication. The same-input replay recovered the
two pinned MobilityData examples, but the full feed still has an intentional count difference
that needs the planned corpus review.

## Same-input evidence

- Feed: `tdg-83134`
- Input SHA-256: `b25af1087a4bd3397419a07024d904979eacee1d339168017096304df321d2ac`
- MobilityData examples: `Lycée Dupuy` ≈ 139.4 m; `Delacroix` ≈ 165.6 m
- Before fix: `SHP_024 = 0`
- After fix: both stops emit `SHP_024` with `139.4m` and `165.6m`
- Full-feed replay after fix: Analyzer `SHP_024 = 69`; MobilityData `= 124`; remaining
  difference is not claimed as exact parity.

## Regression evidence

`crates/pipeline/tests/emit_proof.rs` covers:

- whitespace-bearing bus coordinates with the pinned 139 m/166 m mismatches;
- a boundary pair around the 100 m bus threshold;
- the same geometry on a rail route, where the 200 m rail threshold remains silent.

## Validation

- targeted SHP_024 regression tests — green;
- same-input `tdg-83134` replay — pinned two findings recovered;
- final workspace test/clippy/parity validation — pending after the remaining issue work.

## Remaining acceptance evidence

- complete the planned corpus rerun/adjudication for the remaining 69 vs 124 count difference;
- verify the intentional `tld-715` rail behavior against the final corpus artifact;
- record any remaining aggregation/interpolation differences in the parity evidence.
