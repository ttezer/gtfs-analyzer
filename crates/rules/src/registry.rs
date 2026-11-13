use gtfs_core::AuthoritySource::{
    GoogleTransitInterop, GtfsBestPractice, GtfsSpec, MobilitydataParity, ProjectAnalytics,
    ProjectQuality, RegionalProfile,
};
use gtfs_core::DedupLevel::{Entity, Feed, Field, File, Row};
use gtfs_core::ReportId::{R1, R2, R3, R4, R5, R7, R8, R9};
use gtfs_core::RuleClass::{Analytics, Interop, Quality, Spec};
use gtfs_core::Severity::{Bilgi, Dusuk, Kritik, Orta, Yuksek};
use gtfs_core::{AuthoritySource, DedupLevel, ReportId, RuleClass, Severity};

// ── Static report-view sets ──────────────────────────────────────────────────
//
// R1 → publish blocker (build_r1): ONLY CRITICAL+Spec (official GTFS spec gate). Interop/Quality/
//      Analytics NEVER enter R1. VS_K (Spec+Kritik) is the ONLY view carrying R1.
// R2 → all notices (filterable)
// R3 → ANALYTICS class
// R4 → GEO family + SHP_014/016 (geographic correctness)
// R5 → overall score (all classes; Spec×40%+Interop×30%+Quality×20%+Analytics×10%)
// R7 → accessibility (wheelchair_*, stop_access, PTH_012/013)
// R8 → INTEROP class only
// R9 → remediation queue (including ANALYTICS across all classes)

/// Spec + KRİTİK → Blocker → R1 (the ONLY view carrying R1).
const VS_K: &[ReportId] = &[R1, R2, R5, R9];
/// Spec or Quality, non-blocker.
const VS: &[ReportId] = &[R2, R5, R9];
/// Interop (any severity) → R8; no R1 (not a publish blocker).
const VI: &[ReportId] = &[R2, R5, R8, R9];
/// Analytics → R3 (operational quality) + R9 (all notices).
const VA: &[ReportId] = &[R2, R3, R5, R9];

// Geographic addition (R4).
/// Quality + Geo → R4 (no R1/R8).
const VS_GEO: &[ReportId] = &[R2, R4, R5, R9];
/// Analytics + Geo → R3+R4+R9.
const VA_GEO: &[ReportId] = &[R2, R3, R4, R5, R9];

// Accessibility addition (R7).
/// Analytics + Accessibility → R3+R7+R9.
const VA_ACC: &[ReportId] = &[R2, R3, R5, R7, R9];
/// Spec/Quality + Accessibility → R7.
const VS_ACC: &[ReportId] = &[R2, R5, R7, R9];

pub struct RuleMeta {
    pub id: &'static str,
    pub severity: Severity,
    pub rule_class: RuleClass,
    pub base_effort: u8,
    /// Contribution coefficient to the R5 class score. 0.0 is visible but marks a no-score rule.
    pub score_weight: f64,
    /// Canonical rule IDs—does not include report-view IDs such as GEO_008/010/011.
    pub blocks: &'static [&'static str],
    /// GTFS field assigned to scope_key when producing a notice.
    /// Pipe-separated when a composite key is needed (for example, "from_stop_id|to_stop_id").
    pub scope_key_field: Option<&'static str>,
    /// Reports in which this rule's notices appear.
    pub report_views: &'static [ReportId],
    /// Notice deduplication granularity (architecture Section K1).
    pub dedup_level: DedupLevel,
    /// Short general Turkish title for the rule—shown in the UI.
    pub title: &'static str,
}

macro_rules! r {
    ($id:expr, $sev:expr, $cls:expr, $effort:expr, $blocks:expr, $scope:expr, $views:expr, $dedup:expr, $title:expr) => {
        RuleMeta {
            id: $id,
            severity: $sev,
            rule_class: $cls,
            base_effort: $effort,
            score_weight: 1.0,
            blocks: $blocks,
            scope_key_field: $scope,
            report_views: $views,
            dedup_level: $dedup,
            title: $title,
        }
    };
}

macro_rules! r_noscore {
    ($id:expr, $sev:expr, $cls:expr, $effort:expr, $blocks:expr, $scope:expr, $views:expr, $dedup:expr, $title:expr) => {
        RuleMeta {
            id: $id,
            severity: $sev,
            rule_class: $cls,
            base_effort: $effort,
            score_weight: 0.0,
            blocks: $blocks,
            scope_key_field: $scope,
            report_views: $views,
            dedup_level: $dedup,
            title: $title,
        }
    };
}

