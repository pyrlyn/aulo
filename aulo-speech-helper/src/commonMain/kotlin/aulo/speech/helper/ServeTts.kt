@file:OptIn(ExperimentalAtomicApi::class)

package aulo.speech.helper

import aulo.speech.SpeechRate
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import okio.BufferedSink
import okio.BufferedSource
import kotlin.concurrent.atomics.AtomicInt
import kotlin.concurrent.atomics.AtomicLong
import kotlin.concurrent.atomics.ExperimentalAtomicApi

/** The speech synthesizer a TTS helper serves. */
interface SpeechSynth {
    /** The voices on offer; the helper lists at most [MAX_HELPER_VOICES]. */
    val voices: List<HelperVoice>

    /**
     * Speaks [text] in [voice] at [rate] and returns once all of its audio went to [audio]: mono
     * samples at the voice's rate, the first `count` of the array, which is the synth's own and
     * may be reused once the call returns. [audio] may run on any thread but runs one call at a
     * time and never after `speak` returns. Throws when the voice is unknown or speech fails.
     */
    fun speak(
        text: String,
        voice: String,
        rate: SpeechRate,
        audio: (samples: FloatArray, count: Int) -> Unit,
    )
}

data class TtsServeOptions(
    /** Text of a reply waiting to be spoken; past it the reply fails rather than the helper growing. */
    val maxQueuedTextBytes: Long = 1L shl 20,
) {
    init {
        require(maxQueuedTextBytes > 0) { "maxQueuedTextBytes must be positive" }
    }
}

private sealed interface Job {
    val seq: Int
}

private class Start(
    override val seq: Int,
    val speak: Speak,
) : Job

private class Say(
    override val seq: Int,
    val text: String,
    val bytes: Int,
) : Job

private class End(
    override val seq: Int,
) : Job

private class Overflow(
    override val seq: Int,
) : Job

/**
 * The TTS helper's side of the pipe protocol: lists the [SpeechSynth]'s voices, says
 * [FrameType.READY], then speaks replies until aulod closes the pipe.
 *
 * One thread reads frames and queues the texts of the reply begun last; another speaks them in
 * order, sends the audio as it comes and [FrameType.END] after [FrameType.FINISH]. Reading never
 * waits for speech, so a cancel takes effect at once: the reply's queued text is skipped and the
 * audio still coming for it is dropped. A failing synth answers [FrameType.ERROR] for that reply
 * and the helper keeps serving.
 *
 * Throws [WireException] when aulod's side breaks the protocol, so the helper exits.
 */
fun serveTts(
    input: BufferedSource,
    output: BufferedSink,
    synth: SpeechSynth,
    options: TtsServeOptions = TtsServeOptions(),
) {
    for (voice in synth.voices.take(MAX_HELPER_VOICES)) output.writeVoice(voice)
    output.writeFrame(FrameType.READY, 0)
    output.flush()

    // The reply being served; any other reply's text and audio are dropped.
    val current = AtomicInt(0)
    val queuedBytes = AtomicLong(0)
    // Unlimited in count, bounded by queuedBytes: the reader must never wait, or a cancel would wait behind it.
    val jobs = Channel<Job>(Channel.UNLIMITED)

    runBlocking(Dispatchers.IO) {
        launch { speakLoop(jobs, output, synth, current, queuedBytes) }
        try {
            readLoop(input, jobs, current, queuedBytes, options)
        } finally {
            // aulod is gone or broke the protocol: nothing queued is wanted any more.
            current.store(0)
            jobs.close()
        }
    }
}

private fun readLoop(
    input: BufferedSource,
    jobs: Channel<Job>,
    current: AtomicInt,
    queuedBytes: AtomicLong,
    options: TtsServeOptions,
) {
    while (true) {
        val frame = input.readFrame() ?: return
        when (frame.type) {
            FrameType.SPEAK -> {
                val speak = frame.speak()
                current.store(frame.seq)
                jobs.trySend(Start(frame.seq, speak))
            }

            FrameType.TEXT -> {
                if (frame.seq != current.load()) continue
                val size = frame.payload.size
                if (queuedBytes.addAndFetch(size.toLong()) > options.maxQueuedTextBytes) {
                    queuedBytes.addAndFetch(-size.toLong())
                    current.store(-frame.seq)
                    jobs.trySend(Overflow(frame.seq))
                } else {
                    jobs.trySend(Say(frame.seq, frame.text, size))
                }
            }

            FrameType.FINISH -> {
                if (frame.seq == current.load()) jobs.trySend(End(frame.seq))
            }

            FrameType.CANCEL -> {
                if (frame.seq == current.load()) current.store(-frame.seq)
            }

            else -> {
                throw WireException("${frame.type.name} is not a TTS request")
            }
        }
    }
}

private suspend fun speakLoop(
    jobs: Channel<Job>,
    output: BufferedSink,
    synth: SpeechSynth,
    current: AtomicInt,
    queuedBytes: AtomicLong,
) {
    var speak: Speak? = null
    for (job in jobs) {
        if (job is Say) queuedBytes.addAndFetch(-job.bytes.toLong())
        val seq = job.seq
        // An overflowing reply was already moved off; its error still goes out unless another reply began.
        val live = if (job is Overflow) current.load() == -seq else current.load() == seq
        if (!live) continue
        when (job) {
            is Start -> {
                speak = job.speak
            }

            is Say -> {
                val request = speak ?: continue
                if (job.text.isBlank()) continue
                try {
                    synth.speak(job.text, request.voice, request.rate) { samples, count ->
                        if (seq == current.load()) {
                            for (from in 0 until count step MAX_AUDIO_SAMPLES) {
                                output.writeAudio(seq, samples, from, minOf(MAX_AUDIO_SAMPLES, count - from))
                            }
                            output.flush()
                        }
                    }
                } catch (e: okio.IOException) {
                    // aulod stopped reading: nothing more can be said to it.
                    throw e
                } catch (e: Exception) {
                    if (current.compareAndSet(seq, -seq)) {
                        output.writeText(FrameType.ERROR, seq, "speech failed: ${e.message ?: e::class.simpleName}")
                        output.flush()
                    }
                }
            }

            is End -> {
                output.writeFrame(FrameType.END, seq)
                output.flush()
            }

            is Overflow -> {
                output.writeText(FrameType.ERROR, seq, "reply text over the queue's cap")
                output.flush()
            }
        }
    }
}
