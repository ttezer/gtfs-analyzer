package io.github.transittrack.gtfsanalyzer

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// Kotlin mirror of `sdk/src/types.ts`'s `ValidatorConfigDelta` (lines 180-222)
// and `CalendarOverrideRule` (lines 171-177). This is the object a Kotlin
// caller fills in to override engine tuning values before validating a feed;
// every field is optional/nullable with a `null` default, matching every
// field in the TS interface being `?`.
//
// Encoded via `EngineJson` (Task 6's `JsonCodec.kt`), whose `encodeDefaults =
// false` + `explicitNulls = false` means only fields the caller actually set
// are serialized — the engine rejects unknown keys, so this must never emit
// keys the caller didn't touch.
//
// Field types mirror `crates/config/src/lib.rs`'s `ValidatorConfig` struct:
// Rust `f64` -> Double, Rust `u32` -> Int (per the `Notice`/`FeedMetrics`
// convention in Types.kt of mapping `u32` counts to Int), `Vec<String>` ->
// List<String>, `Vec<CalendarOverrideRule>` -> List<CalendarOverrideRule>.

/** Calendar base/override relationship used by OPR_021/022/023. */
@Serializable
data class CalendarOverrideRule(
    @SerialName("route_id") val routeId: String,
    @SerialName("base_service_ids") val baseServiceIds: List<String>,
    @SerialName("override_service_ids") val overrideServiceIds: List<String>,
    // Rust: `start_date: u32` / `end_date: u32` (YYYYMMDD) — Long per the
    // `*_date` epoch-day-shaped field convention in Types.kt.
    @SerialName("start_date") val startDate: Long,
    @SerialName("end_date") val endDate: Long,
)

/** All configuration keys accepted by the Rust/WASM engine. */
@Serializable
data class ValidatorConfigDelta(
    @SerialName("source_url") val sourceUrl: String? = null,
    // TS: `gtfs_jp_profile?: GtfsJpProfile` ('auto' | 'v3' | 'v4'); kept as a
    // plain string to match `FeedMetrics.gtfsJpProfile`'s convention.
    @SerialName("gtfs_jp_profile") val gtfsJpProfile: String? = null,
    @SerialName("stop_name_best_practices") val stopNameBestPractices: Boolean? = null,
    @SerialName("max_speed_bus_kmh") val maxSpeedBusKmh: Double? = null,
    @SerialName("max_speed_tram_kmh") val maxSpeedTramKmh: Double? = null,
    @SerialName("max_speed_metro_kmh") val maxSpeedMetroKmh: Double? = null,
    @SerialName("max_speed_rail_kmh") val maxSpeedRailKmh: Double? = null,
    @SerialName("max_speed_ferry_kmh") val maxSpeedFerryKmh: Double? = null,
    @SerialName("max_speed_cablecar_kmh") val maxSpeedCablecarKmh: Double? = null,
    @SerialName("min_transfer_time_sec") val minTransferTimeSec: Int? = null,
    @SerialName("max_transfer_distance_m") val maxTransferDistanceM: Double? = null,
    @SerialName("max_shape_jump_km") val maxShapeJumpKm: Double? = null,
    @SerialName("max_shape_jump_km_rail") val maxShapeJumpKmRail: Double? = null,
    @SerialName("stop_too_close_m") val stopTooCloseM: Double? = null,
    @SerialName("stop_far_from_shape_m") val stopFarFromShapeM: Double? = null,
    @SerialName("stop_far_from_shape_m_rail") val stopFarFromShapeMRail: Double? = null,
    @SerialName("stop_far_from_parent_m") val stopFarFromParentM: Double? = null,
    @SerialName("feed_expiry_warning_days") val feedExpiryWarningDays: Int? = null,
    @SerialName("feed_info_expiry_warning_days") val feedInfoExpiryWarningDays: Int? = null,
    @SerialName("service_gap_days") val serviceGapDays: Int? = null,
    @SerialName("big_gap_days") val bigGapDays: Int? = null,
    @SerialName("upcoming_service_days") val upcomingServiceDays: Int? = null,
    @SerialName("max_trip_duration_hours") val maxTripDurationHours: Double? = null,
    @SerialName("max_trip_duration_hours_rail") val maxTripDurationHoursRail: Double? = null,
    @SerialName("min_trip_duration_sec") val minTripDurationSec: Int? = null,
    @SerialName("max_headway_warning_min") val maxHeadwayWarningMin: Int? = null,
    @SerialName("max_headway_warning_min_rail") val maxHeadwayWarningMinRail: Int? = null,
    @SerialName("service_day_window_hours_rail") val serviceDayWindowHoursRail: Int? = null,
    @SerialName("bunching_threshold_min") val bunchingThresholdMin: Int? = null,
    @SerialName("rail_stop_distance_km") val railStopDistanceKm: Double? = null,
    @SerialName("max_trips_per_route") val maxTripsPerRoute: Int? = null,
    @SerialName("duration_outlier_sigma") val durationOutlierSigma: Double? = null,
    @SerialName("headway_outlier_sigma") val headwayOutlierSigma: Double? = null,
    @SerialName("service_day_start_hour") val serviceDayStartHour: Int? = null,
    @SerialName("max_calendar_future_years") val maxCalendarFutureYears: Int? = null,
    @SerialName("rural_route_ids") val ruralRouteIds: List<String>? = null,
    @SerialName("calendar_override_rules") val calendarOverrideRules: List<CalendarOverrideRule>? = null,
    // Rule IDs omitted from notices, scores, report views, and R9.
    @SerialName("disabled_rule_ids") val disabledRuleIds: List<String>? = null,
    // Rule IDs omitted only when the feed is detected as GTFS-JP.
    @SerialName("disabled_rule_ids_gtfs_jp") val disabledRuleIdsGtfsJp: List<String>? = null,
)
