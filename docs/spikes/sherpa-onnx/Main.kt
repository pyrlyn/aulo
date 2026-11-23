@file:OptIn(ExperimentalForeignApi::class)

package spike

import kotlinx.cinterop.CPointer
import kotlinx.cinterop.CVariable
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.FloatVar
import kotlinx.cinterop.MemScope
import kotlinx.cinterop.alloc
import kotlinx.cinterop.convert
import kotlinx.cinterop.cstr
import kotlinx.cinterop.memScoped
import kotlinx.cinterop.pointed
import kotlinx.cinterop.ptr
import kotlinx.cinterop.sizeOf
import kotlinx.cinterop.staticCFunction
import kotlinx.cinterop.get
import kotlinx.cinterop.toKString
import kotlinx.cinterop.usePinned
import kotlinx.cinterop.addressOf
import platform.posix.memset
import sherpa.SherpaOnnxAcceptWaveformOffline
import sherpa.SherpaOnnxCreateKeywordSpotter
import sherpa.SherpaOnnxCreateKeywordStream
import sherpa.SherpaOnnxCreateOfflineRecognizer
import sherpa.SherpaOnnxCreateOfflineStream
import sherpa.SherpaOnnxCreateOfflineTts
import sherpa.SherpaOnnxCreateVoiceActivityDetector
import sherpa.SherpaOnnxDecodeKeywordStream
import sherpa.SherpaOnnxDecodeOfflineStream
import sherpa.SherpaOnnxDestroyKeywordResult
import sherpa.SherpaOnnxDestroyOfflineRecognizerResult
import sherpa.SherpaOnnxDestroyOfflineStream
import sherpa.SherpaOnnxDestroyOfflineTtsGeneratedAudio
import sherpa.SherpaOnnxDestroySpeechSegment
import sherpa.SherpaOnnxFreeWave
import sherpa.SherpaOnnxGenerationConfig
import sherpa.SherpaOnnxGetKeywordResult
import sherpa.SherpaOnnxGetOfflineStreamResult
import sherpa.SherpaOnnxGetVersionStr
import sherpa.SherpaOnnxIsKeywordStreamReady
import sherpa.SherpaOnnxKeywordSpotterConfig
import sherpa.SherpaOnnxOfflineRecognizerConfig
import sherpa.SherpaOnnxOfflineTtsConfig
import sherpa.SherpaOnnxOfflineTtsGenerateWithConfig
import sherpa.SherpaOnnxOnlineStreamAcceptWaveform
import sherpa.SherpaOnnxOnlineStreamInputFinished
import sherpa.SherpaOnnxReadWave
import sherpa.SherpaOnnxResetKeywordStream
import sherpa.SherpaOnnxVadModelConfig
import sherpa.SherpaOnnxVoiceActivityDetectorAcceptWaveform
import sherpa.SherpaOnnxVoiceActivityDetectorEmpty
import sherpa.SherpaOnnxVoiceActivityDetectorFlush
import sherpa.SherpaOnnxVoiceActivityDetectorFront
import sherpa.SherpaOnnxVoiceActivityDetectorPop
import sherpa.SherpaOnnxWriteWave
import kotlin.time.TimeMark
import kotlin.time.TimeSource

// Throwaway harness for spike T0.5: one subcommand per engine, timings on stderr, results on stdout.

private inline fun <reified T : CVariable> MemScope.zeroed(): T {
    val v = alloc<T>()
    memset(v.ptr, 0, sizeOf<T>().convert())
    return v
}

private fun MemScope.c(s: String) = s.cstr.getPointer(this)

private fun secs(mark: TimeMark) = mark.elapsedNow().inWholeMicroseconds / 1e6

private fun err(s: String) = platform.posix.fprintf(platform.posix.stderr, "%s\n", s)

private class Wave(
    val samples: FloatArray,
    val rate: Int,
)

private fun readWave(path: String): Wave {
    val w = SherpaOnnxReadWave(path) ?: error("cannot read $path")
    val p = w.pointed
    val out = Wave(FloatArray(p.num_samples) { p.samples!![it] }, p.sample_rate)
    SherpaOnnxFreeWave(w)
    return out
}

fun main(args: Array<String>) {
    when (args.firstOrNull()) {
        "version" -> println(SherpaOnnxGetVersionStr()?.toKString())
        "stt" -> stt(args[1], args[2], args[3].toInt(), args.drop(4))
        "vad" -> vad(args[1], args.drop(2))
        "kws" -> kws(args[1], args[2], args.drop(3))
        "tts" -> tts(args[1], args[2].toInt(), args[3].toInt(), args[4], args[5])
        else -> error("usage: version | stt <parakeet|moonshine> <dir> <threads> <wav|@list>... | vad <model> <wav>... | kws <dir> <keywords> <wav>... | tts <dir> <threads> <sid> <text> <out>")
    }
}

