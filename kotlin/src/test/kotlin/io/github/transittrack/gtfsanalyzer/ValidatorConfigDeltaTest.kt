package io.github.transittrack.gtfsanalyzer

import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.double
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals

class ValidatorConfigDeltaTest {
    @Test
    fun encodesOnlyTheFieldsThatWereSet() {
        val delta = ValidatorConfigDelta(maxSpeedBusKmh = 80.0, disabledRuleIds = listOf("DQ_004"))
        val json = EngineJson.encodeToString(delta)
        val decoded = EngineJson.parseToJsonElement(json).jsonObject
        assertEquals(setOf("max_speed_bus_kmh", "disabled_rule_ids"), decoded.keys)
        assertEquals(80.0, decoded["max_speed_bus_kmh"]!!.jsonPrimitive.double)
    }

    @Test
    fun defaultInstanceEncodesToEmptyObject() {
        assertEquals("{}", EngineJson.encodeToString(ValidatorConfigDelta()))
    }
}
