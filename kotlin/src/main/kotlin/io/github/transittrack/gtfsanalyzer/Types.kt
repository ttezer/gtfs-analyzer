package io.github.transittrack.gtfsanalyzer

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Kotlin data classes mirroring `sdk/src/types.ts` plus `FatalCode`/`FatalError`
// from the same file. These decode the JSON produced by the Rust engine — the
// same engine the npm SDK, Python SDK, and CLI all read.
//
// Field naming: snake_case JSON keys (the engine's wire format) map to camelCase
// Kotlin properties via `@SerialName`. Nullability and defaults mirror the TS
// `?`/`| null` markers so that partial/older-engine JSON missing an additive
// field still decodes (TS comment: "Eklemeli alan: eski motor sürümleri taşımaz").

@Serializable
enum class Severity {
    CRITICAL,
    HIGH,
    MEDIUM,
    LOW,
    INFO,
}

@Serializable
enum class RuleClass {
    SPEC,
    INTEROP,
    QUALITY,
    ANALYTICS,
}

@Serializable
enum class EntityType {
    Feed,
    File,
    Agency,
    Stop,
    Route,
    Trip,
    Shape,
    Service,
    Fare,
    Transfer,
    Pathway,
    Level,
    Translation,
    Attribution,
    Row,
}

@Serializable
enum class ReportId {
    R1,
    R2,
    R3,
    R4,
    R5,
    R7,
    R8,
    R9,
}

@Serializable
enum class R9Label {
    @SerialName("blocker") Blocker,
    @SerialName("interop") Interop,
    @SerialName("propagation") Propagation,
    @SerialName("quick-win") QuickWin,
    @SerialName("quality") Quality,
    @SerialName("widespread") Widespread,
    @SerialName("analytics") Analytics,
    @SerialName("hard") Hard,
    @SerialName("single") Single,
    @SerialName("high-impact") HighImpact,
}

@Serializable
enum class FatalCode {
    ZipUnreadable,
    Utf8Critical,
    NoRequiredFiles,
    CsvMalformed,
    DecompressionLimit,
    ResourceLimit,
    InvalidInput,
}

// `COMPLETE` | `PARTIAL` literal union from `ValidationResult.validation_status`.
@Serializable
enum class ValidationStatus {
    @SerialName("COMPLETE") COMPLETE,
    @SerialName("PARTIAL") PARTIAL,
}

@Serializable
data class Notice(
    val id: String,
    @SerialName("rule_id") val ruleId: String,
    val severity: Severity,
    @SerialName("rule_class") val ruleClass: RuleClass,
    @SerialName("entity_type") val entityType: EntityType,
    @SerialName("entity_id") val entityId: String? = null,
    @SerialName("scope_key") val scopeKey: String? = null,
    val file: String? = null,
    // Rust: `line: Option<u64>` — Long to tolerate huge files beyond Int range.
    val line: Long? = null,
    val field: String? = null,
    @SerialName("observed_value") val observedValue: String? = null,
    @SerialName("expected_value") val expectedValue: String? = null,
    val details: Map<String, String>? = null,
    val title: String,
    val message: String,
    val remediation: String,
    val blocks: List<String>,
    // Rust: `base_effort: u8` (1/2/3).
    @SerialName("base_effort") val baseEffort: Int,
    // TS: `service_id?: string | null` — additive field; trip-scoped notices only.
    @SerialName("service_id") val serviceId: String? = null,
)

@Serializable
data class ReportItem(
    @SerialName("notice_id") val noticeId: String,
    @SerialName("report_id") val reportId: ReportId,
    @SerialName("display_label") val displayLabel: String,
    @SerialName("is_primary") val isPrimary: Boolean,
)

@Serializable
data class R1Report(
    val publishable: Boolean,
    // Rust: `#[serde(default = "default_true")]` — additive field, defaults true
    // so older-engine JSON without it still reads as "fully covered".
    @SerialName("coverage_complete") val coverageComplete: Boolean = true,
    @SerialName("blocker_notice_ids") val blockerNoticeIds: List<String>,
)

@Serializable
data class R2Report(val items: List<ReportItem>)

@Serializable
data class R3Report(val items: List<ReportItem>)

@Serializable
data class R4Report(val items: List<ReportItem>)

@Serializable
data class R5Report(
    val score: Double,
    @SerialName("pub_score") val pubScore: Double,
    @SerialName("spec_score") val specScore: Double,
    @SerialName("interop_score") val interopScore: Double,
    @SerialName("quality_score") val qualityScore: Double,
    @SerialName("analytics_score") val analyticsScore: Double,
)

@Serializable
data class R7Report(val items: List<ReportItem>)

@Serializable
data class R8Report(val items: List<ReportItem>)

