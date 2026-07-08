use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::{FatalCode, FeedMetrics, Notice, ReportSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FatalError {
    pub code: FatalCode,
    pub message: String,
}

/// UI'da entity_id yerine okunabilir ad göstermek için arama tablosu.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NameIndex {
    pub stops: HashMap<String, String>,                  // stop_id → stop_name
    pub routes: HashMap<String, String>,                 // route_id → short_name (veya long_name)
    pub trips: HashMap<String, String>,                  // trip_id → trip_headsign
    pub trip_routes: HashMap<String, String>,            // trip_id → route_id (filtre için)
    pub trip_directions: HashMap<String, String>,        // trip_id → "0"/"1" (direction_id; yoksa yok)
    pub stop_coords: HashMap<String, [f64; 2]>,           // stop_id → [lat, lon] (harita için)
    pub trip_first_dep: HashMap<String, String>,          // trip_id → "HH:MM" (sefer saati)
    pub shape_routes: HashMap<String, Vec<[String; 2]>>,  // shape_id → [[route_id, yön]]
    pub shape_coords: HashMap<String, Vec<[f64; 2]>>,     // shape_id → [[lat, lon], ...] sıralı nokta listesi
    pub trip_shapes:  HashMap<String, String>,             // trip_id  → shape_id
    pub trip_stops:   HashMap<String, Vec<String>>,        // trip_id  → [stop_id, ...] stop_sequence sıralı
    pub shape_trips:  HashMap<String, String>,             // shape_id → ilk trip_id (shape durakları için)
    pub route_shapes: HashMap<String, Vec<String>>,        // route_id → [shape_id, ...] (terminus haritası için)
    /// Büyük feed modunda harita geometrisi (shape_coords vb.) peşinen serialize
    /// EDİLMEZ; UI ikona tıklayınca WASM'dan on-demand çeker (shape_coords_of).
    pub map_data_deferred: bool,
}

/// Tüm pipeline çıktısı — notices + raporlar + metrikler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub notices: Vec<Notice>,
    pub reports: ReportSet,
    pub metrics: FeedMetrics,
    pub name_index: NameIndex,
    /// Cap'e çarpan kurallar ve gerçek notice sayıları.
    /// Yalnızca cap'i aşan kuralları içerir; aşmayanlar burada yer almaz.
    pub capped_totals: HashMap<String, u32>,
}

/// Validator ana dönüş tipi.
///
/// Kontratlar:
/// - `Ok`    → pipeline tamamlandı; notices boş olabilir (temiz feed)
/// - `Fatal` → pipeline tamamen durdu; UI "feed açılamadı" gösterir
#[derive(Debug, Serialize, Deserialize)]
pub enum ValidateResult {
    Ok(ValidationResult),
    Fatal(FatalError),
}