private fun expand(files: List<String>): List<String> =
    files.flatMap { f ->
        if (f.startsWith("@")) {
            val text = fs.readText(kotlinx.io.files.Path(f.drop(1)))
            text.lines().filter { it.isNotBlank() }
        } else {
            listOf(f)
        }
    }

private fun stt(
    kind: String,
    dir: String,
    threads: Int,
    files: List<String>,
) = memScoped {
    val cfg = zeroed<SherpaOnnxOfflineRecognizerConfig>()
    cfg.feat_config.sample_rate = 16000
    cfg.feat_config.feature_dim = 80
    cfg.decoding_method = c("greedy_search")
    val m = cfg.model_config
    m.num_threads = threads
    m.provider = c("cpu")
    m.tokens = c("$dir/tokens.txt")
    when (kind) {
        "parakeet" -> {
            m.transducer.encoder = c("$dir/encoder.int8.onnx")
            m.transducer.decoder = c("$dir/decoder.int8.onnx")
            m.transducer.joiner = c("$dir/joiner.int8.onnx")
            m.model_type = c("nemo_transducer")
        }
        "moonshine" -> {
            m.moonshine.encoder = c("$dir/encoder_model.ort")
            m.moonshine.merged_decoder = c("$dir/decoder_model_merged.ort")
        }
        else -> error("kind")
    }
    val load = TimeSource.Monotonic.markNow()
    val rec = SherpaOnnxCreateOfflineRecognizer(cfg.ptr) ?: error("recognizer: NULL")
    err("load ${secs(load)}")
    var audio = 0.0
    var spent = 0.0
    for (f in expand(files)) {
        val w = readWave(f)
        val t = TimeSource.Monotonic.markNow()
        val s = SherpaOnnxCreateOfflineStream(rec)
        w.samples.usePinnedFloats { SherpaOnnxAcceptWaveformOffline(s, w.rate, it, w.samples.size) }
        SherpaOnnxDecodeOfflineStream(rec, s)
        val r = SherpaOnnxGetOfflineStreamResult(s)
        val text = r?.pointed?.text?.toKString().orEmpty()
        SherpaOnnxDestroyOfflineRecognizerResult(r)
        SherpaOnnxDestroyOfflineStream(s)
        val took = secs(t)
        val len = w.samples.size.toDouble() / w.rate
        audio += len
        spent += took
        err("$f audio=$len s decode=$took s rtf=${took / len}")
        println("${f.substringAfterLast('/')}\t$text")
    }
    err("total audio=$audio s decode=$spent s rtf=${spent / audio}")
}

private inline fun <R> FloatArray.usePinnedFloats(block: (CPointer<FloatVar>) -> R): R = usePinned { block(it.addressOf(0)) }

private fun vad(
    model: String,
    files: List<String>,
) = memScoped {
    val cfg = zeroed<SherpaOnnxVadModelConfig>()
    cfg.silero_vad.model = c(model)
    cfg.silero_vad.threshold = 0.5f
    cfg.silero_vad.min_silence_duration = 0.25f
    cfg.silero_vad.min_speech_duration = 0.25f
    cfg.silero_vad.max_speech_duration = 20f
    cfg.silero_vad.window_size = 512
    cfg.sample_rate = 16000
    cfg.num_threads = 1
    cfg.provider = c("cpu")
    val load = TimeSource.Monotonic.markNow()
    val vad = SherpaOnnxCreateVoiceActivityDetector(cfg.ptr, 60f) ?: error("vad: NULL")
    err("load ${secs(load)}")
    for (f in files) {
        val w = readWave(f)
        val t = TimeSource.Monotonic.markNow()
        var i = 0
        val segments = mutableListOf<String>()
        fun drain() {
            while (SherpaOnnxVoiceActivityDetectorEmpty(vad) == 0) {
                val seg = SherpaOnnxVoiceActivityDetectorFront(vad)!!.pointed
                segments += "%.2f-%.2f".format(seg.start.toDouble() / 16000, (seg.start + seg.n).toDouble() / 16000)
                SherpaOnnxDestroySpeechSegment(seg.ptr)
                SherpaOnnxVoiceActivityDetectorPop(vad)
            }
        }
        w.samples.usePinnedFloats { p ->
            while (i + 512 <= w.samples.size) {
                SherpaOnnxVoiceActivityDetectorAcceptWaveform(vad, kotlinx.cinterop.interpretCPointer(p.rawValue + i * 4L), 512)
                drain()
                i += 512
            }
        }
        SherpaOnnxVoiceActivityDetectorFlush(vad)
        drain()
        val took = secs(t)
        val len = w.samples.size.toDouble() / w.rate
        err("$f audio=$len s vad=$took s rtf=${took / len}")
        println("${f.substringAfterLast('/')}\t$segments")
    }
}

