# Issue #91 — clippy gate audit

Date: 2026-08-09

The workspace was measured with `cargo clippy --workspace --all-targets`. The
remaining warnings were fixed rather than hidden behind a crate-wide allow:

- removed the unused accessibility report constant;
- applied safe `clamp`/`sort_by_key` and boolean simplifications;
- fixed documentation list/continuation lint and a needless return binding;
- used `from_ref` for the one-element slice clone;
- changed test fixture field assignments to struct initialization;
- renamed rule-code test functions to snake_case.

The CI job now runs:

```text
cargo clippy --workspace --all-targets -- -D warnings
```

This includes libraries, binaries, tests, and examples, so a warning cannot be
hidden by an earlier crate failure or by linting only library targets. No rule
metadata changed in this issue; `RULES.md`, `RULES.en.md`, and `RULES.ja.md`
therefore require no semantic update.
