use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::{EntityType, RuleClass, Severity};

/// Tek bir doğrulama bulgusunu temsil eden canonical veri yapısı.
///
/// Kritik kontratlar (architecture Bölüm 3):
/// - `blocks`: yalnızca canonical rule ID'leri — report-view ID'leri (GEO_008/010/011) içermez
/// - `scope_key`: kural tanımında hangi linking entity kullanılacağı belirlenir;
///   runtime bu değeri notice üretiminde ilgili entity ID'sinden atar
/// - `base_effort`: 1 / 2 / 3 / 5 — kural bazında sabit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notice {
    /// Oluşturulma sırasında atanan benzersiz kimlik — ReportItem referansı için.
    pub id: String,
    pub rule_id: String,
    pub severity: Severity,
    pub rule_class: RuleClass,
    pub entity_type: EntityType,
    pub entity_id: Option<String>,
    /// Root cause → semptom eşleşmesinde kullanılan linking entity ID.
    /// Bileşik anahtar gerektiğinde pipe-separated: "from_stop_id|to_stop_id".
    /// None ise feed-level notice; scope_key == None olan root cause tüm feed'i etkiler.
    pub scope_key: Option<String>,
    pub file: Option<String>,
    pub line: Option<u64>,
    pub field: Option<String>,
    pub observed_value: Option<String>,
    /// Eşik veya beklenen değer (ör. "120 km/h") — dinamik bağlam için.
    pub expected_value: Option<String>,
    /// Ek bağlam: ikinci entity ID, hesaplanan değer, birim vb.
    pub details: Option<HashMap<String, String>>,
    /// Kural için kısa genel Türkçe başlık — UI'da gösterilir.
    pub title: String,
    /// Kural tanımından gelen sabit metin — runtime'da üretilmez.
    pub message: String,
    /// Kural bazında sabit düzeltme talimatı.
    pub remediation: String,
    /// Bu notice düzelirse kapanacak downstream canonical rule ID'leri.
    pub blocks: Vec<String>,
    /// 1=kolay, 5=zor. fix_effort = base_effort × instance_multiplier.
    pub base_effort: u8,
    /// Bulgunun ait olduğu çalışma takvimi (service_id). Sefer-bazlı bulgularda
    /// doldurulur (R2'de "Çalışma Takvimi" sütunu). Feed/dosya kurallarında None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,
}

/// Canonical Notice'ı bir rapor görünümüne projekte eden referans nesnesi.
/// Notice klonlanmaz; `notice_id` üzerinden referans edilir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportItem {
    /// İlgili canonical Notice'ın `id` alanı.
    pub notice_id: String,
    pub report_id: crate::ReportId,
    /// Rapor bağlamına özgü görüntüleme etiketi.
    /// Ör. R4'te SHP_013 → "GEO_008".
    pub display_label: String,
    pub is_primary: bool,
}
