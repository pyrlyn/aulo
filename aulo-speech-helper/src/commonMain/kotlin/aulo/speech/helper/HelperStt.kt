@file:OptIn(ExperimentalAtomicApi::class)

package aulo.speech.helper

import aulo.speech.AudioFormat
import aulo.speech.AudioFrame
import aulo.speech.EngineInfo
import aulo.speech.EngineKind
import aulo.speech.SpeechException
import aulo.speech.SttEngine
import aulo.speech.SttPoll
import aulo.speech.Transcript
import aulo.types.TranscriptKind
import aulo.types.TurnId
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.launch
import okio.BufferedSink
import okio.BufferedSource
import okio.IOException
import kotlin.concurrent.atomics.AtomicBoolean
import kotlin.concurrent.atomics.AtomicInt
import kotlin.concurrent.atomics.AtomicReference
import kotlin.concurrent.atomics.ExperimentalAtomicApi
import kotlin.concurrent.atomics.decrementAndFetch
import kotlin.concurrent.atomics.incrementAndFetch
import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TimeMark
import kotlin.time.TimeSource

/** A running helper process: its stdout, its stdin, and how it ended. */
interface HelperLink : AutoCloseable {
    /** The helper's stdout. */
    val input: BufferedSource

    /** The helper's stdin. */
    val output: BufferedSink

    /** Why the helper ended (exit code or signal), once it has; null while it runs. */
    fun exitDetail(): String?

    /** Stops the helper if it still runs and releases the pipes. */
    override fun close()
}

data class HelperOptions(
    /** Audio frames queued for the helper before `push` drops them: 5 s of 20 ms frames. */
    val queueFrames: Int = 250,
    /**
     * How long the final transcript may take after `finish`. A helper past it is hung in native
     * code, so it is killed and the engine fails over.
     */
    val finalTimeout: Duration = 30.seconds,
    val time: TimeSource = TimeSource.Monotonic,
) {
    init {
        require(queueFrames > 0) { "queueFrames must be positive" }
    }
}

/** Where closed engines stop their helpers, off the caller's thread. */
private val releaser = CoroutineScope(SupervisorJob() + Dispatchers.IO)

/** Begin, cancel and finish of a few utterances at most can wait behind a full audio queue. */
private const val CONTROL_SLOTS = 16

/**
 * An [SttEngine] whose recognizer runs in a helper process (`aulo-speech-sherpa`). The native
 * code can crash or hang on a bad model or input; in its own process that is an engine failure
 * the registry falls back from, not the end of aulod.
 *
 * `push` and `poll` never block: a writer and a reader run on I/O threads behind a bounded
 * queue and atomic slots. Utterances are numbered, so a result for a cancelled utterance is
 * dropped. Anything wrong with the helper (exit, a frame that breaks the protocol, an error
 * reply, no final in time) fails the engine with a [SpeechException] that falls back.
 */
