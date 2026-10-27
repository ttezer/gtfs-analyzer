package io.github.transittrack.gtfsanalyzer

import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class VersionTest {
    @Test
    fun sdkVersionMatchesWorkspaceCargoToml() {
        val cargoToml = File("../Cargo.toml").readText()
        val expected = Regex("""\[workspace\.package][^\[]*?version\s*=\s*"([^"]+)"""", RegexOption.DOT_MATCHES_ALL)
            .find(cargoToml)!!.groupValues[1]
        assertEquals(expected, SDK_VERSION)
        assertTrue(SDK_VERSION.matches(Regex("""\d+\.\d+\.\d+""")))
    }
}
