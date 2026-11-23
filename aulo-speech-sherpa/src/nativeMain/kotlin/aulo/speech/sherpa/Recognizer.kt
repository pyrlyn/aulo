@file:OptIn(ExperimentalForeignApi::class)

package aulo.speech.sherpa

import aulo.speech.PIPELINE_SAMPLE_RATE_HZ
import aulo.speech.helper.ServeOptions
import aulo.speech.helper.UtteranceDecoder
import cnames.structs.SherpaOnnxOfflineRecognizer
import kotlinx.cinterop.CPointer
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.MemScope
import kotlinx.cinterop.addressOf
import kotlinx.cinterop.alloc
import kotlinx.cinterop.convert
import kotlinx.cinterop.cstr
import kotlinx.cinterop.memScoped
import kotlinx.cinterop.pointed
import kotlinx.cinterop.ptr
import kotlinx.cinterop.sizeOf
import kotlinx.cinterop.toKString
import kotlinx.cinterop.usePinned
import kotlinx.io.files.Path
import kotlinx.io.files.SystemFileSystem
import platform.posix.memset
import sherpa.SherpaOnnxAcceptWaveformOffline
import sherpa.SherpaOnnxCreateOfflineRecognizer
import sherpa.SherpaOnnxCreateOfflineStream
import sherpa.SherpaOnnxDecodeOfflineStream
import sherpa.SherpaOnnxDestroyOfflineRecognizer
import sherpa.SherpaOnnxDestroyOfflineRecognizerResult
import sherpa.SherpaOnnxDestroyOfflineStream
import sherpa.SherpaOnnxGetOfflineStreamResult
import sherpa.SherpaOnnxOfflineRecognizerConfig

/** Most decoder threads the helper accepts; past 4 Parakeet barely gets faster (spike T0.5). */
const val MAX_THREADS = 16

private const val FEATURE_DIM = 80

/** Moonshine partials: a decode per half second of new audio. */
private const val MOONSHINE_PARTIAL_EVERY = PIPELINE_SAMPLE_RATE_HZ / 2

/** The offline STT models the helper runs, by the name aulod passes on its command line. */
enum class SttModel(
    val id: String,
    /** The files the model directory must hold; sherpa-onnx's own message for a missing one is poor. */
    val files: List<String>,
    val serve: ServeOptions,
) {
    /** Parakeet TDT v3: 25 languages, a final per VAD segment. */
    PARAKEET("parakeet", listOf("encoder.int8.onnx", "decoder.int8.onnx", "joiner.int8.onnx", "tokens.txt"), ServeOptions()),

    /** Moonshine base, English: fast enough to decode the growing utterance for partials. */
    MOONSHINE(
        "moonshine",
        listOf("encoder_model.ort", "decoder_model_merged.ort", "tokens.txt"),
        ServeOptions(partialEverySamples = MOONSHINE_PARTIAL_EVERY),
    ),
    ;

    companion object {
        fun of(id: String): SttModel? = entries.firstOrNull { it.id == id }
    }
}

/** sherpa-onnx refused to start; the message is for aulod's notice. */
class SherpaException(
    message: String,
) : Exception(message)

/**
 * sherpa-onnx's offline recognizer over one model directory. Not thread-safe: the helper decodes
 * one utterance at a time.
 */
class OfflineRecognizer(
    model: SttModel,
    dir: String,
    threads: Int,
) : UtteranceDecoder,
    AutoCloseable {
    private val handle: CPointer<SherpaOnnxOfflineRecognizer>

    init {
        require(threads in 1..MAX_THREADS) { "threads must be 1 to $MAX_THREADS" }
        val missing = model.files.filter { SystemFileSystem.metadataOrNull(Path(dir, it))?.isRegularFile != true }
        if (missing.isNotEmpty()) throw SherpaException("model files missing in $dir: ${missing.joinToString()}")
        handle =
            memScoped {
                val cfg = zeroed<SherpaOnnxOfflineRecognizerConfig>()
                cfg.feat_config.sample_rate = PIPELINE_SAMPLE_RATE_HZ
                cfg.feat_config.feature_dim = FEATURE_DIM
                cfg.decoding_method = c("greedy_search")
                val m = cfg.model_config
                m.num_threads = threads
                m.provider = c("cpu")
                m.tokens = c(Path(dir, "tokens.txt").toString())
                when (model) {
                    SttModel.PARAKEET -> {
                        m.transducer.encoder = c(Path(dir, "encoder.int8.onnx").toString())
                        m.transducer.decoder = c(Path(dir, "decoder.int8.onnx").toString())
                        m.transducer.joiner = c(Path(dir, "joiner.int8.onnx").toString())
                        m.model_type = c("nemo_transducer")
                    }

                    SttModel.MOONSHINE -> {
                        m.moonshine.encoder = c(Path(dir, "encoder_model.ort").toString())
                        m.moonshine.merged_decoder = c(Path(dir, "decoder_model_merged.ort").toString())
                    }
                }
                // NULL when the C++ side threw, for example on a corrupt model (sherpa-onnx #4044).
                SherpaOnnxCreateOfflineRecognizer(cfg.ptr) ?: throw SherpaException("sherpa-onnx could not load ${model.id} from $dir")
            }
    }

    override fun decode(
        samples: FloatArray,
        count: Int,
        language: String?,
    ): String {
        require(count in 1..samples.size) { "count $count outside 1..${samples.size}" }
        val stream = SherpaOnnxCreateOfflineStream(handle) ?: throw SherpaException("sherpa-onnx could not create a stream")
        try {
            samples.usePinned { SherpaOnnxAcceptWaveformOffline(stream, PIPELINE_SAMPLE_RATE_HZ, it.addressOf(0), count) }
            SherpaOnnxDecodeOfflineStream(handle, stream)
            val result = SherpaOnnxGetOfflineStreamResult(stream)
            try {
                return result
                    ?.pointed
                    ?.text
                    ?.toKString()
                    .orEmpty()
                    .trim()
            } finally {
                SherpaOnnxDestroyOfflineRecognizerResult(result)
            }
        } finally {
            SherpaOnnxDestroyOfflineStream(stream)
        }
    }

    override fun close() {
        SherpaOnnxDestroyOfflineRecognizer(handle)
    }
}

/** sherpa-onnx reads every field of its config structs, so unset ones must be zero, not garbage. */
private inline fun <reified T : kotlinx.cinterop.CVariable> MemScope.zeroed(): T {
    val v = alloc<T>()
    memset(v.ptr, 0, sizeOf<T>().convert())
    return v
}

private fun MemScope.c(s: String) = s.cstr.getPointer(this)
