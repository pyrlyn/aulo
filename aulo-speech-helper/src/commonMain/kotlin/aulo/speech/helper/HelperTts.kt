@file:OptIn(ExperimentalAtomicApi::class)

package aulo.speech.helper

import aulo.speech.AudioFormat
import aulo.speech.Capabilities
import aulo.speech.EngineId
import aulo.speech.EngineInfo
import aulo.speech.EngineKind
import aulo.speech.LanguageSupport
import aulo.speech.MAX_LANGUAGES
import aulo.speech.SpeechException
import aulo.speech.SpeechRate
import aulo.speech.TtsEngine
import aulo.speech.TtsPoll
import aulo.speech.TtsRequest
import aulo.speech.Voice
import aulo.speech.sameLanguage
import aulo.types.AuloEvent
import aulo.voice.AudioRing
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull
import okio.BufferedSink
import okio.BufferedSource
import okio.IOException
import kotlin.concurrent.atomics.AtomicInt
import kotlin.concurrent.atomics.AtomicReference
import kotlin.concurrent.atomics.ExperimentalAtomicApi
import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TimeMark
import kotlin.time.TimeSource

/** Most voices a TTS helper may offer; macOS 27 installs about 190. */
const val MAX_HELPER_VOICES = 1024

data class HelperTtsOptions(
    /** Audio a reply keeps ahead of the speaker; the helper is held back once it is full. */
    val bufferMs: Int = 2_000,
    /** Texts queued for the helper before the engine counts it as stuck. */
    val queueTexts: Int = 256,
    /** How long the helper may take to list its voices. */
    val startTimeout: Duration = 10.seconds,
    /**
     * How long a finished reply may go without new audio. A helper past it is hung in native
     * code, so it is killed and the engine fails over.
     */
    val stallTimeout: Duration = 30.seconds,
    val time: TimeSource = TimeSource.Monotonic,
) {
    init {
        require(bufferMs > 0) { "bufferMs must be positive" }
        require(queueTexts > 0) { "queueTexts must be positive" }
    }
}

/** Speak, cancel and finish of a few replies at most can wait behind a full text queue. */
private const val CONTROL_SLOTS = 16

private const val MS_PER_SECOND = 1_000

/**
 * A [TtsEngine] whose synthesizer runs in a helper process (`aulo-speech-system` for the macOS
 * voices). Native speech code can crash or hang; in its own process that is an engine failure
 * the registry falls back from.
 *
 * The helper streams audio at the voice's own rate, which [begin] returns. A reader on an I/O
 * thread moves it into the reply's [AudioRing], and [poll] takes it from there without
 * blocking. A full ring stops the reader, so the helper's pipe fills and the helper waits: audio
 * is made no faster than it is played. A cancelled reply's ring is closed and whatever the helper
 * still sends for it is dropped.
 */
