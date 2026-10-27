# Kotlin/JVM SDK Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a Kotlin/JVM binding (`crates/jvm` in Rust + a new `kotlin/` Gradle project) that lets a Kotlin backend run the same GTFS validation engine as the CLI/web/Rust/npm/Python SDKs, returning fully-typed Kotlin data classes, and publish it to Maven Central.

**Architecture:** A new Rust crate (`crates/jvm`, cdylib) exposes one JNI-callable function that is a line-for-line port of the existing PyO3 binding (`crates/python/src/lib.rs`): `merge_delta` → `validate_bytes` → optional translation → optional name-index stripping → `serde_json::to_string`. The call is wrapped in `catch_unwind` so an engine panic throws a Kotlin exception instead of crashing the JVM. On the Kotlin side, a new Gradle project (`kotlin/`) bundles prebuilt natives for three platforms as jar resources, loads the right one at runtime, and deserializes the JSON with `kotlinx.serialization` into data classes that mirror `sdk/src/types.ts` 1:1.

**Tech Stack:** Rust (`jni` crate ~0.21), Kotlin 2.0.21 / Gradle (Kotlin DSL), `kotlinx-serialization-json` 1.7.3, JDK 17 toolchain.

**Spec:** [docs/superpowers/specs/2026-10-27-kotlin-sdk-design.md](../specs/2026-10-27-kotlin-sdk-design.md)

## Global Constraints