@Serializable
data class R9Item(
    @SerialName("rule_id") val ruleId: String,
    val labels: List<R9Label>,
    @SerialName("priority_score") val priorityScore: Double,
    @SerialName("score_delta") val scoreDelta: Double,
    @SerialName("pub_score_delta") val pubScoreDelta: Double,
    // Rust: `u32` counts.
    @SerialName("affected_instance_count") val affectedInstanceCount: Int,
    @SerialName("realized_dependent_count") val realizedDependentCount: Int,
    // Rust: `base_effort: u8`.
    @SerialName("base_effort") val baseEffort: Int,
    @SerialName("fix_effort") val fixEffort: Double,
    @SerialName("notice_ids") val noticeIds: List<String>,
)

@Serializable
data class R9Report(val items: List<R9Item>)

@Serializable
data class ReportSet(
    val r1: R1Report,
    val r2: R2Report,
    val r3: R3Report,
    val r4: R4Report,
    val r5: R5Report,
    val r7: R7Report,
    val r8: R8Report,
    val r9: R9Report,
)

@Serializable
data class FileInfo(
    val name: String,
    // Rust: `rows: u32`, `bytes: u32`. Long (not Int): real feeds have been measured
    // with a single file around 2.74 GiB, which exceeds Int.MAX_VALUE (~2.1B) but
    // still fits in u32 (max ~4.29B) — decoding such a value into an Int throws a
    // SerializationException for the whole result. `rows` is converted alongside
    // `bytes` for the same sibling-u32-field reason, even though a rows overflow is
    // far less likely in practice.
    val rows: Long,
    val bytes: Long,
)

@Serializable
data class FeedMetrics(
    // Rust: `u32` entity counts.
    @SerialName("stop_count") val stopCount: Int,
    @SerialName("route_count") val routeCount: Int,
    @SerialName("trip_count") val tripCount: Int,
    @SerialName("shape_count") val shapeCount: Int,
    @SerialName("active_service_days") val activeServiceDays: Int,
    // Rust: `avg_daily_trips: f64`.
    @SerialName("avg_daily_trips") val avgDailyTrips: Double,
    // Rust: `Option<u32>` YYYYMMDD dates. Long per the brief's rule for `*_date`
    // epoch-day-shaped fields, to be safe against overflow.
    @SerialName("feed_start_date") val feedStartDate: Long? = null,
    @SerialName("feed_end_date") val feedEndDate: Long? = null,
    @SerialName("service_start_date") val serviceStartDate: Long? = null,
    @SerialName("service_end_date") val serviceEndDate: Long? = null,
    @SerialName("spec_notice_count") val specNoticeCount: Int,
    @SerialName("interop_notice_count") val interopNoticeCount: Int,
    @SerialName("quality_notice_count") val qualityNoticeCount: Int,
    @SerialName("analytics_notice_count") val analyticsNoticeCount: Int,
    @SerialName("overall_score") val overallScore: Double,
    // TS: `coverage_complete?: boolean`; Rust defaults true (additive field).
    @SerialName("coverage_complete") val coverageComplete: Boolean = true,
    @SerialName("file_stats") val fileStats: List<FileInfo>,
    // TS: `is_gtfs_jp?: boolean`; Rust `#[serde(default)]` → false.
    @SerialName("is_gtfs_jp") val isGtfsJp: Boolean = false,
    // TS: `'auto' | 'v3' | 'v4' | string | null` — kept as a plain string since the
    // union explicitly allows an arbitrary string, not just the three literals.
    @SerialName("gtfs_jp_profile") val gtfsJpProfile: String? = null,
)

@Serializable
data class NameIndex(
    val stops: Map<String, String>,
    val routes: Map<String, String>,
    val trips: Map<String, String>,
    @SerialName("trip_routes") val tripRoutes: Map<String, String>,
    @SerialName("trip_directions") val tripDirections: Map<String, String>,
    // Rust: `BTreeMap<String, [f64; 2]>` — [lat, lon].
    @SerialName("stop_coords") val stopCoords: Map<String, List<Double>>,
    @SerialName("trip_first_dep") val tripFirstDep: Map<String, String>,
    // Rust: `BTreeMap<String, Vec<[String; 2]>>` — [route_id, direction] pairs.
    @SerialName("shape_routes") val shapeRoutes: Map<String, List<List<String>>>,
    // Rust: `BTreeMap<String, Vec<[f64; 2]>>` — ordered [lat, lon] points.
    @SerialName("shape_coords") val shapeCoords: Map<String, List<List<Double>>>,
    @SerialName("trip_shapes") val tripShapes: Map<String, String>,
    @SerialName("trip_stops") val tripStops: Map<String, List<String>>,
    @SerialName("shape_trips") val shapeTrips: Map<String, String>,
    @SerialName("route_shapes") val routeShapes: Map<String, List<String>>,
    @SerialName("map_data_deferred") val mapDataDeferred: Boolean,
)

@Serializable
data class PartialReport(
    @SerialName("root_structural_errors") val rootStructuralErrors: List<String>,
    @SerialName("unavailable_files") val unavailableFiles: List<String>,
    @SerialName("skipped_stages") val skippedStages: List<String>,
    // TS: `skipped_checks?: string[]`; Rust `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
    @SerialName("skipped_checks") val skippedChecks: List<String> = emptyList(),
)

