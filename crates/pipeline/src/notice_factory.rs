//! Notice üretiminin tek gövdesi.
//!
//! Altı katman (K1-K6) neredeyse birebir aynı fabrikayı taşıyordu: aynı `get_rule`
//! + panic, aynı 19 alanın aynı sırayla doldurulması. Farklar yalnızca hangi alanın
//! sabitlendiğiydi (K1 `scope_key`/`expected_value` üretmez, K3 `expected_value`'yu
//! "unique" sabitler). Altı kopyadan biri zaten sapmıştı — K5 id önekini atlamış.
//!
//! Katman wrapper'ları (`make_notice`, `k5_notice`, …) **imzalarını koruyor**; yalnız
//! gövdeleri buraya delege ediyor. Böylece `Notice`'a alan eklemek altı yer yerine
//! tek yer değiştirmek oluyor, çağrı siteleri ise hiç etkilenmiyor.

use gtfs_core::{EntityType, Notice};
use gtfs_rules::get_rule;

/// `id_prefix = None` → id `{rule}#{n}` biçiminde üretilir (artık kullanan yok;
/// altı katman da kendi `kN/` önekini basıyor). Parametre, önek basmayan bir çağıran
/// eklenirse diye korunuyor.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
    layer: &str,
    id_prefix: Option<&str>,
    counter: &mut u32,
    rule_id: &str,
    entity_type: EntityType,
    entity_id: Option<String>,
    scope_key: Option<String>,
    file: Option<String>,
    line: Option<u64>,
    field: Option<String>,
    observed_value: Option<String>,
    expected_value: Option<String>,
    message: String,
    remediation: &str,
) -> Notice {
    *counter += 1;
    let meta =
        get_rule(rule_id).unwrap_or_else(|| panic!("{layer}: bilinmeyen rule_id {rule_id}"));
    let id = match id_prefix {
        Some(p) => format!("{p}/{rule_id}#{counter}"),
        None => format!("{rule_id}#{counter}"),
    };
    Notice {
        id,
        rule_id: rule_id.to_string(),
        severity: meta.severity,
        rule_class: meta.rule_class,
        entity_type,
        entity_id,
        scope_key,
        file,
        line,
        field,
        observed_value,
        expected_value,
        details: None,
        title: meta.title.to_string(),
        message,
        remediation: remediation.to_string(),
        blocks: meta.blocks.iter().map(|s| s.to_string()).collect(),
        base_effort: meta.base_effort,
        service_id: None,
    }
}
