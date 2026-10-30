package io.github.transittrack.gtfsanalyzer

import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.buildClassSerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.put

// Mirrors the Rust engine's `ValidateResult::Ok(ValidationResult) | ValidateResult::Fatal(FatalError)`
// enum, which serde_json serializes externally-tagged as a single-key JSON object:
// `{"Ok": {...}}` or `{"Fatal": {...}}`. kotlinx.serialization's default sealed-class
// serializer adds its own "type" discriminator field instead of this shape, so this
// is a hand-written KSerializer that reads/writes exactly the engine's one-key envelope.
@Serializable(with = EngineResultSerializer::class)
sealed class EngineResult {
    data class Ok(val result: ValidationResult) : EngineResult()
    data class Fatal(val error: FatalError) : EngineResult()
}

object EngineResultSerializer : KSerializer<EngineResult> {
    override val descriptor: SerialDescriptor =
        buildClassSerialDescriptor("io.github.transittrack.gtfsanalyzer.EngineResult")

    override fun deserialize(decoder: Decoder): EngineResult {
        val jsonDecoder = decoder as? JsonDecoder
            ?: error("EngineResult can only be deserialized from JSON")
        val envelope = jsonDecoder.decodeJsonElement().jsonObject
        val key = envelope.keys.singleOrNull()
            ?: throw IllegalArgumentException(
                "expected a single-key EngineResult envelope (\"Ok\" or \"Fatal\"), got keys: ${envelope.keys}"
            )
        val value = envelope.getValue(key)
        return when (key) {
            "Ok" -> EngineResult.Ok(jsonDecoder.json.decodeFromJsonElement(ValidationResult.serializer(), value))
            "Fatal" -> EngineResult.Fatal(jsonDecoder.json.decodeFromJsonElement(FatalError.serializer(), value))
            else -> throw IllegalArgumentException("unknown EngineResult variant: \"$key\"")
        }
    }

    override fun serialize(encoder: Encoder, value: EngineResult) {
        val jsonEncoder = encoder as? JsonEncoder
            ?: error("EngineResult can only be serialized to JSON")
        val envelope = when (value) {
            is EngineResult.Ok -> buildJsonObject {
                put("Ok", jsonEncoder.json.encodeToJsonElement(ValidationResult.serializer(), value.result))
            }
            is EngineResult.Fatal -> buildJsonObject {
                put("Fatal", jsonEncoder.json.encodeToJsonElement(FatalError.serializer(), value.error))
            }
        }
        jsonEncoder.encodeJsonElement(envelope)
    }
}
