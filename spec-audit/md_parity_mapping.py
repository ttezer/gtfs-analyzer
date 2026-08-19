"""Context-aware MobilityData -> GTFS Analyzer parity mappings.

MobilityData reports aggregate a notice code across files and fields.  A flat
``code -> rule ids`` map is therefore safe only for codes whose meaning is
already specific.  Generic codes are resolved from the sample notice context
before they are compared with Analyzer's rule counts.

The module intentionally contains no filesystem or network access.  It is
imported by :mod:`md_parity_audit` and by the CI drift check.
"""

import csv
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Any, Iterable


@dataclass(frozen=True)
class ContextMapping:
    """One candidate mapping for a specific MD notice context."""

    md_code: str
    analyzer_rules: tuple[str, ...]
    filename: tuple[str, ...] = ()
    fields: tuple[str, ...] = ()
    entities: tuple[str, ...] = ()
    value_range: tuple[int, int] | None = None
    label: str = ""

    @property
    def specificity(self) -> int:
        return sum(bool(x) for x in (self.filename, self.fields, self.entities, self.value_range))

    def matches(self, sample: dict[str, Any]) -> bool:
        filename = _filename(sample)
        field = _field(sample)
        entity = _entity(sample, filename)

        if self.filename and filename not in self.filename:
            return False
        if self.fields and field not in self.fields:
            return False
        if self.entities and entity not in self.entities:
            return False
        if self.value_range is not None:
            value = _integer_value(sample)
            if value is None or not (self.value_range[0] <= value <= self.value_range[1]):
                return False
        return True


@dataclass(frozen=True)
class MappingResult:
    """Resolved rule ids plus enough provenance for an auditable report."""

    analyzer_rules: tuple[str, ...]
    kind: str
    contexts: tuple[str, ...] = ()
    unresolved_samples: int = 0
    context_complete: bool = True

    @property
    def is_contextual(self) -> bool:
        return self.kind in {"context-dependent", "context-mixed"}


def _normalise(value: Any) -> str:
    return str(value or "").strip().lower()


# 🔴 MobilityData her notice'ta bağlamı AYNI anahtarlarla taşımaz, ve bu sessizce
# ısırır: eşleşmeyen bir anahtar "bağlam yok" gibi görünür, `_ctx` girişi hiç
# tetiklenmez ve kod sonsuza kadar eşlemesiz sayılır.
#
# İki büyük jenerik kod tam bu yüzden hiç bağlamsal eşleme almamıştı (#165):
#   `foreign_key_violation` → childFilename / childFieldName (+ parent* karşılığı)
#   `duplicate_key`         → fieldName1 (anahtar sütun LİSTESİ, virgülle)
# İkisi birlikte korpusta 9,3M notice taşıyor. `missing_required_field` (18 giriş)
# ve `unexpected_enum_value` (11 giriş) standart anahtarları kullandığı için
# eşlenebilmiş; bu ikisi görünmez kalmıştı.
def _filename(sample: dict[str, Any]) -> str:
    value = sample.get("filename", sample.get("fileName", sample.get("file")))
    if not value:
        # FK ihlalinde ölçtüğümüz kural REFERANSI VEREN dosyada yaşar (child),
        # hedeflenen dosyada (parent) değil: `trips.txt::shape_id` bulunamadı
        # bulgusu TRP_004'ündür, SHP_*'ın değil.
        value = sample.get("childFilename", sample.get("child_filename"))
    return _normalise(value).rsplit("/", 1)[-1]


def _field(sample: dict[str, Any]) -> str:
    value = sample.get("fieldName", sample.get("field_name", sample.get("field")))
    if not value:
        value = sample.get("childFieldName", sample.get("child_field_name"))
    if not value:
        # `duplicate_key` birincil anahtarın TÜM sütunlarını tek dizede verir
        # ("table_name,field_name,language,field_value"). Tek alan adı bekleyen
        # eşleştirici bunu göremez; ilk sütun anahtarın sahibi olarak yeterlidir.
        raw = sample.get("fieldName1", sample.get("field_name1"))
        if raw:
            value = str(raw).split(",", 1)[0]
    return _normalise(value)


def _entity(sample: dict[str, Any], filename: str) -> str:
    value = sample.get("entityType", sample.get("entity_type", sample.get("entity")))
    if value:
        return _normalise(value).replace("_", " ")
    stem = filename.removesuffix(".txt")
    return {
        "routes": "route",
        "trips": "trip",
        "transfers": "transfer",
        "fare_attributes": "fare",
        "rider_categories": "rider category",
        "pathways": "pathway",
        "shapes": "shape",
        "stops": "stop",
    }.get(stem, stem)


def _integer_value(sample: dict[str, Any]) -> int | None:
    value = sample.get("fieldValue", sample.get("field_value", sample.get("value")))
    try:
        # MD values are normally strings.  Do not accept floats such as 1.5 as
        # route_type values just because int(1.5) would otherwise be possible.
        text = str(value).strip()
        if not text or any(c in text for c in ".eE"):
            return None
        return int(text)
    except (TypeError, ValueError):
        return None


def _sample_notices(notice: dict[str, Any] | None) -> list[dict[str, Any]]:
    if not isinstance(notice, dict):
        return []
    samples = notice.get("sampleNotices", notice.get("sample_notices", []))
    return [sample for sample in samples if isinstance(sample, dict)]


def _total_notices(notice: dict[str, Any] | None) -> int | None:
    if not isinstance(notice, dict):
        return None
    value = notice.get("totalNotices", notice.get("total_notices"))
    try:
        total = int(value)
    except (TypeError, ValueError):
        return None
    return total if total >= 0 else None


def _ctx(
    code: str,
    *rules: str,
    filename: Iterable[str] = (),
    fields: Iterable[str] = (),
    entities: Iterable[str] = (),
    value_range: tuple[int, int] | None = None,
    label: str = "",
) -> ContextMapping:
    return ContextMapping(
        md_code=code,
        analyzer_rules=tuple(rules),
        filename=tuple(_normalise(x) for x in filename),
        fields=tuple(_normalise(x) for x in fields),
        entities=tuple(_normalise(x).replace("_", " ") for x in entities),
        value_range=value_range,
        label=label,
    )


