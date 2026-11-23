package aulo.speech.helper

import aulo.speech.EngineId
import aulo.speech.SpeechException
import aulo.voice.TtsRegistry
import process.Command

/** The macOS voices, spoken by the `aulo-speech-system` helper. */
val MAC_VOICES = EngineId("macos-voices")

/**
 * Registers [MAC_VOICES]. Each build starts its own [helper] process (`aulo-speech-system tts`)
 * and waits for its voice list; a helper that cannot start, which is every system but macOS for
 * now, is unavailable, so the registry falls back to the next engine.
 */
fun TtsRegistry.registerMacVoices(
    helper: String,
    options: HelperTtsOptions = HelperTtsOptions(),
) {
    register(MAC_VOICES) { spec ->
        if (spec.model != null) throw SpeechException.Unsupported("model ${spec.model}")
        HelperTts.start(MAC_VOICES, "macOS voices", ProcessLink.spawn(Command(helper, listOf("tts"))), options)
    }
}