class HelperTts private constructor(
    override val info: EngineInfo,
    private val offered: List<HelperVoice>,
    link: HelperLink,
    private val options: HelperTtsOptions,
) : TtsEngine {
    private sealed interface Out {
        val seq: Int
    }

    private class SpeakOut(
        override val seq: Int,
        val voice: String,
        val rate: SpeechRate,
    ) : Out

    private class TextOut(
        override val seq: Int,
        val text: String,
    ) : Out

    private class FinishOut(
        override val seq: Int,
    ) : Out

    private class CancelOut(
        override val seq: Int,
    ) : Out

    private class Reply(
        val seq: Int,
        val ring: AudioRing,
    )

    private enum class Phase { IDLE, OPEN, FINISHED, DONE }

    override val voices: List<Voice> = offered.map { it.voice }

    /** The reply audio belongs to; the reader drops everything else. */
    private val current = AtomicInt(0)
    private val reply = AtomicReference<Reply?>(null)

    /** The last reply whose audio the helper finished sending. */
    private val ended = AtomicInt(0)

    // The helper listed its voices before the engine was built, so any failure is a failed engine.
    private val pipe = HelperPipe<Out>(link, options.queueTexts + CONTROL_SLOTS, { true }, ::write)

    // The caller's thread only.
    private var seq = 0
    private var phase = Phase.IDLE
    private var ring: AudioRing? = null
    private var progressAt: TimeMark? = null

    init {
        pipe.start(::readLoop)
    }

    override fun begin(request: TtsRequest): AudioFormat {
        pipe.checkHealthy()
        val voice = pick(request)
        if (phase == Phase.OPEN || phase == Phase.FINISHED) cancel()
        seq++
        current.store(seq)
        val ring = AudioRing(maxOf(1, (voice.sampleRateHz.toLong() * options.bufferMs / MS_PER_SECOND).toInt()))
        this.ring = ring
        reply.store(Reply(seq, ring))
        phase = Phase.OPEN
        progressAt = null
        pipe.send(SpeakOut(seq, voice.voice.id, request.rate))
        return AudioFormat(voice.sampleRateHz, 1)
    }

    override fun pushText(text: String) {
        pipe.checkHealthy()
        if (phase != Phase.OPEN) throw SpeechException.OutOfOrder("text outside a reply")
        for (chunk in text.utf8Chunks(AuloEvent.Transcript.MAX_TEXT_BYTES)) pipe.send(TextOut(seq, chunk))
    }

    override fun finish() {
        pipe.checkHealthy()
        if (phase != Phase.OPEN) throw SpeechException.OutOfOrder("finish outside a reply")
        phase = Phase.FINISHED
        progressAt = options.time.markNow()
        pipe.send(FinishOut(seq))
    }

    override fun poll(out: FloatArray): TtsPoll {
        pipe.checkHealthy()
        val ring = ring
        if (ring == null || (phase != Phase.OPEN && phase != Phase.FINISHED)) return TtsPoll.DONE
        // Read after checking the end: the reader stores the end only after the reply's last audio.
        val ended = ended.load() == seq
        val n = ring.read(out)
        if (n > 0) {
            if (phase == Phase.FINISHED) progressAt = options.time.markNow()
            return TtsPoll.audio(n)
        }
        if (ended && phase == Phase.FINISHED) {
            phase = Phase.DONE
            return TtsPoll.DONE
        }
        val since = progressAt
        if (phase == Phase.FINISHED && since != null && since.elapsedNow() > options.stallTimeout) {
            pipe.fail(SpeechException.Failed("no audio within ${options.stallTimeout}"))
            close()
            pipe.checkHealthy()
        }
        return TtsPoll.PENDING
    }

    override fun cancel() {
        if (phase == Phase.OPEN || phase == Phase.FINISHED) {
            // Moving `current` on and closing the ring lets go of a reader waiting for room and
            // drops late audio at once; the helper skips the reply's text when the cancel reaches it.
            current.store(-seq)
            ring?.close()
            pipe.send(CancelOut(seq))
        }
        phase = Phase.IDLE
        ring = null
        progressAt = null
    }

    override fun close() {
        ring?.close()
        pipe.close()
    }

    private fun pick(request: TtsRequest): HelperVoice {
        request.voice?.let { id ->
            return offered.firstOrNull { it.voice.id == id } ?: throw SpeechException.Unsupported("voice $id")
        }
        val language = request.language ?: return offered.firstOrNull { it.systemDefault } ?: offered.first()
        val exact = offered.filter { it.voice.language.equals(language, ignoreCase = true) }
        val same = offered.filter { v -> v.voice.language?.let { sameLanguage(it, language) } == true }
        return exact.firstOrNull { it.languageDefault }
            ?: exact.firstOrNull()
            ?: same.firstOrNull { it.systemDefault }
            ?: same.firstOrNull { it.languageDefault }
            ?: same.firstOrNull()
            ?: throw SpeechException.Unsupported("language $language")
    }

    private suspend fun readLoop(input: BufferedSource) {
        while (true) {
            val frame = input.readFrame() ?: return
            when (frame.type) {
                FrameType.AUDIO -> {
                    val reply = reply.load()
                    if (reply != null && reply.seq == frame.seq && current.load() == frame.seq) {
                        // Waits while the ring is full; a cancel closes the ring and lets it go.
                        reply.ring.write(frame.samples())
                    }
                }

                FrameType.END -> {
                    if (frame.seq == current.load()) ended.store(frame.seq)
                }

                FrameType.ERROR -> {
                    // An error of a reply already cancelled no longer matters.
                    if (frame.seq == 0 || frame.seq == current.load()) pipe.fail(SpeechException.Failed(frame.text))
                }

                else -> {
                    throw WireException("${frame.type.name} is not a TTS helper reply")
                }
            }
        }
    }

    private fun write(
        sink: BufferedSink,
        message: Out,
    ) {
        when (message) {
            is SpeakOut -> {
                sink.writeSpeak(message.seq, message.voice, message.rate)
            }

            is TextOut -> {
                // Text of a reply cancelled while it waited is not worth sending.
                if (message.seq == current.load()) sink.writeText(FrameType.TEXT, message.seq, message.text)
            }

            is FinishOut -> {
                sink.writeFrame(FrameType.FINISH, message.seq)
            }

            is CancelOut -> {
                sink.writeFrame(FrameType.CANCEL, message.seq)
            }
        }
    }

    companion object {
        /**
         * Starts the engine on a running helper: waits up to [HelperTtsOptions.startTimeout] for
         * its voice list. A helper that lists nothing, breaks the protocol or exits first is
         * [SpeechException.Unavailable], so the registry falls back; the link is closed then.
         */
        fun start(
            id: EngineId,
            name: String,
            link: HelperLink,
            options: HelperTtsOptions = HelperTtsOptions(),
        ): HelperTts {
            val offered =
                try {
                    listVoices(link, options.startTimeout)
                } catch (e: SpeechException) {
                    link.close()
                    throw e
                }
            val languages = languagesOf(offered.mapNotNull { it.voice.language })
            val info = EngineInfo(id, EngineKind.TTS, name, Capabilities(streaming = true, languages, offline = true, needsNetwork = false))
            return HelperTts(info, offered, link, options)
        }
    }
}

