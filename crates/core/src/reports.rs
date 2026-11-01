use crate::{R9Label, ReportItem};
use serde::{Deserialize, Serialize};

/// R1: Publishability decision — the official GTFS spec gate.
/// Only `Spec + Kritik` (authority `GtfsSpec`) blocks publication; other classes (Interop/
/// Quality/Analytics) do not affect R1. Interop/consumer readiness is separate: R8 + interop_score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R1Report {
    /// Is the feed publishable? This is **NOT THE SAME AS `blockers.is_empty()`** (issue #133):
    /// when validation coverage is incomplete, no findings means "COULD NOT INSPECT", not
    /// "clean". Therefore `publishable = blocker YOK ∧ coverage_complete`.
    pub publishable: bool,
    /// Was ALL evidence required for the publication decision collected?
    ///
    /// When `false`, a required file (agency/stops/routes/trips/stop_times or a calendar
    /// file) could not be read, and rules depending on that file did not run AT ALL.
    /// ⚠️ Failure to read an optional file does NOT LOWER this flag — blocking a valid feed
    /// because of a broken `attributions.txt` is the serious error named in the project's
    /// `PTH_017` asymmetry.
    #[serde(default = "default_true")]
    pub coverage_complete: bool,
    pub blocker_notice_ids: Vec<String>,
}

fn default_true() -> bool {
    true
}

/// R2: Technical list of all notices — including file/line/field/observed_value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R2Report {
    pub items: Vec<ReportItem>,
}

/// R3: Operational quality — ANALYTICS-class notices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R3Report {
    pub items: Vec<ReportItem>,
}

/// R4: Geographic accuracy report.
/// Built from SHP_013/014/016 notices; display_label = "GEO_008/010/011".
/// No separate Notice is produced for GEO_008/010/011.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R4Report {
    pub items: Vec<ReportItem>,
}

/// R5: Overall scorecard — weighted score from 0–100
/// (Spec×40%+Interop×30%+Quality×20%+Analytics×10%) plus class breakdown (MET_004).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R5Report {
    pub score: f64,
    /// Publishability score: only from blocker-eligible notices (Kritik SPEC/INTEROP
    /// + high INTEROP). 100 = clean, 0 = heavy blocker load.
    pub pub_score: f64,
    pub spec_score: f64,
    pub interop_score: f64,
    pub quality_score: f64,
    pub analytics_score: f64,
}

/// R7: Accessibility report — wheelchair_*, stop_access, PTH_012/013.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R7Report {
    pub items: Vec<ReportItem>,
}

/// R8: Journey planner readiness report — INTEROP class only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R8Report {
    pub items: Vec<ReportItem>,
}

/// One remediation item in the R9 queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R9Item {
    pub rule_id: String,
    pub labels: Vec<R9Label>,
    pub priority_score: f64,
    /// How many points would the R5 overall score increase if this rule's notices were fixed?
    pub score_delta: f64,
    /// How many points would the R5 pub_score increase if this rule's notices were fixed?
    /// Greater than zero only for blocker-eligible rules.
    pub pub_score_delta: f64,
    pub affected_instance_count: u32,
    /// Number of rules in blocks[] that actually fired for this feed.
    pub realized_dependent_count: u32,
    pub base_effort: u8,
    pub fix_effort: f64,
    /// Related canonical Notice IDs.
    pub notice_ids: Vec<String>,
}

/// R9: Remediation queue — bucket-first, then descending priority_score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R9Report {
    pub items: Vec<R9Item>,
}

/// Container for all report views.
/// R6 is an export action — the data structure is managed by the UI and does not appear here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSet {
    pub r1: R1Report,
    pub r2: R2Report,
    pub r3: R3Report,
    pub r4: R4Report,
    pub r5: R5Report,
    pub r7: R7Report,
    pub r8: R8Report,
    pub r9: R9Report,
}