# Generic MD notices.  Most specific entries must precede their file/field
# fallbacks; resolve_mapping() also chooses the highest specificity match.
CONTEXT_MAPPINGS: tuple[ContextMapping, ...] = (
    # unexpected_enum_value
    _ctx("unexpected_enum_value", "RTS_030", filename=("routes.txt",), fields=("route_type",), value_range=(100, 1799), label="routes.txt::route_type extended"),
    _ctx("unexpected_enum_value", "RTS_004", filename=("routes.txt",), fields=("route_type",), label="routes.txt::route_type core-invalid"),
    _ctx("unexpected_enum_value", "RTS_013", filename=("routes.txt",), fields=("continuous_pickup",), label="routes.txt::continuous_pickup"),
    _ctx("unexpected_enum_value", "RTS_018", filename=("routes.txt",), fields=("continuous_drop_off",), label="routes.txt::continuous_drop_off"),
    _ctx("unexpected_enum_value", "TRP_005", filename=("trips.txt",), fields=("direction_id",), label="trips.txt::direction_id"),
    _ctx("unexpected_enum_value", "TRP_006", filename=("trips.txt",), fields=("wheelchair_accessible",), label="trips.txt::wheelchair_accessible"),
    _ctx("unexpected_enum_value", "TRP_007", filename=("trips.txt",), fields=("bikes_allowed",), label="trips.txt::bikes_allowed"),
    _ctx("unexpected_enum_value", "TRP_032", filename=("trips.txt",), fields=("cars_allowed",), label="trips.txt::cars_allowed"),
    _ctx("unexpected_enum_value", "TRF_004", filename=("transfers.txt",), fields=("transfer_type",), label="transfers.txt::transfer_type"),
    _ctx("unexpected_enum_value", "FAR_005", filename=("fare_attributes.txt",), fields=("transfers",), label="fare_attributes.txt::transfers"),
    _ctx("unexpected_enum_value", "RCT_003", filename=("rider_categories.txt",), fields=("is_default_fare_category",), label="rider_categories.txt::is_default_fare_category"),
    # number_out_of_range
    # point_near_origin — Null Island. MD tek kod basar; bizde dosyaya göre iki ayrı
    # kural sahiplenir. Düz eşleme GEO_017'yi görünmez kılıyordu: tdg-80960'ta MD 5
    # bulgu shapes.txt kaynaklıydı, bizde GEO_017 tam 5 ateşliyordu ama benchmark
    # GEO_016=0 diye MISS sayıyordu.
    _ctx("point_near_origin", "GEO_016", filename=("stops.txt",), label="stops.txt Null Island"),
    _ctx("point_near_origin", "GEO_017", filename=("shapes.txt",), label="shapes.txt Null Island"),
    # #164: `context_unresolved`'ın 16 satırının 14'ü. MobilityData bu jenerik kodları
    # `filename` + `fieldName` İLE BİRLİKTE basıyor — bağlam raporda VARDI, bizde o
    # kombinasyon için giriş yoktu. Aşağıdaki dokuz kuralın hepsi, MD'nin raporladığı
    # feed'lerin tam olarak hepsinde ateşliyor; eşleme eksikti, kural değil.
    # ── foreign_key_violation ────────────────────────────────────────────────
    # MobilityData tek kod basar ve bağlamı `childFilename`/`childFieldName` ile
    # taşır; biz referansı VEREN dosyaya göre ayrı kural tutarız. Aşağıdaki
    # bağlamların çoğu korpusta GÖZLENDİ (run-32197267205); kardeş yönler
    # (`to_*` karşılıkları) yapısal kesinlik gereği eklendi ve yorumla işaretlendi.
    _ctx("foreign_key_violation", "TRP_004", filename=("trips.txt",), fields=("shape_id",), label="trips.txt::shape_id"),
    _ctx("foreign_key_violation", "TRP_003", filename=("trips.txt",), fields=("service_id",), label="trips.txt::service_id"),
    _ctx("foreign_key_violation", "TRP_002", filename=("trips.txt",), fields=("route_id",), label="trips.txt::route_id"),
    _ctx("foreign_key_violation", "STM_001", filename=("stop_times.txt",), fields=("trip_id",), label="stop_times.txt::trip_id"),
    _ctx("foreign_key_violation", "STM_002", filename=("stop_times.txt",), fields=("stop_id",), label="stop_times.txt::stop_id"),
    _ctx("foreign_key_violation", "BKR_017", "BKR_018", filename=("stop_times.txt",), fields=("pickup_booking_rule_id", "drop_off_booking_rule_id"), label="stop_times.txt::booking_rule_id"),
    _ctx("foreign_key_violation", "FRL_002", filename=("fare_rules.txt",), fields=("route_id",), label="fare_rules.txt::route_id"),
    _ctx("foreign_key_violation", "FRL_003", filename=("fare_rules.txt",), fields=("origin_id",), label="fare_rules.txt::origin_id"),
    _ctx("foreign_key_violation", "FRL_004", filename=("fare_rules.txt",), fields=("destination_id",), label="fare_rules.txt::destination_id"),
    _ctx("foreign_key_violation", "FRL_005", filename=("fare_rules.txt",), fields=("contains_id",), label="fare_rules.txt::contains_id"),
    _ctx("foreign_key_violation", "FRL_001", filename=("fare_rules.txt",), fields=("fare_id",), label="fare_rules.txt::fare_id"),
    _ctx("foreign_key_violation", "FLG_001", filename=("fare_leg_rules.txt",), fields=("fare_product_id",), label="fare_leg_rules.txt::fare_product_id"),
    _ctx("foreign_key_violation", "FLG_002", filename=("fare_leg_rules.txt",), fields=("network_id",), label="fare_leg_rules.txt::network_id"),
    _ctx("foreign_key_violation", "FLG_003", "FLG_004", filename=("fare_leg_rules.txt",), fields=("from_area_id", "to_area_id"), label="fare_leg_rules.txt::area_id"),
    _ctx("foreign_key_violation", "FTR_004", filename=("fare_transfer_rules.txt",), fields=("fare_product_id",), label="fare_transfer_rules.txt::fare_product_id"),
    _ctx("foreign_key_violation", "FPD_005", filename=("fare_products.txt",), fields=("rider_category_id",), label="fare_products.txt::rider_category_id"),
    _ctx("foreign_key_violation", "FAR_008", filename=("fare_attributes.txt",), fields=("agency_id",), label="fare_attributes.txt::agency_id"),
    _ctx("foreign_key_violation", "PTH_002", "PTH_003", filename=("pathways.txt",), fields=("from_stop_id", "to_stop_id"), label="pathways.txt::stop_id"),
    _ctx("foreign_key_violation", "TRF_003", filename=("transfers.txt",), fields=("from_stop_id", "to_stop_id"), label="transfers.txt::stop_id"),
    _ctx("foreign_key_violation", "TRF_008", "TRF_009", filename=("transfers.txt",), fields=("from_route_id", "to_route_id"), label="transfers.txt::route_id"),
    _ctx("foreign_key_violation", "FRQ_001", filename=("frequencies.txt",), fields=("trip_id",), label="frequencies.txt::trip_id"),
    _ctx("foreign_key_violation", "RTS_002", filename=("routes.txt",), fields=("agency_id",), label="routes.txt::agency_id"),
    _ctx("foreign_key_violation", "ATR_011", filename=("attributions.txt",), fields=("route_id",), label="attributions.txt::route_id"),
    _ctx("foreign_key_violation", "STP_009", filename=("stops.txt",), fields=("parent_station",), label="stops.txt::parent_station"),
    _ctx("foreign_key_violation", "SAR_001", "SAR_002", filename=("stop_areas.txt",), fields=("area_id", "stop_id"), label="stop_areas.txt"),

    # ── duplicate_key ────────────────────────────────────────────────────────
    # Bağlam `fieldName1`'de gelir ve anahtarın TÜM sütunlarını virgülle taşır;
    # ilk sütun sahibi belirlemeye yeter. Bileşik anahtarlı Fares v2 / Flex
    # dosyalarını `DQ_021` sahiplenir, tekil anahtarlı çekirdek tabloları ise
    # kendi kuralları — ikisi aynı dosyada örtüştüğünde İKİSİ de listelenir.
    _ctx("duplicate_key", "STP_001", "DQ_021", filename=("stops.txt",), fields=("stop_id",), label="stops.txt::stop_id"),
    _ctx("duplicate_key", "RTS_001", "DQ_021", filename=("routes.txt",), fields=("route_id",), label="routes.txt::route_id"),
    _ctx("duplicate_key", "TRP_001", "DQ_021", filename=("trips.txt",), fields=("trip_id",), label="trips.txt::trip_id"),
    _ctx("duplicate_key", "STM_032", filename=("stop_times.txt",), fields=("trip_id",), label="stop_times.txt::(trip_id, stop_sequence)"),
    _ctx("duplicate_key", "CAL_001", filename=("calendar.txt",), fields=("service_id",), label="calendar.txt::service_id"),
    _ctx("duplicate_key", "AGN_010", filename=("agency.txt",), fields=("agency_id",), label="agency.txt::agency_id"),
    _ctx("duplicate_key", "FAR_001", filename=("fare_attributes.txt",), fields=("fare_id",), label="fare_attributes.txt::fare_id"),
    _ctx("duplicate_key", "TRN_005", filename=("translations.txt",), fields=("table_name",), label="translations.txt::(table_name, ...)"),
    _ctx("duplicate_key", "TRF_012", filename=("transfers.txt",), fields=("from_stop_id",), label="transfers.txt::(from_stop_id, to_stop_id)"),
    _ctx("duplicate_key", "DQ_021", filename=("fare_leg_rules.txt", "fare_transfer_rules.txt", "location_groups.txt"), label="Fares v2 / Flex bileşik anahtarları"),
    _ctx("invalid_time", "BKR_025", filename=("booking_rules.txt",), fields=("prior_notice_start_time", "prior_notice_last_time"), label="booking_rules.txt::prior_notice zamanları"),
    _ctx("invalid_float", "SHP_002", filename=("shapes.txt",), fields=("shape_pt_lat",), label="shapes.txt::shape_pt_lat"),
    _ctx("invalid_float", "SHP_003", filename=("shapes.txt",), fields=("shape_pt_lon",), label="shapes.txt::shape_pt_lon"),
    _ctx("invalid_integer", "CAL_002", filename=("calendar.txt",), fields=("monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"), label="calendar.txt::gün alanları"),
    _ctx("invalid_integer", "FAR_005", filename=("fare_attributes.txt",), fields=("transfers",), label="fare_attributes.txt::transfers"),
    _ctx("invalid_integer", "FAR_004", filename=("fare_attributes.txt",), fields=("payment_method",), label="fare_attributes.txt::payment_method"),
    _ctx("invalid_integer", "RTS_004", filename=("routes.txt",), fields=("route_type",), label="routes.txt::route_type"),
    _ctx("invalid_integer", "STM_022", filename=("stop_times.txt",), fields=("timepoint",), label="stop_times.txt::timepoint"),
    _ctx("number_out_of_range", "STM_030", filename=("stop_times.txt",), fields=("shape_dist_traveled",), label="stop_times.txt::shape_dist_traveled negatif"),
    _ctx("missing_required_field", "CAL_025", filename=("calendar.txt",), fields=("monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"), label="calendar.txt::gün alanı boş"),
    _ctx("number_out_of_range", "PTH_007", filename=("pathways.txt",), fields=("traversal_time",), label="pathways.txt::traversal_time"),
    _ctx("number_out_of_range", "SHP_002", filename=("shapes.txt",), fields=("shape_pt_lat",), label="shapes.txt::shape_pt_lat"),
    _ctx("number_out_of_range", "SHP_003", filename=("shapes.txt",), fields=("shape_pt_lon",), label="shapes.txt::shape_pt_lon"),
    _ctx("number_out_of_range", "STP_003", filename=("stops.txt",), fields=("stop_lat",), label="stops.txt::stop_lat"),
    _ctx("number_out_of_range", "STP_005", filename=("stops.txt",), fields=("stop_lon",), label="stops.txt::stop_lon"),
    _ctx("number_out_of_range", "RCT_004", filename=("rider_categories.txt",), fields=("min_age", "max_age"), label="rider_categories.txt::min_age/max_age"),
    _ctx("number_out_of_range", "FRQ_008", filename=("frequencies.txt",), fields=("headway_secs",), value_range=(0, 0), label="frequencies.txt::headway_secs zero"),
    # Generic notices are resolved by filename/field. A partial sample stays
    # CONTEXT, so an unseen field cannot be presented as exact parity.
    # invalid_time / invalid_float — alan bağlamıyla çözülür (#155). Düz eşleme yanlış
    # olurdu: MD tek kodu iki ayrı alan için basıyor ve bizde iki ayrı kural sahipleniyor.
    _ctx("invalid_time", "STM_003", filename=("stop_times.txt",), fields=("arrival_time",), label="stop_times.txt::arrival_time"),
    _ctx("invalid_time", "STM_004", filename=("stop_times.txt",), fields=("departure_time",), label="stop_times.txt::departure_time"),
    _ctx("invalid_float", "FAR_002", filename=("fare_attributes.txt",), fields=("price",), label="fare_attributes.txt::price"),
    _ctx("invalid_integer", "TRP_005", filename=("trips.txt",), fields=("direction_id",), label="trips.txt::direction_id"),
    # missing_required_file
    #
    # MobilityData reports one code for every file it considers required, including the
    # conditionally required ones. `ARC_004` only covers the five unconditionally required
    # files, so a flat `missing_required_file -> ARC_004` row makes the conditional case
    # look like blindness. The full-catalog audit hit exactly that on `mdb-2933`: MD named
    # `feed_info.txt`, and `ARC_031` had already fired (#145).
    _ctx("missing_required_file", "ARC_031", filename=("feed_info.txt",), label="feed_info.txt::required when translations.txt is present"),
    _ctx(
        "missing_required_file", "ARC_004",
        filename=("agency.txt", "stops.txt", "routes.txt", "trips.txt", "stop_times.txt"),
        label="the five unconditionally required files",
    ),
    # invalid_url — same generic shape. The map covered three of the six URL fields
    # MobilityData actually reports, so stop_url alone accounted for 10 feeds of apparent
    # blindness. Each pair below was verified against the run (#146).
    _ctx("invalid_url", "AGN_003", filename=("agency.txt",), fields=("agency_url",), label="agency.txt::agency_url"),
    _ctx("invalid_url", "AGN_008", filename=("agency.txt",), fields=("agency_fare_url",), label="agency.txt::agency_fare_url"),
    _ctx("invalid_url", "RTS_005", filename=("routes.txt",), fields=("route_url",), label="routes.txt::route_url"),
    _ctx("invalid_url", "STP_042", filename=("stops.txt",), fields=("stop_url",), label="stops.txt::stop_url"),
    _ctx("invalid_url", "BKR_021", filename=("booking_rules.txt",), fields=("info_url",), label="booking_rules.txt::info_url"),
    _ctx("invalid_url", "BKR_020", filename=("booking_rules.txt",), fields=("booking_url",), label="booking_rules.txt::booking_url"),
    _ctx("invalid_url", "FIN_009", filename=("feed_info.txt",), fields=("feed_contact_url",), label="feed_info.txt::feed_contact_url"),
    _ctx("invalid_url", "FIN_002", filename=("feed_info.txt",), fields=("feed_publisher_url",), label="feed_info.txt::feed_publisher_url"),
    # missing_required_field
    #
    # Same shape as missing_required_file: MobilityData emits one code for every required
    # field in every file, so a two-rule map made 35 feeds look like blindness. Each pair
    # below was verified against the full-catalog run -- the named rule actually fires on
    # the feeds MobilityData reports for that field (#146).
    _ctx("missing_required_field", "AGN_002", filename=("agency.txt",), fields=("agency_name",), label="agency.txt::agency_name"),
    _ctx("missing_required_field", "AGN_003", filename=("agency.txt",), fields=("agency_url",), label="agency.txt::agency_url"),
    _ctx("missing_required_field", "AGN_004", filename=("agency.txt",), fields=("agency_timezone",), label="agency.txt::agency_timezone"),
    _ctx("missing_required_field", "RTS_004", filename=("routes.txt",), fields=("route_type",), label="routes.txt::route_type"),
    _ctx("missing_required_field", "TRP_035", filename=("trips.txt",), fields=("service_id",), label="trips.txt::service_id"),
    _ctx("missing_required_field", "TRP_031", filename=("trips.txt",), fields=("route_id",), label="trips.txt::route_id"),
    _ctx("missing_required_field", "STM_006", filename=("stop_times.txt",), fields=("stop_id",), label="stop_times.txt::stop_id"),
    _ctx("missing_required_field", "SHP_001", filename=("shapes.txt",), fields=("shape_id",), label="shapes.txt::shape_id"),
    _ctx("missing_required_field", "FIN_001", filename=("feed_info.txt",), fields=("feed_publisher_name",), label="feed_info.txt::feed_publisher_name"),
    _ctx("missing_required_field", "FIN_002", filename=("feed_info.txt",), fields=("feed_publisher_url",), label="feed_info.txt::feed_publisher_url"),
    _ctx("missing_required_field", "FIN_003", filename=("feed_info.txt",), fields=("feed_lang",), label="feed_info.txt::feed_lang"),
    _ctx("missing_required_field", "TRN_003", filename=("translations.txt",), fields=("language",), label="translations.txt::language"),
    _ctx("missing_required_field", "BKR_019", filename=("booking_rules.txt",), fields=("booking_rule_id",), label="booking_rules.txt::booking_rule_id"),
    _ctx("missing_required_field", "BKR_016", filename=("booking_rules.txt",), fields=("booking_type",), label="booking_rules.txt::booking_type"),
    # transfers.txt::min_transfer_time — ÖNLEYİCİ eşleme. google/transit#640 bu alanı
    # `transfer_type=2` için açıkça Conditionally Required yaptı; MobilityData hükmü
    # uyguladığında `missing_required_field` bu bağlamla gelecek. Eşleme şimdi yazılmazsa
    # o gün doğrudan `md_mapped_missing`'e düşer — bu oturumda temizlediğimiz sahte
    # körlüğün aynısı. v8.0.1 henüz #640'ı içermediği için bugün sessiz kalır.
    _ctx("missing_required_field", "TRF_005", filename=("transfers.txt",), fields=("min_transfer_time",), label="transfers.txt::min_transfer_time"),
    # #153 ile İKİSİ ARTIK EŞLENDİ — kural yazıldığı için eşleme yazılabilir hale geldi.
    # Sıra önemli: boş değerin sahibi ayrı kuraldır, FK/yineleme kuralı değil.
    _ctx("missing_required_field", "RTS_031", filename=("routes.txt",), fields=("route_id",), label="routes.txt::route_id"),
    _ctx("missing_required_field", "FLG_008", filename=("fare_leg_rules.txt",), fields=("fare_product_id",), label="fare_leg_rules.txt::fare_product_id"),
    # ⚠️ BİR ÇİFT BİLEREK EŞLENMEDİ — aday kural korpusta o feed'lerde ATEŞLEMİYOR, yani
    # eşleme yazmak boşluğu gizlemek olurdu (`classify_unmapped` felsefesi, alan düzeyinde):
    #   calendar.txt::sunday (1 feed) — ÖLÇÜLDÜ (#153): boşluk YOKTU. `CAL_025` ("takvim gün
    #   alanı boş") zaten var ve tld-6756'da tam bir kez ateşliyor; `CAL_002` geçersiz DEĞERİ,
    #   `CAL_025` eksik değeri sahiplenir. Bağlam eşlemesi CAL_025'e yazılmadı çünkü MD'nin
    #   bu bağlamdaki tek vakası zaten MATCH; yazmak sayıyı değiştirmez, kararı gizler.
    _ctx("invalid_date", "FIN_005", filename=("feed_info.txt",), fields=("feed_start_date",), label="feed_info.txt::feed_start_date"),
    _ctx("invalid_date", "FIN_006", filename=("feed_info.txt",), fields=("feed_end_date",), label="feed_info.txt::feed_end_date"),
    _ctx("invalid_date", "CAL_003", filename=("calendar.txt",), fields=("start_date",), label="calendar.txt::start_date"),
    _ctx("invalid_date", "CAL_004", filename=("calendar.txt",), fields=("end_date",), label="calendar.txt::end_date"),
    _ctx("invalid_date", "CLD_002", filename=("calendar_dates.txt",), fields=("date",), label="calendar_dates.txt::date"),
    _ctx("start_and_end_range_out_of_order", "CAL_005", filename=("calendar.txt",), label="calendar.txt::service range"),
    _ctx("start_and_end_range_out_of_order", "FIN_012", filename=("feed_info.txt",), label="feed_info.txt::feed range"),
    _ctx("start_and_end_range_out_of_order", "STM_007", filename=("stop_times.txt",), label="stop_times.txt::time range"),
    # missing_recommended_field
    _ctx("missing_recommended_field", "RTS_025", filename=("routes.txt",), fields=("agency_id",), label="routes.txt::agency_id"),
    _ctx("missing_recommended_field", "FIN_013", filename=("fare_attributes.txt",), fields=("agency_id",), label="fare_attributes.txt::agency_id"),
    _ctx("missing_recommended_field", "FIN_007", filename=("feed_info.txt",), fields=("feed_version",), label="feed_info.txt::feed_version"),
    _ctx("missing_recommended_field", "FIN_014", filename=("feed_info.txt",), fields=("feed_start_date", "feed_end_date"), label="feed_info.txt::feed validity dates"),
)


