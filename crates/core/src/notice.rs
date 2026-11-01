// BTreeMap (not HashMap): `details` is serialized to JSON, and HashMap iteration order
// varies between processes → the same input would produce different BYTES in two runs
// (same content, only key order differs). See the same rationale in NameIndex.
use crate::{EntityType, RuleClass, Severity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Canonical data structure representing one validation finding.
///
/// Critical contracts (architecture Section 3):
/// - `blocks`: canonical rule IDs only — does not include report-view IDs (GEO_008/010/011)
/// - `scope_key`: determines which linking entity the rule definition uses; runtime assigns
///   it from the relevant entity ID when producing the notice
/// - `base_effort`: 1 / 2 / 3 — fixed per rule (1=single field/row, 2=limited cross-file, 3=structural/topological)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notice {
    /// Unique ID assigned at creation — used by ReportItem references.
    pub id: String,
    pub rule_id: String,
    pub severity: Severity,
    pub rule_class: RuleClass,
    pub entity_type: EntityType,
    pub entity_id: Option<String>,
    /// Linking entity ID used to match root causes to symptoms.
    /// Pipe-separated when a compound key is required: "from_stop_id|to_stop_id".
    /// If None, this is a feed-level notice; a root cause with scope_key == None affects the entire feed.
    pub scope_key: Option<String>,
    pub file: Option<String>,
    pub line: Option<u64>,
    pub field: Option<String>,
    pub observed_value: Option<String>,
    /// Threshold or expected value (e.g. "120 km/h") — for dynamic context.
    pub expected_value: Option<String>,
    /// Additional context: second entity ID, calculated value, unit, etc.
    pub details: Option<BTreeMap<String, String>>,
    /// Type/enum symptom produced by K2 only because of surrounding whitespace.
    /// An internal pipeline marker; K7 creates the old `details` shape for retained findings.
    #[serde(skip)]
    pub whitespace_derived: bool,
    /// Whitespace-adjacent FK candidate awaiting validation against the trimmed reference in K4.
    /// An internal pipeline marker; K7 creates the old `details` shape for retained findings.
    #[serde(skip)]
    pub whitespace_candidate: bool,
    /// Short general Turkish title for the rule — displayed in the UI.
    pub title: String,
    /// Fixed text from the rule definition — not generated at runtime.
    pub message: String,
    /// Fixed remediation instruction for the rule.
    pub remediation: String,
    /// Downstream canonical rule IDs that will be closed if this notice is fixed.
    pub blocks: Vec<String>,
    /// 1=easy, 3=hard. fix_effort = base_effort × instance_multiplier.
    pub base_effort: u8,
    /// Service calendar the finding belongs to (service_id). Filled for trip-level findings
    /// (the "Service Calendar" column in R2). None for feed/file rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,
    /// Agency distribution of raw notices under a feed-level summary (#2201). Filled only for
    /// aggregated summaries; remains `None` for other notices (8 bytes). An internal pipeline
    /// marker, not serialized: the JSON contract is a separate decision.
    #[serde(skip)]
    pub agency_distribution: Option<Box<crate::agency::AgencyDistribution>>,
}

/// Reference object projecting a canonical Notice into a report view.
/// The Notice is not cloned; it is referenced through `notice_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportItem {
    /// The `id` field of the related canonical Notice.
    pub notice_id: String,
    pub report_id: crate::ReportId,
    /// Display label specific to the report context.
    /// For example, SHP_013 → "GEO_008" in R4.
    pub display_label: String,
    pub is_primary: bool,
}
