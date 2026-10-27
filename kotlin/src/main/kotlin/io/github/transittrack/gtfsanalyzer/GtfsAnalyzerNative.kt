package io.github.transittrack.gtfsanalyzer

/**
 * Raw JNI bridge to the Rust validation engine. The package name, object name, and method name
 * here are mechanically encoded into the exported native symbol
 * `Java_io_github_transittrack_gtfsanalyzer_GtfsAnalyzerNative_validate` (see
 * `crates/jvm/src/lib.rs`) — renaming any of them breaks the link at runtime.
 *
 * `@JvmStatic` is required so this compiles to a true `static` JVM method on the class, matching
 * the `Java_..._ClassName_methodName` symbol pattern the JNI expects, rather than an instance
 * method on the `GtfsAnalyzerNative` singleton (which JNI would see as taking an implicit
 * receiver and never match).
 *
 * Not for direct use — see [GtfsAnalyzer] for the public, typed entry point.
 */
internal object GtfsAnalyzerNative {
    @JvmStatic
    external fun validate(
        feed: ByteArray,
        today: Int,
        configJson: String,
        includeNameIndex: Boolean,
        lang: String,
    ): String
}