CONTEXT_BY_CODE: dict[str, tuple[ContextMapping, ...]] = {}
for _mapping in CONTEXT_MAPPINGS:
    CONTEXT_BY_CODE.setdefault(_mapping.md_code, []).append(_mapping)
CONTEXT_BY_CODE = {code: tuple(entries) for code, entries in CONTEXT_BY_CODE.items()}


# These decisions are deliberately separate from the mapping.  A code can be
# known and reviewed as a real coverage gap without being assigned an Analyzer
# rule (and therefore without being silently dropped from parity_md_only.csv).
UNMAPPED_DECISIONS = {
    "csv_parsing_failed": (
        "md-implementation-limit",
        "Not a GTFS violation and not our gap. MobilityData's own message on mdb-982 reads 'Length of parsed "
        "input (4097) exceeds' - its CSV reader has a 4096-character field limit. We parse the same feed "
        "without difficulty (completed, 3,406 findings). Nothing in the specification caps field length, so "
        "there is no rule to write; the divergence measures their parser, not the data.",
    ),
    # 🔴 2026-08-19: bu iki kod "genuine-gap" olarak KAYITLIYDI ve KARAR YANLIŞTI.
    # Defter "would need its own rule" diyordu; kurallar zaten vardı ve ateşliyordu.
    # Aranan yer yanlıştı: tutarlılık kontrolleri TRF_ önekinde değil, biri XFL_
    # grubunda. TRF_006/003'ün VARLIK kontrolü olduğu tespiti doğruydu; eksik olan
    # şey "başka bir kural bunu yapıyor mu" sorusuydu. Aday listesini kural ADINA
    # göre değil, kuralların NE ÖLÇTÜĞÜNE göre kur.
    "fast_travel_between_far_stops": (
        "genuine-gap",
        "Analyzer has no rule for non-consecutive far-stop pairs; do not alias this to STM_012.",
    ),
    "feed_expiration_date30_days": (
        "config-dependent",
        "The MD notice is a feed_info-level 30-day horizon. FIN_019 uses a 7-day default, but "
        "feed_info_expiry_warning_days=30 makes it semantically equivalent for the same horizon; "
        "do not claim exact parity unless the audit run uses that 30-day configuration. CAL_008 "
        "remains a separate per-calendar-service horizon.",
    ),
    "feed_valid_beyond_total_service_window": (
        "deprecated-md-only",
        "MobilityData marks this notice deprecated. Its containment direction is not an exact "
        "counterpart of CAL_014/CAL_019, which check service windows against feed dates.",
    ),
    "start_and_end_range_equal": (
        "deprecated-md-only",
        "MobilityData marks this generic range notice deprecated; the Analyzer intentionally "
        "does not alias the equal-range frequencies variant to FRQ_005.",
    ),
    "unused_trip": (
        "intentional-difference",
        "MobilityData means a trip is unreferenced by stop_times. TRP_017 is the separate "
        "frequency-trip-without-stop-times check, so it is not an exact alias.",
    ),
    "missing_recommended_field": (
        "context-dependent",
        "This is a generic MD code. The known routes/fare/feed-version/feed-validity contexts "
        "are mapped above; agency.txt agency_id is optional for a single standard GTFS agency "
        "and therefore has no safe one-rule alias.",
    ),
}


