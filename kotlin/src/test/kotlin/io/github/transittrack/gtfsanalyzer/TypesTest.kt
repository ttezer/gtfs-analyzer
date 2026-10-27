package io.github.transittrack.gtfsanalyzer

import kotlinx.serialization.json.Json
import kotlinx.serialization.decodeFromString
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class TypesTest {
    @Test
    fun decodesANoticeWithAllFieldsPresent() {
        val json = """
            {"id":"n1","rule_id":"ARC_001","severity":"CRITICAL","rule_class":"SPEC",
             "entity_type":"Feed","entity_id":null,"scope_key":null,"file":"stops.txt",
             "line":12,"field":"stop_lat","observed_value":"91.0","expected_value":"<=90",
             "details":{"k":"v"},"title":"t","message":"m","remediation":"r",
             "blocks":["ARC_002"],"base_effort":1,"service_id":null}
        """.trimIndent()
        val notice = Json.decodeFromString<Notice>(json)
        assertEquals("n1", notice.id)
        assertEquals(Severity.CRITICAL, notice.severity)
        assertEquals(RuleClass.SPEC, notice.ruleClass)
        assertEquals(EntityType.Feed, notice.entityType)
        assertEquals(12L, notice.line)
        assertEquals(listOf("ARC_002"), notice.blocks)
        assertEquals(1, notice.baseEffort)
        assertNull(notice.serviceId)
    }

    @Test
    fun decodesANoticeWithServiceIdOmittedEntirely() {
        // service_id is additive/optional in the engine contract; older engine JSON
        // (or any notice that isn't trip-scoped) may not carry the key at all.
        val json = """
            {"id":"n2","rule_id":"ARC_002","severity":"LOW","rule_class":"QUALITY",
             "entity_type":"Trip","entity_id":"t1","scope_key":"t1","file":null,
             "line":null,"field":null,"observed_value":null,"expected_value":null,
             "details":null,"title":"t","message":"m","remediation":"r",
             "blocks":[],"base_effort":2}
        """.trimIndent()
        val notice = Json.decodeFromString<Notice>(json)
        assertNull(notice.serviceId)
        assertNull(notice.details)
        assertNull(notice.file)
    }

    @Test
    fun decodesAFatalError() {
        val json = """{"code":"ZipUnreadable","message":"bad zip"}"""
        val fatal = Json.decodeFromString<FatalError>(json)
        assertEquals(FatalCode.ZipUnreadable, fatal.code)
        assertEquals("bad zip", fatal.message)
        assertNull(fatal.detail)
    }

    @Test
    fun decodesAFatalErrorWithDetail() {
        val json = """{"code":"InvalidInput","message":"bad config","detail":"unknown key foo"}"""
        val fatal = Json.decodeFromString<FatalError>(json)
        assertEquals(FatalCode.InvalidInput, fatal.code)
        assertEquals("unknown key foo", fatal.detail)
    }

    @Test
    fun decodesAnR5ReportWithAllScores() {
        val json = """{"score":90.0,"pub_score":95.0,"spec_score":100.0,"interop_score":80.0,"quality_score":70.0,"analytics_score":60.0}"""
        val r5 = Json.decodeFromString<R5Report>(json)
        assertEquals(90.0, r5.score)
    }

    @Test
    fun decodesAnR1ReportWithCoverageCompleteOmitted() {
        // Mirrors the Rust #[serde(default = "default_true")] on R1Report.coverage_complete.
        val json = """{"publishable":true,"blocker_notice_ids":[]}"""
        val r1 = Json.decodeFromString<R1Report>(json)
        assertTrue(r1.coverageComplete)
        assertTrue(r1.publishable)
        assertTrue(r1.blockerNoticeIds.isEmpty())
    }

    @Test
    fun decodesAnR9ReportWithNestedItems() {
        val json = """
            {"items":[
                {"rule_id":"RUL_1","labels":["blocker","quick-win"],"priority_score":9.5,
                 "score_delta":1.2,"pub_score_delta":0.5,"affected_instance_count":3,
                 "realized_dependent_count":1,"base_effort":1,"fix_effort":1.0,
                 "notice_ids":["n1","n2"]}
            ]}
        """.trimIndent()
        val r9 = Json.decodeFromString<R9Report>(json)
        assertEquals(1, r9.items.size)
        val item = r9.items[0]
        assertEquals("RUL_1", item.ruleId)
        assertEquals(listOf(R9Label.Blocker, R9Label.QuickWin), item.labels)
        assertEquals(3, item.affectedInstanceCount)
        assertEquals(listOf("n1", "n2"), item.noticeIds)
    }

    @Test
    fun decodesAPartialReportWithSkippedChecksOmitted() {
        val json = """
            {"root_structural_errors":["bad zip"],"unavailable_files":["stops.txt"],"skipped_stages":["K2"]}
        """.trimIndent()
        val partial = Json.decodeFromString<PartialReport>(json)
        assertEquals(listOf("bad zip"), partial.rootStructuralErrors)
        assertTrue(partial.skippedChecks.isEmpty())
    }

    @Test
    fun decodesFeedMetricsWithOptionalFieldsOmitted() {
        val json = """
            {"stop_count":10,"route_count":2,"trip_count":5,"shape_count":2,
             "active_service_days":30,"avg_daily_trips":12.5,
             "feed_start_date":null,"feed_end_date":null,"service_start_date":null,
             "service_end_date":null,"spec_notice_count":0,"interop_notice_count":0,
             "quality_notice_count":0,"analytics_notice_count":0,"overall_score":95.5,
             "file_stats":[{"name":"stops.txt","rows":10,"bytes":1024}]}
        """.trimIndent()
        val metrics = Json.decodeFromString<FeedMetrics>(json)
        assertTrue(metrics.coverageComplete)
        assertFalse(metrics.isGtfsJp)
        assertNull(metrics.gtfsJpProfile)
        assertEquals(1, metrics.fileStats.size)
        assertEquals("stops.txt", metrics.fileStats[0].name)
    }

    @Test
    fun decodesAFullValidationResultWithNestedReportsAndNameIndex() {
        val json = """
            {
              "validation_status":"COMPLETE",
              "notices":[],
              "reports":{
                "r1":{"publishable":true,"coverage_complete":true,"blocker_notice_ids":[]},
                "r2":{"items":[{"notice_id":"n1","report_id":"R2","display_label":"ARC_001","is_primary":true}]},
                "r3":{"items":[]},
                "r4":{"items":[]},
                "r5":{"score":90.0,"pub_score":95.0,"spec_score":100.0,"interop_score":80.0,"quality_score":70.0,"analytics_score":60.0},
                "r7":{"items":[]},
                "r8":{"items":[]},
                "r9":{"items":[]}
              },
              "metrics":{
                "stop_count":1,"route_count":1,"trip_count":1,"shape_count":1,
                "active_service_days":1,"avg_daily_trips":1.0,
                "feed_start_date":20260101,"feed_end_date":20261231,
                "service_start_date":20260101,"service_end_date":20261231,
                "spec_notice_count":0,"interop_notice_count":0,
                "quality_notice_count":0,"analytics_notice_count":0,"overall_score":100.0,
                "coverage_complete":true,"file_stats":[]
              },
              "name_index":{
                "stops":{"s1":"Main St"},
                "routes":{"r1":"1"},
                "trips":{"t1":"Downtown"},
                "trip_routes":{"t1":"r1"},
                "trip_directions":{"t1":"0"},
                "stop_coords":{"s1":[41.0,29.0]},
                "trip_first_dep":{"t1":"08:00"},
                "shape_routes":{"sh1":[["r1","0"]]},
                "shape_coords":{"sh1":[[41.0,29.0],[41.1,29.1]]},
                "trip_shapes":{"t1":"sh1"},
                "trip_stops":{"t1":["s1"]},
                "shape_trips":{"sh1":"t1"},
                "route_shapes":{"r1":["sh1"]},
                "map_data_deferred":false
              },
              "capped_totals":{"RUL_1":501}
            }
        """.trimIndent()
        val result = Json.decodeFromString<ValidationResult>(json)
        assertEquals(ValidationStatus.COMPLETE, result.validationStatus)
        assertNull(result.partial)
        assertTrue(result.notices.isEmpty())
        assertEquals(1, result.reports.r2.items.size)
        assertEquals(ReportId.R2, result.reports.r2.items[0].reportId)
        assertEquals(20260101L, result.metrics.feedStartDate)
        assertEquals(listOf(41.0, 29.0), result.nameIndex.stopCoords["s1"])
        assertEquals(listOf(listOf("r1", "0")), result.nameIndex.shapeRoutes["sh1"])
        assertEquals(
            listOf(listOf(41.0, 29.0), listOf(41.1, 29.1)),
            result.nameIndex.shapeCoords["sh1"],
        )
        assertEquals(501, result.cappedTotals["RUL_1"])
    }

    @Test
    fun decodesAValidationResultWithValidationStatusAndPartialOmitted() {
        val json = """
            {
              "notices":[],
              "reports":{
                "r1":{"publishable":true,"coverage_complete":true,"blocker_notice_ids":[]},
                "r2":{"items":[]},"r3":{"items":[]},"r4":{"items":[]},
                "r5":{"score":0.0,"pub_score":0.0,"spec_score":0.0,"interop_score":0.0,"quality_score":0.0,"analytics_score":0.0},
                "r7":{"items":[]},"r8":{"items":[]},"r9":{"items":[]}
              },
              "metrics":{
                "stop_count":0,"route_count":0,"trip_count":0,"shape_count":0,
                "active_service_days":0,"avg_daily_trips":0.0,
                "feed_start_date":null,"feed_end_date":null,"service_start_date":null,"service_end_date":null,
                "spec_notice_count":0,"interop_notice_count":0,"quality_notice_count":0,"analytics_notice_count":0,
                "overall_score":0.0,"file_stats":[]
              },
              "name_index":{
                "stops":{},"routes":{},"trips":{},"trip_routes":{},"trip_directions":{},
                "stop_coords":{},"trip_first_dep":{},"shape_routes":{},"shape_coords":{},
                "trip_shapes":{},"trip_stops":{},"shape_trips":{},"route_shapes":{},
                "map_data_deferred":true
              },
              "capped_totals":{}
            }
        """.trimIndent()
        val result = Json.decodeFromString<ValidationResult>(json)
        assertNull(result.validationStatus)
        assertNull(result.partial)
    }
}
