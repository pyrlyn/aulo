package aulo.voice

import aulo.speech.EngineChoice
import aulo.speech.EngineSpec
import aulo.speech.SpeechRate

/**
 * The engines to try, best first, without duplicates. A [chat] choice overrides its [bot], and a
 * bot overrides [config] (`[voice.stt]` or `[voice.tts]`).
 *
 * - Every layer that names an engine adds it, chat first. The configured engine stays behind an
 *   override, so a broken override falls back to what the user set up before the fallback list.
 * - A layer that sets only a model or voice refines the next engine below it. A model or voice
 *   never carries over to a different engine, where it would most likely be unknown.
 * - Rate and fallback come from the highest layer that sets them; fallback engines start with
 *   their default model and voice.
 */
fun candidates(
    config: EngineChoice,
    bot: EngineChoice? = null,
    chat: EngineChoice? = null,
): List<EngineSpec> {
    val layers = listOfNotNull(chat, bot, config)
    val rate = layers.firstNotNullOfOrNull { it.rate } ?: SpeechRate.NORMAL
    val fallback = layers.firstOrNull { it.fallback.isNotEmpty() }?.fallback.orEmpty()

    val out = mutableListOf<EngineSpec>()
    var model: String? = null
    var voice: String? = null
    for (layer in layers) {
        model = model ?: layer.model
        voice = voice ?: layer.voice
        val engine = layer.engine ?: continue
        out.addNew(EngineSpec(engine, model, voice, rate))
        model = null
        voice = null
    }
    fallback.forEach { out.addNew(EngineSpec(it, rate = rate)) }
    return out
}

/** The first mention keeps its model and voice; a retry later in the same list would fail the same way. */
private fun MutableList<EngineSpec>.addNew(spec: EngineSpec) {
    if (none { it.engine == spec.engine }) add(spec)
}