private fun String.format(vararg a: Double): String {
    var i = 0
    return Regex("%\\.2f").replace(this) { ((a[i++] * 100).toLong() / 100.0).toString() }
}

private fun kws(
    dir: String,
    keywords: String,
    files: List<String>,
) = memScoped {
    val cfg = zeroed<SherpaOnnxKeywordSpotterConfig>()
    cfg.feat_config.sample_rate = 16000
    cfg.feat_config.feature_dim = 80
    val m = cfg.model_config
    m.transducer.encoder = c("$dir/encoder-epoch-12-avg-2-chunk-16-left-64.onnx")
    m.transducer.decoder = c("$dir/decoder-epoch-12-avg-2-chunk-16-left-64.onnx")
    m.transducer.joiner = c("$dir/joiner-epoch-12-avg-2-chunk-16-left-64.int8.onnx")
    m.tokens = c("$dir/tokens.txt")
    m.num_threads = 1
    m.provider = c("cpu")
    cfg.max_active_paths = 4
    cfg.num_trailing_blanks = 1
    cfg.keywords_score = 1f
    cfg.keywords_threshold = 0.25f
    cfg.keywords_file = c(keywords)
    val load = TimeSource.Monotonic.markNow()
    val kws = SherpaOnnxCreateKeywordSpotter(cfg.ptr) ?: error("kws: NULL")
    err("load ${secs(load)}")
    for (f in files) {
        val w = readWave(f)
        val t = TimeSource.Monotonic.markNow()
        val s = SherpaOnnxCreateKeywordStream(kws)
        w.samples.usePinnedFloats { SherpaOnnxOnlineStreamAcceptWaveform(s, w.rate, it, w.samples.size) }
        val tail = FloatArray(8000)
        tail.usePinnedFloats { SherpaOnnxOnlineStreamAcceptWaveform(s, w.rate, it, tail.size) }
        SherpaOnnxOnlineStreamInputFinished(s)
        val hits = mutableListOf<String>()
        while (SherpaOnnxIsKeywordStreamReady(kws, s) != 0) {
            SherpaOnnxDecodeKeywordStream(kws, s)
            val r = SherpaOnnxGetKeywordResult(kws, s)
            val k = r?.pointed?.keyword?.toKString().orEmpty()
            if (k.isNotEmpty()) {
                hits += k
                SherpaOnnxResetKeywordStream(kws, s)
            }
            SherpaOnnxDestroyKeywordResult(r)
        }
        val took = secs(t)
        val len = w.samples.size.toDouble() / w.rate
        err("$f audio=$len s kws=$took s rtf=${took / len}")
        println("${f.substringAfterLast('/')}\t$hits")
    }
}

private var started: TimeMark? = null
private var firstAudio = -1.0

private fun tts(
    dir: String,
    threads: Int,
    sid: Int,
    text: String,
    out: String,
) = memScoped {
    val cfg = zeroed<SherpaOnnxOfflineTtsConfig>()
    val k = cfg.model.kokoro
    k.model = c("$dir/model.onnx")
    k.voices = c("$dir/voices.bin")
    k.tokens = c("$dir/tokens.txt")
    k.lexicon = c("$dir/lexicon-us-en.txt")
    k.lang = c("en-us")
    k.length_scale = 1f
    cfg.model.num_threads = threads
    cfg.model.provider = c("cpu")
    cfg.max_num_sentences = 1
    cfg.silence_scale = 0.2f
    val load = TimeSource.Monotonic.markNow()
    val tts = SherpaOnnxCreateOfflineTts(cfg.ptr) ?: error("tts: NULL")
    err("load ${secs(load)}")
    val gen = zeroed<SherpaOnnxGenerationConfig>()
    gen.sid = sid
    gen.speed = 1f
    gen.silence_scale = 0.2f
    started = TimeSource.Monotonic.markNow()
    val cb =
        staticCFunction { _: CPointer<FloatVar>?, n: Int, _: Float, _: kotlinx.cinterop.COpaquePointer? ->
            if (firstAudio < 0 && n > 0) firstAudio = secs(started!!)
            1
        }
    val audio = SherpaOnnxOfflineTtsGenerateWithConfig(tts, text, gen.ptr, cb, null) ?: error("generate: NULL")
    val took = secs(started!!)
    val a = audio.pointed
    val len = a.n.toDouble() / a.sample_rate
    err("audio=$len s generate=$took s rtf=${took / len} first_audio=$firstAudio s rate=${a.sample_rate}")
    SherpaOnnxWriteWave(a.samples, a.n, a.sample_rate, out)
    SherpaOnnxDestroyOfflineTtsGeneratedAudio(audio)
}