private fun languagesOf(tags: List<String>): LanguageSupport {
    if (tags.isEmpty()) return LanguageSupport.Multilingual
    val distinct = tags.distinctBy { it.lowercase() }
    // Matching compares primary subtags, so they say the same when the full tags are too many.
    val listed = if (distinct.size <= MAX_LANGUAGES) distinct else distinct.map { it.substringBefore('-') }.distinctBy { it.lowercase() }
    return LanguageSupport.Listed(listed.take(MAX_LANGUAGES))
}

/** Reads the helper's voices up to READY on an I/O thread, so a silent helper cannot hold the caller past [timeout]. */
private fun listVoices(
    link: HelperLink,
    timeout: Duration,
): List<HelperVoice> {
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    try {
        val listing = scope.async { readVoices(link) }
        val voices =
            try {
                runBlocking { withTimeoutOrNull(timeout) { listing.await() } }
            } catch (e: WireException) {
                throw SpeechException.Unavailable("helper broke the protocol: ${e.message}")
            } catch (e: IOException) {
                throw SpeechException.Unavailable("helper pipe failed (${e.message})")
            }
        voices ?: throw SpeechException.Unavailable("helper listed no voices within $timeout")
        if (voices.isEmpty()) throw SpeechException.Unavailable("helper has no voices")
        return voices
    } finally {
        // A read still blocked ends when the caller closes the link.
        scope.cancel()
    }
}

private fun readVoices(link: HelperLink): List<HelperVoice> {
    val voices = mutableListOf<HelperVoice>()
    val ids = HashSet<String>()
    while (true) {
        val frame =
            link.input.readFrame()
                ?: throw SpeechException.Unavailable("helper exited" + (link.exitDetail()?.let { ": $it" } ?: ""))
        when (frame.type) {
            FrameType.VOICE -> {
                if (voices.size == MAX_HELPER_VOICES) throw WireException("more than $MAX_HELPER_VOICES voices")
                val voice = frame.voice()
                // The first of two voices with one id is the one a request for that id gets.
                if (ids.add(voice.voice.id)) voices += voice
            }

            FrameType.READY -> {
                return voices
            }

            FrameType.ERROR -> {
                throw SpeechException.Unavailable(frame.text)
            }

            else -> {
                throw WireException("${frame.type.name} before READY")
            }
        }
    }
}
