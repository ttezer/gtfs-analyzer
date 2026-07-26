//! CLI surface tests: flags, filters, exit codes and the JSON envelope.
//!
//! The binary is invoked through `CARGO_BIN_EXE_gtfs-analyzer`, so no extra
//! test-harness dependency is needed. Feed fixtures mirror the minimal feed used
//! by `gtfs-pipeline`'s integration tests.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use zip::write::SimpleFileOptions;

/// Fixed validation date — keeps calendar/analytics rules deterministic.
const TODAY: &str = "20260515";

// ── fixtures ──────────────────────────────────────────────────────────────────

static AGENCY: &[u8] =
    b"agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://test.example,UTC\n";
static STOPS: &[u8] =
    b"stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n";
static ROUTES: &[u8] = b"route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n";
static TRIPS: &[u8] = b"route_id,service_id,trip_id\nR1,SVC1,T1\n";
/// `route_id` that routes.txt never declares → TRP_002 (CRITICAL / SPEC),
/// i.e. an R1 publish blocker.
static TRIPS_DANGLING_ROUTE: &[u8] = b"route_id,service_id,trip_id\nR9,SVC1,T1\n";
static STOP_TIMES: &[u8] = b"trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
      T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n";
static CALENDAR: &[u8] =
    b"service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n\
      SVC1,1,1,1,1,1,0,0,20250101,20271231\n";

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

fn base_files(trips: &'static [u8]) -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("agency.txt", AGENCY),
        ("stops.txt", STOPS),
        ("routes.txt", ROUTES),
        ("trips.txt", trips),
        ("stop_times.txt", STOP_TIMES),
        ("calendar.txt", CALENDAR),
    ]
}

/// Writes a fixture ZIP to the temp dir and returns its path.
///
/// Tests run in parallel and several share a fixture, so the write goes to a
/// per-caller temp name and is then renamed into place: a plain `fs::write`
/// truncates the file a concurrent test may already be reading.
fn write_feed(name: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("gtfs-cli-test-{name}.zip"));
    let staging = std::env::temp_dir().join(format!(
        "gtfs-cli-test-{name}.{:?}.tmp",
        std::thread::current().id()
    ));

    std::fs::write(&staging, make_zip(files)).unwrap();
    std::fs::rename(&staging, &path).unwrap();
    path
}

fn feed_ok() -> PathBuf {
    write_feed("ok", &base_files(TRIPS))
}

fn feed_with_critical() -> PathBuf {
    write_feed("critical", &base_files(TRIPS_DANGLING_ROUTE))
}

// ── invocation helpers ────────────────────────────────────────────────────────

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gtfs-analyzer"))
        .args(args)
        .output()
        .expect("failed to run gtfs-analyzer")
}

