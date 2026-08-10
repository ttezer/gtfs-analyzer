# Issue #120 — Untimed stop chronology audit

## Decision

The implementation portion is complete. `STM_008` now retains the latest known
`departure_time` while walking a trip in `stop_sequence` order and compares every later
known `arrival_time` against it. Untimed/interpolated intermediate stops no longer hide a
real chronology reversal.

The issue remains open for the requested SAME_INPUT corpus reruns. No `mdb-276` or
`tld-5537` feed artifact is available in this workspace, so those counts are not claimed
here.

## Regression evidence

`crates/pipeline/tests/emit_proof.rs` covers:

- one untimed intermediate stop: one `STM_008` with the later CSV line and `seq_a/seq_b`;
- multiple untimed intermediate stops: one `STM_008`;
- monotonic interpolation: no `STM_008`;
- after-midnight rollover across an untimed stop: `STM_048` remains and `STM_008` is not
  duplicated after service-day normalization.

The adjacent timed-row behavior remains covered by the existing non-midnight regression.
The speed/zero-duration checks continue to use physical adjacent pairs; only chronology
uses the carried previous departure state.

## Validation

- `cargo test --workspace --all-features` — green;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — green;
- `python3 -m unittest discover -s spec-audit -p 'test_*.py'` — green, no unexplained
  parity rows.

## Remaining acceptance evidence

- rerun SAME_INPUT `mdb-276` and compare the recovered chronology findings;
- rerun SAME_INPUT `tld-5537` and confirm the recovered finding;
- record any intentional aggregation difference in the parity evidence.
