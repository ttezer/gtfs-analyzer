use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// BTreeMap (not HashMap): all of these types are serialized to JSON, and `HashMap`
// iteration order changes with a different seed on every PROCESS → the same binary + feed
// would produce different BYTES on two runs. The content was always the same; only key order moved.
// That made golden comparisons with `cmp`/`shasum` and the "bit-identical output" gate
// (a prerequisite for K2 chunk-parallel work) impossible. BTreeMap fixes order by key.
// See the 2026-07-27 determinism round — that round fixed the `dedup` representative;
// this round closes serialization ordering.
use crate::{FatalCode, FeedMetrics, Notice, ReportSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FatalError {
    pub code: FatalCode,
    /// The pipeline's own (Turkish) text. This is shown if `params` cannot be translated.
    pub message: String,
    /// Message-template parameters. `variant` selects the template key as `{code}.{variant}`;
    /// other keys fill `{placeholder}`. CLI, SDK, and UI translate the fatal message using the
    /// SAME model as notices; an empty map is omitted from JSON.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
}

impl FatalError {
    pub fn new(code: FatalCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            params: BTreeMap::new(),
        }
    }

    pub fn variant(self, variant: &str) -> Self {
        self.param("variant", variant)
    }

    pub fn param(mut self, key: &str, value: impl ToString) -> Self {
        self.params.insert(key.to_string(), value.to_string());
        self
    }

    /// Key in the template dictionary, such as `ZipUnreadable.entry_read`.
    pub fn template_key(&self) -> String {
        match self.params.get("variant") {
            Some(variant) => format!("{:?}.{variant}", self.code),
            None => format!("{:?}", self.code),
        }
    }
}

/// Structural recovery information that explains why a validation report has
/// reduced coverage while preserving findings from readable inputs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PartialReport {
    pub root_structural_errors: Vec<String>,
    pub unavailable_files: Vec<String>,
    pub skipped_stages: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_checks: Vec<String>,
}

impl PartialReport {
    pub fn is_empty(&self) -> bool {
        self.root_structural_errors.is_empty()
            && self.unavailable_files.is_empty()
            && self.skipped_stages.is_empty()
            && self.skipped_checks.is_empty()
    }

