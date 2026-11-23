package aulo.speech.helper

import aulo.speech.Capabilities
import aulo.speech.EngineId
import aulo.speech.EngineInfo
import aulo.speech.EngineKind
import aulo.speech.LanguageSupport
import aulo.speech.SpeechException
import aulo.voice.SttRegistry
import process.Command
import process.currentExe

/** Decoder threads per helper; past 4 Parakeet barely gets faster (spike T0.5). */
const val SHERPA_THREADS = 4

/** The STT engines the `aulo-speech-sherpa` helper runs, each on one catalog model (`aulo models pull <catalogId>`). */
enum class SherpaStt(
    val engineId: EngineId,
    /** The model name on the helper's command line. */
    val helperModel: String,
    val catalogId: String,
    val info: EngineInfo,
) {
    /** Parakeet TDT v3: no partials, a final per utterance; the languages of its model card. */
    PARAKEET(
        EngineId("sherpa-parakeet"),
        "parakeet",
        "parakeet-tdt-0.6b-v3-int8",
        info(
            "sherpa-parakeet",
            "Parakeet TDT v3 (sherpa-onnx)",
            streaming = false,
            LanguageSupport.Listed(
                listOf(
                    "en",
                    "es",
                    "fr",
                    "de",
                    "bg",
                    "hr",
                    "cs",
                    "da",
                    "nl",
                    "et",
                    "fi",
                    "el",
                    "hu",
                    "it",
                    "lv",
                    "lt",
                    "mt",
                    "pl",
                    "pt",
                    "ro",
                    "sk",
                    "sl",
                    "sv",
                    "ru",
                    "uk",
                ),
            ),
        ),
    ),

    /** Moonshine base: English only, partials from re-decoding the growing utterance. */
    MOONSHINE(
        EngineId("sherpa-moonshine"),
        "moonshine",
        "moonshine-base-en-quantized",
        info("sherpa-moonshine", "Moonshine base English (sherpa-onnx)", streaming = true, LanguageSupport.Listed(listOf("en"))),
    ),
}

private fun info(
    id: String,
    name: String,
    streaming: Boolean,
    languages: LanguageSupport,
) = EngineInfo(EngineId(id), EngineKind.STT, name, Capabilities(streaming, languages, offline = true, needsNetwork = false))

/**
 * Registers every [SherpaStt] engine. Each build starts its own [helper] process on the model directory [modelDir]
 * gives for the catalog id; a model that is not installed or a helper that cannot start is unavailable, so the
 * registry falls back to the next engine.
 */
fun SttRegistry.registerSherpa(
    helper: String,
    modelDir: (catalogId: String) -> String?,
    options: HelperOptions = HelperOptions(),
) {
    for (engine in SherpaStt.entries) {
        register(engine.engineId) { spec ->
            if (spec.model != null && spec.model != engine.catalogId) {
                throw SpeechException.Unsupported("model ${spec.model}")
            }
            val dir =
                modelDir(engine.catalogId)
                    ?: throw SpeechException.Unavailable("model ${engine.catalogId} is not installed")
            val command = Command(helper, listOf("stt", engine.helperModel, dir, SHERPA_THREADS.toString()))
            HelperStt(engine.info, ProcessLink.spawn(command), options)
        }
    }
}

/**
 * The helper installed beside this executable, with the same extension (`.exe` on Windows, `.kexe` for a build
 * that has not been packaged); null when the OS will not say where this executable is.
 */
fun sherpaHelperBesideThisExe(): String? {
    val exe = currentExe() ?: return null
    val cut = maxOf(exe.lastIndexOf('/'), exe.lastIndexOf('\\'))
    val name = exe.substring(cut + 1)
    val ext = name.substringAfterLast('.', "").let { if (it.isEmpty()) "" else ".$it" }
    return exe.substring(0, cut + 1) + "aulo-speech-sherpa" + ext
}