// `agency.txt` record, as seen through the per-rule agency breakdown (#2201).
// Rust (crates/core/src/agency.rs `AgencyJson`): `trip_count` is `u64`, so it maps
// to Long; `notice_indices` indexes into the (capped) `notices` array and stays Int.
@Serializable
data class AgencyInfo(
    @SerialName("agency_id") val agencyId: String,
    @SerialName("agency_name") val agencyName: String,
    @SerialName("trip_count") val tripCount: Long,
    @SerialName("notice_indices") val noticeIndices: List<Int>,
)

// A set of agencies jointly affected by a finding (shared stop/shape/service).
// Rust `AgencySetJson`: both counts are `u64` -> Long.
@Serializable
data class AgencySetCount(
    @SerialName("agency_ids") val agencyIds: List<String>,
    @SerialName("affected_entity_count") val affectedEntityCount: Long,
    @SerialName("trim_fallback_count") val trimFallbackCount: Long,
)

// Per-rule counts in the agency breakdown. Rust `RuleJson`: every count field is
// `u64` -> Long, matching the existing `Notice.line`/`u64` convention in this file.
@Serializable
data class RuleAgencyCounts(
    @SerialName("finding_count") val findingCount: Long,
    @SerialName("affected_entity_count") val affectedEntityCount: Long,
    @SerialName("displayed_sample_count") val displayedSampleCount: Long,
    @SerialName("agency_sets") val agencySets: List<AgencySetCount>,
    val unattributed: Map<String, Long>,
    val unsupported: Long,
    @SerialName("not_applicable") val notApplicable: Long,
)

// Rule x agency breakdown (MobilityData gtfs-validator #2201 parity). Reporting-only:
// does not affect severity or score. Rust: `AgencyBreakdown` (crates/core/src/agency.rs).
@Serializable
data class AgencyBreakdown(
    val complete: Boolean,
    val agencies: List<AgencyInfo>,
    val rules: Map<String, RuleAgencyCounts>,
)

// `agency.txt` contact fields (trimmed; blank fields omitted). Rust: `AgencyContact`
// (crates/core/src/feed_summary.rs) — only `agency_name` is required.
@Serializable
data class AgencyContact(
    @SerialName("agency_id") val agencyId: String? = null,
    @SerialName("agency_name") val agencyName: String,
    @SerialName("agency_url") val agencyUrl: String? = null,
    @SerialName("agency_timezone") val agencyTimezone: String? = null,
    @SerialName("agency_lang") val agencyLang: String? = null,
    @SerialName("agency_phone") val agencyPhone: String? = null,
    @SerialName("agency_email") val agencyEmail: String? = null,
    @SerialName("agency_fare_url") val agencyFareUrl: String? = null,
)

// `feed_info.txt` first-record publisher fields (dates live in `FeedMetrics`).
// Rust: `FeedInfoSummary` — every field optional.
@Serializable
data class FeedInfoSummary(
    @SerialName("feed_publisher_name") val feedPublisherName: String? = null,
    @SerialName("feed_publisher_url") val feedPublisherUrl: String? = null,
    @SerialName("feed_lang") val feedLang: String? = null,
    @SerialName("feed_version") val feedVersion: String? = null,
    @SerialName("feed_contact_email") val feedContactEmail: String? = null,
    @SerialName("feed_contact_url") val feedContactUrl: String? = null,
)

// Agency/publisher summary card data. Rust: `FeedSummary` — `agencies` capped at
// `MAX_AGENCIES` (100); the true count is `agency_count` (`u32`, kept as Int: bounded
// by `agency.txt` row count, which stays far below Int range in practice).
@Serializable
data class FeedSummary(
    @SerialName("agency_count") val agencyCount: Int,
    val agencies: List<AgencyContact> = emptyList(),
    @SerialName("feed_info") val feedInfo: FeedInfoSummary? = null,
)

@Serializable
data class ValidationResult(
    // TS: `validation_status?: 'COMPLETE' | 'PARTIAL'`.
    @SerialName("validation_status") val validationStatus: ValidationStatus? = null,
    val partial: PartialReport? = null,
    val notices: List<Notice>,
    val reports: ReportSet,
    val metrics: FeedMetrics,
    @SerialName("name_index") val nameIndex: NameIndex,
    // Rust: `BTreeMap<String, u32>`.
    @SerialName("capped_totals") val cappedTotals: Map<String, Int>,
    // TS: `agency_breakdown?: AgencyBreakdown` — additive field (#2201).
    @SerialName("agency_breakdown") val agencyBreakdown: AgencyBreakdown? = null,
    // TS: `feed_summary?: FeedSummary` — additive field.
    @SerialName("feed_summary") val feedSummary: FeedSummary? = null,
)

@Serializable
data class FatalError(
    val code: FatalCode,
    val message: String,
    // TS: `detail?: string` — optional engine diagnostic.
    val detail: String? = null,
)
