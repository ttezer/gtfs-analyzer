use gtfs_config::{merge_delta, ValidatorConfig};
use gtfs_core::i18n::{Lang, Translator};
use gtfs_core::{NameIndex, ValidateResult};
use gtfs_pipeline::validate_bytes;
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jboolean, jint};
use jni::JNIEnv;

/// Validate one GTFS ZIP buffer using the shared Rust validation pipeline.
pub fn validate_to_json(
    feed: &[u8],
    today: u32,
    config_json: &str,
    include_name_index: bool,
    lang: &str,
) -> Result<String, String> {
    // Lang is checked BEFORE validation: an invalid code should fail immediately,
    // not after the feed has been processed.
    let lang: Lang = lang.parse()?;
    let translator = Translator::new(lang)?;
    let config = merge_delta(&ValidatorConfig::default(), config_json)?;
    let mut result = validate_bytes(feed, &config, today);

    // Same translation as the CLI (crates/cli/src/main.rs): title/message/remediation and fatal.
    if let Some(translator) = &translator {
        match &mut result {
            ValidateResult::Ok(validation) => {
                for notice in &mut validation.notices {
                    translator.translate(notice);
                }
            }
            ValidateResult::Fatal(err) => translator.translate_fatal(err),
        }
    }

    if !include_name_index {
        if let ValidateResult::Ok(validation) = &mut result {
            validation.name_index = NameIndex::default();
        }
    }

    serde_json::to_string(&result).map_err(|e| e.to_string())
}

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

