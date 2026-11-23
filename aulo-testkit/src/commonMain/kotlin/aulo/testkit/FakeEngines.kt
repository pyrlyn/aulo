package aulo.testkit

import aulo.speech.AudioFormat
import aulo.speech.AudioFrame
import aulo.speech.Capabilities
import aulo.speech.EngineId
import aulo.speech.EngineInfo
import aulo.speech.EngineKind
import aulo.speech.KeywordHit
import aulo.speech.KeywordSpotter
import aulo.speech.LanguageSupport
import aulo.speech.PIPELINE_SAMPLE_RATE_HZ
import aulo.speech.SpeechException
import aulo.speech.SttEngine
import aulo.speech.SttPoll
import aulo.speech.Transcript
import aulo.speech.TtsEngine
import aulo.speech.TtsPoll
import aulo.speech.TtsRequest
import aulo.speech.TurnDecision
import aulo.speech.TurnDetector
import aulo.speech.Vad
import aulo.speech.VadEvent
import aulo.speech.Voice
import aulo.types.TranscriptKind
import aulo.types.TurnId
import kotlin.math.abs

// One fake engine per speech interface, small enough to read in a test failure. They pin the
// push/poll protocol the real engines, the registry and the pipeline rely on.

/** 20 ms at 16 kHz, the frame aulo-audio delivers. */
const val FRAME_SAMPLES = 320

/** What [FakeStt] buffers before it starts dropping frames. */
const val STT_CAPACITY_SAMPLES = FRAME_SAMPLES * 4

/** What [FakeStt] "recognizes": a prefix that grows with the audio heard. */
const val SPOKEN = "open the browser and play some music"
private const val SAMPLES_PER_CHAR = FRAME_SAMPLES / 2

const val TTS_RATE_HZ = 24_000
const val TTS_SAMPLES_PER_CHAR = 100

/** The one voice [FakeTts] has; any other is unsupported. */
const val FAKE_VOICE = "anna"

/** The silence after speech that makes [SilenceTurn] end the turn. */
const val SILENCE_FOR_END_OF_TURN = PIPELINE_SAMPLE_RATE_HZ * 700L / 1_000

private const val LOUD = 0.5f
private const val SPEECH_THRESHOLD = 0.1f

/** A frame of speech and a frame of silence for [frame]. */
val loud = FloatArray(FRAME_SAMPLES) { LOUD }
val quiet = FloatArray(FRAME_SAMPLES)

/** The [index]th pipeline frame of [samples]. */
fun frame(
    samples: FloatArray,
    index: Int,
) = AudioFrame(samples, AudioFormat.PIPELINE, index.toLong() * FRAME_SAMPLES)

private fun AudioFrame.isLoud() = (0 until size).any { abs(this[it]) > SPEECH_THRESHOLD }

fun fakeInfo(
    id: String,
    kind: EngineKind,
) = EngineInfo(
    EngineId(id),
    kind,
    id,
    Capabilities(streaming = true, languages = LanguageSupport.Listed(listOf("en")), offline = true, needsNetwork = false),
)

/** Buffers audio in fixed storage; [failBegin] makes every begin throw it. */
class FakeStt(
    id: String = "fake-stt",
    private val failBegin: SpeechException? = null,
) : SttEngine {
    override val info = fakeInfo(id, EngineKind.STT)
    val audio = FloatArray(STT_CAPACITY_SAMPLES)
    private var heard = 0
    private var turn: TurnId? = null
    private var fresh = false
    private var finished = false
    private var finalSent = false
    var closed = false
        private set

    override fun begin(
        turnId: TurnId,
        language: String?,
    ) {
        failBegin?.let { throw it }
        cancel()
        turn = turnId
    }

    override fun push(frame: AudioFrame): Int {
        if (turn == null || finished) throw SpeechException.OutOfOrder("push outside an utterance")
        if (heard + frame.size > audio.size) return frame.length
        frame.copyInto(audio, heard)
        heard += frame.size
        fresh = true
        return 0
    }

    override fun finish() {
        if (turn == null) throw SpeechException.OutOfOrder("finish before begin")
        finished = true
    }

    override fun poll(out: Transcript): SttPoll {
        val turn = turn ?: return SttPoll.PENDING
        if (finalSent) return SttPoll.DONE
        val kind =
            when {
                finished -> TranscriptKind.FINAL.also { finalSent = true }
                fresh -> TranscriptKind.PARTIAL
                else -> return SttPoll.PENDING
            }
        fresh = false
        out.set(turn, kind, SPOKEN.subSequence(0, minOf(heard / SAMPLES_PER_CHAR, SPOKEN.length)), "en")
        return SttPoll.UPDATED
    }

    override fun cancel() {
        turn = null
        heard = 0
        fresh = false
        finished = false
        finalSent = false
    }

    override fun close() {
        closed = true
    }
}