# ── BİLİNÇLİ SAPMALAR: adjudicate EDİLMİŞ, tekrar yargılanmayacak ────────────
# Bu MD kodlarında sapma BEKLENEN davranıştır; status "EXPLAINED" olur ve
# parity_unexplained.csv'ye DÜŞMEZ. Amaç: her koşumda aynı 90+ satırı elle
# ayıklamak zorunda kalmamak → çıktı yalnız AÇIKLANAMAYAN delta olsun.
#
# ⚠️ KURAL: buraya kayıt eklemek "görmezden gel" demek DEĞİL, "yargılandı ve
# gerekçesi şu" demektir. GEREKÇESİZ KAYIT EKLEME — gerekçesiz bir liste,
# sonradan kimsenin sorgulayamayacağı bir kör noktaya dönüşür. Yeni kod önce
# MD-ONLY / MISS'te görünsün, adjudicate edilsin, SONRA buraya insin.
# Kaynak: 2×20-feed kampanyası (memory: project_20feed_campaign) + MD_PARITY_GUIDE.
BY_DESIGN = {
    "unknown_file":
        "BİZ-DOĞRU (GTFS-JP farkındalığı): MD `agency_jp.txt`/`office_jp.txt` dosyalarını "
        "tanımıyor ve 'bilinmeyen dosya' işaretliyor; biz GTFS-JP profilini destekliyoruz "
        "→ 0'ımız DOĞRU. 250-feed: tek vaka mdb-3175 (Tokyo Toei, MD=2). MD ile eşitlemek "
        "gerçek JP feed'inde regresyon olurdu (backlog: jp_ kararı).",
    "route_long_name_contains_short_name":
        "İKİ BİLİNÇLİ KOL, ikisi de kayıtlı. (1) EŞİK: `RTS_022` eşit-olmayan içerme için kısa adın "
        "EN AZ 2 KARAKTER olmasını ister — `short='5', long='Route 5A'` yanlış pozitifini önlemek için "
        "(karar RTS_022 kartında yazılı). Tam katalog koşumunda bu koda düşen 13 feed'in 24 örneğinin "
        "22'si TEK KARAKTERLİ kısa addır. (2) YAPISAL: kalan 2 örnek `mdb-2711`'de tam eşitliktir "
        "(`short=long='LINEA 1'`) ve normalde ateşlerdi; o feed'de `trips.txt`/`stop_times.txt` YOK, "
        "sonuç PARTIAL ve ARC_004 — K6 hat-adı analitiği hiç koşmuyor. Yapısal hata sahiplenir (#146).",
    "equal_shape_distance_diff_coordinates_distance_below_threshold":
        "ATIF SINIRI, körlük değil. Üç kardeş kural aynı olguyu hassasiyete göre böler: `SHP_023` "
        "koordinatlar AYNI · `SHP_029` eşik-altı farklı (BİLGİ, weight 0) · `SHP_028` eşik-üstü (hata). "
        "MD ise tek eşik-altı kodu basar. Koşumda bu koda düşen 11 feed'in 11'i de `SHP_023` alıyor — "
        "yani noktalar bizim okumamızda ÖZDEŞ, MD'nin hesabında alt-metre farklı. Vaka görülüyor ve "
        "raporlanıyor, yalnız kardeş kurala atfediliyor. SHP_029 kartı MD paritesinin TESPİT düzeyinde "
        "olduğunu zaten söylüyor (#146).",
    "single_shape_point":
        "BİZ-DOĞRU (atıf kararı, çift sayımı önler): `SHP_006` yalnız KULLANILAN shape'lere bakar. "
        "Kullanılmayan tek-noktalı kayıt zaten `SHP_018` (öksüz shape) kapsamındadır ve aynı kök "
        "nedeni iki notice'a bölmek puanı şişirirdi — gerekçe `k5_derived.rs`'te yazılı. "
        "Tam katalog koşumunda bu koda düşen 11 feed'in 11'i de SHP_018 alıyor, hiçbiri SHP_006 "
        "almıyor: yani vaka görülüyor, başka kurala atfediliyor. MD referans durumuna bakmaz (#146).",
    "unknown_column":
        "BİZ-DOĞRU (GTFS-JP farkındalığı) — `unknown_file` kararının SÜTUN İKİZİ, aynı gerekçe. "
        "MD `jp_` önekli sütunları tanımıyor ve 'bilinmeyen sütun' işaretliyor; `ARC_017` bu "
        "öneki BİLEREK atlar (`k1_parse.rs`, 'jp_ prefix = GTFS-JP uzantısı, atla'). Tam katalog "
        "koşumunda bu koda düşen 327 feed'in 327'si SALT `jp_` sütunudur — istisna YOK "
        "(`jp_trip_desc` 332 · `jp_pattern_id` 294 · `jp_parent_route_id` 41 · `jp_office_id` 7). "
        "MD ile eşitlemek her GTFS-JP feed'ini gürültüye boğardı → 0'ımız DOĞRU. "
        "⚠️ Bu karar YALNIZ sıfır-saydığımız hâli kapsar: `comment`, `trans_id`, `shape_dist_traveleded` "
        "gibi gerçek bilinmeyen sütunlar zaten ARC_017 üretiyor ve md_mapped_under/over'da görünür; "
        "onlar granülerlik sorusudur, kapsam değil (#146).",
    "non_ascii_or_non_printable_char":
        "MD Japonca/Kiril gibi ASCII-dışı DEĞERLERİ işaretler (ör. service_id='平日'); "
        "ARC_021 yalnız non-printable arar → bizim 0'ımız DOĞRU, MD over-fire ediyor.",
    "platform_without_parent_station":
        "MD parent'sız HER durağı işaretler (tüm feed gürültü); STP_032 kapsamlandırılmış.",
    "mixed_case_recommended_field":
        "DQ_018 yalnız ALL-CAPS işaretler; MD mixed_case'i küçük-harfsiz dillerde (JP) over-fire eder.",
    "missing_recommended_field":
        "Bizde tek MD koduna karşılık BİRDEN ÇOK kural var (DQ_003/004, RTS_025, FIN_013/018) "
        "→ üst-küme; eşleşme 1-1 değil, sayı kıyası anlamsız.",
    "foreign_key_violation":
        "STM_002 distinct-stop agregasyonu (biz 4, MD 2886 per-row) — #2. corpus'ta doğrulandı.",
    "duplicate_key":
        "SHP_008 per-SHAPE agregasyonu (K2 6159 emit → entity-dedup 6 distinct shape; MD per-row).",
    "trip_distance_exceeds_shape_distance_below_threshold":
        "SHP_025 eşik altı varyantı — bizim eşiğimiz farklı, kapsam kararı.",
    "stops_match_shape_out_of_order":
        "SHP_016 kavramsal olarak AYRIK (Faz 5'te doğrulandı) — aynı şeyi ölçmüyorlar.",
    "unused_trip":
        "TRP_017 exact-parite DEĞİL (2. tur audit kararı) — ilişkili ama farklı kural.",
    # ── MAP yorumlarında ZATEN adjudicate edilmiş, makine-okunur hâle getirildi ──
    # ── UNDECIDABLE (sprint ilkesi: "undecidable = bulgu") ──
    "stop_too_far_from_shape_using_user_distance":
        "SHP_024'e eşlenir. tdg-83134'teki 139.4m/165.6m otobüs vakalarının kök nedeni "
        "stop koordinatlarının baş/son boşluk taşımasıydı: K2 strict lexical kaydı boş bırakıyor, "
        "geometri kontrolü ise trim edilmiş sayısal payload'ı artık kullanıyor. Reduced fixture ve "
        "aynı-input feed replay'i iki vakayı geri getiriyor (SHP_024=69; MD=124). Kalan farklar "
        "tam corpus adjudication'ı için açık: MD/analyzer granülerliği ve sdt interpolasyon farkları "
        "ayrıca incelenmeli; 200m rail eşiği bilinçli korunuyor.",
    "service_has_no_active_day_of_the_week":
        "Exact CAL_006 parity. #123 fixed the false negative caused by whitespace-wrapped "
        "weekday zeros: K2 trims only the numeric payload for the weekly-pattern decision, "
        "keeps DQ_016 as the lexical root, and retains CAL_002 for trim-after invalid values. "
        "Same-input mdb-2830 replay recovered 12 CAL_006 findings (services 3308, 3317, "
        "3318, 3338, 3350, 3352, 3354, 3356, 3360, 3362, 3366, 3367); calendar_dates "
        "additions remain an informational dates-only case.",
    "future_feed":
        "future_calendar ile AYNI eksen: MD FEED seviyesinde tek notice, CAL_017 servis başına. "
        "Sayı kıyası anlamsız.",
    "future_calendar":
        "GRANÜLERLİK TERSİ: MD FEED seviyesinde TEK notice basar (örnek: {minServiceStartDate, "
        "currentDate}), CAL_017 ise SERVİS başına. mdb-777: biz 67 (67 servis) ↔ MD 1. "
        "Sayı kıyası anlamsız; içerik aynı.",
    "route_color_contrast":
        "Kontrast eşiği farkı: RTS_008 daha sıkı (20-feed: biz 132 vs MD 3). Erişilebilirlik "
        "tercihi, MD paritesi hedeflenmiyor.",
    "stop_has_too_many_matches_for_shape":
        "DISJOINT: SHP_022 YALNIZ shape_dist_traveled EKSİK trip'lerde çalışır (geometrik "
        "fallback); MD ise shape_dist VARKEN bakar → aynı vakayı ölçmüyorlar. mdb-8'de "
        "biz 9300 / MD 0 çıkmıştı = mis-comparison, parite bug'ı DEĞİL.",
    "trip_distance_exceeds_shape_distance":
        "SHP_025 eşiği >%0,1 (yuvarlamayı tolere eder, SHP_029 felsefesi). mdb-1909: 170 "
        "aşımın 165'i <%0,1 = yuvarlama → biz 5 doğru, MD 170'in hepsini basar.",
    "trip_distance_exceeds_shape_distance_below_threshold":
        "Aynı SHP_025 eşik kararının eşik-altı varyantı — bkz. yukarısı.",
    "service_extends_far_in_the_future":
        "EŞİK FARKI, yapılandırılabilir: max_calendar_future_years=3 (yıl-granüler, tutucu); "
        "MD ~1-2 yıl daha agresif → 2028'i işaretler, biz etmeyiz.",
    "fast_travel_between_consecutive_stops":
        "OVER bilinçli, İKİ bileşen (mdb-767 ile ölçüldü): (1) EŞİK — bizimkiler daha SIKI: "
        "otobüs 120 (MD 150), rail 300 (MD 500); tram/metro/feribot/teleferik AYNI. Yapılandırılabilir "
        "(max_speed_*_kmh). mdb-767: 87 bulgunun 55'i tam bu bantta. (2) MESAFE — biz shape "
        "projeksiyonu (yolun GERÇEK uzunluğu) kullanırız, MD kuş uçuşu (haversine) → hızımız "
        "sistematik olarak yüksek. AYNI segmentte ölçüldü (U2847Z301→U2848Z301, Srbsko→Beroun): "
        "biz 642,2 km/h vs MD 539,1 km/h = 1,19× (MD distanceKm 4,49 → bizimki ~5,35). "
        "Araç yolu takip ettiği için bizim mesafemiz DAHA DOĞRU; birleşik etki ~1,5×. "
        "Shape projeksiyonu Haversine'den kısa olamaz; alt-sınır clamp'i self-near/crossing "
        "shape kaynaklı STM_014 false-negative'lerini önler. #121 ayrıca K2 strict whitespace "
        "kökü nedeniyle kaybolan sayısal stop koordinatlarını K6'da recover eder: aynı-input "
        "replay mdb-510'da STM_014=4, mdb-2712'de STM_014=139 üretti (MD sırasıyla 32 ve 51; "
        "farklar threshold/mesafe ve analyzer segment agregasyonu açısından hâlâ adjudication "
        "gerektiriyor). mdb-2712 pinned 801515, 1049→253 segmenti artık trace edilebilir.",
    "trip_headsign_matches_intermediate_stop":
        "Kalan OVER = YAKIN-YAZIM farkı, bilinçli bırakıldı. 2026-07-17 fix'i (107f929) asıl "
        "FP'yi kapattı (terminal ADI headsign ise atla → mdb-1294: 9.162→171). Kalanlar: "
        "headsign 'MONTI SAN PAOLO/CONFORTI' ↔ terminal 'MONTI S. PAOLO/CONFORTI' = aynı yer, "
        "kısaltma. Çözüm bulanık eşleştirme olurdu → gerçek vakaları bastırma riski; bir FP "
        "sınıfını başkasıyla takas etmeye değmez. TRP_020 INFO/Analytics, SKORU ETKİLEMEZ → "
        "gürültü, zarar değil.",
    "trip_coverage_not_active_for_next7_days":
        "FARKLI EKSEN, ikisi de savunulabilir. MD: 'ÖNEMLİ SAYIDA seferin koştuğu tarih "
        "aralığı' (majority-window) hesaplar ve [bugün, bugün+7] o pencerenin İÇİNDE "
        "olmalıdır (rules.html). TRP_023: 'önümüzdeki 7 günde HERHANGİ bir aktif servis "
        "var mı' — kendi adına birebir sadık. Kanıt mdb-1131 (Vaasa, 2 mevsim): yaz "
        "servisi 566 sefer 1 Haz-2 Ağu AKTİF (17 Tem'de 278 sefer koşuyor) → bizim 0'ımız "
        "DOĞRU; MD kışı (733 sefer, 3 Ağu-31 Ara) 'significant' seçip pencere başlamadı "
        "diye uyarıyor. Aynı desen mdb-2021/2240'te: pencere 1-2 gün SONRA başlıyor, MD "
        "yine uyarıyor (tam kapsama istiyor). NOT: MD'nin ekseni ('kapsama inceliyor') "
        "bizde YOK — yeni kural adayı olabilir, ayrı ürün kararı.",
    "missing_bike_allowance":
        "İKİ YÖNLÜ fark, ikisi de bilinçli: (1) KAPSAM — MD YALNIZ FERİBOT seferlerine bakar "
        "(rules.html: 'All ferry trips should have a valid value in bikes_allowed'), TRP_021 TÜM "
        "seferlere bakar → biz daha geniş, MD'nin vakalarını kapsıyoruz. (2) GRANÜLERLİK — TRP_021 "
        "feed-özeti (tek notice + 5 örnek), MD per-trip (mdb-2933: 118). Sayı kıyası anlamsız; "
        "AGG_RULES'a da eklendi. NOT: MD 'geçerli değer yok' der → GEÇERSİZ değeri de sayar "
        "(ör. bikes_allowed=5); bizde o TRP_007'ye (enum ihlali) gider, TRP_021'e değil.",
    "unexpected_enum_value":
        "Kalan fark YALNIZ route_type: MD temel 0-12 dışını 'beklenmedik' sayar, oysa "
        "genişletilmiş tipler (700 otobüs, 702 ekspres, 200 otokar, 109 banliyö, 712 okul, "
        "715 talep-esaslı) GEÇERLİ ve biz kabul ediyoruz → 0'ımız DOĞRU, MD over-fire. "
        "250-feed: 4.434 örneğin 4.434'ü route_type. Alan-bazlı vakalar MATCH kalır "
        "(EXPLAINED yalnız non-MATCH'i ezer): direction_id→TRP_005 26=26, transfers→FAR_005 6=6.",
}


