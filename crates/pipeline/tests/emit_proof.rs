//! Runtime emit proof (#5 "full version" / C).
//!
//! emit_coverage.rs (rules crate) provides a static *reference proof* (the rule occurs in
//! production code). This test provides a *real emit proof*: each fixture builds a feed, runs the
//! pipeline, and verifies that the expected rule_id is actually produced in `vr.notices`.
//! Canonical rules without a fixture (yet) are kept in the `coverage_debt.txt` ledger; fixtures
//! are added over time to reduce the debt (an incremental "full version"). The ledger equality
//! test catches both regressions (fixture removed → debt increases) and progress (fixture added
//! → debt decreases, ledger must be updated).

use std::collections::BTreeSet;
use std::io::Write as _;
use zip::write::SimpleFileOptions;

use gtfs_pipeline::{
    validate_bytes, CalendarOverrideRule, GtfsJpProfile, ValidateResult, ValidatorConfig,
};
use gtfs_rules::RULES;

const TODAY: u32 = 20_260_515;

// ── Valid baseline feed (same as integration.rs; produces no notices by itself) ──
const AGENCY: &str =
    "agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://test.example,UTC\n";
const STOPS: &str = "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n";
const ROUTES: &str = "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n";
const TRIPS: &str = "route_id,service_id,trip_id\nR1,SVC1,T1\n";
const STOP_TIMES: &str = "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n";
const CALENDAR: &str = "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\n";

// File contents are kept as bytes: string fixtures are converted to UTF-8, while raw
// (invalid-UTF-8) fixtures pass bytes directly (for ARC_002/003).
fn base() -> Vec<(String, Vec<u8>)> {
    [
        ("agency.txt", AGENCY),
        ("stops.txt", STOPS),
        ("routes.txt", ROUTES),
        ("trips.txt", TRIPS),
        ("stop_times.txt", STOP_TIMES),
        ("calendar.txt", CALENDAR),
    ]
    .iter()
    .map(|(n, c)| (n.to_string(), c.as_bytes().to_vec()))
    .collect()
}

/// Takes the baseline feed, applies string and raw-byte overrides (also adding new files),
/// and removes files listed in `removes` (for example, the "agency.txt missing" scenario).
fn with_opts(
    overrides: &[(&str, &str)],
    removes: &[&str],
    raw: &[(&str, &[u8])],
) -> Vec<(String, Vec<u8>)> {
    let mut files = base();
    let apply = |name: &str, bytes: Vec<u8>, files: &mut Vec<(String, Vec<u8>)>| {
        if let Some(slot) = files.iter_mut().find(|(n, _)| n == name) {
            slot.1 = bytes;
        } else {
            files.push((name.to_string(), bytes));
        }
    };
    for (name, content) in overrides {
        apply(name, content.as_bytes().to_vec(), &mut files);
    }
    for (name, content) in raw {
        apply(name, content.to_vec(), &mut files);
    }
    files.retain(|(n, _)| !removes.contains(&n.as_str()));
    files
}

fn make_zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    make_zip_with_modes(files, &[])
}