    pub fn mark_root_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        if !self.root_structural_errors.contains(&message) {
            self.root_structural_errors.push(message);
        }
    }

    pub fn mark_unavailable(&mut self, file: impl Into<String>) {
        let file = file.into();
        if !self.unavailable_files.contains(&file) {
            self.unavailable_files.push(file);
        }
    }

    pub fn skip_stage(&mut self, stage: impl Into<String>) {
        let stage = stage.into();
        if !self.skipped_stages.contains(&stage) {
            self.skipped_stages.push(stage);
        }
    }

    pub fn skip_check(&mut self, check: impl Into<String>) {
        let check = check.into();
        if !self.skipped_checks.contains(&check) {
            self.skipped_checks.push(check);
        }
    }

    pub fn extend_skipped_checks(&mut self, checks: impl IntoIterator<Item = String>) {
        for check in checks {
            self.skip_check(check);
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValidationStatus {
    #[serde(rename = "COMPLETE")]
    Complete,
    #[serde(rename = "PARTIAL")]
    Partial,
}

/// Lookup table for showing readable names in the UI instead of entity_id.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NameIndex {
    pub stops: BTreeMap<String, String>,       // stop_id → stop_name
    pub routes: BTreeMap<String, String>,      // route_id → short_name (or long_name)
    pub trips: BTreeMap<String, String>,       // trip_id → trip_headsign
    pub trip_routes: BTreeMap<String, String>, // trip_id → route_id (for filtering)
    pub trip_directions: BTreeMap<String, String>, // trip_id → "0"/"1" (direction_id; absent if missing)
    pub stop_coords: BTreeMap<String, [f64; 2]>, // stop_id → [lat, lon] (for the map)
    pub trip_first_dep: BTreeMap<String, String>, // trip_id → "HH:MM" (trip time)
    pub shape_routes: BTreeMap<String, Vec<[String; 2]>>, // shape_id → [[route_id, direction]]
    pub shape_coords: BTreeMap<String, Vec<[f64; 2]>>, // shape_id → [[lat, lon], ...] ordered point list
    pub trip_shapes: BTreeMap<String, String>,         // trip_id  → shape_id
    pub trip_stops: BTreeMap<String, Vec<String>>, // trip_id  → [stop_id, ...] ordered by stop_sequence
    pub shape_trips: BTreeMap<String, String>,     // shape_id → first trip_id (for shape stops)
    pub route_shapes: BTreeMap<String, Vec<String>>, // route_id → [shape_id, ...] (for the terminus map)
    /// route_id → [route_color, route_text_color] (6-digit hex, no `#`; empty if text color is absent).
    /// Only routes carrying a valid `route_color` are included. Additive field: absent from older results.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub route_colors: BTreeMap<String, [String; 2]>,
    /// service_id → [day mask "1111100" (Mon..Sun; empty if calendar.txt is absent), start YYYYMMDD,
    /// end YYYYMMDD, calendar_dates exception count]. Used to tell the planner "which days".
    /// Additive field: absent from older results.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub service_days: BTreeMap<String, [String; 4]>,
    /// route_id → route_long_name; only routes that also have a short name and a different long name
    /// (short name in `routes`). Used to say "K701 · İstasyon–Hisar Köy" in the search filter.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub route_long_names: BTreeMap<String, String>,
    /// stop_id → stop_code (the stop code seen by passengers); non-empty values only.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stop_codes: BTreeMap<String, String>,
    /// In large-feed mode, map geometry (shape_coords, etc.) is NOT serialized up front;
    /// the UI fetches it on demand from WASM when the user clicks an icon (shape_coords_of).
    pub map_data_deferred: bool,
}

/// Complete pipeline output — notices + reports + metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    #[serde(rename = "validation_status")]
    pub status: ValidationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial: Option<PartialReport>,
    pub notices: Vec<Notice>,
    pub reports: ReportSet,
    pub metrics: FeedMetrics,
    pub name_index: NameIndex,
    /// Rules that hit the cap and their actual notice counts.
    /// Contains only rules exceeding the cap; rules below it are omitted.
    pub capped_totals: BTreeMap<String, u32>,
    /// Rule × agency breakdown (MobilityData gtfs-validator #2201). Additive field: written only
    /// and ignored when reading; its shape is the JSON contract in `agency.rs`.
    #[serde(default, skip_deserializing)]
    pub agency_breakdown: crate::agency::AgencyBreakdown,
    /// Agency/publisher metadata for the report page (`agency.txt` + `feed_info.txt`). Additive
    /// field: written only and ignored when reading.
    #[serde(default, skip_deserializing)]
    pub feed_summary: crate::feed_summary::FeedSummary,
}

/// Validator's primary return type.
///
/// Contract:
/// - `Ok`    → pipeline completed or produced a PARTIAL report through safe recovery
/// - `Fatal` → pipeline stopped completely; the UI shows "feed açılamadı"
// `large_enum_variant` is deliberately accepted here. Measurement: `ValidationResult`
// is 704 bytes, `FatalError` is 56 bytes (it grew from 32 with `params` on 2026-09-18), and
// the enum is 704 bytes; the propagation difference is 648 bytes. This cost occurs once per
// `pipeline::validate_bytes`/WASM `run_full_pipeline` call, not once per notice.
// Most of the structure is heap headers such as Vec/BTreeMap; boxing adds an allocation on every
// successful return in exchange for 648 bytes of stack/layout savings.
// The CLI also uses its own flat JSON envelope; do not change the externally-tagged serde
// contract at the WASM boundary merely for lint cleanliness.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize)]
pub enum ValidateResult {
    Ok(ValidationResult),
    Fatal(FatalError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_result_layout_measurement() {
        assert!(std::mem::size_of::<ValidationResult>() >= 704);
        assert_eq!(std::mem::size_of::<FatalError>(), 56);
        assert_eq!(
            std::mem::size_of::<ValidateResult>(),
            std::mem::size_of::<ValidationResult>()
        );
    }
}
