use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    #[serde(rename = "CRITICAL")]
    Kritik,
    #[serde(rename = "HIGH")]
    Yuksek,
    #[serde(rename = "MEDIUM")]
    Orta,
    #[serde(rename = "LOW")]
    Dusuk,
    #[serde(rename = "INFO")]
    Bilgi,
}

impl Severity {
    /// severity_weight used in the R9 score formula.
    pub fn weight(self) -> f64 {
        match self {
            Self::Kritik => 4.0,
            Self::Yuksek => 3.0,
            Self::Orta => 2.0,
            Self::Dusuk => 1.0,
            Self::Bilgi => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleClass {
    #[serde(rename = "SPEC")]
    Spec,
    #[serde(rename = "INTEROP")]
    Interop,
    #[serde(rename = "QUALITY")]
    Quality,
    #[serde(rename = "ANALYTICS")]
    Analytics,
}

/// A rule's **authority source** — the basis for the legitimacy of its (`RuleClass`).
///
/// Class-authority integrity (from the authority audit ledger; locally notgit/audits/):
/// the `Spec` class is legitimate ONLY with the `GtfsSpec` source. Other sources cannot
/// produce Spec. Mapping: GtfsSpec→Spec | GtfsBestPractice→Quality | MobilitydataParity/
/// GoogleTransitInterop/RegionalProfile→Interop | ProjectQuality→Quality |
/// ProjectAnalytics→Analytics | Unknown→never Spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthoritySource {
    /// An explicit normative provision in the official GTFS Schedule Reference
    /// (required/enum/FK/uniqueness/format).
    #[serde(rename = "GTFS_SPEC")]
    GtfsSpec,
    /// Official GTFS Best Practices documentation — NOT a normative spec.
    #[serde(rename = "GTFS_BEST_PRACTICE")]
    GtfsBestPractice,
    /// MobilityData validator parity (interop signal only).
    #[serde(rename = "MOBILITYDATA_PARITY")]
    MobilitydataParity,
    /// Google Transit consumer behavior.
    #[serde(rename = "GOOGLE_TRANSIT_INTEROP")]
    GoogleTransitInterop,
    /// Regional profile (e.g. GTFS-JP) — not the official Schedule Reference.
    #[serde(rename = "REGIONAL_PROFILE")]
    RegionalProfile,
    /// Project-specific data-quality / readability check.
    #[serde(rename = "PROJECT_QUALITY")]
    ProjectQuality,
    /// Project-specific statistical / operational signal.
    #[serde(rename = "PROJECT_ANALYTICS")]
    ProjectAnalytics,
    /// Authority not yet determined — can never be Spec.
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

/// Indicates the GTFS entity granularity at which a notice was produced.
///
/// Strategy (architecture Section 3):
/// - `frequencies`, `calendar_dates`, `fare_rules` → `Row`
/// - `feed_info` → `Feed`
/// - other entities → the corresponding variant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityType {
    Feed,
    File,
    Agency,
    Stop,
    Route,
    Trip,
    Shape,
    Service,
    Fare,
    Transfer,
    Pathway,
    Level,
    Translation,
    Attribution,
    Row,
}

impl EntityType {
    /// Stable string representation for fixed contexts such as a dedup key.
    /// Unlike the Debug format, it does not change when an enum variant is renamed.
    pub fn stable_name(self) -> &'static str {
        match self {
            Self::Feed => "feed",
            Self::File => "file",
            Self::Agency => "agency",
            Self::Stop => "stop",
            Self::Route => "route",
            Self::Trip => "trip",
            Self::Shape => "shape",
            Self::Service => "service",
            Self::Fare => "fare",
            Self::Transfer => "transfer",
            Self::Pathway => "pathway",
            Self::Level => "level",
            Self::Translation => "translation",
            Self::Attribution => "attribution",
            Self::Row => "row",
        }
    }
}

/// Error code that stops validation entirely.
///
/// ⚠️ **NOT EVERY VARIANT IS PRODUCED.** These comments once mapped three variants to
/// ARC_002/ARC_004/ARC_013 as their "fatal equivalents"; `66e009c2` (2026-08-09) DELIBERATELY
/// removed those three paths and changed them to partial recovery (`ValidationStatus::Partial`),
/// but the comments remained stale. `spec-audit/silent_rules_scan.py`, which relied on the
/// comments, incorrectly labeled the three rules as a "fatal path" and produced a stale
/// "triggered" result in `fatals.csv` (issue #132). The labels below have been measured —
/// MEASURE before changing them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FatalCode {
    /// ✅ LIVE — `k1_parse`: ZIP could not be opened (ARC_001).
    ZipUnreadable,
    /// ⛔ PRODUCES NO RUST CODE. A UTF-8 violation in a required file now produces an
    /// `ARC_002` NOTICE + Partial; the feed is not rejected.
    Utf8Critical,
    /// ⛔ PRODUCES NO RUST CODE. A missing required file now produces an `ARC_004` NOTICE +
    /// Partial (`66e009c2`).
    NoRequiredFiles,
    /// ⛔ PRODUCES NO RUST CODE. A tokenization error now produces an `ARC_013` NOTICE and
    /// adds the file to `partial.unavailable_files`.
    CsvMalformed,
    /// ✅ LIVE — `k1_parse`: zip bomb / decompressed-size limit exceeded (ARC_029).
    DecompressionLimit,
    /// ✅ LIVE but ONLY on the TypeScript side: produced by the `ui/src/validator-client.ts`
    /// worker for a crash or timeout. Rust never constructs this variant.
    ResourceLimit,
    /// ✅ LIVE — `wasm`: input data is invalid (e.g. a config JSON parse error).
    InvalidInput,
}