def classify_unmapped(code: str) -> tuple[str, str]:
    return UNMAPPED_DECISIONS.get(code, ("unreviewed", "No adjudication recorded for this MD code."))


# A code can be correctly mapped and still diverge at scale, because the two validators
# made different product decisions about the same input. Recording those here keeps the
# count honest: a divergence with a decision behind it is not an open finding, and a
# prose-only note would be dropped from any tally that reads this module.
#
# Key is the MD code; value is (decision, reasoning).
MAPPED_DIVERGENCE_DECISIONS = {
    # #162: `md_mapped_missing` — MD raporluyor, eşlediğimiz kural susuyor.
    "missing_required_field": (
        "structural-fault-owns-it",
        "Four feeds, and all four carry a wholly empty CSV row: routes.txt on tld-7825/7876/7877 and "
        "feed_info.txt on tld-4327 are literally ',,,,,'. ARC_018 reports that row and it never reaches K2, "
        "so RTS_031, RTS_004, FIN_001 and FIN_002 cannot fire on it. Deliberate and documented on the "
        "RTS_031 card: a wholly empty row is one fault, not thirteen missing fields. ARC_018 fires on both "
        "inspected feeds, verified against the run's analyzer-rules.csv.",
    ),
    "invalid_color": (
        "not-adjudicated",
        "tfs-342, 2 findings against RTS_006/RTS_007. The one row in this batch that could not be settled: "
        "the feed returns HTTP 403 on three attempts, so the colour values were never read. Everything "
        "recorded about tfs-342 elsewhere comes from the run's own data rather than the archive. Two findings "
        "on route colours, so the stake is small, but it stays open rather than being guessed - the "
        "invalid_url hypothesis in this same round looked just as safe and was wrong.",
    ),
    "pathway_unreachable_location": (
        "scope-difference",
        "mdb-767 and mdb-990. MobilityData checks reachability for platforms, boarding areas and generic "
        "nodes alike; mdb-767 carries 250 boarding areas and 629 generic nodes among its 19,040 stops. "
        "PTH_012 is written around the locked-platform provision and asks a narrower question. The two rules "
        "answer different questions on the same file rather than one of them being wrong, which is why the "
        "mapping between them was never exact. Widening PTH_012 or adding a sibling rule is a design "
        "decision, not a bug fix.",
    ),
    "block_trips_with_overlapping_stop_times": (
        "config-dependent",
        "tld-7929, 3 findings. TRP_022 does compare trips with different service_ids inside a block - #29 "
        "added a calendar-intersection cache precisely so that variant services in one block are only "
        "compared when they actually run on a common day, after an earlier version produced 201,476 findings "
        "against MobilityData's 908 on TriMet. MobilityData's samples here cite intersection date 20230103, "
        "three and a half years before the run date, on a feed whose calendar has expired. The overlap "
        "existed in 2023 and does not exist in any service day we evaluate. Inferred from the intersection "
        "date in their sample rather than from reading the cache against this feed.",
    ),
    "inconsistent_route_type_for_block_id": (
        "analyzer-defect",
        "mdb-2653, 1,502 findings, and MobilityData is right about every one - the count matches exactly once "
        "the join is done on trimmed ids. routes.txt is space-padded to a fixed width, so route_id reads "
        "'10T0001C1  ' while trips.txt carries '10T0001C1'; 838 of 854 routes are affected. TRP_024 builds "
        "its lookup off RouteRecord.route_id and every lookup misses, so it reports nothing across 138,903 "
        "trips. Reading identity raw is the deliberate convention rather than an oversight - every entity "
        "parser does it (routes.rs:53, stops.rs:49, agency.rs:37, calendar.rs:33, shapes.rs:285) and "
        "k2/common.rs:140 cites #85 for it - and k6_analytics.rs alone has 116 sites keying maps off a raw "
        "id field, so this is a policy question and not a one-line fix. Running the analyzer directly on the "
        "feed shows TRP_002 is also silent, which under raw identity should not happen: it should report "
        "138,821 broken references. Two halves of our own pipeline therefore disagree about what an id is. "
        "Tracked in #169; recorded here as our defect because MobilityData's finding is correct.",
    ),
    "transfer_distance_too_large": (
        "structural-fault-owns-it",
        "mdb-1291 and tfs-413, 961 findings each and the two feeds are the same content. The number is exact: "
        "the feed has 70,960 transfers and 961 of them name a to_stop_id that does not exist in stops.txt at "
        "all. Across the 69,999 transfers whose endpoints both resolve, the largest distance is 0.79 km - "
        "nothing is remotely too far. MobilityData computes a distance against a stop it cannot find; we report "
        "the broken reference through TRF_003, which fires on both feeds with 1,926 findings, and decline the "
        "distance because there is nothing to measure. TRF_003 declares TRF_011 in its blocks, so the "
        "suppression is deliberate.",
    ),
    "missing_feed_contact_email_and_url": (
        "structural-fault-owns-it",
        "mdb-1080 and mdb-3235. Both have a feed_info.txt with more than one row - ten and twenty-five "
        "respectively - and MobilityData evaluates each row, so it reports the rows that carry neither contact "
        "field. The first row of each has contact details, which is what FIN_018 reads. FIN_015 fires on both "
        "feeds and names the real fault: feed_info.txt is defined as a single-row file, and reading past row "
        "one would mean treating a malformed file as if its shape were legitimate.",
    ),
    "same_route_and_agency_url": (
        "scope-difference",
        "tld-4477, 3,564 findings, one per route. Every route_url in the feed does equal some agency_url - but "
        "not the url of the route's own agency. The references are sound: agency.txt defines 74 agencies and "
        "all 58 agency_ids used by routes exist. RTS_020 compares a route against its own agency and is "
        "therefore right to stay silent; MobilityData compares against the set of agency urls. A publisher "
        "reusing one portal url across a 74-operator regional feed is ordinary, so widening our comparison "
        "would turn a common pattern into 3,564 findings.",
    ),
    "unused_shape": (
        "md-implementation-limit",
        "mdb-305, 13 findings, and every one is wrong on the feed as written. shapes.txt defines 22 distinct "
        "shape_ids and trips.txt references exactly the same 22 - the sets are identical, including '0sbu' and "
        "'g4f7' which MobilityData names. Three alternative explanations were tested and eliminated: no trip "
        "lacks stop_times (255 of 255 have them, so no cascade from unused_trip), no shape_id on either side "
        "carries surrounding whitespace that would defeat an exact match, and the archive holds one shapes.txt "
        "at the root with no wrapper. Their own definition is 'all records defined by shapes.txt should be used "
        "in trips.txt', which this feed satisfies. SHP_018 is right to stay silent.",
    ),
    "unsorted_stop_times": (
        "md-implementation-limit",
        "mdb-1806, 45 findings, and the file is correctly sorted. stop_times.txt carries a blank line between "
        "every single data row, and the data itself is clean: 1,750 non-empty rows, 48 trips in contiguous "
        "blocks with no trip split across the file, and stop_sequence strictly increasing inside every one of "
        "them. MobilityData's sample cites rows 155-229 for BLUWD09, roughly double our numbering, which is "
        "what happens when the blank lines are counted as rows - their adjacency check then sees a row with no "
        "trip_id between every pair. Our parser drops empty rows, so STM_036 reads the file as it is. "
        "Separately worth noting: we report a comma-only row via ARC_018 but say nothing about a wholly blank "
        "line, which is its own inconsistency.",
    ),
    "service_window_outside_feed_period": (
        "structural-fault-owns-it",
        "mdb-3155. feed_info.txt is written with a space after every comma, so feed_start_date reads "
        "' 20260301' and feed_end_date ' 20281204'. CAL_014 and CAL_019 need both endpoints as dates to "
        "compare a service window against them, and a value with a leading space is not one. DQ_016 fires on "
        "this feed and names the real defect. The comparison MobilityData makes is correct - the window does "
        "run 25 to 27 days past the feed period - but it is downstream of a lexical fault we already report.",
    ),
    "feed_expiration_date7_days": (
        "structural-fault-owns-it",
        "mdb-2026, same shape as service_window_outside_feed_period and on the same axis. feed_info.txt is "
        "comma-space separated, so feed_end_date reads ' 20250819' and FIN_010/FIN_019 have no parseable date "
        "to compare against today. DQ_016 fires, and so does FIN_008 on the ' planning@wavetransit.com' in the "
        "same row - the whitespace is reported twice under its own name. Note the feed expired a year before "
        "the run date, so CAL_013 and CAL_024 do report the expiry from the calendar side. Verified on mdb-2026; "
        "mdb-3235 not inspected.",
    ),
    "stop_time_with_arrival_before_previous_departure_time": (
        "structural-fault-owns-it",
        "mdb-2366. MobilityData compares each row against the previous one and reports 168 violations, citing "
        "pairs such as row 45701 against row 40169 - more than five thousand rows apart. STM_036 fires on this "
        "feed: the file is not sorted by trip_id and stop_sequence, so 'the previous row' is not the previous "
        "stop. The time comparison is downstream of an ordering fault we already report, and running it on "
        "unsorted input would produce findings about pairs that are not adjacent in the trip.",
    ),
    "invalid_url": (
        "tolerance-by-design",
        "Sixteen rows, every value populated and every one parsing cleanly - checking mattered, because the "
        "empty-optional pattern I expected was not what was there. The stop_url cluster carries 'http://167', "
        "'http://426', 'http://205', which the url crate resolves through the legacy integer form to "
        "0.0.0.167 and friends. The agency_url cluster carries a doubled scheme, underscored hosts, "
        "'http://localhost' and a typo TLD. All satisfy the spec sentence: a fully qualified URL with a "
        "scheme and correctly escaped special characters. MobilityData additionally judges the host to be "
        "implausible; we do not, except for the doubled scheme, which has one unambiguous signature and is "
        "now rejected. The rest stay accepted on purpose: AGN_003 is Kritik+Spec and therefore an R1 publish "
        "blocker, so a false positive there rejects a valid feed. Underscored hosts exist on intranets, "
        "single-label hosts are legal, and TLD validation needs a maintained list that would reject new TLDs. "
        "#144 removed an is_ascii() gate from url_strict_ok for the same asymmetry; a general host-shape "
        "check risks reintroducing it.",
    ),

    "missing_required_file": (
        "tolerance-by-design",
        "In the full-catalog run 33 of the 34 feeds carrying this code also emit ARC_024, and "
        "none emit ARC_004. Those feeds wrap the whole GTFS in one folder. MobilityData refuses "
        "to look inside and calls agency.txt and stop_times.txt missing; detect_wrapped_root "
        "accepts the folder as the root, so the files are present for us and the publisher gets "
        "a full report instead of zero analysis. The divergence is the tolerance working as "
        "designed (see docs/rules/ARC/ARC_024.md), not an ARC_004 gap. The 34th feed, mdb-2933, "
        "was a mapping gap and is fixed: MD named feed_info.txt, which is conditionally required "
        "and belongs to ARC_031. Measured in #145.",
    ),
    "missing_recommended_file": (
        "structural-fault-owns-it",
        "All 35 feeds carrying this code in the full-catalog run are archives we could not "
        "read as a usable GTFS feed, and every one of them names feed_info.txt. 24 are the "
        "wrapped-root population (MobilityData reports invalid_input_files_in_subfolder and "
        "refuses the folder). The other 11 are not GTFS feeds at all: mdb-6 holds a single "
        "directory entry, tdg-84076 and tdg-84073 hold nested ZIPs, mdb-3135 is a GitHub repo "
        "zip carrying .csv files, and 10 of the 11 emit ARC_004. Once the archive is "
        "structurally broken we report that fault and stop; adding 'a recommended file is also "
        "missing' would stack noise on a feed nobody can validate anyway. MobilityData reports "
        "both. Measured in #146.",
    ),
    "missing_calendar_and_calendar_date_files": (
        "tolerance-by-design",
        "All 33 feeds carrying this code are the same wrapped-root population as "
        "missing_required_file, and none emit ARC_008. Same cause, same decision.",
    ),
}