/// Writes files in `no_read` WITHOUT READ PERMISSION (0o200) — for ARC_027.
/// The others use the default allowance.
fn make_zip_with_modes(files: &[(String, Vec<u8>)], no_read: &[&str]) -> Vec<u8> {
    let cur = std::io::Cursor::new(Vec::new());
    let mut zw = zip::ZipWriter::new(cur);
    for (name, data) in files {
        let opts = if no_read.contains(&name.as_str()) {
            SimpleFileOptions::default().unix_permissions(0o200)
        } else {
            SimpleFileOptions::default()
        };
        zw.start_file(name, opts).unwrap();
        zw.write_all(data).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// ARC_027 fixture: writes the given files without read permission.
fn fx_noread(rule: &'static str, no_read: Vec<&'static str>) -> Fixture {
    Fixture {
        rule,
        overrides: Vec::new(),
        removes: Vec::new(),
        raw: Vec::new(),
        config: None,
        no_read,
        flip_stream_flags: false,
    }
}

/// ARC_036 fixture: adds bit 3 to local-header flags AFTER the zip is BUILT.
/// Contents and the central directory stay unchanged — the archive remains readable;
/// only the stream view is inconsistent.
/// `ARC_027` precedent: the rule fires from ZIP METADATA and cannot be produced through a
/// normal fixture path.
fn flip_data_descriptor_flags(mut zip_bytes: Vec<u8>) -> Vec<u8> {
    let starts: Vec<u64> = {
        let mut a = zip::ZipArchive::new(std::io::Cursor::new(&zip_bytes[..])).unwrap();
        (0..a.len())
            .map(|i| a.by_index_raw(i).unwrap().header_start())
            .collect()
    };
    for s in starts {
        let off = s as usize + 6; // general purpose bit flag
        let flag = u16::from_le_bytes([zip_bytes[off], zip_bytes[off + 1]]);
        zip_bytes[off..off + 2].copy_from_slice(&(flag | 0x0008).to_le_bytes());
    }
    zip_bytes
}

/// ARC_036 fixture: baseline feed with only the zip framing corrupted.
fn fx_stream_flag(rule: &'static str) -> Fixture {
    Fixture {
        rule,
        overrides: Vec::new(),
        removes: Vec::new(),
        raw: Vec::new(),
        config: None,
        no_read: Vec::new(),
        flip_stream_flags: true,
    }
}

fn emitted_rules(files: &[(String, Vec<u8>)], config: &ValidatorConfig) -> BTreeSet<String> {
    emitted_rules_modes(files, config, &[], false)
}

fn emitted_rules_modes(
    files: &[(String, Vec<u8>)],
    config: &ValidatorConfig,
    no_read: &[&str],
    flip_stream_flags: bool,
) -> BTreeSet<String> {
    let zip_bytes = make_zip_with_modes(files, no_read);
    let zip_bytes = if flip_stream_flags {
        flip_data_descriptor_flags(zip_bytes)
    } else {
        zip_bytes
    };
    match validate_bytes(&zip_bytes, config, TODAY) {
        ValidateResult::Ok(vr) => vr.notices.iter().map(|n| n.rule_id.clone()).collect(),
        ValidateResult::Fatal(e) => {
            // Fatal path: return an empty set instead of deriving a rule_id; archive Fatals
            // are in the allowlist.
            let _ = e;
            BTreeSet::new()
        }
    }
}

fn notices_for(files: &[(String, Vec<u8>)], config: &ValidatorConfig) -> Vec<gtfs_core::Notice> {
    match validate_bytes(&make_zip(files), config, TODAY) {
        ValidateResult::Ok(vr) => vr.notices,
        ValidateResult::Fatal(e) => panic!("fixture validation unexpectedly fatal: {e:?}"),
    }
}

/// A fixture: expected rule_id plus the feed that triggers it (overrides on the baseline).
struct Fixture {
    rule: &'static str,
    overrides: Vec<(&'static str, &'static str)>,
    removes: Vec<&'static str>,
    raw: Vec<(&'static str, &'static [u8])>,
    config: Option<ValidatorConfig>,
    /// For ARC_027: these files are written to the zip WITHOUT READ PERMISSION (unix mode 0o200).
    /// The rule checks `zf.unix_mode() & 0o400 == 0`; it is triggered by zip METADATA, not content, so the
    /// normal fixture path could not produce it.
    no_read: Vec<&'static str>,
    /// For ARC_036: bit 3 is added to local-header flags AFTER the zip is built.
    /// Same rationale — the trigger is zip framing, not content.
    flip_stream_flags: bool,
}

fn fx(rule: &'static str, overrides: Vec<(&'static str, &'static str)>) -> Fixture {
    Fixture {
        rule,
        overrides,
        removes: Vec::new(),
        raw: Vec::new(),
        config: None,
        no_read: Vec::new(),
        flip_stream_flags: false,
    }
}

/// Emit proof for V3 extension rules: although production defaults to Auto, these fixtures
/// deliberately select the legacy/V3 profile. The proof test therefore measures that V3
/// behavior still works, rather than whether V4 suppresses these rules.
fn fx_v3(rule: &'static str, overrides: Vec<(&'static str, &'static str)>) -> Fixture {
    Fixture {
        rule,
        overrides,
        removes: Vec::new(),
        raw: Vec::new(),
        config: Some(ValidatorConfig {
            gtfs_jp_profile: GtfsJpProfile::V3,
            ..ValidatorConfig::default()
        }),
        no_read: Vec::new(),
        flip_stream_flags: false,
    }
}

/// Emit-proof fixture for a V4 rule: the profile version is selected explicitly by the fixture.
fn fx_v4(rule: &'static str, overrides: Vec<(&'static str, &'static str)>) -> Fixture {
    Fixture {
        rule,
        overrides,
        removes: Vec::new(),
        raw: Vec::new(),
        config: Some(ValidatorConfig {
            gtfs_jp_profile: GtfsJpProfile::V4,
            ..ValidatorConfig::default()
        }),
        no_read: Vec::new(),
        flip_stream_flags: false,
    }
}

/// Fixture that removes files ("X.txt missing" scenarios).
fn fx_rm(
    rule: &'static str,
    overrides: Vec<(&'static str, &'static str)>,
    removes: Vec<&'static str>,
) -> Fixture {
    Fixture {
        rule,
        overrides,
        removes,
        raw: Vec::new(),
        config: None,
        no_read: Vec::new(),
        flip_stream_flags: false,
    }
}

/// Raw-byte fixture — invalid UTF-8 scenarios (ARC_002/003).
fn fx_raw(rule: &'static str, raw: Vec<(&'static str, &'static [u8])>) -> Fixture {
    Fixture {
        rule,
        overrides: Vec::new(),
        removes: Vec::new(),
        raw,
        config: None,
        no_read: Vec::new(),
        flip_stream_flags: false,
    }
}

/// Config-override fixture — for example, calendar_override_rules (OPR_021/022/023).
fn fx_cfg(
    rule: &'static str,
    overrides: Vec<(&'static str, &'static str)>,
    config: ValidatorConfig,
) -> Fixture {
    Fixture {
        rule,
        overrides,
        removes: Vec::new(),
        raw: Vec::new(),
        config: Some(config),
        no_read: Vec::new(),
        flip_stream_flags: false,
    }
}

/// Rules that are not emitted as notices (Fatal path or dynamic) — exempt from proof.
/// Rules that are not emitted as notices — EXEMPT from this harness.
///
/// ⚠️ **EXEMPTION DOES NOT MEAN LACK OF PROOF.** For every rule on this list, the location
/// of its proof must be written down; if it cannot be, the rule is dead or incomplete.
/// Audited on 2026-08-02: two of the three entries had proof; ARC_004 now has a direct fixture.
///
/// This list is a blind spot in both ledgers: because these rules never emit notices,
/// they are INVISIBLE in the coverage ledger, claim ledger, and field=None ledger.
const PROOF_ALLOWLIST: &[&str] = &[
    // Fatal path — proof: integration.rs::arc001_corrupt_zip_returns_fatal_zip_unreadable
    "ARC_001",
    // Fatal path — proof: integration.rs::arc029_* (two read_fatal paths with DEFAULT limits
    // plus the ratio_floor FP guard). A notice fixture is structurally impossible.
    "ARC_029",
];

// ── DEBT LEDGER IS NOW EMPTY (coverage_debt.txt) ──
// #28 RESOLVED: ARC_002/003 (raw bytes via fx_raw), OPR_021/022/023 + ARC_028/STP_040/STP_041
// (config via fx_cfg) have runtime proof in fixtures().
//
// 2026-08-07 (issue #71): all seven remaining entries were closed. The ledger rationales
// ("cannot be written inline", "impractical") were MOSTLY WRONG; the real blocker was a
// hard-coded threshold three times, and zip metadata once:
//   OPR_024/VAT_006/STM_043 — the threshold was already configurable or the row count was writable
//   SHP_026/STM_044/ARC_022 — the threshold moved to `ValidatorConfig` (default UNCHANGED)
//   ARC_027                 — the harness gained a constructor writing `unix_permissions` (fx_noread)
// ⚠️ LESSON: "cannot be written" was a guess, not a MEASUREMENT. Each entry fell when tried.
// ARC_001/ARC_029 are in PROOF_ALLOWLIST (Fatal path). NOTE (2026-07-16): ARC_029 was in
// the debt ledger but was NOT debt — like ARC_001, it is a Fatal path; it moved to the
// allowlist, with end-to-end proof in integration.rs::arc029_*.

// #28 Group 1 helpers ─────────────────────────────────────────────────────────
// Invalid UTF-8 (0xFF/0xFE) in an OPTIONAL file (feed_info.txt) → ARC_002 + ARC_003 (not Fatal).
const BAD_UTF8_FEED_INFO: &[u8] =
    b"feed_publisher_name,feed_publisher_url,feed_lang\n\xff\xfe,https://x.example,en\n";
// calendar_dates: SVC_B active on 0101+0102, SVC_O only on 0101. The rule window is 0101-0103:
// 0101 base+override → OPR_021, 0102 base only → OPR_022, 0103 neither → OPR_023.
const OVERRIDE_CD: &str =
    "service_id,date,exception_type\nSVC_B,20260101,1\nSVC_B,20260102,1\nSVC_O,20260101,1\n";
fn override_config() -> ValidatorConfig {
    ValidatorConfig {
        calendar_override_rules: vec![CalendarOverrideRule {
            route_id: "R1".into(),
            base_service_ids: vec!["SVC_B".into()],
            override_service_ids: vec!["SVC_O".into()],
            start_date: 20260101,
            end_date: 20260103,
        }],
        ..ValidatorConfig::default()
    }
}
// ARC_028: triggered when source_url is provided and does not end in .zip (config-gated).
fn source_url_config() -> ValidatorConfig {
    ValidatorConfig {
        source_url: Some("https://example.org/gtfs".into()),
        ..ValidatorConfig::default()
    }
}
// STP_040/041: opt-in stop-name profile. P1 "Main Stop" → generic 'stop' word (STP_040)
// AND does not contain parent ST1 "Central Hub" name (STP_041). S1/S2 remain for stop_times references.
fn stop_name_config() -> ValidatorConfig {
    ValidatorConfig {
        stop_name_best_practices: true,
        ..ValidatorConfig::default()
    }
}
const STOP_NAME_STOPS: &str = "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nS1,Stop1,41.0,29.0,0,\nS2,Stop2,41.1,29.1,0,\nST1,Central Hub,41.0,29.0,1,\nP1,Main Stop,41.0,29.0,0,ST1\n";

/// rule_id → triggering fixture. Filled incrementally; every row is real runtime proof.
fn fixtures() -> Vec<Fixture> {
    vec![
        // ── #28 Grup 1: ham-byte (ARC_002/003) + config-override (OPR_021/022/023) ──
        fx_raw("ARC_002", vec![("feed_info.txt", BAD_UTF8_FEED_INFO)]),
        fx_raw("ARC_003", vec![("feed_info.txt", BAD_UTF8_FEED_INFO)]),
        fx_cfg("OPR_021", vec![("calendar_dates.txt", OVERRIDE_CD)], override_config()),
        fx_cfg("OPR_022", vec![("calendar_dates.txt", OVERRIDE_CD)], override_config()),
        fx_cfg("OPR_023", vec![("calendar_dates.txt", OVERRIDE_CD)], override_config()),
        // Bonus: config-gated rules (with fx_cfg).
        fx_cfg("ARC_028", vec![], source_url_config()),
        fx_cfg("STP_040", vec![("stops.txt", STOP_NAME_STOPS)], stop_name_config()),
        fx_cfg("STP_041", vec![("stops.txt", STOP_NAME_STOPS)], stop_name_config()),
        // ── Seed batch: K2/K6 rules with clear trigger conditions ──────────────
        // STP_003: stop_lat outside the allowed range.
        fx("STP_003", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,999.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // STP_016: two stops with identical coordinates (not parent/child).
        fx("STP_016", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.0,29.0\n")]),
        // STP_022: no stop_code (the baseline already has none → feed summary/per-stop). Two baseline stops.
        fx("STP_022", vec![]),
        // RTS_017: route without a shape (the baseline has no shape).
        fx("RTS_017", vec![]),

        // ── AGN group (agency.txt field validation) ────────────────────────────
        fx("AGN_002", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,,https://x.example,UTC\n")]),
        fx("AGN_003", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,Test,notaurl,UTC\n")]),
        fx("AGN_004", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,Test,https://x.example,NotAZone\n")]),
        fx("AGN_006", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,Test,https://x.example,UTC,!!bad\n")]),
        fx("AGN_007", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_phone\n1,Test,https://x.example,UTC,12\n")]),
        fx("AGN_008", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_fare_url\n1,Test,https://x.example,UTC,notaurl\n")]),
        fx("AGN_009", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_email\n1,Test,https://x.example,UTC,notanemail\n")]),
        fx("AGN_012", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,cemv_support\n1,Test,https://x.example,UTC,5\n")]),
        fx("AGN_015", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://x.example,UTC\n")]),
        fx("AGN_016", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_phone\n1,Test,https://x.example,UTC,888-281-2681\n")]),
        // Multiple-agency scenarios:
        fx("AGN_014", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n,B,https://b.example,UTC\n")]),
        fx("AGN_005", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n2,B,https://b.example,Europe/Istanbul\n")]),
        fx("AGN_017", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,A,https://a.example,UTC,tr\n2,B,https://b.example,UTC,en\n")]),
        // AGN_011: route agency_id is empty while multiple agencies exist.
        // AGN_011 covers TWO files: routes.txt and fare_attributes.txt (the spec states the
        // same requirement for both). The fixture triggers both so both anchors are covered.
        fx("AGN_011", vec![
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n2,B,https://b.example,UTC\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,,101,3\n"),
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfers\nF1,1.00,USD,0,0\n"),
        ]),
        // ── ARC group (archive/file/header level) ─────────────────────────────
        // ARC_008: no calendar file (neither calendar nor calendar_dates).
        fx_rm("ARC_008", vec![], vec!["calendar.txt"]),
        // ARC_004: missing required file, reported as a notice and PARTIAL.
        fx_rm("ARC_004", vec![], vec!["routes.txt"]),
        // ARC_031: translations.txt exists but feed_info.txt does not → the spec makes
        // feed_info REQUIRED in this case (otherwise it is only recommended → ARC_020, Quality).
        fx("ARC_031", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,fr,Gare,S1\n")]),
        // ARC_009: the file has no data rows (header only).
        // ARC_009 now reports ONLY optional files; the empty required-file case moved to
        // ARC_035 (2026-08-31), so this fixture is an optional file.
        fx("ARC_009", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\n")]),
        // ARC_012: row column count does not match the header.
        fx("ARC_012", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0\nS2,Stop2,41.1,29.1\n")]),
        // ARC_015: duplicate header column.
        fx("ARC_015", vec![("stops.txt", "stop_id,stop_id,stop_lat,stop_lon\nS1,X,41.0,29.0\nS2,Y,41.1,29.1\n")]),
        // ARC_019: empty column name in the header.
        fx("ARC_019", vec![("stops.txt", "stop_id,,stop_lat,stop_lon\nS1,X,41.0,29.0\nS2,Y,41.1,29.1\n")]),
        // ARC_025: required column missing from the header (stop_id).
        fx("ARC_025", vec![("stops.txt", "stop_name,stop_lat,stop_lon\nStop1,41.0,29.0\nStop2,41.1,29.1\n")]),
        // ARC_024: GTFS .txt file nested in a ZIP subdirectory.
        fx("ARC_024", vec![("feed/stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS9,Extra,41.0,29.0\n")]),
        fx("ARC_026", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\rPub,https://x.example,en\r")]),

        // ── ATR group (attributions.txt; absent from baseline, added) ──────────
        fx("ATR_001", vec![("attributions.txt", "organization_name,is_producer\nOrg,1\n")]),
        fx("ATR_002", vec![("attributions.txt", "attribution_id,organization_name,is_producer\nA1,,1\n")]),
        fx("ATR_003", vec![("attributions.txt", "attribution_id,organization_name,is_producer,is_operator,is_authority\nA1,Org,0,0,0\n")]),
        fx("ATR_004", vec![("attributions.txt", "attribution_id,organization_name,is_producer\nA1,Org,5\n")]),
        fx("ATR_005", vec![("attributions.txt", "attribution_id,organization_name,is_operator\nA1,Org,5\n")]),
        fx("ATR_006", vec![("attributions.txt", "attribution_id,organization_name,is_authority\nA1,Org,5\n")]),
        fx("ATR_007", vec![("attributions.txt", "attribution_id,organization_name,is_producer,attribution_url\nA1,Org,1,notaurl\n")]),
        fx("ATR_008", vec![("attributions.txt", "attribution_id,organization_name,is_producer,attribution_email\nA1,Org,1,notanemail\n")]),

        // ── BKR group (booking_rules.txt; prior_notice consistency) ───────────
        fx("BKR_001", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min,prior_notice_last_day\nBR1,1,30,2\n")]),
        fx("BKR_003", vec![("booking_rules.txt", "booking_rule_id,prior_notice_start_time\nBR1,09:00:00\n")]),
        fx("BKR_004", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min\nBR1,0,15\n")]),
        fx("BKR_005", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_day,prior_notice_last_time,prior_notice_duration_max\nBR1,2,3,12:00:00,10\n")]),
        fx("BKR_006", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min\nBR1,1,0\n")]),
        fx("BKR_007", vec![("booking_rules.txt", "booking_rule_id,booking_type\nBR1,1\n")]),
        fx("BKR_008", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_time\nBR1,2,12:00:00\n")]),
        fx("BKR_009", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_day\nBR1,2,3\n")]),
        fx("BKR_010", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_day,prior_notice_last_time,prior_notice_start_day\nBR1,2,3,12:00:00,7\n")]),
        fx("BKR_011", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_day,prior_notice_last_time,prior_notice_start_day,prior_notice_start_time\nBR1,2,5,12:00:00,3,09:00:00\n")]),
        fx("BKR_012", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_day,prior_notice_last_time,prior_notice_duration_min\nBR1,2,3,12:00:00,30\n")]),
        fx("BKR_013", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min,prior_notice_last_time\nBR1,1,30,17:00:00\n")]),
        fx("BKR_014", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min,prior_notice_service_id\nBR1,1,30,SVC1\n")]),
        // BKR_015: the field is valid for type=2, but the service is absent from calendar
        // (k4 cross-reference).
        fx("BKR_015", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_last_day,prior_notice_last_time,prior_notice_service_id\nBR1,2,3,12:00:00,MISSING_SVC\n")]),
        // BKR_016: booking_type outside the enum. BKR_019: duplicate booking_rule_id.
        fx("BKR_016", vec![("booking_rules.txt", "booking_rule_id,booking_type\nBR1,7\n")]),
        fx("BKR_019", vec![("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min\nBR1,1,30\nBR1,1,45\n")]),
        // BKR_020/021/022: the three contact fields were never read (#60).
        fx("BKR_020", vec![("booking_rules.txt", "booking_rule_id,booking_type,booking_url\nBR1,0,notaurl\n")]),
        fx("BKR_021", vec![("booking_rules.txt", "booking_rule_id,booking_type,info_url\nBR1,0,notaurl\n")]),
        fx("BKR_022", vec![("booking_rules.txt", "booking_rule_id,booking_type,phone_number\nBR1,0,abc\n")]),
        // BKR_023: a prior_notice numeric field is not an integer (opt_int used to swallow it).
        // FOUR rows: BKR_023 measures all four fields, with Entity (booking_rule_id) dedup →
        // a separate record is needed for each field, or only one is anchored (CAL_002 lesson).
        fx("BKR_023", vec![("booking_rules.txt", concat!(
            "booking_rule_id,booking_type,prior_notice_duration_min,prior_notice_duration_max,prior_notice_last_day,prior_notice_start_day\n",
            "BR1,2,abc,,,\n",
            "BR2,2,,abc,,\n",
            "BR3,2,,,abc,\n",
            "BR4,2,,,,abc\n",
        ))]),
        fx("BKR_024", vec![("booking_rules.txt","booking_rule_id,booking_type,prior_notice_duration_min,prior_notice_duration_max,prior_notice_start_day,prior_notice_last_day\nB1,1,30,120,2,3\n")]),
        // BKR_025: a prior_notice time field cannot be parsed as GTFS Time.
        // TWO rows: the rule measures both fields (start_time / last_time) and deduplicates
        // by Entity — one row anchors only one of them (the BKR_023 lesson).
        fx("BKR_025", vec![("booking_rules.txt", concat!(
            "booking_rule_id,booking_type,prior_notice_start_day,prior_notice_start_time,prior_notice_last_day,prior_notice_last_time\n",
            "BR1,2,3,yarin sabah,3,12:00:00\n",
            "BR2,2,3,08:00:00,3,12:60\n",
        ))]),
        // BKR_017/018: booking_rule_id in stop_times is absent from booking_rules.txt (k4 cross-reference).
        fx("BKR_017", vec![
            ("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min\nBR1,1,30\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id\nT1,08:00:00,08:00:00,S1,1,,,,\nT1,,,,2,LOC1,09:00:00,10:00:00,MISSING_BR\n"),
        ]),
        fx("BKR_018", vec![
            ("booking_rules.txt", "booking_rule_id,booking_type,prior_notice_duration_min\nBR1,1,30\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,drop_off_booking_rule_id\nT1,08:00:00,08:00:00,S1,1,,,,\nT1,,,,2,LOC1,09:00:00,10:00:00,MISSING_BR\n"),
        ]),

        // ── ARS: duplicate areas.txt area_id ───────────────────────────────────
        fx("ARS_001", vec![("areas.txt", "area_id,area_name\nA1,Area1\nA1,Area2\n")]),

        // ── CAL group (calendar.txt) ───────────────────────────────────────────
        fx("CAL_001", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\nSVC1,1,1,1,1,1,0,0,20250101,20271231\n")]),
        // CAL_002 also checks all SEVEN days; the fixture used to break only `monday`, so
        // coverage measurement called the other six columns "unanchored" — not a rule gap,
        // but a fixture weakness.
        //
        // ⚠️ Breaking all seven days in one row is NOT ENOUGH: CAL_002 deduplicates at Entity
        // (`service_id`) level, so each service produces only ONE notice and only one day is anchored.
        // Therefore use seven SEPARATE services, each breaking a different day.
        fx("CAL_002", vec![("calendar.txt", concat!(
            "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n",
            "SVC1,5,1,1,1,1,0,0,20250101,20271231\n",
            "SVC2,1,5,1,1,1,0,0,20250101,20271231\n",
            "SVC3,1,1,5,1,1,0,0,20250101,20271231\n",
            "SVC4,1,1,1,5,1,0,0,20250101,20271231\n",
            "SVC5,1,1,1,1,5,0,0,20250101,20271231\n",
            "SVC6,1,1,1,1,1,5,0,20250101,20271231\n",
            "SVC7,1,1,1,1,1,0,5,20250101,20271231\n",
        ))]),
        fx("CAL_003", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,,20271231\n")]),
        fx("CAL_004", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,\n")]),
        fx("CAL_005", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20271231,20250101\n")]),
        fx("CAL_022", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n,1,1,1,1,1,0,0,20250101,20271231\n")]),

        // ── CLD group (calendar_dates.txt; absent from baseline) ──────────────
        fx("CLD_001", vec![("calendar_dates.txt", "service_id,date,exception_type\n,20260601,1\n")]),
        fx("CLD_002", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,notadate,1\n")]),
        fx("CLD_003", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260601,5\n")]),
        fx("CLD_005", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,30000601,1\n")]),

        // ── FAR group (fare_attributes.txt; Fares v1) ──────────────────────────
        fx("FAR_002", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,-1,USD,0\n")]),
        fx("FAR_003", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,usd,0\n")]),
        fx("FAR_004", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,5\n")]),
        fx("FAR_005", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfers\nF1,2.5,USD,0,9\n")]),
        fx("FAR_006", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfer_duration\nF1,2.5,USD,0,-1\n")]),
        fx("FAR_011", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,\n")]),
        fx("FAR_012", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\n,2.5,USD,0\n")]),
        fx("FAR_013", vec![("fare_attributes.txt","fare_id,price,currency_type,payment_method,transfers\nF1,0.9,EUR,0,0\n")]),

        // ── FIN group (feed_info.txt; absent from baseline) ────────────────────
        fx("FIN_001", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\n,https://x.example,en\n")]),
        fx("FIN_002", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,notaurl,en\n")]),
        fx("FIN_003", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,!!bad\n")]),
        fx("FIN_004", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,default_lang\nPub,https://x.example,en,!!bad\n")]),
        fx("FIN_005", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date\nPub,https://x.example,en,notadate\n")]),
        fx("FIN_006", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_end_date\nPub,https://x.example,en,notadate\n")]),

        // ── FLG: rule_priority negatif (fare_leg_rules.txt) ────────────────────
        fx("FLG_007", vec![("fare_leg_rules.txt", "leg_group_id,rule_priority\nLG1,-5\n")]),
        // FLJ_001..004: fare_leg_join_rules.txt (#59) — the file was not recognized at all
        // until 2026-08-02. Network fields are unconditionally Required; stop fields are
        // mutually conditional.
        fx("FLJ_001", vec![
            ("networks.txt", "network_id,network_name\nN1,Net1\n"),
            ("fare_leg_join_rules.txt", "from_network_id,to_network_id\nNOPE,N1\n"),
        ]),
        fx("FLJ_002", vec![
            ("networks.txt", "network_id,network_name\nN1,Net1\n"),
            ("fare_leg_join_rules.txt", "from_network_id,to_network_id\nN1,NOPE\n"),
        ]),
        // FLJ_003 measures TWO requirements; two rows prove both:
        //   row 1 — from_stop_id empty but to_stop_id populated → mutual CONDITION violation
        //   row 2 — from_stop_id populated but absent from stops.txt → FOREIGN KEY violation
        fx("FLJ_003", vec![
            ("networks.txt", "network_id,network_name\nN1,Net1\n"),
            ("fare_leg_join_rules.txt", concat!(
                "from_network_id,to_network_id,from_stop_id,to_stop_id\n",
                "N1,N1,,S1\n",
                "N1,N1,NOPE,S1\n",
            )),
        ]),
        // FLJ_004 follows the same pattern: row 1 foreign key, row 2 mutual condition.
        fx("FLJ_004", vec![
            ("networks.txt", "network_id,network_name\nN1,Net1\n"),
            ("fare_leg_join_rules.txt", concat!(
                "from_network_id,to_network_id,from_stop_id,to_stop_id\n",
                "N1,N1,S1,NOPE\n",
                "N1,N1,S1,\n",
            )),
        ]),

        // ── FRQ group (frequencies.txt; trip_id=T1 exists in the base) ─────────
        fx("FRQ_001", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\n,08:00:00,10:00:00,600\n")]),
        fx("FRQ_002", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,,10:00:00,600\n")]),
        fx("FRQ_003", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,08:00:00,,600\n")]),
        fx("FRQ_004", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,08:00:00,10:00:00,\n")]),
        fx("FRQ_005", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,10:00:00,08:00:00,600\n")]),
        fx("FRQ_007", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs,exact_times\nT1,08:00:00,10:00:00,600,5\n")]),
        fx("FRQ_008", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,08:00:00,10:00:00,0\n")]),
        fx("FRQ_009", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,08:00:00,10:00:00,30\n")]),

        // ── FTR group (fare_transfer_rules.txt; K2 field checks) ──────────────
        fx("FTR_001", vec![("fare_transfer_rules.txt", "from_leg_group_id,fare_transfer_type\nLG1,5\n")]),
        fx("FTR_005", vec![("fare_transfer_rules.txt", "from_leg_group_id,fare_transfer_type,duration_limit_type,duration_limit\nLG1,0,9,100\n")]),
        fx("FTR_006", vec![("fare_transfer_rules.txt", "from_leg_group_id,fare_transfer_type,duration_limit\nLG1,0,0\n")]),
        fx("FTR_007", vec![("fare_transfer_rules.txt", "from_leg_group_id,fare_transfer_type,duration_limit_type\nLG1,0,1\n")]),
        fx("FTR_008", vec![("fare_transfer_rules.txt", "from_leg_group_id,fare_transfer_type,transfer_count\nLG1,0,-5\n")]),

        // ── FMD / FPD (fare_media.txt / fare_products.txt) ─────────────────────
        fx("FMD_002", vec![("fare_media.txt", "fare_media_id,fare_media_type\nM1,9\n")]),
        fx("FMD_003", vec![("fare_media.txt", "fare_media_id,fare_media_type\nM1,2\n")]),
        // FPD_002 now measures a MISSING/NON-NUMERIC amount. A negative amount is VALID
        // (spec: "May be negative to represent transfer discounts") → the fixture is non-numeric.
        fx("FPD_002", vec![("fare_products.txt", "fare_product_id,amount,currency\nP1,abc,USD\n")]),

        // ── LVL (levels.txt) ───────────────────────────────────────────────────
        fx("LVL_007", vec![("levels.txt", "level_id,level_index\nL1,\n")]),
        fx("LVL_008", vec![("levels.txt", "level_id,level_index\n,0\n")]),

        // ── RCT (rider_categories.txt) ─────────────────────────────────────────
        fx("RCT_002", vec![("rider_categories.txt", "rider_category_id,rider_category_name\nRC1,\n")]),
        fx("RCT_003", vec![("rider_categories.txt", "rider_category_id,rider_category_name,is_default_fare_category\nRC1,Adult,5\n")]),
        fx("RCT_004", vec![("rider_categories.txt", "rider_category_id,rider_category_name,min_age\nRC1,Adult,abc\n")]),
        fx("RCT_005", vec![("rider_categories.txt", "rider_category_id,rider_category_name,min_age,max_age\nRC1,Adult,65,18\n")]),
        // RCT_007: eligibility_url is not URL-shaped.
        fx("RCT_007", vec![("rider_categories.txt", "rider_category_id,rider_category_name,eligibility_url\nRC1,Adult,notaurl\n")]),

        // ── TFR (timeframes.txt) ───────────────────────────────────────────────
        fx("TFR_001", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\n,08:00:00,10:00:00,SVC1\n")]),
        fx("TFR_003", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\nTG1,notatime,10:00:00,SVC1\n")]),
        fx("TFR_004", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\nTG1,10:00:00,08:00:00,SVC1\n")]),
        fx("TFR_005", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\nTG1,08:00:00,12:00:00,SVC1\nTG1,10:00:00,14:00:00,SVC1\n")]),
        fx("TFR_006", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\nTG1,25:00:00,26:00:00,SVC1\n")]),
        // TWO directions: TFR_007 names the missing side; one row anchors only one side.
        fx("TFR_007", vec![("timeframes.txt", concat!(
            "timeframe_group_id,start_time,end_time,service_id\n",
            "TG1,08:00:00,,SVC1\n",
            "TG2,,17:00:00,SVC1\n",
        ))]),
        fx("ARC_030", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,\"Test\nTransit\",http://x.com,UTC\n")]),
        // ARC_032: `<BR>` form observed in the wild (mdb-1924, 1176 stop names).
        fx("ARC_032", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Ana<br>Durak,41.0,29.0\n")]),
        fx("AGN_018", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n,Test,http://test.example,UTC\n"), ("routes.txt", "route_id,route_short_name,route_type\nR1,101,3\n")]),
        fx("STP_045", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,platform_code\nS1,Stop1,41.0,29.0,1\nS2,Stop2,41.1,29.1,\n")]),
        fx("ARC_037", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nSüd,Stop3,41.2,29.2\n")]),
        fx("ARS_002", vec![("areas.txt","area_id,area_name\n,Bolge\n")]),
        // ⚠️ SEVEN SEPARATE SERVICES ARE REQUIRED. Clearing all seven days in one row is
        // NOT ENOUGH: dedup leaves one notice for the row and anchors only one day field.
        // The same trap occurred in the CAL_002 fixture (coverage-ledger triage, 2026-08-03).
        fx("CAL_025", vec![("calendar.txt","service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,,1,1,1,1,1,1,20250101,20261231\nSVC2,1,,1,1,1,1,1,20250101,20261231\nSVC3,1,1,,1,1,1,1,20250101,20261231\nSVC4,1,1,1,,1,1,1,20250101,20261231\nSVC5,1,1,1,1,,1,1,20250101,20261231\nSVC6,1,1,1,1,1,,1,20250101,20261231\nSVC7,1,1,1,1,1,1,,20250101,20261231\n")]),
        fx("FMD_004", vec![("fare_media.txt","fare_media_id,fare_media_name,fare_media_type\n,Kart,2\n")]),
        fx("NET_004", vec![("networks.txt","network_id,network_name\n,Ag\n")]),
        fx("RCT_008", vec![("rider_categories.txt","rider_category_id,rider_category_name\n,Yetiskin\n")]),
        fx("XFL_032", vec![("location_groups.txt","location_group_id,location_group_name\n,Grup\n")]),
        fx("XFL_033", vec![("location_group_stops.txt","location_group_id,stop_id\n,S1\n")]),
        fx("XFL_034", vec![("location_group_stops.txt","location_group_id,stop_id\nLG1,\n"), ("location_groups.txt","location_group_id,location_group_name\nLG1,Grup\n")]),
        fx("SAR_003", vec![("stop_areas.txt","area_id,stop_id\n,S1\n")]),
        fx("SAR_004", vec![("stop_areas.txt","area_id,stop_id\nA1,\n"), ("areas.txt","area_id,area_name\nA1,Bolge\n")]),
        fx("TFR_008", vec![("timeframes.txt","timeframe_group_id,start_time,end_time,service_id\nTG1,08:00:00,10:00:00,\n")]),
        fx("TRP_035", vec![("trips.txt","route_id,service_id,trip_id\nR1,,T1\n")]),

        // ── STP group (stops.txt; K2 field checks) ─────────────────────────────
        fx("STP_002", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\n,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        fx("STP_004", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,abc,29.0\nS2,Stop2,41.1,29.1\n")]),
        fx("STP_005", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,200\nS2,Stop2,41.1,29.1\n")]),
        fx("STP_006", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,,29.0\nS2,Stop2,41.1,29.1\n")]),
        fx("STP_007", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,\nS2,Stop2,41.1,29.1\n")]),
        fx("STP_008", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Stop1,41.0,29.0,9\nS2,Stop2,41.1,29.1,0\n")]),
        fx("STP_013", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,wheelchair_boarding\nS1,Stop1,41.0,29.0,9\nS2,Stop2,41.1,29.1,0\n")]),
        fx("STP_014", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_timezone\nS1,Stop1,41.0,29.0,NotAZone\nS2,Stop2,41.1,29.1,UTC\n")]),
        fx("STP_018", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\n")]),

        // ── RTS group (routes.txt; K2 field checks) ────────────────────────────
        fx("RTS_003", vec![("routes.txt", "route_id,agency_id,route_type\nR1,1,3\n")]),
        fx("RTS_004", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,9999\n")]),
        fx("RTS_005", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,route_url\nR1,1,101,3,notaurl\n")]),
        // RTS_030: extended (HVT) route_type — outside the core enum, not RTS_004 (#93).
        fx("RTS_030", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,1501\n")]),
        // RTS_029: route_sort_order is not a non-negative integer (formerly silent).
        fx("RTS_029", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,route_sort_order\nR1,1,101,3,-5\n")]),
        fx("RTS_006", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,route_color\nR1,1,101,3,XYZ\n")]),
        fx("RTS_013", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,continuous_pickup\nR1,1,101,3,9\n")]),
        fx("RTS_018", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,continuous_drop_off\nR1,1,101,3,9\n")]),
        // RTS_026: same short name, DIFFERENT long name (→ not RTS_019, Bilgi).
        fx("RTS_026", vec![("routes.txt", "route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,100,Line North,3\nR2,1,100,Line South,3\n")]),
        // RTS_027: same long name, DIFFERENT short name (→ not RTS_019, Bilgi).
        fx("RTS_027", vec![("routes.txt", "route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,10,City Line,3\nR2,1,20,City Line,3\n")]),
        // RTS_028: one trip on the route has a Flex window → routes.continuous_pickup=0 forbidden (K4).
        fx("RTS_028", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,continuous_pickup\nR1,1,101,3,0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id\nT1,08:00:00,08:00:00,S1,1,,,,\nT1,,,,2,LOC1,09:00:00,10:00:00,BR1\n"),
        ]),

        // ── TRP group (trips.txt; K2 field checks) ─────────────────────────────
        fx("TRP_001", vec![("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,\n")]),
        fx("TRP_005", vec![("trips.txt", "route_id,service_id,trip_id,direction_id\nR1,SVC1,T1,5\n")]),
        fx("TRP_006", vec![("trips.txt", "route_id,service_id,trip_id,wheelchair_accessible\nR1,SVC1,T1,9\n")]),
        fx("TRP_007", vec![("trips.txt", "route_id,service_id,trip_id,bikes_allowed\nR1,SVC1,T1,9\n")]),
        fx("TRP_031", vec![("trips.txt", "route_id,service_id,trip_id\n,SVC1,T1\n")]),
        fx("TRP_032", vec![("trips.txt", "route_id,service_id,trip_id,cars_allowed\nR1,SVC1,T1,9\n")]),

        // ── TRF group (transfers.txt) ──────────────────────────────────────────
        fx("TRF_001", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type\n,S2,1\n")]),
        fx("TRF_002", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type\nS1,,1\n")]),
        fx("TRF_020", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,min_transfer_time\nS1,S2,2,60\n")]),
        fx("TRF_004", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type\nS1,S2,9\n")]),
        fx("TRF_005", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,min_transfer_time\nS1,S2,2,abc\n")]),

        // ── SHP group (shapes.txt; K2) ─────────────────────────────────────────
        fx("SHP_001", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\n,41.0,29.0,1\n")]),
        fx("SHP_002", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,999,29.0,1\n")]),
        fx("SHP_003", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,999,1\n")]),
        fx("SHP_004", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,\n")]),
        fx("SHP_005", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,41.0,29.0,1,10\nSH1,41.1,29.1,2,5\n")]),
        fx("SHP_008", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\nSH1,41.1,29.1,1\n")]),

        // ── TRN group (translations.txt; K2) ───────────────────────────────────
        fx("TRN_001", vec![("translations.txt", "table_name,field_name,language,translation\nbadtable,stop_name,en,X\n")]),
        fx("TRN_002", vec![("translations.txt", "table_name,field_name,language,translation\nstops,,en,X\n")]),
        fx("TRN_003", vec![("translations.txt", "table_name,field_name,language,translation\nstops,stop_name,!!bad,X\n")]),
        fx("TRN_009", vec![("translations.txt", "table_name,field_name,language,translation,record_id,field_value\nstops,stop_name,en,X,S1,val\n")]),
        fx("TRN_011", vec![("translations.txt", "table_name,field_name,language,translation\nstops,stop_lat,en,X\n")]),

        // ── STM group (stop_times.txt; K2 row checks) ──────────────────────────
        fx("STM_003", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,notatime,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n")]),
        fx("STM_004", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,notatime,S1,1\nT1,08:10:00,08:10:00,S2,2\n")]),
        fx("STM_005", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,\nT1,08:10:00,08:10:00,S2,2\n")]),
        fx("STM_006", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,,1\nT1,08:10:00,08:10:00,S2,2\n")]),
        fx("STM_009", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,pickup_type\nT1,08:00:00,08:00:00,S1,1,9\nT1,08:10:00,08:10:00,S2,2,0\n")]),
        fx("STM_010", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,drop_off_type\nT1,08:00:00,08:00:00,S1,1,9\nT1,08:10:00,08:10:00,S2,2,0\n")]),
        fx("STM_018", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,continuous_pickup\nT1,08:00:00,08:00:00,S1,1,9\nT1,08:10:00,08:10:00,S2,2,1\n")]),
        fx("STM_019", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,continuous_drop_off\nT1,08:00:00,08:00:00,S1,1,9\nT1,08:10:00,08:10:00,S2,2,1\n")]),
        fx("STM_022", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,timepoint\nT1,08:00:00,08:00:00,S1,1,5\nT1,08:10:00,08:10:00,S2,2,1\n")]),
        fx("STM_030", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,-5\nT1,08:10:00,08:10:00,S2,2,10\n")]),
        fx("STM_034", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,,S1,1\nT1,08:10:00,08:10:00,S2,2\n")]),
        fx("STM_046", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n")]),
        fx("STM_047", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,timepoint\nT1,,,S1,1,1\nT1,08:10:00,08:10:00,S2,2,1\n")]),
        // Flex (has_flex_cols → window columns)
        fx("STM_037", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window\nT1,08:00:00,08:00:00,S1,1,09:00:00,10:00:00\n")]),
        fx("STM_038", vec![("stop_times.txt", "trip_id,stop_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window\nT1,S1,1,10:00:00,09:00:00\n")]),
        // TWO trips: STM_039 names the missing window field; one row anchors only one.
        // end_ is missing in T1, start_ is missing in T2.
        fx("STM_039", vec![("stop_times.txt", concat!(
            "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window\n",
            "T1,1,LOC1,09:00:00,\n",
            "T2,1,LOC1,,17:00:00\n",
        ))]),
        // STM_058: a window field cannot be parsed as Time (formerly silently swallowed).
        // TWO rows: STM_058 measures both window fields but deduplicates at Entity (trip_id),
        // so breaking both in one row anchors only ONE (same lesson as CAL_002).
        fx("STM_058", vec![("stop_times.txt", concat!(
            "trip_id,stop_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window\n",
            "T1,S1,1,9am,10:00:00\n",
            "T2,S1,1,09:00:00,10pm\n",
        ))]),
        fx("STM_040", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window\nT1,1,LOC1,09:00:00,10:00:00\n")]),
        // STM_060: TWO DIFFERENT areas on one trip — their geometries intersect, their windows
        // overlap, and both offer boarding. ⚠️ Two records for the SAME area are NOT a VIOLATION
        // (the spec requires this for intra-area travel) → the fixture deliberately uses DIFFERENT IDs.
        // OPR_024: too many trips per route. The threshold is `config.max_trips_per_route` (default 500,
        // configurable minimum 50) → the fixture lowers it to 50 and writes 51 trips.
        // The 2026-08-07 external audit said "7 fixture debts STILL 7"; all ledger entries
        // claimed "cannot be written inline", but THIS rule could be written with config.
        // VAT_006: >=50 trips, >=3 routes AND one route >=40% of total (R1 25/50). Removed from
        // the debt ledger on 2026-08-07 — the "cannot be written inline" label was WRONG; 50 rows suffice.
        fx("VAT_006", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,3\nR3,1,103,3\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\nR1,SVC1,T4\nR1,SVC1,T5\nR1,SVC1,T6\nR1,SVC1,T7\nR1,SVC1,T8\nR1,SVC1,T9\nR1,SVC1,T10\nR1,SVC1,T11\nR1,SVC1,T12\nR1,SVC1,T13\nR1,SVC1,T14\nR1,SVC1,T15\nR1,SVC1,T16\nR1,SVC1,T17\nR1,SVC1,T18\nR1,SVC1,T19\nR1,SVC1,T20\nR1,SVC1,T21\nR1,SVC1,T22\nR1,SVC1,T23\nR1,SVC1,T24\nR1,SVC1,T25\nR2,SVC1,T26\nR2,SVC1,T27\nR2,SVC1,T28\nR2,SVC1,T29\nR2,SVC1,T30\nR2,SVC1,T31\nR2,SVC1,T32\nR2,SVC1,T33\nR2,SVC1,T34\nR2,SVC1,T35\nR2,SVC1,T36\nR2,SVC1,T37\nR2,SVC1,T38\nR3,SVC1,T39\nR3,SVC1,T40\nR3,SVC1,T41\nR3,SVC1,T42\nR3,SVC1,T43\nR3,SVC1,T44\nR3,SVC1,T45\nR3,SVC1,T46\nR3,SVC1,T47\nR3,SVC1,T48\nR3,SVC1,T49\nR3,SVC1,T50\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:01:00,08:01:00,S1,1\nT1,09:01:00,09:01:00,S2,2\nT2,08:02:00,08:02:00,S1,1\nT2,09:02:00,09:02:00,S2,2\nT3,08:03:00,08:03:00,S1,1\nT3,09:03:00,09:03:00,S2,2\nT4,08:04:00,08:04:00,S1,1\nT4,09:04:00,09:04:00,S2,2\nT5,08:05:00,08:05:00,S1,1\nT5,09:05:00,09:05:00,S2,2\nT6,08:06:00,08:06:00,S1,1\nT6,09:06:00,09:06:00,S2,2\nT7,08:07:00,08:07:00,S1,1\nT7,09:07:00,09:07:00,S2,2\nT8,08:08:00,08:08:00,S1,1\nT8,09:08:00,09:08:00,S2,2\nT9,08:09:00,08:09:00,S1,1\nT9,09:09:00,09:09:00,S2,2\nT10,08:10:00,08:10:00,S1,1\nT10,09:10:00,09:10:00,S2,2\nT11,08:11:00,08:11:00,S1,1\nT11,09:11:00,09:11:00,S2,2\nT12,08:12:00,08:12:00,S1,1\nT12,09:12:00,09:12:00,S2,2\nT13,08:13:00,08:13:00,S1,1\nT13,09:13:00,09:13:00,S2,2\nT14,08:14:00,08:14:00,S1,1\nT14,09:14:00,09:14:00,S2,2\nT15,08:15:00,08:15:00,S1,1\nT15,09:15:00,09:15:00,S2,2\nT16,08:16:00,08:16:00,S1,1\nT16,09:16:00,09:16:00,S2,2\nT17,08:17:00,08:17:00,S1,1\nT17,09:17:00,09:17:00,S2,2\nT18,08:18:00,08:18:00,S1,1\nT18,09:18:00,09:18:00,S2,2\nT19,08:19:00,08:19:00,S1,1\nT19,09:19:00,09:19:00,S2,2\nT20,08:20:00,08:20:00,S1,1\nT20,09:20:00,09:20:00,S2,2\nT21,08:21:00,08:21:00,S1,1\nT21,09:21:00,09:21:00,S2,2\nT22,08:22:00,08:22:00,S1,1\nT22,09:22:00,09:22:00,S2,2\nT23,08:23:00,08:23:00,S1,1\nT23,09:23:00,09:23:00,S2,2\nT24,08:24:00,08:24:00,S1,1\nT24,09:24:00,09:24:00,S2,2\nT25,08:25:00,08:25:00,S1,1\nT25,09:25:00,09:25:00,S2,2\nT26,08:26:00,08:26:00,S1,1\nT26,09:26:00,09:26:00,S2,2\nT27,08:27:00,08:27:00,S1,1\nT27,09:27:00,09:27:00,S2,2\nT28,08:28:00,08:28:00,S1,1\nT28,09:28:00,09:28:00,S2,2\nT29,08:29:00,08:29:00,S1,1\nT29,09:29:00,09:29:00,S2,2\nT30,08:30:00,08:30:00,S1,1\nT30,09:30:00,09:30:00,S2,2\nT31,08:31:00,08:31:00,S1,1\nT31,09:31:00,09:31:00,S2,2\nT32,08:32:00,08:32:00,S1,1\nT32,09:32:00,09:32:00,S2,2\nT33,08:33:00,08:33:00,S1,1\nT33,09:33:00,09:33:00,S2,2\nT34,08:34:00,08:34:00,S1,1\nT34,09:34:00,09:34:00,S2,2\nT35,08:35:00,08:35:00,S1,1\nT35,09:35:00,09:35:00,S2,2\nT36,08:36:00,08:36:00,S1,1\nT36,09:36:00,09:36:00,S2,2\nT37,08:37:00,08:37:00,S1,1\nT37,09:37:00,09:37:00,S2,2\nT38,08:38:00,08:38:00,S1,1\nT38,09:38:00,09:38:00,S2,2\nT39,08:39:00,08:39:00,S1,1\nT39,09:39:00,09:39:00,S2,2\nT40,08:40:00,08:40:00,S1,1\nT40,09:40:00,09:40:00,S2,2\nT41,08:41:00,08:41:00,S1,1\nT41,09:41:00,09:41:00,S2,2\nT42,08:42:00,08:42:00,S1,1\nT42,09:42:00,09:42:00,S2,2\nT43,08:43:00,08:43:00,S1,1\nT43,09:43:00,09:43:00,S2,2\nT44,08:44:00,08:44:00,S1,1\nT44,09:44:00,09:44:00,S2,2\nT45,08:45:00,08:45:00,S1,1\nT45,09:45:00,09:45:00,S2,2\nT46,08:46:00,08:46:00,S1,1\nT46,09:46:00,09:46:00,S2,2\nT47,08:47:00,08:47:00,S1,1\nT47,09:47:00,09:47:00,S2,2\nT48,08:48:00,08:48:00,S1,1\nT48,09:48:00,09:48:00,S2,2\nT49,08:49:00,08:49:00,S1,1\nT49,09:49:00,09:49:00,S2,2\nT50,08:50:00,08:50:00,S1,1\nT50,09:50:00,09:50:00,S2,2\n"),
        ]),
        // STM_043: more than 200 stop_times in one trip. This could likewise be written inline.
        fx("STM_043", vec![
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,06:01:00,06:01:00,S1,1\nT1,06:02:00,06:02:00,S2,2\nT1,06:03:00,06:03:00,S1,3\nT1,06:04:00,06:04:00,S2,4\nT1,06:05:00,06:05:00,S1,5\nT1,06:06:00,06:06:00,S2,6\nT1,06:07:00,06:07:00,S1,7\nT1,06:08:00,06:08:00,S2,8\nT1,06:09:00,06:09:00,S1,9\nT1,06:10:00,06:10:00,S2,10\nT1,06:11:00,06:11:00,S1,11\nT1,06:12:00,06:12:00,S2,12\nT1,06:13:00,06:13:00,S1,13\nT1,06:14:00,06:14:00,S2,14\nT1,06:15:00,06:15:00,S1,15\nT1,06:16:00,06:16:00,S2,16\nT1,06:17:00,06:17:00,S1,17\nT1,06:18:00,06:18:00,S2,18\nT1,06:19:00,06:19:00,S1,19\nT1,06:20:00,06:20:00,S2,20\nT1,06:21:00,06:21:00,S1,21\nT1,06:22:00,06:22:00,S2,22\nT1,06:23:00,06:23:00,S1,23\nT1,06:24:00,06:24:00,S2,24\nT1,06:25:00,06:25:00,S1,25\nT1,06:26:00,06:26:00,S2,26\nT1,06:27:00,06:27:00,S1,27\nT1,06:28:00,06:28:00,S2,28\nT1,06:29:00,06:29:00,S1,29\nT1,06:30:00,06:30:00,S2,30\nT1,06:31:00,06:31:00,S1,31\nT1,06:32:00,06:32:00,S2,32\nT1,06:33:00,06:33:00,S1,33\nT1,06:34:00,06:34:00,S2,34\nT1,06:35:00,06:35:00,S1,35\nT1,06:36:00,06:36:00,S2,36\nT1,06:37:00,06:37:00,S1,37\nT1,06:38:00,06:38:00,S2,38\nT1,06:39:00,06:39:00,S1,39\nT1,06:40:00,06:40:00,S2,40\nT1,06:41:00,06:41:00,S1,41\nT1,06:42:00,06:42:00,S2,42\nT1,06:43:00,06:43:00,S1,43\nT1,06:44:00,06:44:00,S2,44\nT1,06:45:00,06:45:00,S1,45\nT1,06:46:00,06:46:00,S2,46\nT1,06:47:00,06:47:00,S1,47\nT1,06:48:00,06:48:00,S2,48\nT1,06:49:00,06:49:00,S1,49\nT1,06:50:00,06:50:00,S2,50\nT1,06:51:00,06:51:00,S1,51\nT1,06:52:00,06:52:00,S2,52\nT1,06:53:00,06:53:00,S1,53\nT1,06:54:00,06:54:00,S2,54\nT1,06:55:00,06:55:00,S1,55\nT1,06:56:00,06:56:00,S2,56\nT1,06:57:00,06:57:00,S1,57\nT1,06:58:00,06:58:00,S2,58\nT1,06:59:00,06:59:00,S1,59\nT1,07:00:00,07:00:00,S2,60\nT1,07:01:00,07:01:00,S1,61\nT1,07:02:00,07:02:00,S2,62\nT1,07:03:00,07:03:00,S1,63\nT1,07:04:00,07:04:00,S2,64\nT1,07:05:00,07:05:00,S1,65\nT1,07:06:00,07:06:00,S2,66\nT1,07:07:00,07:07:00,S1,67\nT1,07:08:00,07:08:00,S2,68\nT1,07:09:00,07:09:00,S1,69\nT1,07:10:00,07:10:00,S2,70\nT1,07:11:00,07:11:00,S1,71\nT1,07:12:00,07:12:00,S2,72\nT1,07:13:00,07:13:00,S1,73\nT1,07:14:00,07:14:00,S2,74\nT1,07:15:00,07:15:00,S1,75\nT1,07:16:00,07:16:00,S2,76\nT1,07:17:00,07:17:00,S1,77\nT1,07:18:00,07:18:00,S2,78\nT1,07:19:00,07:19:00,S1,79\nT1,07:20:00,07:20:00,S2,80\nT1,07:21:00,07:21:00,S1,81\nT1,07:22:00,07:22:00,S2,82\nT1,07:23:00,07:23:00,S1,83\nT1,07:24:00,07:24:00,S2,84\nT1,07:25:00,07:25:00,S1,85\nT1,07:26:00,07:26:00,S2,86\nT1,07:27:00,07:27:00,S1,87\nT1,07:28:00,07:28:00,S2,88\nT1,07:29:00,07:29:00,S1,89\nT1,07:30:00,07:30:00,S2,90\nT1,07:31:00,07:31:00,S1,91\nT1,07:32:00,07:32:00,S2,92\nT1,07:33:00,07:33:00,S1,93\nT1,07:34:00,07:34:00,S2,94\nT1,07:35:00,07:35:00,S1,95\nT1,07:36:00,07:36:00,S2,96\nT1,07:37:00,07:37:00,S1,97\nT1,07:38:00,07:38:00,S2,98\nT1,07:39:00,07:39:00,S1,99\nT1,07:40:00,07:40:00,S2,100\nT1,07:41:00,07:41:00,S1,101\nT1,07:42:00,07:42:00,S2,102\nT1,07:43:00,07:43:00,S1,103\nT1,07:44:00,07:44:00,S2,104\nT1,07:45:00,07:45:00,S1,105\nT1,07:46:00,07:46:00,S2,106\nT1,07:47:00,07:47:00,S1,107\nT1,07:48:00,07:48:00,S2,108\nT1,07:49:00,07:49:00,S1,109\nT1,07:50:00,07:50:00,S2,110\nT1,07:51:00,07:51:00,S1,111\nT1,07:52:00,07:52:00,S2,112\nT1,07:53:00,07:53:00,S1,113\nT1,07:54:00,07:54:00,S2,114\nT1,07:55:00,07:55:00,S1,115\nT1,07:56:00,07:56:00,S2,116\nT1,07:57:00,07:57:00,S1,117\nT1,07:58:00,07:58:00,S2,118\nT1,07:59:00,07:59:00,S1,119\nT1,08:00:00,08:00:00,S2,120\nT1,08:01:00,08:01:00,S1,121\nT1,08:02:00,08:02:00,S2,122\nT1,08:03:00,08:03:00,S1,123\nT1,08:04:00,08:04:00,S2,124\nT1,08:05:00,08:05:00,S1,125\nT1,08:06:00,08:06:00,S2,126\nT1,08:07:00,08:07:00,S1,127\nT1,08:08:00,08:08:00,S2,128\nT1,08:09:00,08:09:00,S1,129\nT1,08:10:00,08:10:00,S2,130\nT1,08:11:00,08:11:00,S1,131\nT1,08:12:00,08:12:00,S2,132\nT1,08:13:00,08:13:00,S1,133\nT1,08:14:00,08:14:00,S2,134\nT1,08:15:00,08:15:00,S1,135\nT1,08:16:00,08:16:00,S2,136\nT1,08:17:00,08:17:00,S1,137\nT1,08:18:00,08:18:00,S2,138\nT1,08:19:00,08:19:00,S1,139\nT1,08:20:00,08:20:00,S2,140\nT1,08:21:00,08:21:00,S1,141\nT1,08:22:00,08:22:00,S2,142\nT1,08:23:00,08:23:00,S1,143\nT1,08:24:00,08:24:00,S2,144\nT1,08:25:00,08:25:00,S1,145\nT1,08:26:00,08:26:00,S2,146\nT1,08:27:00,08:27:00,S1,147\nT1,08:28:00,08:28:00,S2,148\nT1,08:29:00,08:29:00,S1,149\nT1,08:30:00,08:30:00,S2,150\nT1,08:31:00,08:31:00,S1,151\nT1,08:32:00,08:32:00,S2,152\nT1,08:33:00,08:33:00,S1,153\nT1,08:34:00,08:34:00,S2,154\nT1,08:35:00,08:35:00,S1,155\nT1,08:36:00,08:36:00,S2,156\nT1,08:37:00,08:37:00,S1,157\nT1,08:38:00,08:38:00,S2,158\nT1,08:39:00,08:39:00,S1,159\nT1,08:40:00,08:40:00,S2,160\nT1,08:41:00,08:41:00,S1,161\nT1,08:42:00,08:42:00,S2,162\nT1,08:43:00,08:43:00,S1,163\nT1,08:44:00,08:44:00,S2,164\nT1,08:45:00,08:45:00,S1,165\nT1,08:46:00,08:46:00,S2,166\nT1,08:47:00,08:47:00,S1,167\nT1,08:48:00,08:48:00,S2,168\nT1,08:49:00,08:49:00,S1,169\nT1,08:50:00,08:50:00,S2,170\nT1,08:51:00,08:51:00,S1,171\nT1,08:52:00,08:52:00,S2,172\nT1,08:53:00,08:53:00,S1,173\nT1,08:54:00,08:54:00,S2,174\nT1,08:55:00,08:55:00,S1,175\nT1,08:56:00,08:56:00,S2,176\nT1,08:57:00,08:57:00,S1,177\nT1,08:58:00,08:58:00,S2,178\nT1,08:59:00,08:59:00,S1,179\nT1,09:00:00,09:00:00,S2,180\nT1,09:01:00,09:01:00,S1,181\nT1,09:02:00,09:02:00,S2,182\nT1,09:03:00,09:03:00,S1,183\nT1,09:04:00,09:04:00,S2,184\nT1,09:05:00,09:05:00,S1,185\nT1,09:06:00,09:06:00,S2,186\nT1,09:07:00,09:07:00,S1,187\nT1,09:08:00,09:08:00,S2,188\nT1,09:09:00,09:09:00,S1,189\nT1,09:10:00,09:10:00,S2,190\nT1,09:11:00,09:11:00,S1,191\nT1,09:12:00,09:12:00,S2,192\nT1,09:13:00,09:13:00,S1,193\nT1,09:14:00,09:14:00,S2,194\nT1,09:15:00,09:15:00,S1,195\nT1,09:16:00,09:16:00,S2,196\nT1,09:17:00,09:17:00,S1,197\nT1,09:18:00,09:18:00,S2,198\nT1,09:19:00,09:19:00,S1,199\nT1,09:20:00,09:20:00,S2,200\nT1,09:21:00,09:21:00,S1,201\n"),
        ]),
        // SHP_026 / STM_044: thresholds moved to `ValidatorConfig` on 2026-08-07 (the OPR_024
        // precedent) — the only reason was that it could be written as a fixture; default
        // behavior did not change.
        // The debt ledger called these "cannot be written inline"; the truth was that the
        // threshold was hard-coded, which was fixable.
        fx_cfg("SHP_026", vec![
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.001,29.001,1\nSH1,41.002,29.002,2\nSH1,41.003,29.003,3\nSH1,41.004,29.004,4\nSH1,41.005,29.005,5\nSH1,41.006,29.006,6\nSH1,41.007,29.007,7\nSH1,41.008,29.008,8\nSH1,41.009,29.009,9\nSH1,41.010,29.010,10\nSH1,41.011,29.011,11\nSH1,41.012,29.012,12\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
        ], ValidatorConfig { max_shape_points: 10, ..ValidatorConfig::default() }),
        fx_cfg("STM_044", vec![
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,06:01:00,06:01:00,S1,1\nT1,06:02:00,06:02:00,S2,2\nT1,06:03:00,06:03:00,S1,3\nT1,06:04:00,06:04:00,S2,4\nT1,06:05:00,06:05:00,S1,5\nT1,06:06:00,06:06:00,S2,6\nT1,06:07:00,06:07:00,S1,7\nT1,06:08:00,06:08:00,S2,8\nT1,06:09:00,06:09:00,S1,9\nT1,06:10:00,06:10:00,S2,10\nT1,06:11:00,06:11:00,S1,11\nT1,06:12:00,06:12:00,S2,12\nT1,06:13:00,06:13:00,S1,13\nT1,06:14:00,06:14:00,S2,14\nT1,06:15:00,06:15:00,S1,15\nT1,06:16:00,06:16:00,S2,16\nT1,06:17:00,06:17:00,S1,17\nT1,06:18:00,06:18:00,S2,18\nT1,06:19:00,06:19:00,S1,19\nT1,06:20:00,06:20:00,S2,20\nT1,06:21:00,06:21:00,S1,21\nT1,06:22:00,06:22:00,S2,22\nT1,06:23:00,06:23:00,S1,23\nT1,06:24:00,06:24:00,S2,24\nT1,06:25:00,06:25:00,S1,25\nT1,06:26:00,06:26:00,S2,26\nT1,06:27:00,06:27:00,S1,27\nT1,06:28:00,06:28:00,S2,28\nT1,06:29:00,06:29:00,S1,29\nT1,06:30:00,06:30:00,S2,30\nT1,06:31:00,06:31:00,S1,31\nT1,06:32:00,06:32:00,S2,32\nT1,06:33:00,06:33:00,S1,33\nT1,06:34:00,06:34:00,S2,34\nT1,06:35:00,06:35:00,S1,35\nT1,06:36:00,06:36:00,S2,36\nT1,06:37:00,06:37:00,S1,37\nT1,06:38:00,06:38:00,S2,38\nT1,06:39:00,06:39:00,S1,39\nT1,06:40:00,06:40:00,S2,40\nT1,06:41:00,06:41:00,S1,41\nT1,06:42:00,06:42:00,S2,42\nT1,06:43:00,06:43:00,S1,43\nT1,06:44:00,06:44:00,S2,44\nT1,06:45:00,06:45:00,S1,45\nT1,06:46:00,06:46:00,S2,46\nT1,06:47:00,06:47:00,S1,47\nT1,06:48:00,06:48:00,S2,48\nT1,06:49:00,06:49:00,S1,49\nT1,06:50:00,06:50:00,S2,50\nT1,06:51:00,06:51:00,S1,51\nT1,06:52:00,06:52:00,S2,52\nT1,06:53:00,06:53:00,S1,53\nT1,06:54:00,06:54:00,S2,54\nT1,06:55:00,06:55:00,S1,55\nT1,06:56:00,06:56:00,S2,56\nT1,06:57:00,06:57:00,S1,57\nT1,06:58:00,06:58:00,S2,58\nT1,06:59:00,06:59:00,S1,59\nT1,07:00:00,07:00:00,S2,60\nT1,07:01:00,07:01:00,S1,61\nT1,07:02:00,07:02:00,S2,62\nT1,07:03:00,07:03:00,S1,63\nT1,07:04:00,07:04:00,S2,64\nT1,07:05:00,07:05:00,S1,65\nT1,07:06:00,07:06:00,S2,66\nT1,07:07:00,07:07:00,S1,67\nT1,07:08:00,07:08:00,S2,68\nT1,07:09:00,07:09:00,S1,69\nT1,07:10:00,07:10:00,S2,70\nT1,07:11:00,07:11:00,S1,71\nT1,07:12:00,07:12:00,S2,72\nT1,07:13:00,07:13:00,S1,73\nT1,07:14:00,07:14:00,S2,74\nT1,07:15:00,07:15:00,S1,75\nT1,07:16:00,07:16:00,S2,76\nT1,07:17:00,07:17:00,S1,77\nT1,07:18:00,07:18:00,S2,78\nT1,07:19:00,07:19:00,S1,79\nT1,07:20:00,07:20:00,S2,80\nT1,07:21:00,07:21:00,S1,81\nT1,07:22:00,07:22:00,S2,82\nT1,07:23:00,07:23:00,S1,83\nT1,07:24:00,07:24:00,S2,84\nT1,07:25:00,07:25:00,S1,85\nT1,07:26:00,07:26:00,S2,86\nT1,07:27:00,07:27:00,S1,87\nT1,07:28:00,07:28:00,S2,88\nT1,07:29:00,07:29:00,S1,89\nT1,07:30:00,07:30:00,S2,90\nT1,07:31:00,07:31:00,S1,91\nT1,07:32:00,07:32:00,S2,92\nT1,07:33:00,07:33:00,S1,93\nT1,07:34:00,07:34:00,S2,94\nT1,07:35:00,07:35:00,S1,95\nT1,07:36:00,07:36:00,S2,96\nT1,07:37:00,07:37:00,S1,97\nT1,07:38:00,07:38:00,S2,98\nT1,07:39:00,07:39:00,S1,99\nT1,07:40:00,07:40:00,S2,100\nT1,07:41:00,07:41:00,S1,101\n"),
        ], ValidatorConfig { max_total_stop_times: 100, ..ValidatorConfig::default() }),
        // ARC_033: RFC 4180 — quote in an unquoted field. `12" Street` is INVALID; the correct
        // form is `"12"" Street"`. The tokenizer treated this as an ordinary character until 2026-08-07.
        fx("ARC_033", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,12\" Street,41.0,29.0\nS2,Stop2,41.1,29.1\n"),
        ]),
        // ARC_022: file row-count threshold. On 2026-08-07 (issue #71), the threshold moved
        // from hard-coded `k1_parse` to `ValidatorConfig` — the debt ledger said "requires 1M
        // rows, cannot be written inline", but the truth was "K1 `parse()` did not accept config",
        // which was fixable. This proves the K1 path (stops.txt is read out of stream);
        // K2 streaming paths (shapes/stop_times) are proven by integration.rs::arc022_*.
        // Config minimum is 10 → 11 rows suffice.
        fx_cfg("ARC_022", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.001,29.001\nS2,Stop2,41.002,29.002\nS3,Stop3,41.003,29.003\nS4,Stop4,41.004,29.004\nS5,Stop5,41.005,29.005\nS6,Stop6,41.006,29.006\nS7,Stop7,41.007,29.007\nS8,Stop8,41.008,29.008\nS9,Stop9,41.009,29.009\nS10,Stop10,41.010,29.010\nS11,Stop11,41.011,29.011\n"),
        ], ValidatorConfig { max_file_rows: 10, ..ValidatorConfig::default() }),
        // ARC_027: the ZIP entry lacks the owner-read bit (0o200). It is triggered by zip
        // METADATA, not content, so the normal fixture path could not produce it; the harness
        // gained a constructor that writes `unix_permissions`.
        fx_noread("ARC_027", vec!["stops.txt"]),
        // ARC_036: after building the baseline feed's zip, add only bit 3 to local-header flags.
        // The central directory remains sound — the archive opens, and only the stream view is
        // inconsistent. Real counterpart: `tdg-81618` (active feed, MD therefore crashes).
        fx_stream_flag("ARC_036"),
        // STM_061: a distant NON-CONSECUTIVE pair. A→C is ~60 km as the crow flies, covered
        // in 6 minutes (600 km/h, bus threshold 120). The intermediate stop B has a late time.
        fx("STM_061", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nA,A,41.000,29.000\nB,B,41.100,29.000\nC,C,41.540,29.000\n"),
            ("routes.txt", "route_id,route_short_name,route_long_name,route_type\nR1,R,Route,3\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,S1,T1\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,A,1\nT1,08:05:00,08:05:00,B,2\nT1,08:06:00,08:06:00,C,3\n"),
        ]),
        fx_cfg("OPR_024", vec![
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\nR1,SVC1,T4\nR1,SVC1,T5\nR1,SVC1,T6\nR1,SVC1,T7\nR1,SVC1,T8\nR1,SVC1,T9\nR1,SVC1,T10\nR1,SVC1,T11\nR1,SVC1,T12\nR1,SVC1,T13\nR1,SVC1,T14\nR1,SVC1,T15\nR1,SVC1,T16\nR1,SVC1,T17\nR1,SVC1,T18\nR1,SVC1,T19\nR1,SVC1,T20\nR1,SVC1,T21\nR1,SVC1,T22\nR1,SVC1,T23\nR1,SVC1,T24\nR1,SVC1,T25\nR1,SVC1,T26\nR1,SVC1,T27\nR1,SVC1,T28\nR1,SVC1,T29\nR1,SVC1,T30\nR1,SVC1,T31\nR1,SVC1,T32\nR1,SVC1,T33\nR1,SVC1,T34\nR1,SVC1,T35\nR1,SVC1,T36\nR1,SVC1,T37\nR1,SVC1,T38\nR1,SVC1,T39\nR1,SVC1,T40\nR1,SVC1,T41\nR1,SVC1,T42\nR1,SVC1,T43\nR1,SVC1,T44\nR1,SVC1,T45\nR1,SVC1,T46\nR1,SVC1,T47\nR1,SVC1,T48\nR1,SVC1,T49\nR1,SVC1,T50\nR1,SVC1,T51\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:01:00,08:01:00,S1,1\nT1,09:01:00,09:01:00,S2,2\nT2,08:02:00,08:02:00,S1,1\nT2,09:02:00,09:02:00,S2,2\nT3,08:03:00,08:03:00,S1,1\nT3,09:03:00,09:03:00,S2,2\nT4,08:04:00,08:04:00,S1,1\nT4,09:04:00,09:04:00,S2,2\nT5,08:05:00,08:05:00,S1,1\nT5,09:05:00,09:05:00,S2,2\nT6,08:06:00,08:06:00,S1,1\nT6,09:06:00,09:06:00,S2,2\nT7,08:07:00,08:07:00,S1,1\nT7,09:07:00,09:07:00,S2,2\nT8,08:08:00,08:08:00,S1,1\nT8,09:08:00,09:08:00,S2,2\nT9,08:09:00,08:09:00,S1,1\nT9,09:09:00,09:09:00,S2,2\nT10,08:10:00,08:10:00,S1,1\nT10,09:10:00,09:10:00,S2,2\nT11,08:11:00,08:11:00,S1,1\nT11,09:11:00,09:11:00,S2,2\nT12,08:12:00,08:12:00,S1,1\nT12,09:12:00,09:12:00,S2,2\nT13,08:13:00,08:13:00,S1,1\nT13,09:13:00,09:13:00,S2,2\nT14,08:14:00,08:14:00,S1,1\nT14,09:14:00,09:14:00,S2,2\nT15,08:15:00,08:15:00,S1,1\nT15,09:15:00,09:15:00,S2,2\nT16,08:16:00,08:16:00,S1,1\nT16,09:16:00,09:16:00,S2,2\nT17,08:17:00,08:17:00,S1,1\nT17,09:17:00,09:17:00,S2,2\nT18,08:18:00,08:18:00,S1,1\nT18,09:18:00,09:18:00,S2,2\nT19,08:19:00,08:19:00,S1,1\nT19,09:19:00,09:19:00,S2,2\nT20,08:20:00,08:20:00,S1,1\nT20,09:20:00,09:20:00,S2,2\nT21,08:21:00,08:21:00,S1,1\nT21,09:21:00,09:21:00,S2,2\nT22,08:22:00,08:22:00,S1,1\nT22,09:22:00,09:22:00,S2,2\nT23,08:23:00,08:23:00,S1,1\nT23,09:23:00,09:23:00,S2,2\nT24,08:24:00,08:24:00,S1,1\nT24,09:24:00,09:24:00,S2,2\nT25,08:25:00,08:25:00,S1,1\nT25,09:25:00,09:25:00,S2,2\nT26,08:26:00,08:26:00,S1,1\nT26,09:26:00,09:26:00,S2,2\nT27,08:27:00,08:27:00,S1,1\nT27,09:27:00,09:27:00,S2,2\nT28,08:28:00,08:28:00,S1,1\nT28,09:28:00,09:28:00,S2,2\nT29,08:29:00,08:29:00,S1,1\nT29,09:29:00,09:29:00,S2,2\nT30,08:30:00,08:30:00,S1,1\nT30,09:30:00,09:30:00,S2,2\nT31,08:31:00,08:31:00,S1,1\nT31,09:31:00,09:31:00,S2,2\nT32,08:32:00,08:32:00,S1,1\nT32,09:32:00,09:32:00,S2,2\nT33,08:33:00,08:33:00,S1,1\nT33,09:33:00,09:33:00,S2,2\nT34,08:34:00,08:34:00,S1,1\nT34,09:34:00,09:34:00,S2,2\nT35,08:35:00,08:35:00,S1,1\nT35,09:35:00,09:35:00,S2,2\nT36,08:36:00,08:36:00,S1,1\nT36,09:36:00,09:36:00,S2,2\nT37,08:37:00,08:37:00,S1,1\nT37,09:37:00,09:37:00,S2,2\nT38,08:38:00,08:38:00,S1,1\nT38,09:38:00,09:38:00,S2,2\nT39,08:39:00,08:39:00,S1,1\nT39,09:39:00,09:39:00,S2,2\nT40,08:40:00,08:40:00,S1,1\nT40,09:40:00,09:40:00,S2,2\nT41,08:41:00,08:41:00,S1,1\nT41,09:41:00,09:41:00,S2,2\nT42,08:42:00,08:42:00,S1,1\nT42,09:42:00,09:42:00,S2,2\nT43,08:43:00,08:43:00,S1,1\nT43,09:43:00,09:43:00,S2,2\nT44,08:44:00,08:44:00,S1,1\nT44,09:44:00,09:44:00,S2,2\nT45,08:45:00,08:45:00,S1,1\nT45,09:45:00,09:45:00,S2,2\nT46,08:46:00,08:46:00,S1,1\nT46,09:46:00,09:46:00,S2,2\nT47,08:47:00,08:47:00,S1,1\nT47,09:47:00,09:47:00,S2,2\nT48,08:48:00,08:48:00,S1,1\nT48,09:48:00,09:48:00,S2,2\nT49,08:49:00,08:49:00,S1,1\nT49,09:49:00,09:49:00,S2,2\nT50,08:50:00,08:50:00,S1,1\nT50,09:50:00,09:50:00,S2,2\nT51,08:51:00,08:51:00,S1,1\nT51,09:51:00,09:51:00,S2,2\n"),
        ], ValidatorConfig { max_trips_per_route: 50, ..ValidatorConfig::default() }),
        fx("STM_060", vec![
            ("locations.geojson", concat!(
                r#"{"type":"FeatureCollection","features":["#,
                r#"{"type":"Feature","id":"Z1","properties":{},"geometry":{"type":"Polygon","coordinates":[[[0,0],[0,1],[1,1],[1,0],[0,0]]]}},"#,
                r#"{"type":"Feature","id":"Z2","properties":{},"geometry":{"type":"Polygon","coordinates":[[[0.5,0.5],[0.5,1.5],[1.5,1.5],[1.5,0.5],[0.5,0.5]]]}}]}"#)),
            ("stop_times.txt", concat!(
                "trip_id,location_id,stop_sequence,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_type,drop_off_type\n",
                "T1,Z1,1,08:00:00,10:00:00,2,2\n",
                "T1,Z2,2,09:00:00,11:00:00,2,2\n")),
        ]),
        fx("STM_059", vec![("stop_times.txt", "trip_id,stop_id,stop_sequence,pickup_type\nT1,S1,1,2\n"), ("booking_rules.txt","booking_rule_id,booking_type\nBR1,0\n")]),
        fx("STM_041", vec![("stop_times.txt", "trip_id,stop_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id\nT1,S1,1,LOC1,09:00:00,10:00:00,BR1\n")]),
        fx("STM_051", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id,pickup_type\nT1,1,LOC1,09:00:00,10:00:00,BR1,0\n")]),
        fx("STM_052", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id,drop_off_type\nT1,1,LOC1,09:00:00,10:00:00,BR1,0\n")]),
        // STM_054/055: continuous_pickup/drop_off is outside 1/empty while a Flex window is defined (row-wide).
        fx("STM_054", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id,continuous_pickup\nT1,1,LOC1,09:00:00,10:00:00,BR1,0\n")]),
        fx("STM_055", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window,pickup_booking_rule_id,continuous_drop_off\nT1,1,LOC1,09:00:00,10:00:00,BR1,2\n")]),

        // ── PTH group (pathways.txt; absent from baseline, added) ──────────────
        // K2 row checks (k2/pathways.rs). Clean row template: mode=3, bidir=0.
        fx("PTH_004", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,9,0\n")]),
        fx("PTH_005", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,3,5\n")]),
        fx("PTH_006", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,length\nP1,S1,S2,3,0,-1\n")]),
        fx("PTH_007", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,traversal_time\nP1,S1,S2,3,0,0\n")]),
        fx("PTH_008", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,2,0\n")]),
        // PTH_027: stair_count zero — the only value excluded by the `Non-null integer` type.
        fx("PTH_027", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,stair_count\nP1,S1,S2,2,0,0\n")]),
        fx("PTH_025", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,length\nP1,S1,S2,6,0,\n")]),
        fx("PTH_029", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,traversal_time\nP1,S1,S2,4,0,\n")]),
        fx("PTH_030", vec![("stops.txt","stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nST1,St,41.0,29.0,1,\nP1,Pl,41.0,29.0,0,ST1\nBA1,Ba,41.0,29.0,4,P1\nE1,En,41.0,29.0,2,ST1\n"), ("pathways.txt","pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nW1,E1,P1,1,0\n")]),
        fx("PTH_009", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,1,0\n")]),
        fx("PTH_010", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,min_width\nP1,S1,S2,3,0,0\n")]),
        fx("PTH_011", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S1,3,0\n")]),
        fx("PTH_016", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,7,1\n")]),
        // PTH_017 is now ONLY a type violation: max_slope is not numeric (spec type `Float`).
        fx("PTH_017", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,max_slope\nP1,S1,S2,1,0,abc\n")]),
        // PTH_028: max_slope is valid but pathway_mode is not 1/3 — the spec says "should" here.
        fx("PTH_028", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,max_slope\nP1,S1,S2,4,0,0.05\n")]),
        // TRP_034: safe_duration is not a decimal number (formerly silently swallowed).
        // TWO trips: TRP_034 also deduplicates by Entity (trip_id); breaking two fields in one
        // row anchors only one (same lesson as CAL_002).
        fx("TRP_034", vec![("trips.txt", concat!(
            "route_id,service_id,trip_id,safe_duration_factor,safe_duration_offset\n",
            "R1,SVC1,T1,abc,\n",
            "R1,SVC1,T2,,abc\n",
        ))]),
        fx("PTH_018", vec![("pathways.txt", concat!("pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,signposted_as\nP1,S1,S2,3,0,",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n"))]),
        fx("PTH_020", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\n,S1,S2,3,0\n")]),
        fx("PTH_021", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,,S2,3,0\n")]),
        fx("PTH_022", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,,3,0\n")]),
        fx("PTH_023", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,,0\n")]),
        fx("PTH_024", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,3,\n")]),
        // K3: pathway_id duplicate (k3_entity_graph::build_pathways).
        fx("PTH_001", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,3,0\nP1,S2,S1,3,0\n")]),
        // K4: cross-ref (k4_cross_ref::check_pathways).
        fx("PTH_002", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,NOPE,S2,3,0\n")]),
        // PTH_026: a pathway endpoint cannot be a station (loc=1) (k4).
        fx("PTH_026", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Station,41.0,29.0,1\nS2,Stop2,41.1,29.1,0\n"),
            ("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,3,0\n"),
        ]),
        // PTH_031: the second arm of the same spec sentence — a platform endpoint with stop_access=1.
        // `stop_access` is valid only on a platform with parent_station (otherwise STP_026/028),
        // so the fixture builds one station and two platforms.
        fx("PTH_031", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station,stop_access\nST,Station,41.0,29.0,1,,\nS1,Platform1,41.0,29.0,0,ST,1\nS2,Platform2,41.1,29.1,0,ST,0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n"),
            ("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,1,1\n"),
        ]),
        fx("PTH_003", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,NOPE,3,0\n")]),
        // PTH_014: from/to belong to different stations (resolved through parent_station).
        fx("PTH_014", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nS1,Stop1,41.0,29.0,0,\nS2,Stop2,41.1,29.1,0,\nST_A,StationA,41.0,29.0,1,\nST_B,StationB,41.2,29.2,1,\nP_A,PlatA,41.0,29.0,0,ST_A\nP_B,PlatB,41.2,29.2,0,ST_B\n"),
            ("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,P_A,P_B,3,0\n"),
        ]),
        // PTH_019: a generic node (location_type=3) has one pathway → dead end.
        fx("PTH_019", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Stop1,41.0,29.0,0\nS2,Stop2,41.1,29.1,0\nGN,Node,41.05,29.05,3\n"),
            ("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,GN,S1,3,0\n"),
        ]),
        // K6 analytics (k6_analytics::check_pathway_analytics).
        // PTH_012: the station has no entrance (location_type=2), while a platform is in the pathway graph.
        fx("PTH_012", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nS1,Stop1,41.0,29.0,0,\nS2,Stop2,41.1,29.1,0,\nST1,Station,41.0,29.0,1,\nPLAT,Platform,41.0,29.0,0,ST1\nGN,Node,41.01,29.01,3,ST1\n"),
            ("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,PLAT,GN,3,0\n"),
        ]),
        // PTH_013: an entrance→platform route exists but is not accessible (max_slope>8%).
        fx("PTH_013", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nS1,Stop1,41.0,29.0,0,\nS2,Stop2,41.1,29.1,0,\nST1,Station,41.0,29.0,1,\nENT,Entrance,41.0,29.0,2,ST1\nPLAT,Platform,41.0,29.0,0,ST1\n"),
            ("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,max_slope\nP1,ENT,PLAT,1,1,0.5\n"),
        ]),
        // PTH_015: speed derived from length/traversal_time exceeds 3 m/s (100m / 10s = 10 m/s).
        fx("PTH_015", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional,length,traversal_time\nP1,S1,S2,3,0,100,10\n")]),

        // ── JPN group (GTFS-JP; k4_cross_ref::check_gtfs_jp) ───────────────────
        // Gate: valid feed_lang=ja, agency_lang=ja + Asia/Tokyo, a ja-Hrkt translation,
        // or an office_jp/agency_jp file (is_gtfs_jp).
        // JPN_001/008/009/010 are behind the same GTFS-JP detection gate.
        // JPN_001: Japanese stop name has no kana (ja-Hrkt) reading.
        fx("JPN_001", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,東京駅,41.0,29.0\nS2,渋谷駅,41.1,29.1\n"),
        ]),
        // JPN_008: route_long_name is populated but has no kana reading.
        fx("JPN_008", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,route_long_name\nR1,1,101,3,渋谷線\n"),
        ]),
        // JPN_009: trip_headsign is populated but has no kana reading.
        fx("JPN_009", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
            ("trips.txt", "route_id,service_id,trip_id,trip_headsign\nR1,SVC1,T1,渋谷\n"),
        ]),
        // JPN_010: Japanese agency_name is populated but has no kana reading.
        fx("JPN_010", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,東京交通,http://test.example,UTC\n"),
        ]),
        // JPN_002: trips.jp_office_id is undefined in office_jp.txt.
        fx_v3("JPN_002", vec![
            ("office_jp.txt", "office_id,office_name\nO1,Office1\n"),
            ("trips.txt", "route_id,service_id,trip_id,jp_office_id\nR1,SVC1,T1,BADREF\n"),
        ]),
        // JPN_003: agency_jp.agency_id is undefined in agency.txt.
        fx_v3("JPN_003", vec![("agency_jp.txt", "agency_id\nNOPE\n")]),
        // JPN_004: GTFS-JP signal (office_jp) exists but translations.txt is absent.
        fx("JPN_004", vec![("office_jp.txt", "office_id,office_name\nO1,Office1\n")]),
        // JPN_005: office_jp.office_name is empty.
        fx_v3("JPN_005", vec![("office_jp.txt", "office_id,office_name\nO1,\n")]),
        // JPN_006: GTFS-JP (feed_lang=ja) but fare_attributes/fare_rules are absent.
        fx("JPN_006", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n")]),
        // JPN_007: GTFS-JP signal (office_jp) exists but feed_info.txt is absent.
        fx("JPN_007", vec![("office_jp.txt", "office_id,office_name\nO1,Office1\n")]),
        // JPN_011: agency_id is empty in GTFS-JP (required even with one operator).
        fx("JPN_011", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n,Test,http://test.example,UTC\n"),
        ]),
        // JPN_012: agency_id is missing from the agency_jp row.
        fx_v3("JPN_012", vec![
            ("agency_jp.txt", "agency_id,agency_zip_number\n,1234567\n"),
        ]),
        // JPN_013: agency_zip_number is not exactly seven ASCII digits.
        fx_v3("JPN_013", vec![
            ("agency_jp.txt", "agency_id,agency_zip_number\n1,123-4567\n"),
        ]),
        // JPN_014: empty office_id and duplicate office_id.
        fx_v3("JPN_014", vec![
            ("office_jp.txt", "office_id,office_name\n,OfficeEmpty\nO1,OfficeA\nO1,OfficeB\n"),
        ]),
        // JPN_015: legacy routes_jp.route_id is absent from routes.txt (V3/Auto compatibility).
        fx_v3("JPN_015", vec![
            ("routes_jp.txt", "route_id,route_update_date,origin_stop,via_stop,destination_stop\nMISSING,20260101,Origin,Via,Destination\n"),
        ]),
        // JPN_016: explicit V3 pattern_jp route_update_date is invalid.
        fx_v3("JPN_016", vec![
            ("pattern_jp.txt", "jp_pattern_id,route_update_date,origin_stop,via_stop,destination_stop\nP1,20260231,Origin,Via,Destination\n"),
            ("routes_jp.txt", "route_id,route_update_date,origin_stop,via_stop,destination_stop\nR1,令和8年4月6日,Origin,Via,Destination\n"),
        ]),
        // JPN_017: empty and duplicate jp_pattern_id values in pattern_jp.
        fx_v3("JPN_017", vec![
            ("pattern_jp.txt", "jp_pattern_id,route_update_date,origin_stop,via_stop,destination_stop\n,20260101,Origin,Via,Destination\nP1,20260101,Origin,Via,Destination\nP1,20260101,Origin,Via,Destination\n"),
        ]),
        // JPN_018: pattern_jp.txt exists but no trips.jp_pattern_id references it.
        fx_v3("JPN_018", vec![
            ("pattern_jp.txt", "jp_pattern_id,route_update_date,origin_stop,via_stop,destination_stop\nKNOWN_PATTERN,20260101,Origin,Via,Destination\n"),
            ("trips.txt", "route_id,service_id,trip_id,jp_pattern_id\nR1,SVC1,T1,MISSING_PATTERN\n"),
        ]),
        // JPN_019: ja-Hrkt translation points to a stop record that does not exist.
        fx("JPN_019", vec![
            ("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,ja-Hrkt,とうきょう,MISSING_STOP\n"),
        ]),
        // JPN_020: office_url and office_phone have suspicious formats.
        fx_v3("JPN_020", vec![
            ("office_jp.txt", "office_id,office_name,office_url,office_phone\nO1,Office1,not-a-url,x\n"),
        ]),
        // JPN_021: the same ja-Hrkt translation target has two different kana values.
        fx("JPN_021", vec![
            ("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,ja-Hrkt,とうきょう,S1\nstops,stop_name,ja-Hrkt,トウキョウ,S1\n"),
        ]),
        // JPN_022: GTFS-JP v4 is missing the agency_lang, location_type, and feed_info core fields.
        fx_v4("JPN_022", vec![
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,Test,http://test.example,UTC,\n"),
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop,35.0,139.0\n"),
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
        ]),
        // JPN_023-026: Japan-specific constants are checked only in the explicit profile.
        fx_v4("JPN_023", vec![(
            "feed_info.txt",
            "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,en\n",
        ), (
            "agency.txt",
            "agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,Test,http://test.example,Asia/Tokyo,ja\n",
        )]),
        fx_v4("JPN_024", vec![(
            "agency.txt",
            "agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,Test,http://test.example,Asia/Tokyo,en\n",
        ), (
            "feed_info.txt",
            "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n",
        )]),
        fx_v4("JPN_025", vec![(
            "agency.txt",
            "agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,Test,http://test.example,UTC,ja\n",
        ), (
            "feed_info.txt",
            "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n",
        )]),
        fx_v4("JPN_026", vec![(
            "fare_attributes.txt",
            "fare_id,price,currency_type,payment_method,transfers\nF1,100,USD,0,0\n",
        ), (
            "feed_info.txt",
            "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n",
        )]),
        // Explicit V3 applies the numeric route_type=3 constant directly; this fixture proves
        // the route_type=2 violation independently of the distribution.
        // JP detector gate is provided by feed_info.txt in the fixture.
        fx_v3("JPN_027", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,3\nR3,1,103,2\n"),
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
        ]),
        fx_v4("JPN_031", vec![
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfers\nF1,100,JPY,0,0\n"),
            ("fare_rules.txt", "fare_id,route_id,origin_id\nF1,R1,Z\n"),
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,zone_id\nS1,Stop1,41.0,29.0,Z\nS2,Stop2,41.1,29.1,\n"),
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
        ]),
        // JPN_032: strict V3 agency_id Corporate Number format.
        fx_v3("JPN_032", vec![
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_lang\nA-1,Test,http://test.example,UTC,ja\n"),
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
        ]),
        // JPN_033: reserved custom file suffix while the detection gate is open.
        fx_v3("JPN_033", vec![
            ("custom_jp.txt", "foo,bar\n1,2\n"),
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
        ]),
        // JPN_028/030: V3 requires both kana and ja records for Japanese route_desc.
        fx_v3("JPN_028", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,route_desc\nR1,1,101,3,渋谷線\n"),
            ("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,en,Stop,S1\n"),
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
        ]),
        fx_v3("JPN_030", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,route_desc\nR1,1,101,3,渋谷線\n"),
            ("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,en,Stop,S1\nroutes,route_desc,ja-Hrkt,シブヤ線,R1\n"),
        ]),
        // JPN_029: V4 is missing the recommended kana reading for Japanese stop_headsign.
        fx_v4("JPN_029", vec![
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\nT1,08:00:00,08:00:00,S1,1,渋谷\nT1,08:10:00,08:10:00,S2,2,\n"),
            ("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,en,Stop,S1\n"),
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n"),
        ]),

        // ── LOC group (locations.geojson; k1_parse::validate_locations_geojson) ─
        // LOC_001: feature geometry type is unsupported (Point). ⚠️ The fixture changed on
        // 2026-09-11: it used to be "not valid json", but that path now belongs to LOC_012 —
        // if the document cannot be parsed, geometry was not read and LOC_001 must not speak.
        fx("LOC_001", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"properties\":{},\"geometry\":{\"type\":\"Point\",\"coordinates\":[0,0]}}]}")]),
        // LOC_012: document is not JSON at all (corpus: tdg-81648..tdg-81652).
        fx("LOC_012", vec![("locations.geojson", "not valid json")]),
        // LOC_005: empty FeatureCollection.
        fx("LOC_005", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[]}")]),
        // LOC_007: duplicate feature 'id'.
        fx("LOC_007", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[0,0.01],[0.01,0.01],[0,0]]]}},{\"type\":\"Feature\",\"id\":\"L1\",\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[0,0.01],[0.01,0.01],[0,0]]]}}]}")]),
        // LOC_003: feature has no 'id'.
        // LOC_008: feature has no "type".
        fx("LOC_008", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"id\":\"L1\",\"properties\":{},\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[0,0.01],[0.01,0.01],[0,0]]]}}]}")]),
        // LOC_009: feature has no "properties".
        fx("LOC_009", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[0,0.01],[0.01,0.01],[0,0]]]}}]}")]),
        // LOC_010: geometry exists but has no "coordinates".
        fx("LOC_010", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"properties\":{},\"geometry\":{\"type\":\"Polygon\"}}]}")]),
        // Bowtie: the exterior ring self-intersects — OpenGIS 6.1.11 violation.
        fx("LOC_011", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"properties\":{},\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[2,2],[2,0],[0,2],[0,0]]]}}]}")]),
        fx("LOC_003", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[0,0.01],[0.01,0.01],[0,0]]]}}]}")]),
        // LOC_002: geometry null.
        fx("LOC_002", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"geometry\":null}]}")]),
        // LOC_004: Polygon ring is not closed (first point != last point).
        fx("LOC_004", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[0,0.01],[0.01,0.01],[0.01,0]]]}}]}")]),
        // LOC_006: bbox area > 500 km² (~5°×5°).
        fx("LOC_006", vec![("locations.geojson", "{\"type\":\"FeatureCollection\",\"features\":[{\"type\":\"Feature\",\"id\":\"L1\",\"geometry\":{\"type\":\"Polygon\",\"coordinates\":[[[0,0],[0,5],[5,5],[0,0]]]}}]}")]),

        // ── GEO group (geospatial analytics; k6_analytics::check_geo_analytics) ─────
        // Config defaults: max_shape_jump_km=10, stop_too_close_m=5, stop_far_from_shape_m=100.
        // GEO_006: shape segment jumps > 10 km (~22 km).
        fx("GEO_006", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.2,40.0,2\n")]),
        // GEO_007: critical shape jump > 30 km (3× threshold, ~55 km).
        fx("GEO_007", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.5,40.0,2\n")]),
        // GEO_009: stop is > 100 m from its trip's shape.
        fx("GEO_009", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.05,40.05\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.1,2\n"),
        ]),
        // GEO_002: stop is > 200 km from the feed median (>=3 stops with coordinates).
        fx("GEO_002", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Far,10.123,10.123\n")]),
        // GEO_012: 3+ stops clustered within 5 m.
        fx("GEO_012", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.00001,40.0\nS3,Stop3,40.0,40.00001\n")]),
        // GEO_013: feed containing stops with coordinates (Info) — base already triggers it.
        fx("GEO_013", vec![]),
        // GEO_014: feed bbox diagonal > 500 km (Info).
        fx("GEO_014", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,29.0\nS2,Stop2,45.0,35.0\n")]),
        // GEO_015: feed_lang=ja but stops are outside Japan (base lon 29).
        fx("GEO_015", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,ja\n")]),
        // GEO_016: stop near Null Island (|lat|,|lon| < 0.1).
        fx("GEO_016", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,0.01,0.01\nS2,Stop2,41.1,29.1\n")]),
        // GEO_017: shape point near Null Island.
        fx("GEO_017", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,0.01,0.01,1\nSH1,0.02,0.02,2\n")]),
        // GEO_018: all stops within a < 200 m area (test/placeholder data).
        fx("GEO_018", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.0005,40.0\nS3,Stop3,40.0,40.0005\n")]),
        // GEO_019: integer coordinates — base S1 (41.0,29.0) already triggers it.
        fx("GEO_019", vec![]),
        // GEO_020: all shape points have the same coordinates (degenerate geometry).
        fx("GEO_020", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.0,2\n")]),
        // GEO_021: >30% of stops share coordinates (total>=5).
        fx("GEO_021", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,A,40.1,40.1\nS2,B,40.1,40.1\nS3,C,40.1,40.1\nS4,D,40.1,40.1\nS5,E,41.1,29.1\n")]),
        // GEO_022: stop latitude is extremely close to a pole (|lat| > 89).
        fx("GEO_022", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,89.5,29.0\nS2,Stop2,41.1,29.1\n")]),

        // ── DQ group (data quality; k6_analytics::check_data_quality + k1_parse) ─
        // DQ_003: route_desc empty — base R1 already triggers it.
        fx("DQ_003", vec![]),
        // DQ_004: route_url empty — base R1 already triggers it.
        fx("DQ_004", vec![]),
        // DQ_005: no active service as of today (calendar period has expired).
        fx("DQ_005", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20200101,20201231\n")]),
        // DQ_005b: no trip has a stop_times record.
        fx("DQ_005b", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n")]),
        // DQ_005c: >50% of stops lack coordinates.
        fx("DQ_005c", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,,\nS2,Stop2,,\n")]),
        // DQ_006: shape_id is missing from >80% of trips — base (one trip without a shape)
        // triggers it.
        fx("DQ_006", vec![]),
        // DQ_009: trip exists but stop_times is completely empty.
        fx("DQ_009", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n")]),
        // DQ_010: agency unused by any route (agency_id=2 is idle).
        fx("DQ_010", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n2,B,https://b.example,UTC\n")]),
        // DQ_011: feed contains only 1 stop.
        fx("DQ_011", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n")]),
        // DQ_012: >5 agencies and no route has an agency_id.
        fx("DQ_012", vec![
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n2,B,https://b.example,UTC\n3,C,https://c.example,UTC\n4,D,https://d.example,UTC\n5,E,https://e.example,UTC\n6,F,https://f.example,UTC\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,,101,3\n"),
        ]),
        // DQ_013: <3 trips in the feed — base (one trip) triggers it.
        fx("DQ_013", vec![]),
        // DQ_016: leading/trailing whitespace in a value (k1_parse).
        fx("DQ_016", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1, Stop1 ,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // DQ_017: suspicious coordinates (|lat|,|lon| < 1°).
        fx("DQ_017", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,0.5,0.5\nS2,Stop2,41.1,29.1\n")]),
        // DQ_018: recommended text field is all uppercase (stop_name).
        fx("DQ_018", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,MERKEZ,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // DQ_019: recommended text field is all lowercase (stop_name).
        fx("DQ_019", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,merkez,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // DQ_020: recommended trip_headsign is empty — base (trip without a headsign)
        // triggers it.
        fx("DQ_020", vec![]),
        // DQ_021: duplicate primary key (stop_id duplicate).
        // DQ_021 proves two unique primary keys: stops.stop_id and
        // attributions.attribution_id (spec type `Unique ID`). The second needs no
        // separate fixture — one rule, one fact, two files.
        fx("DQ_021", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,A,40.0,40.0\nS1,B,41.1,29.1\n"),
            ("attributions.txt", "attribution_id,organization_name,is_producer\nA1,Org,1\nA1,Org2,1\n"),
        ]),
        // DQ_022: >80% of stops have the same stop_name (total>=5).
        fx("DQ_022", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop,40.0,40.0\nS2,Stop,40.1,40.1\nS3,Stop,40.2,40.2\nS4,Stop,40.3,40.3\nS5,Stop,40.4,40.4\n")]),

        // ── OPR group (operational analytics; k6_analytics) ─────────────────────
        // Config: max_headway_warning_min=240, bunching_threshold_min=2, max_speed_bus=120,
        // service_gap_days=7, max_trips_per_route=500, headway_outlier_sigma=2.5.
        // NOTE: OPR_021/022/023 only work with config.calendar_override_rules (impossible
        // by default); OPR_024 requires >500 trips (cannot be written inline) → left in debt.
        // OPR_001: maximum headway across a route > 240 min and irregular.
        fx("OPR_001", vec![
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:05:00,08:05:00,S2,2\nT2,08:10:00,08:10:00,S1,1\nT2,08:15:00,08:15:00,S2,2\nT3,18:00:00,18:00:00,S1,1\nT3,18:05:00,18:05:00,S2,2\n"),
        ]),
        // OPR_003: consecutive trips at the same departure stop are less than 2 min apart
        // (bunching).
        fx("OPR_003", vec![
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:01:00,08:01:00,S1,1\nT2,08:11:00,08:11:00,S2,2\n"),
        ]),
        // OPR_004: route has no weekend service — base (SVC1 Monday-Friday) triggers it.
        fx("OPR_004", vec![]),
        // OPR_005: route_type-relative headway outlier (6 groups; R6 is 6× the median).
        fx("OPR_005", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,1,3\nR2,1,2,3\nR3,1,3,3\nR4,1,4,3\nR5,1,5,3\nR6,1,6,3\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR2,SVC1,T3\nR2,SVC1,T4\nR3,SVC1,T5\nR3,SVC1,T6\nR4,SVC1,T7\nR4,SVC1,T8\nR5,SVC1,T9\nR5,SVC1,T10\nR6,SVC1,T11\nR6,SVC1,T12\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT2,08:10:00,08:10:00,S1,1\nT3,08:00:00,08:00:00,S1,1\nT4,08:10:00,08:10:00,S1,1\nT5,08:00:00,08:00:00,S1,1\nT6,08:10:00,08:10:00,S1,1\nT7,08:00:00,08:00:00,S1,1\nT8,08:10:00,08:10:00,S1,1\nT9,08:00:00,08:00:00,S1,1\nT10,08:10:00,08:10:00,S1,1\nT11,08:00:00,08:00:00,S1,1\nT12,09:00:00,09:00:00,S1,1\n"),
        ]),
        // OPR_006: <2 stops in a trip.
        fx("OPR_006", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n")]),
        // OPR_007: non-consecutive repeated stop in a trip (not a ring).
        fx("OPR_007", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT1,08:20:00,08:20:00,S3,3\nT1,08:30:00,08:30:00,S2,4\n")]),
        // OPR_008: >1 invalid speed segment in a trip (~660 km/h, bus threshold 120).
        fx("OPR_008", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.05,40.0\nS3,Stop3,40.10,40.0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:00:30,08:00:30,S2,2\nT1,08:01:00,08:01:00,S3,3\n"),
        ]),
        // OPR_009: nighttime trip (departure >= 23:00).
        fx("OPR_009", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:30:00,23:30:00,S1,1\nT1,23:40:00,23:40:00,S2,2\n")]),
        // OPR_010: inconsistent wheelchair_accessible values on a route (1 and 2).
        fx("OPR_010", vec![("trips.txt", "route_id,service_id,trip_id,wheelchair_accessible\nR1,SVC1,T1,1\nR1,SVC1,T2,2\n")]),
        // OPR_011: trip's service_id has no active days (not in the calendar).
        fx("OPR_011", vec![("trips.txt", "route_id,service_id,trip_id\nR1,SVCX,T1\n")]),
        // OPR_012: >=7-day service gap (two distant dates in calendar_dates).
        fx_rm("OPR_012",
            vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260518,1\nSVC1,20260601,1\n")],
            vec!["calendar.txt"]),
        // OPR_013: all trips on the route run in one direction (one direction_id value).
        fx("OPR_013", vec![("trips.txt", "route_id,service_id,trip_id,direction_id\nR1,SVC1,T1,0\n")]),
        // OPR_014: average transfer time across the feed > 10 min (type=2).
        fx("OPR_014", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,min_transfer_time\nS1,S2,2,1200\n")]),
        // OPR_015: bidirectional route uses one shape (bus).
        fx("OPR_015", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id,direction_id\nR1,SVC1,T1,SH1,0\nR1,SVC1,T2,SH1,1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\nSH1,41.1,29.1,2\n"),
        ]),
        // OPR_016: no service in the feed has active days (all flags are 0).
        fx("OPR_016", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,0,0,0,0,0,0,0,20260518,20260524\n")]),
        // OPR_017: trip covers a very short distance (< 100 m, no shape → stop coordinates).
        fx("OPR_017", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.0005,40.0\n")]),
        // OPR_019: >1 active service on a route on the same (non-exception) day.
        // T1 and T2 are the SAME operational trip (same direction, departure, and stop
        // sequence), but are active from two separate services → a real collision. It would
        // be legitimate if they were different trips.
        fx("OPR_019", vec![
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260518,20260524\nSVC2,1,1,1,1,1,1,1,20260518,20260524\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC2,T2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:00:00,08:00:00,S1,1\nT2,08:10:00,08:10:00,S2,2\n"),
        ]),
        // OPR_020: >1 active service on a route on an exception day (override collision).
        fx("OPR_020", vec![
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260518,20260518\n"),
            ("calendar_dates.txt", "service_id,date,exception_type\nSVC2,20260518,1\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC2,T2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:00:00,08:00:00,S1,1\nT2,08:10:00,08:10:00,S2,2\n"),
        ]),
        // OPR_025: average trip duration in the feed < 60 s (5 trips, 10 s each).
        fx("OPR_025", vec![
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\nR1,SVC1,T4\nR1,SVC1,T5\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:00:10,08:00:10,S2,2\nT2,08:00:00,08:00:00,S1,1\nT2,08:00:10,08:00:10,S2,2\nT3,08:00:00,08:00:00,S1,1\nT3,08:00:10,08:00:10,S2,2\nT4,08:00:00,08:00:00,S1,1\nT4,08:00:10,08:00:10,S2,2\nT5,08:00:00,08:00:00,S1,1\nT5,08:00:10,08:00:10,S2,2\n"),
        ]),

        // ── XFL group (cross-file FK; k4_cross_ref::check_xfl + cemv/fares v2) ──
        // XFL_002: trip has no stop_times record (T2).
        fx("XFL_002", vec![("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n")]),
        // XFL_006: calendar_dates has only exception_type=2 and is absent from calendar.
        fx("XFL_006", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVCZ,20260601,2\n")]),
        // XFL_012: none of the route's trips has stop_times (empty stop_times).
        fx("XFL_012", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n")]),
        // XFL_013: shape is used in both outbound (0) and return (1) directions.
        fx("XFL_013", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id,direction_id\nR1,SVC1,T1,SH1,0\nR1,SVC1,T2,SH1,1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\nSH1,41.1,29.1,2\n"),
        ]),
        // XFL_014: translation record_id points to a nonexistent record.
        fx("XFL_014", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,en,X,NOPE\n")]),
        // XFL_015: attribution has an invalid reference (agency_id).
        fx("XFL_015", vec![("attributions.txt", "attribution_id,organization_name,is_producer,agency_id\nA1,Org,1,NOPE\n")]),
        // XFL_016: translation table_name=feed_info but feed_info.txt is absent.
        fx("XFL_016", vec![("translations.txt", "table_name,field_name,language,translation\nfeed_info,feed_publisher_name,en,X\n")]),
        // XFL_019: network_id appears in both routes.txt and route_networks.txt (collision).
        fx("XFL_019", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,network_id\nR1,1,101,3,N1\n"),
            ("route_networks.txt", "network_id,route_id\nN1,R1\n"),
        ]),
        // XFL_020: transfer (trip_id, route_id) pair is invalid.
        fx("XFL_020", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,from_route_id\nS1,S2,1,T1,RWRONG\n")]),
        // XFL_021: from_stop_id is absent from from_trip_id's stop_times.
        fx("XFL_021", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Stop3,41.2,29.2\n"),
            ("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id\nS3,S2,1,T1\n"),
        ]),
        // XFL_022: location_group_stops group is absent from location_groups.txt.
        fx("XFL_022", vec![("location_group_stops.txt", "location_group_id,stop_id\nLGNOPE,S1\n")]),
        // XFL_023: location_group_stops stop_id is absent from stops.txt.
        fx("XFL_023", vec![
            ("location_groups.txt", "location_group_id,location_group_name\nLG1,Group1\n"),
            ("location_group_stops.txt", "location_group_id,stop_id\nLG1,NOPE\n"),
        ]),
        // XFL_024: stop_times.location_group_id is absent from location_groups.txt (Flex).
        fx("XFL_024", vec![("stop_times.txt", "trip_id,stop_sequence,location_group_id,start_pickup_drop_off_window,end_pickup_drop_off_window\nT1,1,NOPE,09:00:00,10:00:00\n")]),
        // XFL_025: stop_times.location_id is absent from locations.geojson (Flex).
        fx("XFL_025", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window\nT1,1,NOPE,09:00:00,10:00:00\n")]),
        // XFL_031: location_group_id and stop_id collide in the same namespace.
        fx("XFL_031", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nLG1,Cakisan,41.2,29.2\n"),
            ("location_groups.txt", "location_group_id,location_group_name\nLG1,Bolge\n"),
        ]),
        // XFL_027: route cemv=2 but a usable contactless fare product exists (contradiction).
        fx("XFL_027", vec![
            ("fare_media.txt", "fare_media_id,fare_media_type\nM3,3\n"),
            ("fare_products.txt", "fare_product_id,fare_media_id,amount,currency\nP1,M3,2.5,USD\n"),
            ("fare_leg_rules.txt", "leg_group_id,fare_product_id\nLG1,P1\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,cemv_support\nR1,1,101,3,2\n"),
        ]),
        // XFL_028: agency_cemv_support=1 + Fares v2 exists but no type-3 media.
        fx("XFL_028", vec![
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,cemv_support\n1,Test,http://test.example,UTC,1\n"),
            ("fare_products.txt", "fare_product_id,amount,currency\nP1,2.5,USD\n"),
        ]),
        // XFL_029: route_cemv_support=1 + Fares v2 exists but no type-3 media.
        fx("XFL_029", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,cemv_support\nR1,1,101,3,1\n"),
            ("fare_products.txt", "fare_product_id,amount,currency\nP1,2.5,USD\n"),
        ]),
        // XFL_030: type-3 media exists but no agency/route has cemv=1.
        fx("XFL_030", vec![("fare_media.txt", "fare_media_id,fare_media_type\nM3,3\n")]),

        // ── NET / PDW ──────────────────────────────────────────────────────────
        // NET_001: duplicate network_id in networks.txt (k3).
        fx("NET_001", vec![("networks.txt", "network_id,network_name\nN1,Net1\nN1,Net2\n")]),
        // NET_002: route_networks.network_id is absent from networks.txt (k4).
        fx("NET_002", vec![
            ("networks.txt", "network_id,network_name\nN1,Net1\n"),
            ("route_networks.txt", "network_id,route_id\nNOPE,R1\n"),
        ]),
        // NET_003: route_networks.route_id is absent from routes.txt (k4).
        fx("NET_003", vec![
            ("networks.txt", "network_id,network_name\nN1,Net1\n"),
            ("route_networks.txt", "network_id,route_id\nN1,NOPE\n"),
        ]),
        // PDW_006: overlapping pickup/drop-off windows in the same trip+zone (k6 Flex).
        fx("PDW_006", vec![("stop_times.txt", "trip_id,stop_sequence,location_id,start_pickup_drop_off_window,end_pickup_drop_off_window\nT1,1,Z1,09:00:00,10:00:00\nT1,2,Z1,09:30:00,11:00:00\n")]),

        // ── CAL group (calendar analytics k6 + cross-ref k4 + k2) ────────────────
        // TODAY=20260515. CAL_006: whitespace-padded all-zero row in mdb-2830 format (k2);
        // the weekly pattern finding remains even with a calendar_dates override.
        fx("CAL_006", vec![
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1, 0, 0, 0, 0, 0, 0, 0, 20250101, 20271231\n"),
            ("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260520,1\n"),
        ]),
        // CAL_007: >=7-day service gap (calendar_dates, no calendar). The gap is IN THE PAST
        // (before today=20260515) → produces CAL_007, not near-future CAL_012
        // (which takes its place in the near future, #29).
        fx_rm("CAL_007", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260401,1\nSVC1,20260420,1\n")], vec!["calendar.txt"]),
        // CAL_008: end_date expires within 30 days (today+10).
        fx("CAL_008", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260101,20260525\n")]),
        // CAL_009: all services have expired (k4 critical).
        fx("CAL_009", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20200101,20201231\n")]),
        // CAL_010: total active days <= 7 (one-week window).
        // CAL_010: short service. SVC2 opens the publication window to ≥30 days (absolute
        // threshold path); with a short window the rule would use the RATIO path and SVC1,
        // which covers 100%, would be silent.
        fx("CAL_010", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260518,20260524\nSVC2,1,1,1,1,1,1,1,20260101,20261231\n")]),
        // CAL_011: unused by any trip (SVC2 is idle).
        fx("CAL_011", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\nSVC2,1,1,1,1,1,0,0,20250101,20271231\n")]),
        // CAL_012: service gap in the near future.
        fx_rm("CAL_012", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260516,1\nSVC1,20260603,1\n")], vec!["calendar.txt"]),
        // CAL_013: individual service has expired (k6).
        fx("CAL_013", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20190101,20201231\n")]),
        // CAL_014: service active dates are outside the feed_info window. NO calendar_dates:
        // until 2026-10-11, the rule only considered services with exceptions.
        fx("CAL_014", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20260601,20260630\n"),
        ]),
        // CAL_015: earliest active date is in the future (feed-level).
        fx("CAL_015", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20270101,20270107\n")]),
        // CAL_016: latest active date > today+2 years.
        fx("CAL_016", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20260101,20300101\n")]),
        // CAL_017: all dates for an individual service are in the future.
        fx("CAL_017", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20270101,20270107\n")]),
        // CAL_018: all weekdays are inactive, with no exception_type=1 (k4).
        fx("CAL_018", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,0,0,0,0,0,0,0,20250101,20271231\n")]),
        // CAL_020: feed validity window > 5 years.
        fx("CAL_020", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20200101,20271231\n")]),
        // CAL_021: covers today but has no active day in the next 7 days.
        fx_rm("CAL_021", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260101,1\nSVC1,20261231,1\n")], vec!["calendar.txt"]),
        // CAL_023: end_date >= today_year+3 (including the boundary year; e.g. 2026+3=2029
        // is exactly on the threshold).
        fx("CAL_023", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20260101,20290101\n")]),
        // CAL_024: service is inactive in the next 7 days (per service).
        fx("CAL_024", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260401,20260630\n"), ("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260515,2\nSVC1,20260516,2\nSVC1,20260517,2\nSVC1,20260518,2\nSVC1,20260519,2\nSVC1,20260520,2\nSVC1,20260521,2\n")]),

        // ── FIN group (feed_info.txt k2 + k6) ──────────────────────────────────
        // FIN_007: feed_version missing.
        fx("FIN_007", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,en\n")]),
        // FIN_008: feed_contact_email invalid.
        fx("FIN_008", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_contact_email\nPub,https://x.example,en,notanemail\n")]),
        // FIN_009: feed_contact_url invalid.
        fx("FIN_009", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_contact_url\nPub,https://x.example,en,notaurl\n")]),
        // FIN_010: feed_end_date in the past (k6).
        fx("FIN_010", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20190101,20200101\n")]),
        // FIN_012: feed_start_date > feed_end_date (k2).
        fx("FIN_012", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20271231,20250101\n")]),
        // FIN_013: triggers when fare_attribute agency_id is missing even with one agency (k4).
        fx("FIN_013", vec![
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n"),
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
        ]),
        // FIN_014: feed_start_date/feed_end_date missing.
        fx("FIN_014", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_version\nPub,https://x.example,en,1.0\n")]),
        // FIN_015: multiple feed_info records.
        fx("FIN_015", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,en\nPub2,https://y.example,en\n")]),
        // FIN_016: feed_start_date in the future (k6).
        fx("FIN_016", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20270101,20271231\n")]),
        // FIN_017: feed_end_date > today+2 years (k6).
        fx("FIN_017", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20260101,20300101\n")]),
        // FIN_018: both feed_contact_email and feed_contact_url are missing (k6).
        fx("FIN_018", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20260101,20261231\n")]),
        // FIN_019: feed expires within 7 days (today+5).
        fx("FIN_019", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20260101,20260520\n")]),
        // FIN_020: feed validity window < 7 days.
        fx("FIN_020", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang,feed_start_date,feed_end_date\nPub,https://x.example,en,20260101,20260103\n")]),

        // ── CLD group (calendar_dates) ─────────────────────────────────────────
        // CLD_004: no calendar and no exception_type=1 record for the trip's service (k4).
        fx_rm("CLD_004", vec![("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260601,2\n")], vec!["calendar.txt"]),
        // CLD_006: >60 exception days for one service_id (k2).
        fx("CLD_006", vec![("calendar_dates.txt", concat!(
            "service_id,date,exception_type\n",
            "SVC1,20260101,1\nSVC1,20260102,1\nSVC1,20260103,1\nSVC1,20260104,1\nSVC1,20260105,1\nSVC1,20260106,1\nSVC1,20260107,1\nSVC1,20260108,1\nSVC1,20260109,1\nSVC1,20260110,1\n",
            "SVC1,20260111,1\nSVC1,20260112,1\nSVC1,20260113,1\nSVC1,20260114,1\nSVC1,20260115,1\nSVC1,20260116,1\nSVC1,20260117,1\nSVC1,20260118,1\nSVC1,20260119,1\nSVC1,20260120,1\n",
            "SVC1,20260121,1\nSVC1,20260122,1\nSVC1,20260123,1\nSVC1,20260124,1\nSVC1,20260125,1\nSVC1,20260126,1\nSVC1,20260127,1\nSVC1,20260128,1\nSVC1,20260129,1\nSVC1,20260130,1\n",
            "SVC1,20260131,1\nSVC1,20260201,1\nSVC1,20260202,1\nSVC1,20260203,1\nSVC1,20260204,1\nSVC1,20260205,1\nSVC1,20260206,1\nSVC1,20260207,1\nSVC1,20260208,1\nSVC1,20260209,1\n",
            "SVC1,20260210,1\nSVC1,20260211,1\nSVC1,20260212,1\nSVC1,20260213,1\nSVC1,20260214,1\nSVC1,20260215,1\nSVC1,20260216,1\nSVC1,20260217,1\nSVC1,20260218,1\nSVC1,20260219,1\n",
            "SVC1,20260220,1\nSVC1,20260221,1\nSVC1,20260222,1\nSVC1,20260223,1\nSVC1,20260224,1\nSVC1,20260225,1\nSVC1,20260226,1\nSVC1,20260227,1\nSVC1,20260228,1\nSVC1,20260301,1\nSVC1,20260302,1\n"
        ))]),
        // CLD_007: more than half of active days are overridden (short service + 4 removals).
        fx("CLD_007", vec![
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20260518,20260524\n"),
            ("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260518,2\nSVC1,20260519,2\nSVC1,20260520,2\nSVC1,20260521,2\n"),
        ]),

        // ── FAR group (fares v1) ───────────────────────────────────────────────
        // FAR_001: duplicate fare_id (k3).
        fx("FAR_001", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\nF1,2.5,USD,0\n")]),
        // FAR_008: fare_attribute agency_id is absent from agency.txt (k4).
        fx("FAR_008", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method,agency_id\nF1,2.5,USD,0,NOPE\n")]),
        // FAR_010: multiple fare_id values for the same (route,origin,destination,contains) (k4).
        fx("FAR_010", vec![("fare_rules.txt", "fare_id,route_id\nF1,R1\nF2,R1\n")]),

        // ── AGN / ATR (remaining) ──────────────────────────────────────────────
        // AGN_010: duplicate agency_id (k3).
        fx("AGN_010", vec![("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\n1,A,https://a.example,UTC\n1,B,https://b.example,UTC\n")]),
        // AGN_013: feed_lang does not match agency_lang (k4 check_agencies).
        fx("AGN_013", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,en\n"),
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone,agency_lang\n1,Test,http://test.example,UTC,fr\n"),
        ]),
        // ATR_009: more than one of agency_id/route_id/trip_id is populated (k4).
        fx("ATR_009", vec![("attributions.txt", "attribution_id,organization_name,is_producer,agency_id,route_id\nA1,Org,1,1,R1\n")]),
        // ATR_010: attribution agency_id is absent from agency.txt (k4).
        fx("ATR_010", vec![("attributions.txt", "attribution_id,organization_name,is_producer,agency_id\nA1,Org,1,NOPE\n")]),
        // ATR_011: attribution route_id is absent from routes.txt (k4).
        fx("ATR_011", vec![("attributions.txt", "attribution_id,organization_name,is_producer,route_id\nA1,Org,1,NOPE\n")]),
        // ATR_012: attribution trip_id is absent from trips.txt (k4).
        fx("ATR_012", vec![("attributions.txt", "attribution_id,organization_name,is_producer,trip_id\nA1,Org,1,NOPE\n")]),

        // ── LVL group (levels.txt) ─────────────────────────────────────────────
        // LVL_001: duplicate level_id (k3).
        fx("LVL_001", vec![("levels.txt", "level_id,level_index\nL1,0\nL1,1\n")]),
        // LVL_002: level_index is not numeric (k2).
        fx("LVL_002", vec![("levels.txt", "level_id,level_index\nL1,abc\n")]),
        // LVL_003: level_name missing (k2).
        fx("LVL_003", vec![("levels.txt", "level_id,level_index\nL1,0\n")]),
        // LVL_004: level is unused by any stop (k4).
        fx("LVL_004", vec![("levels.txt", "level_id,level_index,level_name\nL1,0,Ground\n")]),
        // LVL_005: level_name > 255 karakter (k2).
        fx("LVL_005", vec![("levels.txt", concat!("level_id,level_index,level_name\nL1,0,",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n"))]),
        // LVL_006: endpoint stop of an elevator (pathway_mode=5) has no level_id (k4).
        fx("LVL_006", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,5,0\n")]),

        // ── FLG group (fare_leg_rules cross-ref k4) ────────────────────────────
        fx("FLG_001", vec![("fare_leg_rules.txt", "leg_group_id,fare_product_id\nLG1,NOPE\n")]),
        // FLG_008: fare_product_id EMPTY — FLG_001's FK check is guarded by !is_empty() (#153).
        fx("FLG_008", vec![("fare_leg_rules.txt", "leg_group_id,fare_product_id\nLG1,\n")]),
        fx("FLG_002", vec![("fare_leg_rules.txt", "leg_group_id,network_id\nLG1,NOPE\n")]),
        fx("FLG_003", vec![("fare_leg_rules.txt", "leg_group_id,from_area_id\nLG1,NOPE\n")]),
        fx("FLG_004", vec![("fare_leg_rules.txt", "leg_group_id,to_area_id\nLG1,NOPE\n")]),
        fx("FLG_005", vec![("fare_leg_rules.txt", "leg_group_id,from_timeframe_group_id\nLG1,NOPE\n")]),
        fx("FLG_006", vec![("fare_leg_rules.txt", "leg_group_id,to_timeframe_group_id\nLG1,NOPE\n")]),

        // ── FPD group (fare_products) ──────────────────────────────────────────
        // FPD_001: duplicate composite PK (k3), same fare_product_id + rider_category_id +
        // fare_media_id.
        fx("FPD_001", vec![
            ("fare_media.txt", "fare_media_id,fare_media_name,fare_media_type\ncash,Cash,0\n"),
            ("fare_products.txt", "fare_product_id,amount,currency,fare_media_id\nP1,2.5,USD,cash\nP1,2.5,USD,cash\n"),
        ]),
        // FPD_003: invalid currency (k2).
        fx("FPD_003", vec![("fare_products.txt", "fare_product_id,amount,currency\nP1,2.5,usd\n")]),
        // FPD_004: fare_media_id is absent from fare_media.txt (k4).
        fx("FPD_004", vec![("fare_products.txt", "fare_product_id,amount,currency,fare_media_id\nP1,2.5,USD,MNOPE\n")]),
        // FPD_005: rider_category_id is absent from rider_categories.txt (k4).
        fx("FPD_005", vec![("fare_products.txt", "fare_product_id,amount,currency,rider_category_id\nP1,2.5,USD,RCNOPE\n")]),
        fx("FPD_007", vec![("fare_products.txt","fare_product_id,fare_product_name,amount,currency\nP1,Bilet,0.9,EUR\n")]),

        // ── FTR group (fare_transfer_rules) ────────────────────────────────────
        fx("FTR_002", vec![("fare_transfer_rules.txt", "from_leg_group_id,fare_transfer_type\nLGNOPE,0\n")]),
        fx("FTR_003", vec![("fare_transfer_rules.txt", "to_leg_group_id,fare_transfer_type\nLGNOPE,0\n")]),
        fx("FTR_004", vec![("fare_transfer_rules.txt", "fare_product_id,fare_transfer_type\nPNOPE,0\n")]),
        fx("FTR_009", vec![("fare_transfer_rules.txt", "from_leg_group_id,to_leg_group_id,fare_transfer_type\nLG1,LG1,0\n")]),
        fx("FTR_010", vec![("fare_transfer_rules.txt", "from_leg_group_id,to_leg_group_id,fare_transfer_type,transfer_count\nLG1,LG2,0,3\n")]),
        fx("FTR_011", vec![("fare_transfer_rules.txt", "from_leg_group_id,fare_transfer_type,duration_limit\nLG1,0,3600\n")]),

        // ── RCT / SAR / TFR / FMD / GGL ────────────────────────────────────────
        // RCT_001: duplicate rider_category_id (k3).
        fx("RCT_001", vec![("rider_categories.txt", "rider_category_id,rider_category_name\nRC1,A\nRC1,B\n")]),
        // RCT_006: multiple default rider_categories per fare_product (k4).
        fx("RCT_006", vec![
            ("rider_categories.txt", "rider_category_id,rider_category_name,is_default_fare_category\nRC1,Adult,1\nRC2,Senior,1\n"),
            ("fare_products.txt", "fare_product_id,amount,currency,rider_category_id\nP1,2.5,USD,RC1\nP1,3.5,USD,RC2\n"),
        ]),
        // SAR_001: stop_areas area_id is absent from areas.txt (k4).
        fx("SAR_001", vec![("stop_areas.txt", "area_id,stop_id\nANOPE,S1\n")]),
        // SAR_002: stop_areas stop_id is absent from stops.txt (k4).
        fx("SAR_002", vec![
            ("areas.txt", "area_id,area_name\nA1,Area1\n"),
            ("stop_areas.txt", "area_id,stop_id\nA1,NOPE\n"),
        ]),
        // TFR_002: timeframes service_id is absent from calendar (k4).
        fx("TFR_002", vec![("timeframes.txt", "timeframe_group_id,start_time,end_time,service_id\nTG1,08:00:00,10:00:00,SVCX\n")]),
        // FMD_001: duplicate fare_media_id (k3).
        fx("FMD_001", vec![("fare_media.txt", "fare_media_id,fare_media_type\nM1,2\nM1,2\n")]),
        // GGL_001: Google does not support transfer_type=4/5 (in-seat) (k2).
        fx("GGL_001", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type\nS1,S2,4\n")]),
        // GGL_002: invalid Google ic_price extension (-5) in fare_attributes.txt (k2).
        fx("GGL_002", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method,ic_price\nF1,2.5,JPY,0,-5\n")]),

        // ── FRQ group (remaining) ──────────────────────────────────────────────
        // FRQ_006: headway > 240dk (k6).
        fx("FRQ_006", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,08:00:00,10:00:00,20000\n")]),
        // FRQ_010: headway <= bunching threshold (k6).
        fx("FRQ_010", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,08:00:00,10:00:00,60\n")]),
        // FRQ_011: frequencies periods for the same trip overlap (k2).
        fx("FRQ_011", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT1,08:00:00,10:00:00,600\nT1,09:00:00,11:00:00,600\n")]),
        fx("FRQ_012", vec![("frequencies.txt", "trip_id,start_time,end_time,headway_secs,exact_times\nT1,05:00:00,06:00:00,3600,1\n"), ("trips.txt","route_id,service_id,trip_id\nR1,SVC,T1\n")]),

        // ── RTS group (remaining: k2 field + k3 dup + k4 FK + k6) ──────────────
        // RTS_001: duplicate route_id (k3).
        fx("RTS_001", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR1,1,102,3\n")]),
        // RTS_002: agency_id is absent from agency.txt (k4 per-route).
        fx("RTS_002", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,NOPE,101,3\n")]),
        // RTS_007: invalid route_text_color hex (k2).
        fx("RTS_007", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,route_text_color\nR1,1,101,3,ZZZ\n")]),
        // RTS_008: low route_text_color/route_color contrast (k2).
        fx("RTS_008", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,route_color,route_text_color\nR1,1,101,3,FFFFFF,FEFEFE\n")]),
        // RTS_010: route_short_name > 12 karakter (k2).
        fx("RTS_010", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,ABCDEFGHIJKLM,3\n")]),
        // RTS_011: route_long_name > 100 karakter (k2).
        fx("RTS_011", vec![("routes.txt", concat!("route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,101,",
            "Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna",
            ",3\n"))]),
        // RTS_012: route unused by any trip (k4 orphan).
        fx("RTS_012", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,3\n")]),
        // RTS_016: no trip on the route has an active calendar day (k6).
        fx("RTS_016", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,0,0,0,0,0,0,0,20250101,20271231\n")]),
        // RTS_019: yinelenen route_short_name (k2).
        fx("RTS_019", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,101,3\n")]),
        // RTS_020: route_url matches the agency URL (k6).
        fx("RTS_020", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,route_url\nR1,1,101,3,http://test.example\n")]),
        // RTS_021: route_short_name > 6 characters (Google threshold) (k2).
        fx("RTS_021", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,ABCDEFG,3\n")]),
        // RTS_022: route_long_name contains route_short_name (k6).
        fx("RTS_022", vec![("routes.txt", "route_id,agency_id,route_short_name,route_long_name,route_type\nR1,1,5A,5A Line,3\n")]),
        // RTS_023: route_long_name == route_desc (k2).
        fx("RTS_023", vec![("routes.txt", "route_id,agency_id,route_short_name,route_long_name,route_desc,route_type\nR1,1,101,Main Line,Main Line,3\n")]),
        // RTS_024: cemv_support > 2 (k2).
        fx("RTS_024", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,cemv_support\nR1,1,101,3,5\n")]),
        // RTS_025: route agency_id empty with one agency (k6 best practice).
        fx("RTS_025", vec![
            ("agency.txt", "agency_id,agency_name,agency_url,agency_timezone\nA1,Test Agency,https://test.example,Europe/Istanbul\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,,101,3\n"),
        ]),
        // RTS_031: route_id EMPTY — RTS_001 measures duplicates; a missing ID cannot duplicate
        // (#153).
        fx("RTS_031", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type\n,1,101,3\n")]),

        // ── TRP group (remaining: k2 + k4 FK + k6) ─────────────────────────────
        // TRP_002: route_id is absent from routes.txt (k4).
        fx("TRP_002", vec![("trips.txt", "route_id,service_id,trip_id\nRNOPE,SVC1,T1\n")]),
        // TRP_003: service_id is absent from calendar (k4).
        fx("TRP_003", vec![("trips.txt", "route_id,service_id,trip_id\nR1,SVCX,T1\n")]),
        // TRP_004: shape_id is absent from shapes.txt (k4).
        fx("TRP_004", vec![("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SHNOPE\n")]),
        // TRP_011: no trip_headsign + trip_short_name AND route is also unnamed
        // (route-compensation guard #29).
        // (Base route_short_name=101 → guard filters it; an unnamed route is required.)
        fx("TRP_011", vec![("routes.txt", "route_id,agency_id,route_type\nR1,1,3\n")]),
        // TRP_012: direction_id missing from some trips on a bidirectional route (k6).
        fx("TRP_012", vec![("trips.txt", "route_id,service_id,trip_id,direction_id\nR1,SVC1,T1,0\nR1,SVC1,T2,1\nR1,SVC1,T3,\n")]),
        // TRP_013: one trip per route — base triggers it (k6).
        fx("TRP_013", vec![]),
        // TRP_014: trip_short_name > 20 karakter (k2).
        fx("TRP_014", vec![("trips.txt", "route_id,service_id,trip_id,trip_short_name\nR1,SVC1,T1,ABCDEFGHIJKLMNOPQRSTU\n")]),
        // TRP_015: block_id groups no trips (k6, one finding per FEED).
        // One trip + one block → no grouping. (A single-trip block is VALID under the spec;
        // the finding is for the field not grouping anything.)
        fx("TRP_015", vec![("trips.txt", "route_id,service_id,trip_id,block_id\nR1,SVC1,T1,B1\n")]),
        // TRP_017: frequency-based trip is absent from stop_times (k4).
        fx("TRP_017", vec![
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n"),
            ("frequencies.txt", "trip_id,start_time,end_time,headway_secs\nT2,08:00:00,10:00:00,600\n"),
        ]),
        // TRP_019: continuous pickup/drop-off active but shape_id is absent (k4).
        fx("TRP_019", vec![("routes.txt", "route_id,agency_id,route_short_name,route_type,continuous_pickup\nR1,1,101,3,0\n")]),
        fx("STM_053", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:00:00,08:00:00,S2,2\nT1,08:00:00,08:00:00,S1,3\n")]),
        // TRP_020: trip_headsign matches an intermediate, not terminal, stop name (k6).
        fx("TRP_020", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Stop3,41.2,29.2\n"),
            ("trips.txt", "route_id,service_id,trip_id,trip_headsign\nR1,SVC1,T1,Stop2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT1,08:20:00,08:20:00,S3,3\n"),
        ]),
        // TRP_021: bikes_allowed is never specified — base triggers it (k2).
        fx("TRP_021", vec![]),
        // TRP_022: overlapping trip times within a block (k6).
        fx("TRP_022", vec![
            ("trips.txt", "route_id,service_id,trip_id,block_id\nR1,SVC1,T1,B1\nR1,SVC1,T2,B1\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,09:00:00,09:00:00,S2,2\nT2,08:30:00,08:30:00,S1,1\nT2,09:30:00,09:30:00,S2,2\n"),
        ]),
        // TRP_023: no active trip in the next 7 days (feed-level k6).
        fx("TRP_023", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,1,1,20270101,20270131\n")]),
        // TRP_024: inconsistent route_type within a block (k6).
        fx("TRP_024", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,0\n"),
            ("trips.txt", "route_id,service_id,trip_id,block_id\nR1,SVC1,T1,B1\nR2,SVC1,T2,B1\n"),
        ]),
        // TRP_025: wheelchair_accessible unknown ratio 80–99%. The base feed's one trip would
        // give 100%, which belongs to TRP_029 — here 1 of 6 trips is specified (83%).
        fx("TRP_025", vec![("trips.txt", "route_id,service_id,trip_id,wheelchair_accessible\n\
              R1,SVC1,T1,1\nR1,SVC1,T2,\nR1,SVC1,T3,\nR1,SVC1,T4,\nR1,SVC1,T5,\nR1,SVC1,T6,\n")]),
        // TRP_026: service has no active dates (k6).
        fx("TRP_026", vec![("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,0,0,0,0,0,0,0,20250101,20271231\n")]),
        // TRP_028: wheelchair_accessible missing from some trips (k6).
        fx("TRP_028", vec![("trips.txt", "route_id,service_id,trip_id,wheelchair_accessible\nR1,SVC1,T1,1\nR1,SVC1,T2,\n")]),
        // TRP_029: no trip has wheelchair_accessible — base triggers it (k6).
        fx("TRP_029", vec![]),

        // ── TRN group (remaining: k4 + k2) ─────────────────────────────────────
        // TRN_004: record_id is absent from the referenced table (k4).
        fx("TRN_004", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,fr,X,NOPE\n")]),
        // TRN_005: same key + same translation on multiple rows (k4).
        fx("TRN_005", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,fr,X,S1\nstops,stop_name,fr,X,S1\n")]),
        // TRN_006: same key + different translation (conflict) (k4).
        fx("TRN_006", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,fr,X,S1\nstops,stop_name,fr,Y,S1\n")]),
        // TRN_007: translation language matches feed_lang (>1 row → feed-level) (k4).
        fx("TRN_007", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,en\n"),
            ("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,en,X,S1\nstops,stop_name,en,Y,S2\n"),
        ]),
        // TRN_008: translation value is empty (k2).
        fx("TRN_008", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nstops,stop_name,fr,,S1\n")]),
        // TRN_010: record_sub_id in a stop_times translation is not the trip's stop_sequence (k4).
        fx("TRN_010", vec![
            ("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,en\n"),
            ("translations.txt", "table_name,field_name,language,translation,record_id,record_sub_id\nstop_times,stop_headsign,fr,X,T1,99\n"),
        ]),
        // TRN_013: feed_info translation uses record_id/sub_id/field_value (k2).
        fx("TRN_013", vec![("translations.txt", "table_name,field_name,language,translation,record_id\nfeed_info,feed_publisher_name,fr,X,FI\n")]),
        // TRN_014: record_sub_id in a table other than stop_times (k2).
        fx("TRN_014", vec![("translations.txt", "table_name,field_name,language,translation,record_id,record_sub_id\nstops,stop_name,fr,X,S1,2\n")]),
        // TRN_015: both record_id and field_value are empty — "Required if record_id is empty".
        fx("TRN_015", vec![("translations.txt", "table_name,field_name,language,translation\nstops,stop_name,fr,X\n")]),
        fx("TRN_016", vec![("stops.txt","stop_id,stop_name,stop_lat,stop_lon\nS1,Merkez,41.0,29.0\n"), ("translations.txt","table_name,field_name,language,translation,field_value\nstops,stop_name,en,Centre,Yokistan\n")]),
        fx("TRN_017", vec![("stop_times.txt","trip_id,stop_id,stop_sequence,arrival_time,departure_time\nT1,S1,1,08:00:00,08:00:00\n"), ("translations.txt","table_name,field_name,language,translation,record_id\nstop_times,stop_headsign,en,Centre,T1\n")]),

        // ── STP group (remaining: k2 + k3 dup + k4 FK + k6) ────────────────────
        // STP_001: duplicate stop_id (k3).
        fx("STP_001", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,A,41.0,29.0\nS1,B,41.1,29.1\n")]),
        // STP_009: parent_station is absent from stops.txt (k4).
        fx("STP_009", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,parent_station\nS1,Stop1,41.0,29.0,\nS2,Stop2,41.1,29.1,NOPE\n")]),
        // STP_010: parent_station is not location_type=1 (k4).
        fx("STP_010", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nS1,Stop1,41.0,29.0,0,\nS2,Stop2,41.1,29.1,0,S1\n")]),
        // STP_011: no parent_station for location_type 2/3/4 (k4).
        fx("STP_011", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Stop1,41.0,29.0,2\nS2,Stop2,41.1,29.1,0\n")]),
        // STP_012: stop used in stop_times has location_type != 0 (k4).
        fx("STP_012", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Stop1,41.0,29.0,1\nS2,Stop2,41.1,29.1,0\n")]),
        // STP_015: level_id is absent from levels.txt (k4).
        fx("STP_015", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,level_id\nS1,Stop1,41.0,29.0,NOPE\nS2,Stop2,41.1,29.1,\n")]),
        // STP_017: two stops are very close (< 5 m, > 0) (k6).
        fx("STP_017", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.00002,40.0\n")]),
        // STP_019: stop_name > 100 karakter (k2).
        fx("STP_019", vec![("stops.txt", concat!("stop_id,stop_name,stop_lat,stop_lon\nS1,",
            "Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore",
            ",41.0,29.0\nS2,Stop2,41.1,29.1\n"))]),
        // STP_020: physical stop unused in stop_times (k6).
        fx("STP_020", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Stop3,41.2,29.2\n")]),
        // STP_021: boarding area (loc=4) parent is not a platform (loc=0) — BA1 parent is a
        // station (loc=1) (k4).
        fx("STP_021", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nST1,Station,41.0,29.0,1,\nBA1,BoardArea,41.0,29.0,4,ST1\nS1,Stop1,41.0,29.0,0,\nS2,Stop2,41.1,29.1,0,\n")]),
        // STP_023: tts_stop_name contains '<'/'>' (k2).
        fx("STP_023", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,tts_stop_name\nS1,Stop1,41.0,29.0,<b>X\nS2,Stop2,41.1,29.1,\n")]),
        // STP_024: invalid stop_access enum (k2).
        fx("STP_024", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_access\nS1,Stop1,41.0,29.0,5\nS2,Stop2,41.1,29.1,\n")]),
        // STP_026: invalid raw stop_access enum (k4).
        fx("STP_026", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_access\nS1,Stop1,41.0,29.0,2\nS2,Stop2,41.1,29.1,9\n")]),
        // STP_028: stop_code > 50 karakter (k2).
        fx("STP_028", vec![("stops.txt", "stop_id,stop_code,stop_name,stop_lat,stop_lon\nS1,AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA,Stop1,41.0,29.0\nS2,C2,Stop2,41.1,29.1\n")]),
        // STP_029: stop is far from parent_station (> 150 m) (k6).
        fx("STP_029", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nST1,Station,41.0,29.0,1,\nS1,Stop1,41.5,29.5,0,ST1\nS2,Stop2,41.1,29.1,0,\n")]),
        // STP_030: parent station with no children (loc=1) (k6).
        fx("STP_030", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Stop1,41.0,29.0,0\nS2,Stop2,41.1,29.1,0\nST1,Station,41.2,29.2,1\n")]),
        // STP_031: stop_name == stop_desc (k2).
        fx("STP_031", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_desc\nS1,Stop1,41.0,29.0,Stop1\nS2,Stop2,41.1,29.1,\n")]),
        // STP_032: no parent_station for the platform connected by the pathway (k4).
        fx("STP_032", vec![("pathways.txt", "pathway_id,from_stop_id,to_stop_id,pathway_mode,is_bidirectional\nP1,S1,S2,3,0\n")]),
        // STP_034: stop_url matches the agency URL (k6).
        fx("STP_034", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_url\nS1,Stop1,41.0,29.0,http://test.example\nS2,Stop2,41.1,29.1,\n")]),
        // STP_042: stop_url is not a URL.
        fx("STP_042", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_url\nS1,Stop1,41.0,29.0,notaurl\n")]),
        // STP_043: stop_access used on a stop without parent_station (spec: forbidden).
        fx("STP_043", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station,stop_access\nS1,Stop1,41.0,29.0,0,,1\nS2,Stop2,41.1,29.1,,,\n")]),
        fx("STP_044", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,platform_code\nS1,Stop1,41.0,29.0,Gleis 5\n")]),
        // STP_035: stop_url matches the route URL (k6).
        fx("STP_035", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type,route_url\nR1,1,101,3,http://r1.example\n"),
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_url\nS1,Stop1,41.0,29.0,http://r1.example\nS2,Stop2,41.1,29.1,\n"),
        ]),
        // STP_036: station (loc=1) contains parent_station (k4).
        fx("STP_036", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\nST1,Station1,41.0,29.0,1,ST2\nST2,Station2,41.2,29.2,1,\nS1,Stop1,41.0,29.0,0,\nS2,Stop2,41.1,29.1,0,\n")]),
        // STP_037: wheelchair_boarding missing from some physical stops (k6).
        fx("STP_037", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,wheelchair_boarding\nS1,Stop1,41.0,29.0,1\nS2,Stop2,41.1,29.1,\n")]),
        // STP_022: stop_code missing — base triggers it (k2).
        fx("STP_022", vec![]),
        // STP_033: zone_id missing — meaningful when fare_rules use zones (k2 + lib).
        fx("STP_033", vec![
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method,transfers\nF1,2.5,USD,0,0\n"),
            ("fare_rules.txt", "fare_id,route_id,origin_id,destination_id\nF1,R1,A,B\n"),
        ]),
        // STP_038: no stop has wheelchair_boarding — base triggers it (k6).
        fx("STP_038", vec![]),
        fx("STP_039", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,stop_code\nS1,Stop1,41.0,29.0,100\nS2,Stop2,41.1,29.1,100\n")]),

        // ── SHP group (remaining: k5 geometry + k4 + k6). SHP_026 (>5000 points) cannot
        //    be written inline → left in debt. ──────────────────────────────────
        // SHP_006: one-point shape in use (k5); unused one-point shapes are covered by SHP_018.
        fx("SHP_006", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\n"),
        ]),
        // SHP_009: self-intersecting shape — seg(2→3) intersects seg(4→5) (k6).
        // (Four points are insufficient: the only comparison (0,n-2) is skipped by the
        // "adjacent endpoint" guard.)
        fx("SHP_009", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.1,2\nSH1,40.1,40.1,3\nSH1,40.05,40.15,4\nSH1,40.05,40.05,5\n")]),
        // SHP_010: consecutive identical coordinates (k5).
        fx("SHP_010", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.0,2\nSH1,40.1,40.1,3\n")]),
        // SHP_012: shape is farther from stops than the threshold (k6).
        fx("SHP_012", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.05,40.05\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.05,2\nSH1,40.0,40.1,3\n"),
        ]),
        // SHP_014: shape start point is > 100 m from the first stop (k6).
        fx("SHP_014", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,41.0\nS2,Stop2,40.0,40.1\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.1,2\nSH1,40.0,40.2,3\n"),
        ]),
        // SHP_015: low point density (3 points / ~66 km) (k6).
        fx("SHP_015", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.3,2\nSH1,40.0,40.6,3\n")]),
        // SHP_016: shape drawn in the reverse direction (k6).
        fx("SHP_016", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.0,40.5\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.5,1\nSH1,40.0,40.25,2\nSH1,40.0,40.0,3\n"),
        ]),
        // SHP_017: stop order conflicts with shape projection (decreasing sdt) (k6).
        fx("SHP_017", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.0,40.1\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.1,2\nSH1,40.0,40.2,3\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,1000\nT1,08:10:00,08:10:00,S2,2,500\n"),
        ]),
        // SHP_018: shape referenced by no trip (k5).
        fx("SHP_018", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.001,40.001,2\nSH1,40.002,40.002,3\n")]),
        // SHP_019: all trips referencing the shape have no stop_times (k4).
        fx_rm("SHP_019",
            vec![
                ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
                ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.1,40.1,2\nSH1,40.2,40.2,3\n"),
                ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n"),
            ],
            vec![]),
        // SHP_020: non-consecutive repeated point (k6).
        fx("SHP_020", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.1,40.1,2\nSH1,40.0,40.0,3\n")]),
        // SHP_022: stop is near 2 separate sections of the shape (loop, no sdt) (k6).
        // #52: rule now skips trips with valid stop_sequence (resolves order ambiguity);
        // fixture uses DUPLICATE stop_sequence → order is unusable → emits SHP_022.
        fx("SHP_022", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.1,40.1\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,40.0,40.0,1\nSH1,40.0,40.2,2\nSH1,40.2,40.2,3\nSH1,40.2,40.0,4\nSH1,40.0,40.0,5\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,1\n"),
        ]),
        // SHP_023: same dist_traveled + same coordinates (k6).
        fx("SHP_023", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,40.0,40.0,1,5\nSH1,40.0,40.0,2,5\nSH1,40.1,40.1,3,10\n")]),
        // SHP_024: stop is far from its shape_dist_traveled position (k6).
        fx("SHP_024", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,41.0\nS2,Stop2,40.0,40.0\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,40.0,40.0,1,0\nSH1,40.0,40.1,2,1000\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S2,1,0\nT1,08:10:00,08:10:00,S1,2,500\n"),
        ]),
        // SHP_025: trip shape_dist_traveled exceeds the shape maximum (k6).
        fx("SHP_025", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,40.0,40.0,1,0\nSH1,40.0,40.1,2,100\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,0\nT1,08:10:00,08:10:00,S2,2,200\n"),
        ]),
        // SHP_028: same distance, different coordinates (above threshold ~0.1°) (k6).
        fx("SHP_028", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,40.0,40.0,1,5\nSH1,40.1,40.0,2,5\nSH1,40.2,40.2,3,10\n")]),
        // SHP_029: same distance, different coordinates (below threshold ~1e-6°) (k6).
        fx("SHP_029", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,40.0,40.0,1,5\nSH1,40.000001,40.0,2,5\nSH1,40.1,40.1,3,10\n")]),
        // SHP_030: stop_times uses shape_dist_traveled, but only one shape point has a
        // corresponding shapes.txt distance (k6, shape summary).
        fx("SHP_030", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,41.0,29.0,1,0\nSH1,41.1,29.1,2,\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,0\nT1,08:10:00,08:10:00,S2,2,1\n"),
        ]),

        // ── STM group (remaining: k4 + k6 + k2). STM_043 (>200 stops) and STM_044
        //    (>2M rows) cannot be written inline → left in debt. ─────────────────
        // STM_001: stop_times trip_id is absent from trips.txt (k4).
        fx("STM_001", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nTGHOST,08:00:00,08:00:00,S1,1\n")]),
        // STM_002: stop_times stop_id is absent from stops.txt (k4).
        fx("STM_002", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,SNOPE,2\n")]),
        // STM_007: departure < arrival at the same stop (k6).
        fx("STM_007", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:10:00,08:00:00,S1,1\nT1,08:20:00,08:20:00,S2,2\n")]),
        // STM_008: time moves backward between stops (k6).
        fx("STM_008", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,09:00:00,09:00:00,S1,1\nT1,08:00:00,08:00:00,S2,2\n")]),
        // STM_012: zero travel time but distance >= 1 km (base stops ~14 km apart) (k6).
        fx("STM_012", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:00:00,08:00:00,S2,2\n")]),
        // STM_014: speed threshold exceeded (~208 km/h: bus threshold 120 < speed < 700
        // impossible limit) (k6).
        fx("STM_014", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:04:00,08:04:00,S2,2\n")]),
        // STM_015: departure_time missing at the first stop (k6).
        fx("STM_015", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,,,S1,1\nT1,08:10:00,08:10:00,S2,2\n")]),
        // STM_016: arrival_time missing at the last stop (k6).
        fx("STM_016", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,,,S2,2\n")]),
        // STM_017: shape_dist_traveled missing from a trip with a shape (k6).
        fx("STM_017", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\nSH1,41.05,29.05,2\nSH1,41.1,29.1,3\n"),
        ]),
        // STM_020: zero-travel-time segment, distance 0.2-1 km, seconds present (k6).
        fx("STM_020", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.005,40.0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:30,08:00:30,S1,1\nT1,08:00:30,08:00:30,S2,2\n"),
        ]),
        // STM_021: different stops at the same coordinates (k6).
        fx("STM_021", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.0,40.0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n"),
        ]),
        // STM_024: shape_dist_traveled unit mismatch (10× ratio) (k4).
        fx("STM_024", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,41.0,29.0,1,0\nSH1,41.1,29.1,2,100\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,0\nT1,08:10:00,08:10:00,S2,2,1000\n"),
        ]),
        // STM_025: segment travel time < 10 s (k6).
        fx("STM_025", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.001,40.0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:00:05,08:00:05,S2,2\n"),
        ]),
        // STM_026: distance between stops > 50 km (k6).
        fx("STM_026", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,41.0,40.0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,10:00:00,10:00:00,S2,2\n"),
        ]),
        // STM_028: trip duration > 24 hours (k6).
        fx("STM_028", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,33:00:00,33:00:00,S2,2\n")]),
        // STM_029: trip duration < 60 s (k6).
        fx("STM_029", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,40.0,40.0\nS2,Stop2,40.0005,40.0\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:00:30,08:00:30,S2,2\n"),
        ]),
        // STM_032: duplicate (trip_id, stop_sequence) (k2).
        fx("STM_032", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,1\n")]),
        // STM_056: consecutive shape_dist_traveled values do not increase (second equals first).
        fx("STM_056", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,100.0\nT1,08:10:00,08:10:00,S2,2,100.0\n")]),
        // STM_033: one-stop trip (k4).
        fx("STM_033", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\n")]),
        // STM_035: same stop twice consecutively (k6).
        fx("STM_035", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S1,2\n")]),
        // STM_036: stop_sequence in descending order (k6, detected by k2).
        fx("STM_036", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,2\nT1,08:10:00,08:10:00,S2,1\n")]),
        // STM_042: stop_headsign contains a forbidden character (k2).
        fx("STM_042", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\nT1,08:00:00,08:00:00,S1,1,Bad!Sign\nT1,08:10:00,08:10:00,S2,2,\n")]),
        // STM_045: departure time exceeds the service-day window (>27 h) (k6).
        fx("STM_045", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,28:00:00,28:00:00,S2,2\n")]),
        // STM_048: 00:xx written after midnight (k2 raw Spec, before normalization).
        fx("STM_048", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,23:50:00,S1,1\nT1,00:10:00,00:10:00,S2,2\n")]),
        // STM_049: 00:xx departure after midnight on the same row (k2 raw Spec).
        fx("STM_049", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,00:10:00,S1,1\nT1,24:10:00,24:10:00,S2,2\n")]),
        // STM_050: timepoint column exists but the value is empty (k2).
        fx("STM_050", vec![("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,timepoint\nT1,08:00:00,08:00:00,S1,1,\nT1,08:10:00,08:10:00,S2,2,\n")]),

        // ── ARC group (k1_parse archive/file/header). ARC_002/003 are proven with a raw-byte
        //    fixture (fx_raw), ARC_022 with a config threshold (fx_cfg, above); ARC_004 is fatal
        //    → allowlist. Debt ledger EMPTY. ──────────────────────────────────────
        // ARC_006: optional file present (feed_info.txt).
        fx("ARC_006", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\nPub,https://x.example,en\n")]),
        // ARC_007: unknown non-GTFS file.
        fx("ARC_007", vec![("extra.txt", "col1\nval1\n")]),
        // ARC_010: file starts with a UTF-8 BOM (EF BB BF).
        fx("ARC_010", vec![("stops.txt", "\u{feff}stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // ARC_011: file size (Info) — base already triggers it.
        fx("ARC_011", vec![]),
        // ARC_013: CSV tokenization error (unclosed quote, optional file).
        fx("ARC_013", vec![("feed_info.txt", "feed_publisher_name,feed_publisher_url,feed_lang\n\"unclosed,https://x.example,en\n")]),
        // ARC_014: leading/trailing whitespace in a header.
        fx("ARC_014", vec![("stops.txt", "stop_id, stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // ARC_017: unknown non-GTFS column.
        fx("ARC_017", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,bogus_col\nS1,Stop1,41.0,29.0,x\nS2,Stop2,41.1,29.1,y\n")]),
        // ARC_018: completely empty data row.
        fx("ARC_018", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\n,,,\nS2,Stop2,41.1,29.1\n")]),
        // ARC_034: header row repeated as data in the middle of the file.
        // To trigger it, ALL fields in the row must equal their own column names.
        fx("ARC_034", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nstop_id,stop_name,stop_lat,stop_lon\nS2,Stop2,41.1,29.1\n")]),
        // ARC_020: recommended file missing (shapes/feed_info) — base already triggers it.
        fx("ARC_020", vec![]),
        // ARC_021: value containing an unprintable/control character (U+0001).
        fx("ARC_021", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,A\u{1}B,41.0,29.0\nS2,Stop2,41.1,29.1\n")]),
        // ARC_023: nested ZIP file inside the ZIP.
        fx("ARC_023", vec![("inner.zip", "dummy\n")]),
        // ARC_035: required file exists but contains only a header row (no data).
        fx("ARC_035", vec![("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\n")]),

        // ── TRF group (remaining: transfers k4 FK + k6) ────────────────────────
        // TRF_003: from/to_stop_id is absent from stops.txt (k4).
        fx("TRF_003", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type\nNOPE,S2,1\n")]),
        // TRF_006: from_trip_id is absent from trips.txt (k4).
        fx("TRF_006", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id\nS1,S2,1,TNOPE\n")]),
        // TRF_007: to_trip_id is absent from trips.txt (k4).
        fx("TRF_007", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,to_trip_id\nS1,S2,1,TNOPE\n")]),
        // TRF_008: from_route_id is absent from routes.txt (k4).
        fx("TRF_008", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_route_id\nS1,S2,1,RNOPE\n")]),
        // TRF_009: to_route_id is absent from routes.txt (k4).
        fx("TRF_009", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,to_route_id\nS1,S2,1,RNOPE\n")]),
        // TRF_010: min_transfer_time > 3600s (k4).
        fx("TRF_010", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,min_transfer_time\nS1,S2,2,5000\n")]),
        // TRF_011: transfer distance > 2000 m (base S1↔S2 ~14 km) (k6).
        fx("TRF_011", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,min_transfer_time\nS1,S2,2,300\n")]),
        // TRF_012: duplicate from/to stop pair (without trip/route context) (k4).
        fx("TRF_012", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type\nS1,S2,1\nS1,S2,1\n")]),
        // TRF_013: from/to_trip_id required for type=4/5 (k4).
        fx("TRF_013", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type\nS1,S2,4\n")]),
        // TRF_014: in-seat transfer trip has no stop_times record (k4).
        fx("TRF_014", vec![
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n"),
            ("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T2,T1\n"),
        ]),
        // TRF_015: from/to_stop cannot be a station (loc=1) for type=4/5 (k4).
        fx("TRF_015", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Stop1,41.0,29.0,1\nS2,Stop2,41.1,29.1,0\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n"),
            ("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T1,T2\n"),
        ]),
        // TRF_021: transfer endpoint cannot be an entry/intermediate node/boarding area (k4).
        fx("TRF_021", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,location_type\nS1,Entrance,41.0,29.0,2\nS2,Stop2,41.1,29.1,0\n"),
            ("transfers.txt", "from_stop_id,to_stop_id,transfer_type\nS1,S2,1\n"),
        ]),
        // TRF_016: conflicting transfer for the same (stop,trip,route) combination (k4).
        fx("TRF_016", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id\nS1,S2,1,T1\nS1,S2,1,T1\n")]),
        // TRF_017: actual route of from_trip_id does not match from_route_id (k4).
        fx("TRF_017", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,from_route_id\nS1,S2,1,T1,RWRONG\n")]),
        // TRF_018: from_trip_id == to_trip_id (meaningless transfer) (k4).
        fx("TRF_018", vec![("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,1,T1,T1\n")]),
        // TRF_022: 1-to-n continuation — T1 connects to both T2 and T3; their calendars differ
        // but overlap (SVC1 weekdays ⊂ SVC2 every day) → continuation is ambiguous on those
        // days. ⚠️ With the same calendar the rule should be SILENT (true 1-to-n split), and
        // it should also be SILENT without overlap (separate continuations allowed by the spec)
        // — see the k6 note.
        fx("TRF_022", vec![
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\nSVC2,1,1,1,1,1,1,1,20250101,20271231\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC2,T3\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\nT3,10:00:00,10:00:00,S1,1\nT3,10:10:00,10:10:00,S2,2\n"),
            ("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T1,T2\nS1,S2,4,T1,T3\n"),
        ]),
        // TRF_023: n-to-1 continuation — both T2 and T3 connect to T1, same overlap.
        fx("TRF_023", vec![
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\nSVC2,1,1,1,1,1,1,1,20250101,20271231\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC2,T3\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\nT3,10:00:00,10:00:00,S1,1\nT3,10:10:00,10:10:00,S2,2\n"),
            ("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T2,T1\nS1,S2,4,T3,T1\n"),
        ]),
        // TRF_019: different route_type in an in-seat transfer (k4).
        fx("TRF_019", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,0\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR2,SVC1,T2\n"),
            ("transfers.txt", "from_stop_id,to_stop_id,transfer_type,from_trip_id,to_trip_id\nS1,S2,4,T1,T2\n"),
        ]),

        // ── VAT group (entity analytics, k6). VAT_006 (≥50 trips) inline bloat →
        //    left in debt. ─────────────────────────────────────────────────────
        // VAT_001: two routes (same route_type) share >=85% of stops (route >=5 stops).
        fx("VAT_001", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Stop3,41.2,29.2\nS4,Stop4,41.3,29.3\nS5,Stop5,41.4,29.4\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,3\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR2,SVC1,T2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT1,08:20:00,08:20:00,S3,3\nT1,08:30:00,08:30:00,S4,4\nT1,08:40:00,08:40:00,S5,5\nT2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\nT2,09:20:00,09:20:00,S3,3\nT2,09:30:00,09:30:00,S4,4\nT2,09:40:00,09:40:00,S5,5\n"),
        ]),
        // VAT_002: stop has >=4 routes but no transfers are defined (transfer hub).
        // Four routes pass through S1 AND a route absent from S1 (R5) passes through S9
        // ~200 m away: because transfer at the same stop_id is implicit, the rule now needs
        // a separate nearby stop that is not connected.
        fx("VAT_002", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS9,Near,41.0018,29.0\nS8,Other,41.05,29.05\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,1,3\nR2,1,2,3\nR3,1,3,3\nR4,1,4,3\nR5,1,5,3\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR2,SVC1,T2\nR3,SVC1,T3\nR4,SVC1,T4\nR5,SVC1,T5\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:30:00,08:30:00,S1,1\nT2,08:40:00,08:40:00,S2,2\nT3,09:00:00,09:00:00,S1,1\nT3,09:10:00,09:10:00,S2,2\nT4,09:30:00,09:30:00,S1,1\nT4,09:40:00,09:40:00,S2,2\nT5,10:00:00,10:00:00,S9,1\nT5,10:10:00,10:10:00,S8,2\n"),
        ]),
        // VAT_003: statistically unusual trip duration (same shape, 6 trips, one ~50 min).
        fx("VAT_003", vec![
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\nR1,SVC1,T2,SH1\nR1,SVC1,T3,SH1\nR1,SVC1,T4,SH1\nR1,SVC1,T5,SH1\nR1,SVC1,T6,SH1\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\nSH1,41.1,29.1,2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:05:00,08:05:00,S1,1\nT2,08:15:00,08:15:00,S2,2\nT3,08:10:00,08:10:00,S1,1\nT3,08:20:00,08:20:00,S2,2\nT4,08:15:00,08:15:00,S1,1\nT4,08:25:00,08:25:00,S2,2\nT5,08:20:00,08:20:00,S1,1\nT5,08:30:00,08:30:00,S2,2\nT6,08:25:00,08:25:00,S1,1\nT6,09:15:00,09:15:00,S2,2\n"),
        ]),
        // VAT_005: isolated stop cluster disconnected from the main network (BFS component).
        fx("VAT_005", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.01,29.01\nS3,Stop3,41.02,29.02\nS4,Stop4,41.03,29.03\nS5,Iso1,10.0,10.0\nS6,Iso2,10.01,10.01\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT1,08:20:00,08:20:00,S3,3\nT1,08:30:00,08:30:00,S4,4\nT2,09:00:00,09:00:00,S5,1\nT2,09:10:00,09:10:00,S6,2\n"),
        ]),
        // VAT_007: terminus stop has >=3 routes, no transfer.
        // S2 is the terminus of three routes AND a route absent from S2 (R4) passes through S9
        // ~200 m away.
        fx("VAT_007", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\nS3,Stop3,41.2,29.2\nS4,Stop4,41.3,29.3\nS9,Near,41.1018,29.1\nS5,Other,41.15,29.15\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,1,3\nR2,1,2,3\nR3,1,3,3\nR4,1,4,3\n"),
            ("trips.txt", "route_id,service_id,trip_id\nR1,SVC1,T1\nR2,SVC1,T2\nR3,SVC1,T3\nR4,SVC1,T4\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,08:30:00,08:30:00,S3,1\nT2,08:40:00,08:40:00,S2,2\nT3,09:00:00,09:00:00,S4,1\nT3,09:10:00,09:10:00,S2,2\nT4,10:00:00,10:00:00,S9,1\nT4,10:10:00,10:10:00,S5,2\n"),
        ]),
        // VAT_008: same shape on >30% of routes AND >=3 routes (5 routes, SH1 on 3) (k6).
        fx("VAT_008", vec![
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,1,3\nR2,1,2,3\nR3,1,3,3\nR4,1,4,3\nR5,1,5,3\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\nR2,SVC1,T2,SH1\nR3,SVC1,T3,SH1\nR4,SVC1,T4,\nR5,SVC1,T5,\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\nSH1,41.1,29.1,2\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nT2,09:00:00,09:00:00,S1,1\nT2,09:10:00,09:10:00,S2,2\nT3,10:00:00,10:00:00,S1,1\nT3,10:10:00,10:10:00,S2,2\n"),
        ]),

        // ── FRL group (fare_rules cross-ref k4) ────────────────────────────────
        // FRL_001: fare_id is absent from fare_attributes.txt.
        fx("FRL_001", vec![("fare_rules.txt", "fare_id,route_id\nFNOPE,R1\n")]),
        // FRL_002: route_id is absent from routes.txt.
        fx("FRL_002", vec![
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
            ("fare_rules.txt", "fare_id,route_id\nF1,RNOPE\n"),
        ]),
        // FRL_003: origin_id is not a valid zone.
        fx("FRL_003", vec![
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
            ("fare_rules.txt", "fare_id,origin_id\nF1,ZNOPE\n"),
        ]),
        // FRL_004: destination_id is not a valid zone.
        fx("FRL_004", vec![
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
            ("fare_rules.txt", "fare_id,destination_id\nF1,ZNOPE\n"),
        ]),
        // FRL_005: contains_id is not a valid zone.
        fx("FRL_005", vec![
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
            ("fare_rules.txt", "fare_id,contains_id\nF1,ZNOPE\n"),
        ]),
        // FRL_006: fare_attributes exists but fare_rules is absent.
        fx("FRL_006", vec![("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n")]),
        // FRL_007: rule has no discriminating criterion (route/zone/contains all empty).
        fx("FRL_007", vec![
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
            ("fare_rules.txt", "fare_id\nF1\n"),
        ]),
        // FRL_008: route-based fare but some routes are out of scope (R2).
        fx("FRL_008", vec![
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
            ("routes.txt", "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\nR2,1,102,3\n"),
            ("fare_rules.txt", "fare_id,route_id\nF1,R1\n"),
        ]),
        // FRL_009: only reverse direction (Z2→Z1) is priced; Z1→Z2, which the trip actually
        // connects, is missing.
        fx("FRL_009", vec![
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon,zone_id\nS1,Stop1,41.0,29.0,Z1\nS2,Stop2,41.1,29.1,Z2\n"),
            ("fare_attributes.txt", "fare_id,price,currency_type,payment_method\nF1,2.5,USD,0\n"),
            ("fare_rules.txt", "fare_id,route_id,origin_id,destination_id\nF1,R1,Z2,Z1\n"),
        ]),

        // ── SHP_021 (k2) ───────────────────────────────────────────────────────
        // SHP_021: shape_dist_traveled negatif.
        fx("SHP_021", vec![("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,41.0,29.0,1,-5\nSH1,41.1,29.1,2,10\n")]),
    ]
}

#[test]
fn each_fixture_actually_emits_its_rule() {
    let mut failures = Vec::new();
    for f in fixtures() {
        let cfg = f.config.clone().unwrap_or_default();
        let no_read: Vec<&str> = f.no_read.clone();
        let emitted = emitted_rules_modes(
            &with_opts(&f.overrides, &f.removes, &f.raw),
            &cfg,
            &no_read,
            f.flip_stream_flags,
        );
        if !emitted.contains(f.rule) {
            failures.push(format!("  {} emit etmedi → {:?}", f.rule, emitted));
        }
    }
    assert!(
        failures.is_empty(),
        "Emit etmeyen fixture(lar):\n{}",
        failures.join("\n")
    );
}

/// All values of the `column` column in fixture files (trimmed). Simple CSV:
/// fixtures contain no quoted commas; quote marks are only stripped.
fn column_values(files: &[(String, Vec<u8>)], column: &str) -> Option<BTreeSet<String>> {
    let mut values = BTreeSet::new();
    let mut seen = false;
    for (_, bytes) in files {
        let text = String::from_utf8_lossy(bytes);
        let mut lines = text.lines();
        let Some(header) = lines.next() else { continue };
        let header = header.trim_start_matches('\u{feff}');
        let Some(idx) = header.split(',').position(|h| h.trim().trim_matches('"') == column) else {
            continue;
        };
        seen = true;
        for line in lines {
            if let Some(v) = line.split(',').nth(idx) {
                values.insert(v.trim().trim_matches('"').trim().to_string());
            }
        }
    }
    seen.then_some(values)
}

/// `scope_key` must have the registry identity field's (`scope_key_field`) type: `blocks`
/// suppression and agency attribution match on this key. SHP_017 (2026-10-11) carried
/// trip_id while its identity field was `shape_id`; the XFL_013 → SHP_017 link never matched.
/// Feed-level summaries (`scope_key = None`) and fields absent from the fixture column
/// cannot be evaluated and are skipped.
#[test]
fn scope_key_is_a_value_of_the_registry_identity_field() {
    // Deliberately malformed CSV: the simple parser cannot read the column correctly (this is
    // a fixture property, not a gate property). ARC_015 has a repeated header; ARC_030 has a
    // newline inside quotes.
    const MALFORMED_CSV_FIXTURES: &[&str] = &["ARC_015", "ARC_030"];
    let mut failures = BTreeSet::new();
    let mut checked = BTreeSet::new();
    for f in fixtures() {
        if !f.no_read.is_empty() || f.flip_stream_flags || MALFORMED_CSV_FIXTURES.contains(&f.rule) {
            continue;
        }
        let files = with_opts(&f.overrides, &f.removes, &f.raw);
        let cfg = f.config.clone().unwrap_or_default();
        let ValidateResult::Ok(vr) = validate_bytes(&make_zip(&files), &cfg, TODAY) else {
            continue;
        };
        for n in &vr.notices {
            let Some(field) = gtfs_rules::get_rule(&n.rule_id).and_then(|m| m.scope_key_field) else {
                continue;
            };
            let Some(key) = n.scope_key.as_deref().filter(|k| !k.is_empty()) else {
                continue;
            };
            let Some(values) = column_values(&files, field) else {
                continue;
            };
            checked.insert(n.rule_id.clone());
            if !values.contains(key.trim()) {
                failures.insert(format!(
                    "  {} ({} fixture): scope_key '{}' is not a {}; fixture {} values: {:?}",
                    n.rule_id, f.rule, key, field, field, values
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "scope_key does not match the identity field ({} rules checked):\n{}",
        checked.len(),
        failures.into_iter().collect::<Vec<_>>().join("\n")
    );
    assert!(checked.len() > 100, "gate checks almost nothing: {}", checked.len());
}

/// A rule that declares no identity field (`scope_key_field = None`) must not carry a
/// `scope_key` at runtime either: the `blocks` gate below consults the registry and would
/// treat such a root as feed-level. SHP_004 (2026-10-11) declared `None` but carried shape_id
/// through K2's `entity_id` fallback; ARC_035/JPN_033 carried a file name.
#[test]
fn rules_without_identity_field_carry_no_scope_key() {
    let mut drifting = BTreeSet::new();
    for f in fixtures() {
        if !f.no_read.is_empty() || f.flip_stream_flags {
            continue;
        }
        let files = with_opts(&f.overrides, &f.removes, &f.raw);
        let cfg = f.config.clone().unwrap_or_default();
        let ValidateResult::Ok(vr) = validate_bytes(&make_zip(&files), &cfg, TODAY) else {
            continue;
        };
        for n in &vr.notices {
            let undeclared = gtfs_rules::get_rule(&n.rule_id).is_some_and(|m| m.scope_key_field.is_none());
            if undeclared && n.scope_key.is_some() {
                drifting.insert(n.rule_id.as_str().to_owned());
            }
        }
    }
    assert!(drifting.is_empty(), "no identity field declared but scope_key is present: {drifting:?}");
}

/// Rules that lose `scope_key` after feed-level aggregation. Since suppression runs AFTER
/// aggregation, these are unscoped regardless of the registry. Ask the aggregation function
/// itself; do not maintain a duplicate list.
fn scope_dropped_by_aggregation(rule_id: &str) -> bool {
    let meta = gtfs_rules::get_rule(rule_id).expect("registry'de olmayan kural");
    let probe = gtfs_core::Notice {
        id: format!("probe/{rule_id}#1"),
        rule_id: rule_id.to_string(),
        severity: meta.severity,
        rule_class: meta.rule_class,
        entity_type: gtfs_core::EntityType::Row,
        entity_id: Some("X".to_string()),
        scope_key: Some("X".to_string()),
        file: Some("stop_times.txt".to_string()),
        line: Some(1),
        field: Some("field".to_string()),
        observed_value: Some("raw".to_string()),
        expected_value: None,
        details: None,
        whitespace_derived: false,
        whitespace_candidate: false,
        title: meta.title.to_string(),
        message: "raw".to_string(),
        remediation: "raw".to_string(),
        blocks: Vec::new(),
        base_effort: meta.base_effort,
        service_id: None,
        agency_distribution: None,
    };
    let mut notices = vec![probe];
    gtfs_pipeline::aggregate_feed_level_notices(
        &mut notices,
        &gtfs_pipeline::EntityRecords::default(),
        &gtfs_pipeline::DerivedData::default(),
    );
    notices.iter().all(|n| n.scope_key.is_none())
}

/// Can two identity fields carry the same value? The `from_`/`to_`/`pickup_`/`drop_off_`
/// prefix indicates the identity of the same table (`from_trip_id` is a trip_id). A composite
/// key (`from_stop_id|to_stop_id` → "A|B") never matches a single identity; it only matches
/// the exact same composite field.
fn same_id_domain(a: &str, b: &str) -> bool {
    if a.contains('|') || b.contains('|') {
        return a == b;
    }
    fn table(f: &str) -> &str {
        ["from_", "to_", "pickup_", "drop_off_"]
            .iter()
            .find_map(|p| f.strip_prefix(p))
            .unwrap_or(f)
    }
    table(a) == table(b)
}

/// A scoped root (`scope_key = Some(x)`) suppresses the rule it blocks ONLY with the same
/// `Some(x)` (`resolve_symptoms`). If the blocked rule is unscoped or carries another table's
/// identity, the link never matches: the card says "blocks" but does not suppress the report.
/// On 2026-10-11, 58 such dead links were removed (e.g. CAL_003 service_id → CAL_008 feed
/// summary, STP_001 stop_id → PTH_002 pathway_id).
#[test]
fn scoped_root_blocks_only_rules_sharing_its_id_domain() {
    let mut failures = Vec::new();
    for root in RULES {
        let Some(root_field) = root.scope_key_field else { continue };
        if root.blocks.is_empty() || scope_dropped_by_aggregation(root.id) {
            continue;
        }
        for &blocked in root.blocks {
            let meta = gtfs_rules::get_rule(blocked).expect("registry'de olmayan kural");
            let reason = match meta.scope_key_field {
                None => "unscoped".to_string(),
                Some(_) if scope_dropped_by_aggregation(blocked) => "becomes unscoped during aggregation".to_string(),
                Some(field) if !same_id_domain(root_field, field) => {
                    format!("carries {field}")
                }
                Some(_) => continue,
            };
            failures.push(format!("  {} ({root_field}) → {blocked}: {reason}", root.id));
        }
    }
    assert!(
        failures.is_empty(),
        "blocks link that can never match:\n{}",
        failures.join("\n")
    );
}

/// Reverse direction: if a root that declares an identity field and has `blocks`
/// emits `scope_key = None`, `resolve_symptoms` treats it as feed-level and suppresses
/// ALL findings for the blocked rule in the feed (over-masking). The old TRF_010 was in this
/// class. This can happen in two ways: aggregation drops the scope or emission leaves it empty.
#[test]
fn scoped_root_with_blocks_never_emits_without_scope_key() {
    let mut failures = BTreeSet::new();
    for root in RULES {
        if root.scope_key_field.is_some() && !root.blocks.is_empty() && scope_dropped_by_aggregation(root.id) {
            failures.insert(format!("  {}: becomes unscoped during aggregation", root.id));
        }
    }
    let mut checked = BTreeSet::new();
    for f in fixtures() {
        if !f.no_read.is_empty() || f.flip_stream_flags {
            continue;
        }
        let files = with_opts(&f.overrides, &f.removes, &f.raw);
        let cfg = f.config.clone().unwrap_or_default();
        let ValidateResult::Ok(vr) = validate_bytes(&make_zip(&files), &cfg, TODAY) else {
            continue;
        };
        for n in &vr.notices {
            // Suppression looks at the notice's own `blocks`: an identity-less row variant
            // (TRP_001 empty trip_id) empties it and removes the root status.
            let scoped_root = !n.blocks.is_empty()
                && gtfs_rules::get_rule(&n.rule_id).is_some_and(|m| m.scope_key_field.is_some());
            if !scoped_root {
                continue;
            }
            checked.insert(n.rule_id.clone());
            if n.scope_key.is_none() {
                failures.insert(format!("  {} ({} fixture): scope_key None", n.rule_id, f.rule));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "scoped root emits without scope ({} roots checked):\n{}",
        checked.len(),
        failures.into_iter().collect::<Vec<_>>().join("\n")
    );
    assert!(checked.len() > 20, "gate checks almost nothing: {}", checked.len());
}

/// `trips.txt` and `calendar_dates.txt` are read only at header level in K1 (streamed files);
/// per-row file hygiene checks must run in the K2 stream. ARC_012/034 had been moved to these
/// two files in #181, while ARC_018/021/030 were missed (noticed on 2026-10-13 while writing
/// ARC_037).
#[test]
fn row_hygiene_checks_run_on_streamed_trips_and_calendar_dates() {
    let files = with_opts(
        &[
            ("trips.txt", "route_id,service_id,trip_id,trip_headsign\nR1,SVC1,T1,Merkez\u{1}\nR1,SVC1,T2,\"Doğu\tYönü\"\n,,,\n"),
            ("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260101,2\n,,\nSVC1,20260102\u{1},2\nSVC1,\"2026\t0103\",2\n"),
        ],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    for file in ["trips.txt", "calendar_dates.txt"] {
        for rule in ["ARC_018", "ARC_021", "ARC_030"] {
            assert!(
                notices.iter().any(|n| n.rule_id == rule && n.file.as_deref() == Some(file)),
                "{rule} should run for {file}: {:?}",
                notices.iter().filter(|n| n.file.as_deref() == Some(file)).map(|n| &n.rule_id).collect::<Vec<_>>()
            );
        }
    }
    // An empty row is the only structural finding; TRP_001 must not be inferred from an empty
    // trip_id (K1 precedent).
    assert!(!notices.iter().any(|n| n.rule_id == "TRP_001"), "empty row must not produce TRP_001");
}

/// ARC_032 must run on all four streamed files. Its card used to state "streamed files are out
/// of scope": `trip_headsign`/`stop_headsign` were never checked (2026-10-13).
#[test]
fn arc032_runs_on_every_streamed_file() {
    let files = with_opts(
        &[
            ("trips.txt", "route_id,service_id,trip_id,trip_headsign,shape_id\nR1,SVC1,T1,Merkez<br>Gar,SH1\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,stop_headsign\nT1,08:00:00,08:00:00,S1,1,Ana&nbsp;Cadde\nT1,08:10:00,08:10:00,S2,2,\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_note\nSH1,41.0,29.0,1,<b>x</b>\nSH1,41.1,29.1,2,\n"),
            ("calendar_dates.txt", "service_id,date,exception_type,note\nSVC1,20260101,2,Bayram&amp;Tatil\n"),
        ],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    let arc032: BTreeSet<&str> = notices
        .iter()
        .filter(|n| n.rule_id == "ARC_032")
        .filter_map(|n| n.file.as_deref())
        .collect();
    assert_eq!(
        arc032,
        BTreeSet::from(["calendar_dates.txt", "shapes.txt", "stop_times.txt", "trips.txt"]),
    );
    assert_eq!(notices.iter().filter(|n| n.rule_id == "ARC_032").count(), 4, "one per file");
}

/// TRP_024 looks for the same VEHICLE changing type within a block. Extended codes are
/// subtypes of the same type: 3 ↔ 715 (demand-responsive bus), 204 ↔ 712 (coach ↔ school bus),
/// 106 ↔ 109 (regional ↔ commuter rail). Corpus run 38363983999: 23 of 37 feeds, and ~96% of
/// notices, had this pattern. A real type change (3 ↔ 0) remains a finding.
#[test]
fn trp024_compares_vehicle_modes_not_raw_route_type_codes() {
    let block = |a: u32, b: u32| {
        with_opts(
            &[
                ("routes.txt", Box::leak(format!("route_id,agency_id,route_short_name,route_type\nR1,1,101,{a}\nR2,1,102,{b}\n").into_boxed_str())),
                ("trips.txt", "route_id,service_id,trip_id,block_id\nR1,SVC1,T1,B1\nR2,SVC1,T2,B1\n"),
            ],
            &[],
            &[],
        )
    };
    let fires = |a, b| {
        notices_for(&block(a, b), &ValidatorConfig::default())
            .iter()
            .any(|n| n.rule_id == "TRP_024")
    };
    for (a, b) in [(3, 715), (204, 712), (106, 109), (700, 3), (1, 401), (0, 900)] {
        assert!(!fires(a, b), "{a} ↔ {b} same vehicle type, must not report");
    }
    for (a, b) in [(3, 0), (2, 3), (3, 11), (109, 3)] {
        assert!(fires(a, b), "{a} ↔ {b} different vehicle type, must report");
    }
}

/// ARC_037 must run on all three parsing paths: K1 (general files), K2 `stop_times` stream
/// (parallel chunks are merged), and K2 `shapes` stream. ONE summary per file; non-ASCII
/// text in display-name fields (stop_name) is not the subject of the rule.
#[test]
fn arc037_reports_non_ascii_ids_once_per_file_on_every_parse_path() {
    let files = with_opts(
        &[
            ("stops.txt", "stop_id,stop_name,stop_lat,stop_lon\nS1,Hauptbahnhof Süd,41.0,29.0\nS2,Stop2,41.1,29.1\n"),
            ("calendar.txt", "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\nSVC1,1,1,1,1,1,0,0,20250101,20271231\n平日,1,1,1,1,1,0,0,20250101,20271231\n休日,0,0,0,0,0,1,1,20250101,20271231\n"),
            ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\nR1,SVC1,Tü2,SH1\n"),
            ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\nTü2,09:00:00,09:00:00,S1,1\nTü2,09:10:00,09:10:00,S2,2\n"),
            ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence\nSH1,41.0,29.0,1\nSH1,41.1,29.1,2\nŞ2,41.0,29.0,1\nŞ2,41.1,29.1,2\n"),
            ("calendar_dates.txt", "service_id,date,exception_type\nSVC1,20260101,2\n祝日,20260101,1\n"),
        ],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    let arc037: Vec<_> = notices.iter().filter(|n| n.rule_id == "ARC_037").collect();
    let by_file: BTreeSet<&str> = arc037.iter().filter_map(|n| n.file.as_deref()).collect();
    assert_eq!(
        by_file,
        BTreeSet::from(["calendar.txt", "calendar_dates.txt", "shapes.txt", "stop_times.txt", "trips.txt"]),
        "{arc037:#?}"
    );
    assert_eq!(arc037.len(), 5, "one summary per file: {arc037:#?}");
    let cal = arc037.iter().find(|n| n.file.as_deref() == Some("calendar.txt")).unwrap();
    assert!(cal.message.contains("2 satırda") || cal.message.contains("2 rows"), "{}", cal.message);
    assert!(!by_file.contains("stops.txt"), "stop_name is a display name, not an ID");
}

#[test]
fn shp030_aggregates_and_does_not_duplicate_related_findings() {
    let overrides = [
        ("trips.txt", "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\nR1,SVC1,T2,SH1\n"),
        ("shapes.txt", "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,41.0,29.0,1,0\nSH1,41.1,29.1,2,\n"),
        ("stop_times.txt", "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\nT1,08:00:00,08:00:00,S1,1,0\nT1,08:10:00,08:10:00,S2,2,1\nT2,09:00:00,09:00:00,S1,1,0\nT2,09:10:00,09:10:00,S2,2,1\n"),
    ];
    let positive = notices_for(
        &with_opts(&overrides, &[], &[]),
        &ValidatorConfig::default(),
    );
    let shp030: Vec<_> = positive.iter().filter(|n| n.rule_id == "SHP_030").collect();
    assert_eq!(shp030.len(), 1, "one SHP_030");
    assert_eq!(shp030[0].entity_id.as_deref(), Some("SH1"));
    assert_eq!(
        shp030[0]
            .details
            .as_ref()
            .and_then(|d| d.get("affected_trip_count"))
            .map(String::as_str),
        Some("2")
    );
    for related in ["STM_017", "SHP_024", "SHP_025"] {
        assert!(
            !positive.iter().any(|n| n.rule_id == related),
            "{related} must not be derived by the SHP_030 fixture"
        );
    }

    let complete_shapes = "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\nSH1,41.0,29.0,1,0\nSH1,41.1,29.1,2,1\n";
    let negative = notices_for(
        &with_opts(
            &[overrides[0], ("shapes.txt", complete_shapes), overrides[2]],
            &[],
            &[],
        ),
        &ValidatorConfig::default(),
    );
    assert!(
        !negative.iter().any(|n| n.rule_id == "SHP_030"),
        "complete shape distance must not produce SHP_030"
    );
}

#[test]
fn nul_shape_distance_keeps_lexical_and_field_type_signals_separate() {
    let shapes = b"shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
SH1,41.0,29.0,1,\0\n\
SH1,41.1,29.1,2,10\n";
    let notices = notices_for(
        &with_opts(&[], &[], &[("shapes.txt", shapes)]),
        &ValidatorConfig::default(),
    );
    assert!(
        notices.iter().any(|n| n.rule_id == "ARC_021"),
        "NUL character must preserve the file-hygiene signal"
    );
    assert!(
        notices.iter().any(|n| n.rule_id == "SHP_021"),
        "NUL character must also preserve the shape_dist_traveled type violation"
    );
}

#[test]
fn shp024_recovers_whitespace_coordinates_and_preserves_thresholds() {
    // tdg-83134 reduced case: coordinates are numerically valid after trimming, but the
    // lexical K2 path keeps them invalid so DQ_016/STP evidence remains available. SHP_024
    // must still use that payload for the user-distance comparison (139.4m and 165.6m).
    let stops = "stop_id,stop_name,stop_lat,stop_lon\n\
S1,Lycée Dupuy, 43.229610,   0.062651\n\
S2,Delacroix, 43.227402,   0.061883\n";
    let routes = "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n";
    let trips = "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n";
    let shapes = "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
SH1,43.229612,0.060398,1,2.094\n\
SH1,43.229628,0.061190,2,2.158\n\
SH1,43.229660,0.062172,3,2.238\n\
SH1,43.229665,0.062354,4,2.253\n\
SH1,43.228891,0.062634,5,2.376\n\
SH1,43.228047,0.062443,6,2.471\n";
    let stop_times =
        "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\n\
T1,08:00:00,08:00:00,S1,1,2.137\n\
T1,08:10:00,08:10:00,S2,2,2.387\n";
    let files = with_opts(
        &[
            ("stops.txt", stops),
            ("routes.txt", routes),
            ("trips.txt", trips),
            ("shapes.txt", shapes),
            ("stop_times.txt", stop_times),
        ],
        &[],
        &[],
    );
    let bus = notices_for(&files, &ValidatorConfig::default());
    let bus_shp024: Vec<_> = bus.iter().filter(|n| n.rule_id == "SHP_024").collect();
    assert_eq!(
        bus_shp024.len(),
        2,
        "two >100 m bus cases must be reported separately: {bus_shp024:?}"
    );
    assert!(bus_shp024
        .iter()
        .any(|n| n.entity_id.as_deref() == Some("S1")
            && n.observed_value
                .as_deref()
                .is_some_and(|v| v.starts_with("distance=139."))));
    assert!(bus_shp024
        .iter()
        .any(|n| n.entity_id.as_deref() == Some("S2")
            && n.observed_value
                .as_deref()
                .is_some_and(|v| v.starts_with("distance=165."))));

    // Same geometry on a rail route: the intentional 200m rail threshold keeps both
    // ~139m/~166m platform-center offsets unflagged.
    let rail_routes = "route_id,agency_id,route_short_name,route_type\nR1,1,101,2\n";
    let rail_files = with_opts(
        &[
            ("routes.txt", rail_routes),
            ("stops.txt", stops),
            ("trips.txt", trips),
            ("shapes.txt", shapes),
            ("stop_times.txt", stop_times),
        ],
        &[],
        &[],
    );
    let rail = notices_for(&rail_files, &ValidatorConfig::default());
    assert!(
        !rail.iter().any(|n| n.rule_id == "SHP_024"),
        "rail threshold exception must be preserved: {rail:?}"
    );
}

#[test]
fn shp024_boundary_uses_bus_threshold_not_geometric_projection() {
    let stops = "stop_id,stop_name,stop_lat,stop_lon\n\
NEAR,Near,40.0,0.0058\n\
FAR,Far,40.0,0.0062\n";
    let trips = "route_id,service_id,trip_id,shape_id\nR1,SVC1,T1,SH1\n";
    let shapes = "shape_id,shape_pt_lat,shape_pt_lon,shape_pt_sequence,shape_dist_traveled\n\
SH1,40.0,0.0,1,0\nSH1,40.0,0.01,2,1\n";
    let stop_times =
        "trip_id,arrival_time,departure_time,stop_id,stop_sequence,shape_dist_traveled\n\
T1,08:00:00,08:00:00,NEAR,1,0.5\n\
T1,08:10:00,08:10:00,FAR,2,0.5\n";
    let files = with_opts(
        &[
            ("stops.txt", stops),
            ("trips.txt", trips),
            ("shapes.txt", shapes),
            ("stop_times.txt", stop_times),
        ],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    let shp024: Vec<_> = notices.iter().filter(|n| n.rule_id == "SHP_024").collect();
    assert_eq!(
        shp024.len(),
        1,
        "only the FAR case above 100 m must be reported: {shp024:?}"
    );
    assert_eq!(shp024[0].entity_id.as_deref(), Some("FAR"));
}

/// Different rules sharing the same fixture body (overrides+removes+raw+config).
/// This was exactly the COMMON signature of today's two duplicate escapes: STM_023/STM_036
/// and OPR_006/STM_033 had an IDENTICAL fixture record in `emit_proof.rs`, and
/// `each_fixture_actually_emits_its_rule` (coverage only) passed both.
///
/// If the same body triggers two different rules, it is either (a) legitimate — the same
/// scenario naturally concerns multiple rules (with a reason in the allowlist), or
/// (b) DUPLICATION — two rules report the same fact (one must be REMOVED/layered).
/// Empty bodies (`vec![]`, those using the base feed as-is) are excluded —
/// the base already produces ~15 rules, all legitimately sharing the same empty body.
#[test]
fn shared_fixture_bodies_match_ledger() {
    let fxs = fixtures();
    let mut by_body: std::collections::HashMap<String, Vec<&str>> =
        std::collections::HashMap::new();
    for f in &fxs {
        // Empty body = natural base-feed output (base already produces ~15 rules, all
        // sharing the same empty body) — skip.
        if f.overrides.is_empty() && f.removes.is_empty() && f.raw.is_empty() {
            continue;
        }
        let key = format!("{:?}|{:?}|{:?}", f.overrides, f.removes, f.raw);
        by_body.entry(key).or_default().push(f.rule);
    }

    let mut pairs: Vec<String> = Vec::new();
    for rules in by_body.values() {
        if rules.len() < 2 {
            continue;
        }
        let mut sorted = rules.clone();
        sorted.sort_unstable();
        for i in 0..sorted.len() {
            for j in (i + 1)..sorted.len() {
                pairs.push(format!("{} {}", sorted[i], sorted[j]));
            }
        }
    }
    pairs.sort_unstable();
    pairs.dedup();

    let ledger_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("shared_fixture_ledger.txt");
    let ledger_raw = std::fs::read_to_string(&ledger_path).unwrap_or_default();
    let ledger: Vec<String> = ledger_raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();

    if pairs != ledger {
        if std::env::var("UPDATE_LEDGER").is_ok() {
            let header = "# Rule pairs sharing the same fixture body (G2 duplicate early warning).\n\
                          # The common signature of today's two escapes: STM_023/STM_036 and\n\
                          # OPR_006/STM_033 had an IDENTICAL fixture in emit_proof and the\n\
                          # coverage-only test was green.\n\
                          # NEW row = either legitimate (one scenario naturally concerns several rules)\n\
                          # or DUPLICATION (same fact twice). If unsure, remove/layer it; otherwise adjudicate.\n";
            std::fs::write(&ledger_path, format!("{header}{}\n", pairs.join("\n"))).unwrap();
            return;
        }
        let added: Vec<&String> = pairs.iter().filter(|p| !ledger.contains(p)).collect();
        let removed: Vec<&String> = ledger.iter().filter(|l| !pairs.contains(l)).collect();
        panic!(
            "shared_fixture_ledger is stale ({} computed, {} in ledger).\n\
             NEW same-body pair (possible DUPLICATION — adjudicate it!): {:#?}\n\
             Extra in ledger (fixture changed/removed): {:#?}\n\
             To update: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof shared_fixture_bodies_match_ledger",
            pairs.len(), ledger.len(), added, removed,
        );
    }
}

// FLG_002 regression: if network_id is defined by the routes.txt.network_id column instead of
// networks.txt (GTFS Fares v2, second valid source), FLG_002 MUST NOT produce a false positive.
#[test]
fn flg_002_network_id_from_routes_txt_is_not_a_false_positive() {
    let routes = "route_id,agency_id,route_short_name,route_type,network_id\nR1,1,101,3,TRAM\n";
    let flr = "leg_group_id,network_id\nLG1,TRAM\n";
    let files = with_opts(
        &[("routes.txt", routes), ("fare_leg_rules.txt", flr)],
        &[],
        &[],
    );
    let emitted = emitted_rules(&files, &ValidatorConfig::default());
    assert!(
        !emitted.contains("FLG_002"),
        "FLG_002 must not fire for TRAM defined by routes.txt.network_id, emitted: {:?}",
        emitted,
    );
}

// FLG_002 regression: if network_id is defined by a route_networks.txt row (GTFS Fares v2,
// third valid source), FLG_002 MUST NOT produce a false positive.
#[test]
fn flg_002_network_id_from_route_networks_txt_is_not_a_false_positive() {
    let route_networks = "network_id,route_id\nTRAM,R1\n";
    let flr = "leg_group_id,network_id\nLG1,TRAM\n";
    let files = with_opts(
        &[
            ("route_networks.txt", route_networks),
            ("fare_leg_rules.txt", flr),
        ],
        &[],
        &[],
    );
    let emitted = emitted_rules(&files, &ValidatorConfig::default());
    assert!(
        !emitted.contains("FLG_002"),
        "FLG_002 must not fire for TRAM defined by route_networks.txt, emitted: {:?}",
        emitted,
    );
}

// FLG_002 positive control: it must still trigger when network_id is absent from every source.
#[test]
fn flg_002_still_fires_for_truly_undefined_network_id() {
    let flr = "leg_group_id,network_id\nLG1,NOPE\n";
    let files = with_opts(&[("fare_leg_rules.txt", flr)], &[], &[]);
    let emitted = emitted_rules(&files, &ValidatorConfig::default());
    assert!(
        emitted.contains("FLG_002"),
        "undefined network_id must produce FLG_002, emitted: {:?}",
        emitted
    );
}

// #105 regression: different fare_media_id variants of the same fare_product_id
// are independently valid records for the composite PK.
#[test]
fn fpd_distinct_fare_media_variants_are_not_duplicate_fpd_001() {
    let files = with_opts(
        &[
            ("fare_media.txt", "fare_media_id,fare_media_name,fare_media_type\ncash,Cash,0\ncontactless,Contactless,3\n"),
            ("fare_products.txt", "fare_product_id,amount,currency,fare_media_id\nP1,2.00,USD,cash\nP1,2.00,USD,contactless\n"),
        ],
        &[],
        &[],
    );
    let emitted = emitted_rules(&files, &ValidatorConfig::default());
    assert!(
        !emitted.contains("FPD_001"),
        "different fare_media_id variants must not produce FPD_001, emitted: {:?}",
        emitted,
    );
}

// #109 regression: raw after-midnight notation is a Spec finding, while the normalized
// analytical representation must not create a duplicate STM_008 finding.
#[test]
fn stm_048_reports_raw_rollover_as_spec_without_stm_008_duplicate() {
    let files = with_opts(
        &[
            (
                "stop_times.txt",
                "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,23:50:00,S1,1\nT1,00:10:00,00:10:00,S2,2\n",
            ),
        ],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    let stm048: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_048").collect();
    assert_eq!(
        stm048.len(),
        1,
        "raw rollover must be one feed summary: {stm048:?}"
    );
    assert_eq!(stm048[0].rule_class, gtfs_core::RuleClass::Spec);
    assert_eq!(stm048[0].severity, gtfs_core::Severity::Yuksek);
    assert_eq!(stm048[0].entity_type, gtfs_core::EntityType::Trip);
    assert_eq!(stm048[0].entity_id.as_deref(), Some("T1"));
    assert_eq!(stm048[0].line, Some(3));
    assert_eq!(stm048[0].field.as_deref(), Some("arrival_time"));
    assert_eq!(stm048[0].observed_value.as_deref(), Some("00:10:00"));
    assert_eq!(stm048[0].expected_value.as_deref(), Some("24:10:00"));
    assert!(
        !notices.iter().any(|n| n.rule_id == "STM_008"),
        "the same event must not recur as STM_008 after normalization: {notices:?}"
    );
}

#[test]
fn stm_048_raw_detection_is_not_disabled_by_service_day_config() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,23:50:00,S1,1\nT1,00:10:00,00:10:00,S2,2\n",
        )],
        &[],
        &[],
    );
    let config = ValidatorConfig {
        service_day_start_hour: 0,
        ..ValidatorConfig::default()
    };
    let notices = notices_for(&files, &config);
    assert!(
        notices.iter().any(|n| n.rule_id == "STM_048"),
        "service_day_start_hour=0 must not suppress raw STM_048: {notices:?}"
    );
}

#[test]
fn stm_048_detects_rollover_after_the_normalization_threshold() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,23:50:00,S1,1\nT1,03:30:00,03:30:00,S2,2\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(
        notices.iter().any(|n| n.rule_id == "STM_048"),
        "03:30 raw rollover must produce STM_048: {notices:?}"
    );
}

#[test]
fn raw_same_row_departure_is_stm_049_spec_without_config_gate() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:59:00,00:02:00,S1,1\nT1,24:10:00,24:10:00,S2,2\n",
        )],
        &[],
        &[],
    );
    let config = ValidatorConfig {
        service_day_start_hour: 0,
        ..ValidatorConfig::default()
    };
    let notices = notices_for(&files, &config);
    let stm049: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_049").collect();
    assert_eq!(
        stm049.len(),
        1,
        "same-row raw departure must be one feed summary: {stm049:?}"
    );
    assert_eq!(stm049[0].rule_class, gtfs_core::RuleClass::Spec);
    assert_eq!(stm049[0].severity, gtfs_core::Severity::Yuksek);
    assert_eq!(stm049[0].entity_type, gtfs_core::EntityType::Trip);
    assert_eq!(stm049[0].entity_id.as_deref(), Some("T1"));
    assert_eq!(stm049[0].observed_value.as_deref(), Some("00:02:00"));
    assert_eq!(stm049[0].expected_value.as_deref(), Some("24:02:00"));
    assert!(
        !notices.iter().any(|n| n.rule_id == "STM_048"),
        "same-row departure must not be mixed into STM_048: {notices:?}"
    );
}

#[test]
fn stm_048_and_stm_049_are_separate_high_spec_findings() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
T1,23:50:00,23:50:00,S1,1\n\
T1,00:10:00,00:10:00,S2,2\n\
T2,23:59:00,00:02:00,S3,1\n\
T2,24:10:00,24:10:00,S4,2\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    let stm048: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_048").collect();
    let stm049: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_049").collect();

    assert_eq!(
        stm048.len(),
        1,
        "arrival rollover must have its own finding: {stm048:?}"
    );
    assert_eq!(
        stm049.len(),
        1,
        "same-row departure rollover must have its own finding: {stm049:?}"
    );
    assert_eq!(stm048[0].field.as_deref(), Some("arrival_time"));
    assert_eq!(stm049[0].field.as_deref(), Some("departure_time"));
    assert_eq!(stm048[0].severity, gtfs_core::Severity::Yuksek);
    assert_eq!(stm049[0].severity, gtfs_core::Severity::Yuksek);
    assert_eq!(stm048[0].rule_class, gtfs_core::RuleClass::Spec);
    assert_eq!(stm049[0].rule_class, gtfs_core::RuleClass::Spec);
}

#[test]
fn each_raw_midnight_wrap_is_its_own_trip_finding() {
    let files = with_opts(
        &[
            (
                "trips.txt",
                "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\nR1,SVC1,T3\nR1,SVC1,T4\n",
            ),
            (
                "stop_times.txt",
                "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n\
T1,23:50:00,23:50:00,S1,1\nT1,00:10:00,00:10:00,S2,2\n\
T2,23:51:00,23:51:00,S1,1\nT2,00:11:00,00:11:00,S2,2\n\
T3,23:52:00,23:52:00,S1,1\nT3,00:12:00,00:12:00,S2,2\n\
T4,23:53:00,23:53:00,S1,1\nT4,00:13:00,00:13:00,S2,2\n",
            ),
        ],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    let stm048: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_048").collect();
    assert_eq!(
        stm048.len(),
        4,
        "four raw events must yield four separate findings: {stm048:?}"
    );
    let trips: BTreeSet<_> = stm048
        .iter()
        .filter_map(|n| n.entity_id.as_deref())
        .collect();
    assert_eq!(trips, BTreeSet::from(["T1", "T2", "T3", "T4"]));
    assert!(stm048
        .iter()
        .all(|n| n.line.is_some() && n.details.is_some()));
}

#[test]
fn trip_starting_after_midnight_without_raw_wrap_is_silent() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,00:10:00,00:10:00,S1,1\nT1,00:20:00,00:20:00,S2,2\nT1,00:35:00,00:35:00,S1,3\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(
        !notices.iter().any(|n| n.rule_id == "STM_048"),
        "a trip starting at midnight must not produce STM_048"
    );
    assert!(
        !notices.iter().any(|n| n.rule_id == "STM_049"),
        "a trip starting at midnight must not produce STM_049"
    );
    assert!(
        !notices.iter().any(|n| n.rule_id == "STM_008"),
        "a monotonic trip must not produce STM_008"
    );
}

#[test]
fn valid_service_day_rollover_does_not_emit_stm_048() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,23:50:00,S1,1\nT1,24:10:00,24:10:00,S2,2\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(
        !notices.iter().any(|n| n.rule_id == "STM_048"),
        "a valid 24:xx rollover must not be flagged"
    );
    assert!(
        !notices.iter().any(|n| n.rule_id == "STM_008"),
        "a valid 24:xx rollover must not produce STM_008"
    );
}

#[test]
fn real_non_midnight_decrease_remains_stm_008_without_stm_048() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,07:55:00,07:55:00,S2,2\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(
        notices.iter().any(|n| n.rule_id == "STM_008"),
        "a real within-day backward step must produce STM_008"
    );
    assert!(
        !notices.iter().any(|n| n.rule_id == "STM_048"),
        "a within-day backward step must not be STM_048"
    );
}

#[test]
fn stm008_carries_previous_departure_across_one_untimed_stop() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,,,S2,2\nT1,07:55:00,07:55:00,S1,3\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    let stm008: Vec<_> = notices.iter().filter(|n| n.rule_id == "STM_008").collect();
    assert_eq!(
        stm008.len(),
        1,
        "an untimed intermediate stop must not hide a real backward step: {stm008:?}"
    );
    assert_eq!(
        stm008[0].line,
        Some(4),
        "the finding must point to the next timed row"
    );
    assert_eq!(
        stm008[0]
            .details
            .as_ref()
            .and_then(|d| d.get("seq_a"))
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(
        stm008[0]
            .details
            .as_ref()
            .and_then(|d| d.get("seq_b"))
            .map(String::as_str),
        Some("3")
    );
}

#[test]
fn stm008_carries_previous_departure_across_multiple_untimed_stops() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,,,S2,2\nT1,,,S1,3\nT1,,,S2,4\nT1,07:55:00,07:55:00,S1,5\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert_eq!(
        notices.iter().filter(|n| n.rule_id == "STM_008").count(),
        1,
        "multiple untimed intermediate stops must not hide a real backward step"
    );
}

#[test]
fn stm008_does_not_flag_monotonic_interpolation_across_untimed_stop() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,08:00:00,08:00:00,S1,1\nT1,,,S2,2\nT1,08:05:00,08:05:00,S1,3\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(
        !notices.iter().any(|n| n.rule_id == "STM_008"),
        "monotonic interpolation must not produce a false positive"
    );
}

#[test]
fn stm008_keeps_midnight_normalization_silent_across_untimed_stop() {
    let files = with_opts(
        &[(
            "stop_times.txt",
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\nT1,23:50:00,23:50:00,S1,1\nT1,,,S2,2\nT1,00:10:00,00:10:00,S1,3\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(
        notices.iter().any(|n| n.rule_id == "STM_048"),
        "raw rollover must remain STM_048"
    );
    assert!(
        !notices.iter().any(|n| n.rule_id == "STM_008"),
        "the normalized rollover must not multiply into STM_008"
    );
}

#[test]
fn coverage_debt_matches_ledger() {
    let canonical: BTreeSet<&str> = RULES.iter().map(|r| r.id).collect();
    let proven: BTreeSet<&str> = fixtures().iter().map(|f| f.rule).collect();
    let allow: BTreeSet<&str> = PROOF_ALLOWLIST.iter().copied().collect();

    let mut debt: Vec<&str> = canonical
        .iter()
        .copied()
        .filter(|id| !proven.contains(id) && !allow.contains(id))
        .collect();
    debt.sort_unstable();

    let ledger_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("coverage_debt.txt");
    let ledger_raw = std::fs::read_to_string(&ledger_path).unwrap_or_default();
    let ledger: Vec<&str> = ledger_raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();

    if debt != ledger {
        // Write the new ledger content (UPDATE_LEDGER=1) or only report the difference.
        if std::env::var("UPDATE_LEDGER").is_ok() {
            let header = format!(
                "# emit-proof coverage debt (#5 C). Canonical rules without a fixture.\n\
                 # Add a fixture to emit_proof.rs::fixtures() to reduce it. {} rules.\n",
                debt.len()
            );
            std::fs::write(&ledger_path, format!("{header}{}\n", debt.join("\n"))).unwrap();
            return;
        }
        let added: Vec<&&str> = debt.iter().filter(|d| !ledger.contains(d)).collect();
        let removed: Vec<&&str> = ledger.iter().filter(|l| !debt.contains(l)).collect();
        panic!(
            "coverage_debt ledger is stale ({} computed, {} in ledger).\n\
             Missing from ledger (fixture removed/new rule?): {:?}\n\
             Extra in ledger (fixture added → ledger should shrink): {:?}\n\
             To update: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof coverage_debt_matches_ledger",
            debt.len(), ledger.len(), added, removed,
        );
    }
}

// ── WP-2: field-level spec anchoring ──────────────────────────────────────────
// emit_proof asks "can the rule fire?", while spec_conformance.rs asks "is valid data silent?"
// This third gate asks: **does the rule measure a field that REALLY EXISTS in the spec?**
// It catches MISCLASSIFICATION. Before 2026-08-01, RCT_004 emitted `min_age`/`max_age`;
// these fields do not appear AT ALL in the 27 April 2026 Schedule Reference — an extension
// field was classified as `Spec` and fed the publication-score gate.
//
// The anchor comes from the rule's OWN OUTPUT (the notice's `file` + `field` fields), not
// a hand-maintained list: a hand-maintained list goes stale; a notice cannot go stale.

/// Files WITHOUT a field table in the spec — no anchor can be established, so they are skipped.
/// The list is deliberately SHORT: an unknown file not listed here fails the gate
/// (e.g. a `Spec`-authority rule anchored to a GTFS-JP file has the wrong class).
const NO_FIELD_TABLE: &[&str] = &[
    // GeoJSON; the spec describes members (type/properties/geometry/coordinates) in prose,
    // with no "Field Name" table. LOC_002/003/004/007/008/009/010 fall here.
    "locations.geojson",
];

/// Pseudo-values without a file name in the `file` field (feed-level context).
const PSEUDO_FILES: &[&str] = &["feed"];

#[test]
fn spec_rules_anchor_to_fields_that_exist_in_the_spec() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec-audit/spec_fields.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} could not be read: {e}", path.display()));
    let doc: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let table = doc["files"]
        .as_object()
        .expect("spec_fields.json: 'files' nesnesi yok");
    assert!(
        table.len() >= 25,
        "spec_fields.json is too small ({}) — regenerate it",
        table.len()
    );

    let mut failures: BTreeSet<String> = BTreeSet::new();

    for f in fixtures() {
        let cfg = f.config.clone().unwrap_or_default();
        let vr = match validate_bytes(
            &make_zip(&with_opts(&f.overrides, &f.removes, &f.raw)),
            &cfg,
            TODAY,
        ) {
            ValidateResult::Ok(vr) => vr,
            ValidateResult::Fatal(_) => continue,
        };
        for n in &vr.notices {
            if gtfs_rules::registry::authority_source(&n.rule_id)
                != gtfs_core::AuthoritySource::GtfsSpec
            {
                continue;
            }
            // No field means there is no field to anchor (file/feed-level finding).
            let (Some(file), Some(field)) = (n.file.as_deref(), n.field.as_deref()) else {
                continue;
            };
            // Split multi-file labels such as "a.txt/b.txt".
            let parts: Vec<&str> = file.split('/').map(str::trim).collect();
            let known: Vec<&serde_json::Value> =
                parts.iter().filter_map(|p| table.get(*p)).collect();

            if known.is_empty() {
                if parts
                    .iter()
                    .all(|p| NO_FIELD_TABLE.contains(p) || PSEUDO_FILES.contains(p))
                {
                    continue;
                }
                failures.insert(format!(
                    "{}: '{}' spec dosya tablosunda YOK (GtfsSpec otoritesi bu dosyaya \
                     cannot be anchored — the class may be wrong)",
                    n.rule_id, file,
                ));
                continue;
            }

            // Composite field labels ("stop_lat|stop_lon") are checked separately; the field
            // must be defined in AT LEAST ONE of the files named by the label.
            for one in field
                .split(['|', ','])
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                let ok = known.iter().any(|entry| {
                    entry["fields"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|f| f["name"].as_str() == Some(one))
                });
                if !ok {
                    failures.insert(format!(
                        "{}: field '{}' is NOT in the spec for '{}'",
                        n.rule_id, one, file,
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "GtfsSpec-authority rule(s) are anchored to a field absent from the spec — the class \
         should be Quality/Interop or the field name is wrong:\n{}\n\n\
         (Spec list: spec-audit/spec_fields.json — regenerate with \
         `python3 spec-audit/extract_fields.py`)",
        failures
            .iter()
            .map(|s| format!("  {s}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

// ── WP-3: provision coverage (A = missing provision · B = partial coverage) ──
// WP-1 and WP-2 are CERTAINTY gates: "we are not reporting errors in the wrong place."
// This third one asks about COVERAGE: "the spec has a normative provision; do we have a rule
// that measures it?"
//
// NOT A GATE, A LEDGER: a coverage gap is not an "error" today (a missing rule cannot
// wrongly REJECT anyone); but it MUST NOT GROW SILENTLY. `spec_coverage_ledger.txt` records
// the gaps; when the spec adds a field or a rule is removed, the ledger changes and the test fails.
//
// ══ THREE MEASUREMENT LIMITS — CARRY THESE ALONG WHEN REPORTING NUMBERS ══════
// 1. IT IS A LOWER BOUND. The question is not "is there a rule?" but "is a Spec notice for
//    this file+field anchored in the emit_proof corpus?" A multi-field rule (CAL_002 checks
//    seven days, while the fixture breaks only `monday`) and rules emitting `field=None`
//    look like gaps. The correct wording is "32 fields have normative provisions but no Spec
//    anchor appears in the corpus" — NOT "we have 32 missing rules."
// 2. PROVISION EXTRACTION IS CRUDE. It is derived only from Presence/Type/PK columns in the
//    field table. OUT OF SCOPE: prose conditions ("usable only when B=1", "one of two is
//    required", "A and B must be provided together", "usable if this file is absent"), cross-file
//    conditions, per-trip consistency, and GeoJSON structure requirements. Thus an area with
//    "192 provisions" is NOT the GTFS Spec's TOTAL number of normative provisions.
// 3. `[only: …]` DOES NOT MEAN THE PROVISION IS SATISFIED. It only says "this rule emitted a
//    notice for this field." Example: `route_text_color` → RTS_007 (hex format) + RTS_008
//    (contrast). RTS_008 is a legitimate Quality rule but does not satisfy the spec's Color
//    type provision. The row asks about CLASSIFICATION, not coverage.

/// Number of normative provisions carried by a field (0 means the spec imposes nothing on it).
/// Derived from the `presence` + `type` columns; both are the spec's OWN vocabulary.
/// ⚠️ Limit 2 above: prose/cross-file provisions do not come from here.
fn provision_atoms(ftype: &str, presence: &str) -> Vec<&'static str> {
    let mut atoms = Vec::new();
    match presence {
        // "Optional"/"Recommended" imposes nothing → not a provision (STM_040 lesson).
        "Required" => atoms.push("presence:required"),
        "Conditionally Required" => atoms.push("presence:conditional"),
        "Conditionally Forbidden" => atoms.push("presence:conditional-forbidden"),
        _ => {}
    }
    if ftype.starts_with("Foreign ID") {
        atoms.push("foreign-key");
    }
    if ftype == "Unique ID" {
        atoms.push("unique");
    }
    match ftype {
        "Enum" => atoms.push("enum-domain"),
        // `Phone number` is INTENTIONALLY ABSENT: the spec's type definition is exactly
        // "A phone number." — it imposes no grammar, so there is no CHECKABLE provision.
        // Including it here created an untracked debt (agency_phone showed AGN_007 as "missing",
        // although AGN_007 is correctly Quality). A strict phone check rejects valid
        // international formats.
        "Time" | "Local time" | "Date" | "Color" | "URL" | "Email" | "Language code"
        | "Timezone" | "Currency code" | "Currency amount" | "Latitude" | "Longitude" => {
            atoms.push("format")
        }
        "Non-negative integer"
        | "Positive integer"
        | "Non-negative float"
        | "Positive float"
        | "Non-null integer"
        | "Non-zero integer" => atoms.push("range"),
        // Unqualified numeric types impose no RANGE but do impose PARSEABILITY:
        // "abc" is not a Float. This is a separate atom because `range` describes range
        // `Text`, `ID`, and `Text or URL or …` are INTENTIONALLY ABSENT: the spec calls
        // them any UTF-8 string, so there is no checkable provision (same lesson as `Phone number`).
        "Float" | "Integer" => atoms.push("numeric"),
        _ => {}
    }
    atoms
}

/// Per field: which provisions exist · which `GtfsSpec` rules are anchored · which OTHER-CLASS
/// rules are anchored.
///
/// The third set is critical: AGN_007 (Quality) measures `agency_phone`; no Spec rule does.
/// A Spec-only ledger shows it as "unchecked" and tempts the reader to rewrite an EXISTING rule.
/// Measured: 11 of 32 ledger rows are in this situation.
type CoverageRow = (
    String,
    String,
    Vec<&'static str>,
    BTreeSet<String>,
    BTreeSet<String>,
);

fn coverage_rows() -> Vec<CoverageRow> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec-audit/spec_fields.json");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let table = doc["files"].as_object().unwrap();

    // (file, field) → anchored rules; Spec and non-Spec rules are separate.
    type Anchors = std::collections::HashMap<(String, String), BTreeSet<String>>;
    let (mut spec, mut other): (Anchors, Anchors) = Default::default();
    for f in fixtures() {
        let cfg = f.config.clone().unwrap_or_default();
        let vr = match validate_bytes(
            &make_zip(&with_opts(&f.overrides, &f.removes, &f.raw)),
            &cfg,
            TODAY,
        ) {
            ValidateResult::Ok(vr) => vr,
            ValidateResult::Fatal(_) => continue,
        };
        for n in &vr.notices {
            let (Some(file), Some(field)) = (n.file.as_deref(), n.field.as_deref()) else {
                continue;
            };
            let bucket = if gtfs_rules::registry::authority_source(&n.rule_id)
                == gtfs_core::AuthoritySource::GtfsSpec
            {
                &mut spec
            } else {
                &mut other
            };
            for p in file
                .split('/')
                .map(str::trim)
                .filter(|p| table.contains_key(*p))
            {
                for one in field
                    .split(['|', ','])
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    bucket
                        .entry((p.to_string(), one.to_string()))
                        .or_default()
                        .insert(n.rule_id.clone());
                }
            }
        }
    }

    let mut rows = Vec::new();
    for (file, entry) in table {
        for f in entry["fields"].as_array().unwrap() {
            let name = f["name"].as_str().unwrap().to_string();
            let atoms = provision_atoms(
                f["type"].as_str().unwrap_or(""),
                f["presence"].as_str().unwrap_or(""),
            );
            let key = (file.clone(), name.clone());
            rows.push((
                file.clone(),
                name,
                atoms,
                spec.get(&key).cloned().unwrap_or_default(),
                other.get(&key).cloned().unwrap_or_default(),
            ));
        }
    }
    rows.sort();
    rows
}

/// Does the field_name literal occur in the CHECK stages (k2–k7)?
///
/// `k1_parse.rs` is INTENTIONALLY EXCLUDED: `known_columns` counts every official spec
/// column there as a literal (for ARC_017 "unknown column"). Including it would show every field
/// as "present in code" and distinguish nothing — measured: 2 certain gaps without exclusion,
/// 5 with exclusion (`booking_url`/`info_url`/`phone_number` occur ONLY in column records).
///
/// If it does not occur, no check examines that field → the gap is CERTAIN. If it occurs, only
/// "possibly": the literal may belong to another file (`from_stop_id` exists in
/// transfers/pathways, not fare_leg_join_rules). The metric has HIGH PRECISION / low recall.
fn identifiers_in_check_stages() -> BTreeSet<String> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs")
                && p.file_name().is_some_and(|n| n != "k1_parse.rs")
            {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );

    let mut ids = BTreeSet::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        for lit in text.split('"').skip(1).step_by(2) {
            if !lit.is_empty()
                && lit.len() <= 40
                && lit
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'_' || b.is_ascii_digit())
            {
                ids.insert(lit.to_string());
            }
        }
    }
    ids
}

/// Category A — fields with normative provisions but no `GtfsSpec` notice anchored in the
/// emit_proof corpus. It must match the ledger: the ledger shrinks when a gap closes and shows
/// a difference when the spec grows.
///
/// ⚠️ **LOWER BOUND — DOES NOT MEAN "NO RULE".** A rule may measure the field while its
/// fixture breaks another field (CAL_002 also checks seven days, while the fixture breaks only
/// `monday`), or it may emit its notice with `field=None` (RTS_003, "one of two required"),
/// causing the row to appear here. That is why the ledger has a PROOF column:
/// `[kaynakta-yok]` = the field name never occurs in the production source → gap CERTAIN.
/// `[yalnız: …]` = a rule measures the field but is not Spec class (e.g. `agency_phone` → AGN_007,
/// Quality); first ask "is the provision really a Spec matter?" before adding a rule. An
/// unsupported row needs ADJUDICATION.
/// **SEARCH FOR AN EXISTING RULE BEFORE ADDING ONE** (recurring error class).
#[test]
fn spec_coverage_gaps_match_ledger() {
    let ids = identifiers_in_check_stages();
    let gaps: Vec<String> = coverage_rows()
        .into_iter()
        .filter(|(_, _, atoms, spec, _)| !atoms.is_empty() && spec.is_empty())
        .map(|(file, field, atoms, _, other)| {
            // Evidence order: from most certain to least specific.
            let proof = if !other.is_empty() {
                // A rule DOES measure the field, but is not Spec class → before adding a rule,
                // decide whether the provision is really a Spec matter or the existing rule suffices.
                format!(
                    "  [yalnız: {}]",
                    other.iter().cloned().collect::<Vec<_>>().join(",")
                )
            } else if !ids.contains(&field) {
                "  [denetim-yok]".to_string()
            } else {
                String::new()
            };
            format!("{file}:{field}  {}{proof}", atoms.join(","))
        })
        .collect();

    let ledger_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("spec_coverage_ledger.txt");
    let ledger_raw = std::fs::read_to_string(&ledger_path).unwrap_or_default();
    let ledger: Vec<String> = ledger_raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();

    if gaps != ledger {
        if std::env::var("UPDATE_LEDGER").is_ok() {
            let header = format!(
                "# WP-3/A — fields with normative provisions in the spec but no\n\
                 # GtfsSpec notice not ANCHORED in the emit_proof corpus. Format: file:field  provisions [proof].\n\
                 # {} fields.\n\
                 #\n\
                 # This is a DEBT ledger, not a gate: a missing rule does not wrongly REJECT anyone,\n\
                 # but the gap MUST NOT GROW silently. The row disappears when a rule is added.\n\
                 #\n\
                 # ⚠️ LOWER BOUND — this does NOT mean \"there is no rule\". A provision may be measured\n\
                 # yet still appear here if the fixture triggers only one branch, or if\n\
                 # the rule emits `field=None` and therefore has no anchor. Both happened on\n\
                 # 2026-08-02 and BOTH WERE FIXED (see below), but the pattern may recur.\n\
                 # PRECISE WORDING: \"these fields have normative provisions, but no Spec notice\n\
                 # anchor for them appears in the emit_proof corpus\" — NOT \"we have N missing rules\".\n\
                 #\n\
                 # ⚠️ PROVISION EXTRACTION IS COARSE: it is derived only from the field table's\n\
                 # Presence/Type/PK columns. Prose conditions (\"only when B=1\", \"one of two is\n\
                 # required\", \"A and B together\"), cross-file conditions, trip-level consistency,\n\
                 # and GeoJSON internal-structure requirements are OUT OF SCOPE. This ledger does not\n\
                 # measure ALL normative provisions, only the portion derivable from the field table.\n\
                 #   [no-check] = the field name never occurs in k2–k7 checking stages → gap CERTAIN\n\
                 #               (k1_parse.rs is excluded: known_columns already counts every column).\n\
                 #   [only: …]  = a rule measures the field, but is NOT Spec class. Not a gap;\n\
                 #               decide whether the provision is a Spec matter and whether the rule is enough.\n\
                 #                   ⚠️ This marker does NOT mean \"provision SATISFIED\" — only that\n\
                 #                   \"this rule emitted a notice for this field\". E.g. route_text_color → RTS_007\n\
                 #                   (hex format) + RTS_008 (contrast); RTS_008 is valid Quality,\n\
                 #                   but does not satisfy the spec's Color-type provision.\n\
                 #   unsupported row = ADJUDICATION required; SEARCH FOR AN EXISTING RULE FIRST.\n\
                 #\n\
                 # ── STATUS (2026-08-02): LEDGER EMPTY — every measurable provision is anchored ────\n\
                 # The ledger opened with 32 rows; 9 CLOSED (RTS_007 moved to Spec;\n\
                 # AGN_012/RTS_024/TRN_008 class fixed; the `Phone number` provision was invented,\n\
                 # atom removed → 3 rows dropped; attributions route_id/trip_id issue #62\n\
                 # split into ATR_011/ATR_012 → 2 rows dropped). Every remaining row was reviewed:\n\
                 #   • booking_rules booking_url/info_url .. ✅ CLOSED (BKR_020/021)\n\
                 #   • fare_leg_join_rules (4 fields) ...... ✅ CLOSED (FLJ_001..004)\n\
                 #   • issue #61 ........................... FULLY CLOSED (9 rows):\n\
                 #     STM_058 · DQ_021 expanded (attribution_id + route_networks.route_id)\n\
                 #     PTH_027 · STP_042 · NET_002 · NET_003 · RCT_007 · RTS_029 · TRN_015\n\
                 #   • calendar 6 days (enum) ................ ✅ CLOSED: confirmed as an artifact\n\
                 #     and FIXED — the fixture breaks all seven days with seven SEPARATE services\n\
                 #     (one row is insufficient: CAL_002 is Entity-deduped, one notice per service).\n\
                 #   • routes route_short_name/route_long_name ✅ CLOSED: RTS_003 already measured\n\
                 #     them but emitted `field=None`. It now names both fields\n\
                 #     (`route_short_name|route_long_name`) — the repository's established convention.\n\
                 #     The reason was NOT this ledger: R2's \"Field\" column was empty in a finding\n\
                 #     that blocks a feed from publication.\n\
                 #\n\
                 # ⚠️ AN EMPTY LEDGER DOES NOT MEAN \"THE ENTIRE SPEC IS MEASURED\" — it applies only\n\
                 # to provisions derivable from the FIELD TABLE (see the scope warning above).\n\
                 # If a NEW row appears, the spec changed, a rule was removed, or a fixture weakened\n\
                 # — the test fails and the row requires adjudication.\n\
                 #\n\
                 # Regenerate: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof \\\n\
                 #   spec_coverage_gaps_match_ledger\n",
                gaps.len());
            std::fs::write(&ledger_path, format!("{header}{}\n", gaps.join("\n"))).unwrap();
            return;
        }
        let added: Vec<&String> = gaps.iter().filter(|g| !ledger.contains(g)).collect();
        let removed: Vec<&String> = ledger.iter().filter(|l| !gaps.contains(l)).collect();
        panic!(
            "spec coverage ledger is stale ({} computed, {} in ledger).\n\
             NEW gap (did the spec add a field, or was a rule removed?): {:#?}\n\
             Extra in ledger (gap CLOSED → shrink the ledger): {:#?}\n\
             To fix: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof \
             spec_coverage_gaps_match_ledger",
            gaps.len(),
            ledger.len(),
            added,
            removed,
        );
    }
}

/// Category B — REPORT (not a gate): fields whose provision count exceeds the number of anchored rules.
///
/// ⚠️ This is a SIGNAL, not a decision. One rule can measure several provisions at once
/// (e.g. "monday invalid" can cover both presence and the enum domain), so most
/// "2 provisions, 1 rule" rows are legitimate. I tried to infer the measured provision from
/// rule titles: only 48% of titles can be classified (252 of 487 match no keyword) — putting
/// a classifier with this reliability in the repository would be another form of the
/// "stale hand-maintained list" we are trying to avoid. The list is therefore a queue for
/// HUMAN adjudication.
///
/// ## ADJUDICATION (2026-08-02) — 62 rows reduced to 13
/// First, the measurement was corrected: the `presence:required` atom is no longer counted
/// when the field is in **ARC_025's required-column list**. ARC_025 emits the missing column
/// as `field=<column>` (proved by the fixture through `stops.txt:stop_id`); the mechanism
/// is one loop and the list is locked to the spec by `required_columns_match_the_specification`.
/// This removed 46 rows. Then the `STM_058` fixture was strengthened to break both window
/// fields (−1).
///
/// **Triage of the remaining 13 — three classes:**
/// 1. **MEASUREMENT BLINDNESS, NO GAP** — one rule covers two atoms, while the measurement
///    counts RULES per field rather than matching atoms: `FLJ_003`/`FLJ_004` (they cover
///    the reciprocal condition + FK together, written by me), and the `STM_058`↔`STM_039`
///    pair (format + presence).
/// 2. **ANOTHER RULE COVERS IT but does not anchor it** — `RTS_028` (Interop) measures the
///    conditional-forbidden provision of `routes.continuous_pickup/drop_off`; `STP_043`
///    covers `stops.stop_access`. Neither anchors that field in its fixture.
/// 3. **SPEC TEXT READ (2026-08-02) — 2 of 7 candidates were CLOSED:**
///    - `timeframes.start_time` ✅ — *"Required if end_time is defined. Forbidden otherwise."*
///      `TFR_007` measures exactly this ("only one of start_time and end_time is defined").
///    - `trips.shape_id` ✅ — *"Required if the trip has a continuous pickup or drop-off
///      behavior."* `TRP_019` measures exactly this.
///
///    **Two candidates closed in this round:**
///    - `fare_attributes.agency_id` — *"Required if multiple agencies are defined."* is now
///      measured by `AGN_011`; `FAR_008` continues to measure the FK.
///    - `stop_times.departure_time` — has two sub-provisions: FORBIDDEN with a Flex window
///      (`STM_037` ✅) and *"Required for timepoint=1"* (`STM_047` ✅). ⚠️ A lasting
///      nuance was confirmed: the spec requires the first/last stop only for `arrival_time`.
///
///    **Three REAL GAP candidates remain:**
///    - `booking_rules.prior_notice_duration_max` — the `numeric` atom is not checked:
///      `opt_int()` uses `.parse::<i64>().ok()`, so non-numeric values are silently dropped.
///    - `booking_rules.prior_notice_start_day` — the same silent numeric-parse gap.
///    - `stop_times.location_group_id` — the conditional-forbidden provision is unchecked
///      (`XFL_024` is an FK).
///      **These three are part of a SYSTEMIC pattern; see below.**
///
/// ## 🔁 SYSTEMIC FINDING — silent parse swallowing (12 points)
/// The `Err(_) => None` / `.parse().ok()` pattern occurs in k2 at **12 points**, and 11 have
/// a rule for the field RIGHT BESIDE them: `location_type`→STP_008 ·
/// `wheelchair_boarding`→STP_013 · `stop_access`→STP_024 · `timepoint`→STM_022 ·
/// `shape_dist_traveled`→STM_030/SHP_021 · `cemv_support`→AGN_012/RTS_024 ·
/// `wheelchair_accessible`→TRP_006 · `bikes_allowed`→TRP_007 · `exact_times`→FRQ_007 ·
/// four booking_rules fields→`opt_int`.
/// **Result: OUT-OF-RANGE numbers are caught, NON-NUMERIC values are silently dropped.** This
/// pattern was fixed five times individually in this session (STM_058 · PTH_027 · RTS_029 ·
/// TRP_034 · ARC_031); all 12 should be fixed — in most cases reusing the adjacent rule is enough
/// (PTH_027 precedent).
/// REVERSE DIRECTION — the mirror image of the coverage ledger.
///
/// The ledger asks "is every provision measured?"; this report asks "does every Spec claim rest
/// on a provision?" The second question seeks a MORE EXPENSIVE error: a coverage gap gives the
/// user incomplete information, while a false Spec claim KEEPS A VALID FEED FROM PUBLICATION
/// (the R1 gate is pure `Spec ∧ Kritik`).
///
/// Metric: is a `Spec`-class rule anchored to a field for which the spec produces NO
/// provision atom? If the field is `Optional`/`Recommended` and its type imposes nothing,
/// claiming "the spec says so" is unsupported — that was exactly STM_040 (Spec class on an
/// Optional field, moved to Quality).
///
/// NOT A GATE, A REPORT: legitimate examples may exist (the measured provision is defined in
/// prose rather than the field table). Every row needs human adjudication.
/// SHARED BLIND SPOT OF BOTH LEDGERS: rules emitting `field: None`.
///
/// Both the coverage and claim ledgers anchor on `(file, field)`. If a rule omits `field`,
/// it appears in NEITHER: it does not say "this provision is measured" and is not checked as an
/// "unsupported claim." `RTS_003` appeared as a "gap" in the ledger for exactly this reason.
///
/// **This ledger covers only Entity/Row level.** File/feed-level findings (`DedupLevel::
/// Feed`/`File`) tek bir alan yoktur ve `None` is correct — they are counted but not listed.
///
/// ## ADJUDICATION (2026-08-02) — two classes
/// **(A) DERIVED FACT → `None` CORRECT.** The finding comes from a relation/statistic across
/// rows, not one field's value: a "stop unused by any trip" (STP_020) does not point to a wrong
/// field in `stops.txt`; it comes from the relation with `stop_times`. Naming a field
/// would send the user to the wrong place. MOST rows in this ledger are of this kind:
/// analytics (OPR_*, CAL_007/010/012/021, CLD_007, VAT_008, PTH_012/013, STP_020), and
/// graph/statistics (RTS_016, STP_030, TRP_013, not the opposite of ATR_003).
///
/// **(B) ROW SHAPE → `None` CORRECT.** `ARC_012` (column count does not match the header)
/// and `ARC_018` (empty data row) concern the row itself, not any field.
///
/// **(C) CROSS-FILE → `None` REQUIRED.** `RCT_006` is violated at `rider_categories.
/// is_default_fare_category`'dedir ama notice `fare_products.txt` is shown. Naming a field
/// absent from that file would correctly fail the
/// `spec_rules_anchor_to_fields_that_exist_in_the_spec` gate. The correct fix would be to
/// change the notice's file — a separate decision.
///
/// ## FIXED ON 2026-08-02 (these violated specific fields; `None` was wrong)
/// `RTS_003` · `ATR_003` · `PTH_016` · `TRF_012` · `TRF_016` · `TRN_005` · `TRN_006` ·
/// `TRN_009` · `TRN_013` · `ATR_009` · `CAL_006` · `CAL_018` · `FRL_007` · `GEO_020` · `PTH_011`
///
/// ## Boundary
/// Rules in `PROOF_ALLOWLIST` (those taking the fatal path) produce no notices and are therefore
/// INVISIBLE here — a separate blind spot and separate work.
#[test]
fn field_none_emitters_match_ledger() {
    use gtfs_core::DedupLevel;
    let mut with_field: BTreeSet<String> = BTreeSet::new();
    let mut without_field: BTreeSet<String> = BTreeSet::new();
    for f in fixtures() {
        let cfg = f.config.clone().unwrap_or_default();
        let vr = match validate_bytes(
            &make_zip(&with_opts(&f.overrides, &f.removes, &f.raw)),
            &cfg,
            TODAY,
        ) {
            ValidateResult::Ok(vr) => vr,
            ValidateResult::Fatal(_) => continue,
        };
        for n in &vr.notices {
            if n.field.is_some() {
                with_field.insert(n.rule_id.clone());
            } else {
                without_field.insert(n.rule_id.clone());
            }
        }
    }
    let mut found: Vec<String> = Vec::new();
    for id in without_field.iter().filter(|r| !with_field.contains(*r)) {
        let Some(m) = gtfs_rules::registry::get_rule(id) else {
            continue;
        };
        if matches!(m.dedup_level, DedupLevel::Feed | DedupLevel::File) {
            continue;
        }
        found.push(format!("{id}  {:?}", m.rule_class));
    }
    found.sort_unstable();

    let ledger_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("field_none_ledger.txt");
    let ledger_raw = std::fs::read_to_string(&ledger_path).unwrap_or_default();
    let ledger: Vec<String> = ledger_raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();

    if found != ledger {
        if std::env::var("UPDATE_LEDGER").is_ok() {
            let header = format!(
                "# Rules at Entity/Row level that ALWAYS emit `field: None`. {} rules.\n\
                 # Format: RULE_ID  class\n\
                 #\n\
                 # The coverage and claim ledgers anchor on `(file, field)`; these\n\
                 # rules appear in NEITHER. File/feed-level findings (DedupLevel Feed/\n\
                 # File) is NOT COUNTED here — there is no single field, so None is correct.\n\
                 #\n\
                 # ⚠️ A ROW IS NOT AUTOMATICALLY AN ERROR. Adjudication is in the test doc comment; three legitimate\n\
                 # classes exist: (A) derived fact — the finding comes from a relation across rows, so a\n\
                 # field name would mislead the user; (B) row shape (ARC_012/018);\n\
                 # (C) cross-file — the field is in another file, so anchoring fails (RCT_006).\n\
                 #\n\
                 # If a NEW row appears: does the rule really measure a derived fact, or does it\n\
                 # describe violations of specific fields? If the latter, use the PIPE CONVENTION\n\
                 # (`a|b`); do not leave None — R2's \"Field\" column would be empty.\n\
                 #\n\
                 # Regenerate: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof \\\n\
                 #   field_none_emitters_match_ledger\n",
                found.len());
            std::fs::write(&ledger_path, format!("{header}{}\n", found.join("\n"))).unwrap();
            return;
        }
        let added: Vec<&String> = found.iter().filter(|f| !ledger.contains(f)).collect();
        let removed: Vec<&String> = ledger.iter().filter(|l| !found.contains(l)).collect();
        panic!(
            "field=None defteri güncel değil ({} hesaplanan, {} defter).\n\
             YENİ alansız kural (türetilmiş olgu mu, yoksa boru konvansiyonu mu gerekiyor?): {:#?}\n\
             Defterde fazla (alan yazmaya başladı): {:#?}\n\
             Düzeltmek için: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof \
             field_none_emitters_match_ledger",
            found.len(), ledger.len(), added, removed,
        );
    }
}

#[test]
fn spec_claims_without_a_provision_match_ledger() {
    let rows = coverage_rows();
    let mut found: Vec<String> = rows
        .iter()
        .filter(|(_, _, atoms, spec_rules, _)| atoms.is_empty() && !spec_rules.is_empty())
        .map(|(file, field, _, spec_rules, _)| {
            let mut rs: Vec<String> = spec_rules.iter().cloned().collect();
            rs.sort();
            format!("{file}:{field}  {}", rs.join(","))
        })
        .collect();
    found.sort_unstable();

    let ledger_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("spec_claims_ledger.txt");
    let ledger_raw = std::fs::read_to_string(&ledger_path).unwrap_or_default();
    let ledger: Vec<String> = ledger_raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();

    if found != ledger {
        if std::env::var("UPDATE_LEDGER").is_ok() {
            let header = format!(
                "# REVERSE DIRECTION — the mirror image of the coverage ledger. {} rows.\n\
                 # Format: file:field  rule[,rule]\n\
                 #\n\
                 # The coverage ledger asks \"is every provision measured?\" This ledger asks the reverse:\n\
                 # \"does every Spec CLAIM rest on a provision?\" The second question seeks a MORE EXPENSIVE\n\
                 # error — a coverage gap gives the user incomplete information, while a false Spec claim\n\
                 # KEEPS A VALID FEED FROM PUBLICATION (the R1 gate is pure `Spec ∧ Kritik`).\n\
                 #\n\
                 # Is a `Spec`-class rule anchored to a field for which the field table produces no provision atom?\n\
                 # If an Optional/Recommended field with type Text/ID/Phone imposes nothing, saying\n\
                 # \"the spec says so\" there is unsupported. Precedent: STM_040 (Spec class on an\n\
                 # Optional field moved to Quality), PTH_017/PTH_028 (spec says \"should\" → advice branch moved to Quality).\n\
                 #\n\
                 # ⚠️ A ROW IS NOT ALWAYS AN ERROR: the provision may be defined in PROSE rather than\n\
                 # the field table. But every row MUST BE VERIFIED against the spec text; an unverified\n\
                 # rule must be Quality class.\n\
                 #\n\
                 # ⚠️ LOWER BOUND, in two ways: (1) rules emitting `field=None` are invisible;\n\
                 # (2) only the \"no atoms at all\" case is caught — if a field has ONE provision and\n\
                 # the rule imposes something ELSE, it remains invisible (the coverage ledger's atom blindness mirrored).\n\
                 #\n\
                 # Regenerate: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof \\\n\
                 #   spec_claims_without_a_provision_match_ledger\n",
                found.len());
            std::fs::write(&ledger_path, format!("{header}{}\n", found.join("\n"))).unwrap();
            return;
        }
        let added: Vec<&String> = found.iter().filter(|f| !ledger.contains(f)).collect();
        let removed: Vec<&String> = ledger.iter().filter(|l| !found.contains(l)).collect();
        panic!(
            "spec claim ledger is stale ({} computed, {} in ledger).\n\
             NEW unsupported Spec claim (VERIFY against spec text — otherwise it must be Quality): {:#?}\n\
             Extra in ledger (claim removed/class fixed): {:#?}\n\
             To fix: UPDATE_LEDGER=1 cargo test -p gtfs-pipeline --test emit_proof \
             spec_claims_without_a_provision_match_ledger",
            found.len(),
            ledger.len(),
            added,
            removed,
        );
    }
}

#[test]
#[ignore = "rapor: cargo test -p gtfs-pipeline --test emit_proof spec_partial -- --ignored --nocapture"]
fn spec_partial_coverage_report() {
    let rows = coverage_rows();
    let total: usize = rows.iter().map(|(_, _, a, _, _)| a.len()).sum();
    let (mut none, mut partial, mut full) = (0usize, 0usize, 0usize);
    println!("\n### B — provision count > anchored-rule count (SIGNAL, not a decision)");
    println!("The `presence:required` atom is excluded when the field is in ARC_025's required-column list:");
    println!("missing column via `field=<column>`; same loop as the list");
    println!("`required_columns_match_the_specification` ile spec'e kilitlidir.\n");
    for (file, field, atoms, rules, _) in &rows {
        if atoms.is_empty() {
            continue;
        }
        // Declared anchor source: ARC_025's required-column list.
        let declared_presence =
            gtfs_pipeline::k1_parse::required_fields(file).contains(&field.as_str());
        let atoms: Vec<&str> = atoms
            .iter()
            .copied()
            .filter(|a| !(*a == "presence:required" && declared_presence))
            .collect();
        if atoms.is_empty() {
            full += 1;
            continue;
        }
        if rules.is_empty() {
            none += 1;
        } else if rules.len() < atoms.len() {
            partial += 1;
            println!("  {file}:{field} — [{}] · {:?}", atoms.join(","), rules);
        } else {
            full += 1;
        }
    }
    println!(
        "\n### SUMMARY ({} fields with provisions · {total} provision atoms):",
        none + partial + full
    );
    println!("  NO Spec anchor ....... {none}   (ledger; NOT a \"missing rule\")");
    println!("  provision > anchors .. {partial}   (signal, not a decision)");
    println!("  provision <= anchors . {full}");
    println!("\n⚠️ These are NOT THE SPEC'S TOTAL NORMATIVE PROVISIONS; they come only from");
    println!("   from the field table's Presence/Type/PK columns. Out of scope:");
    println!("   prose conditions (\"only when B=1\", \"one of two is required\", \"A and B together\"),");
    println!("   cross-file conditions, trip-level consistency, and GeoJSON internal-structure requirements.");
}

/// REPORT (not a gate): anchoring is at FIELD level, not ATOM level.
///
/// `spec_coverage_gaps_match_ledger` considers a field "covered" when it sees AT LEAST ONE
/// Spec notice. But a field may have several provision atoms (`presence:required`, `format`,
/// and `foreign-key`) and one notice may anchor all of them. This report measures the SIZE of
/// that crudeness: how many atoms, how many fields, and how many fields have more atoms than
/// the number of distinct rules anchoring them.
///
/// An overfull field does not mean "unsupported" — one rule may legitimately measure several
/// atoms (`STP_003` checks both `presence` and `format`). This is a suspicion list, not debt.
#[test]
#[ignore = "rapor: cargo test -- --ignored anchor_granularity_report --nocapture"]
fn anchor_granularity_report() {
    let rows = coverage_rows();
    let (mut atoms, mut fields_with_atoms, mut suspect) = (0usize, 0usize, Vec::new());
    for (file, field, provs, spec_rules, other_rules) in &rows {
        if provs.is_empty() {
            continue;
        }
        atoms += provs.len();
        fields_with_atoms += 1;
        if provs.len() > spec_rules.len() {
            suspect.push((
                file.clone(),
                field.clone(),
                provs.clone(),
                spec_rules.clone(),
                other_rules.clone(),
            ));
        }
    }
    suspect.sort_by_key(|(f, n, p, s, _)| {
        (std::cmp::Reverse(p.len() - s.len()), f.clone(), n.clone())
    });
    println!("\n=== ANCHOR GRANULARITY ===");
    println!("provision atom        : {atoms}");
    println!("atomu olan alan       : {fields_with_atoms}");
    println!("atom > Spec rule      : {} fields", suspect.len());
    let gap: usize = suspect
        .iter()
        .map(|(_, _, p, s, _)| p.len() - s.len())
        .sum();
    println!("uncovered atom gap    : {gap}");
    println!("\n--- 20 largest gaps ---");
    for (file, field, provs, spec_rules, other) in suspect.iter() {
        let titles: Vec<String> = spec_rules
            .iter()
            .map(|r| {
                format!(
                    "{r}={}",
                    gtfs_rules::registry::get_rule(r)
                        .map(|x| x.title)
                        .unwrap_or("?")
                )
            })
            .collect();
        println!(
            "{file}:{field}\t{:?}\t{}\t{:?}",
            provs,
            titles.join(" | "),
            other
        );
    }
}

/// `FILE_LEVEL_PROVISIONS.md` is the manual adjudication of the spec's FILE-level conditional
/// provisions and was attached to no gate.
///
/// These provisions do not derive from the field table, so the coverage ledger cannot see them;
/// the prose catalogue also deliberately excludes `File: …` rows (`extract_provisions.py`,
/// because they are already adjudicated in this document). Thus seven provisions live ONLY there.
///
/// The document itself said: "This measurement is NOT automatic — nobody reminds us to do it."
/// This test invalidates that sentence: if the spec adds a conditional file or an existing one
/// becomes unconditional, the test stays red until the document is updated.
#[test]
fn file_level_provisions_doc_covers_every_conditional_file() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let doc: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("spec-audit/spec_fields.json")).unwrap(),
    )
    .unwrap();
    let md = std::fs::read_to_string(root.join("spec-audit/FILE_LEVEL_PROVISIONS.md")).unwrap();

    let mut missing = Vec::new();
    let mut conditional = 0usize;
    for (file, entry) in doc["files"].as_object().unwrap() {
        let presence = entry["presence"].as_str().unwrap_or("");
        if !presence.starts_with("Conditionally") {
            continue;
        }
        conditional += 1;
        if !md.contains(&format!("`{file}`")) {
            missing.push(format!("{file} ({presence})"));
        }
    }
    assert!(
        missing.is_empty(),
        "spec has {} conditional files not adjudicated in FILE_LEVEL_PROVISIONS.md:\n  {}\n\n\
         These provisions appear in no other ledger. Update the document with the condition text, \
         the rule that satisfies it, and the decision (the existing seven rows are examples).",
        missing.len(),
        missing.join("\n  ")
    );
    assert!(
        conditional >= 7,
        "spec_fields.json has only {conditional} conditional files; there were seven on 2026-08-02. \
         If a file became unconditional, its row in FILE_LEVEL_PROVISIONS.md must also be removed."
    );
}

/// The ledger's SUMMARY sections must not contradict the STATE MACHINE table.
///
/// `PROVISION_TRIAGE.md` exceeds 900 lines and its round sections carry decisions from each day.
/// It happened twice in one session: a gap was closed and a rule written, but the summary table
/// still said "OPEN" — and because `badge_status.py` reads the state machine, the percentage
/// and document disagreed. This test catches that drift.
#[test]
fn triage_ledger_has_no_stale_open_claims() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let md = std::fs::read_to_string(root.join("spec-audit/PROVISION_TRIAGE.md")).unwrap();

    // Candidates considered closed by the state machine.
    let start = md
        .find("## 📊 DURUM MAKİNESİ")
        .expect("STATE MACHINE section is missing");
    let end = md[start..]
        .find("**KISMİ adjudikasyonu")
        .map(|i| start + i)
        .unwrap_or(md.len());
    let closed: Vec<&str> = md[start..end]
        .lines()
        .filter(|l| l.starts_with('|') && l.contains('`'))
        .flat_map(|l| l.split('`').skip(1).step_by(2))
        .filter(|s| s.len() == 9 && s.starts_with('P'))
        .collect();
    assert!(
        closed.len() >= 12,
        "state machine is too short: {} rows",
        closed.len()
    );

    // A closed candidate must not occur in a line saying "OPEN" or "WILL NOT BE CLOSED".
    let mut stale = Vec::new();
    for line in md.lines() {
        if !line.contains("AÇIK") && !line.contains("KAPATILMAYACAK") {
            continue;
        }
        for id in &closed {
            if line.contains(&format!("`{id}`")) {
                stale.push(format!(
                    "{id}: {}",
                    line.trim().chars().take(90).collect::<String>()
                ));
            }
        }
    }
    assert!(
        stale.is_empty(),
        "{} candidates marked CLOSED in the state machine still appear OPEN in the ledger:\n  {}\n\n\
         Record the closure in one place and update the summary row too.",
        stale.len(),
        stale.join("\n  ")
    );
}

/// The ledger's HEADER COUNTS must agree with the catalogue and calculated percentage.
///
/// ⚠️ 2026-08-06 EXTERNAL AUDIT: `triage_ledger_has_no_stale_open_claims` checked only OPEN/CLOSED
/// labels. The auditor showed that the ledger header still said "273 candidates · 131/131"
/// while the lower section said "299 · 146/146" — the same file contradicted itself, and a gate
/// existed for this but did not cover the counts.
///
/// This test binds two things: (a) the ledger catalogue total to `spec_provisions.json`,
/// (b) no row counted in the numerator (PROVEN/INDIRECT) may carry "[Assumption]" — an untested
/// inference cannot count as a "measured" number.
#[test]
fn ledger_header_counts_match_catalogue() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let md = std::fs::read_to_string(root.join("spec-audit/PROVISION_TRIAGE.md")).unwrap();
    let json = std::fs::read_to_string(root.join("spec-audit/spec_provisions.json")).unwrap();

    // Count catalogue candidates: count `"id": "P…"` lines (dependency-free).
    let total = json.matches("\"id\":").count();
    assert!(total > 200, "catalogue could not be read ({total})");
    let head: String = md.lines().take(20).collect::<Vec<_>>().join("\n");
    assert!(
        head.contains(&format!("**{total} aday**")),
        "ledger header disagrees with the catalogue: the catalogue has {total} candidates, but the first 20 lines \
         do not contain '**{total} aday**'. Update this line MANUALLY when the catalogue grows.\n\
         Header:\n{head}"
    );

    // Rows counted in the numerator must contain no untested assumption.
    let bad: Vec<&str> = md
        .lines()
        .filter(|l| l.starts_with('|'))
        .filter(|l| l.contains("KANITLI") || l.contains("DOLAYLI"))
        .filter(|l| l.contains("Varsayım") || l.contains("varsayım"))
        .collect();
    assert!(
        bad.is_empty(),
        "{} rows counted in the numerator (KANITLI+DOLAYLI) contain the word 'varsayım' — either MEASURE it \
         or downgrade it to PARTIAL. ⚠️ The gate is intentionally STRICT: the word should describe only a \
         LIVE assumption and must not appear even in historical notes (otherwise the gate cannot distinguish \
         them and becomes unreliable if made clever):\n  {}",
        bad.len(),
        bad.join("\n  ")
    );
}

// Full-catalog regression: transfer_duration=0 is a valid non-negative integer.
#[test]
fn full_catalog_far006_zero_is_valid_non_negative() {
    let files = with_opts(
        &[(
            "fare_attributes.txt",
            "fare_id,price,currency_type,payment_method,transfer_duration\nF1,2.5,USD,0,0\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&files, &ValidatorConfig::default());
    assert!(!notices.iter().any(|n| n.rule_id == "FAR_006"));
}

// Full-catalog regression: FRQ_001 owns a missing frequency trip FK; TRP_017
// remains for a real trip that has no stop_times rows.
#[test]
fn full_catalog_trp017_requires_an_existing_trip() {
    let missing_trip = with_opts(
        &[(
            "frequencies.txt",
            "trip_id,start_time,end_time,headway_secs\nMISSING,08:00:00,10:00:00,600\n",
        )],
        &[],
        &[],
    );
    let notices = notices_for(&missing_trip, &ValidatorConfig::default());
    assert!(notices.iter().any(|n| n.rule_id == "FRQ_001"));
    assert!(!notices.iter().any(|n| n.rule_id == "TRP_017"));

    let known_without_stop_times = with_opts(
        &[
            (
                "trips.txt",
                "route_id,service_id,trip_id\nR1,SVC1,T1\nR1,SVC1,T2\n",
            ),
            (
                "frequencies.txt",
                "trip_id,start_time,end_time,headway_secs\nT2,08:00:00,10:00:00,600\n",
            ),
        ],
        &[],
        &[],
    );
    let notices = notices_for(&known_without_stop_times, &ValidatorConfig::default());
    assert!(!notices.iter().any(|n| n.rule_id == "FRQ_001"));
    assert!(notices.iter().any(|n| n.rule_id == "TRP_017"));
}
