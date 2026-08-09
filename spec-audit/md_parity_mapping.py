"""Context-aware MobilityData -> GTFS Analyzer parity mappings.

MobilityData reports aggregate a notice code across files and fields.  A flat
``code -> rule ids`` map is therefore safe only for codes whose meaning is
already specific.  Generic codes are resolved from the sample notice context
before they are compared with Analyzer's rule counts.

The module intentionally contains no filesystem or network access.  It is
imported by :mod:`md_parity_audit` and by the CI drift check.
"""

from dataclasses import dataclass
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


def _filename(sample: dict[str, Any]) -> str:
    value = sample.get("filename", sample.get("fileName", sample.get("file")))
    return _normalise(value).rsplit("/", 1)[-1]


def _field(sample: dict[str, Any]) -> str:
    value = sample.get("fieldName", sample.get("field_name", sample.get("field")))
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
    _ctx("number_out_of_range", "PTH_007", filename=("pathways.txt",), fields=("traversal_time",), label="pathways.txt::traversal_time"),
    _ctx("number_out_of_range", "SHP_002", filename=("shapes.txt",), fields=("shape_pt_lat",), label="shapes.txt::shape_pt_lat"),
    _ctx("number_out_of_range", "SHP_003", filename=("shapes.txt",), fields=("shape_pt_lon",), label="shapes.txt::shape_pt_lon"),
    _ctx("number_out_of_range", "STP_003", filename=("stops.txt",), fields=("stop_lat",), label="stops.txt::stop_lat"),
    _ctx("number_out_of_range", "STP_005", filename=("stops.txt",), fields=("stop_lon",), label="stops.txt::stop_lon"),
    _ctx("number_out_of_range", "RCT_004", filename=("rider_categories.txt",), fields=("min_age", "max_age"), label="rider_categories.txt::min_age/max_age"),
    _ctx("number_out_of_range", "FRQ_008", filename=("frequencies.txt",), fields=("headway_secs",), value_range=(0, 0), label="frequencies.txt::headway_secs zero"),
    # Generic notices are resolved by filename/field. A partial sample stays
    # CONTEXT, so an unseen field cannot be presented as exact parity.
    _ctx("invalid_integer", "TRP_005", filename=("trips.txt",), fields=("direction_id",), label="trips.txt::direction_id"),
    _ctx("invalid_url", "AGN_003", filename=("agency.txt",), fields=("agency_url",), label="agency.txt::agency_url"),
    _ctx("invalid_url", "RTS_005", filename=("routes.txt",), fields=("route_url",), label="routes.txt::route_url"),
    _ctx("invalid_url", "FIN_002", filename=("feed_info.txt",), fields=("feed_publisher_url",), label="feed_info.txt::feed_publisher_url"),
    _ctx("missing_required_field", "AGN_003", filename=("agency.txt",), fields=("agency_url",), label="agency.txt::agency_url"),
    _ctx("missing_required_field", "FIN_002", filename=("feed_info.txt",), fields=("feed_publisher_url",), label="feed_info.txt::feed_publisher_url"),
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
    "fast_travel_between_far_stops": (
        "genuine-gap",
        "Analyzer has no rule for non-consecutive far-stop pairs; do not alias this to STM_012.",
    ),
    "feed_expiration_date30_days": (
        "genuine-gap",
        "The MD notice is a feed_info-level 30-day horizon. CAL_008 is per-calendar-service and "
        "FIN_019 is a 7-day feed warning, so neither is an exact counterpart.",
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


def classify_unmapped(code: str) -> tuple[str, str]:
    return UNMAPPED_DECISIONS.get(code, ("unreviewed", "No adjudication recorded for this MD code."))


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
