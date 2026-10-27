# Kotlin/JVM SDK design

## Intent and why

The validation engine is already available from Rust, the CLI, the browser
(`gtfs-sdk` on npm, via WASM), and Python (`gtfs-analyzer` on PyPI, via PyO3).
There is no JVM binding. The immediate driver: the user wants to run rule
validation jobs over GTFS feeds from a Kotlin backend service, storing the
full findings (notices, reports, metrics — not just a pass/fail or score) for
each feed. Beyond that immediate need, this SDK is meant to join the public
lineup — published to Maven Central the way `gtfs-sdk` is on npm and
`gtfs-analyzer` is on PyPI — so any JVM app can depend on it, not just the
one backend that prompted it.

Scope is JVM/server-side only. No Android, no Kotlin Multiplatform, no
browser/JS interop (that's already covered by `gtfs-sdk`).

## Goals

- A Kotlin API that runs a full validation on a GTFS feed and returns
  **typed Kotlin data classes** (not raw JSON) mirroring the full result
  shape already defined in [`sdk/src/types.ts`](../../../sdk/src/types.ts):
  `Notice`, `ReportSet` (`R1Report`..`R9Report`), `FeedMetrics`, `NameIndex`,
  `ValidationResult`, `FatalError`, `EngineResult`.
- Same engine, same rules, same scores as the CLI/web/Rust/npm/Python — this
  is a binding, not a reimplementation. It calls the same
  `gtfs_pipeline::validate_bytes` entry point the Python crate already calls.
- Works out of the box on:
  - macOS, Apple Silicon (arm64) — local dev machine.
  - Linux x86_64 — container deployment target.
  - Linux arm64 — container deployment target (multi-arch builds).
- Published to Maven Central as a normal Gradle/Maven dependency. No local
  Rust toolchain required by consumers — native libraries ship prebuilt,
  bundled in the jar, one per platform, auto-selected at load time.
- A small, direct API surface for the batch-validation-job use case: hand it
  feed bytes (or a file), get back `EngineResult` (`Ok(ValidationResult)` or
  `Fatal(FatalError)`), store whatever subset of it you want.

## Non-goals

- Android / Kotlin Multiplatform targets. JVM-only for now.
- Windows native build. Not a stated requirement (dev machine is macOS,
  deployment is Linux containers); can be added later the same way the
  Python wheel matrix already covers it, without changing the Kotlin API.
- Streaming/progress callbacks (the WASM `ValidatorEngine`'s staged
  `onFileList`/`onStageDone` hooks). The backend use case is "submit a feed,
  get a full result back," not an interactive progress UI. Can be added
  later without a breaking change if a job UI ever needs it.
- Re-deriving the rule engine, config schema, or notice catalog in Kotlin —
  all of that stays server-side in Rust; Kotlin only models the *shape* of
  the JSON it gets back.
- Setting up the actual Maven Central / Sonatype Central Portal account and
  GPG signing key. That requires the maintainer's own account and secrets
  the way the existing PyPI/npm publish workflows already do — this design
  wires the CI job to use them, but creating the account and secrets is a
  manual step outside what gets built here.

## Anti-goals

(what would make this a failure even if it technically runs)

- A result model that drifts from `types.ts`/the Rust structs — e.g. a
  missing field, wrong nullability, or a severity/rule-class enum that's
  out of sync. The three existing bindings (npm, PyPI, crates.io) all read
  the same engine output; Kotlin must not become the one SDK that silently
  disagrees with the others on shape.
- Requiring consumers to install a Rust toolchain or hand-build a native
  library. The whole point of shipping to Maven Central is `implementation
  ("...")` and it just works.
- A binding that reimplements validation logic in Kotlin "for convenience."
  Kotlin must stay a thin FFI + deserialization layer, exactly as thin as
  the existing PyO3 binding.

## Constraints

- Single source of truth for engine logic stays in `gtfs-pipeline`/`gtfs-core`.
  The new crate adds zero validation logic.
- Single source of truth for the version number stays the workspace
  `Cargo.toml` (`[workspace.package] version`), same as every other binding;
  the Kotlin/Gradle version is derived from it, not hand-maintained.
- MIT license, matching the rest of the repo.
- Must not regress or duplicate the Python/npm build pipelines; it's a new,
  independent crate and a new, independent CI workflow.

## Decided (the how, as discussed)

**Rust side — `crates/jvm`** (new crate, `gtfs-analyzer-jvm`, `cdylib`,
`publish = false` like `crates/wasm`):

- Uses the `jni` crate (the `jni`-crate equivalent of what PyO3 is for
  Python) to expose one native function, matching the Python binding's
  signature almost exactly:

  ```text
  Java_..._GtfsAnalyzerNative_validate(
      feed: ByteArray,
      today: Int,            // YYYYMMDD, same convention as elsewhere
      configJson: String,    // "{}" default
      includeNameIndex: Boolean,
      lang: String,          // "en" default
  ): String                  // JSON — same serde_json::to_string(&result) as Python
  ```

- Internally: `merge_delta` → `validate_bytes` → optional translation →
  optional name-index stripping → `serde_json::to_string`. This is a
  line-for-line port of [`crates/python/src/lib.rs`](../../../crates/python/src/lib.rs)
  onto the `jni` crate instead of `pyo3`. No new engine logic.
- Output stays JSON across the FFI boundary (not JNI object graphs) — this
  is the cheapest, lowest-risk way to cross into Kotlin, keeps the Rust side
  nearly identical to the Python binding, and pushes all the "shape" work
  to one place: typed deserialization on the Kotlin side.

**Kotlin side — new top-level `kotlin/` Gradle project** (sibling to `sdk/`
and `python/`):

- Data classes mirroring `sdk/src/types.ts` 1:1, annotated with
  `kotlinx.serialization`'s `@Serializable`: `Severity`, `RuleClass`,
  `EntityType`, `Notice`, `ReportItem`, `R1Report`..`R9Report`, `ReportSet`,
  `FileInfo`, `FeedMetrics`, `NameIndex`, `PartialReport`, `ValidationResult`,
  `FatalError`, `ValidatorConfigDelta`.
- `EngineResult` is a sealed class (`Ok`/`Fatal`) with a small hand-written
  `KSerializer` to decode Rust's externally-tagged enum shape
  (`{"Ok": {...}}` / `{"Fatal": {...}}`) — the one place that needs custom
  serialization code instead of a derived one.
- Public API, shaped for the batch-job use case:

  ```kotlin
  object GtfsAnalyzer {
      fun validate(
          feed: ByteArray,
          today: LocalDate = LocalDate.now(),
          config: ValidatorConfigDelta = ValidatorConfigDelta(),
          includeNameIndex: Boolean = false,
          lang: String = "en",
      ): EngineResult

      fun validate(feedPath: Path, /* same params */): EngineResult
  }
  ```

- Native library loading: natives for each supported platform are bundled
  as jar resources at `natives/<os>-<arch>/libgtfs_jvm.{so,dylib}`. A loader
  resolves `os.name`/`os.arch` at first use, extracts the matching library
  to a temp file, and `System.load()`s it — the same resource-bundling
  pattern used by other JNI-backed JVM libraries (e.g. RocksDB's, SQLite
  JDBC's). Platforms shipped in v1: `darwin-aarch64`, `linux-x86_64`,
  `linux-aarch64`.

**CI / release — new `publish-kotlin.yml`**, modeled directly on
[`publish-python.yml`](../../../.github/workflows/publish-python.yml):

- `workflow_dispatch`, gated the same way on a `vX.Y.Z` release tag, so the
  Maven artifact comes from the same commit as every other ecosystem's
  release.
- Matrix build of the native library:
  - `macos-latest` (Apple Silicon runner) → `darwin-aarch64`
  - `ubuntu-latest` → `linux-x86_64` (glibc — confirmed; the deployment
    container is glibc-based, e.g. Debian/Ubuntu/`eclipse-temurin`, so no
    musl build leg is needed, unlike the manylinux/musllinux split the
    Python wheel ecosystem has to deal with)
  - `ubuntu-24.04-arm` → `linux-aarch64` (glibc)
- Assembly job downloads all three native-lib artifacts, puts them in the
  Gradle project's resources, builds the jar (+ sources + javadoc jars, as
  Central requires), signs, and publishes via the Central Portal — the
  Kotlin-side equivalent of the existing `publish` job that uploads to PyPI.
- Version check step mirrors the Python one: Gradle project version must
  equal the workspace `Cargo.toml` version and the release tag.

**Native library loading, read-only-filesystem tolerant.** Whether the
backend's container runs with a read-only root filesystem wasn't confirmed,
so the loader is designed to tolerate either:

1. Check a system property (`gtfs.analyzer.nativeDir`) for an
   operator-supplied, already-writable directory first.
2. Otherwise try `java.io.tmpdir` (the default, works on a normal writable
   container).
3. If extraction fails in both locations (e.g. a hardened
   `readOnlyRootFilesystem: true` deployment with no writable `/tmp`), throw
   a clear `UnsatisfiedLinkError`-wrapping exception naming the directories
   it tried and explaining the fix (mount a writable `emptyDir`/tmpfs volume
   and point `gtfs.analyzer.nativeDir` at it), rather than failing with an
   opaque linker error.

**Threading contract.** `validate()` is a plain blocking call — no
`suspend` wrapper in v1, since the backend's concurrency model (coroutine
vs. thread-pool) wasn't pinned down. This is documented explicitly in the
Kotlin API's KDoc: it blocks the calling thread for the duration of the
validation (typically well under a second per the engine's existing
benchmarks), so callers on a coroutine dispatcher should wrap it with
`withContext(Dispatchers.IO) { GtfsAnalyzer.validate(...) }` themselves. A
`suspend` convenience can be added later as a non-breaking addition if it
turns out to be worth it.

