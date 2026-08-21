# Contributing to GTFS Analyzer

Thanks for your interest in contributing! GTFS Analyzer is an open-source, fully
client-side GTFS validator: a Rust validation core compiled to WebAssembly, with a
TypeScript/Vite user interface. Everything runs in the browser — uploaded feeds are
never sent to a server.

## Prerequisites

- **Rust** with the **GNU toolchain** (not MSVC). On Windows, MSVC's `link.exe` is not
  used; install the GNU toolchain and MinGW:
  ```
  rustup toolchain install stable-x86_64-pc-windows-gnu
  rustup override set stable-x86_64-pc-windows-gnu   # this directory only
  ```
  Make sure a MinGW `gcc` linker is on your `PATH`.
- **Nightly Rust** with `rust-src` — required for the threaded WASM build (`build-std`):
  ```
  rustup toolchain install nightly --component rust-src
  rustup target add wasm32-unknown-unknown --toolchain nightly
  ```
- **wasm-pack** — `curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh`
- **Node.js** — a maintained LTS release (exact range in `ui/package.json` `engines`) and npm.

## Setup & build

```bash
cd ui
npm ci
npm run wasm:all   # builds ui/pkg (serial) + ui/pkg-threads (threaded)
npm run build      # tsc --noEmit && vite build  → ui/dist
npm run dev        # local dev server
```

> Note: `ui/pkg` and `ui/pkg-threads` are wasm-pack outputs and are **not** tracked in
> git. Run `npm run wasm:all` before `npm run build`, `npm run typecheck`, or `npm run dev`.

## Tests

```bash
cargo test --workspace          # Rust unit/integration tests
cd ui && npm test               # Vitest unit tests
cd ui && npm run test:coverage  # Vitest + coverage (gated in CI)
cd ui && npx playwright test    # E2E (needs a prior build)
```

Type checking runs as part of `npm run build` (`tsc --noEmit`). You can run it
standalone with `npm run typecheck` (requires the wasm packages to be built first).

## Project layout

- `crates/` — Rust workspace: `core`, `rules`, `pipeline`, `config`, `wasm`.
- `ui/src/` — TypeScript front-end.
- `docs/rules/` — per-rule documentation cards.
- `.github/workflows/` — CI (`ci.yml`) and Pages deploy (`deploy.yml`, builds from source).

## Conventions

- **Commits:** Conventional Commits style — `feat(...)`, `fix(...)`, `ci(...)`,
  `docs(...)`, `test(...)`, `build(...)`.
- **Tests location:** Prefer integration tests in `crates/<crate>/tests/`. When adding a
  test to `crates/<crate>/tests/integration.rs`, append it to the **end** of the file —
  rule cards reference test line numbers and an insertion in the middle can shift them.
- **Artifact drift check:** CI warns (non-blocking) when `crates/` or `ui/src` change
  without a corresponding `ui/dist` / `ui/pkg` rebuild. Production is always built from
  source by the deploy workflow, so this is only a reminder. Add `[skip-drift]` to a
  commit message to silence it for a deliberately artifact-free change.
- **Rules & i18n:** Adding/changing/removing a rule means updating the registry
  (`crates/rules/src/registry.rs`), its card in `docs/rules/`, and the locale files
  (`ui/src/locales/{en,tr,ja}.ts`). Locale parity is enforced by a test.
- **Formatting: this repository is deliberately not rustfmt-formatted.** `cargo fmt --check`
  fails, and that is the recorded decision rather than neglect — do not "fix" it, and do not
  add a formatting gate. Match the style of the code around you instead.

  Measured on 2026-08-09 before deciding: running `cargo fmt` touches **66 files** and adds
  **+18,505 / −6,910 lines, a net +11,595**. Almost all of that growth hits one pattern —
  the dense notice-emission calls that make up most of the validator:

  ```rust
  // today: the argument block reads as a table
  notices.push(make_k2_notice(
      &mut counter, "AGN_002", EntityType::Agency, entity_id.clone(), Some(&row_map),
      &file.name, Some(line), Some("agency_name"), Some(String::new()), None,
      "agency_name zorunludur.".to_string(), "agency_name alanını doldurun.",
  ));
  ```

  rustfmt puts each of those twelve arguments on its own line. `k6_analytics.rs` alone goes
  from 2,131 to 5,755 lines in the diff. The gain would be consistency we already get from
  reading the surrounding code; the cost is the shape that makes these emissions scannable.

  One thing worth recording because it is easy to assume otherwise: the aligned `r!(...)`
  tables in `registry.rs` are **not** at risk. rustfmt does not reformat them — the whole
  2,382-line file produces only 13 hunks (import order, one method chain, one blank line, a
  few `assert!` wraps). Protecting them was never a reason for or against this decision.

## Pull requests

1. Branch from `main`.
2. Keep changes focused; ensure `cargo test --workspace` and the UI tests pass.
3. Open a PR against `main`; CI must be green (drift check may warn, that's fine).

### Full MobilityDatabase audit

The full MobilityDatabase GTFS Schedule audit is intentionally **manual**; it is
not triggered automatically by a push or pull request. Run it from the Actions
page with `workflow_dispatch` after the final commit has been pushed.

Any pull request that changes `crates/pipeline/**` requires one successful manual
full-corpus audit before merge. The audit must be run against the final PR head,
and its run URL and result must be recorded in the PR. This requirement is in
addition to the normal CI checks; normal CI remains automatic.

## License

By contributing, you agree that your contributions are licensed under the
[MIT License](LICENSE).
