use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// BTreeMap (HashMap değil): bu tiplerin tamamı JSON'a serialize edilir ve `HashMap`
// iterasyon sırası her SÜREÇTE farklı tohumla değişir → aynı binary + aynı feed iki
// koşuda farklı BAYT üretirdi. İçerik hep aynıydı, yalnız anahtar sırası oynuyordu;
// bu da `cmp`/`shasum` ile golden karşılaştırmayı ve "çıktı bit-özdeş" kapısını
// (K2 chunk-paralel işinin ön koşulu) imkânsız kılıyordu. BTreeMap sırayı anahtar
// üzerinden sabitler. Bkz. 2026-07-27 determinizm turu — o tur `dedup` temsilcisini
// düzeltti, bu tur serileştirme sırasını kapatıyor.
use crate::{FatalCode, FeedMetrics, Notice, ReportSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FatalError {
    pub code: FatalCode,
    pub message: String,
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

/// UI'da entity_id yerine okunabilir ad göstermek için arama tablosu.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NameIndex {
    pub stops: BTreeMap<String, String>,                  // stop_id → stop_name
    pub routes: BTreeMap<String, String>,                 // route_id → short_name (veya long_name)
    pub trips: BTreeMap<String, String>,                  // trip_id → trip_headsign
    pub trip_routes: BTreeMap<String, String>,            // trip_id → route_id (filtre için)
    pub trip_directions: BTreeMap<String, String>,        // trip_id → "0"/"1" (direction_id; yoksa yok)
    pub stop_coords: BTreeMap<String, [f64; 2]>,           // stop_id → [lat, lon] (harita için)
    pub trip_first_dep: BTreeMap<String, String>,          // trip_id → "HH:MM" (sefer saati)
    pub shape_routes: BTreeMap<String, Vec<[String; 2]>>,  // shape_id → [[route_id, yön]]
    pub shape_coords: BTreeMap<String, Vec<[f64; 2]>>,     // shape_id → [[lat, lon], ...] sıralı nokta listesi
    pub trip_shapes:  BTreeMap<String, String>,             // trip_id  → shape_id
    pub trip_stops:   BTreeMap<String, Vec<String>>,        // trip_id  → [stop_id, ...] stop_sequence sıralı
    pub shape_trips:  BTreeMap<String, String>,             // shape_id → ilk trip_id (shape durakları için)
    pub route_shapes: BTreeMap<String, Vec<String>>,        // route_id → [shape_id, ...] (terminus haritası için)
    /// Büyük feed modunda harita geometrisi (shape_coords vb.) peşinen serialize
    /// EDİLMEZ; UI ikona tıklayınca WASM'dan on-demand çeker (shape_coords_of).
    pub map_data_deferred: bool,
}

/// Tüm pipeline çıktısı — notices + raporlar + metrikler.
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
    /// Cap'e çarpan kurallar ve gerçek notice sayıları.
    /// Yalnızca cap'i aşan kuralları içerir; aşmayanlar burada yer almaz.
    pub capped_totals: BTreeMap<String, u32>,
}

/// Validator ana dönüş tipi.
///
/// Kontratlar:
/// - `Ok`    → pipeline tamamlandı veya güvenli recovery ile PARTIAL rapor üretildi
/// - `Fatal` → pipeline tamamen durdu; UI "feed açılamadı" gösterir
// `large_enum_variant` burada bilinçli olarak kabul edilir. Ölçüm: `ValidationResult`
// 704 bayt, `FatalError` 32 bayt ve enum 704 bayt; yayılım farkı 672 bayttır. Bu değer
// notice başına değil, `pipeline::validate_bytes`/WASM `run_full_pipeline` çağrısı başına
// bir kez oluşur. Yapının büyük kısmı Vec/BTreeMap gibi heap başlıklarıdır; boxing 672
// baytlık stack/layout tasarrufu karşılığında her başarılı dönüşte ek allocation getirir.
// CLI ayrıca kendi flat JSON envelope'unu kullanır; WASM sınırındaki externally-tagged
// serde sözleşmesini lint temizliği uğruna değiştirmemek gerekir.
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
        assert_eq!(std::mem::size_of::<FatalError>(), 32);
        assert_eq!(std::mem::size_of::<ValidateResult>(), std::mem::size_of::<ValidationResult>());
    }
}
