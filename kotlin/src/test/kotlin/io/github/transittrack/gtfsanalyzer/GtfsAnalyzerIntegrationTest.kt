package io.github.transittrack.gtfsanalyzer

import org.junit.jupiter.api.assertThrows
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.nio.file.Path
import java.time.LocalDate
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs

class GtfsAnalyzerIntegrationTest {
    @Test
    fun validateThrowsIOExceptionForAMissingFeedPath() {
        assertThrows<IOException> {
            GtfsAnalyzer.validate(Path.of("/does/not/exist.zip"))
        }
    }

    // Same fixture bytes as crates/pipeline/tests/integration.rs's base_files(): 1 agency,
    // 2 stops, 1 route, 1 trip, one calendar entry covering 2026-05-15 (TODAY below).
    private val TODAY: LocalDate = LocalDate.of(2026, 5, 15)

    private fun minimalFeedZip(): ByteArray {
        val agency =
            "agency_id,agency_name,agency_url,agency_timezone\n1,Test,http://test.example,UTC\n"
        val stops =
            "stop_id,stop_name,stop_lat,stop_lon\nS1,Stop1,41.0,29.0\nS2,Stop2,41.1,29.1\n"
        val routes = "route_id,agency_id,route_short_name,route_type\nR1,1,101,3\n"
        val trips = "route_id,service_id,trip_id\nR1,SVC1,T1\n"
        val stopTimes =
            "trip_id,arrival_time,departure_time,stop_id,stop_sequence\n" +
                "T1,08:00:00,08:00:00,S1,1\nT1,08:10:00,08:10:00,S2,2\n"
        val calendar =
            "service_id,monday,tuesday,wednesday,thursday,friday,saturday,sunday,start_date,end_date\n" +
                "SVC1,1,1,1,1,1,0,0,20250101,20271231\n"

        val files = listOf(
            "agency.txt" to agency,
            "stops.txt" to stops,
            "routes.txt" to routes,
            "trips.txt" to trips,
            "stop_times.txt" to stopTimes,
            "calendar.txt" to calendar,
        )

        val buffer = ByteArrayOutputStream()
        ZipOutputStream(buffer).use { zip ->
            for ((name, content) in files) {
                zip.putNextEntry(ZipEntry(name))
                zip.write(content.toByteArray(Charsets.UTF_8))
                zip.closeEntry()
            }
        }
        return buffer.toByteArray()
    }

    @Test
    fun validateReturnsOkWithExpectedMetricsForACleanFeed() {
        val result = GtfsAnalyzer.validate(minimalFeedZip(), today = TODAY)
        assertIs<EngineResult.Ok>(result)
        val ok = result as EngineResult.Ok
        assertEquals(2, ok.result.metrics.stopCount)
        assertEquals(1, ok.result.metrics.routeCount)
        assertEquals(1, ok.result.metrics.tripCount)
    }

    @Test
    fun validateReturnsFatalForACorruptZip() {
        val result = GtfsAnalyzer.validate("not a zip".toByteArray(), today = TODAY)
        assertIs<EngineResult.Fatal>(result)
        assertEquals(FatalCode.ZipUnreadable, (result as EngineResult.Fatal).error.code)
    }

    @Test
    fun validateThrowsIllegalArgumentExceptionForAnUnknownConfigKey() {
        // ValidatorConfigDelta only exposes known fields, so simulate an unknown key the
        // way a hand-built config would reach the engine: encode, then splice in a bad key.
        val badConfigJson = """{"not_a_real_key": 1}"""
        assertThrows<IllegalArgumentException> {
            GtfsAnalyzerNative.validate(minimalFeedZip(), 20260515, badConfigJson, false, "en")
        }
    }

    @Test
    fun validateIsSafeToCallConcurrently() {
        val executor = Executors.newFixedThreadPool(8)
        val futures = (1..32).map {
            executor.submit<EngineResult> { GtfsAnalyzer.validate(minimalFeedZip(), today = TODAY) }
        }
        futures.forEach { f -> assertIs<EngineResult.Ok>(f.get(30, TimeUnit.SECONDS)) }
        executor.shutdown()
    }
}