- Version: the Gradle project version is *derived* from the workspace `Cargo.toml`'s `[workspace.package] version` (currently `0.16.0`) at build time — never hand-maintained as a separate number.
- License: MIT, matching the rest of the repo.
- Zero new validation logic in Rust: `crates/jvm` only calls `gtfs_pipeline::validate_bytes` and `gtfs_config::merge_delta`, exactly as `crates/python` does.
- Native platforms shipped in v1: `darwin-aarch64`, `linux-x86_64`, `linux-aarch64` — all glibc (confirmed: the deployment container is glibc-based, so no musl build leg is needed). No Windows in v1.
- The `ValidationResult`/`EngineResult` shape must exactly mirror [`sdk/src/types.ts`](../../../sdk/src/types.ts) — no drift between SDKs.
- No `suspend` API in v1 — `GtfsAnalyzer.validate(...)` is a plain blocking call; its KDoc documents the threading contract (callers on a coroutine dispatcher wrap it themselves with `withContext(Dispatchers.IO)`).
- A panic inside the engine must never crash the JVM: it is caught in Rust (`catch_unwind`) and surfaces in Kotlin as a thrown `GtfsEngineException`, never folded into `EngineResult`.
- Infra-level failures (unreadable `feedPath`, no native library for the current platform) throw ordinary JVM exceptions (`IOException`, a dedicated `UnsupportedPlatformException`), not `EngineResult`.
- Maven coordinates: groupId `io.github.transittrack` (unconfirmed guess — flag before the first real publish), artifactId `gtfs-analyzer` (confirmed with the user, matches PyPI/crates naming rather than npm's `gtfs-sdk`).
- The native-library resource loader must try a writable directory in this order: `-Dgtfs.analyzer.nativeDir` system property, then `java.io.tmpdir`; if neither is writable, it throws naming both attempted paths and the fix (mount a writable volume and point the system property at it).

## Review Focus

- A `ValidatorConfigDelta` carrying an unrecognized key must surface as a clear `IllegalArgumentException` from `GtfsAnalyzer.validate(...)`, not an opaque JSON error or a silent no-op — covered in Task 10.
- A genuinely corrupt/unreadable ZIP must deserialize into `EngineResult.Fatal(...)`, not throw — covered in Task 10 (end-to-end, not just the hand-written JSON fixture in Task 6).
- Concurrent calls to `GtfsAnalyzer.validate(...)` from multiple threads must not crash or double-load the native library — covered in Task 10.
- A `feedPath` pointing at a missing/unreadable file must throw `IOException` with a clear message, not propagate a raw `NoSuchFileException` or NPE — covered in Task 9.
- Running on an unsupported OS/arch combination must throw a clear `UnsupportedPlatformException` naming what was detected, not an opaque `UnsatisfiedLinkError` — covered in Task 8.

---

## File Structure

```
crates/jvm/
  Cargo.toml
  src/lib.rs                 # validate_to_json, ValidateJvmOutcome, JNI extern fn

kotlin/
  settings.gradle.kts
  build.gradle.kts
  src/main/kotlin/io/github/transittrack/gtfsanalyzer/
    Types.kt                 # Notice, ReportSet, FeedMetrics, NameIndex, ValidationResult, FatalError, ...
    ValidatorConfigDelta.kt
    JsonCodec.kt              # shared kotlinx.serialization Json instance
    EngineResult.kt           # sealed class + custom KSerializer
    GtfsAnalyzerPlatform.kt   # platform detection + native library loader
    GtfsAnalyzerNative.kt     # `external fun` JNI declarations
    GtfsEngineException.kt
    GtfsAnalyzer.kt           # public API
  src/test/kotlin/io/github/transittrack/gtfsanalyzer/
    TypesTest.kt
    ValidatorConfigDeltaTest.kt
    EngineResultTest.kt
    GtfsAnalyzerPlatformTest.kt
    GtfsAnalyzerIntegrationTest.kt

.github/workflows/publish-kotlin.yml
```

---

### Task 1: Rust — core `validate_to_json` function

**Files:**
- Create: `crates/jvm/Cargo.toml`
- Create: `crates/jvm/src/lib.rs`
- Modify: `Cargo.toml:2-9` (add `"crates/jvm"` to `[workspace] members`)

**Interfaces:**
- Produces: `pub fn validate_to_json(feed: &[u8], today: u32, config_json: &str, include_name_index: bool, lang: &str) -> Result<String, String>` in `crates/jvm/src/lib.rs`, used by Task 2.

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "gtfs-analyzer-jvm"
version.workspace = true
edition = "2021"
license.workspace = true
description = "JVM/Kotlin bindings for the GTFS Analyzer validation engine."
repository = "https://github.com/TransitTrack/gtfs-analyzer"
homepage = "https://github.com/TransitTrack/gtfs-analyzer"
publish = false

[lib]
name = "gtfs_jvm"
crate-type = ["cdylib"]

[dependencies]
gtfs-config = { path = "../config" }
gtfs-core = { path = "../core", features = ["i18n"] }
gtfs-pipeline = { path = "../pipeline" }
jni = "0.21"
serde_json = { workspace = true }
```

- [ ] **Step 2: Add `"crates/jvm"` to the root `[workspace] members` list** in `Cargo.toml`, alongside `"crates/python"`.

- [ ] **Step 3: Write the failing test** (in `crates/jvm/src/lib.rs`, `#[cfg(test)] mod tests`) — reuse the exact minimal-feed pattern from `crates/pipeline/tests/integration.rs:1-35` (`make_zip`, `AGENCY`/`STOPS`/`ROUTES`/`TRIPS`/`STOP_TIMES`/`CALENDAR` byte strings, `TODAY = 20_260_515`), transcribed locally into this crate (no cross-crate test-fixture dependency):

```rust
#[test]
fn validate_to_json_returns_ok_envelope_for_a_clean_feed() {
    let zip = make_zip(&base_files());
    let json = validate_to_json(&zip, TODAY, "{}", false, "en").unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(value.get("Ok").is_some(), "expected an {{\"Ok\": ...}} envelope, got: {json}");
    let metrics = &value["Ok"]["metrics"];
    assert_eq!(metrics["stop_count"], 2);
    assert_eq!(metrics["route_count"], 1);
    assert_eq!(metrics["trip_count"], 1);
}

#[test]
fn validate_to_json_rejects_unknown_config_key() {
    let zip = make_zip(&base_files());
    let err = validate_to_json(&zip, TODAY, r#"{"not_a_real_key": 1}"#, false, "en").unwrap_err();
    assert!(err.contains("not_a_real_key"), "error should name the bad key: {err}");
}

#[test]
fn validate_to_json_rejects_unknown_lang() {
    let zip = make_zip(&base_files());
    let err = validate_to_json(&zip, TODAY, "{}", false, "xx").unwrap_err();
    assert!(!err.is_empty());
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cargo test -p gtfs-analyzer-jvm`
Expected: FAIL to compile — `validate_to_json`, `make_zip`, `base_files` not defined.

- [ ] **Step 5: Implement `make_zip`/`base_files`/`TODAY` test helpers** (copy verbatim from `crates/pipeline/tests/integration.rs:1-35`), and implement:

```rust
pub fn validate_to_json(
    feed: &[u8],
    today: u32,
    config_json: &str,
    include_name_index: bool,
    lang: &str,
) -> Result<String, String>
```

Body: port `crates/python/src/lib.rs`'s `validate` function exactly, replacing `PyResult`/`PyValueError`/`PyRuntimeError` with plain `Result<String, String>` (`Err` for the lang-parse and config-merge failures, translating `Translator`/`NameIndex` the same way, ending in `serde_json::to_string(&result).map_err(|e| e.to_string())`).

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p gtfs-analyzer-jvm`
Expected: PASS (3 tests)

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/jvm
git commit -m "feat(jvm): add gtfs-analyzer-jvm crate with validate_to_json"
```

---

### Task 2: Rust — panic-safe wrapper

**Files:**
- Modify: `crates/jvm/src/lib.rs`

**Interfaces:**
- Consumes: `validate_to_json(&[u8], u32, &str, bool, &str) -> Result<String, String>` (Task 1).
- Produces: `pub enum ValidateJvmOutcome { Json(String), InvalidArgument(String), Panicked(String) }` and `pub fn validate_guarded(feed: &[u8], today: u32, config_json: &str, include_name_index: bool, lang: &str) -> ValidateJvmOutcome`, used by Task 3.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn validate_guarded_returns_json_on_success() {
    let zip = make_zip(&base_files());
    match validate_guarded(&zip, TODAY, "{}", false, "en") {
        ValidateJvmOutcome::Json(_) => {}
        other => panic!("expected Json, got {other:?}"),
    }
}

#[test]
fn validate_guarded_returns_invalid_argument_on_bad_config() {
    let zip = make_zip(&base_files());
    match validate_guarded(&zip, TODAY, r#"{"not_a_real_key": 1}"#, false, "en") {
        ValidateJvmOutcome::InvalidArgument(msg) => assert!(msg.contains("not_a_real_key")),
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

#[test]
fn catch_unwind_to_outcome_converts_panic_into_panicked_variant() {
    let outcome = catch_unwind_to_outcome(|| -> Result<String, String> {
        panic!("boom");
    });
    match outcome {
        ValidateJvmOutcome::Panicked(msg) => assert!(msg.contains("boom")),
        other => panic!("expected Panicked, got {other:?}"),
    }
}
```

(`ValidateJvmOutcome` needs `#[derive(Debug)]` for the `{:?}` in the panic branches above.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p gtfs-analyzer-jvm`
Expected: FAIL to compile — `ValidateJvmOutcome`, `validate_guarded`, `catch_unwind_to_outcome` not defined.

- [ ] **Step 3: Implement**

```rust
#[derive(Debug)]
pub enum ValidateJvmOutcome {
    Json(String),
    InvalidArgument(String),
    Panicked(String),
}

fn catch_unwind_to_outcome<F>(f: F) -> ValidateJvmOutcome
where
    F: FnOnce() -> Result<String, String> + std::panic::UnwindSafe,
{
    match std::panic::catch_unwind(f) {
        Ok(Ok(json)) => ValidateJvmOutcome::Json(json),
        Ok(Err(msg)) => ValidateJvmOutcome::InvalidArgument(msg),
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic payload".to_string());
            ValidateJvmOutcome::Panicked(msg)
        }
    }
}

pub fn validate_guarded(
    feed: &[u8],
    today: u32,
    config_json: &str,
    include_name_index: bool,
    lang: &str,
) -> ValidateJvmOutcome {
    catch_unwind_to_outcome(|| validate_to_json(feed, today, config_json, include_name_index, lang))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p gtfs-analyzer-jvm`
Expected: PASS (6 tests total)

- [ ] **Step 5: Commit**

```bash
git add crates/jvm/src/lib.rs
git commit -m "feat(jvm): catch panics instead of unwinding across the JNI boundary"
```

---

### Task 3: Rust — JNI entry point

**Files:**
- Modify: `crates/jvm/src/lib.rs`
- Modify: `crates/jvm/Cargo.toml` (dev-dependencies, if needed for JNI test scaffolding — none required; this task has no Rust-side automated test, see Step 3 below)

**Interfaces:**
- Consumes: `validate_guarded(...)  -> ValidateJvmOutcome` (Task 2).
- Produces: the exported native symbol `Java_io_github_transittrack_gtfsanalyzer_GtfsAnalyzerNative_validate`, consumed by Kotlin's `GtfsAnalyzerNative.validate(...)` (Task 8).

- [ ] **Step 1: Implement the JNI extern function**

```rust
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jboolean, jint};
use jni::JNIEnv;

#[no_mangle]
pub extern "system" fn Java_io_github_transittrack_gtfsanalyzer_GtfsAnalyzerNative_validate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    feed: JByteArray<'local>,
    today: jint,
    config_json: JString<'local>,
    include_name_index: jboolean,
    lang: JString<'local>,
) -> JString<'local> {
    // elements() to Vec<u8>, GetStringUTFChars via env.get_string(...), bool via != 0.
    // Dispatch to validate_guarded(...), then:
    //   Json(s)            -> env.new_string(s).unwrap()
    //   InvalidArgument(m) -> env.throw_new("java/lang/IllegalArgumentException", m), return empty string
    //   Panicked(m)        -> env.throw_new(
    //       "io/github/transittrack/gtfsanalyzer/GtfsEngineException", m), return empty string
}
```

One line on the approach: use `env.convert_byte_array(&feed)` for the feed bytes and `env.get_string(&config_json)`/`env.get_string(&lang)` for the two string args (both already UTF-8 by Java string contract); after calling `env.throw_new(...)`, JNI convention requires still returning a value of the declared return type — return `env.new_string("").unwrap()` in both error branches, since the pending exception is what the JVM actually observes on return to Kotlin.

- [ ] **Step 2: Build it**

Run: `cargo build -p gtfs-analyzer-jvm`
Expected: builds successfully, producing `target/debug/libgtfs_jvm.{so,dylib}`.

- [ ] **Step 3: Run the existing unit tests to confirm nothing broke**

Run: `cargo test -p gtfs-analyzer-jvm`
Expected: PASS (6 tests — this task adds no new Rust-side test, since exercising a `Java_...` symbol requires a real JVM; it is exercised end-to-end in Task 10's Kotlin integration test instead).

- [ ] **Step 4: Commit**

```bash
git add crates/jvm/src/lib.rs
git commit -m "feat(jvm): expose validate via JNI"
```

---

### Task 4: Kotlin — Gradle project skeleton, derived version, native-lib tasks

**Files:**
- Create: `kotlin/settings.gradle.kts`
- Create: `kotlin/build.gradle.kts`
- Create: `kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/.gitkeep` (placeholder; removed once Task 5 adds real sources)
- Test: `kotlin/src/test/kotlin/io/github/transittrack/gtfsanalyzer/VersionTest.kt`

**Interfaces:**
- Produces: Gradle tasks `copyNativeLib` (params: `-PnativeLibFile=<path>`, `-PplatformTag=<tag>`) and `buildNativeForCurrentPlatform` (no params — builds + installs the current host's native lib), both consumed by Task 10 (integration test) and Task 11 (CI). Produces a generated Kotlin constant `io.github.transittrack.gtfsanalyzer.SDK_VERSION: String`, consumed by nothing yet but asserted against directly in this task's test.

- [ ] **Step 1: Write `settings.gradle.kts`**

```kotlin
rootProject.name = "gtfs-analyzer-kotlin"
```

- [ ] **Step 2: Write the failing test**

```kotlin
class VersionTest {
    @Test
    fun sdkVersionMatchesWorkspaceCargoToml() {
        val cargoToml = File("../Cargo.toml").readText()
        val expected = Regex("""\[workspace\.package][^\[]*?version\s*=\s*"([^"]+)"""", RegexOption.DOT_MATCHES_ALL)
            .find(cargoToml)!!.groupValues[1]
        assertEquals(expected, SDK_VERSION)
        assertTrue(SDK_VERSION.matches(Regex("""\d+\.\d+\.\d+""")))
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cd kotlin && ./gradlew test`
Expected: FAIL — project doesn't build yet (no `build.gradle.kts`, no `SDK_VERSION`).

- [ ] **Step 4: Write `build.gradle.kts`**

Plugins: `kotlin("jvm") version "2.0.21"`, `kotlin("plugin.serialization") version "2.0.21"`, `maven-publish`, `signing`. JVM toolchain 17. `dependencies { implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3"); testImplementation(kotlin("test")) }`.

Version derivation (evaluated at configure time, same regex as the test above, read from `rootProject.projectDir.resolve("../Cargo.toml")`), assigned to `project.version`.

A generated-sources task that writes `io/github/transittrack/gtfsanalyzer/Version.kt` (containing `const val SDK_VERSION: String = "<the derived version>"`) into `build/generated/version/`, registered as a `kotlin.sourceSets.main.kotlin.srcDir(...)`, wired as a dependency of `compileKotlin`.

```kotlin
tasks.register<Copy>("copyNativeLib") {
    val libFile = (project.findProperty("nativeLibFile") as String?)
        ?: error("usage: -PnativeLibFile=<path> -PplatformTag=<tag>")
    val tag = (project.findProperty("platformTag") as String?)
        ?: error("usage: -PnativeLibFile=<path> -PplatformTag=<tag>")
    from(libFile)
    into(layout.projectDirectory.dir("src/main/resources/natives/$tag"))
}

tasks.register("buildNativeForCurrentPlatform") {
    doLast {
        val repoRoot = rootDir.parentFile // kotlin/ is a direct child of the repo root
        exec { workingDir = repoRoot; commandLine("cargo", "build", "--release", "-p", "gtfs-analyzer-jvm") }
        val os = org.gradle.internal.os.OperatingSystem.current()
        val arch = System.getProperty("os.arch")
        val tag = when {
            os.isMacOsX && arch.contains("aarch64") -> "darwin-aarch64"
            os.isLinux && arch.contains("aarch64") -> "linux-aarch64"
            os.isLinux -> "linux-x86_64"
            else -> error("unsupported host platform for local native build: $os/$arch")
        }
        val ext = if (os.isMacOsX) "dylib" else "so"
        val built = repoRoot.resolve("target/release/libgtfs_jvm.$ext")
        copy {
            from(built)
            into(layout.projectDirectory.dir("src/main/resources/natives/$tag"))
        }
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd kotlin && ./gradlew test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add kotlin
git commit -m "feat(kotlin): add Gradle project skeleton with Cargo-derived version"
```

---

### Task 5: Kotlin — result/report data classes (`Types.kt`)

**Files:**
- Create: `kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/Types.kt`
- Test: `kotlin/src/test/kotlin/io/github/transittrack/gtfsanalyzer/TypesTest.kt`

**Interfaces:**
- Produces: every type in `sdk/src/types.ts` lines 1-147 (`Severity`, `RuleClass`, `EntityType`, `Notice`, `ReportId`, `R9Label`, `ReportItem`, `R1Report`..`R9Report`, `R9Item`, `ReportSet`, `FileInfo`, `FeedMetrics`, `NameIndex`, `PartialReport`, `ValidationResult`), plus `FatalCode`/`FatalError` (from `sdk/src/types.ts` `FatalCode`/`FatalError`, same file). All consumed by Task 6 (`EngineResult`) and Task 9 (public API return type).

- [ ] **Step 1: Write the failing test** — one representative fixture per nontrivial type, asserting field-by-field equality after `Json.decodeFromString`:

```kotlin
class TypesTest {
    @Test
    fun decodesANoticeWithAllFieldsPresent() {
        val json = """
            {"id":"n1","rule_id":"ARC_001","severity":"CRITICAL","rule_class":"SPEC",
             "entity_type":"Feed","entity_id":null,"scope_key":null,"file":"stops.txt",
             "line":12,"field":"stop_lat","observed_value":"91.0","expected_value":"<=90",
             "details":{"k":"v"},"title":"t","message":"m","remediation":"r",
             "blocks":["ARC_002"],"base_effort":1,"service_id":null}
        """.trimIndent()
        val notice = Json.decodeFromString<Notice>(json)
        assertEquals("n1", notice.id)
        assertEquals(Severity.CRITICAL, notice.severity)
        assertEquals(RuleClass.SPEC, notice.ruleClass)
        assertEquals(EntityType.Feed, notice.entityType)
        assertEquals(12L, notice.line)
        assertEquals(listOf("ARC_002"), notice.blocks)
        assertEquals(1, notice.baseEffort)
        assertNull(notice.serviceId)
    }

    @Test
    fun decodesAFatalError() {
        val json = """{"code":"ZipUnreadable","message":"bad zip"}"""
        val fatal = Json.decodeFromString<FatalError>(json)
        assertEquals(FatalCode.ZipUnreadable, fatal.code)
        assertEquals("bad zip", fatal.message)
    }

    @Test
    fun decodesAnR5ReportWithAllScores() {
        val json = """{"score":90.0,"pub_score":95.0,"spec_score":100.0,"interop_score":80.0,"quality_score":70.0,"analytics_score":60.0}"""
        val r5 = Json.decodeFromString<R5Report>(json)
        assertEquals(90.0, r5.score)
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd kotlin && ./gradlew test --tests TypesTest`
Expected: FAIL to compile — none of these classes exist yet.

- [ ] **Step 3: Implement `Types.kt`**

Transcribe every interface in `sdk/src/types.ts` (lines 1-147) into a `@Serializable` Kotlin `enum class` or `data class`, one-to-one:
- TS `string | null` → Kotlin `String?` (default `null` where TS marks it optional with `?`).
- TS `number` → `Int` for count/id-like fields, `Long` for `line` (can exceed `Int` on huge files) and the `*_date` epoch-day fields, `Double` for all `*_score`/`avg_daily_trips` fields — match each field's unit to the Rust struct it comes from (`crates/core/src/notice.rs`, and the pipeline's report/metrics structs under `crates/pipeline/src` / `crates/core/src`) wherever `types.ts` alone leaves `number` ambiguous between `Int`/`Long`/`Double`.
- TS enums (`Severity`, `RuleClass`, `EntityType`, `FatalCode`) → Kotlin `@Serializable enum class` with `@SerialName("...")` matching the exact Rust variant spelling (e.g. `EntityType.Feed`, not `EntityType.FEED` — the wire value is `"Feed"`, not `"FEED"`).
- snake_case JSON field → camelCase Kotlin property with `@SerialName("snake_case_name")`.
- `Record<string, string>` → `Map<String, String>`; `Record<string, string[]>` → `Map<String, List<String>>`; `Record<string, [number, number]>` (coordinate pairs) → `Map<String, List<Double>>`.
- Every field that is optional in TS gets a Kotlin default (`= null` for nullable fields, `= false`/`= emptyMap()` for the `?`-suffixed non-nullable ones like `coverage_complete?: boolean`), so partial/older-engine JSON missing a field still decodes (mirrors the TS comment "Eklemeli alan: eski motor sürümleri taşımaz").

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd kotlin && ./gradlew test --tests TypesTest`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/Types.kt kotlin/src/test
git commit -m "feat(kotlin): add data classes mirroring sdk/src/types.ts"
```

---

### Task 6: Kotlin — `EngineResult` sealed class + custom serializer + `JsonCodec`

**Files:**
- Create: `kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/EngineResult.kt`
- Create: `kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/JsonCodec.kt`
- Test: `kotlin/src/test/kotlin/io/github/transittrack/gtfsanalyzer/EngineResultTest.kt`

**Interfaces:**
- Consumes: `ValidationResult`, `FatalError` (Task 5).
- Produces: `sealed class EngineResult { data class Ok(val result: ValidationResult) : EngineResult(); data class Fatal(val error: FatalError) : EngineResult() }` with a registered `KSerializer`, and `val EngineJson: Json` (the shared, configured instance), both consumed by Task 9.

- [ ] **Step 1: Write the failing tests**

```kotlin
class EngineResultTest {
    @Test
    fun decodesOkEnvelope() {
        val json = """{"Ok": $MINIMAL_VALIDATION_RESULT_JSON}"""
        val result = EngineJson.decodeFromString<EngineResult>(json)
        assertIs<EngineResult.Ok>(result)
    }

    @Test
    fun decodesFatalEnvelope() {
        val json = """{"Fatal": {"code":"ZipUnreadable","message":"bad zip"}}"""
        val result = EngineJson.decodeFromString<EngineResult>(json)
        assertIs<EngineResult.Fatal>(result)
        assertEquals(FatalCode.ZipUnreadable, (result as EngineResult.Fatal).error.code)
    }

    @Test
    fun roundTripsFatalEnvelope() {
        val original = EngineResult.Fatal(FatalError(FatalCode.ZipUnreadable, "bad zip"))
        val encoded = EngineJson.encodeToString(original)
        val decoded = EngineJson.decodeFromString<EngineResult>(encoded)
        assertEquals(original, decoded)
    }
}
```

(`MINIMAL_VALIDATION_RESULT_JSON` is a `const val` fixture string in the test file with the smallest legal `ValidationResult` — `notices: []`, a zeroed `reports`/`metrics`/`name_index`, `capped_totals: {}`; write it by hand from Task 5's `Types.kt` field list, this is the "exact copy the spec fixes" the fixture is for.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd kotlin && ./gradlew test --tests EngineResultTest`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

`JsonCodec.kt`:
```kotlin
val EngineJson = Json {
    ignoreUnknownKeys = true   // forward-compat: an older SDK must not choke on a newer engine's new fields
    encodeDefaults = false     // used for ValidatorConfigDelta in Task 7: omit unset fields entirely
    explicitNulls = false
}
```

`EngineResult.kt`: a custom `KSerializer<EngineResult>` (via `JsonContentPolymorphicSerializer<EngineResult>` or a hand-written `KSerializer` reading the single top-level key) that inspects the decoded `JsonObject`'s one key (`"Ok"` or `"Fatal"`) and delegates to `ValidationResult.serializer()`/`FatalError.serializer()` accordingly; `serialize` writes the same `{"Ok": ...}`/`{"Fatal": ...}` shape back out.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd kotlin && ./gradlew test --tests EngineResultTest`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/EngineResult.kt kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/JsonCodec.kt kotlin/src/test
git commit -m "feat(kotlin): decode the engine's Ok/Fatal envelope into EngineResult"
```

---

### Task 7: Kotlin — `ValidatorConfigDelta`

**Files:**
- Create: `kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/ValidatorConfigDelta.kt`
- Test: `kotlin/src/test/kotlin/io/github/transittrack/gtfsanalyzer/ValidatorConfigDeltaTest.kt`

**Interfaces:**
- Produces: `@Serializable data class ValidatorConfigDelta(...)`, consumed by Task 9 (`GtfsAnalyzer.validate`'s `config` parameter).

- [ ] **Step 1: Write the failing test**

```kotlin
class ValidatorConfigDeltaTest {
    @Test
    fun encodesOnlyTheFieldsThatWereSet() {
        val delta = ValidatorConfigDelta(maxSpeedBusKmh = 80.0, disabledRuleIds = listOf("DQ_004"))
        val json = EngineJson.encodeToString(delta)
        val decoded = EngineJson.parseToJsonElement(json).jsonObject
        assertEquals(setOf("max_speed_bus_kmh", "disabled_rule_ids"), decoded.keys)
        assertEquals(80.0, decoded["max_speed_bus_kmh"]!!.jsonPrimitive.double)
    }

    @Test
    fun defaultInstanceEncodesToEmptyObject() {
        assertEquals("{}", EngineJson.encodeToString(ValidatorConfigDelta()))
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd kotlin && ./gradlew test --tests ValidatorConfigDeltaTest`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

Transcribe every field of `ValidatorConfigDelta` from `sdk/src/types.ts` lines 150-220 into `@Serializable data class ValidatorConfigDelta(...)`, all nullable with `= null` defaults, `@SerialName` matching the TS/JSON snake_case name exactly (e.g. `val maxSpeedBusKmh: Double? = null` with `@SerialName("max_speed_bus_kmh")`); `sourceUrl: String?`, `gtfsJpProfile: String?`, `ruralRouteIds: List<String>? = null`, `calendarOverrideRules: List<CalendarOverrideRule>? = null` (new small `@Serializable data class CalendarOverrideRule(...)` per `types.ts`'s `CalendarOverrideRule`, in the same file), `disabledRuleIds`/`disabledRuleIdsGtfsJp: List<String>? = null`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd kotlin && ./gradlew test --tests ValidatorConfigDeltaTest`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/ValidatorConfigDelta.kt kotlin/src/test
git commit -m "feat(kotlin): add ValidatorConfigDelta mirroring the engine config schema"
```

---

### Task 8: Kotlin — platform detection and native library loader

**Files:**
- Create: `kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/GtfsAnalyzerPlatform.kt`
- Create: `kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/GtfsEngineException.kt`
- Test: `kotlin/src/test/kotlin/io/github/transittrack/gtfsanalyzer/GtfsAnalyzerPlatformTest.kt`

**Interfaces:**
- Produces: `fun detectPlatformTag(osName: String = System.getProperty("os.name"), osArch: String = System.getProperty("os.arch")): String` (throws `UnsupportedPlatformException` for anything outside `darwin-aarch64`/`linux-x86_64`/`linux-aarch64`); `fun resolveWritableNativeLibDir(candidates: List<Path>, isWritable: (Path) -> Boolean = Files::isWritable): Path` (throws `NativeLibraryLoadException` naming every candidate it tried, otherwise returns the first writable one); `fun loadNativeLibrary()` (impure: calls the two above with real system properties, extracts the matching jar resource, `System.load`s it, memoized so repeat calls are no-ops). `class UnsupportedPlatformException`, `class NativeLibraryLoadException`, `class GtfsEngineException` — all consumed by Task 9.

- [ ] **Step 1: Write the failing tests**

```kotlin
class GtfsAnalyzerPlatformTest {
    @Test
    fun detectsDarwinAarch64() {
        assertEquals("darwin-aarch64", detectPlatformTag("Mac OS X", "aarch64"))
    }

    @Test
    fun detectsLinuxX86_64() {
        assertEquals("linux-x86_64", detectPlatformTag("Linux", "amd64"))
    }

    @Test
    fun detectsLinuxAarch64() {
        assertEquals("linux-aarch64", detectPlatformTag("Linux", "aarch64"))
    }

    @Test
    fun rejectsUnsupportedPlatform() {
        val ex = assertThrows<UnsupportedPlatformException> { detectPlatformTag("Windows 11", "amd64") }
        assertTrue(ex.message!!.contains("Windows 11"))
    }

    @Test
    fun resolveWritableNativeLibDirThrowsNamingEveryCandidateWhenNoneAreWritable() {
        val candidates = listOf(Path.of("/a"), Path.of("/b"))
        val ex = assertThrows<NativeLibraryLoadException> {
            resolveWritableNativeLibDir(candidates, isWritable = { false })
        }
        assertTrue(ex.message!!.contains("/a") && ex.message!!.contains("/b"))
    }

    @Test
    fun resolveWritableNativeLibDirReturnsFirstWritableCandidate() {
        val candidates = listOf(Path.of("/a"), Path.of("/b"))
        assertEquals(Path.of("/b"), resolveWritableNativeLibDir(candidates, isWritable = { it == Path.of("/b") }))
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd kotlin && ./gradlew test --tests GtfsAnalyzerPlatformTest`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

`detectPlatformTag`: lowercase both inputs; `osName.contains("mac")` + `osArch.contains("aarch64")` → `"darwin-aarch64"`; `osName.contains("linux")` + (`osArch.contains("amd64")` or `osArch.contains("x86_64")`) → `"linux-x86_64"`; `osName.contains("linux")` + `osArch.contains("aarch64")` → `"linux-aarch64"`; else throw `UnsupportedPlatformException("unsupported platform: osName=$osName osArch=$osArch; supported: darwin-aarch64, linux-x86_64, linux-aarch64")`.

`resolveWritableNativeLibDir`: `candidates.firstOrNull(isWritable) ?: throw NativeLibraryLoadException("no writable directory for the native library among: ${candidates.joinToString()}; set -Dgtfs.analyzer.nativeDir=<a writable dir> or mount a writable volume")`.

`loadNativeLibrary()`: `by lazy` (default `LazyThreadSafetyMode.SYNCHRONIZED`, satisfying the "safe from multiple threads" requirement) — resolve tag via `detectPlatformTag()`, locate classpath resource `/natives/$tag/libgtfs_jvm.${if (tag.startsWith("darwin")) "dylib" else "so"}`, candidates = `[System.getProperty("gtfs.analyzer.nativeDir"), System.getProperty("java.io.tmpdir")].filterNotNull().map(Path::of)`, pick via `resolveWritableNativeLibDir`, write resource bytes to `<dir>/libgtfs_jvm-<version>.<ext>`, `System.load(path.toString())`.

`GtfsEngineException(message: String) : RuntimeException(message)` — thrown by `GtfsAnalyzer` (Task 9) when the native call reports a caught panic.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd kotlin && ./gradlew test --tests GtfsAnalyzerPlatformTest`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/GtfsAnalyzerPlatform.kt kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/GtfsEngineException.kt kotlin/src/test
git commit -m "feat(kotlin): add platform detection and native library loader"
```

---

### Task 9: Kotlin — native method declarations + public `GtfsAnalyzer` API

**Files:**
- Create: `kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/GtfsAnalyzerNative.kt`
- Create: `kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/GtfsAnalyzer.kt`
- Test: part of `kotlin/src/test/kotlin/io/github/transittrack/gtfsanalyzer/GtfsAnalyzerIntegrationTest.kt` (the `feedPath` IOException case only — the rest of this test class is Task 10)

**Interfaces:**
- Consumes: `loadNativeLibrary()` (Task 8), `EngineResult`/`EngineJson` (Task 6), `ValidatorConfigDelta` (Task 7).
- Produces: `object GtfsAnalyzer { fun validate(feed: ByteArray, today: LocalDate = LocalDate.now(), config: ValidatorConfigDelta = ValidatorConfigDelta(), includeNameIndex: Boolean = false, lang: String = "en"): EngineResult; fun validate(feedPath: Path, today: LocalDate = LocalDate.now(), config: ValidatorConfigDelta = ValidatorConfigDelta(), includeNameIndex: Boolean = false, lang: String = "en"): EngineResult }`, the SDK's sole public entry point, used by Task 10's remaining tests and by consumers.

- [ ] **Step 1: Write the failing test**

```kotlin
class GtfsAnalyzerIntegrationTest {
    @Test
    fun validateThrowsIOExceptionForAMissingFeedPath() {
        assertThrows<IOException> {
            GtfsAnalyzer.validate(Path.of("/does/not/exist.zip"))
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd kotlin && ./gradlew test --tests GtfsAnalyzerIntegrationTest`
Expected: FAIL to compile — `GtfsAnalyzer` doesn't exist yet.

- [ ] **Step 3: Implement `GtfsAnalyzerNative.kt`**

```kotlin
internal object GtfsAnalyzerNative {
    @JvmStatic
    external fun validate(
        feed: ByteArray,
        today: Int,
        configJson: String,
        includeNameIndex: Boolean,
        lang: String,
    ): String
}
```

The package/object name here must exactly match the JNI symbol produced in Task 3 (`Java_io_github_transittrack_gtfsanalyzer_GtfsAnalyzerNative_validate`).

- [ ] **Step 4: Implement `GtfsAnalyzer.kt`**

```kotlin
object GtfsAnalyzer {
    /**
     * Blocking — runs the validation engine on the calling thread. Typically
     * completes in well under a second; callers on a coroutine dispatcher
     * should wrap this with `withContext(Dispatchers.IO)`.
     */
    fun validate(
        feed: ByteArray,
        today: LocalDate = LocalDate.now(),
        config: ValidatorConfigDelta = ValidatorConfigDelta(),
        includeNameIndex: Boolean = false,
        lang: String = "en",
    ): EngineResult {
        loadNativeLibrary()
        val todayInt = today.format(DateTimeFormatter.BASIC_ISO_DATE).toInt() // YYYYMMDD
        val configJson = EngineJson.encodeToString(config)
        val json = GtfsAnalyzerNative.validate(feed, todayInt, configJson, includeNameIndex, lang)
        return EngineJson.decodeFromString(json)
    }

    fun validate(
        feedPath: Path,
        today: LocalDate = LocalDate.now(),
        config: ValidatorConfigDelta = ValidatorConfigDelta(),
        includeNameIndex: Boolean = false,
        lang: String = "en",
    ): EngineResult = validate(Files.readAllBytes(feedPath), today, config, includeNameIndex, lang)
}
```

`Files.readAllBytes` already throws `IOException`/`NoSuchFileException` (an `IOException` subtype) for a missing path, satisfying Step 1's test without extra handling.

- [ ] **Step 5: Run test to verify it passes**

Run: `cd kotlin && ./gradlew test --tests GtfsAnalyzerIntegrationTest`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/GtfsAnalyzerNative.kt kotlin/src/main/kotlin/io/github/transittrack/gtfsanalyzer/GtfsAnalyzer.kt kotlin/src/test
git commit -m "feat(kotlin): add the public GtfsAnalyzer.validate API"
```

---

### Task 10: Kotlin — end-to-end integration tests (native build wiring)

**Files:**
- Modify: `kotlin/build.gradle.kts` (wire `test` to depend on `buildNativeForCurrentPlatform`, Task 4)
- Modify: `kotlin/src/test/kotlin/io/github/transittrack/gtfsanalyzer/GtfsAnalyzerIntegrationTest.kt`

**Interfaces:**
- Consumes: `GtfsAnalyzer.validate(...)` (Task 9), `buildNativeForCurrentPlatform` Gradle task (Task 4).
- Produces: nothing new for later tasks — this is the full-stack proof that Tasks 1-9 interoperate.

- [ ] **Step 1: Wire the native build into the test task**

In `kotlin/build.gradle.kts`: `tasks.named("test") { dependsOn("buildNativeForCurrentPlatform") }`.

- [ ] **Step 2: Write the failing tests** (added to `GtfsAnalyzerIntegrationTest.kt`)

```kotlin
// Same fixture bytes as crates/pipeline/tests/integration.rs's base_files(): 1 agency,
// 2 stops, 1 route, 1 trip, one calendar entry covering 2026-05-15 (TODAY below).
private val TODAY: LocalDate = LocalDate.of(2026, 5, 15)
private fun minimalFeedZip(): ByteArray { /* build the same 6 CSV files via java.util.zip.ZipOutputStream */ }

@Test
fun validateReturnsOkWithExpectedMetricsForACleanFeed() {
    val result = GtfsAnalyzer.validate(minimalFeedZip(), today = TODAY)
    assertIs<EngineResult.Ok>(result)
    val ok = result as EngineResult.Ok
    assertEquals(2, ok.result.metrics.stopCount)
    assertEquals(1, ok.result.metrics.routeCount)
    assertEquals(1, ok.result.metrics.tripCount)
}

@Test
fun validateReturnsFatalForACorruptZip() {
    val result = GtfsAnalyzer.validate("not a zip".toByteArray(), today = TODAY)
    assertIs<EngineResult.Fatal>(result)
    assertEquals(FatalCode.ZipUnreadable, (result as EngineResult.Fatal).error.code)
}

@Test
fun validateThrowsIllegalArgumentExceptionForAnUnknownConfigKey() {
    // ValidatorConfigDelta only exposes known fields, so simulate an unknown key the
    // way a hand-built config would reach the engine: encode, then splice in a bad key.
    val badConfigJson = """{"not_a_real_key": 1}"""
    assertThrows<IllegalArgumentException> {
        GtfsAnalyzerNative.validate(minimalFeedZip(), 20260515, badConfigJson, false, "en")
    }
}

@Test
fun validateIsSafeToCallConcurrently() {
    val executor = Executors.newFixedThreadPool(8)
    val futures = (1..32).map {
        executor.submit<EngineResult> { GtfsAnalyzer.validate(minimalFeedZip(), today = TODAY) }
    }
    futures.forEach { f -> assertIs<EngineResult.Ok>(f.get(30, TimeUnit.SECONDS)) }
    executor.shutdown()
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd kotlin && ./gradlew test --tests GtfsAnalyzerIntegrationTest`
Expected: FAIL — `minimalFeedZip()` unimplemented / native lib not yet built for this host.

- [ ] **Step 4: Implement `minimalFeedZip()`** using `java.util.zip.ZipOutputStream`, writing the same six CSV files and byte content as `crates/pipeline/tests/integration.rs:24-34` (`agency.txt`, `stops.txt`, `routes.txt`, `trips.txt`, `stop_times.txt`, `calendar.txt`).

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd kotlin && ./gradlew test --tests GtfsAnalyzerIntegrationTest`
Expected: PASS (all 5 tests in this class)

- [ ] **Step 6: Run the full Kotlin test suite**

Run: `cd kotlin && ./gradlew test`
Expected: PASS (all tests across Tasks 4-10)

- [ ] **Step 7: Commit**

```bash
git add kotlin
git commit -m "test(kotlin): add end-to-end GtfsAnalyzer integration tests"
```

---

### Task 11: CI — `publish-kotlin.yml`

**Files:**
- Create: `.github/workflows/publish-kotlin.yml`

**Interfaces:**
- Consumes: `copyNativeLib`/`cargo build -p gtfs-analyzer-jvm --release` (Tasks 1-4), the workspace `Cargo.toml` version (Global Constraints).
- Produces: nothing consumed elsewhere; this is the release pipeline.

- [ ] **Step 1: Write the workflow**, modeled directly on `.github/workflows/publish-python.yml`:
  - `on: workflow_dispatch` only.
  - `require-tag` job: identical tag-format gate (`^v[0-9]+\.[0-9]+\.[0-9]+$`), copied from `publish-python.yml:20-30`.
  - `build-natives` job: `strategy.matrix` of `{os: ubuntu-latest, tag: linux-x86_64}`, `{os: ubuntu-24.04-arm, tag: linux-aarch64}`, `{os: macos-latest, tag: darwin-aarch64}`; each runs `cargo build --release -p gtfs-analyzer-jvm`, then uploads `target/release/libgtfs_jvm.{so,dylib}` as artifact `native-${{ matrix.tag }}`.
  - `publish` job (`needs: [require-tag, build-natives]`, `permissions: { contents: read }`): downloads all three `native-*` artifacts; for each, runs `cd kotlin && ./gradlew copyNativeLib -PnativeLibFile=<downloaded path> -PplatformTag=<tag>`; verifies the release tag equals the version `kotlin/build.gradle.kts` derives from `Cargo.toml` (same regex check pattern as `publish-python.yml`'s "Check release version" step); runs `./gradlew publish` with Central Portal credentials from secrets (same secret-usage pattern as the existing PyPI/npm publish jobs — the actual Sonatype/Central Portal account and GPG key are the maintainer's manual setup step, per the design doc's non-goals).

- [ ] **Step 2: Validate the YAML**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/publish-kotlin.yml'))"`
Expected: no error.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/publish-kotlin.yml
git commit -m "ci: add Kotlin/Maven Central publish workflow"
```

---

## Self-Review

**Spec coverage:** every "Decided" item in the design doc maps to a task — Rust JNI entry point (1-3), Kotlin data model (5-7), native loading (8), public API + threading contract (9), CI/release (11), and the read-only-filesystem-tolerant loader + panic/infra-exception split are in Task 8/9's implementation steps. The design doc's non-goals (Android/KMP, Windows, streaming callbacks, Maven account setup) are correctly absent from the task list.

**Type consistency:** `ValidateJvmOutcome` (Task 2) is consumed only by Task 3's JNI function, which never leaks it past the JNI boundary — Kotlin only ever sees a `String` (success) or a thrown exception (`IllegalArgumentException`/`GtfsEngineException`), checked against Task 9/10's tests. `detectPlatformTag`/`resolveWritableNativeLibDir`/`loadNativeLibrary` (Task 8) are used with identical names and signatures in Task 9. `GtfsAnalyzerNative.validate(...)`'s JNI-facing signature (Task 9) matches the native symbol's parameter order and types from Task 3 exactly (`ByteArray, Int, String, Boolean, String`).

**Review Focus coverage:** all five items are mapped to an owning task's tests (unknown config key → Task 10; corrupt-zip Fatal → Task 10; concurrent calls → Task 10; missing feedPath → Task 9; unsupported platform → Task 8).

**Proportion:** code blocks are signatures, test bodies, and the two algorithms that needed pinning exactly (the panic→outcome mapping, the platform-tag mapping table) — data-class and config-field transcription is delegated to the already-canonical `sdk/src/types.ts`/Rust struct sources by reference rather than retyped here, consistent with the file-structure/size of the spec being implemented.