/// Static metadata table for all canonical rules.
///
/// Out of scope (NOT in the registry):
/// - GEO_008/010/011 — report-view IDs (R4 projection, do not produce notices)
/// - GEO_META — preprocessing step, not a rule
/// - STM_011 → STM_008, TRP_010 → OPR_007, GEO_001 → GEO_META,
///   GEO_005 → STP_017, DQ_007 → MET_001, DQ_008 → MET_002, DQ_015 → RTS_007
pub static RULES: &[RuleMeta] = &[
    // ── ARC: Archive / File Level ─────────────────────────────────────────────
    r!("ARC_001", Kritik, Spec, 1, &[], None, VS_K, Feed,
        "ZIP archive could not be opened"),
    r!("ARC_002", Kritik, Quality, 1,
        &["ARC_010","AGN_003","AGN_009","FIN_008","ATR_010","STP_022","DQ_016"],
        None, VS, File,
        "File cannot be read as UTF-8"),
    r!("ARC_003", Orta,   Quality, 1, &[], None, VS, File,
        "UTF-8 encoding error in optional file"),
    r!("ARC_004", Kritik, Spec, 1,
        &["RTS_002","STM_002","XFL_002","DQ_005","DQ_005b"],
        None, VS_K, File,
        "Required file missing"),
    r_noscore!("ARC_006", Bilgi, Analytics, 1, &[], None, VA, File,
        "Optional GTFS file present"),
    r!("ARC_007", Bilgi,  Quality, 1, &[], None, VS, File,
        "Unrecognized non-GTFS file"),
    r!("ARC_008", Kritik, Spec,    1, &[], None, VS_K, Feed,
        "Calendar file missing (calendar.txt and calendar_dates.txt)"),
    // ARC_031: the spec's FILE-LEVEL requirement—"feed_info.txt · Conditionally Required:
    // Required if translations.txt is provided. Recommended otherwise." Two sentences, two classes:
    // when the condition holds, NORM (this rule, Spec); otherwise, RECOMMENDED (ARC_020, Quality).
    // Until 2026-08-02 both cases were counted by ARC_020 as "recommended file missing" (Low·Quality),
    // so a NORM was reported as a recommendation (the exact opposite of the PTH_017 failure).
    // Practical result: the meaning of translations.txt depends on `feed_info.feed_lang`; without it,
    // the default language is unknown and TRN_007 cannot run at all.
    r!("ARC_031", Kritik, Spec,    1, &[], None, VS_K, Feed,
        "feed_info.txt missing while translations.txt is present"),
    // ARC_009 now reports ONLY optional files: an empty required file was moved to ARC_035
    // (Kritik·Spec) on 2026-08-31. An empty `shapes.txt`/`transfers.txt` is harmless,
    // so the severity is fixed Bilgi rather than dynamic.
    r!("ARC_009", Bilgi, Quality, 1, &[], None, VS, File,
        "Optional file has no data rows"),
    r_noscore!("ARC_010", Bilgi, Quality, 1, &[], None, VS, File,
        "File contains UTF-8 BOM"),
    r!("ARC_011", Bilgi,  Analytics, 1, &[], None, VA, File,
        "File size (info)"),
    r!("ARC_012", Kritik, Spec,    1, &["ARC_015"], None, VS_K, File,
        "Row column count does not match header"),
    r!("ARC_013", Kritik, Spec,    1,
        &["AGN_003","RTS_005","STM_005","STM_006"],
        None, VS_K, File,
        "CSV parse error"),
    r!("ARC_014", Orta,   Quality, 1, &[], None, VS, Field,
        "Leading/trailing whitespace in header"),
    // ARC_015 2026-07-24: MD parity CONFIRMED (`duplicated_column`, ERROR,
    // "Duplicated column in CSV ... same column name repeated") → promoted to Interop.
    r!("ARC_015", Kritik, Interop, 1, &[], None, VI, Field,
        "Duplicate header column"),
    r!("ARC_025", Kritik, Spec,    1, &[], None, VS_K, Field,
        "Required column missing from header"),
    r!("ARC_017", Bilgi,  Quality, 1, &[], None, VS, Field,
        "Unknown column (not defined in GTFS specification)"),
    r!("ARC_018", Orta,   Quality, 1, &[], None, VS, Row,
        "Empty data row"),
    r!("ARC_019", Yuksek, Quality, 1, &[], None, VS, File,
        "Empty column name in header"),
    r!("ARC_020", Dusuk,  Quality, 1, &[], None, VS, Feed,
        "Recommended GTFS file missing (shapes.txt or feed_info.txt)"),
    r!("ARC_021", Dusuk,  Quality, 1, &[], None, VS, File,
        "Non-printable or problematic character in field"),
    r_noscore!("ARC_022", Bilgi, Analytics, 1, &[], None, VA, File,
        "File row count exceeds the 1,000,000 informational threshold"),
    r!("ARC_023", Orta,   Quality, 2, &[], None, VS, File,
        "Nested ZIP file inside GTFS archive"),
    r!("ARC_024", Orta,   Spec,    2, &[], None, VS, File,
        "GTFS .txt file in subdirectory (will not be parsed)"),
    // CLASS CHANGED (#96, 2026-08-09): evidence class ↔ authority of the requirement. Only
    // this rule measured a strict Spec requirement, but the rule was not in the Spec class;
    // users filtering for Spec could not see the violation. Severity UNCHANGED → the R1 gate
    // (`Spec ∧ Kritik`) is unchanged. Ruling: P536c3b95.
    r!("ARC_026", Orta, Spec, 1, &[], None, VS, File,
        "Malformed end-of-line characters"),
    r!("ARC_027", Bilgi, Quality, 1, &[], None, VS, File,
        "ZIP entry lacks user read permission"),
    r!("ARC_028", Dusuk, Quality, 1, &[], None, VS, Feed,
        "GTFS publishing URL does not end with a .zip filename"),
    r!("ARC_029", Kritik, Quality, 1, &[], None, VS, File,
        "Zip-bomb protection triggered"),
    r!("ARC_030", Yuksek, Spec, 1, &[], None, VS, File,
        "Tab or line break in a field value"),
    // ARC_032: the third prohibition in the same spec sentence—File Requirements, "Field values must not
    // contain HTML tags, comments or escape sequences." ARC_030 (tab/newline) and ARC_021
    // (non-printable character) MISS this: `<` (U+003C) is in none of ARC_021's ranges.
    // Measured (239 feeds): 4 feeds, 1,955 values—one feed has `<BR>` in 1,176 stop names.
    r!("ARC_032", Yuksek, Spec, 1, &[], None, VS, File,
        "HTML markup or character escape in a field value"),
    // ARC_033: File Requirements' QUOTATION sentences—"Field values that contain quotation
    // marks or commas must be enclosed within quotation marks" + "each quotation mark in the
    // field value must be preceded with a quotation mark". Until 2026-08-07 the tokenizer
    // treated the `"` character in an unquoted field as an ORDINARY CHARACTER; the violation was invisible
    // and both requirements were INDIRECTLY attributed to `ARC_013` in the ledger—that basis was WRONG.
    // ⚠️ NOT Critical: the data remains readable and the tokenizer recovers. Linking it to `ARC_013`
    // (FATAL in a required file) would reject a nearly valid feed—the `PTH_017` lesson.
    r!("ARC_033", Yuksek, Spec, 1, &[], None, VS, File,
        "Unescaped quotation mark in a field value (RFC 4180)"),
    r!("ARC_034", Yuksek, Spec, 1, &[], None, VS, File,
        "Header row repeated as a data row"),
    // ARC_035 (13th corpus run, 2026-08-31): the "required file EXISTS" test checked file
    // PRESENCE, not CONTENT. A zero-byte `stop_times.txt` counted as "present", and only
    // ARC_009 (Quality) saw the emptiness; because the R1 gate is `Spec ∧ Kritik`, no publish
    // blocker arose. Measurement: at least 32 feeds had an empty required file, and FIVE
    // (mdb-1817 · mdb-1819 · mdb-3181 · mdb-342 · mdb-3459) scored 100.0 for PUBLISH SCORE
    // with an empty `stop_times.txt`. Deleting the file produced ARC_004 (Kritik·Spec), while
    // emptying it was penalty-free—so emptying the file INCREASED the score.
    // Same axis and exemption as ARC_004: `stops.txt` is exempt in a pure Flex feed.
    r!("ARC_035", Kritik, Spec, 1, &[], None, VS_K, File,
        "Required file is empty"),
    // ARC_036 (2026-09-08): the ZIP entry's STREAM view conflicts with the central directory—
    // the local header has the data-descriptor flag (bit 3) set but no descriptor record, or
    // its crc/sizes do not match the central directory (APPNOTE 4.4.4). Tools reading the
    // central directory (us, Python `zipfile`, commons-compress) notice nothing; stream readers
    // cannot open the archive. Two corpus cases: `tdg-81618` (flag set, sizes present, NO descriptor)
    // and `mdb-2607` (descriptor present, wrong `usize=0`).
    // 🔑 Class is NOT SPEC: the violated document is the ZIP APPNOTE, not GTFS; authority is ProjectQuality.
    // Severity is LOW BY MEASUREMENT: 0 cases in a random sample of 200 feeds, 2 of all known cases in 4,332.
    // ⚠️ FP gate: 40% of the sample (80/200) uses data descriptors LEGITIMATELY (sizes 0, descriptor present)—
    // the rule fires for NONE of them. Relaxing the condition would create false findings on ~1,700 feeds;
    // do not fire without COMPARING the descriptor to the central directory.
    r!("ARC_036", Dusuk, Quality, 1, &[], None, VS, Feed,
        "ZIP stream header contradicts the central directory"),
    // Spec ID recommendation ("printable ASCII recommended"); GTFS-JP IDs intentionally use Japanese → excluded from score.
    r_noscore!("ARC_037", Dusuk, Quality, 1, &[], None, VS, File,
        "Non-ASCII character in an ID field"),

    // ── BKR: Booking Rules ─────────────────────────────────────────────────────
    r!("BKR_001", Yuksek, Spec, 1, &[], Some("booking_rule_id"), VS, Entity,
        "Prior-day booking field set in prohibited context"),
    // BKR_002 (2026-09-19): retired. `prior_notice_start_day` is optional for booking_type=1
    // when `prior_notice_duration_max` is empty; this ID must not be reused (enforced by the
    // removed_ids_absent test).
    r!("BKR_003", Yuksek, Spec, 1, &[], Some("booking_rule_id"), VS, Entity,
        "prior_notice_start_time only valid with prior_notice_start_day"),
    r!("BKR_004", Yuksek, Spec, 1, &[], Some("booking_rule_id"), VS, Entity,
        "prior_notice fields prohibited for real-time booking"),
    r!("BKR_005", Orta,   Spec, 1, &[], Some("booking_rule_id"), VS, Entity,
        "prior_notice_duration_max only valid with booking_type=1"),
    r!("BKR_006", Yuksek, Spec, 1, &[], Some("booking_rule_id"), VS, Entity,
        "prior_notice_duration_min invalid (≤ 0 or non-numeric)"),
    r!("BKR_007", Kritik, Spec, 1, &[], Some("booking_rule_id"), VS_K, Entity,
        "prior_notice_duration_min required for booking_type=1"),
    r!("BKR_008", Kritik, Spec, 1, &[], Some("booking_rule_id"), VS_K, Entity,
        "prior_notice_last_day required for booking_type=2"),
    r!("BKR_009", Kritik, Spec, 1, &[], Some("booking_rule_id"), VS_K, Entity,
        "prior_notice_last_time required for booking_type=2"),
    r!("BKR_010", Yuksek, Spec, 1, &[], Some("booking_rule_id"), VS, Entity,
        "prior_notice_start_time required when prior_notice_start_day is set"),
    r!("BKR_011", Yuksek, Interop, 1, &[], Some("booking_rule_id"), VI, Entity,
        "prior_notice_last_day > prior_notice_start_day: invalid booking window"),
    r!("BKR_012", Orta,   Spec, 1, &[], Some("booking_rule_id"), VS, Entity,
        "prior_notice_duration_min forbidden with booking_type=2"),
    r!("BKR_013", Yuksek, Spec, 1, &[], Some("booking_rule_id"), VS, Entity,
        "prior_notice_last_time only valid with prior_notice_last_day"),
    r!("BKR_014", Yuksek, Spec, 1, &[], Some("booking_rule_id"), VS, Entity,
        "prior_notice_service_id only valid with booking_type=2"),
    r!("BKR_015", Kritik, Spec, 1, &[], Some("booking_rule_id"), VS_K, Entity,
        "prior_notice_service_id not found (calendar/calendar_dates)"),
    r!("BKR_016", Kritik, Spec, 1,
        &["BKR_001","BKR_004","BKR_005","BKR_007","BKR_008","BKR_009","BKR_012","BKR_014"],
        Some("booking_rule_id"), VS_K, Entity,
        "booking_type missing or invalid"),
    r!("BKR_017", Kritik, Spec, 1, &[], Some("pickup_booking_rule_id"), VS_K, Row,
        "pickup_booking_rule_id not found (booking_rules)"),
    r!("BKR_018", Kritik, Spec, 1, &[], Some("drop_off_booking_rule_id"), VS_K, Row,
        "drop_off_booking_rule_id not found (booking_rules)"),
    r!("BKR_019", Kritik, Spec, 1, &["BKR_017","BKR_018"], Some("booking_rule_id"), VS_K, Entity,
        "booking_rule_id missing or duplicated"),
    // BKR_020/021/022: the file's three communication fields were listed in `known_columns` (so
    // they did not trigger an "unknown column" warning), but NO validation stage read them—issue #60.
    // booking_url is the address where the passenger ACTUALLY makes the booking; info_url is explanatory only.
    // BKR_024: the spec says `prior_notice_start_day` is "Forbidden for booking_type=1 if
    // prior_notice_duration_max is defined". `duration_max` occurs in three parser locations,
    // and none was linked to `start_day`; BKR_024 measures only this three-field condition.
    // ⚠️ NO corpus example → synthetic fixture.
    r!("BKR_024", Orta, Spec, 1, &[], Some("booking_rule_id"), VS, Row,
        "prior_notice_start_day forbidden with booking_type=1 and duration_max"),
    r!("BKR_020", Orta,   Spec, 1, &[], Some("booking_rule_id"), VS, Entity,
        "booking_url is invalid"),
    r!("BKR_021", Dusuk,  Spec, 1, &[], Some("booking_rule_id"), VS, Entity,
        "info_url is invalid"),
    // phone_number: the spec's `Phone number` type defines no GRAMMAR ("A phone number."),
    // so a strict check would reject valid international formats. Following the AGN_007 precedent,
    // this is NOT Spec, but Quality.
    r!("BKR_022", Dusuk,  Quality, 1, &[], Some("booking_rule_id"), VS, Entity,
        "phone_number is invalid"),
    // BKR_023: the four prior_notice numeric fields are `Integer` in the spec. Non-numeric values
    // used to be SILENTLY dropped by `opt_int`'s `.parse().ok()`. Reusing BKR_001/002/004/005
    // would have been WRONG: those are CONTEXT rules ("valid only with booking_type=1"),
    // not type rules—`emit_identity` caught this attempt.
    r!("BKR_023", Orta,   Spec, 1, &[], Some("booking_rule_id"), VS, Entity,
        "prior_notice numeric field is not an integer"),
    r!("BKR_025", Orta,   Spec, 1, &[], Some("booking_rule_id"), VS, Entity,
        "prior_notice time field is not a valid time"),

    // ── AGN: Agency ────────────────────────────────────────────────────────────
    // AGN_001 REMOVED (2026-08-02): it was never emitted by any path—neither as a Notice
    // nor as a fatal code. ARC_004 (Fatal NoRequiredFiles) represents a missing agency.txt.
    // It had been kept to DOCUMENT MD `missing_required_file` parity; the documentation belongs
    // on ARC_004's card, not in a fake registry entry. The cost: counting it as Kritik·Spec inflated
    // "number of Spec rules / R1 blockers", it had a card and 3 locale entries users could never see,
    // and `sync_cards` printed "unresolved" on every run. The `blocks: ["RTS_002"]` link was also
    // removed—a rule that never fires cannot block anything.
    r!("AGN_002", Kritik, Spec, 1, &[], Some("agency_id"), VS_K, Entity,
        "agency_name missing"),
    r!("AGN_003", Kritik, Spec, 1, &[], Some("agency_id"), VS_K, Entity,
        "agency_url missing or invalid"),
    r!("AGN_004", Kritik, Spec, 1, &[], Some("agency_id"), VS_K, Entity,
        "agency_timezone missing or invalid"),
    // CLASS CHANGED (#96, 2026-08-09): evidence class ↔ authority of the requirement. Only
    // this rule measured a strict Spec requirement, but the rule was not in the Spec class;
    // users filtering for Spec could not see the violation. Severity UNCHANGED → the R1 gate
    // (`Spec ∧ Kritik`) is unchanged. Ruling: Pf27a2fd0.
    r!("AGN_005", Orta,   Spec, 1, &[], None, VS, Feed,
        "Timezone inconsistency across agencies"),
    r!("AGN_006", Dusuk,  Spec, 1, &[], Some("agency_id"), VS, Entity,
        "agency_lang invalid"),
    r!("AGN_007", Dusuk,  Quality, 1, &[], Some("agency_id"), VS, Entity,
        "agency_phone invalid"),
    r!("AGN_008", Dusuk,  Spec, 1, &[], Some("agency_id"), VS, Entity,
        "agency_fare_url invalid"),
    r!("AGN_009", Dusuk,  Spec, 1, &[], Some("agency_id"), VS, Entity,
        "agency_email invalid"),
    r!("AGN_010", Kritik, Spec, 1, &[], Some("agency_id"), VS_K, Entity,
        "Duplicate agency_id"),
    r!("AGN_011", Kritik, Spec, 1, &[], None, VS_K, Feed,
        "Multiple agencies with no agency_id"),
    // Spec defines the `Enum` type normatively: "An option from a set of predefined constants
    // defined in the Description column." An out-of-set value is a spec violation → class Quality→Spec
    // (2026-08-01, WP-3). Severity remains LOW: precedents for optional enum invalidity are RTS_013 /
    // TRP_006 / TRP_007 (all Low·Spec). The publish gate is unaffected (R1 = Spec ∧ Critical).
    r!("AGN_012", Dusuk,  Spec, 1, &[], Some("agency_id"), VS, Entity,
        "cemv_support invalid (agency)"),
    r!("AGN_013", Dusuk,  Interop, 1, &[], Some("agency_id"), VI, Entity,
        "Feed language and agency language mismatch"),
    r!("AGN_014", Kritik, Spec, 1, &[], None, VS_K, Entity,
        "Multiple agencies but agency.txt agency_id missing"),
    r!("AGN_015", Bilgi,  Quality, 1, &[], Some("agency_url"), VS, Entity,
        "agency_url uses insecure http"),
    r!("AGN_016", Bilgi,  Quality, 1, &[], Some("agency_phone"), VS, Entity,
        "agency_phone suspect/placeholder"),
    r!("AGN_017", Dusuk,  Interop, 1, &[], None, VI, Feed,
        "Inconsistent agency_lang across agencies"),
    r_noscore!("AGN_018", Bilgi, Quality, 1, &[], None, VS, Feed,
        "agency_id empty in a single-agency feed"),

    // ── STP: Stops ─────────────────────────────────────────────────────────────
    r!("STP_001", Kritik, Spec, 1,
        &["STP_009","STM_002","GEO_002"],
        Some("stop_id"), VS_K, Entity,
        "Duplicate stop_id"),
    r!("STP_002", Kritik, Spec, 1, &[], Some("stop_id"), VS_K, Entity,
        "stop_id is empty"),
    r!("STP_003", Kritik, Spec, 2,
        &["STP_021","GEO_002","GEO_009","GEO_012","STP_016","STP_017"],
        Some("stop_id"), VS_K, Entity,
        "stop_name missing or stop_lat out of range"),
    r!("STP_004", Kritik, Spec, 2,
        &["STP_021","GEO_002","GEO_009","GEO_012","STP_016","STP_017"],
        None, VS_K, Feed,
        "stop_lat is not numeric"),
    // STP_005 relies on the SAME spec sentence as its lat siblings (STP_003/004/006/007);
    // 2026-07-24 alignment: Quality→Spec, VS→VS_K, and blocks were added (a bad lon
    // masks coordinate-derived GEO/proximity rules—symmetric with lat).
    r!("STP_005", Kritik, Spec, 2,
        &["STP_021","GEO_002","GEO_009","GEO_012","STP_016","STP_017"],
        None, VS_K, Feed,
        "stop_lon invalid or out of range"),
    r!("STP_006", Kritik, Spec, 2,
        &["GEO_002","GEO_009","GEO_012","STP_016","STP_017"],
        Some("stop_id"), VS_K, Entity,
        "stop_lat missing"),
    r!("STP_007", Kritik, Spec, 2,
        &["GEO_002","GEO_009","GEO_012","STP_016","STP_017"],
        Some("stop_id"), VS_K, Entity,
        "stop_lon missing"),
    r!("STP_008", Yuksek, Spec, 1,
        &["STP_010","STP_011","STP_012"],
        Some("stop_id"), VS, Entity,
        "location_type invalid"),
    r!("STP_009", Kritik, Spec, 1,
        &["STP_010","STP_011"],
        Some("stop_id"), VS_K, Entity,
        "parent_station not found"),
    r!("STP_010", Yuksek, Spec, 1, &[], Some("stop_id"), VS, Entity,
        "parent_station is not location_type=1"),
    r!("STP_011", Kritik, Spec, 1, &[], Some("stop_id"), VS_K, Entity,
        "parent_station required for entrance/exit/boarding area"),
    r!("STP_012", Kritik, Spec, 1, &[], Some("stop_id"), VS_K, Entity,
        "Station or entrance used in stop_times"),
    r!("STP_013", Dusuk,  Spec, 1, &[], Some("stop_id"), VS_ACC, Entity,
        "wheelchair_boarding invalid"),
    r!("STP_014", Orta,   Spec, 1, &[], Some("stop_id"), VS, Entity,
        "stop_timezone invalid"),
    // Critical: the removed XFL_009 reported the same level_id FK violation as Critical;
    // lowering the severity would let this violation escape the R1 publish gate.
    r!("STP_015", Kritik, Spec, 1, &[], Some("stop_id"), VS_K, Entity,
        "level_id not found"),
    r!("STP_016", Orta,   Quality, 2, &[], Some("stop_id"), VS, Entity,
        "Two stops at identical coordinates"),
    r!("STP_017", Dusuk,  Quality, 1, &[], Some("stop_id"), VS, Entity,
        "Two stops too close together"),
    r!("STP_018", Kritik, Spec, 1, &[], None, VS_K, Feed,
        "No stops found"),
    r!("STP_019", Bilgi,  Quality, 1, &[], Some("stop_id"), VS, Entity,
        "stop_name too long"),
    r!("STP_020", Orta,   Analytics, 2, &[], Some("stop_id"), VA, Entity,
        "Stop with no trips"),
    r!("STP_021", Yuksek, Quality, 2, &[], Some("stop_id"), VS, Entity,
        "Boarding area parent is not a platform"),
    r_noscore!("STP_022", Orta, Quality, 1, &[], Some("stop_id"), VS, Entity,
        "stop_code missing"),
    r!("STP_023", Dusuk,  Quality, 1, &[], Some("stop_id"), VS, Entity,
        "tts_stop_name invalid"),
    r!("STP_024", Bilgi,  Quality, 1, &["STP_026"], Some("stop_id"), VS, Entity,
        "stop_access outside K2 compatibility range"),
    r!("STP_026", Dusuk,  Spec, 1,
        &[],
        Some("stop_id"), VS_ACC, Feed,
        "stop_access invalid value"),
    r!("STP_028", Bilgi,  Quality, 1, &[], Some("stop_id"), VS, Entity,
        "stop_code too long"),
    r!("STP_029", Orta, Quality, 2, &[], Some("stop_id"), VS, Entity,
        "Stop inside station but coordinate too far"),
    r!("STP_030", Orta,   Quality, 1, &[], Some("stop_id"), VS, Entity,
        "Parent station has no child stops"),
    r!("STP_031", Bilgi,  Quality, 1, &[], Some("stop_id"), VS, Entity,
        "Stop name and description are identical"),
    r!("STP_032", Orta,   Quality, 2, &[], None, VS, Feed,
        "parent_station missing for pathway-connected platform"),
    r!("STP_033", Bilgi,  Quality, 1, &[], Some("stop_id"), VS, Entity,
        "Stop zone_id missing (required for fare calculation)"),
    r!("STP_034", Dusuk,  Quality, 1, &[], Some("stop_id"), VS, Entity,
        "stop_url matches agency URL"),
    r!("STP_035", Dusuk,  Quality, 1, &[], Some("stop_id"), VS, Entity,
        "stop_url matches route URL"),
    r!("STP_036", Dusuk,  Spec, 1, &[], Some("stop_id"), VS, Entity,
        "Station has parent_station (invalid)"),
    r!("STP_037", Bilgi,  Quality, 1, &[], None, VS_ACC, Feed,
        "Some stops have not set wheelchair boarding"),
    r!("STP_038", Bilgi,  Quality, 1, &[], None, VS_ACC, Feed,
        "No stops report wheelchair boarding"),
    r_noscore!("STP_039", Dusuk, Quality, 1, &[], Some("stop_code"), VS, Entity,
        "stop_code is duplicated"),
    r!("STP_040", Dusuk, Quality, 1, &[], Some("stop_id"), VS, Entity,
        "Stop name contains a redundant stop/station word"),
    // STP_044: the spec says `platform_code` "should be just the platform identifier" and
    // explicitly excludes the words "platform"/"track" (or their feed-language equivalents).
    // The field was not parsed AT ALL until 2026-08-03. STP_040 applies the same shape to
    // `stop_name`, but that is OPT-IN (a generic name may be legitimate: "Union Station");
    // here it is NOT opt-in because the spec directly forbids it. Measured (239 feeds):
    // 1 feed, 1 record (`gleis`).
    r!("STP_044", Dusuk, Quality, 1, &[], Some("stop_id"), VS, Entity,
        "platform_code contains a redundant platform/track word"),
    r_noscore!("STP_045", Bilgi, Quality, 1, &[], None, VS, Feed,
        "platform_code without parent_station"),
    r!("STP_041", Dusuk, Quality, 1, &[], Some("stop_id"), VS, Entity,
        "Child stop name does not include the parent station name"),
    // STP_042: spec type `URL`—"fully qualified URL that includes http:// or https://".
    // Every sibling field had a format rule (agency_url→AGN_002, agency_fare_url→AGN_008,
    // route_url→RTS_005, feed_publisher_url→FIN_002, attribution_url→ATR_007); stop_url
    // had only STP_034/035 (same as another URL), so its format was never measured.
    r!("STP_042", Dusuk, Spec, 1, &[], None, VS, Feed,
        "stop_url is invalid"),
    // STP_043: the spec gives `stop_access` a SEPARATE conditional prohibition—
    // "Forbidden for locations which are stations (1), entrances (2), generic nodes (3) or
    // boarding areas (4). Forbidden if parent_station is empty." It was not checked at all
    // until 2026-08-03. Reusing STP_026 (enum validity) would be WRONG: an enum error and
    // a contextual prohibition are two separate facts (the ATR_006 lesson).
    r!("STP_043", Orta,  Spec, 1, &[], Some("stop_id"), VS, Entity,
        "stop_access used in a forbidden context"),

    // ── RTS: Routes ────────────────────────────────────────────────────────────
    r!("RTS_001", Kritik, Spec, 1,
        &["FRL_002"],
        Some("route_id"), VS_K, Entity,
        "Duplicate route_id"),
    r!("RTS_002", Kritik, Spec, 1,
        &[],
        Some("route_id"), VS_K, Entity,
        "agency_id not found"),
    r!("RTS_003", Kritik, Spec, 1,
        &["DQ_003"],
        Some("route_id"), VS_K, Entity,
        "Both route_short_name and route_long_name missing"),
    r!("RTS_004", Kritik, Spec, 1, &[], Some("route_id"), VS_K, Entity,
        "route_type missing or invalid"),
    r!("RTS_005", Orta,   Spec, 1, &[], Some("route_id"), VS, Entity,
        "route_url invalid"),
    r!("RTS_006", Orta,   Spec, 1, &[], Some("route_id"), VS, Entity,
        "route_color invalid hex color"),
    // SYMMETRIC with RTS_006 (2026-08-01, WP-3): the spec defines BOTH `route_color` and
    // `route_text_color` with the `Color` type, and the type is normative—verbatim: "A color encoded as a
    // six-digit hexadecimal number … the leading '#' must not be included". Two rules measuring
    // the same requirement (same helper, same empty-value filter) were in different classes;
    // RTS_007 was Quality·Low. There was no material reason for the distinction. Optionality
    // does not make the type non-binding: when present, the value must be six-digit hex.
    // NOTE: the contrast requirement uses "should" → RTS_008 correctly remains Quality.
    r!("RTS_007", Orta,   Spec, 1, &[], Some("route_id"), VS, Entity,
        "route_text_color invalid hex color"),
    r!("RTS_008", Orta,   Quality, 1, &[], Some("route_id"), VS, Entity,
        "Route color and text color have low contrast"),
    r!("RTS_010", Dusuk,  Quality, 1, &[], Some("route_id"), VS, Entity,
        "route_short_name too long"),
    r!("RTS_011", Bilgi,  Quality, 1, &[], Some("route_id"), VS, Entity,
        "route_long_name too long"),
    r!("RTS_012", Orta,   Quality, 1, &[], Some("route_id"), VS, Entity,
        "Route with no trips"),
    r!("RTS_013", Dusuk,  Spec, 1, &[], Some("route_id"), VS, Entity,
        "continuous_pickup invalid"),
    r!("RTS_016", Dusuk,  Quality, 2, &[], Some("route_id"), VS, Entity,
        "Route with no active service days"),
    r_noscore!("RTS_017", Bilgi, Quality, 2, &[], Some("route_id"), VS, Entity,
        "Route with no shape defined"),
    r!("RTS_018", Dusuk,  Spec, 1, &[], Some("route_id"), VS, Entity,
        "continuous_drop_off invalid"),
    r!("RTS_019", Orta,   Quality, 1, &[], Some("route_id"), VS, Row,
        "Duplicate route name"),
    r!("RTS_020", Bilgi,  Quality, 1, &[], Some("route_id"), VS, Entity,
        "Route and agency share the same URL"),
    r!("RTS_021", Dusuk, Quality, 1, &[], Some("route_id"), VS, Entity,
        "route_short_name exceeds Google Transit limit (6 characters)"),
    r!("RTS_022", Dusuk,  Quality, 1, &[], Some("route_id"), VS, Entity,
        "route_long_name contains route_short_name"),
    r!("RTS_023", Bilgi,  Quality, 1, &[], Some("route_id"), VS, Entity,
        "route_desc repeats a route name"),
    // Same rationale as AGN_012: the `Enum` type is normative, so an out-of-set value is a spec violation.
    // Class Quality→Spec (2026-08-01, WP-3); severity Low (precedents RTS_013/RTS_018).
    // Enum validity for cEMV fields is measured here; the precedence relationship between
    // Fares v2 and cEMV is not a feed violation.
    r!("RTS_024", Dusuk,  Spec, 1, &[], Some("route_id"), VS, Entity,
        "cemv_support invalid (route)"),
    r!("RTS_025", Bilgi,  Quality, 1, &[], Some("route_id"), VS, Entity,
        "routes.txt agency_id empty (recommended)"),
    r!("RTS_026", Bilgi,  Quality, 1, &[], Some("route_id"), VS, Row,
        "Duplicate short route name"),
    r!("RTS_027", Bilgi,  Quality, 1, &[], Some("route_id"), VS, Row,
        "Duplicate long route name"),
    // RTS_028 CLASS CHANGED (2026-08-03): Interop → Spec. The spec states this prohibition NORMATIVELY
    // in the `routes.txt` field table: "Any value other than 1 or empty is Forbidden if
    // stop_times.start/end_pickup_drop_off_window are defined for any trip of this route."
    // The old rationale (issue #57) was the MD `forbidden_continuous_pickup_drop_off`; but MD parity
    // determines the rule SHAPE (two fields, one notice), not its CLASS. The same note left the
    // stop_times side as "no MD counterpart → Spec", so the SAME spec requirement had two classes.
    // The authority is the spec; MD parity remains a note on the card.
    r!("RTS_028", Yuksek, Spec, 1, &[], Some("route_id"), VS, Entity,
        "continuous_pickup/drop_off forbidden on a Flex route"),
    // RTS_029: spec type `Non-negative integer`. Negative or non-numeric values used to be
    // SILENTLY dropped by `Err(_) => None`—route ordering disappeared and users saw nothing.
    // (The same swallowing pattern also existed in STM_058 and PTH_027.)
    r!("RTS_029", Dusuk,  Spec, 1, &[], Some("route_id"), VS, Entity,
        "route_sort_order is invalid"),
    // RTS_030 (#93): the core Schedule Reference `route_type` enum lists ONLY 0-7,11,12;
    // 100-1799 is the Google Transit/HVT convention. RTS_004 used to accept both (Critical·Spec,
    // R1 gate), presenting an extension as if it were core spec compliance.
    // Class Interop because MD parity EXISTS: corpus MD reports issue `unexpected_enum_value`
    // (WARNING) for extended values—in 19 of 225 feeds.
    r!("RTS_030", Dusuk,  Interop, 1, &[], Some("route_id"), VI, Entity,
        "route_type uses an extended value outside the core enum"),
    // RTS_031: `route_id` is EMPTY. `RTS_001` measures duplicates; a missing ID cannot be duplicated.
    // In `k2/routes.rs`, an empty ID silently became `entity_id=None` and no rule saw it.
    // Four feeds in the full catalog run (#153). Primary key → Kritik·Spec.
    r!("RTS_031", Kritik, Spec, 1, &[], Some("route_id"), VS_K, Row,
        "route_id missing"),

    // ── TRP: Trips ─────────────────────────────────────────────────────────────
    r!("TRP_001", Kritik, Spec, 1,
        &["STM_001","FRQ_001","XFL_002","OPR_006","OPR_007"],
        Some("trip_id"), VS_K, Entity,
        "trip_id missing or duplicate"),
    r!("TRP_002", Kritik, Spec, 1, &[], Some("trip_id"), VS_K, Entity,
        "route_id not found"),
    // OPR_016 was inherited from the removed XFL_001 (same service_id FK violation).
    r!("TRP_003", Kritik, Spec, 1,
        &["DQ_005","DQ_005b","CAL_009","CAL_012","OPR_011","OPR_016"],
        None, VS_K, Feed,
        "service_id not found"),
    r!("TRP_004", Yuksek, Spec, 1,
        &["SHP_014","SHP_016","SHP_017"],
        Some("shape_id"), VS, Entity,
        "shape_id not found"),
    r!("TRP_005", Orta,   Spec, 1, &[], None, VS, Feed,
        "direction_id invalid"),
    r!("TRP_006", Dusuk,  Spec, 1, &[], Some("trip_id"), VS, Entity,
        "wheelchair_accessible invalid"),
    r!("TRP_007", Dusuk,  Spec, 1, &[], Some("trip_id"), VS, Entity,
        "bikes_allowed invalid"),
    r!("TRP_032", Dusuk,  Spec, 1, &[], Some("trip_id"), VS, Entity,
        "cars_allowed invalid"),
    r_noscore!("TRP_011", Yuksek, Quality, 3, &[], Some("trip_id"), VS, Entity,
        "Trip headsign not set"),
    r!("TRP_012", Dusuk,  Quality, 1, &[], Some("route_id"), VS, Entity,
        "direction_id missing on bidirectional route"),
    r_noscore!("TRP_013", Bilgi, Analytics, 1, &[], Some("route_id"), VA, Entity,
        "Route has only one trip"),
    r!("TRP_014", Bilgi,  Quality, 1, &[], Some("trip_id"), VS, Entity,
        "trip_short_name too long"),
    // TRP_015 REDEFINED (2026-09-01, 13th corpus run). The old requirement was "a block must
    // contain at least 2 trips" and was AGAINST THE SPEC: GTFS Schedule Reference `trips.block_id`
    // says "A block consists of **a single trip** or many sequential trips ... defined by
    // shared service days and block_id. A block_id may have trips with different service
    // days, making distinct blocks."—a single-trip block IS VALID.
    // Measurement: the rule fired per trip → 1,549,904 notices / 675 feeds (3.9% of the corpus),
    // with NO MD counterpart. In a 14-feed sample, applying the spec's pair definition ((block_id, service_id))
    // did NOT REDUCE single-trip blocks; it INCREASED them (mdb-2727 101,089 → 101,621),
    // because the same block_id creates separate blocks on different service days. So defining
    // the requirement "correctly" would increase noise; the requirement itself was wrong.
    // New requirement—the only defensible remaining signal: if block_id groups NO trips,
    // the field is effectively a `trip_id` copy and carries no vehicle-continuity information.
    // In the same sample, 8 of 14 feeds were exactly 100%, so this is a clear data class, not an arbitrary threshold.
    // ONE finding per FEED.
    r!("TRP_015", Dusuk,  Quality, 1, &[], None, VS, Feed,
        "block_id groups no trips"),
    r!("TRP_017", Orta,   Quality, 1, &[], Some("trip_id"), VS, Entity,
        "Frequency-based trip missing from stop_times"),
    // TRP_019 CLASS CHANGED (2026-08-03): Quality → Spec. For `trips.shape_id`, the spec says
    // "Conditionally Required: Required if the trip has a continuous pickup or drop-off
    // behavior defined either in routes.txt or in stop_times.txt"—this is a NORM, not a preference.
    // (ARC_020 had the opposite error: a norm was reported as a recommendation.)
    r!("TRP_019", Yuksek, Spec, 1, &[], Some("trip_id"), VS, Entity,
        "shape_id missing with continuous service active"),
    r!("TRP_020", Bilgi,  Analytics, 1, &[], Some("trip_id"), VA, Entity,
        "trip_headsign matches intermediate stop name"),
    r!("TRP_021", Bilgi,  Quality, 1, &[], None, VS, Entity,
        "Bicycle allowance (bikes_allowed) not specified"),
    r!("TRP_022", Yuksek, Interop, 3, &[], Some("trip_id"), VI, Entity,
        "Overlapping trip times within block"),
    // If the feed has no active trip on any of 7 days, TRP_023 (feed-level, None scope) is
    // the single root message and suppresses repeated TRP_030 spam for every trip.
    // (The old direction was ineffective because TRP_030→TRP_023 scopes did not match.)
    r!("TRP_023", Dusuk,  Quality, 1, &["CAL_024"], None, VS, Feed,
        "No active trips in the next 7 days"),
    r!("TRP_024", Dusuk,  Quality, 1, &[], Some("block_id"), VS, Entity,
        "Inconsistent route type within block"),
    r!("TRP_025", Bilgi,  Quality, 1, &[], None, VS, Feed,
        "High proportion of trips without wheelchair accessibility information"),
    r!("TRP_026", Orta, Analytics, 2, &[], Some("trip_id"), VA, Entity,
        "Trip with empty active-date set"),
    r!("TRP_028", Bilgi, Quality, 1, &[], None, VS, Feed,
        "Some trips have not set wheelchair accessibility"),
    r!("TRP_029", Bilgi, Quality, 1, &[], None, VS, Feed,
        "No trips report wheelchair accessibility"),
    r!("TRP_031", Kritik, Spec, 1, &[], Some("route_id"), VS_K, Entity,
        "route_id missing"),
    // TRP_034: `safe_duration_factor` and `safe_duration_offset` are `Float` in the spec. Parse
    // errors used to be SILENTLY swallowed by `.ok().flatten()` (the day's fourth example).
    // Also, `safe_duration_offset` was read as u32—the spec says Float, so a VALID `12.5`-second
    // value was silently deleted. Both issues were fixed.
    r!("TRP_034", Orta, Spec, 1, &[], Some("trip_id"), VS, Entity,
        "safe_duration field is not a number"),
    r!("TRP_035", Kritik, Spec, 1, &[], Some("trip_id"), VS_K, Row,
        "trips.txt has an empty service_id"),

    // ── STM: Stop Times ────────────────────────────────────────────────────────
    r!("STM_001", Kritik, Spec, 1,
        &["STM_008","STM_056","OPR_008"],
        Some("trip_id"), VS_K, Row,
        "trip_id not found"),
    // blocks: inherited from the removed XFL_005, STP_012 + DQ_005c—it repeated the same FK
    // violation as a feed-level summary (see CHANGELOG 0.8.0).
    r!("STM_002", Kritik, Spec, 1,
        &["GEO_002","STP_012"],
        Some("stop_id"), VS_K, Row,
        "stop_id not found"),
    // STM_015/STM_016 were added (#155): both measure `arrival_time` ABSENCE, but the
    // `arrival_time()` accessor returns `None` both when the field is MISSING and when it
    // CANNOT BE PARSED. If the time is written as `HH:MM`, STM_003 already rejects it and the
    // claim "no time at the first/last stop" is WRONG—the value exists but its format is bad.
    // In mdb-2727, 101,621 of STM_015's 101,621 rows also carry STM_003.
    // ⚠️ Suppression scopes must match (trip): in feeds where STM_003 never fires, nothing is
    // suppressed because there is no root—9 of the 10 corpus feeds with STM_015 are like this,
    // and their findings are LEGITIMATE. Remaining risk: the SAME trip has both an invalid time
    // and a genuinely missing first-stop time; mdb-2727 had 0 such cases.
    r!("STM_003", Kritik, Spec, 1,
        &["STM_004","STM_008","STM_056","STM_015","STM_016"],
        Some("trip_id"), VS_K, Row,
        "arrival_time invalid format"),
    r!("STM_004", Kritik, Spec, 1,
        &["STM_008","STM_012","STM_056"],
        Some("trip_id"), VS_K, Row,
        "departure_time invalid format"),
    r!("STM_005", Kritik, Spec, 2,
        &["STM_008","STM_012","STM_028","STM_029","OPR_008"],
        Some("trip_id"), VS_K, Row,
        "stop_sequence missing or invalid"),
    r!("STM_006", Kritik, Spec, 2,
        &["STM_007","STM_008","STM_012","STM_028","STM_029","OPR_008"],
        Some("trip_id"), VS_K, Row,
        "stop_id missing (stop_times)"),
    r!("STM_007", Yuksek, Interop, 2, &[], Some("trip_id"), VI, Row,
        "Departure time before arrival time"),
    r!("STM_008", Kritik, Interop, 2, &[], Some("trip_id"), VI, Entity,
        "Time decreases between stops"),
    r!("STM_009", Yuksek, Spec, 1, &[], Some("trip_id"), VS, Row,
        "pickup_type invalid"),
    r!("STM_010", Yuksek, Spec, 1, &[], Some("trip_id"), VS, Row,
        "drop_off_type invalid"),
    r!("STM_012", Yuksek, Interop, 2, &[], Some("trip_id"), VI, Row,
        "Unrealistic speed between stops"),
    // STM_013 (2026-09-21): retired. Leaving times out at intermediate stops is optional in GTFS;
    // this rule penalized the timepoint model as a broken feed and created systematic false positives
    // for valid timing-point producers. The ID must not be reused.
    // Emit is grouped per (route, direction, segment) → Entity; entity_id is a composite segment
    // key, scope_key is route_id (UI route filter). Formerly Row/trip_id, it reported the same
    // segment repeatedly for every trip (604× in TriMet).
    r!("STM_014", Yuksek, Analytics, 3, &[], Some("route_id"), VA, Entity,
        "Excessive speed in segment"),
    r!("STM_015", Kritik, Spec, 2,
        // XFL_013/SHP_017 (2026-10-11) and STM_016/STM_017 (2026-10-13) were removed: no causality.
        &[],
        Some("trip_id"), VS_K, Row,
        "First stop is missing arrival_time"),
    r!("STM_016", Kritik, Spec, 2, &[], Some("trip_id"), VS_K, Row,
        "Last stop is missing arrival_time"),
    r_noscore!("STM_017", Orta,   Quality, 3, &[], Some("trip_id"), VS, Row,
        "Shape distance missing in stop times"),
    r!("STM_018", Orta,   Spec, 1, &[], Some("trip_id"), VS, Row,
        "continuous_pickup invalid (stop_times)"),
    r!("STM_019", Orta,   Spec, 1, &[], Some("trip_id"), VS, Row,
        "continuous_drop_off invalid (stop_times)"),
    r!("STM_020", Yuksek, Quality, 3, &[], Some("trip_id"), VS, Row,
        "Zero travel time (distance > 200m)"),
    r_noscore!("STM_021", Bilgi, Analytics, 3, &[], Some("trip_id"), VA, Row,
        "Distance between stops is zero or negative"),
    r!("STM_022", Orta,   Spec, 1, &[], None, VS, Feed,
        "timepoint invalid"),
    r!("STM_024", Bilgi,  Quality, 2, &[], Some("trip_id"), VS, Row,
        "shape_dist_traveled unit inconsistency"),
    r!("STM_025", Bilgi,  Analytics, 2, &[], Some("route_id"), VA, Entity,
        "Short segment timing"),
    r_noscore!("STM_026", Bilgi, Analytics, 2, &[], Some("trip_id"), VA, Row,
        "Excessive distance between stops"),
    r!("STM_028", Yuksek, Analytics, 2, &[], Some("trip_id"), VA, Row,
        "Trip duration too long"),
    r!("STM_029", Orta,   Analytics, 2, &[], Some("trip_id"), VA, Row,
        "Trip duration too short"),
    r!("STM_030", Dusuk,  Spec, 1, &[], Some("trip_id"), VS, Row,
        "shape_dist_traveled is negative or not a number"),
    // STM_032 CLASS CHANGED (#97, 2026-08-09): Quality → Spec. The requirement is normative and
    // appears in TWO places: the `stop_sequence` field definition *"The values must increase along the trip but do
    // not need to be consecutive"*, and the `stop_times.txt` heading *"Primary key (trip_id,
    // stop_sequence)"*. `DQ_021`, measuring the same fact in other files, is already Spec.
    // ⚠️ SEVERITY UNCHANGED (Low): the R1 gate is `Spec ∧ Kritik`, so publish behavior is unchanged.
    // The asymmetry with DQ_021 being HIGH was intentionally left as a SEPARATE task—changing severity
    // would affect the R5 score of 3 corpus feeds and requires measurement.
    r!("STM_032", Dusuk,  Spec, 1, &[], Some("trip_id"), VS, Row,
        "Duplicate stop_sequence within trip"),
    r!("STM_033", Yuksek, Interop, 2, &[], Some("trip_id"), VI, Entity,
        "Single-stop trip (unusable)"),
    r!("STM_034", Orta,   Interop, 2, &[], Some("trip_id"), VI, Row,
        "Only one of arrival or departure time defined"),
    r_noscore!("STM_035", Bilgi, Analytics, 1, &[], Some("trip_id"), VS, Row,
        "Same stop visited twice consecutively (terminal/loop)"),
    r!("STM_036", Bilgi,  Quality,   2, &[], None, VS, Feed,
        "stop_times not sorted by trip_id + stop_sequence (unsorted_stop_times)"),
    r!("STM_037", Yuksek, Spec, 1, &[], Some("trip_id"), VS, Entity,
        "arrival_time/departure_time prohibited in Flex window"),
    r!("STM_038", Yuksek, Interop, 1, &[], Some("trip_id"), VI, Entity,
        "start_pickup_drop_off_window >= end_pickup_drop_off_window"),
    r!("STM_039", Kritik, Spec, 1, &[], Some("trip_id"), VS_K, Entity,
        "Pickup/drop-off window missing in Flex context"),
    // pickup_booking_rule_id / drop_off_booking_rule_id are **Optional** in the OFFICIAL SPEC—
    // verbatim in the 27 April 2026 Schedule Reference: "Optional … Recommended when pickup_type=2".
    // Recommendation ≠ norm: treating absence as Kritik·Spec dropped a valid Flex feed from the
    // publish gate (R1 = Spec ∧ Kritik). Also, the trigger ("any window exists + both booking_rule_id
    // values are empty") is the project's tightening, not the spec recommendation itself → authority
    // ProjectQuality. Class Spec→Quality (2026-08-01, WP-1).
    r!("STM_040", Orta,   Quality, 1, &[], Some("trip_id"), VS, Entity,
        "pickup/drop_off_booking_rule_id missing in Flex stop_times (Optional in the spec)"),
    // STM_059: the spec says `pickup_booking_rule_id` is "Recommended when pickup_type=2"
    // (and its `drop_off_type=2` counterpart). `STM_040` instead looks when a FLEX WINDOW exists—
    // a different population: `pickup_type=2` ("call the agency by phone") is used in classic
    // dial-a-ride without a window. The conditions were not combined; the `emit_identity` gate
    // requires the rule title and emit to say the same thing.
    // Measured (239 feeds): 4 feeds, 27 rows—only in feeds carrying booking_rules.txt.
    // STM_060: GTFS Reference `stop_times.txt` prose—*"Simultaneous overlap of
    // locations.geojson id geometry, start/end_pickup_drop_off_window time, and pickup_type
    // or drop_off_type between two or more stop_times.txt records with the same trip_id is
    // forbidden."*
    //
    // 🔴 Until 2026-08-06 this requirement was marked **OUT OF SCOPE** in the ledger because
    // "polygon geometry is not stored". External review correctly showed this was an ARCHITECTURAL
    // GAP, not unverifiability: every input to the requirement is inside the feed.
    // Geometry is now carried from K1 to K4 (`EntityMap::geojson_geometries`).
    //
    // Severity `Yuksek`, NOT `Kritik`: the requirement is a PROHIBITION, but violation only
    // makes Flex-region semantics ambiguous; the R1 publish gate (pure `Spec ∧ Kritik`) would be disproportionate.
    r!("STM_060", Yuksek, Spec, 2, &[], Some("trip_id"), VS, Entity,
        "Zone, window and pickup/drop-off overlap at once within a trip"),
    r!("STM_061", Yuksek, Interop, 2, &[], Some("route_id"), VI, Entity,
        "Impossible speed between distant non-consecutive stops"),
    r!("STM_059", Dusuk, Quality, 1, &[], Some("trip_id"), VS, Entity,
        "booking_rule_id recommended when pickup_type/drop_off_type=2"),
    r!("STM_041", Yuksek, Spec, 1, &[], Some("trip_id"), VS, Entity,
        "stop_id and location_id/group_id cannot be used together"),
    r!("STM_042", Dusuk, Interop, 1, &[], Some("trip_id"), VI, Row,
        "stop_headsign contains characters unsupported by Google Transit"),
    r!("STM_043", Bilgi,  Analytics, 1, &[], Some("trip_id"), VA, Entity,
        "Trip has extreme stop count (>200)"),
    r!("STM_044", Bilgi,  Analytics, 1, &[], None, VA, Feed,
        "Feed stop_times exceeds 2,000,000 rows (WASM performance warning)"),
    r_noscore!("STM_045", Bilgi, Analytics, 1, &[], Some("trip_id"), VA, Entity,
        "Trip departure time exceeds the service day window"),
    r!("STM_046", Kritik, Spec, 1, &[], Some("trip_id"), VS_K, Row,
        "trip_id missing"),
    r!("STM_047", Kritik, Spec, 1, &[], Some("trip_id"), VS_K, Entity,
        "timepoint=1 without arrival/departure time"),
    r!("STM_048", Yuksek, Spec, 1, &[], Some("trip_id"), VS, Row,
        "After-midnight service-day times written as 00:xx (GTFS Spec)"),
    r!("STM_049", Yuksek, Spec, 1, &[], Some("trip_id"), VS, Row,
        "After-midnight departure written as 00:xx on the same row (GTFS Spec)"),
    r!("STM_050", Dusuk, Quality, 1, &[], None, VS, Feed,
        "timepoint column present but value empty"),
    r!("STM_051", Yuksek, Spec, 1, &[], Some("trip_id"), VS, Entity,
        "pickup_type 0/3 forbidden with Flex window"),
    r!("STM_052", Yuksek, Spec, 1, &[], Some("trip_id"), VS, Entity,
        "drop_off_type 0 forbidden with Flex window"),
    r!("STM_053", Bilgi, Analytics, 2, &[], Some("route_id"), VA, Entity,
        "Many consecutive stops have the same time"),
    r!("STM_054", Yuksek, Spec, 1, &[], Some("trip_id"), VS, Entity,
        "continuous_pickup forbidden with Flex window"),
    r!("STM_055", Yuksek, Spec, 1, &[], Some("trip_id"), VS, Entity,
        "continuous_drop_off forbidden with Flex window"),
    // MD parity: `decreasing_or_equal_stop_time_distance` (ERROR). GTFS Reference requires
    // an INCREASING value along stop_sequence for stop_times.shape_dist_traveled.
    r!("STM_056", Kritik, Spec, 1, &[], Some("trip_id"), VS_K, Row,
        "shape_dist_traveled does not increase"),
    // STM_058: Flex window fields are also `Time`; format errors used to be silently swallowed
    // (`.ok().flatten()`) → the value disappeared, STM_038 had nothing to compare, and entity
    // rules also fell silent because the row was considered "windowed". arrival_time was parsed
    // with the same helper and produced STM_003, but these two fields were not.
    // ONE rule, TWO fields: the fact is the same (Time could not be parsed); the message names the field.
    r!("STM_058", Kritik, Spec, 1, &["STM_038"], Some("trip_id"), VS_K, Entity,
        "Flex pickup/drop-off window time is invalid"),

    // ── PDW: Pickup/Drop-off Window ──────────────────────────────────────────
    r!("PDW_006", Orta, Analytics, 2, &[], Some("trip_id"), VA, Entity,
        "Overlapping pickup/drop-off window for same trip+zone"),

    // ── LOC: locations.geojson ───────────────────────────────────────────────
    r!("LOC_001", Yuksek, Spec, 2, &[], None, VS, File,
        "Unknown or invalid geometry type in locations.geojson"),
    // LOC_012 (16th corpus run, 2026-09-11): `LOC_001` carried FIVE DISTINCT facts—
    // file unreadable · JSON unparsable · root type not FeatureCollection ·
    // no `features` array · unsupported feature geometry type. The localization layer has ONE
    // template per rule (`i18n.rs::translate`), and that template said "Invalid geometry type".
    // Result: for `tdg-81648`..`tdg-81652`, when the file was NOT JSON at all
    // (`expected value at line 1 column 1`), en/fr/ja output claimed "invalid geometry type".
    // Document-level defects moved here; `LOC_001` now remains only on the `geometry.type`
    // branch and its title is CORRECT.
    // 🔑 ARC_001 pattern: the localized template states the invariant; detail remains in the Turkish message.
    r!("LOC_012", Yuksek, Spec, 2, &[], None, VS, File,
        "locations.geojson is not a valid GeoJSON FeatureCollection"),
    r!("LOC_002", Kritik, Spec, 2, &[], None, VS_K, File,
        "Feature has null or missing geometry"),
    r!("LOC_003", Kritik, Spec, 2, &[], None, VS_K, File,
        "Feature missing required 'id' property"),
    r!("LOC_004", Orta,   Spec, 2, &[], None, VS, File,
        "Polygon ring is not closed"),
    r!("LOC_005", Dusuk,  Quality, 1, &[], None, VS, File,
        "FeatureCollection has no features"),
    r!("LOC_006", Bilgi,  Quality, 1, &[], None, VS, File,
        "Polygon area exceeds 500km²"),
    r!("LOC_007", Orta,   Spec,    2, &[], None, VS, File,
        "Duplicate feature 'id' in FeatureCollection"),
    // The spec (locations.geojson field table) marks type/properties/geometry.coordinates as Required
    // for every Feature. They were not checked at all until 2026-08-01: when missing, the code
    // silently skipped them and the file appeared "validated".
    r!("LOC_008", Orta,   Spec,    2, &[], None, VS, File,
        "Feature 'type' member missing or not \"Feature\""),
    r!("LOC_009", Orta,   Spec,    2, &[], None, VS, File,
        "Feature 'properties' object missing"),
    // LOC_011: for `locations.geojson`, the spec says "Each polygon must be valid by the definition of
    // the OpenGIS Simple Features Specification, section 6.1.11". `LOC_004` measured only ring
    // CLOSURE; the core of 6.1.11 is that the ring is SIMPLE (does not self-intersect) and
    // holes remain INSIDE the exterior ring.
    //
    // ⚠️ Detection uses `robust::orient2d`, NOT naive f64. Measured (2026-08-06):
    // on the same 21 polygons, epsilon/implementation choices produce results from 5 to 17
    // and are not even monotonic—this rule cannot be written reliably with naive arithmetic.
    r!("LOC_011", Yuksek, Spec,    2, &[], None, VS, File,
        "Invalid polygon: a ring self-intersects or a hole lies outside"),
    r!("LOC_010", Kritik, Spec,    2, &[], None, VS_K, File,
        "Geometry 'coordinates' missing or not an array — the zone geometry cannot be resolved"),

    // ── CAL: Calendar ──────────────────────────────────────────────────────────
    r!("CAL_001", Kritik, Spec, 1,
        &[],
        Some("service_id"), VS_K, Entity,
        "Duplicate service_id"),
    r!("CAL_002", Kritik, Spec, 1,
        &["CAL_006","OPR_011"],
        Some("service_id"), VS_K, Entity,
        "Calendar day field invalid value"),
    r!("CAL_003", Kritik, Spec, 2,
        &["CAL_005"],
        Some("service_id"), VS_K, Entity,
        "start_date invalid"),
    r!("CAL_004", Kritik, Spec, 2,
        &["CAL_005","CAL_010"],
        Some("service_id"), VS_K, Entity,
        "end_date invalid"),
    r!("CAL_005", Kritik, Interop, 2, &[], Some("service_id"), VI, Entity,
        "start_date is after end_date"),
    r!("CAL_006", Bilgi,  Quality, 2, &[], Some("service_id"), VS, Entity,
        "Weekly schedule has all days disabled"),
    r!("CAL_007", Bilgi,  Analytics, 2, &[], None, VA, Entity,
        "Gap in service period"),
    r!("CAL_008", Bilgi,  Analytics, 2, &[], None, VA, Feed,
        "Service expires soon"),
    r_noscore!("CAL_009", Bilgi, Analytics, 1, &[], None, VA, Feed,
        "All feed services have expired"),
    r!("CAL_010", Orta,   Analytics, 2, &[], Some("service_id"), VA, Entity,
        "Service has too few active days"),
    r!("CAL_011", Dusuk,  Quality, 1, &[], Some("service_id"), VS, Entity,
        "Unused service"),
    r!("CAL_012", Bilgi,  Analytics, 2, &[], None, VA, Entity,
        "Service gap in the near future"),
    r!("CAL_013", Bilgi,  Analytics, 1, &[], None, VA, Entity,
        "Expired service period"),
    // CAL_014 (2026-10-11): same active-date semantics as MD `service_window_outside_feed_period` (INFO)
    // → single carrier. Bilgi because the spec recommends data outside the window.
    r_noscore!("CAL_014", Bilgi,  Interop,   1, &[], Some("service_id"), VI, Entity,
        "Service dates outside feed_info validity range"),
    r_noscore!("CAL_015", Bilgi, Analytics, 1, &[], None, VA, Feed,
        "All calendar dates in the future (no active trips today)"),
    r!("CAL_016", Bilgi,  Quality,   1, &[], None, VS, Feed,
        "Service extends to a very distant future date"),
    r_noscore!("CAL_017", Bilgi, Analytics, 1, &[], Some("service_id"), VA, Entity,
        "Calendar has not yet started (all active dates in the future)"),
    r!("CAL_018", Bilgi,  Quality,   2, &[], Some("service_id"), VS, Entity,
        "Service has no active weekdays (all days 0, none overridden by calendar_dates)"),
    // CAL_019 (2026-10-11): retired. It compared the raw calendar range with the feed_info window
    // and reported the same fact as CAL_014 a second time. The ID must not be reused.
    r!("CAL_020", Bilgi,  Quality,   1, &[], None, VS, Feed,
        "Feed validity window exceeds 5 years"),
    r!("CAL_021", Bilgi,  Analytics, 1, &[], Some("service_id"), VA, Entity,
        "Active today but no service in the upcoming days"),
    r!("CAL_022", Kritik, Spec, 1, &[], Some("service_id"), VS_K, Row,
        "service_id missing"),
    r!("CAL_023", Orta, Quality, 2, &[], Some("service_id"), VA, Entity,
        "Calendar end_date far in the future (suspect)"),
    // CAL_024: former TRP_030 (moved to the CAL group because it focuses on calendars, with #23).
    r_noscore!("CAL_024", Bilgi, Analytics, 1, &[], Some("service_id"), VS, Entity,
        "Calendar inactive in the next 7 days"),
    r!("CAL_025", Kritik, Spec, 1, &[], Some("service_id"), VS_K, Row,
        "Calendar day field is empty"),

    // ── CLD: Calendar Dates ────────────────────────────────────────────────────
    r!("CLD_001", Kritik, Spec, 1, &[], Some("service_id"), VS_K, Row,
        "service_id missing"),
    r!("CLD_002", Kritik, Spec, 2, &[], Some("service_id"), VS_K, Row,
        "date invalid format"),
    r!("CLD_003", Kritik, Spec, 1,
        &["CLD_005","CLD_006","CLD_007"],
        None, VS_K, File,
        "exception_type missing or invalid"),
    r!("CLD_004", Yuksek, Quality, 2, &[], Some("service_id"), VS, Row,
        "Calendar-dates-only service has no active dates"),
    r!("CLD_005", Bilgi, Quality, 2, &[], Some("service_id"), VS, Row,
        "Date out of range"),
    r_noscore!("CLD_006", Bilgi, Analytics, 1, &[], Some("service_id"), VA, Row,
        "Too many exception days"),
    r!("CLD_007", Bilgi,  Analytics, 1, &[], Some("service_id"), VA, Row,
        "Excessive calendar exceptions"),

    // ── SHP: Shapes ────────────────────────────────────────────────────────────
    r!("SHP_001", Kritik, Spec, 1, &[], Some("shape_id"), VS_K, Entity,
        "shape_id missing"),
    r!("SHP_002", Kritik, Spec, 1,
        &["SHP_003","SHP_014","SHP_016","SHP_017"],
        Some("shape_id"), VS_K, Entity,
        "shape_pt_lat missing or invalid"),
    r!("SHP_003", Kritik, Spec, 1, &[], Some("shape_id"), VS_K, Entity,
        "shape_pt_lon missing or invalid"),
    r!("SHP_004", Kritik, Spec, 2,
        &["SHP_009","SHP_012","GEO_006","GEO_007"],
        Some("shape_id"), VS_K, Feed,
        "shape_pt_sequence missing or invalid"),
    r!("SHP_005", Kritik, Spec, 2,
        &[], // Grouped at feed level: the link suppressed every shape (2026-10-13).
        Some("shape_id"), VS_K, Row,
        "shape_dist_traveled decreases"),
    r!("SHP_006", Dusuk,  Quality, 2,
        &["SHP_017"],
        Some("shape_id"), VS, Entity,
        "Shape consists of a single point only"),
    r!("SHP_008", Kritik, Spec, 3, &[], Some("shape_id"), VS_K, Entity,
        "Duplicate shape_pt_sequence"),
    r!("SHP_009", Bilgi,  Analytics, 3, &[], Some("shape_id"), VA, Entity,
        "Shape self-intersects"),
    r!("SHP_010", Dusuk,  Quality, 1, &[], Some("shape_id"), VS, Entity,
        "Repeated shape point (consecutive identical coordinates)"),
    r!("SHP_012", Yuksek, Analytics, 3, &[], Some("shape_id"), VA, Entity,
        "Shape too far from trip stops"),
    r!("SHP_014", Bilgi,  Analytics, 3, &[], Some("shape_id"), VA_GEO, Entity,
        "First or last stop far from shape endpoint"),
    r!("SHP_015", Orta,   Quality, 2, &[], Some("shape_id"), VS, Entity,
        "Shape has statistically too few points"),
    r!("SHP_016", Yuksek, Quality, 2, &[], Some("shape_id"), VS_GEO, Entity,
        "Shape direction inconsistent with trip direction"),
    r!("SHP_017", Bilgi,  Analytics, 3, &[], Some("shape_id"), VA_GEO, Entity,
        "Stop sequence conflicts with shape"),
    r!("SHP_018", Dusuk,  Quality, 1, &[], Some("shape_id"), VS, Entity,
        "Shape not referenced by any trip"),
    r!("SHP_019", Orta,   Quality, 2, &[], Some("shape_id"), VS, Entity,
        "Shape's trips have no stop times"),
    r!("SHP_020", Bilgi,  Analytics, 3, &[], Some("shape_id"), VA, Entity,
        "Repeated point in shape"),
    r!("SHP_021", Dusuk,  Spec, 1, &[], Some("shape_id"), VS, Entity,
        "shape_dist_traveled is negative or not a number"),
    r!("SHP_022", Yuksek, Quality, 2, &[], Some("stop_id"), VS, Entity,
        "Stop position ambiguous on shape"),
    r!("SHP_023", Orta,   Quality, 2, &[], Some("shape_id"), VS, Entity,
        "Consecutive points with same shape_dist_traveled at same coordinates"),
    r!("SHP_024", Orta,   Quality, 2, &[], Some("stop_id"), VS, Entity,
        "Stop-to-shape distance inconsistent with shape_dist_traveled"),
    r!("SHP_025", Orta,   Quality, 2, &[], Some("trip_id"), VS, Entity,
        "Trip stop_times distance exceeds total shape distance"),
    r!("SHP_026", Bilgi,  Analytics, 1, &[], Some("shape_id"), VA_GEO, Entity,
        "Shape has extreme point count (>5,000)"),
    r!("SHP_028", Yuksek, Spec,    2, &[], Some("shape_id"), VS, Entity,
        "Same shape_dist_traveled with different coordinates"),
    r!("SHP_029", Bilgi,  Quality, 2, &[], Some("shape_id"), VS, Entity,
        "Same shape_dist_traveled, tiny coordinate difference"),
    r!("SHP_030", Orta,   Quality, 2, &[], Some("shape_id"), VS, Entity,
        "stop_times uses shape_dist_traveled but shape distances are missing"),

    // ── FRQ: Frequencies ───────────────────────────────────────────────────────
    r!("FRQ_001", Kritik, Spec, 1, &[], Some("trip_id"), VS_K, Row,
        "trip_id not found"),
    r!("FRQ_002", Kritik, Spec, 2, &[], Some("trip_id"), VS_K, Row,
        "start_time invalid"),
    r!("FRQ_003", Kritik, Spec, 2, &[], Some("trip_id"), VS_K, Row,
        "end_time invalid"),
    r!("FRQ_004", Kritik, Spec, 2, &[], Some("trip_id"), VS_K, Row,
        "headway_secs missing or invalid"),
    r!("FRQ_005", Kritik, Quality, 2, &[], Some("trip_id"), VS, Row,
        "end_time is before start_time"),
    r!("FRQ_006", Orta,   Analytics, 2, &[], Some("trip_id"), VA, Row,
        "headway_secs too long"),
    r!("FRQ_007", Orta,   Spec, 1, &[], None, VS, Feed,
        "exact_times invalid"),
    r!("FRQ_008", Kritik, Spec, 3, &[], Some("trip_id"), VS_K, Row,
        "headway_secs is zero (invalid frequency)"),
    r!("FRQ_009", Orta,   Quality, 3, &[], Some("trip_id"), VS, Row,
        "Frequency interval too short"),
    r!("FRQ_010", Bilgi,  Analytics, 1, &[], Some("trip_id"), VA, Row,
        "Very high frequency (bunching risk)"),
    // CLASS CHANGED (#96, 2026-08-09): evidence class ↔ authority of the requirement. Only
    // this rule measured a strict Spec requirement, but the rule was not in the Spec class;
    // users filtering for Spec could not see the violation. Severity UNCHANGED → the R1 gate (`Spec ∧ Kritik`) is unchanged.
    // Ruling: Pec6b6920.
    r!("FRQ_011", Yuksek, Spec, 2, &[], Some("trip_id"), VS, Row,
        "Overlapping frequency periods"),
    // FRQ_012: for `exact_times=1` the spec says "the end_time value must be greater than the last
    // desired trip start_time but less than the last desired trip start_time + headway_secs"
    // → `(end - start)` cannot be an exact multiple of headway. `FRQ_005` measures only `end <= start`.
    // ⚠️ TENSION: the same section says "New headways may start at the exact time the previous headway
    // ends"; adjacent periods are GENERALLY allowed. The `exact_times=1` constraint NARROWS this
    // general permission (a special rule limits the general rule)—the card documents the tension.
    // Measured (239 feeds): 4 feeds, 452 rows.
    r!("FRQ_012", Dusuk, Spec, 1, &[], Some("trip_id"), VS, Row,
        "exact_times=1 with end_time on a headway boundary"),

    // ── TRF: Transfers ─────────────────────────────────────────────────────────
    r!("TRF_001", Kritik, Spec, 1, &[], None, VS_K, Row,
        "from_stop_id missing"),
    r!("TRF_002", Kritik, Spec, 1, &[], None, VS_K, Row,
        "to_stop_id missing"),
    r!("TRF_003", Kritik, Spec, 1,
        &["TRF_004","TRF_010","TRF_011"],
        Some("from_stop_id|to_stop_id"), VS_K, Row,
        "from_stop_id or to_stop_id not found"),
    r!("TRF_004", Yuksek, Spec, 2, &[], Some("from_stop_id|to_stop_id"), VS, Row,
        "transfer_type invalid"),
    r!("TRF_005", Kritik, Spec, 2, &[], None, VS_K, Feed,
        "min_transfer_time missing"),
    r!("TRF_006", Kritik, Spec, 1,
        &["OPR_007"],
        Some("from_trip_id"), VS_K, Row,
        "from_trip_id not found"),
    r!("TRF_007", Kritik, Spec, 1,
        &["OPR_007"],
        Some("to_trip_id"), VS_K, Row,
        "to_trip_id not found"),
    r!("TRF_008", Kritik, Spec, 1, &[], Some("from_route_id"), VS_K, Row,
        "from_route_id not found"),
    r!("TRF_009", Kritik, Spec, 1, &[], Some("to_route_id"), VS_K, Row,
        "to_route_id not found"),
    r!("TRF_010", Orta,   Analytics, 2, &[], Some("from_stop_id|to_stop_id"), VA, Row,
        "Transfer time too long"),
    r!("TRF_011", Bilgi,  Quality, 2, &[], Some("from_stop_id|to_stop_id"), VS, Row,
        "Transfer defined but distance is far"),
    // GTFS Reference transfers.txt is NORMATIVE: "Primary key (from_stop_id, to_stop_id,
    // from_trip_id, to_trip_id, from_route_id, to_route_id)". Primary-key uniqueness is a spec
    // requirement → NOT Quality, Spec. TRF_012 is the trip/route-free subcase of this key.
    r!("TRF_012", Kritik, Spec, 1, &[], None, VS_K, Row,
        "Duplicate transfer record"),
    r!("TRF_013", Kritik, Quality, 2, &[], None, VS, Row,
        "Transfer type inconsistent with context"),
    r!("TRF_014", Yuksek, Spec, 2, &[], None, VS, Row,
        "No trip for in-seat transfer"),
    r!("TRF_015", Yuksek, Quality, 2, &[], None, VS, Row,
        "in-seat transfer invalid"),
    // The same primary-key requirement, in its full combination. MD counterpart `duplicate_key` (ERROR);
    // MD 10 in Entur mdb-1078, this project 10 → exact parity. Class is Spec, NOT Interop,
    // because the spec text determines the class, not MD parity alone.
    r!("TRF_016", Kritik, Spec, 2, &[], None, VS_K, Row,
        "Transfer condition conflicting"),
    // XFL_020 checks the same (trip, route) mismatch; TRF_017 is the root rule and suppresses
    // XFL_020 as a symptom to prevent duplicate reporting.
    // CLASS CHANGED (#96, 2026-08-09): evidence class ↔ authority of the requirement. Only this
    // rule measured a strict Spec requirement, but was not in the Spec class; Spec-filtering users
    // could not see the violation. Severity UNCHANGED → the R1 gate (`Spec ∧ Kritik`) is unchanged.
    // Ruling: P0fa71358·P585a24d4·P134a2d40·P446972ca.
    r!("TRF_017", Yuksek, Spec, 2, &["XFL_020"], None, VS, Row,
        "Trip transfer on wrong route"),
    r!("TRF_018", Orta,   Quality, 2, &[], None, VS, Row,
        "Trip transfer references the same trip"),
    r!("TRF_019", Orta, Interop, 2, &[], None, VI, Feed,
        "Different route_type in in-seat transfer"),
    r!("TRF_020", Orta, Quality, 2, &[], Some("from_stop_id|to_stop_id"), VS, Row,
        "Transfer walking speed is too fast"),
    r!("TRF_021", Kritik, Spec, 1, &[], Some("from_stop_id|to_stop_id"), VS_K, Row,
        "Transfer endpoint is not a stop or station"),
    // TRF_022/023: GTFS Reference `transfers.txt` → "Linked trips". The spec uses uppercase `MUST`
    // and the prose catalog could not see it until 2026-08-06 (it searched for lowercase `must`
    // with a case-sensitive regex) → three strict requirements were never measured.
    //
    // ⚠️ Severity `Yuksek`, NOT `Kritik`—deliberate. The R1 publish gate is pure `Spec ∧ Kritik`;
    // an ambiguous continuation is a real spec violation but only breaks linked-trip semantics,
    // not the rest of the feed. Blocking publication would be disproportionate.
    //
    // ⚠️ They are SEPARATE rules because the measurements are separate: one groups by `from_trip_id`
    // and checks the calendars of `to_trip_id`, the other does the reverse. The fix is on a different side.
    // The spec's "n-to-n continuations must respect both constraints" sentence is satisfied by both working TOGETHER.
    r!("TRF_022", Yuksek, Spec, 2, &[], Some("trip_id"), VS, Entity,
        "Conflicting calendars in a 1-to-n trip continuation"),
    r!("TRF_023", Yuksek, Spec, 2, &[], Some("trip_id"), VS, Entity,
        "Conflicting calendars in an n-to-1 trip continuation"),

    // ── GGL: Google Transit Compatibility Rules ──────────────────────────────
    r!("GGL_001", Dusuk, Interop, 1, &[], None, VI, Feed,
        "transfer_type=4/5 not supported by Google Transit"),
    r!("GGL_002", Dusuk, Interop, 1, &[], Some("fare_id"), VI, Row,
        "ic_price (Google-specific) invalid value"),

    // ── FAR: Fare Attributes ───────────────────────────────────────────────────
    r!("FAR_001", Kritik, Spec, 1,
        &["FRL_001"],
        Some("fare_id"), VS_K, Entity,
        "Duplicate fare_id"),
    r!("FAR_002", Kritik, Spec, 1, &[], Some("fare_id"), VS_K, Entity,
        "price missing or invalid"),
    r!("FAR_003", Kritik, Spec, 1, &[], Some("fare_id"), VS_K, Entity,
        "currency_type missing"),
    r!("FAR_004", Kritik, Spec, 1, &[], Some("fare_id"), VS_K, Entity,
        "payment_method invalid"),
    r!("FAR_005", Kritik, Spec, 1, &[], Some("fare_id"), VS_K, Entity,
        "transfers invalid"),
    r!("FAR_006", Orta,   Spec, 1, &[], Some("fare_id"), VS, Entity,
        "transfer_duration invalid"),
    r!("FAR_008", Kritik, Spec, 1, &[], Some("fare_id"), VS_K, Entity,
        "agency_id not found"),
    r!("FAR_010", Orta,   Quality, 2, &[], None, VS, Feed,
        "Overlapping fare rules"),
    r!("FAR_011", Kritik, Spec, 1, &[], Some("fare_id"), VS_K, Entity,
        "payment_method missing"),
    // FAR_013 — `Dusuk·Spec` → `Bilgi·Quality` on 2026-09-11. Three measurements led here:
    //
    // 1. THE REQUIREMENT IS NOT WRITTEN FOR THIS FIELD. The only requirement containing the ISO 4217 decimal sentence
    //    is `P38cd4e78`, in the `fare_products` section, for `amount`. The `fare_attributes`
    //    section has NO decimal requirement; the spec has no sentence for `price`/`currency_type`.
    //    The field types also differ: `amount` = "Currency amount" (the decimal requirement is a
    //    property of the TYPE), `price` = "Non-negative float". The rule transferred the condition by analogy.
    // 2. NO BRANCH FINDS A REAL DATA ERROR. A missing digit (`2.8` USD) is the same amount.
    //    Extra digits (`100.50` JPY) are NOT invalid either: zero decimal places state the cash
    //    denomination; calculating a fare to finer precision than that denomination is not an error
    //    (the fuel price per liter precedent). The only serious danger is a 100× unit error (`150` =
    //    150 cents or 150 dollars), but the rule CANNOT distinguish it, so the signal does not lead to action.
    // 3. THE SPEC'S OWN REQUIREMENT IS ALSO A FORMAT RULE. *"...must contain the number of decimal
    //    places specified by the norm ISO 4217..."* says nothing about value validity. `FPD_007` also
    //    measures format; the difference is that the spec EXPLICITLY requires it there.
    //
    // Measurement (16th corpus): 394 feeds / 2,166,496 rows, median 2 rows per feed, one row in 190 feeds.
    // MD counterpart 0—and cannot be otherwise: none of the 394 feeds has Fares V2, while MD's
    // `invalid_currency_amount` rule checks `fare_products.amount`. MD silence means not
    // "I checked and it is clean" but "I have no rule to check it" → MD is NOT THE ARBITER here.
    // Since `Bilgi` has weight 0.0, it never penalizes a score; the observation remains in the report,
    // and the defect claim is removed. Class `Quality` is REQUIRED: the registry gate forbids `Spec ∧ Bilgi`.
    r!("FAR_013", Bilgi, Quality, 1, &[], None, VS, File,
        "price does not carry the currency's ISO 4217 decimal places"),
    r!("FAR_012", Kritik, Spec, 1, &[], Some("fare_id"), VS_K, Entity,
        "fare_id missing"),

    // ── FRL: Fare Rules ────────────────────────────────────────────────────────
    r!("FRL_001", Kritik, Spec, 1, &[], Some("fare_id"), VS_K, Row,
        "fare_id not found"),
    r!("FRL_002", Kritik, Spec, 1, &[], Some("route_id"), VS_K, Row,
        "route_id not found"),
    r!("FRL_003", Kritik, Spec, 1, &[], Some("fare_id"), VS_K, Row,
        "origin_id invalid"),
    r!("FRL_004", Kritik, Spec, 1, &[], Some("fare_id"), VS_K, Row,
        "destination_id invalid"),
    r!("FRL_005", Kritik, Spec, 1, &[], Some("fare_id"), VS_K, Row,
        "contains_id invalid"),
    r!("FRL_006", Bilgi,  Quality, 1, &[], None, VS, Feed,
        "No fare rules defined"),
    r!("FRL_007", Orta,   Quality, 3, &[], Some("fare_id"), VS, Row,
        "Fare rule logical inconsistency"),
    r!("FRL_008", Bilgi,  Quality, 3, &[], None, VS, Feed,
        "No fare defined for all routes"),
    r!("FRL_009", Orta,   Quality, 3, &[], None, VS, Feed,
        "Origin/destination fare coverage incomplete"),

    // ── RCT: Rider Categories (Fares v2) ──────────────────────────────────────
    r!("RCT_001", Kritik, Spec, 1, &[],
        Some("rider_category_id"), VS_K, Row,
        "Duplicate rider_category_id"),
    r!("RCT_002", Kritik, Spec, 1, &[], Some("rider_category_id"), VS_K, Row,
        "rider_category_name missing"),
    r!("RCT_003", Kritik, Spec, 1, &[], Some("rider_category_id"), VS_K, Row,
        "is_default_fare_category invalid"),
    // min_age/max_age are NOT OFFICIAL GTFS FIELDS—in the 27 April 2026 Schedule Reference,
    // rider_categories.txt carries only rider_category_id / rider_category_name /
    // is_default_fare_category / eligibility_url. The fields exist as extensions in some feeds
    // and reading them is harmless; the WRONG part was treating them as a spec violation.
    // Class Spec→Quality (2026-08-01): Spec is legitimate ONLY for an official norm and feeds
    // the Spec publish-score gate; extension fields must not enter it.
    r!("RCT_004", Orta,   Quality, 1, &[], Some("rider_category_id"), VS, Row,
        "min_age or max_age invalid (GTFS extension field — not in the official spec)"),
    r!("RCT_005", Orta,   Quality, 1, &[], Some("rider_category_id"), VS, Row,
        "max_age less than min_age"),
    r!("RCT_006", Orta,   Spec, 2, &[], Some("fare_product_id"), VS, Entity,
        "fare_product does not have exactly one default rider category"),
    // RCT_007: spec type `URL`. Every sibling URL field has a format rule
    // (stop_url→STP_042, route_url→RTS_005, agency_fare_url→AGN_008); this one did not.
    // ⚠️ The field name was corrected on 2026-08-01—the parser used to read the non-spec
    // `rider_category_eligibility_url` column.
    r!("RCT_007", Dusuk,  Spec, 1, &[], Some("rider_category_id"), VS, Entity,
        "eligibility_url is invalid"),
    r!("RCT_008", Kritik, Spec, 1, &[], Some("rider_category_id"), VS_K, Row,
        "rider_category_id is empty"),

    // ── FMD: Fare Media (Fares v2) ────────────────────────────────────────────
    r!("FMD_001", Kritik, Spec, 1, &[],
        Some("fare_media_id"), VS_K, Row,
        "Duplicate fare_media_id"),
    r!("FMD_002", Kritik, Spec, 1, &[], Some("fare_media_id"), VS_K, Row,
        "fare_media_type missing or invalid"),
    r!("FMD_003", Dusuk, Quality, 1, &[], Some("fare_media_id"), VS, Row,
        "fare_media_name recommended for TransitCard/MobileApp"),
    r!("FMD_004", Kritik, Spec, 1, &[], Some("fare_media_id"), VS_K, Row,
        "fare_media_id is empty"),

    // ── FPD: Fare Products (Fares v2) ─────────────────────────────────────────
    r!("FPD_001", Kritik, Spec, 1, &[],
        Some("fare_product_id|rider_category_id|fare_media_id"), VS_K, Row,
        "Duplicate fare_products composite key"),
    r!("FPD_002", Kritik, Spec, 1, &[], Some("fare_product_id"), VS_K, Row,
        "amount missing or not numeric"),
    r!("FPD_003", Kritik, Spec, 1, &[], Some("fare_product_id"), VS_K, Row,
        "currency invalid ISO 4217 code"),
    r!("FPD_004", Kritik, Spec, 1, &[], Some("fare_product_id"), VS_K, Row,
        "fare_media_id not found"),
    r!("FPD_005", Kritik, Spec, 1, &[], Some("fare_product_id"), VS_K, Row,
        "rider_category_id not found"),
    // ISO 4217 minor unit—spec: "The currency amount must contain the number of decimal
    // places specified by the norm ISO 4217 for the accompanying Currency code."
    //
    // ⚠️ MEASUREMENT DETERMINED THIS RULE'S SHAPE. Full spec application matches 2,001,806 rows
    // in 239 feeds—values such as `EUR 0.9` and `HKD 6.7` are technically violations but lose no
    // meaning, and two feeds produce 99.7% of the total. Therefore:
    //   • emit ONE FILE-LEVEL SUMMARY (DQ_016/ARC_032 pattern)—38 notices, not 2M,
    //   • severity LOW: the R1 publish gate is pure `Spec ∧ Kritik`, so this rule NEVER reaches it.
    // Class remains `Spec`: the requirement is explicit in the spec, not a recommendation.
    r!("FPD_007", Dusuk, Spec, 1, &[], None, VS, File,
        "amount does not carry the currency's ISO 4217 decimal places"),
    // FPD_006 REMOVED (2026-08-09): an empty rider_category_id is not "default";
    // it means the product is suitable for any rider category. Repetition of the same triple
    // (fare_product_id, rider_category_id, fare_media_id) belongs to FPD_001;
    // the actual default category is measured by rider_categories.is_default_fare_category (RCT_006).

    // ── FLG: Fare Leg Rules (Fares v2) ────────────────────────────────────────
    r!("FLG_001", Kritik, Spec, 1, &[], Some("leg_group_id"), VS_K, Row,
        "fare_product_id not found"),
    r!("FLG_002", Kritik, Spec, 1, &[], Some("leg_group_id"), VS_K, Row,
        "network_id not found"),
    r!("FLG_003", Kritik, Spec, 1, &[], Some("leg_group_id"), VS_K, Row,
        "from_area_id not found"),
    r!("FLG_004", Kritik, Spec, 1, &[], Some("leg_group_id"), VS_K, Row,
        "to_area_id not found"),
    r!("FLG_005", Kritik, Spec, 1, &[], Some("leg_group_id"), VS_K, Row,
        "from_timeframe_group_id not found"),
    r!("FLG_006", Kritik, Spec, 1, &[], Some("leg_group_id"), VS_K, Row,
        "to_timeframe_group_id not found"),
    r!("FLG_007", Orta,   Spec, 1, &[], Some("leg_group_id"), VS, Row,
        "rule_priority invalid"),
    // FLG_008: `fare_product_id` is EMPTY. `FLG_001` is an FK check and never runs for an empty
    // value because of the `!is_empty()` guard; all siblings are FKs or enums. 16 feeds (#153).
    r!("FLG_008", Kritik, Spec, 1, &[], Some("leg_group_id"), VS_K, Row,
        "fare_product_id missing"),

    // ── FLJ: Fare Leg Join Rules (Fares v2) ───────────────────────────────────
    // `fare_leg_join_rules.txt` was NOT KNOWN to the pipeline until 2026-08-02 (issue #59):
    // it was absent from KNOWN_FILES → an "unknown file" warning, no parser, and no rule.
    // The UI file map already drew the node—the inconsistency was closed in the same change.
    //
    // Each rule measures both presence and resolution of ONE field (same pattern as NET_002/003).
    // Network fields are unconditionally Required; stop fields are MUTUALLY conditional—the spec says
    // "Required if to_stop_id is defined. Optional otherwise." and the symmetric condition.
    r!("FLJ_001", Kritik, Spec, 1, &[], Some("from_network_id"), VS_K, Row,
        "from_network_id missing or not found"),
    r!("FLJ_002", Kritik, Spec, 1, &[], Some("to_network_id"), VS_K, Row,
        "to_network_id missing or not found"),
    r!("FLJ_003", Kritik, Spec, 1, &[], Some("from_stop_id"), VS_K, Row,
        "from_stop_id missing or not found"),
    r!("FLJ_004", Kritik, Spec, 1, &[], Some("to_stop_id"), VS_K, Row,
        "to_stop_id missing or not found"),

    // ── FTR: Fare Transfer Rules (Fares v2) ───────────────────────────────────
    r!("FTR_001", Kritik, Spec, 1, &[], Some("from_leg_group_id"), VS_K, Row,
        "fare_transfer_type missing or invalid"),
    r!("FTR_002", Kritik, Spec, 1, &[], Some("from_leg_group_id"), VS_K, Row,
        "from_leg_group_id not found"),
    r!("FTR_003", Kritik, Spec, 1, &[], Some("to_leg_group_id"), VS_K, Row,
        "to_leg_group_id not found"),
    r!("FTR_004", Kritik, Spec, 1, &[], Some("from_leg_group_id"), VS_K, Row,
        "fare_product_id not found"),
    r!("FTR_005", Kritik, Spec, 1, &[], Some("from_leg_group_id"), VS_K, Row,
        "duration_limit_type invalid"),
    r!("FTR_006", Orta,   Spec, 1, &[], Some("from_leg_group_id"), VS, Row,
        "duration_limit invalid"),
    r!("FTR_007", Orta,   Spec, 1, &[], Some("from_leg_group_id"), VS, Row,
        "duration_limit_type defined without duration_limit"),
    r!("FTR_008", Orta,   Spec, 1, &[], Some("from_leg_group_id"), VS, Row,
        "transfer_count invalid"),
    r!("FTR_009", Orta,   Spec, 1, &[], Some("from_leg_group_id"), VS, Row,
        "transfer_count required when leg groups equal"),
    r!("FTR_010", Orta,   Spec, 1, &[], Some("from_leg_group_id"), VS, Row,
        "transfer_count forbidden when leg groups differ"),
    r!("FTR_011", Orta,   Spec, 1, &[], Some("from_leg_group_id"), VS, Row,
        "duration_limit set but duration_limit_type missing"),

    // ── ARS: Areas (Fares v2) ─────────────────────────────────────────────────
    r!("ARS_001", Kritik, Spec, 1, &["SAR_001"],
        Some("area_id"), VS_K, Row,
        "Duplicate area_id"),
    // ── presence:required gaps (anchor-granularity triage, 2026-08-05) ────────
    // Measured: rules titled "X duplicated" and "X not found" NEVER see EMPTY VALUES.
    // Duplicate checks skip empty keys (correct—empties are not copies of each other), and FK
    // resolution treats empty as "no reference" and passes. The requirement between them—"this
    // field is required"—fell through the gap. `presence_probe.py` tested each with an empty value.
    r!("ARS_002", Kritik, Spec, 1, &[], Some("area_id"), VS_K, Row,
        "areas.txt has an empty area_id"),

    // ── SAR: Stop Areas (Fares v2) ────────────────────────────────────────────
    r!("SAR_001", Kritik, Spec, 1, &[], Some("area_id"), VS_K, Row,
        "area_id not found"),
    r!("SAR_002", Kritik, Spec, 1, &[], Some("stop_id"), VS_K, Row,
        "stop_id not found"),
    r!("SAR_003", Kritik, Spec, 1, &[], Some("area_id"), VS_K, Row,
        "stop_areas has an empty area_id"),
    r!("SAR_004", Kritik, Spec, 1, &[], Some("stop_id"), VS_K, Row,
        "stop_areas has an empty stop_id"),

    // ── NET: Networks (Fares v2) ──────────────────────────────────────────────
    r!("NET_001", Kritik, Spec, 1, &[],
        Some("network_id"), VS_K, Row,
        "Duplicate network_id"),
    // NET_002/003: both route_networks.txt fields are Required and Foreign ID in the spec.
    // The file was parsed but NOT checked until 2026-08-01: a row referencing a nonexistent route
    // or network passed silently—the route drops from its network and fare rules do not apply to it.
    // Each rule measures both presence and resolution of one field; an empty value is also an
    // "unresolved" state, and the message names which field it is.
    r!("NET_002", Kritik, Spec, 1, &[],
        Some("route_id"), VS_K, Row,
        "route_networks.network_id is missing or not found"),
    r!("NET_003", Kritik, Spec, 1, &[],
        Some("route_id"), VS_K, Row,
        "route_networks.route_id is missing or not found"),
    r!("NET_004", Kritik, Spec, 1, &[], Some("network_id"), VS_K, Row,
        "networks.txt has an empty network_id"),

    // ── TFR: Timeframes (Fares v2) ────────────────────────────────────────────
    r!("TFR_001", Kritik, Spec, 1, &[], Some("timeframe_group_id"), VS_K, Row,
        "timeframe_group_id missing"),
    r!("TFR_002", Kritik, Spec, 1, &[], Some("timeframe_group_id"), VS_K, Row,
        "service_id not found"),
    r!("TFR_003", Yuksek, Spec, 1, &[], Some("timeframe_group_id"), VS, Row,
        "start_time or end_time format error"),
    r!("TFR_004", Orta,   Quality, 1, &[], Some("timeframe_group_id"), VS, Row,
        "end_time less than start_time"),
    // CLASS CHANGED (#96, 2026-08-09): evidence class ↔ authority of the requirement. Only
    // this rule measured a strict Spec requirement, but was not in the Spec class; Spec-filtering
    // users could not see the violation. Severity UNCHANGED → the R1 gate (`Spec ∧ Kritik`) is unchanged.
    // Ruling: Pa586fe5b.
    r!("TFR_005", Orta,   Spec, 1, &[], Some("timeframe_group_id"), VS, Row,
        "Overlapping time ranges within same group and service_id"),
    r!("TFR_006", Kritik, Spec, 1, &[], Some("timeframe_group_id"), VS_K, Row,
        "start_time or end_time greater than 24:00:00"),
    r!("TFR_007", Kritik, Spec, 1, &[], Some("timeframe_group_id"), VS_K, Row,
        "Only one of start_time and end_time is specified"),
    r!("TFR_008", Kritik, Spec, 1, &[], Some("timeframe_group_id"), VS_K, Row,
        "timeframes has an empty service_id"),

    // ── PTH: Pathways ──────────────────────────────────────────────────────────
    r!("PTH_001", Kritik, Spec, 1, &[], Some("pathway_id"), VS_K, Entity,
        "Duplicate pathway_id"),
    r!("PTH_002", Kritik, Spec, 1,
        &["PTH_011","PTH_015"],
        Some("pathway_id"), VS_K, Entity,
        "from_stop_id not found"),
    r!("PTH_003", Kritik, Spec, 1,
        &["PTH_011","PTH_015"],
        Some("pathway_id"), VS_K, Entity,
        "to_stop_id not found"),
    r!("PTH_004", Kritik, Spec, 1,
    // PTH_008/009 became feed-level summaries (#15) → incompatible with per-pathway suppression, removed.
        &["PTH_016","PTH_017","PTH_018"],
        Some("pathway_id"), VS_K, Entity,
        "pathway_mode missing or invalid"),
    r!("PTH_005", Kritik, Spec, 1,
        &["PTH_016"],
        Some("pathway_id"), VS_K, Entity,
        "is_bidirectional missing"),
    r!("PTH_006", Orta,   Spec, 1, &[], Some("pathway_id"), VS, Entity,
        "length invalid"),
    r!("PTH_007", Orta,   Spec, 1, &[], Some("pathway_id"), VS, Entity,
        "traversal_time invalid"),
    r_noscore!("PTH_008", Dusuk,  Quality, 2, &[], None, VS, Feed,
        "stair_count missing"),
    r_noscore!("PTH_009", Dusuk,  Quality, 2, &[], None, VS, Feed,
        "max_slope missing"),
    r!("PTH_010", Dusuk,  Spec, 1, &[], Some("pathway_id"), VS, Entity,
        "min_width invalid"),
    r!("PTH_011", Yuksek, Quality, 3, &[], Some("pathway_id"), VS, Entity,
        "Pathway forms a loop"),
    // CLASS CHANGED (#96, 2026-08-09): evidence class ↔ ruling authority. This was the only
    // rule measuring a hard Spec requirement, but it was not in the Spec class; users filtering
    // for Spec violations saw NOTHING. Severity UNCHANGED → the R1 gate (`Spec ∧ Kritik`) is the same.
    // Ruling: Pde6fb4a5
    r!("PTH_012", Yuksek, Spec, 3,
        &[], // Grouped at feed level: the link suppressed every station (2026-10-13).
        Some("stop_id"), VS_ACC, Entity,
        "No accessible pathway to station"),
    r!("PTH_013", Bilgi,  Analytics, 3, &[], Some("stop_id"), VA_ACC, Entity,
        "Accessible pathway analysis"),
    r!("PTH_014", Kritik, Quality, 1, &[], Some("pathway_id"), VS, Entity,
        "Pathway crosses station boundary"),
    r!("PTH_015", Orta,   Analytics, 2, &[], Some("pathway_id"), VA, Entity,
        "Pathway leads to an unreachable stop"),
    r!("PTH_016", Yuksek, Spec, 3, &[], Some("pathway_id"), VS, Entity,
        "Exit defined as bidirectional"),
    // PTH_017 is now ONLY a TYPE violation: the spec types `max_slope` as `Float`, and a non-numeric
    // value violates that type → legitimate Spec claim. The "use only with pathway_mode 1/3" branch
    // moved to PTH_028 (Quality); the spec says "should" there, not a norm.
    r!("PTH_017", Orta,   Spec, 2, &[], Some("pathway_id"), VS, Entity,
        "max_slope is not a number"),
    r!("PTH_018", Dusuk,  Quality, 1, &[], Some("pathway_id"), VS, Entity,
        "signposted_as too long"),
    r!("PTH_019", Orta,   Quality, 2, &[], Some("stop_id"), VS, Entity,
        "Generic node connected to single pathway (dead-end)"),
    r!("PTH_020", Kritik, Spec, 1, &[], Some("pathway_id"), VS_K, Entity,
        "pathway_id missing"),
    r!("PTH_021", Kritik, Spec, 1, &[], Some("pathway_id"), VS_K, Entity,
        "from_stop_id missing"),
    r!("PTH_022", Kritik, Spec, 1, &[], Some("pathway_id"), VS_K, Entity,
        "to_stop_id missing"),
    r!("PTH_023", Kritik, Spec, 1, &[], Some("pathway_id"), VS_K, Entity,
        "pathway_mode missing"),
    r!("PTH_024", Kritik, Spec, 1, &[], Some("pathway_id"), VS_K, Entity,
        "is_bidirectional missing"),
    r_noscore!("PTH_025", Dusuk, Quality, 1, &[], None, VS, Feed,
        "Recommended pathway length missing"),
    // PTH_029: sibling of PTH_025. The spec says `traversal_time` is "recommended for moving
    // sidewalks (3), escalators (4) and elevator (5)"; PTH_007 validates the value WHEN PRESENT,
    // but did not measure absence. Two of the three recommended fields (length→PTH_025, stair_count→PTH_008)
    // were already measured. Measured (239 feeds): 4 feeds, 24 records.
    // PTH_030: when a platform's boarding areas are modeled, the spec requires pathways to connect
    // to the BOARDING AREAS and adds: "In such cases, the platform must not have pathways assigned."
    // Graph code TOLERATED nested modeling (k6_analytics.rs:7287) but did not measure the prohibition.
    // ⚠️ NO corpus example—the feared noise does not occur → synthetic fixture.
    r!("PTH_030", Dusuk, Spec, 1, &[], Some("pathway_id"), VS, Row,
        "Pathway assigned to a platform that has boarding areas"),
    r_noscore!("PTH_029", Dusuk, Quality, 1, &[], None, VS, Feed,
        "Recommended pathway traversal_time missing"),
    r!("PTH_026", Kritik, Spec, 1, &[], Some("pathway_id"), VS_K, Row,
        "Pathway endpoint is a station"),
    // PTH_031: the SECOND BRANCH of `PTH_026`. For `from_stop_id`/`to_stop_id`, the spec forbids two
    // things in ONE sentence: *"Values for stop_id that identify stations (location_type=1), **or
    // stops (location_type=0 or empty) with stop_access=1**, are forbidden."* The station branch
    // was PTH_026; the `stop_access=1` branch was NOT WRITTEN until 2026-08-06—a comment in
    // the code said "out of scope" but the requirement itself was not measured. The catalog also
    // missed it because the prohibition sentence uses lowercase ("are forbidden").
    // It was not added to PTH_026 as a separate rule: MD also has a separate notice and the facts differ—
    // one is the wrong node type, the other is "this stop is already street-accessible and is not a pathway subject".
    r!("PTH_031", Kritik, Spec, 1, &[], Some("pathway_id"), VS_K, Row,
        "Pathway endpoint is a street-accessed stop (stop_access=1)"),
    // PTH_027: spec type `Non-null integer` requires both an integer and NON-ZERO value.
    // Zero is meaningless: a positive value says the passenger walks upward, a negative value downward,
    // while zero says neither. Non-numeric values also used to be silently dropped.
    r!("PTH_027", Orta, Spec, 1, &[], Some("pathway_id"), VS, Entity,
        "stair_count is invalid (zero or not an integer)"),
    // PTH_028: SPLIT from PTH_017. For `max_slope`, the spec says "This field SHOULD only be used with
    // walkways (pathway_mode=1) and moving sidewalks (pathway_mode=3)"—"should", with a plain
    // `Optional` Presence column. The spec expresses real prohibitions in 14 fields using
    // `Conditionally Forbidden`; it deliberately does not do so here. We cannot impose a RECOMMENDATION
    // as a norm → Quality. (Same rationale as STM_040 and AGN_007.)
    r!("PTH_028", Dusuk, Quality, 1, &[], Some("pathway_id"), VS, Entity,
        "max_slope used outside a walkway"),

    // ── LVL: Levels ────────────────────────────────────────────────────────────
    r!("LVL_001", Kritik, Spec, 1,
        &[],
        Some("level_id"), VS_K, Entity,
        "Duplicate level_id"),
    r!("LVL_002", Kritik, Spec, 1, &[], Some("level_id"), VS_K, Entity,
        "level_index invalid"),
    r!("LVL_003", Dusuk,  Quality, 1, &[], Some("level_id"), VS, Entity,
        "level_name missing"),
    r!("LVL_004", Dusuk,  Quality, 1, &[], Some("level_id"), VS, Entity,
        "Unused level"),
    r!("LVL_005", Orta,   Quality, 1, &[], Some("level_id"), VS, Entity,
        "level_name too long"),
    r!("LVL_006", Orta,   Quality, 2, &[], Some("stop_id"), VS, Entity,
        "level_id missing on elevator-connected stop"),
    r!("LVL_007", Kritik, Spec, 1, &[], Some("level_id"), VS_K, Row,
        "level_index missing"),
    r!("LVL_008", Kritik, Spec, 1, &[], Some("level_id"), VS_K, Entity,
        "level_id missing"),

    // ── FIN: Feed Info ─────────────────────────────────────────────────────────
    r!("FIN_001", Kritik, Spec, 1, &[], None, VS_K, Feed,
        "feed_publisher_name missing"),
    r!("FIN_002", Kritik, Spec, 1, &[], None, VS_K, Feed,
        "feed_publisher_url missing or invalid"),
    r!("FIN_003", Kritik, Spec, 1,
        &["TRN_007"],
        None, VS_K, Feed,
        "feed_lang missing"),
    r!("FIN_004", Orta,   Spec, 1, &[], None, VS, Feed,
        "default_lang invalid"),
    r!("FIN_005", Orta,   Spec, 1, &[], None, VS, Feed,
        "feed_start_date invalid"),
    r!("FIN_006", Yuksek, Spec, 2, &[], None, VS, Feed,
        "feed_end_date invalid or past"),
    r!("FIN_007", Dusuk,  Quality, 1, &[], None, VS, Feed,
        "feed_version missing"),
    r!("FIN_008", Dusuk,  Spec, 1, &[], None, VS, Feed,
        "feed_contact_email invalid"),
    r!("FIN_009", Dusuk,  Spec, 1, &[], None, VS, Feed,
        "feed_contact_url invalid"),
    r!("FIN_010", Yuksek, Analytics, 2, &[], None, VA, Feed,
        "Feed validity expired"),
    // CLASS CHANGED (#96, 2026-08-09): evidence class ↔ authority of the requirement. Only
    // this rule measured a strict Spec requirement, but was not in the Spec class; Spec-filtering
    // users could not see the violation. Severity UNCHANGED → the R1 gate (`Spec ∧ Kritik`) is unchanged.
    // Ruling: P3e2e5c05.
    r!("FIN_012", Dusuk,  Spec, 1, &[], None, VS, Feed,
        "feed_start_date is after feed_end_date"),
    r!("FIN_013", Bilgi,  Quality, 1, &[], None, VS, Feed,
        "fare_attributes.agency_id recommended but missing"),
    r!("FIN_014", Dusuk,  Quality, 1, &[], None, VS, Feed,
        "Feed validity dates (feed_start_date/feed_end_date) missing"),
    r!("FIN_015", Orta,   Quality, 1, &[], None, VS, File,
        "Multiple feed_info records"),
    r!("FIN_016", Dusuk,  Quality, 1, &[], None, VS, Feed,
        "feed_start_date in the future (feed not yet active)"),
    r!("FIN_017", Bilgi,  Quality, 1, &[], None, VS, Feed,
        "Feed expires in the very distant future"),
    r!("FIN_018", Dusuk,  Quality, 1, &[], None, VS, Feed,
        "Both feed_contact_email and feed_contact_url missing"),
    r!("FIN_019", Dusuk,  Quality, 1, &[], None, VS, Feed,
        "Feed validity expires within 7 days"),
    r!("FIN_020", Bilgi,  Quality, 1, &[], None, VS, Feed,
        "Feed validity window shorter than 7 days"),

    // ── TRN: Translations ──────────────────────────────────────────────────────
    r!("TRN_001", Kritik, Spec, 1, &[], None, VS_K, Feed,
        "table_name invalid value"),
    r!("TRN_002", Kritik, Spec, 1, &[], None, VS_K, Row,
        "field_name invalid for this table"),
    r!("TRN_003", Orta,   Spec, 1, &[], None, VS, Row,
        "language invalid"),
    r!("TRN_004", Yuksek, Spec, 1, &[], Some("record_id"), VS, Row,
        "record_id not found"),
    r!("TRN_005", Kritik, Spec, 1, &[], Some("record_id"), VS_K, Row,
        "Duplicate translation"),
    r!("TRN_006", Kritik, Spec, 1, &[], Some("record_id"), VS_K, Row,
        "Translation record conflicting"),
    r!("TRN_007", Dusuk,  Quality, 1, &[], None, VS, Row,
        "Translation in same language as feed_lang"),
    // `translations.translation` is **Required** in the spec; an empty value fails the required field
    // → spec violation. Class Quality→Spec, severity Bilgi→KRİTİK (2026-08-01, WP-3).
    // Precedent: empty/missing required field = Kritik·Spec (FIN_001 feed_publisher_name, ATR_002
    // organization_name, LVL_007/008)—the file being Optional does not change this;
    // attributions.txt and levels.txt are Optional files too. Kritik+Spec → view VS_K
    // (it MUST carry R1, verified by the registry test) and the finding blocks publication.
    r!("TRN_008", Kritik, Spec, 1, &[], None, VS_K, Row,
        "translation value is empty"),
    r!("TRN_009", Yuksek, Spec, 1, &[], Some("record_id"), VS, Row,
        "record_id and field_value cannot be used together"),
    r!("TRN_010", Yuksek, Spec, 1, &[], Some("record_id"), VS, Row,
        "record_sub_id invalid"),
    r!("TRN_011", Yuksek, Spec, 1, &[], None, VS, Row,
        "field_name is not translatable"),
    r!("TRN_013", Yuksek, Spec, 1, &[], None, VS, Row,
        "Identity field cannot be used in feed_info translation"),
    r!("TRN_014", Yuksek, Spec, 1, &[], Some("record_id"), VS, Row,
        "record_sub_id only valid for stop_times"),
    // TRN_015: the spec has THREE requirements for `field_value`; two were already covered
    // (forbidden in feed_info → TRN_013, forbidden with record_id → TRN_009). The third was missing:
    // "Required if record_id is empty". If both are empty, the row does not say which record it
    // translates—the translation can never be applied.
    // TRN_017: the REVERSE BRANCH of `TRN_014`. The spec has two directions for `record_sub_id`:
    // forbidden OUTSIDE stop_times (→TRN_014), REQUIRED INSIDE stop_times when `record_id` is populated.
    // The primary key of `stop_times.txt` is COMPOSITE (`trip_id`+`stop_sequence`); `record_id` carries
    // only the first. Without the second, the translation cannot identify which ROW it belongs to—
    // the reference is not dangling but AMBIGUOUS, so XFL_014 also does not see it.
    // ⚠️ NO corpus example (no feed translates stop_times) → synthetic fixture.
    r!("TRN_017", Orta, Spec, 1, &[], Some("record_id"), VS, Row,
        "stop_times translation missing record_sub_id — the row is ambiguous"),
    r!("TRN_015", Yuksek, Spec, 1, &[], Some("record_id"), VS, Row,
        "record_id and field_value are both empty"),
    // TRN_016: `field_value` matching is based only on the VALUE—the spec says: "The field must have
    // exactly the value defined in field_value." If no row matches, the translation is SILENTLY inert:
    // the producer thinks it translated, but it is not translated. `XFL_014` covered only the `record_id` branch.
    // Measured (239 feeds): 6 feeds, 998 rows; in `mdb-2848`, 810 of 2,413 translations were dead.
    r!("TRN_016", Orta, Spec, 1, &[], None, VS, Feed,
        "field_value matches no record — the translation is never applied"),

    // ── ATR: Attributions ──────────────────────────────────────────────────────
    r_noscore!("ATR_001", Yuksek, Quality, 1, &[], Some("attribution_id"), VS, File,
        "attribution_id missing"),
    r!("ATR_002", Kritik, Spec, 1, &[], Some("attribution_id"), VS_K, Entity,
        "organization_name missing"),
    r!("ATR_003", Yuksek, Spec, 1, &[], Some("attribution_id"), VS, File,
        "Attribution role not defined"),
    r!("ATR_004", Kritik, Spec, 1, &[], Some("attribution_id"), VS_K, Entity,
        "is_producer invalid"),
    r!("ATR_005", Kritik, Spec, 1, &[], Some("attribution_id"), VS_K, Entity,
        "is_operator invalid"),
    r!("ATR_006", Kritik, Spec, 1, &[], Some("attribution_id"), VS_K, Entity,
        "is_authority invalid"),
    r!("ATR_007", Kritik, Spec, 1, &[], Some("attribution_id"), VS_K, Entity,
        "attribution_url invalid"),
    r!("ATR_008", Dusuk,  Spec, 1, &[], Some("attribution_id"), VS, Entity,
        "attribution_email invalid"),
    // CLASS CHANGED (#96, 2026-08-09): evidence class ↔ authority of the requirement. Only
    // this rule measured a strict Spec requirement, but was not in the Spec class; Spec-filtering
    // users could not see the violation. Severity UNCHANGED → the R1 gate (`Spec ∧ Kritik`) is unchanged.
    // Ruling: P7917f3c5.
    r!("ATR_009", Yuksek, Spec, 1, &[], Some("attribution_id"), VS, File,
        "multiple attribution reference fields set"),
    r!("ATR_010", Dusuk,  Spec, 1, &[], Some("attribution_id"), VS, Row,
        "agency_id not found"),
    // ATR_011/012: the three reference fields in attributions.txt are measured by three separate
    // rules. Previously route_id was reported under ATR_006 and trip_id under ATR_007—so they
    // shared IDs with enum/URL format errors. Feed-level summary XFL_015 (Kritik) covers all
    // three, so the publication gate remains LOW, matching ATR_010.
    r!("ATR_011", Dusuk,  Spec, 1, &[], Some("attribution_id"), VS, Row,
        "route_id not found"),
    r!("ATR_012", Dusuk,  Spec, 1, &[], Some("attribution_id"), VS, Row,
        "trip_id not found"),

    // ── XFL: Cross-File / Semantic ───────────────────────────────────────────
    r!("XFL_002", Yuksek, Interop, 1, &[], Some("trip_id"), VI, Entity,
        "Trip has no stop_times records"),
    r!("XFL_006", Orta,   Analytics, 2, &[], Some("service_id"), VA, Entity,
        "service_id contains only cancellation exceptions (no active days)"),
    // XFL_011 (2026-10-11): retired. The raw calendar range classified data outside the
    // spec-recommended window as Medium/Interop; the MD counterpart is CAL_014, which checks active dates.
    r!("XFL_012", Yuksek, Quality, 2, &[], Some("route_id"), VS, Entity,
        "Route with no operational trips"),
    r!("XFL_013", Yuksek, Quality, 2,
        // STM_017 is trip- or feed-scoped: a shape-scoped root could never match it (2026-10-11).
        &["SHP_017"],
        Some("shape_id"), VS, Entity,
        "shape_id used in multiple directions"),
    r!("XFL_014", Orta,   Quality, 1, &[], None, VS, Feed,
        "Invalid translation reference (source record not found)"),
    r!("XFL_015", Kritik, Spec, 1, &[], None, VS_K, Feed,
        "Invalid reference in attribution"),
    r!("XFL_016", Yuksek, Spec, 1, &[], None, VS, Feed,
        "Translation references feed_info but feed_info.txt is missing"),
    r!("XFL_019", Orta,   Spec, 2, &[], None, VS, Feed,
        "Network defined in two separate files (routes.network_id + networks.txt or route_networks.txt)"),
    // XFL_020 → Spec: GTFS Reference transfers.txt is NORMATIVE—"If both from_trip_id and
    // from_route_id are defined, the trip_id MUST BELONG TO the route_id" (the same applies to to_*).
    // MD counterpart: `transfer_with_invalid_trip_and_route` (ERROR).
    r!("XFL_020", Kritik, Spec, 2, &[], None, VS_K, Row,
        "Invalid (from_trip_id/to_trip_id, route_id) pair in transfers"),
    // XFL_021 → INTEROP, NOT Spec: the spec only says "the transfer will APPLY TO the arriving trip
    // for the given from_stop_id"; it has NO normative requirement that it contain the trip's stop
    // (confirmed from the reference text on 2026-07-27). MD treats this as ERROR and notice parity is
    // EXACT (`transfer_with_invalid_trip_and_stop`, 12/12 in Entur mdb-1078) → class Interop.
    r!("XFL_021", Yuksek, Interop, 2, &[], None, VI, Row,
        "Invalid (from_trip_id/to_trip_id, stop_id) pair in transfers"),
    r!("XFL_022", Kritik, Spec, 1, &[], Some("location_group_id"), VS_K, Row,
        "location_group_id not found"),
    r!("XFL_023", Kritik, Spec, 1, &[], Some("stop_id"), VS_K, Row,
        "stop_id not found (location_group_stops)"),
    r!("XFL_024", Kritik, Spec, 1, &[], Some("location_group_id"), VS_K, Row,
        "location_group_id not found (stop_times)"),
    r!("XFL_025", Kritik, Spec, 1, &[], Some("location_id"), VS_K, Row,
        "location_id not found (locations.geojson)"),
    // Spec (location_groups.location_group_id): "ID must be unique across all stops.stop_id,
    // locations.geojson id, and location_groups.location_group_id values." The three files share
    // ONE namespace; a colliding ID makes a stop_times.location_id / location_group_id reference
    // impossible to resolve.
    r!("XFL_031", Kritik, Spec, 2, &[], Some("location_group_id"), VS_K, Row,
        "ID clash: stop_id, locations.geojson id and location_group_id share one namespace"),
    r!("XFL_032", Kritik, Spec, 1, &[], Some("location_group_id"), VS_K, Row,
        "location_groups.txt has an empty location_group_id"),
    r!("XFL_033", Kritik, Spec, 1, &[], Some("location_group_id"), VS_K, Row,
        "location_group_stops has an empty location_group_id"),
    r!("XFL_034", Kritik, Spec, 1, &[], Some("stop_id"), VS_K, Row,
        "location_group_stops has an empty stop_id"),
    r_noscore!("XFL_027", Bilgi, Interop, 2, &[], Some("route_id"), VI, Entity,
        "route cemv=2 but applicable contactless product"),
    r!("XFL_028", Bilgi, Quality, 1, &[], None, VS, Feed,
        "agency cemv=1 but no contactless media"),
    r!("XFL_029", Bilgi, Quality, 1, &[], None, VS, Feed,
        "route cemv=1 but no contactless media"),
    r!("XFL_030", Bilgi, Quality, 1, &[], None, VS, Feed,
        "contactless media but no cemv=1"),

    // ── OPR: Operational Consistency (all ANALYTICS) ───────────────────────────
    r!("OPR_001", Orta,   Analytics, 3, &[], Some("route_id"), VA, Entity,
        "Route headway too long"),
    r!("OPR_003", Dusuk,  Analytics, 2, &[], Some("route_id"), VA, Entity,
        "Trip bunching (minimum headway too small)"),
    r!("OPR_004", Bilgi,  Analytics, 1, &[], Some("route_id"), VA, Entity,
        "No weekend service"),
    r!("OPR_005", Bilgi,  Analytics, 2, &[], Some("route_id"), VA, Entity,
        "Unusual service frequency"),
    r!("OPR_006", Yuksek, Analytics, 3, &[], Some("trip_id"), VA, Entity,
        "Trip has too few stops (not functional)"),
    r_noscore!("OPR_007", Bilgi, Analytics, 3, &[], Some("trip_id"), VA, Entity,
        "Repeated stop pattern within trip"),
    r_noscore!("OPR_008", Bilgi, Analytics, 3, &[], Some("trip_id"), VA, Entity,
        "Excessive speed in multiple segments"),
    r!("OPR_009", Bilgi,  Analytics, 2, &[], Some("route_id"), VA, Entity,
        "Night trip start time too late"),
    r!("OPR_010", Orta,   Analytics, 2, &[], Some("route_id"), VA, Entity,
        "Route accessibility or bicycle policy conflicts"),
    r!("OPR_011", Yuksek, Analytics, 2, &[], Some("service_id"), VA, Entity,
        "Service has no active days"),
    r!("OPR_012", Orta,   Analytics, 2, &[], Some("service_id"), VA, Entity,
        "Service gap"),
    r!("OPR_013", Bilgi,  Analytics, 2, &[], Some("route_id"), VA, Entity,
        "Route operates in one direction only (no return direction)"),
    r!("OPR_014", Orta,   Analytics, 2, &[], None, VA, Feed,
        "Average transfer time too long"),
    r!("OPR_015", Bilgi,  Analytics, 1, &[], Some("route_id"), VA, Entity,
        "Route operates with a single shape only"),
    r!("OPR_016", Bilgi,  Analytics, 1, &[], None, VA, Feed,
        "No active service across the entire feed"),
    r!("OPR_017", Orta,   Analytics, 2, &[], Some("trip_id"), VA, Entity,
        "Trip distance too short"),
    r!("OPR_019", Bilgi,  Analytics, 2, &[], Some("route_id"),   VA, Entity,
        "Route calendar overlap (multiple services on same day)"),
    // OPR_020 2026-07-24: High→Low. The rule cannot prove a base↔override relationship;
    // it also appears in a legitimate multi-variant model with "2+ active services per day +
    // one with an exception" (VBB: 74% of routes trigger it). The card's own FP note accepts this.
    // HIGH means "probable real issue"; it does not fit a saturated accepted-FP heuristic.
    r!("OPR_020", Dusuk, Analytics, 2, &[], Some("route_id"),   VA, Entity,
        "Route exception day overlap"),
    r!("OPR_021", Yuksek, Analytics, 3, &[], Some("route_id"),   VA, Entity,
        "Calendar override conflict: override and base simultaneously active"),
    r!("OPR_022", Yuksek, Analytics, 3, &[], Some("route_id"),   VA, Entity,
        "Calendar override not applied: base service running on override day"),
    r!("OPR_023", Orta,   Analytics, 3, &[], Some("route_id"),   VA, Entity,
        "Calendar override gap: no service active within window"),
    r!("OPR_024", Bilgi,  Analytics, 1, &[], Some("route_id"),   VA, Entity,
        "Route has an extreme trip count"),
    r!("OPR_025", Yuksek, Analytics, 2, &[], None, VA, Feed,
        "Feed average trip duration under 60 seconds"),

    // ── GEO: Geographic / Spatial (canonical rules—excluding GEO_008/010/011) ─
    r!("GEO_002", Yuksek, Analytics, 2, &[], Some("stop_id"), VA_GEO, Entity,
        "Stop too far from feed median"),
    r!("GEO_006", Yuksek, Analytics, 3, &[], Some("shape_id"), VA_GEO, Entity,
        "Large jump in shape"),
    r!("GEO_007", Yuksek, Analytics, 3, &[], Some("shape_id"), VA_GEO, Entity,
        "Critical jump in shape (3× threshold)"),
    r_noscore!("GEO_009", Yuksek, Quality,   3, &[], Some("stop_id"), VS_GEO, Entity,
        "Stop too far from shape route"),
    r!("GEO_012", Orta,   Analytics, 2, &[], Some("stop_id"), VA_GEO, Entity,
        "Stop cluster (stops too close together)"),
    r!("GEO_013", Bilgi,  Analytics, 2, &[], None, VA_GEO, Feed,
        "Feed geographic coverage summary"),
    r!("GEO_014", Bilgi,  Analytics, 1, &[], None, VA_GEO, Feed,
        "Feed geographic coverage too wide"),
    r!("GEO_015", Orta,   Quality,   1, &[], Some("stop_id"), VS, Entity,
        "Stop coordinates outside Japan bounds (feed_lang: ja)"),
    r!("GEO_016", Yuksek, Quality,   1, &[], Some("stop_id"), VA_GEO, Entity,
        "Stop at or near Null Island (|lat|<0.1 and |lon|<0.1)"),
    r!("GEO_017", Yuksek, Quality,   1, &[], Some("shape_id"), VA_GEO, Entity,
        "Shape point at or near Null Island"),
    r!("GEO_018", Yuksek, Analytics, 2, &[], None, VA_GEO, Feed,
        "All feed stops within 200m radius (possible test data)"),
    r!("GEO_019", Orta,   Quality,   1, &[], Some("stop_id"), VS, Entity,
        "Stop has integer (zero-precision) coordinates"),
    r!("GEO_020", Yuksek, Quality,   1, &[], Some("shape_id"), VA_GEO, Entity,
        "Shape is degenerate — all points at the same location"),
    r!("GEO_021", Yuksek, Analytics, 2, &[], None, VA_GEO, Feed,
        "More than 30% of stops share coordinates (systematic issue)"),
    r!("GEO_022", Yuksek, Quality,   1, &[], Some("stop_id"), VA_GEO, Entity,
        "Stop latitude near a pole (|lat| > 89)"),

    // ── DQ: Data Quality / User Experience ───────────────────────────────────
    // DQ_001/DQ_002 (missing feed_publisher_name/url) removed: ARC_020 (file absent)
    // + FIN_001/002 (value empty) + ARC_025 (column absent) already cover them.
    r!("DQ_003",  Bilgi,  Quality,  1, &[], Some("route_id"), VS, Entity,
        "Route description missing"),
    r!("DQ_004",  Bilgi,  Quality,  1, &[], Some("route_id"), VS, Entity,
        "Route URL missing"),
    r!("DQ_005",  Yuksek, Quality,  2, &[], None, VS, Feed,
        "No valid service period"),
    r!("DQ_005b", Yuksek, Quality,  2, &["DQ_009"], None, VS, Feed,
        "No trip has stop times"),
    r!("DQ_005c", Yuksek, Quality,  2, &[], None, VS, Feed,
        "High proportion of stops without coordinates"),
    r_noscore!("DQ_006", Yuksek, Quality, 2, &[], None, VS, Feed,
        "High proportion of trips without shapes"),
    r!("DQ_009",  Bilgi,  Quality,  1, &[], None, VS, Feed,
        "No stop times in trips"),
    r!("DQ_010",  Bilgi,  Quality,  1, &[], Some("agency_id"), VS, Entity,
        "Agency not used by any route"),
    r!("DQ_011",  Dusuk,  Quality,  1, &[], None, VS, Feed,
        "Only one stop exists"),
    r!("DQ_012",  Dusuk,  Quality,  1, &[], None, VS, Feed,
        "Too many agencies, agency_id not used"),
    r_noscore!("DQ_013", Bilgi, Analytics, 1, &[], None, VA, Feed,
        "Too few trips"),
    // Emit one summary per FILE → Entity=File. Previously it was Row, and because leading/trailing
    // whitespace was a generator habit (e.g. separator ', '), it appeared on EVERY row in the file:
    // 2,361,873 notices in mdb-992 (MD: 12) → browser OOM risk. Same decision as STM_050.
    r!("DQ_016",  Orta,   Quality,  1, &[], None, VS, File,
        "Extra whitespace in field value"),
    r!("DQ_017",  Bilgi,  Quality,  1, &[], None, VS, Feed,
        "Suspicious coordinate value"),
    r!("DQ_018",  Orta,   Quality,  1, &[], Some("stop_id"), VS, Entity,
        "All-caps value in recommended field"),
    r!("DQ_019",  Orta,   Quality,  1, &[], Some("file"), VS, Field,
        "All-lowercase value in recommended field"),
    r!("DQ_020",  Dusuk,  Quality,  1, &[], Some("file"), VS, Field,
        "Recommended field missing or empty"),
    r!("DQ_021",  Yuksek, Spec,     1, &[], None, VS_K, File,
        "Primary key duplicate (duplicate_key)"),
    r!("DQ_022",  Yuksek, Quality,  2, &[], None, VS, Feed,
        "More than 80% of stops share the same stop_name"),

    // ── VAT: Entity Analytics Detection (all ANALYTICS) ──────────────────────
    r!("VAT_001", Orta,   Analytics, 3, &[], Some("route_id"), VA, Entity,
        "Route shape similarity (likely duplicate route)"),
    r!("VAT_002", Bilgi,  Analytics, 3, &[], Some("stop_id"), VA, Entity,
        "Transfer hub undefined — many routes pass but no transfers defined"),
    r!("VAT_003", Dusuk,  Analytics, 3, &[], Some("trip_id"), VA, Entity,
        "Trip duration statistical outlier"),
    r!("VAT_005", Orta,   Analytics, 3, &[], None, VA, Feed,
        "Isolated stop cluster — stops disconnected from main network component"),
    r!("VAT_006", Bilgi,  Analytics, 2, &[], Some("route_id"), VA, Entity,
        "Service density imbalance — single route accounts for large proportion of feed trips"),
    r!("VAT_007", Bilgi,  Analytics, 3, &[], Some("stop_id"), VA, Entity,
        "Terminal transfer missing — another route serves the terminus but no transfer defined"),
    r!("VAT_008", Bilgi,  Analytics, 2, &[], Some("shape_id"), VA_GEO, Entity,
        "Same shape used by more than 30% of routes"),

    // ── JPN — GTFS-JP (Japan profile) rules; only on GTFS-JP feeds ────────────
    r!("JPN_001", Orta,   Quality, 2, &[], Some("stop_id"), VS, Entity,
        "GTFS-JP: stop name missing kana (ja-Hrkt) reading"),
    r!("JPN_002", Yuksek, Interop, 1, &[], Some("trip_id"), VI, Entity,
        "GTFS-JP: jp_office_id not defined in office_jp.txt"),
    r!("JPN_003", Yuksek, Interop, 1, &[], Some("agency_id"), VI, Entity,
        "GTFS-JP: agency_jp agency_id not defined in agency.txt"),
    r!("JPN_004", Yuksek, Interop, 2, &[], None, VI, Feed,
        "GTFS-JP: translations.txt missing"),
    r!("JPN_005", Yuksek, Interop, 1, &[], Some("office_id"), VI, Entity,
        "GTFS-JP: office_jp office_name empty"),
    r!("JPN_006", Orta,   Quality, 2, &[], None, VS, Feed,
        "GTFS-JP: fare files missing"),
    r!("JPN_007", Orta,   Quality, 2, &[], None, VS, Feed,
        "GTFS-JP: feed_info.txt missing"),
    r!("JPN_008", Orta,   Quality, 2, &[], Some("route_id"), VS, Field,
        "GTFS-JP: route_long_name kana reading missing"),
    r!("JPN_009", Orta,   Quality, 2, &[], Some("trip_id"), VS, Entity,
        "GTFS-JP: trip_headsign kana reading missing"),
    r!("JPN_010", Orta,   Quality, 2, &[], Some("agency_id"), VS, Entity,
        "GTFS-JP: agency_name kana reading missing"),
    r!("JPN_011", Yuksek, Interop, 1, &[], None, VI, File,
        "GTFS-JP: agency_id required even with one agency"),
    r!("JPN_012", Yuksek, Interop, 1, &[], Some("agency_id"), VI, Entity,
        "GTFS-JP: agency_jp agency_id missing"),
    r!("JPN_013", Orta, Quality, 1, &[], Some("agency_id"), VS, Entity,
        "GTFS-JP: agency_zip_number format invalid"),
    r!("JPN_014", Yuksek, Interop, 1, &[], Some("office_id"), VI, Entity,
        "GTFS-JP: office_jp office_id missing or duplicated"),
    r!("JPN_015", Yuksek, Interop, 1, &[], Some("route_id"), VI, Entity,
        "GTFS-JP legacy: routes_jp route_id invalid"),
    // A K4 notice carries jp_pattern_id for pattern_jp and route_id for legacy routes_jp;
    // a single K2 scope field cannot correctly express these two file-dependent identities.
    r!("JPN_016", Orta, Quality, 1, &[], None, VS, Row,
        "GTFS-JP: pattern_jp or legacy routes_jp route_update_date invalid"),
    r!("JPN_017", Yuksek, Interop, 1, &[], Some("jp_pattern_id"), VI, Entity,
        "GTFS-JP: jp_pattern_id missing or duplicated"),
    r!("JPN_018", Yuksek, Interop, 1, &[], Some("trip_id"), VI, Entity,
        "GTFS-JP: jp_pattern_id undefined"),
    r!("JPN_019", Orta, Interop, 1, &[], Some("record_id"), VI, Entity,
        "GTFS-JP: kana translation reference invalid"),
    r!("JPN_020", Orta, Quality, 1, &[], Some("office_id"), VS, Entity,
        "GTFS-JP: office contact format suspicious"),
    r!("JPN_021", Orta, Quality, 2, &[], Some("record_id"), VS, Entity,
        "GTFS-JP: kana translation inconsistent"),
    r!("JPN_022", Orta, Interop, 1, &[], None, VI, Field,
        "GTFS-JP v4: required core field missing"),
    r!("JPN_023", Yuksek, Interop, 1, &[], None, VI, Field,
        "GTFS-JP: feed_lang is not ja"),
    r!("JPN_024", Yuksek, Interop, 1, &[], Some("agency_id"), VI, Field,
        "GTFS-JP: agency_lang is not ja"),
    r!("JPN_025", Yuksek, Interop, 1, &[], Some("agency_id"), VI, Field,
        "GTFS-JP: agency_timezone is not Asia/Tokyo"),
    r!("JPN_026", Yuksek, Interop, 1, &[], Some("fare_id"), VI, Field,
        "GTFS-JP: fare currency is not JPY"),
    r!("JPN_027", Yuksek, Interop, 1, &[], Some("route_id"), VI, Field,
        "GTFS-JP V3: route_type is not 3"),
    r!("JPN_028", Orta, Quality, 2, &[], None, VS, Field,
        "GTFS-JP v3: required kana translation missing"),
    r!("JPN_029", Dusuk, Quality, 1, &[], None, VS, Field,
        "GTFS-JP v4: recommended kana translation missing"),
    r!("JPN_030", Orta, Quality, 2, &[], None, VS, Field,
        "GTFS-JP v3: required Japanese translation missing"),
    r!("JPN_031", Yuksek, Interop, 2, &[], Some("stop_id"), VI, Entity,
        "GTFS-JP: zone_id missing at a stop served by zone-based fares"),
    r!("JPN_032", Orta, Interop, 1, &[], Some("agency_id"), VI, Entity,
        "GTFS-JP V3: agency_id is not in Corporate Number format"),
    r!("JPN_033", Orta, Interop, 1, &[], None, VI, Field,
        "GTFS-JP: reserved jp namespace used by a custom file or field"),
];

