package aulo.speech

// What picks an engine. These live in the contract so config, storage, the gRPC layer and engine
// modules share them without depending on the registry.

/** Most fallback engines one layer may list; each may load a model, so a begin stays bounded. */
const val MAX_FALLBACK_ENGINES = 16

/** Everything a factory needs to build one engine. STT ignores the rate. */
data class EngineSpec(
    val engine: EngineId,
    /** Engine-specific model name; null takes the engine's default. */
    val model: String? = null,
    /** A [Voice.id]; null takes the engine's default. */
    val voice: String? = null,
    val rate: SpeechRate = SpeechRate.NORMAL,
) {
    init {
        model?.let { requireLabel("model", it) }
        voice?.let { requireLabel("voice id", it) }
    }
}

/**
 * One layer of engine choice: `[voice.stt]` or `[voice.tts]`, a bot override, or a chat override.
 * Unset fields defer to the layer below, so an override names only what it changes. Overrides
 * come from gRPC clients, so model and voice are bounded like any engine label.
 */
data class EngineChoice(
    val engine: EngineId? = null,
    val model: String? = null,
    val voice: String? = null,
    val rate: SpeechRate? = null,
    /** Engines tried in order when the chosen ones fail. */
    val fallback: List<EngineId> = emptyList(),
) {
    init {
        model?.let { requireLabel("model", it) }
        voice?.let { requireLabel("voice id", it) }
        if (fallback.size > MAX_FALLBACK_ENGINES) {
            throw SpeechException.Invalid("fallback", "lists more than $MAX_FALLBACK_ENGINES engines")
        }
    }
}
