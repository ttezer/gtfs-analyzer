//! The single implementation of notice construction.
//!
//! Six layers (K1-K6) carried nearly identical copies of the same factory.
//! The same `get_rule` + panic and the same 19 fields were filled in order. The only
//! differences were which fields were fixed (K1 does not produce `scope_key`/`expected_value`;
//! K3 fixes `expected_value` to "unique"). One of the six copies had already drifted —
//! K5 omitted the ID prefix.
//!
//! Layer wrappers (`make_notice`, `k5_notice`, …) **keep their signatures**; only their
//! bodies delegate here. Adding a field to `Notice` therefore changes one place instead
//! of six, while call sites remain untouched.

use gtfs_core::{EntityType, Notice};
use gtfs_rules::get_rule;

/// `id_prefix = None` → ID is generated as `{rule}#{n}` (no callers use this anymore;
/// all six layers emit their own `kN/` prefix). The parameter is retained in case a caller
/// that emits no prefix is added.
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
    let meta = get_rule(rule_id).unwrap_or_else(|| panic!("{layer}: bilinmeyen rule_id {rule_id}"));
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
        whitespace_derived: false,
        whitespace_candidate: false,
        title: meta.title.to_string(),
        message,
        remediation: remediation.to_string(),
        blocks: meta.blocks.iter().map(|s| s.to_string()).collect(),
        base_effort: meta.base_effort,
        service_id: None,
        agency_distribution: None,
    }
}
