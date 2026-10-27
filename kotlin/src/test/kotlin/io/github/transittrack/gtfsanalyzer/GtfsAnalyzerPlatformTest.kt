package io.github.transittrack.gtfsanalyzer

import org.junit.jupiter.api.assertThrows
import java.nio.file.Path
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class GtfsAnalyzerPlatformTest {
    @Test
    fun detectsDarwinAarch64() {
        assertEquals("darwin-aarch64", detectPlatformTag("Mac OS X", "aarch64"))
    }

    @Test
    fun detectsLinuxX86_64() {
        assertEquals("linux-x86_64", detectPlatformTag("Linux", "amd64"))
    }

    @Test
    fun detectsLinuxAarch64() {
        assertEquals("linux-aarch64", detectPlatformTag("Linux", "aarch64"))
    }

    @Test
    fun rejectsUnsupportedPlatform() {
        val ex = assertThrows<UnsupportedPlatformException> { detectPlatformTag("Windows 11", "amd64") }
        assertTrue(ex.message!!.contains("Windows 11"))
    }

    @Test
    fun resolveWritableNativeLibDirThrowsNamingEveryCandidateWhenNoneAreWritable() {
        val candidates = listOf(Path.of("/a"), Path.of("/b"))
        val ex = assertThrows<NativeLibraryLoadException> {
            resolveWritableNativeLibDir(candidates, isWritable = { false })
        }
        assertTrue(ex.message!!.contains("/a") && ex.message!!.contains("/b"))
    }

    @Test
    fun resolveWritableNativeLibDirReturnsFirstWritableCandidate() {
        val candidates = listOf(Path.of("/a"), Path.of("/b"))
        assertEquals(Path.of("/b"), resolveWritableNativeLibDir(candidates, isWritable = { it == Path.of("/b") }))
    }
}