def classify_mapped_divergence(code: str) -> tuple[str, str]:
    """Why a correctly mapped code still diverges, when we have decided that it may."""
    return MAPPED_DIVERGENCE_DECISIONS.get(
        code, ("unreviewed", "No decision recorded for a mapped divergence on this MD code.")
    )


def classify_divergence(code: str) -> tuple[str, str]:
    """The one call any parity consumer should make before reporting a divergence.

    Three ledgers now live in this module — ``BY_DESIGN``, ``UNMAPPED_DECISIONS`` and
    ``MAPPED_DIVERGENCE_DECISIONS``. They were written at different times for different
    shapes of divergence, but from a consumer's side they answer one question: has this
    difference already been judged, and why?

    Ask through here rather than reading a single table. The full-catalog benchmark
    (#146) loaded only ``MAP`` and ``AGG_RULES``, so it never saw ``BY_DESIGN`` and
    re-reported **74% of an already-adjudicated backlog as fresh blindness** — 4,220 of
    5,714 rows, including 782 feeds of ``non_ascii_or_non_printable_char`` whose reasoning
    had been on record since 2026-07-03. A ledger a consumer does not read is not a ledger.
    """
    for source, table in (
        ("by-design", BY_DESIGN),
        ("mapped-divergence", MAPPED_DIVERGENCE_DECISIONS),
        ("unmapped", UNMAPPED_DECISIONS),
    ):
        entry = table.get(code)
        if entry is None:
            continue
        # BY_DESIGN stores a bare reason; the other two store (decision, reason).
        return (source, entry) if isinstance(entry, str) else (f"{source}:{entry[0]}", entry[1])
    return ("unreviewed", "No decision recorded for this MD code in any parity ledger.")