**Panic and infra-failure handling.** The native `validate` call is
wrapped in `std::panic::catch_unwind` on the Rust side — a panic unwinding
across the JNI boundary is undefined behavior and can crash the whole JVM,
which is unacceptable for a backend service running many validation jobs.
A caught panic is translated into a thrown Kotlin exception (a new
`GtfsEngineException`, via `JNIEnv::throw_new`), not folded into
`EngineResult`: `EngineResult`'s `Ok`/`Fatal` cases represent legitimate
engine-level outcomes (a parseable-but-broken feed), while a panic is a
bug. Other infra-level failures — a missing/unreadable `feedPath`, no
native library for the current platform — throw ordinary JVM exceptions
(`IOException`, `UnsatisfiedLinkError`) rather than being folded into
`EngineResult` either, consistent with that same distinction.

**Testing:**

- Rust: a couple of `#[test]`s in `crates/jvm` exercising the native
  function directly against a fixture feed (same spirit as the Python
  crate's tests, if any exist — otherwise new).
- Kotlin: unit tests for the hand-written `EngineResult` serializer against
  captured JSON fixtures (reuse fixtures already in the repo if suitable);
  one integration test that loads the native lib for the current platform
  and validates a small known-good fixture feed, asserting on `metrics`
  and `reports.r1.publishable` the way the npm SDK's `smoke-test.mjs` does.

## My calls (flag for review, not asked about directly)

- **Maven coordinates**: groupId `io.github.transittrack` (derived from the
  GitHub org, `TransitTrack`, the standard Central Portal namespace for a
  GitHub-hosted project without its own domain — still a guess, not
  confirmed). Artifact id is `gtfs-analyzer`, matching PyPI/crates — this
  was asked and confirmed directly: one consistent project name across
  ecosystems except npm, rather than parity with npm's `gtfs-sdk` naming.
- **Directory name**: top-level `kotlin/`, alongside `sdk/` (JS/TS) and
  `python/`.
- **Crate name**: `crates/jvm`, package name `gtfs-analyzer-jvm`.
- **No Windows target in v1** — not a stated requirement; can be added the
  same way the Python wheel matrix already covers Windows, without an API
  change.
- **No streaming/progress API in v1** — batch job use case doesn't need it.

## What's left to the builder

- Exact Gradle project structure/module layout (single module vs. separate
  `api`/`natives` modules).
- Exact resource-extraction/temp-file strategy for the native loader
  (e.g. extract-once-per-process vs. per-call, cleanup on JVM shutdown).
- Whether `ValidatorConfigDelta`'s ~30 numeric/string tuning fields are
  generated from `types.ts`/the Rust config struct or hand-transcribed —
  either is fine as long as it stays in sync; a small script (mirroring
  `sdk/scripts/check-version.mjs`'s role) that checks parity is a reasonable
  addition but not required for v1.
- Picking the actual `jni` crate version and minimum JVM target (suggest
  JDK 17+, in line with current backend Kotlin conventions, but not
  confirmed with the user).
- cibuildwheel-equivalent tooling for cross-compiling `linux-aarch64` if
  `ubuntu-24.04-arm` runners turn out to be unsuitable (e.g. GitHub Actions
  availability/cost) — fall back to `cross` + QEMU if needed.
