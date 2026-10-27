package io.github.transittrack.gtfsanalyzer

import kotlinx.serialization.json.Json

// Shared Json configuration for everything that talks to the Rust engine's wire
// format (EngineResult today; ValidatorConfigDelta in Task 7).
val EngineJson = Json {
    ignoreUnknownKeys = true // forward-compat: an older SDK must not choke on a newer engine's new fields
    encodeDefaults = false // used for ValidatorConfigDelta in Task 7: omit unset fields entirely
    explicitNulls = false
}