/** Turns every pushed character into [TTS_SAMPLES_PER_CHAR] silent samples; [failBegin] makes every begin throw it. */
class FakeTts(
    id: String = "fake-tts",
    private val failBegin: SpeechException? = null,
) : TtsEngine {
    override val info = fakeInfo(id, EngineKind.TTS)
    override val voices = listOf(Voice(FAKE_VOICE, "Anna", "en"))
    private var active = false
    private var pending = 0
    private var finished = false
    var closed = false
        private set

    override fun begin(request: TtsRequest): AudioFormat {
        failBegin?.let { throw it }
        val voice = request.voice
        if (voice != null && voices.none { it.id == voice }) throw SpeechException.Unsupported("voice `$voice`")
        cancel()
        active = true
        return AudioFormat(TTS_RATE_HZ, 1)
    }

    override fun pushText(text: String) {
        if (!active || finished) throw SpeechException.OutOfOrder("text outside a reply")
        pending += text.length * TTS_SAMPLES_PER_CHAR
    }

    override fun finish() {
        finished = true
    }

    override fun poll(out: FloatArray): TtsPoll {
        if (!active) return TtsPoll.DONE
        val samples = minOf(pending, out.size)
        if (samples > 0) {
            out.fill(0f, 0, samples)
            pending -= samples
            return TtsPoll.audio(samples)
        }
        if (finished) {
            active = false
            return TtsPoll.DONE
        }
        return TtsPoll.PENDING
    }

    override fun cancel() {
        active = false
        pending = 0
        finished = false
    }

    override fun close() {
        closed = true
    }
}

/** Peak-energy VAD. */
class FakeVad : Vad {
    override val info = fakeInfo("fake-vad", EngineKind.VAD)
    private var inSpeech = false

    override fun push(frame: AudioFrame): VadEvent? {
        val loud = frame.isLoud()
        val event =
            when {
                loud && !inSpeech -> VadEvent.SpeechStarted(frame.position)
                !loud && inSpeech -> VadEvent.SpeechEnded(frame.position)
                else -> null
            }
        inSpeech = loud
        return event
    }

    override fun reset() {
        inSpeech = false
    }
}

/** Hears the first keyword in any loud frame. */
class FakeKeywords : KeywordSpotter {
    override val info = fakeInfo("fake-kws", EngineKind.KEYWORD_SPOTTER)
    private var keywords = 0

    override fun setKeywords(phrases: List<String>) {
        keywords = phrases.size
    }

    override fun push(frame: AudioFrame) = if (frame.isLoud() && keywords > 0) KeywordHit(0, frame.endPosition) else null

    override fun reset() {}
}

/** Ends the turn after [SILENCE_FOR_END_OF_TURN] of silence that follows speech. */
class SilenceTurn : TurnDetector {
    override val info = fakeInfo("silence", EngineKind.TURN_DETECTOR)
    private var heardSpeech = false
    private var silent = 0L
    var lastTranscript = ""
        private set

    override fun push(
        frame: AudioFrame,
        inSpeech: Boolean,
    ): TurnDecision {
        if (inSpeech) {
            heardSpeech = true
            silent = 0
            return TurnDecision.Continue
        }
        silent += frame.length
        return if (heardSpeech && silent >= SILENCE_FOR_END_OF_TURN) TurnDecision.EndOfTurn(frame.endPosition) else TurnDecision.Continue
    }

    override fun observeTranscript(text: CharSequence) {
        lastTranscript = text.toString()
    }

    override fun reset() {
        heardSpeech = false
        silent = 0
    }
}
