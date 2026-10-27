package io.github.transittrack.gtfsanalyzer

import kotlinx.serialization.decodeFromString
import kotlinx.serialization.encodeToString
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

// The smallest legal `ValidationResult`: every field with a Kotlin-side default
// (validation_status, partial, the *_date fields, coverage_complete, is_gtfs_jp,
// gtfs_jp_profile) is omitted entirely; every field without a default (notices,
// reports, metrics, name_index, capped_totals, and their own required sub-fields)
// is present with the smallest valid value (empty list/map, or 0/0.0/false).
private const val MINIMAL_VALIDATION_RESULT_JSON = """
{
  "notices":[],
  "reports":{
    "r1":{"publishable":true,"blocker_notice_ids":[]},
    "r2":{"items":[]},"r3":{"items":[]},"r4":{"items":[]},
    "r5":{"score":0.0,"pub_score":0.0,"spec_score":0.0,"interop_score":0.0,"quality_score":0.0,"analytics_score":0.0},
    "r7":{"items":[]},"r8":{"items":[]},"r9":{"items":[]}
  },
  "metrics":{
    "stop_count":0,"route_count":0,"trip_count":0,"shape_count":0,
    "active_service_days":0,"avg_daily_trips":0.0,
    "spec_notice_count":0,"interop_notice_count":0,"quality_notice_count":0,"analytics_notice_count":0,
    "overall_score":0.0,"file_stats":[]
  },
  "name_index":{
    "stops":{},"routes":{},"trips":{},"trip_routes":{},"trip_directions":{},
    "stop_coords":{},"trip_first_dep":{},"shape_routes":{},"shape_coords":{},
    "trip_shapes":{},"trip_stops":{},"shape_trips":{},"route_shapes":{},
    "map_data_deferred":false
  },
  "capped_totals":{}
}
"""

class EngineResultTest {
    @Test
    fun decodesOkEnvelope() {
        val json = """{"Ok": $MINIMAL_VALIDATION_RESULT_JSON}"""
        val result = EngineJson.decodeFromString<EngineResult>(json)
        assertIs<EngineResult.Ok>(result)
    }

    @Test
    fun decodesFatalEnvelope() {
        val json = """{"Fatal": {"code":"ZipUnreadable","message":"bad zip"}}"""
        val result = EngineJson.decodeFromString<EngineResult>(json)
        assertIs<EngineResult.Fatal>(result)
        assertEquals(FatalCode.ZipUnreadable, (result as EngineResult.Fatal).error.code)
    }

    @Test
    fun roundTripsFatalEnvelope() {
        val original: EngineResult = EngineResult.Fatal(FatalError(FatalCode.ZipUnreadable, "bad zip"))
        val encoded = EngineJson.encodeToString(original)
        val decoded = EngineJson.decodeFromString<EngineResult>(encoded)
        assertEquals(original, decoded)
    }
}
