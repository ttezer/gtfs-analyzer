//! ÜRETİLMİŞ DOSYA — elle düzenlemeyin.
//!
//! Kaynak  : `spec-audit/spec_fields.json` (o da üretilmiş: `spec-audit/extract_fields.py`)
//! Üretim  : `python3 spec-audit/gen_translatable_fields.py`
//! Drift   : `python3 spec-audit/gen_translatable_fields.py --check`
//!
//! Spec'in `translations.txt::field_name` hakkındaki TEK hükmü tür eksenindedir
//! (`P7c7134fe`, soft): *"Fields with other types should not be translated."*
//! Çevrilebilir türler: Email, Phone number, Text, URL.
//!
//! ⚠️ Tabloda OLMAYAN bir alan hakkında bu dosya hüküm vermez. Uzantı alanları
//! (`jp_trip_desc_symbol` gibi) spec kataloğunda yoktur ve türleri bilinmez;
//! `spec_field_is_translatable` orada `None` döner ve çağıran SUSMALIDIR.

/// (tablo, alan, çevrilebilir tür mü) — (tablo, alan)'a göre SIRALI;
/// `binary_search_by` bunu varsayar.
pub(crate) const SPEC_TRANSLATION_FIELDS: &[(&str, &str, bool)] = &[
    ("agency", "agency_email", true),
    ("agency", "agency_fare_url", true),
    ("agency", "agency_id", false),
    ("agency", "agency_lang", false),
    ("agency", "agency_name", true),
    ("agency", "agency_phone", true),
    ("agency", "agency_timezone", false),
    ("agency", "agency_url", true),
    ("agency", "cemv_support", false),
    ("attributions", "agency_id", false),
    ("attributions", "attribution_email", true),
    ("attributions", "attribution_id", false),
    ("attributions", "attribution_phone", true),
    ("attributions", "attribution_url", true),
    ("attributions", "is_authority", false),
    ("attributions", "is_operator", false),
    ("attributions", "is_producer", false),
    ("attributions", "organization_name", true),
    ("attributions", "route_id", false),
    ("attributions", "trip_id", false),
    ("feed_info", "default_lang", false),
    ("feed_info", "feed_contact_email", true),
    ("feed_info", "feed_contact_url", true),
    ("feed_info", "feed_end_date", false),
    ("feed_info", "feed_lang", false),
    ("feed_info", "feed_publisher_name", true),
    ("feed_info", "feed_publisher_url", true),
    ("feed_info", "feed_start_date", false),
    ("feed_info", "feed_version", true),
    ("levels", "level_id", false),
    ("levels", "level_index", false),
    ("levels", "level_name", true),
    ("pathways", "from_stop_id", false),
    ("pathways", "is_bidirectional", false),
    ("pathways", "length", false),
    ("pathways", "max_slope", false),
    ("pathways", "min_width", false),
    ("pathways", "pathway_id", false),
    ("pathways", "pathway_mode", false),
    ("pathways", "reversed_signposted_as", true),
    ("pathways", "signposted_as", true),
    ("pathways", "stair_count", false),
    ("pathways", "to_stop_id", false),
    ("pathways", "traversal_time", false),
    ("routes", "agency_id", false),
    ("routes", "cemv_support", false),
    ("routes", "continuous_drop_off", false),
    ("routes", "continuous_pickup", false),
    ("routes", "network_id", false),
    ("routes", "route_color", false),
    ("routes", "route_desc", true),
    ("routes", "route_id", false),
    ("routes", "route_long_name", true),
    ("routes", "route_short_name", true),
    ("routes", "route_sort_order", false),
    ("routes", "route_text_color", false),
    ("routes", "route_type", false),
    ("routes", "route_url", true),
    ("stop_times", "arrival_time", false),
    ("stop_times", "continuous_drop_off", false),
    ("stop_times", "continuous_pickup", false),
    ("stop_times", "departure_time", false),
    ("stop_times", "drop_off_booking_rule_id", false),
    ("stop_times", "drop_off_type", false),
    ("stop_times", "end_pickup_drop_off_window", false),
    ("stop_times", "location_group_id", false),
    ("stop_times", "location_id", false),
    ("stop_times", "pickup_booking_rule_id", false),
    ("stop_times", "pickup_type", false),
    ("stop_times", "shape_dist_traveled", false),
    ("stop_times", "start_pickup_drop_off_window", false),
    ("stop_times", "stop_headsign", true),
    ("stop_times", "stop_id", false),
    ("stop_times", "stop_sequence", false),
    ("stop_times", "timepoint", false),
    ("stop_times", "trip_id", false),
    ("stops", "level_id", false),
    ("stops", "location_type", false),
    ("stops", "parent_station", false),
    ("stops", "platform_code", true),
    ("stops", "stop_access", false),
    ("stops", "stop_code", true),
    ("stops", "stop_desc", true),
    ("stops", "stop_id", false),
    ("stops", "stop_lat", false),
    ("stops", "stop_lon", false),
    ("stops", "stop_name", true),
    ("stops", "stop_timezone", false),
    ("stops", "stop_url", true),
    ("stops", "tts_stop_name", true),
    ("stops", "wheelchair_boarding", false),
    ("stops", "zone_id", false),
    ("trips", "bikes_allowed", false),
    ("trips", "block_id", false),
    ("trips", "cars_allowed", false),
    ("trips", "direction_id", false),
    ("trips", "route_id", false),
    ("trips", "safe_duration_factor", false),
    ("trips", "safe_duration_offset", false),
    ("trips", "service_id", false),
    ("trips", "shape_id", false),
    ("trips", "trip_headsign", true),
    ("trips", "trip_id", false),
    ("trips", "trip_short_name", true),
    ("trips", "wheelchair_accessible", false),
];

/// `Some(true)` çevrilebilir tür · `Some(false)` bilinen ama çevrilemez tür ·
/// `None` spec kataloğunda yok (uzantı alanı) → hüküm verilemez.
pub(crate) fn spec_field_is_translatable(table: &str, field: &str) -> Option<bool> {
    SPEC_TRANSLATION_FIELDS
        .binary_search_by(|(t, f, _)| (*t, *f).cmp(&(table, field)))
        .ok()
        .map(|i| SPEC_TRANSLATION_FIELDS[i].2)
}