/// One-time O(1) lookup table for id → metadata from `RULES`.
/// Because `RULES` is a compile-time constant, it is initialized once on the first call;
/// later calls use a hash lookup instead of a linear scan. `OnceLock` is thread-safe, so this
/// is also safe in the `pkg-threads` (rayon) build. The output/return value is EXACTLY the
/// same as a linear scan (IDs are guaranteed unique by the `no_duplicate_ids` test).
fn rule_index() -> &'static std::collections::HashMap<&'static str, &'static RuleMeta> {
    static IDX: std::sync::OnceLock<std::collections::HashMap<&'static str, &'static RuleMeta>> =
        std::sync::OnceLock::new();
    IDX.get_or_init(|| RULES.iter().map(|r| (r.id, r)).collect())
}

/// Returns metadata for a rule ID.
pub fn get_rule(id: &str) -> Option<&'static RuleMeta> {
    rule_index().get(id).copied()
}

/// Rule ID → authority source (generated from the authority audit ledger; local `notgit/audits/`).
/// Class-authority integrity SSOT: the `Spec` class is legitimate ONLY with `GtfsSpec`.
/// Kept separate from the r!() calls; coverage/uniqueness is checked by the
/// `authority_covers_all_rules` test.
static AUTHORITY: &[(&str, AuthoritySource)] = &[
    ("AGN_002", GtfsSpec),
    ("AGN_003", GtfsSpec),
    ("AGN_004", GtfsSpec),
    ("AGN_005", GtfsSpec),
    ("AGN_006", GtfsSpec),
    ("AGN_007", ProjectQuality),
    ("AGN_008", GtfsSpec),
    ("AGN_009", GtfsSpec),
    ("AGN_010", GtfsSpec),
    ("AGN_011", GtfsSpec),
    ("AGN_012", GtfsSpec),
    ("AGN_013", MobilitydataParity),
    ("AGN_014", GtfsSpec),
    ("AGN_015", ProjectQuality),
    ("AGN_016", ProjectQuality),
    ("AGN_017", MobilitydataParity),
    ("AGN_018", ProjectQuality),
    ("ARC_001", GtfsSpec),
    ("ARC_002", GtfsBestPractice),
    ("ARC_003", ProjectQuality),
    ("ARC_004", GtfsSpec),
    ("ARC_035", GtfsSpec),
    ("ARC_036", ProjectQuality),
    ("ARC_037", ProjectQuality),
    ("ARC_006", ProjectAnalytics),
    ("ARC_007", ProjectQuality),
    ("ARC_008", GtfsSpec),
    ("ARC_031", GtfsSpec),
    ("ARC_009", ProjectQuality),
    ("ARC_010", ProjectQuality),
    ("ARC_011", ProjectAnalytics),
    ("ARC_012", GtfsSpec),
    ("ARC_013", GtfsSpec),
    ("ARC_014", ProjectQuality),
    ("ARC_015", MobilitydataParity),
    ("ARC_017", ProjectQuality),
    ("ARC_018", ProjectQuality),
    ("ARC_019", ProjectQuality),
    ("ARC_020", ProjectQuality),
    ("ARC_021", ProjectQuality),
    ("ARC_022", ProjectAnalytics),
    ("ARC_023", ProjectQuality),
    ("ARC_024", GtfsSpec),
    ("ARC_025", GtfsSpec),
    ("ARC_026", GtfsSpec),
    ("ARC_027", ProjectQuality),
    ("ARC_028", ProjectQuality),
    ("ARC_029", ProjectQuality),
    ("ARC_030", GtfsSpec),
    ("ARC_032", GtfsSpec),
    ("ARC_033", GtfsSpec),
    ("ARC_034", GtfsSpec),
    ("ARS_001", GtfsSpec),
    ("ARS_002", GtfsSpec),
    ("ATR_001", ProjectQuality),
    ("ATR_002", GtfsSpec),
    ("ATR_003", GtfsSpec),
    ("ATR_004", GtfsSpec),
    ("ATR_005", GtfsSpec),
    ("ATR_006", GtfsSpec),
    ("ATR_007", GtfsSpec),
    ("ATR_008", GtfsSpec),
    ("ATR_009", GtfsSpec),
    ("ATR_010", GtfsSpec),
    ("ATR_011", GtfsSpec),
    ("ATR_012", GtfsSpec),
    ("BKR_001", GtfsSpec),
    ("BKR_003", GtfsSpec),
    ("BKR_004", GtfsSpec),
    ("BKR_005", GtfsSpec),
    ("BKR_006", GtfsSpec),
    ("BKR_007", GtfsSpec),
    ("BKR_008", GtfsSpec),
    ("BKR_009", GtfsSpec),
    ("BKR_010", GtfsSpec),
    ("BKR_011", MobilitydataParity),
    ("BKR_012", GtfsSpec),
    ("BKR_013", GtfsSpec),
    ("BKR_014", GtfsSpec),
    ("BKR_015", GtfsSpec),
    ("BKR_016", GtfsSpec),
    ("BKR_017", GtfsSpec),
    ("BKR_018", GtfsSpec),
    ("BKR_019", GtfsSpec),
    ("BKR_020", GtfsSpec),
    ("BKR_021", GtfsSpec),
    ("BKR_022", ProjectQuality),
    ("BKR_023", GtfsSpec),
    ("BKR_024", GtfsSpec),
    ("BKR_025", GtfsSpec),
    ("CAL_001", GtfsSpec),
    ("CAL_002", GtfsSpec),
    ("CAL_003", GtfsSpec),
    ("CAL_004", GtfsSpec),
    ("CAL_005", MobilitydataParity),
    ("CAL_006", ProjectQuality),
    ("CAL_007", ProjectAnalytics),
    ("CAL_008", ProjectAnalytics),
    ("CAL_009", ProjectAnalytics),
    ("CAL_010", ProjectAnalytics),
    ("CAL_011", ProjectQuality),
    ("CAL_012", ProjectAnalytics),
    ("CAL_013", ProjectAnalytics),
    ("CAL_014", MobilitydataParity),
    ("CAL_015", ProjectAnalytics),
    ("CAL_016", ProjectQuality),
    ("CAL_017", ProjectAnalytics),
    ("CAL_018", ProjectQuality),
    ("CAL_020", ProjectQuality),
    ("CAL_021", ProjectAnalytics),
    ("CAL_022", GtfsSpec),
    ("CAL_023", ProjectQuality),
    ("CAL_024", ProjectAnalytics),
    ("CAL_025", GtfsSpec),
    ("CLD_001", GtfsSpec),
    ("CLD_002", GtfsSpec),
    ("CLD_003", GtfsSpec),
    ("CLD_004", ProjectQuality),
    ("CLD_005", ProjectQuality),
    ("CLD_006", ProjectAnalytics),
    ("CLD_007", ProjectAnalytics),
    ("DQ_003", ProjectQuality),
    ("DQ_004", ProjectQuality),
    ("DQ_005", ProjectQuality),
    ("DQ_005b", ProjectQuality),
    ("DQ_005c", ProjectQuality),
    ("DQ_006", ProjectQuality),
    ("DQ_009", ProjectQuality),
    ("DQ_010", ProjectQuality),
    ("DQ_011", ProjectQuality),
    ("DQ_012", ProjectQuality),
    ("DQ_013", ProjectAnalytics),
    ("DQ_016", ProjectQuality),
    ("DQ_017", ProjectQuality),
    ("DQ_018", ProjectQuality),
    ("DQ_019", ProjectQuality),
    ("DQ_020", ProjectQuality),
    ("DQ_021", GtfsSpec),
    ("DQ_022", ProjectQuality),
    ("FAR_001", GtfsSpec),
    ("FAR_002", GtfsSpec),
    ("FAR_003", GtfsSpec),
    ("FAR_004", GtfsSpec),
    ("FAR_005", GtfsSpec),
    ("FAR_006", GtfsSpec),
    ("FAR_008", GtfsSpec),
    ("FAR_010", ProjectQuality),
    ("FAR_011", GtfsSpec),
    ("FAR_012", GtfsSpec),
    // The fact it catches is an ISO 4217 formatting convention; the GTFS spec does NOT require
    // decimals for `fare_attributes.price` (the requirement is for `fare_products.amount` → `FPD_007`).
    ("FAR_013", ProjectQuality),
    ("FIN_001", GtfsSpec),
    ("FIN_002", GtfsSpec),
    ("FIN_003", GtfsSpec),
    ("FIN_004", GtfsSpec),
    ("FIN_005", GtfsSpec),
    ("FIN_006", GtfsSpec),
    ("FIN_007", ProjectQuality),
    ("FIN_008", GtfsSpec),
    ("FIN_009", GtfsSpec),
    ("FIN_010", ProjectAnalytics),
    ("FIN_012", GtfsSpec),
    ("FIN_013", ProjectQuality),
    ("FIN_014", ProjectQuality),
    ("FIN_015", ProjectQuality),
    ("FIN_016", ProjectQuality),
    ("FIN_017", ProjectQuality),
    ("FIN_018", ProjectQuality),
    ("FIN_019", ProjectQuality),
    ("FIN_020", ProjectQuality),
    ("FLG_001", GtfsSpec),
    ("FLG_002", GtfsSpec),
    ("FLG_003", GtfsSpec),
    ("FLG_004", GtfsSpec),
    ("FLG_005", GtfsSpec),
    ("FLG_006", GtfsSpec),
    ("FLG_007", GtfsSpec),
    ("FLG_008", GtfsSpec),
    ("FLJ_001", GtfsSpec),
    ("FLJ_002", GtfsSpec),
    ("FLJ_003", GtfsSpec),
    ("FLJ_004", GtfsSpec),
    ("FMD_001", GtfsSpec),
    ("FMD_002", GtfsSpec),
    ("FMD_003", ProjectQuality),
    ("FMD_004", GtfsSpec),
    ("FPD_001", GtfsSpec),
    ("FPD_002", GtfsSpec),
    ("FPD_003", GtfsSpec),
    ("FPD_004", GtfsSpec),
    ("FPD_005", GtfsSpec),
    ("FPD_007", GtfsSpec),
    ("FRL_001", GtfsSpec),
    ("FRL_002", GtfsSpec),
    ("FRL_003", GtfsSpec),
    ("FRL_004", GtfsSpec),
    ("FRL_005", GtfsSpec),
    ("FRL_006", ProjectQuality),
    ("FRL_007", ProjectQuality),
    ("FRL_008", ProjectQuality),
    ("FRL_009", ProjectQuality),
    ("FRQ_001", GtfsSpec),
    ("FRQ_002", GtfsSpec),
    ("FRQ_003", GtfsSpec),
    ("FRQ_004", GtfsSpec),
    ("FRQ_005", ProjectQuality),
    ("FRQ_006", ProjectAnalytics),
    ("FRQ_007", GtfsSpec),
    ("FRQ_008", GtfsSpec),
    ("FRQ_009", ProjectQuality),
    ("FRQ_010", ProjectAnalytics),
    ("FRQ_011", GtfsSpec),
    ("FRQ_012", GtfsSpec),
    ("FTR_001", GtfsSpec),
    ("FTR_002", GtfsSpec),
    ("FTR_003", GtfsSpec),
    ("FTR_004", GtfsSpec),
    ("FTR_005", GtfsSpec),
    ("FTR_006", GtfsSpec),
    ("FTR_007", GtfsSpec),
    ("FTR_008", GtfsSpec),
    ("FTR_009", GtfsSpec),
    ("FTR_010", GtfsSpec),
    ("FTR_011", GtfsSpec),
    ("GEO_002", ProjectAnalytics),
    ("GEO_006", ProjectAnalytics),
    ("GEO_007", ProjectAnalytics),
    ("GEO_009", ProjectQuality),
    ("GEO_012", ProjectAnalytics),
    ("GEO_013", ProjectAnalytics),
    ("GEO_014", ProjectAnalytics),
    ("GEO_015", ProjectQuality),
    ("GEO_016", ProjectQuality),
    ("GEO_017", ProjectQuality),
    ("GEO_018", ProjectAnalytics),
    ("GEO_019", ProjectQuality),
    ("GEO_020", ProjectQuality),
    ("GEO_021", ProjectAnalytics),
    ("GEO_022", ProjectQuality),
    ("GGL_001", GoogleTransitInterop),
    ("GGL_002", GoogleTransitInterop),
    ("JPN_001", ProjectQuality),
    ("JPN_002", RegionalProfile),
    ("JPN_003", RegionalProfile),
    ("JPN_004", RegionalProfile),
    ("JPN_005", RegionalProfile),
    ("JPN_006", ProjectQuality),
    ("JPN_007", ProjectQuality),
    ("JPN_008", ProjectQuality),
    ("JPN_009", ProjectQuality),
    ("JPN_010", ProjectQuality),
    ("JPN_011", RegionalProfile),
    ("JPN_012", RegionalProfile),
    ("JPN_013", ProjectQuality),
    ("JPN_014", RegionalProfile),
    ("JPN_015", RegionalProfile),
    ("JPN_016", ProjectQuality),
    ("JPN_017", RegionalProfile),
    ("JPN_018", RegionalProfile),
    ("JPN_019", RegionalProfile),
    ("JPN_020", ProjectQuality),
    ("JPN_021", ProjectQuality),
    ("JPN_022", RegionalProfile),
    ("JPN_023", RegionalProfile),
    ("JPN_024", RegionalProfile),
    ("JPN_025", RegionalProfile),
    ("JPN_026", RegionalProfile),
    ("JPN_027", RegionalProfile),
    ("JPN_028", ProjectQuality),
    ("JPN_029", ProjectQuality),
    ("JPN_030", ProjectQuality),
    ("JPN_031", RegionalProfile),
    ("JPN_032", RegionalProfile),
    ("JPN_033", RegionalProfile),
    ("LOC_001", GtfsSpec),
    ("LOC_002", GtfsSpec),
    ("LOC_003", GtfsSpec),
    ("LOC_004", GtfsSpec),
    ("LOC_005", ProjectQuality),
    ("LOC_006", ProjectQuality),
    ("LOC_007", GtfsSpec),
    ("LOC_008", GtfsSpec),
    ("LOC_009", GtfsSpec),
    ("LOC_010", GtfsSpec),
    ("LOC_011", GtfsSpec),
    ("LOC_012", GtfsSpec),
    ("LVL_001", GtfsSpec),
    ("LVL_002", GtfsSpec),
    ("LVL_003", ProjectQuality),
    ("LVL_004", ProjectQuality),
    ("LVL_005", ProjectQuality),
    ("LVL_006", ProjectQuality),
    ("LVL_007", GtfsSpec),
    ("LVL_008", GtfsSpec),
    ("NET_001", GtfsSpec),
    ("NET_002", GtfsSpec),
    ("NET_003", GtfsSpec),
    ("NET_004", GtfsSpec),
    ("OPR_001", ProjectAnalytics),
    ("OPR_003", ProjectAnalytics),
    ("OPR_004", ProjectAnalytics),
    ("OPR_005", ProjectAnalytics),
    ("OPR_006", ProjectAnalytics),
    ("OPR_007", ProjectAnalytics),
    ("OPR_008", ProjectAnalytics),
    ("OPR_009", ProjectAnalytics),
    ("OPR_010", ProjectAnalytics),
    ("OPR_011", ProjectAnalytics),
    ("OPR_012", ProjectAnalytics),
    ("OPR_013", ProjectAnalytics),
    ("OPR_014", ProjectAnalytics),
    ("OPR_015", ProjectAnalytics),
    ("OPR_016", ProjectAnalytics),
    ("OPR_017", ProjectAnalytics),
    ("OPR_019", ProjectAnalytics),
    ("OPR_020", ProjectAnalytics),
    ("OPR_021", ProjectAnalytics),
    ("OPR_022", ProjectAnalytics),
    ("OPR_023", ProjectAnalytics),
    ("OPR_024", ProjectAnalytics),
    ("OPR_025", ProjectAnalytics),
    ("PDW_006", ProjectAnalytics),
    ("PTH_001", GtfsSpec),
    ("PTH_002", GtfsSpec),
    ("PTH_003", GtfsSpec),
    ("PTH_004", GtfsSpec),
    ("PTH_005", GtfsSpec),
    ("PTH_006", GtfsSpec),
    ("PTH_007", GtfsSpec),
    ("PTH_008", ProjectQuality),
    ("PTH_009", ProjectQuality),
    ("PTH_010", GtfsSpec),
    ("PTH_011", ProjectQuality),
    ("PTH_012", GtfsSpec),
    ("PTH_013", ProjectAnalytics),
    ("PTH_014", ProjectQuality),
    ("PTH_015", ProjectAnalytics),
    ("PTH_016", GtfsSpec),
    ("PTH_017", GtfsSpec),
    ("PTH_018", ProjectQuality),
    ("PTH_019", ProjectQuality),
    ("PTH_020", GtfsSpec),
    ("PTH_021", GtfsSpec),
    ("PTH_022", GtfsSpec),
    ("PTH_023", GtfsSpec),
    ("PTH_024", GtfsSpec),
    ("PTH_025", ProjectQuality),
    ("RCT_001", GtfsSpec),
    ("RCT_002", GtfsSpec),
    ("RCT_003", GtfsSpec),
    ("RCT_004", ProjectQuality),
    ("RCT_005", ProjectQuality),
    ("RCT_006", GtfsSpec),
    ("RCT_007", GtfsSpec),
    ("RCT_008", GtfsSpec),
    ("RTS_001", GtfsSpec),
    ("RTS_002", GtfsSpec),
    ("RTS_003", GtfsSpec),
    ("RTS_004", GtfsSpec),
    ("RTS_005", GtfsSpec),
    ("RTS_006", GtfsSpec),
    ("RTS_007", GtfsSpec),
    ("RTS_008", ProjectQuality),
    ("RTS_010", ProjectQuality),
    ("RTS_011", ProjectQuality),
    ("RTS_012", ProjectQuality),
    ("RTS_013", GtfsSpec),
    ("RTS_016", ProjectQuality),
    ("RTS_017", ProjectQuality),
    ("RTS_018", GtfsSpec),
    ("RTS_019", ProjectQuality),
    ("RTS_020", ProjectQuality),
    ("RTS_021", ProjectQuality),
    ("RTS_022", ProjectQuality),
    ("RTS_023", ProjectQuality),
    ("RTS_024", GtfsSpec),
    ("RTS_025", ProjectQuality),
    ("RTS_026", ProjectQuality),
    ("RTS_027", ProjectQuality),
    ("RTS_028", GtfsSpec),
    ("RTS_029", GtfsSpec),
    ("RTS_030", MobilitydataParity),
    ("RTS_031", GtfsSpec),
    ("SAR_001", GtfsSpec),
    ("SAR_002", GtfsSpec),
    ("SAR_003", GtfsSpec),
    ("SAR_004", GtfsSpec),
    ("SHP_001", GtfsSpec),
    ("SHP_002", GtfsSpec),
    ("SHP_003", GtfsSpec),
    ("SHP_004", GtfsSpec),
    ("SHP_005", GtfsSpec),
    ("SHP_006", ProjectQuality),
    ("SHP_008", GtfsSpec),
    ("SHP_009", ProjectAnalytics),
    ("SHP_010", ProjectQuality),
    ("SHP_012", ProjectAnalytics),
    ("SHP_014", ProjectAnalytics),
    ("SHP_015", ProjectQuality),
    ("SHP_016", ProjectQuality),
    ("SHP_017", ProjectAnalytics),
    ("SHP_018", ProjectQuality),
    ("SHP_019", ProjectQuality),
    ("SHP_020", ProjectAnalytics),
    ("SHP_021", GtfsSpec),
    ("SHP_022", ProjectQuality),
    ("SHP_023", ProjectQuality),
    ("SHP_024", ProjectQuality),
    ("SHP_025", ProjectQuality),
    ("SHP_026", ProjectAnalytics),
    ("SHP_028", GtfsSpec),
    ("SHP_029", ProjectQuality),
    ("SHP_030", ProjectQuality),
    ("STM_001", GtfsSpec),
    ("STM_002", GtfsSpec),
    ("STM_003", GtfsSpec),
    ("STM_004", GtfsSpec),
    ("STM_005", GtfsSpec),
    ("STM_006", GtfsSpec),
    ("STM_007", MobilitydataParity),
    ("STM_008", MobilitydataParity),
    ("STM_009", GtfsSpec),
    ("STM_010", GtfsSpec),
    ("STM_012", MobilitydataParity),
    ("STM_014", ProjectAnalytics),
    ("STM_015", GtfsSpec),
    ("STM_016", GtfsSpec),
    ("STM_017", ProjectQuality),
    ("STM_018", GtfsSpec),
    ("STM_019", GtfsSpec),
    ("STM_020", ProjectQuality),
    ("STM_021", ProjectAnalytics),
    ("STM_022", GtfsSpec),
    ("STM_024", ProjectQuality),
    ("STM_025", ProjectAnalytics),
    ("STM_026", ProjectAnalytics),
    ("STM_028", ProjectAnalytics),
    ("STM_029", ProjectAnalytics),
    ("STM_030", GtfsSpec),
    ("STM_032", GtfsSpec),
    ("STM_033", MobilitydataParity),
    ("STM_034", MobilitydataParity),
    ("STM_035", ProjectAnalytics),
    ("STM_036", ProjectQuality),
    ("STM_037", GtfsSpec),
    ("STM_038", MobilitydataParity),
    ("STM_039", GtfsSpec),
    ("STM_040", ProjectQuality),
    ("STM_041", GtfsSpec),
    ("STM_042", GoogleTransitInterop),
    ("STM_043", ProjectAnalytics),
    ("STM_044", ProjectAnalytics),
    ("STM_045", ProjectAnalytics),
    ("STM_046", GtfsSpec),
    ("STM_047", GtfsSpec),
    ("STM_048", GtfsSpec),
    ("STM_049", GtfsSpec),
    ("STM_050", ProjectQuality),
    ("STM_051", GtfsSpec),
    ("STM_052", GtfsSpec),
    ("STM_053", ProjectAnalytics),
    ("STM_054", GtfsSpec),
    ("STM_055", GtfsSpec),
    ("STM_056", GtfsSpec),
    ("STM_058", GtfsSpec),
    ("STM_059", ProjectQuality),
    ("STM_060", GtfsSpec),
    ("STM_061", MobilitydataParity),
    ("STP_001", GtfsSpec),
    ("STP_002", GtfsSpec),
    ("STP_003", GtfsSpec),
    ("STP_004", GtfsSpec),
    ("STP_005", GtfsSpec),
    ("STP_006", GtfsSpec),
    ("STP_007", GtfsSpec),
    ("STP_008", GtfsSpec),
    ("STP_009", GtfsSpec),
    ("STP_010", GtfsSpec),
    ("STP_011", GtfsSpec),
    ("STP_012", GtfsSpec),
    ("STP_013", GtfsSpec),
    ("STP_014", GtfsSpec),
    ("STP_015", GtfsSpec),
    ("STP_016", ProjectQuality),
    ("STP_017", ProjectQuality),
    ("STP_018", GtfsSpec),
    ("STP_019", ProjectQuality),
    ("STP_020", ProjectAnalytics),
    ("STP_021", ProjectQuality),
    ("STP_022", ProjectQuality),
    ("STP_023", ProjectQuality),
    ("STP_024", ProjectQuality),
    ("STP_026", GtfsSpec),
    ("STP_028", ProjectQuality),
    ("STP_029", ProjectQuality),
    ("STP_030", ProjectQuality),
    ("STP_031", ProjectQuality),
    ("STP_032", ProjectQuality),
    ("STP_033", ProjectQuality),
    ("STP_034", ProjectQuality),
    ("STP_035", ProjectQuality),
    ("STP_036", GtfsSpec),
    ("STP_037", ProjectQuality),
    ("STP_038", ProjectQuality),
    ("STP_039", ProjectQuality),
    ("STP_040", ProjectQuality),
    ("STP_041", ProjectQuality),
    ("STP_042", GtfsSpec),
    ("STP_043", GtfsSpec),
    ("STP_044", ProjectQuality),
    ("STP_045", ProjectQuality),
    ("TFR_001", GtfsSpec),
    ("TFR_002", GtfsSpec),
    ("TFR_003", GtfsSpec),
    ("TFR_004", ProjectQuality),
    ("TFR_005", GtfsSpec),
    ("TFR_006", GtfsSpec),
    ("TFR_007", GtfsSpec),
    ("TFR_008", GtfsSpec),
    ("TRF_021", GtfsSpec),
    ("TRF_022", GtfsSpec),
    ("TRF_023", GtfsSpec),
    ("PTH_026", GtfsSpec),
    ("PTH_027", GtfsSpec),
    ("PTH_028", ProjectQuality),
    ("PTH_029", ProjectQuality),
    ("PTH_030", GtfsSpec),
    ("PTH_031", GtfsSpec),
    ("TRF_001", GtfsSpec),
    ("TRF_002", GtfsSpec),
    ("TRF_003", GtfsSpec),
    ("TRF_004", GtfsSpec),
    ("TRF_005", GtfsSpec),
    ("TRF_006", GtfsSpec),
    ("TRF_007", GtfsSpec),
    ("TRF_008", GtfsSpec),
    ("TRF_009", GtfsSpec),
    ("TRF_010", ProjectAnalytics),
    ("TRF_011", ProjectQuality),
    ("TRF_012", GtfsSpec),
    ("TRF_013", ProjectQuality),
    ("TRF_014", GtfsSpec),
    ("TRF_015", ProjectQuality),
    ("TRF_016", GtfsSpec),
    ("TRF_017", GtfsSpec),
    ("TRF_018", ProjectQuality),
    ("TRF_019", MobilitydataParity),
    ("TRF_020", ProjectQuality),
    ("TRN_001", GtfsSpec),
    ("TRN_002", GtfsSpec),
    ("TRN_003", GtfsSpec),
    ("TRN_004", GtfsSpec),
    ("TRN_005", GtfsSpec),
    ("TRN_006", GtfsSpec),
    ("TRN_007", ProjectQuality),
    ("TRN_008", GtfsSpec),
    ("TRN_009", GtfsSpec),
    ("TRN_010", GtfsSpec),
    ("TRN_011", GtfsSpec),
    ("TRN_013", GtfsSpec),
    ("TRN_014", GtfsSpec),
    ("TRN_015", GtfsSpec),
    ("TRN_016", GtfsSpec),
    ("TRN_017", GtfsSpec),
    ("TRP_001", GtfsSpec),
    ("TRP_002", GtfsSpec),
    ("TRP_003", GtfsSpec),
    ("TRP_004", GtfsSpec),
    ("TRP_005", GtfsSpec),
    ("TRP_006", GtfsSpec),
    ("TRP_007", GtfsSpec),
    ("TRP_011", ProjectQuality),
    ("TRP_012", ProjectQuality),
    ("TRP_013", ProjectAnalytics),
    ("TRP_014", ProjectQuality),
    ("TRP_015", ProjectQuality),
    ("TRP_017", ProjectQuality),
    ("TRP_019", GtfsSpec),
    ("TRP_020", ProjectAnalytics),
    ("TRP_021", ProjectQuality),
    ("TRP_022", MobilitydataParity),
    ("TRP_023", ProjectQuality),
    ("TRP_024", ProjectQuality),
    ("TRP_025", ProjectQuality),
    ("TRP_026", ProjectAnalytics),
    ("TRP_028", ProjectQuality),
    ("TRP_029", ProjectQuality),
    ("TRP_031", GtfsSpec),
    ("TRP_032", GtfsSpec),
    // The Spec does not EXPLICITLY require a single mode in a block → NOT GtfsSpec, a project decision.
    ("TRP_034", GtfsSpec),
    ("TRP_035", GtfsSpec),
    ("VAT_001", ProjectAnalytics),
    ("VAT_002", ProjectAnalytics),
    ("VAT_003", ProjectAnalytics),
    ("VAT_005", ProjectAnalytics),
    ("VAT_006", ProjectAnalytics),
    ("VAT_007", ProjectAnalytics),
    ("VAT_008", ProjectAnalytics),
    ("XFL_002", MobilitydataParity),
    ("XFL_006", ProjectAnalytics),
    ("XFL_012", ProjectQuality),
    ("XFL_013", ProjectQuality),
    ("XFL_014", ProjectQuality),
    ("XFL_015", GtfsSpec),
    ("XFL_016", GtfsSpec),
    ("XFL_019", GtfsSpec),
    ("XFL_020", GtfsSpec),
    ("XFL_021", MobilitydataParity),
    ("XFL_022", GtfsSpec),
    ("XFL_023", GtfsSpec),
    ("XFL_024", GtfsSpec),
    ("XFL_025", GtfsSpec),
    ("XFL_031", GtfsSpec),
    ("XFL_032", GtfsSpec),
    ("XFL_033", GtfsSpec),
    ("XFL_034", GtfsSpec),
    ("XFL_027", GoogleTransitInterop),
    ("XFL_028", ProjectQuality),
    ("XFL_029", ProjectQuality),
    ("XFL_030", ProjectQuality),
];

