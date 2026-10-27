package io.github.transittrack.gtfsanalyzer

import org.junit.jupiter.api.assertThrows
import java.io.IOException
import java.nio.file.Path
import kotlin.test.Test

class GtfsAnalyzerIntegrationTest {
    @Test
    fun validateThrowsIOExceptionForAMissingFeedPath() {
        assertThrows<IOException> {
            GtfsAnalyzer.validate(Path.of("/does/not/exist.zip"))
        }
    }
}