/// Indicates the granularity at which a notice is deduplicated.
///
/// Dedup key (architecture Section K1):
/// - `Feed`   → rule_id
/// - `Entity` → rule_id + entity_type + entity_id
/// - `Row`    → rule_id + file + line
/// - `Field`  → rule_id + file + line + field
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DedupLevel {
    /// rule_id → one notice per feed
    Feed,
    /// rule_id + file → one notice per file (ARC file-level checks)
    File,
    /// rule_id + entity_type + entity_id → one notice per entity
    Entity,
    /// rule_id + file + line → one notice per row
    Row,
    /// rule_id + file + line + field → one notice per field
    Field,
}

/// Data-report ID. R1–R5, R7–R9.
/// R6 is not a report (PDF/HTML export action); it is not included in this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReportId {
    R1,
    R2,
    R3,
    R4,
    R5,
    R7,
    R8,
    R9,
}

/// R9 remediation queue labels — conditions are defined in architecture Section 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum R9Label {
    /// KRİTİK + SPEC — an error that definitively blocks publication under the official GTFS spec.
    #[serde(rename = "blocker")]
    Blocker,
    /// INTEROP class — consumer/interop compatibility signal (not a publication blocker).
    #[serde(rename = "interop")]
    Interop,
    /// realized_dependent_count > 2
    #[serde(rename = "propagation")]
    Propagation,
    /// fix_effort == 1 AND severity >= YÜKSEK
    #[serde(rename = "quick-win")]
    QuickWin,
    /// QUALITY class + YÜKSEK severity
    #[serde(rename = "quality")]
    Quality,
    /// affected_instance_count > 20
    #[serde(rename = "widespread")]
    Widespread,
    /// Analytics-class rule.
    #[serde(rename = "analytics")]
    Analytics,
    /// fix_effort ≥ 4.0 — requires a broad data revision.
    /// After normalizing base_effort to the 1-2-3 scale, the threshold is deliberately kept
    /// at 4.0: fix_effort = base_effort × instance_multiplier (max 2×), so the upper bound is
    /// 6.0. The 4.0 threshold means base_effort=3 + widespread (≥1.5×), or base_effort=2 +
    /// very widespread (2×). If lowered to 3.0, structural rules alone (base_effort=3) would
    /// count as Hard and the label would be inflated.
    #[serde(rename = "hard")]
    Hard,
    /// affected_instance_count == 1 — a one-row error, easy to find.
    #[serde(rename = "single")]
    Single,
    /// score_delta or pub_score_delta is high — fixing it makes a large contribution to the score.
    #[serde(rename = "high-impact")]
    HighImpact,
}
