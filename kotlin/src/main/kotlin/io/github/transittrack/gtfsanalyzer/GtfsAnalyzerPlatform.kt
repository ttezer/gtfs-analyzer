package io.github.transittrack.gtfsanalyzer

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardCopyOption

/**
 * Thrown by [detectPlatformTag] when the current JVM is running on an operating system / CPU
 * architecture combination for which this SDK does not ship a prebuilt native library.
 */
class UnsupportedPlatformException(message: String) : RuntimeException(message)

/**
 * Thrown by [resolveWritableNativeLibDir] (and, transitively, [loadNativeLibrary]) when no
 * writable directory can be found to extract the native library into, or when the native
 * library resource itself cannot be located on the classpath.
 */
class NativeLibraryLoadException(message: String) : RuntimeException(message)

/**
 * Maps the current JVM's `os.name` / `os.arch` system properties onto one of the platform tags
 * this SDK ships a prebuilt native library for: `darwin-aarch64`, `linux-x86_64`, or
 * `linux-aarch64`. Throws [UnsupportedPlatformException] for anything else.
 */
fun detectPlatformTag(
    osName: String = System.getProperty("os.name"),
    osArch: String = System.getProperty("os.arch"),
): String {
    val name = osName.lowercase()
    val arch = osArch.lowercase()
    return when {
        name.contains("mac") && arch.contains("aarch64") -> "darwin-aarch64"
        name.contains("linux") && (arch.contains("amd64") || arch.contains("x86_64")) -> "linux-x86_64"
        name.contains("linux") && arch.contains("aarch64") -> "linux-aarch64"
        else -> throw UnsupportedPlatformException(
            "unsupported platform: osName=$osName osArch=$osArch; supported: darwin-aarch64, linux-x86_64, linux-aarch64"
        )
    }
}

/**
 * Returns the first of [candidates] accepted by [isWritable], in order. Throws
 * [NativeLibraryLoadException] naming every candidate that was tried if none of them are
 * writable.
 */
fun resolveWritableNativeLibDir(
    candidates: List<Path>,
    isWritable: (Path) -> Boolean = Files::isWritable,
): Path =
    candidates.firstOrNull(isWritable) ?: throw NativeLibraryLoadException(
        "no writable directory for the native library among: ${candidates.joinToString()}; " +
            "set -Dgtfs.analyzer.nativeDir=<a writable dir> or mount a writable volume"
    )

// Anchors classpath resource lookups to this file's own classloader, independent of whatever
// class a caller happens to invoke loadNativeLibrary() through.
private object NativeLibraryResource

/**
 * Loads the prebuilt native library matching the current JVM's platform, extracting it from the
 * jar's bundled resources into a writable directory and calling [System.load] on it.
 *
 * Safe to call repeatedly and from multiple threads: the actual load work runs at most once,
 * guarded by Kotlin's default `by lazy` (`LazyThreadSafetyMode.SYNCHRONIZED`) delegate; later
 * calls are no-ops.
 *
 * Throws [UnsupportedPlatformException] if the current platform is unsupported, or
 * [NativeLibraryLoadException] if the matching native library resource is missing from the
 * classpath or no writable directory can be found to extract it into.
 */
fun loadNativeLibrary() {
    nativeLibraryLoaded
}

private val nativeLibraryLoaded: Unit by lazy {
    val tag = detectPlatformTag()
    val ext = if (tag.startsWith("darwin")) "dylib" else "so"
    val resourcePath = "/natives/$tag/libgtfs_jvm.$ext"
    val resourceStream = NativeLibraryResource::class.java.getResourceAsStream(resourcePath)
        ?: throw NativeLibraryLoadException("native library resource not found on classpath: $resourcePath")

    val candidates = listOf(
        System.getProperty("gtfs.analyzer.nativeDir"),
        System.getProperty("java.io.tmpdir"),
    ).filterNotNull().map(Path::of)
    val dir = resolveWritableNativeLibDir(candidates)

    val extractedPath = dir.resolve("libgtfs_jvm-$SDK_VERSION.$ext")
    resourceStream.use { input ->
        Files.copy(input, extractedPath, StandardCopyOption.REPLACE_EXISTING)
    }

    System.load(extractedPath.toString())
}