/// JNI entry point for `GtfsAnalyzerNative.validate(...)` on the Kotlin side.
///
/// Converts the JNI arguments to Rust types, dispatches to [`validate_guarded`],
/// and converts the outcome back across the JNI boundary:
/// - `Json(s)` returns a new Java string containing `s`.
/// - `InvalidArgument(msg)` throws `java.lang.IllegalArgumentException` with `msg`.
/// - `Panicked(msg)` throws `io/github/transittrack/gtfsanalyzer/GtfsEngineException` with `msg`.
///
/// In both error cases a pending exception is set via `throw_new`; JNI still requires
/// this function to return a value of its declared return type, so a null `JString`
/// (`JString::default()`) is returned without making any further JNI call — the JVM observes
/// the pending exception on return and discards this value, but the JNI spec forbids calling
/// most JNI functions (including `NewStringUTF`, i.e. `env.new_string`) while an exception is
/// pending, so building that placeholder must not touch the JVM at all.
#[no_mangle]
pub extern "system" fn Java_io_github_transittrack_gtfsanalyzer_GtfsAnalyzerNative_validate<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    feed: JByteArray<'local>,
    today: jint,
    config_json: JString<'local>,
    include_name_index: jboolean,
    lang: JString<'local>,
) -> JString<'local> {
    let feed_bytes = match env.convert_byte_array(&feed) {
        Ok(bytes) => bytes,
        Err(e) => {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                format!("invalid feed byte array: {e}"),
            );
            // Must not make any further JNI calls now that an exception is pending (the JNI
            // spec only permits a handful of Exception*/Release* calls in that state) — the
            // `jni` crate's 0.21 pending-exception check turns a violation (e.g. `new_string`)
            // into `Err(JavaException)`, so the prior `.unwrap()` here aborted the JVM process.
            // `JString::default()` wraps a null pointer without calling into the JVM at all.
            return JString::default();
        }
    };

    let config_json_str: String = match env.get_string(&config_json) {
        Ok(s) => s.into(),
        Err(e) => {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                format!("invalid config_json string: {e}"),
            );
            // See the comment on the matching branch above: no further JNI calls once an
            // exception is pending.
            return JString::default();
        }
    };

    let lang_str: String = match env.get_string(&lang) {
        Ok(s) => s.into(),
        Err(e) => {
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                format!("invalid lang string: {e}"),
            );
            // See the comment on the matching branch above: no further JNI calls once an
            // exception is pending.
            return JString::default();
        }
    };

    let include_name_index_bool = include_name_index != 0;

    match validate_guarded(
        &feed_bytes,
        today as u32,
        &config_json_str,
        include_name_index_bool,
        &lang_str,
    ) {
        // `new_string` can fail (e.g. JVM heap exhaustion on a very large result, realistic
        // when `includeNameIndex=true`). The `jni` crate already turns that failure into a
        // pending exception, so — same as every other branch in this function — just hand
        // back a null `JString` (`unwrap_or_default()`, which does not touch the JVM again
        // on the `Err` path) rather than `.unwrap()`ing and aborting the whole JVM process.
        ValidateJvmOutcome::Json(s) => env.new_string(s).unwrap_or_default(),
        ValidateJvmOutcome::InvalidArgument(msg) => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", msg);
            // See the comment above: no further JNI calls once an exception is pending.
            JString::default()
        }
        ValidateJvmOutcome::Panicked(msg) => {
            let _ = env.throw_new(
                "io/github/transittrack/gtfsanalyzer/GtfsEngineException",
                msg,
            );
            JString::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use zip::write::SimpleFileOptions;

    // Same fixed date used across tests — deterministic analytics output.
    const TODAY: u32 = 20_260_515;

    // ── Helpers ─────────────────────────────────────────────────────────

    fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let cur = std::io::Cursor::new(Vec::new());
        let mut zw = zip::ZipWriter::new(cur);
        let opts = SimpleFileOptions::default();
        for (name, data) in files {
            zw.start_file(*name, opts).unwrap();
            zw.write_all(data).unwrap();
        }
        zw.finish().unwrap().into_inner()
    }

    // Date range (20250101-20271231) covers the test day (20260515, Friday).
    // Monday-Friday=1 -> SVC1 active -> no service-gap notice.
    static AGENCY: &[u8] =
        b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://test.example,UTC\n";
    static STOPS: &[u8] =
        b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n";
    static ROUTES: &[u8] = b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n";
    static TRIPS: &[u8] = b"route_id,service_id,trip_id\nR1,SVC1,T1\n";
    static STOP_TIMES: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
          T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n";
    static CALENDAR: &[u8] =
        b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
          SVC1,1,1,1,1,1,0,0,20250101,20271231\n";

    fn base_files() -> Vec<(&'static str, &'static [u8])> {
        vec![
            ("agency.txt", AGENCY),
            ("stops.txt", STOPS),
            ("routes.txt", ROUTES),
            ("trips.txt", TRIPS),
            ("stop_times.txt", STOP_TIMES),
            ("calendar.txt", CALENDAR),
        ]
    }

    #[test]
    fn validate_to_json_returns_ok_envelope_for_a_clean_feed() {
        let zip = make_zip(&base_files());
        let json = validate_to_json(&zip, TODAY, "{}", false, "en").unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            value.get("Ok").is_some(),
            "expected an {{\"Ok\": ...}} envelope, got: {json}"
        );
        let metrics = &value["Ok"]["metrics"];
        assert_eq!(metrics["stop_count"], 2);
        assert_eq!(metrics["route_count"], 1);
        assert_eq!(metrics["trip_count"], 1);
    }

    #[test]
    fn validate_to_json_rejects_unknown_config_key() {
        let zip = make_zip(&base_files());
        let err =
            validate_to_json(&zip, TODAY, r#"{"not_a_real_key": 1}"#, false, "en").unwrap_err();
        assert!(
            err.contains("not_a_real_key"),
            "error should name the bad key: {err}"
        );
    }

    #[test]
    fn validate_to_json_rejects_unknown_lang() {
        let zip = make_zip(&base_files());
        let err = validate_to_json(&zip, TODAY, "{}", false, "xx").unwrap_err();
        assert!(!err.is_empty());
    }

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
}
