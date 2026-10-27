package io.github.transittrack.gtfsanalyzer

/**
 * Thrown by [GtfsAnalyzer] (Task 9) when a native call reports that the Rust engine caught a
 * panic. The message is the panic message surfaced through the JNI boundary.
 */
class GtfsEngineException(message: String) : RuntimeException(message)