/// O(1) lookup through `AUTHORITY` from ID → AuthoritySource (`rule_index` pattern).
fn authority_index() -> &'static std::collections::HashMap<&'static str, AuthoritySource> {
    static IDX: std::sync::OnceLock<std::collections::HashMap<&'static str, AuthoritySource>> =
        std::sync::OnceLock::new();
    IDX.get_or_init(|| AUTHORITY.iter().map(|&(k, v)| (k, v)).collect())
}

/// Authority source for a rule ID. Returns `Unknown` when it is not known.
pub fn authority_source(id: &str) -> AuthoritySource {
    authority_index()
        .get(id)
        .copied()
        .unwrap_or(AuthoritySource::Unknown)
}

impl RuleMeta {
    /// Authority source for this rule (the basis legitimizing its class).
    pub fn authority_source(&self) -> AuthoritySource {
        authority_source(self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Authority gate (completed in Phase 3, hard-fail in Phase 4): `Sınıf=Spec` is legitimate ONLY
    /// with `authority=GtfsSpec`. During Phases 1–3, 45→0 mismatched rules were removed
    /// (batch history: `notgit/audits/`). There is now NO exception/allowlist—violation = CI red.

    #[test]
    fn no_duplicate_ids() {
        let mut ids: Vec<&str> = RULES.iter().map(|r| r.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "Tekrar eden kural ID'si var");
    }

    /// The AUTHORITY table must cover every RULES rule exactly (no drift):
    /// no missing, orphaned, or duplicate entries. This is the foundation of authority SSOT integrity.
    #[test]
    fn authority_covers_all_rules() {
        use std::collections::HashSet;
        let rule_ids: HashSet<&str> = RULES.iter().map(|r| r.id).collect();
        let auth_ids: HashSet<&str> = AUTHORITY.iter().map(|&(k, _)| k).collect();
        assert_eq!(AUTHORITY.len(), auth_ids.len(), "AUTHORITY'de mükerrer id");
        let missing: Vec<&str> = rule_ids.difference(&auth_ids).copied().collect();
        let orphan: Vec<&str> = auth_ids.difference(&rule_ids).copied().collect();
        assert!(
            missing.is_empty(),
            "AUTHORITY'de eksik kural(lar): {missing:?}"
        );
        assert!(
            orphan.is_empty(),
            "AUTHORITY'de yetim giriş(ler): {orphan:?}"
        );
    }

    /// Core authority-integrity invariant: while a rule is in the `Spec` class,
    /// authority_source cannot be ≠ GtfsSpec. Existing violations were cleared at the end of
    /// Phase 3; the allowlist remains empty.
    #[test]
    fn spec_class_requires_gtfs_spec_authority() {
        let violations: Vec<&str> = RULES
            .iter()
            .filter(|r| r.rule_class == RuleClass::Spec)
            .filter(|r| authority_source(r.id) != AuthoritySource::GtfsSpec)
            .map(|r| r.id)
            .collect();
        assert!(
            violations.is_empty(),
            "Spec sınıfı ama authority≠GtfsSpec ve allowlist'te değil: {violations:?}"
        );
    }

    /// Class ⟺ authority biconditional: every rule's `rule_class` must exactly match the class
    /// expected from its `authority_source`. Strengthened form of
    /// `spec_class_requires_gtfs_spec_authority`—it catches not only Spec⟹GtfsSpec but also the
    /// reverse direction (e.g. a rule downgraded to Quality while AUTHORITY is left at
    /// GtfsSpec/MobilitydataParity).
    #[test]
    fn class_matches_authority_source() {
        use AuthoritySource::*;
        use RuleClass::*;
        // authority → expected class. Unknown: no fixed class, only the invariant "never Spec".
        fn expected(a: AuthoritySource) -> Option<RuleClass> {
            match a {
                GtfsSpec => Some(Spec),
                GtfsBestPractice | ProjectQuality => Some(Quality),
                MobilitydataParity | GoogleTransitInterop | RegionalProfile => Some(Interop),
                ProjectAnalytics => Some(Analytics),
                Unknown => None,
            }
        }
        let violations: Vec<String> = RULES
            .iter()
            .filter_map(|r| {
                let a = authority_source(r.id);
                match expected(a) {
                    Some(c) if r.rule_class != c => Some(format!(
                        "{}: class={:?} ama authority={a:?} → beklenen class={c:?}",
                        r.id, r.rule_class
                    )),
                    None if r.rule_class == Spec => {
                        Some(format!("{}: authority=Unknown asla Spec olamaz", r.id))
                    }
                    _ => None,
                }
            })
            .collect();
        assert!(
            violations.is_empty(),
            "sınıf↔otorite tutarsızlıkları:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn geo_view_ids_absent() {
        assert!(
            get_rule("GEO_008").is_none(),
            "GEO_008 registry'de olmamalı"
        );
        assert!(
            get_rule("GEO_010").is_none(),
            "GEO_010 registry'de olmamalı"
        );
        assert!(
            get_rule("GEO_011").is_none(),
            "GEO_011 registry'de olmamalı"
        );
    }

    #[test]
    fn removed_ids_absent() {
        assert!(get_rule("STM_011").is_none());
        assert!(get_rule("TRP_010").is_none());
        assert!(get_rule("GEO_001").is_none());
        assert!(get_rule("GEO_005").is_none());
        assert!(get_rule("DQ_007").is_none());
        assert!(get_rule("DQ_008").is_none());
        assert!(get_rule("DQ_015").is_none());
        // STM_027 (2026-07-28): removed because it duplicated the same finding as STM_056.
        assert!(get_rule("STM_027").is_none());
        // SHP_027 (2026-07-31): shapes.txt represents the PHYSICAL route, while stop_times
        // represents the STOPPING MODEL; trains stopping at different stops on the same corridor
        // may normally share one shape. GEO_009/SHP_012 already measure the remaining real signal
        // (the stop is not on the shape) DIRECTLY.
        assert!(get_rule("SHP_027").is_none());
        // STM_057 (2026-08-01, WP-1): the spec says "travel WITHIN THE SAME location group…requires two
        // records"; the rule turned this into a two-record requirement for every location and rejected
        // a normal point-to-point Flex trip (L1 → L2) as Kritik·Spec. The only auditable remainder
        // (a trip's sole record being a Flex location) is covered by STM_033.
        assert!(get_rule("STM_057").is_none());
        // FPD_006 (2026-08-09): an empty rider_category_id is not a default; duplicate
        // composite keys are measured by FPD_001, while is_default_fare_category is measured by RCT_006.
        assert!(get_rule("FPD_006").is_none());
        // STP_025 (2026-09-18): leading/trailing whitespace in `stop_name` was an exact subset of
        // `DQ_016`—in a measurement of 1,366 feeds, it fired TOGETHER with DQ_016 in all 481 affected
        // feeds; it was counted twice in the same Quality score cell.
        assert!(get_rule("STP_025").is_none());
        // BKR_002 (2026-09-19): imposed a companion requirement between `prior_notice_start_day`
        // and `prior_notice_last_day` that is not in the spec. The ID was retired and must not be
        // assigned to another rule in the future.
        assert!(get_rule("BKR_002").is_none());
        // STM_013 (2026-09-21): treated optional intermediate stop times as a Quality violation;
        // retired because it produced systematic false positives in timing-point producers.
        assert!(get_rule("STM_013").is_none());
        // STP_027 (2026-09-21): the optional stop_access check overlapped STP_024/STP_026;
        // absence of the field at a pathway station is not a defect by itself.
        assert!(get_rule("STP_027").is_none());
        // FAR_009 (2026-09-21): because fare_rules.txt is optional, a general fare + empty
        // fare_rules is a valid model; FRL_006 preserves reference consistency.
        assert!(get_rule("FAR_009").is_none());
        // CAL_019 / XFL_011 (2026-10-11): two copies comparing the raw calendar range with the
        // feed_info window; the spec recommends data outside the window, and the MD counterpart is CAL_014.
        assert!(get_rule("CAL_019").is_none());
        assert!(get_rule("XFL_011").is_none());
    }

    #[test]
    fn blocks_only_canonical_ids() {
        let view_ids = ["GEO_008", "GEO_010", "GEO_011"];
        let removed_ids = [
            "STM_011", "TRP_010", "GEO_001", "GEO_005", "DQ_007", "DQ_008", "DQ_015", "STM_027",
            "SHP_027", "STM_057", "AGN_001", "FPD_006", "STP_025", "BKR_002", "STM_013", "TRP_033",
            "STP_027", "FAR_009", "CAL_019", "XFL_011",
        ];
        for rule in RULES {
            for &b in rule.blocks {
                assert!(
                    !view_ids.contains(&b),
                    "{}: blocks[] içinde rapor görünüm ID'si var: {}",
                    rule.id,
                    b
                );
                assert!(
                    !removed_ids.contains(&b),
                    "{}: blocks[] içinde kaldırılmış ID var: {}",
                    rule.id,
                    b
                );
                assert!(
                    get_rule(b).is_some(),
                    "{}: blocks[] içinde bilinmeyen canonical ID: {}",
                    rule.id,
                    b
                );
            }
        }
    }

    #[test]
    fn get_rule_lookup() {
        assert!(get_rule("STP_003").is_some());
        assert!(get_rule("STM_014").is_some());
        assert!(get_rule("NONEXISTENT").is_none());
    }

    #[test]
    fn no_score_rules_are_explicit() {
        for rule_id in [
            "CAL_009", "CAL_014", "CAL_015", "CAL_017", "CAL_024", "GEO_009", "ARC_006",
            "STM_017", "STM_021", "STM_026", "STM_035", "OPR_007", "OPR_008", "STM_045",
            "STP_022", "ARC_022", "ARC_010", "CLD_006", "ATR_001", "DQ_006", "RTS_017", "TRP_011",
            "TRP_013", "DQ_013", "PTH_008", "PTH_009", "PTH_025", "PTH_029", "STP_039",
            "XFL_027",
        ] {
            assert_eq!(get_rule(rule_id).unwrap().score_weight, 0.0, "{rule_id}");
        }
        assert_eq!(get_rule("OPR_016").unwrap().score_weight, 1.0);
        for rule in RULES.iter().filter(|rule| rule.score_weight == 0.0) {
            assert!(
                !(matches!(rule.severity, Severity::Kritik) && rule.rule_class == RuleClass::Spec),
                "no-score rule cannot be Spec/Kritik: {}",
                rule.id
            );
        }
    }

    #[test]
    fn base_effort_valid() {
        for rule in RULES {
            assert!(
                matches!(rule.base_effort, 1..=3),
                "{}: geçersiz base_effort = {}",
                rule.id,
                rule.base_effort
            );
        }
    }

    #[test]
    fn report_views_nonempty() {
        for rule in RULES {
            assert!(
                !rule.report_views.is_empty(),
                "{}: report_views boş olamaz",
                rule.id
            );
        }
    }

    #[test]
    fn interop_always_has_r8() {
        use gtfs_core::{ReportId::R8, RuleClass::Interop};
        for rule in RULES {
            if rule.rule_class == Interop {
                assert!(
                    rule.report_views.contains(&R8),
                    "{}: INTEROP kuralı R8'i içermeli",
                    rule.id
                );
            }
        }
    }

    #[test]
    fn blocker_rules_have_r1() {
        use gtfs_core::{ReportId::R1, RuleClass::Spec, Severity::Kritik};
        // Authority integrity (Phase 4): the R1 publication block is only `Spec` + `Kritik`.
        // build_r1/is_pub_relevant now uses ONLY the Spec+Kritik gate (Interop does not produce
        // R1). report_views/R1 metadata cleanup is a separate WP. This test verifies only that
        // Spec+Kritik rules have R1 in report_views (not the reverse).
        for rule in RULES {
            if rule.severity == Kritik && rule.rule_class == Spec {
                assert!(
                    rule.report_views.contains(&R1),
                    "{}: Spec+Kritik kural R1'i içermeli",
                    rule.id
                );
            }
        }
    }

    #[test]
    fn spec_severity_rubric_has_no_info_findings() {
        use gtfs_core::{
            RuleClass::Spec,
            Severity::{Bilgi, Yuksek},
        };

        let info_rules: Vec<&str> = RULES
            .iter()
            .filter(|rule| rule.rule_class == Spec && rule.severity == Bilgi)
            .map(|rule| rule.id)
            .collect();
        assert!(
            info_rules.is_empty(),
            "normatif Spec ihlalleri Bilgi olamaz; yalnız ölçüm/bağlam bulguları Bilgi olabilir: {info_rules:?}"
        );

        for id in ["STM_048", "STM_049"] {
            let rule = get_rule(id).expect("STM raw midnight rule registry'de olmalı");
            assert_eq!(rule.rule_class, Spec);
            assert_eq!(
                rule.severity, Yuksek,
                "{id} ham servis-günü ihlali Yüksek olmalı"
            );
        }
    }

    #[test]
    fn all_rules_have_title() {
        for rule in RULES {
            assert!(!rule.title.is_empty(), "{}: title boş olamaz", rule.id);
        }
    }
}
