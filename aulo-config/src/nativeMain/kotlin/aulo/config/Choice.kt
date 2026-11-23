package aulo.config

import aulo.speech.EngineChoice
import aulo.speech.EngineId
import aulo.speech.SpeechException
import aulo.speech.SpeechRate

/**
 * The validated choice the engine registry resolves from a raw `[voice.stt]` or `[voice.tts]`
 * section. Validation lives in the speech contract; this only names the offending key and, when
 * [provenance] is given, the layer that set it. [key] is the section's dotted path (`voice.tts`).
 */
fun EngineConfig.choice(
    key: String,
    provenance: Provenance? = null,
): EngineChoice {
    fun <T> field(
        name: String,
        build: () -> T,
    ): T =
        try {
            build()
        } catch (e: SpeechException) {
            // The contract's messages never quote the value, so a misplaced secret stays out.
            val path = "$key.$name"
            throw ConfigException(provenance?.get(path), path, "${e.message} for key `$path`")
        }

    val fallbackIds = field("fallback") { fallback.map(::EngineId).also { EngineChoice(fallback = it) } }
    return EngineChoice(
        engine = field("engine") { engine?.let(::EngineId) },
        model = field("model") { model?.also { EngineChoice(model = it) } },
        voice = field("voice") { voice?.also { EngineChoice(voice = it) } },
        rate = field("rate") { rate?.let(::SpeechRate) },
        fallback = fallbackIds,
    )
}