# ── Analyzer tarafı defteri ────────────────────────────────────────────────────
#
# Yukarıdaki üç defter MD KODUNA göre anahtarlanır ve `classify_divergence()` onları
# okur. Ama sapmanın diğer yönü — "biz bir Spec ihlali iddia ediyoruz, MobilityData
# susuyor" — KURAL KİMLİĞİNE göre anahtarlanır ve hükümleri `fp_adjudication.tsv`'de
# durur. O dosyayı hiçbir tüketici okumuyordu.
#
# 🔴 Sonuç #146'nın birebir tekrarıydı: 251 satırın HEPSİ yargılanmış olduğu hâlde
# her koşumda "yargılanmamış" raporlanıyordu. Okunmayan defter defter değildir.

_FP_LEDGER = Path(__file__).resolve().parent / "fp_adjudication.tsv"

# Bir hükmün ayrışmayı KAPATIP kapatmadığı, hükmün NE İDDİA ETTİĞİNE bağlıdır.
#
# Kapatanlar KALICI bir durum anlatır: "biz haklıyız, onlar dar" (TRUE_POSITIVE),
# "eşleme yanlış kodu gösteriyor" (MAPPING_ARTIFACT), "kök neden başka kuralda"
# (CASCADE_*). Bunlar bir sonraki koşumda da geçerlidir.
#
# Kapatmayanlar bir DEĞİŞİKLİK iddia eder: `FALSE_POSITIVE_FIXED` "kuralı bunu
# yapmasın diye değiştirdik" demektir. Kural yine ateşliyorsa bu bir GERİLEMEDİR ve
# görünmelidir — bastırmak, tam da yakalamak için var olduğu şeyi gizler.
SETTLING_VERDICTS = frozenset({
    "TRUE_POSITIVE",
    "MAPPING_ARTIFACT",
    "SPLIT_VERDICT",
    "CASCADE_NOT_SUPPRESSIBLE",
    "SUPPRESSED_SINCE",
})
REGRESSION_SENSITIVE_VERDICTS = frozenset({
    "FALSE_POSITIVE_FIXED",
    "DERIVATIVE_FIXED",
    "DUPLICATE_FIXED",
    "NOT_ADJUDICATED",
})


