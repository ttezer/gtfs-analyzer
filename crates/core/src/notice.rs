// BTreeMap (not HashMap): `details` is serialized to JSON, and HashMap iteration order
// varies between processes → the same input would produce different BYTES in two runs
// (same content, only key order differs). See the same rationale in NameIndex.
use crate::{EntityType, RuleClass, Severity};
use serde::de::Deserializer;
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::BTreeMap;
use std::ops::Index;

/// Compact, deterministic replacement for the per-notice `BTreeMap` used by
/// `Notice.details`.
///
/// Detail sets are small, so a sorted vector avoids one tree-node allocation per
/// key while preserving the same lookup and JSON-object semantics. The public JSON
/// shape remains an object with sorted string keys.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NoticeDetails(Vec<(String, String)>);

impl NoticeDetails {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) -> Option<String> {
        let key = key.into();
        let value = value.into();
        match self.0.binary_search_by(|(existing, _)| existing.as_str().cmp(&key)) {
            Ok(index) => Some(std::mem::replace(&mut self.0[index].1, value)),
            Err(index) => {
                self.0.insert(index, (key, value));
                None
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.0
            .binary_search_by(|(existing, _)| existing.as_str().cmp(key))
            .ok()
            .map(|index| &self.0[index].1)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, (String, String)> {
        self.0.iter()
    }

    pub fn values(&self) -> impl Iterator<Item = &String> {
        self.0.iter().map(|(_, value)| value)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<BTreeMap<String, String>> for NoticeDetails {
    fn from(details: BTreeMap<String, String>) -> Self {
        Self(details.into_iter().collect())
    }
}

impl FromIterator<(String, String)> for NoticeDetails {
    fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
        let mut details = Self::new();
        for (key, value) in iter {
            details.insert(key, value);
        }
        details
    }
}

impl Index<&str> for NoticeDetails {
    type Output = String;

    fn index(&self, key: &str) -> &Self::Output {
        self.get(key).unwrap_or_else(|| panic!("no entry found for key: {key}"))
    }
}

impl Serialize for NoticeDetails {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for NoticeDetails {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BTreeMap::<String, String>::deserialize(deserializer).map(Into::into)
    }
}

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
    pub details: Option<NoticeDetails>,
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

#[cfg(test)]
mod tests {
    use super::NoticeDetails;

    #[test]
    fn details_keep_sorted_json_and_replace_existing_keys() {
        let mut details = NoticeDetails::new();
        assert_eq!(details.insert("z", "last"), None);
        assert_eq!(details.insert("a", "first"), None);
        assert_eq!(details.insert("z", "updated"), Some("last".to_string()));
        assert_eq!(details.get("z").map(String::as_str), Some("updated"));
        assert_eq!(details["a"], "first");

        let json = serde_json::to_string(&details).expect("details serialize");
        assert_eq!(json, r#"{"a":"first","z":"updated"}"#);
        let decoded: NoticeDetails = serde_json::from_str(&json).expect("details deserialize");
        assert_eq!(decoded, details);
    }
}