fn validate(feed: &Path, extra: &[&str]) -> Output {
    let mut args = vec!["validate", feed.to_str().unwrap(), "--today", TODAY];
    args.extend_from_slice(extra);
    run(&args)
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn json_of(out: &Output) -> serde_json::Value {
    serde_json::from_str(&stdout_of(out)).expect("stdout was not valid JSON")
}

fn code(out: &Output) -> i32 {
    out.status.code().expect("process terminated by signal")
}

fn summary_field(text: &str, key: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{key}: ")))
        .map(str::to_string)
}

// ── basics ────────────────────────────────────────────────────────────────────

#[test]
fn version_flag_is_supported() {
    let out = run(&["--version"]);
    assert_eq!(code(&out), 0);
    assert!(stdout_of(&out).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn missing_feed_file_is_a_cli_error() {
    let out = run(&["validate", "/nonexistent/feed.zip"]);
    assert_eq!(code(&out), 2);
    assert!(String::from_utf8(out.stderr)
        .unwrap()
        .contains("failed to read"));
}

#[test]
fn corrupt_zip_is_fatal_with_exit_2() {
    let path = std::env::temp_dir().join("gtfs-cli-test-corrupt.zip");
    std::fs::write(&path, b"not a zip at all").unwrap();

    let out = validate(&path, &["--json"]);
    assert_eq!(code(&out), 2);
    assert_eq!(json_of(&out)["status"], "fatal");
}

#[test]
fn feed_with_notices_exits_1() {
    let out = validate(&feed_with_critical(), &[]);
    assert_eq!(code(&out), 1);
    assert_eq!(
        summary_field(&stdout_of(&out), "status").as_deref(),
        Some("OK")
    );
}

#[test]
fn summary_and_json_are_mutually_exclusive() {
    let out = validate(&feed_ok(), &["--json", "--summary"]);
    assert_eq!(code(&out), 2, "clap should reject the conflicting pair");
}

// ── JSON envelope ─────────────────────────────────────────────────────────────

#[test]
fn json_envelope_is_flat_and_tagged_by_status() {
    let out = validate(&feed_with_critical(), &["--json"]);
    let json = json_of(&out);

    assert_eq!(json["status"], "ok");
    // Flat: no `Ok` / `Fatal` enum wrapper to unpack.
    assert!(json.get("Ok").is_none());
    assert!(json["notices"].is_array());
    assert!(json["reports"].is_object());
}

#[test]
fn name_index_is_omitted_unless_requested() {
    let feed = feed_with_critical();

    let lean = json_of(&validate(&feed, &["--json"]));
    assert!(lean["name_index"]["stops"].as_object().unwrap().is_empty());

    let full = json_of(&validate(&feed, &["--json", "--include-name-index"]));
    assert!(!full["name_index"]["stops"].as_object().unwrap().is_empty());
}

#[test]
fn pretty_flag_indents_the_json() {
    let out = validate(&feed_with_critical(), &["--json", "--pretty"]);
    assert!(stdout_of(&out).contains("\n  \""));
}

#[test]
fn output_flag_writes_to_a_file_instead_of_stdout() {
    let dest = std::env::temp_dir().join("gtfs-cli-test-out.json");
    let _ = std::fs::remove_file(&dest);

    let out = validate(
        &feed_with_critical(),
        &["--json", "-o", dest.to_str().unwrap()],
    );
    assert!(stdout_of(&out).is_empty());

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&dest).unwrap()).unwrap();
    assert_eq!(written["status"], "ok");
}

// ── filters ───────────────────────────────────────────────────────────────────

#[test]
fn class_filter_keeps_only_the_requested_class() {
    let out = validate(&feed_with_critical(), &["--json", "--class", "spec"]);
    let json = json_of(&out);

    let notices = json["notices"].as_array().unwrap();
    assert!(!notices.is_empty(), "fixture should produce SPEC notices");
    assert!(notices.iter().all(|n| n["rule_class"] == "SPEC"));
    assert_eq!(json["filtered"]["applied"][0], "class=SPEC");
}

#[test]
fn class_filter_accepts_multiple_values() {
    let unfiltered = json_of(&validate(&feed_with_critical(), &["--json"]));
    let filtered = json_of(&validate(
        &feed_with_critical(),
        &["--json", "--class", "spec,interop,quality,analytics"],
    ));

    assert_eq!(
        unfiltered["notices"].as_array().unwrap().len(),
        filtered["notices"].as_array().unwrap().len(),
        "listing every class must keep every notice"
    );
}

#[test]
fn min_severity_keeps_the_severity_and_everything_worse() {
    let out = validate(&feed_with_critical(), &["--json", "--min-severity", "high"]);
    let notices = json_of(&out)["notices"].as_array().unwrap().clone();

    assert!(notices
        .iter()
        .all(|n| n["severity"] == "CRITICAL" || n["severity"] == "HIGH"));
}

#[test]
fn severity_and_min_severity_are_mutually_exclusive() {
    let out = validate(
        &feed_ok(),
        &["--severity", "high", "--min-severity", "high"],
    );
    assert_eq!(code(&out), 2);
}

#[test]
fn unknown_rule_filter_yields_no_notices_and_exit_0() {
    let out = validate(&feed_with_critical(), &["--json", "--rule", "NOPE_999"]);
    assert_eq!(code(&out), 0);
    assert!(json_of(&out)["notices"].as_array().unwrap().is_empty());
}

/// Regression: filters are display-only. Hiding the critical SPEC notices must
/// not flip the publish verdict — that verdict describes the whole feed.
#[test]
fn filtering_does_not_change_the_publish_verdict() {
    let feed = feed_with_critical();

    let unfiltered = json_of(&validate(&feed, &["--json"]));
    assert_eq!(
        unfiltered["reports"]["r1"]["publishable"], false,
        "fixture must contain a critical SPEC blocker for this test to mean anything"
    );

    for filter in [
        vec!["--class", "analytics"],
        vec!["--severity", "info"],
        vec!["--rule", "NOPE_999"],
    ] {
        let mut args = vec!["--json"];
        args.extend_from_slice(&filter);
        let json = json_of(&validate(&feed, &args));

        assert_eq!(
            json["reports"]["r1"]["publishable"], false,
            "filter {filter:?} must not make an unpublishable feed look publishable"
        );
        assert_eq!(
            json["reports"]["r5"]["pub_score"], unfiltered["reports"]["r5"]["pub_score"],
            "filter {filter:?} must not move the scores"
        );
    }
}

#[test]
fn summary_reports_the_active_filter() {
    let out = validate(&feed_with_critical(), &["--class", "spec"]);
    let text = stdout_of(&out);
    assert_eq!(
        summary_field(&text, "filter").as_deref(),
        Some("class=SPEC")
    );
    assert!(text.contains("filter_note:"));
}

// ── exit-code gates ───────────────────────────────────────────────────────────

#[test]
fn fail_on_critical_ignores_lesser_notices() {
    let clean = feed_ok();

    // The clean fixture still emits non-critical findings → default gate fails.
    assert_eq!(code(&validate(&clean, &[])), 1);
    // …but the critical gate stays green.
    assert_eq!(code(&validate(&clean, &["--fail-on", "critical"])), 0);
}

#[test]
fn fail_on_critical_trips_on_a_critical_notice() {
    let out = validate(&feed_with_critical(), &["--fail-on", "critical"]);
    assert_eq!(code(&out), 1);
}

#[test]
fn fail_on_class_gates_on_the_rule_class() {
    let feed = feed_with_critical();
    assert_eq!(code(&validate(&feed, &["--fail-on-class", "spec"])), 1);
}

// ── rules subcommand ──────────────────────────────────────────────────────────

#[test]
fn rules_lists_the_whole_registry() {
    let out = run(&["rules", "--json"]);
    assert_eq!(code(&out), 0);
    let rows = json_of(&out);
    assert_eq!(rows.as_array().unwrap().len(), gtfs_rules::RULES.len());
}

#[test]
fn rules_filters_by_class_and_severity() {
    let out = run(&[
        "rules",
        "--json",
        "--class",
        "spec",
        "--severity",
        "critical",
    ]);
    let rows = json_of(&out);
    let rows = rows.as_array().unwrap();

    assert!(!rows.is_empty());
    assert!(rows
        .iter()
        .all(|r| r["class"] == "SPEC" && r["severity"] == "CRITICAL"));
}

#[test]
fn rules_exposes_the_authority_source() {
    let out = run(&["rules", "--json", "--rule", "STM_004"]);
    let rows = json_of(&out);
    let row = &rows.as_array().unwrap()[0];

    assert_eq!(row["id"], "STM_004");
    assert_eq!(row["class"], "SPEC");
    // Class-authority invariant: SPEC is only legitimate with GTFS_SPEC authority.
    assert_eq!(row["authority_source"], "GTFS_SPEC");
}

#[test]
fn rules_rejects_an_unknown_rule_id() {
    let out = run(&["rules", "--rule", "NOPE_999"]);
    assert_eq!(code(&out), 2);
    assert!(String::from_utf8(out.stderr)
        .unwrap()
        .contains("unknown rule id"));
}

#[test]
fn rules_text_output_ends_with_a_count() {
    let out = run(&["rules", "--class", "spec"]);
    let text = stdout_of(&out);
    assert!(text.trim_end().ends_with("rules"));
}

// ── --lang ────────────────────────────────────────────────────────────────────

/// The fixture's blocker is TRP_002; its notice carries `{entity_id}`-style
/// placeholders, so this also proves interpolation survives the CLI path.
fn first_spec_notice(feed: &Path, lang: &str) -> serde_json::Value {
    let out = validate(feed, &["--json", "--class", "spec", "--lang", lang]);
    json_of(&out)["notices"][0].clone()
}

#[test]
fn lang_defaults_to_the_pipelines_turkish_text() {
    let feed = feed_with_critical();
    let default = first_spec_notice(&feed, "tr");
    let explicit = json_of(&validate(&feed, &["--json", "--class", "spec"]))["notices"][0].clone();

    assert_eq!(default["title"], explicit["title"]);
    assert_eq!(default["message"], explicit["message"]);
}

#[test]
fn lang_en_translates_title_message_and_remediation() {
    let feed = feed_with_critical();
    let tr = first_spec_notice(&feed, "tr");
    let en = first_spec_notice(&feed, "en");

    assert_eq!(en["rule_id"], tr["rule_id"], "same notice, different text");
    for field in ["title", "message", "remediation"] {
        assert_ne!(en[field], tr[field], "{field} should be translated");
        assert!(
            en[field].as_str().unwrap().is_ascii(),
            "expected English {field}, got: {}",
            en[field]
        );
    }
    // Placeholders resolved from the notice, not left as `{entity_id}`.
    let message = en["message"].as_str().unwrap();
    assert!(!message.contains('{'), "unfilled placeholder in: {message}");
    assert!(message.contains("R9"), "entity not interpolated: {message}");
}

#[test]
fn lang_ja_differs_from_both_tr_and_en() {
    let feed = feed_with_critical();
    let ja = first_spec_notice(&feed, "ja");

    assert_ne!(ja["message"], first_spec_notice(&feed, "tr")["message"]);
    assert_ne!(ja["message"], first_spec_notice(&feed, "en")["message"]);
    assert!(!ja["message"].as_str().unwrap().contains('{'));
}

#[test]
fn rules_subcommand_honours_lang() {
    let tr = json_of(&run(&["rules", "--json", "--rule", "TRP_002"]));
    let en = json_of(&run(&[
        "rules", "--json", "--rule", "TRP_002", "--lang", "en",
    ]));

    assert_ne!(en[0]["title"], tr[0]["title"]);
    assert!(en[0]["title"].as_str().unwrap().is_ascii());
}

#[test]
fn unknown_lang_is_rejected() {
    let out = validate(&feed_ok(), &["--lang", "de"]);
    assert_eq!(code(&out), 2);
}