class HelperStt(
    override val info: EngineInfo,
    private val link: HelperLink,
    private val options: HelperOptions = HelperOptions(),
) : SttEngine {
    init {
        require(info.kind == EngineKind.STT) { "not an STT engine: ${info.kind}" }
    }

    private class Result(
        val seq: Int,
        val final: Boolean,
        val text: String,
    )

    private sealed interface Out {
        val seq: Int
    }

    private class Begin(
        override val seq: Int,
        val language: String?,
    ) : Out

    private class Audio(
        override val seq: Int,
        val samples: FloatArray,
    ) : Out

    private class Finish(
        override val seq: Int,
    ) : Out

    private class Cancel(
        override val seq: Int,
    ) : Out

    private enum class Phase { IDLE, OPEN, FINISHED, DONE }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val queue = Channel<Out>(options.queueFrames + CONTROL_SLOTS)
    private val queuedAudio = AtomicInt(0)

    /** The utterance results belong to; the reader drops everything else. */
    private val current = AtomicInt(0)
    private val latest = AtomicReference<Result?>(null)
    private val failure = AtomicReference<SpeechException?>(null)
    private val ready = AtomicBoolean(false)
    private val closed = AtomicBoolean(false)

    // The caller's thread only.
    private var seq = 0
    private var phase = Phase.IDLE
    private var turn: TurnId? = null
    private var language: String? = null
    private var delivered: Result? = null
    private var finishedAt: TimeMark? = null

    init {
        scope.launch { readLoop() }
        scope.launch { writeLoop() }
    }

    /** True once the helper loaded its model. */
    val isReady: Boolean get() = ready.load()

    override fun begin(
        turnId: TurnId,
        language: String?,
    ) {
        checkHealthy()
        if (language != null && !info.capabilities.languages.supports(language)) {
            throw SpeechException.Unsupported("language $language")
        }
        if (phase == Phase.OPEN || phase == Phase.FINISHED) cancel()
        seq++
        current.store(seq)
        turn = turnId
        this.language = language
        delivered = null
        finishedAt = null
        phase = Phase.OPEN
        send(Begin(seq, language))
    }

    override fun push(frame: AudioFrame): Int {
        checkHealthy()
        if (phase != Phase.OPEN) throw SpeechException.OutOfOrder("push outside an utterance")
        if (frame.format != AudioFormat.PIPELINE) throw SpeechException.Unsupported("audio format ${frame.format}")
        if (frame.size == 0) return 0
        if (queuedAudio.incrementAndFetch() > options.queueFrames) {
            queuedAudio.decrementAndFetch()
            return frame.length
        }
        val samples = FloatArray(frame.size)
        frame.copyInto(samples)
        if (queue.trySend(Audio(seq, samples)).isFailure) {
            queuedAudio.decrementAndFetch()
            return frame.length
        }
        return 0
    }

    override fun finish() {
        checkHealthy()
        if (phase != Phase.OPEN) throw SpeechException.OutOfOrder("finish outside an utterance")
        phase = Phase.FINISHED
        finishedAt = options.time.markNow()
        send(Finish(seq))
    }

    override fun poll(out: Transcript): SttPoll {
        checkHealthy()
        val turn = turn ?: return SttPoll.PENDING
        if (phase == Phase.DONE) return SttPoll.DONE
        val result = latest.load()
        if (result != null && result.seq == seq && result !== delivered) {
            // A partial that lands after finish is stale: only the final counts then.
            if (result.final || phase == Phase.OPEN) {
                delivered = result
                if (result.final) phase = Phase.DONE
                out.set(turn, if (result.final) TranscriptKind.FINAL else TranscriptKind.PARTIAL, result.text, language)
                return SttPoll.UPDATED
            }
        }
        val since = finishedAt
        if (phase == Phase.FINISHED && since != null && since.elapsedNow() > options.finalTimeout) {
            fail(SpeechException.Failed("no transcript within ${options.finalTimeout}"))
            close()
            checkHealthy()
        }
        return SttPoll.PENDING
    }

    override fun cancel() {
        if (phase == Phase.OPEN || phase == Phase.FINISHED) {
            // Moving `current` on first makes the reader drop a late result at once; the helper
            // drops its audio when the cancel reaches it.
            current.store(-seq)
            send(Cancel(seq))
        }
        turn = null
        phase = Phase.IDLE
        delivered = null
        finishedAt = null
    }

    override fun close() {
        if (!closed.compareAndSet(expectedValue = false, newValue = true)) return
        queue.close()
        scope.cancel()
        // Stopping a process waits for it to be reaped, and close may come from poll, which never waits.
        releaser.launch { link.close() }
    }

    private fun send(message: Out) {
        if (queue.trySend(message).isFailure) {
            fail(SpeechException.Failed("helper is not reading its input"))
            close()
            checkHealthy()
        }
    }

    private fun checkHealthy() {
        failure.load()?.let { throw it }
        if (closed.load()) throw SpeechException.Unavailable("engine is closed")
    }

    private fun fail(error: SpeechException) {
        failure.compareAndSet(null, error)
    }

    private fun readLoop() {
        try {
            while (true) {
                val frame = link.input.readFrame() ?: break
                when (frame.type) {
                    FrameType.READY -> {
                        ready.store(true)
                    }

                    FrameType.PARTIAL, FrameType.FINAL -> {
                        if (frame.seq == current.load()) latest.store(Result(frame.seq, frame.type == FrameType.FINAL, frame.text))
                    }

                    FrameType.ERROR -> {
                        // Before READY the helper could not load its model; after it, decoding broke.
                        val error =
                            if (frame.seq == 0) SpeechException.Unavailable(frame.text) else SpeechException.Failed(frame.text)
                        fail(error)
                    }

                    else -> {
                        throw WireException("${frame.type.name} is not a helper reply")
                    }
                }
            }
            fail(exited("exited"))
        } catch (e: WireException) {
            fail(SpeechException.Failed("helper broke the protocol: ${e.message}"))
        } catch (e: IOException) {
            fail(exited("pipe failed (${e.message})"))
        }
        // The helper is gone or untrusted from here on; stop it and free its pipes.
        close()
    }

    private fun exited(how: String): SpeechException {
        val detail = "helper $how" + (link.exitDetail()?.let { ": $it" } ?: "")
        return if (ready.load()) SpeechException.Failed(detail) else SpeechException.Unavailable(detail)
    }

    private suspend fun writeLoop() {
        try {
            for (first in queue) {
                // Write what is queued, then flush once: one pipe write per burst, not per frame.
                var message: Out? = first
                while (message != null) {
                    write(message)
                    message = queue.tryReceive().getOrNull()
                }
                link.output.flush()
            }
        } catch (e: IOException) {
            fail(exited("stopped reading (${e.message})"))
            close()
        }
    }

    private fun write(message: Out) {
        val sink = link.output
        when (message) {
            is Audio -> {
                queuedAudio.decrementAndFetch()
                // Audio of an utterance cancelled while it waited is not worth sending.
                if (message.seq != current.load()) return
                val samples = message.samples
                for (from in samples.indices step MAX_AUDIO_SAMPLES) {
                    sink.writeAudio(message.seq, samples, from, minOf(MAX_AUDIO_SAMPLES, samples.size - from))
                }
            }

            is Begin -> {
                sink.writeFrame(FrameType.BEGIN, message.seq, message.language.orEmpty().encodeToByteArray())
            }

            is Finish -> {
                sink.writeFrame(FrameType.FINISH, message.seq)
            }

            is Cancel -> {
                sink.writeFrame(FrameType.CANCEL, message.seq)
            }
        }
    }
}