@lru_cache(maxsize=1)
def _fp_verdicts():
    """rule_id → (verdict, run_id, reasoning).

    Aynı kural birden çok koşumda yargılandıysa EN SON koşumun hükmü geçerlidir:
    kural o arada değişmiş olabilir ve eski hüküm eski davranış hakkındadır.
    """
    out = {}
    if not _FP_LEDGER.exists():
        return out
    with _FP_LEDGER.open(encoding="utf-8", newline="") as fh:
        for row in csv.DictReader(fh, delimiter="\t"):
            rid = (row.get("rule_id") or "").strip()
            if not rid:
                continue
            run = (row.get("run_id") or "").strip()
            verdict = (row.get("verdict") or "").strip()
            prev = out.get(rid)
            # 🔴 "En son kazanır" TEK BAŞINA yanlıştır. `NOT_ADJUDICATED` bir HÜKÜM
            # DEĞİL, hüküm verilmediğinin kaydıdır — genellikle bir kuralın YENİ bir
            # alt vakası bulunduğunda düşülür. Onun daha eski bir gerçek hükmü ezmesi,
            # verilmiş bir kararı geri almaktır. `AGN_003` tam bu durumdadır: 
            # run 31934698855'te TRUE_POSITIVE, run 31981225727'de çift-şema alt
            # vakası için NOT_ADJUDICATED. İkisi aynı soruya iki cevap değildir.
            if verdict == "NOT_ADJUDICATED" and prev is not None:
                continue
            if prev is not None and prev[0] != "NOT_ADJUDICATED" and run <= prev[1]:
                continue
            out[rid] = (verdict, run, (row.get("reasoning") or "").strip())
    return out


def classify_analyzer_divergence(rule_id: str):
    """`classify_divergence`'ın analyzer tarafındaki karşılığı.

    ``("adjudicated:<verdict>", gerekçe)`` ya da ``("unreviewed", sebep)`` döner.
    `CASCADE_ROOT_ARC_012` gibi son ek taşıyan hükümler önekle eşleşir.
    """
    entry = _fp_verdicts().get(rule_id)
    if entry is None:
        return ("unreviewed", "No verdict recorded for this rule in fp_adjudication.tsv.")
    verdict, run, reason = entry
    if verdict in SETTLING_VERDICTS or verdict.startswith("CASCADE_ROOT_"):
        return (f"adjudicated:{verdict}", f"run {run}: {reason}")
    return ("unreviewed",
            f"Verdict {verdict} (run {run}) claims a change was made; a fresh "
            f"occurrence is a regression and stays visible. {reason}")


def is_adjudicated(code: str) -> bool:
    """True when a divergence on this MD code has a recorded decision behind it."""
    return classify_divergence(code)[0] != "unreviewed"


def resolve_mapping(
    code: str,
    notice: dict[str, Any] | None,
    fallback_rules: Iterable[str] = (),
) -> MappingResult:
    """Resolve one aggregate MD notice using its sample context.

    If samples identify a context, only the most specific matching candidate
    is used for each sample.  If the report has no usable context, the legacy
    fallback mapping is retained and the report says ``fallback``.  Unknown
    contexts are counted as unresolved so they cannot be mistaken for a clean
    mapping.
    """

    entries = CONTEXT_BY_CODE.get(code)
    fallback = tuple(fallback_rules)
    if not entries:
        return MappingResult(fallback, "static")

    samples = _sample_notices(notice)
    total_notices = _total_notices(notice)
    # sampleNotices is representative, not necessarily exhaustive.  If the
    # aggregate is larger than the visible sample, a single observed field
    # cannot safely explain the whole MD count.
    context_complete = total_notices is None or total_notices <= len(samples)
    if not samples or not any(_filename(s) or _field(s) or _entity(s, _filename(s)) for s in samples):
        return MappingResult(fallback, "fallback", context_complete=context_complete and bool(samples))

    rules: list[str] = []
    labels: list[str] = []
    unresolved = 0
    for sample in samples:
        matches = [entry for entry in entries if entry.matches(sample)]
        if not matches:
            unresolved += 1
            continue
        best = max(entry.specificity for entry in matches)
        for entry in matches:
            if entry.specificity == best:
                rules.extend(entry.analyzer_rules)
                if entry.label and entry.label not in labels:
                    labels.append(entry.label)

    # A report may contain samples from more than one field.  Keep all
    # candidate rules, but mark the result as mixed: aggregate counts from the
    # golden snapshot cannot be apportioned to those fields safely.
    unique_rules = tuple(dict.fromkeys(rules))
    if not unique_rules:
        return MappingResult(
            (),
            "unresolved-context",
            (),
            unresolved or len(samples),
            False,
        )
    kind = (
        "context-dependent"
        if len(labels) <= 1 and unresolved == 0 and context_complete
        else "context-mixed"
    )
    return MappingResult(unique_rules, kind, tuple(labels), unresolved, context_complete)


def context_rules() -> set[str]:
    return {rule for entry in CONTEXT_MAPPINGS for rule in entry.analyzer_rules}
