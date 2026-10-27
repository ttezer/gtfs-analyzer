package io.github.transittrack.gtfsanalyzer

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Kotlin data classes mirroring `sdk/src/types.ts` (lines 1-147) plus `FatalCode`/
// `FatalError` from the same file. These decode the JSON produced by the Rust
// engine — the same engine the npm SDK, Python SDK, and CLI all read.
//
// Field naming: snake_case JSON keys (the engine's wire format) map to camelCase
// Kotlin properties via `@SerialName`. Nullability and defaults mirror the TS
// `?`/`| null` markers so that partial/older-engine JSON missing an additive
// field still decodes (TS comment: "Eklemeli alan: eski motor sürümleri taşımaz").
//
// Scope note: `ValidationResult.agency_breakdown`/`feed_summary` (and the
// `AgencyBreakdown`/`FeedSummary` type family) live past line 147 of types.ts
// and are intentionally out of scope for this file — a later task's job.

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
    // Rust: `rows: u32`, `bytes: u32`.
    val rows: Int,
    val bytes: Int,
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
)

@Serializable
data class FatalError(
    val code: FatalCode,
    val message: String,
    // TS: `detail?: string` — optional engine diagnostic.
    val detail: String? = null,
)
