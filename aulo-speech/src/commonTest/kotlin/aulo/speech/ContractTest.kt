package aulo.speech

import aulo.types.TranscriptKind
import aulo.types.TurnId
import kotlin.math.abs
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

// One fake engine per interface, driven only through the interface. They pin the push/poll
// protocol that the real engines and the pipeline rely on.

/** 20 ms at 16 kHz, the frame aulo-audio delivers. */
private const val FRAME_SAMPLES = 320
private const val LOUD = 0.5f
private const val SPEECH_THRESHOLD = 0.1f
private const val STT_CAPACITY_SAMPLES = FRAME_SAMPLES * 4
private const val SAMPLES_PER_CHAR = FRAME_SAMPLES / 2
private const val SPOKEN = "open the browser and play some music"
private const val TTS_RATE_HZ = 24_000
private const val TTS_SAMPLES_PER_CHAR = 100
private const val SILENCE_FOR_END_OF_TURN = PIPELINE_SAMPLE_RATE_HZ * 700L / 1_000

private fun info(
    id: String,
    kind: EngineKind,
) = EngineInfo(
    EngineId(id),
    kind,
    id,
    Capabilities(streaming = true, languages = LanguageSupport.Listed(listOf("en")), offline = true, needsNetwork = false),
)

private val loud = FloatArray(FRAME_SAMPLES) { LOUD }
private val quiet = FloatArray(FRAME_SAMPLES)

private fun frame(
    samples: FloatArray,
    index: Int,
) = AudioFrame(samples, AudioFormat.PIPELINE, index.toLong() * FRAME_SAMPLES)

private fun AudioFrame.isLoud() = (0 until size).any { abs(this[it]) > SPEECH_THRESHOLD }

/** Buffers audio in fixed storage and "recognizes" a prefix of [SPOKEN] that grows with the audio heard. */
private class FakeStt(
    id: String,
    private val failBegin: SpeechException? = null,
) : SttEngine {
    override val info = info(id, EngineKind.STT)
    val audio = FloatArray(STT_CAPACITY_SAMPLES)
    private var heard = 0
    private var turn: TurnId? = null
    private var fresh = false
    private var finished = false
    private var finalSent = false
    var closed = false

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

/** Turns every pushed character into a fixed number of silent samples. */
private class FakeTts : TtsEngine {
    override val info = info("fake-tts", EngineKind.TTS)
    override val voices = listOf(Voice("anna", "Anna", "en"))
    private var active = false
    private var pending = 0
    private var finished = false

    override fun begin(request: TtsRequest): AudioFormat {
        val voice = request.voice
        if (voice != null && voices.none { it.id == voice }) throw SpeechException.Unsupported(voice)
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
}

/** Peak-energy VAD. */
private class FakeVad : Vad {
    override val info = info("fake-vad", EngineKind.VAD)
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
private class FakeKeywords : KeywordSpotter {
    override val info = info("fake-kws", EngineKind.KEYWORD_SPOTTER)
    private var keywords = 0

    override fun setKeywords(phrases: List<String>) {
        keywords = phrases.size
    }

    override fun push(frame: AudioFrame) = if (frame.isLoud() && keywords > 0) KeywordHit(0, frame.endPosition) else null

    override fun reset() {}
}

/** Ends the turn after a fixed silence that follows speech. */
private class SilenceTurn : TurnDetector {
    override val info = info("silence", EngineKind.TURN_DETECTOR)
    private var heardSpeech = false
    private var silent = 0L
    var lastTranscript = ""

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

/**
 * What the registry does: the first engine that starts wins, engine faults are noted and the
 * engine skipped, anything else stops the search.
 */
private fun beginWithFallback(
    engines: List<SttEngine>,
    turn: TurnId,
    notices: MutableList<String>,
): SttEngine {
    var last: SpeechException = SpeechException.Unavailable("no stt engine configured")
    for (engine in engines) {
        try {
            engine.begin(turn, "en")
            return engine
        } catch (e: SpeechException) {
            if (!e.shouldFallBack) throw e
            notices += "${engine.info.id}: ${e.message}"
            last = e
        }
    }
    throw last
}

class ContractTest {
    @Test
    fun stt_streams_partials_then_exactly_one_final() {
        val stt: SttEngine = FakeStt("fake-stt")
        val turn = TurnId.new()
        val out = Transcript(turn)
        stt.begin(turn, null)
        for (index in 0 until 3) {
            assertEquals(0, stt.push(frame(loud, index)))
            assertEquals(SttPoll.UPDATED, stt.poll(out))
            assertEquals(TranscriptKind.PARTIAL, out.kind)
        }
        assertEquals(SttPoll.PENDING, stt.poll(out))
        stt.finish()
        assertEquals(SttPoll.UPDATED, stt.poll(out))
        assertEquals(TranscriptKind.FINAL to turn, out.kind to out.turnId)
        assertEquals("open t", out.text.toString())
        assertEquals(SttPoll.DONE, stt.poll(out))
    }

    @Test
    fun a_full_stt_queue_drops_the_frame_without_growing() {
        val stt = FakeStt("fake-stt")
        stt.begin(TurnId.new(), null)
        repeat(4) { assertEquals(0, stt.push(frame(loud, it))) }
        assertEquals(FRAME_SAMPLES, stt.push(frame(loud, 4)))
        assertEquals(STT_CAPACITY_SAMPLES, stt.audio.size)
    }

    @Test
    fun stt_refuses_audio_outside_an_utterance() {
        val stt: SttEngine = FakeStt("fake-stt")
        assertFailsWith<SpeechException.OutOfOrder> { stt.push(frame(loud, 0)) }
        stt.begin(TurnId.new(), null)
        stt.finish()
        assertFailsWith<SpeechException.OutOfOrder> { stt.push(frame(loud, 1)) }
    }

    @Test
    fun begin_again_drops_the_previous_utterance() {
        val stt: SttEngine = FakeStt("fake-stt")
        val old = TurnId.new()
        val new = TurnId.new()
        val out = Transcript(old)
        stt.begin(old, null)
        stt.push(frame(loud, 0))
        stt.begin(new, null)
        assertEquals(SttPoll.PENDING, stt.poll(out))
        stt.push(frame(loud, 1))
        stt.poll(out)
        assertEquals(new, out.turnId)
    }

    @Test
    fun the_registry_falls_back_past_a_failing_engine() {
        val engines = listOf(FakeStt("broken-plugin", SpeechException.Unavailable("plugin process exited")), FakeStt("fake-stt"))
        val notices = mutableListOf<String>()
        assertEquals("fake-stt", beginWithFallback(engines, TurnId.new(), notices).info.id.value)
        assertEquals(listOf("broken-plugin: engine unavailable: plugin process exited"), notices)
    }

    @Test
    fun caller_bugs_do_not_trigger_fallback() {
        val engines = listOf(FakeStt("fake-a", SpeechException.OutOfOrder("finish before begin")), FakeStt("fake-b"))
        val notices = mutableListOf<String>()
        assertFailsWith<SpeechException.OutOfOrder> { beginWithFallback(engines, TurnId.new(), notices) }
        assertTrue(notices.isEmpty())
    }

    @Test
    fun an_engine_closes_through_the_shared_interface() {
        val stt = FakeStt("fake-stt")
        val engines: List<SpeechEngine> = listOf(stt, FakeTts(), FakeVad())
        engines.forEach { it.close() }
        assertTrue(stt.closed)
    }

    @Test
    fun tts_plays_the_first_sentence_before_the_reply_is_finished() {
        val tts: TtsEngine = FakeTts()
        val out = FloatArray(FRAME_SAMPLES)
        assertEquals(TTS_RATE_HZ, tts.begin(TtsRequest(TurnId.new(), voice = "anna", language = "en")).sampleRateHz)
        tts.pushText("Opening the browser.")
        assertTrue(tts.poll(out).samples > 0)
    }

    @Test
    fun tts_fills_at_most_the_callers_array_then_ends() {
        val tts: TtsEngine = FakeTts()
        val out = FloatArray(FRAME_SAMPLES)
        val sentence = "Done."
        tts.begin(TtsRequest(TurnId.new()))
        tts.pushText(sentence)
        tts.finish()
        var total = 0
        while (true) {
            val poll = tts.poll(out)
            if (poll.isDone) break
            assertTrue(poll.samples <= out.size)
            total += poll.samples
        }
        assertEquals(sentence.length * TTS_SAMPLES_PER_CHAR, total)
    }

    @Test
    fun barge_in_cancel_ends_the_reply_on_the_next_poll() {
        val tts: TtsEngine = FakeTts()
        val out = FloatArray(FRAME_SAMPLES)
        tts.begin(TtsRequest(TurnId.new()))
        tts.pushText("A long answer that would take many seconds to speak.")
        assertTrue(tts.poll(out).samples > 0)
        tts.cancel()
        assertTrue(tts.poll(out).isDone)
        assertFailsWith<SpeechException.OutOfOrder> { tts.pushText("more") }
    }

    @Test
    fun an_unknown_voice_is_unsupported_so_the_registry_falls_back() {
        val tts: TtsEngine = FakeTts()
        val error = assertFailsWith<SpeechException.Unsupported> { tts.begin(TtsRequest(TurnId.new(), voice = "boris")) }
        assertTrue(error.shouldFallBack)
        assertEquals(1, tts.voices.size)
    }

    @Test
    fun detectors_find_the_wake_word_speech_and_end_of_turn() {
        val keywords: KeywordSpotter = FakeKeywords()
        val vad: Vad = FakeVad()
        val silence = SilenceTurn()
        val turn: TurnDetector = silence
        keywords.setKeywords(listOf("hey aulo"))
        val speechFrames = 5
        val events = mutableListOf<VadEvent>()
        var wake: KeywordHit? = null
        var end: Long? = null
        for (index in 0 until 100) {
            val frame = frame(if (index < speechFrames) loud else quiet, index)
            wake = wake ?: keywords.push(frame)
            vad.push(frame)?.let { events += it }
            turn.observeTranscript("open the")
            val decision = turn.push(frame, index < speechFrames)
            if (decision is TurnDecision.EndOfTurn) {
                end = decision.position
                break
            }
        }
        val speechEnd = speechFrames.toLong() * FRAME_SAMPLES
        assertEquals(0, wake?.keyword)
        assertEquals(listOf(VadEvent.SpeechStarted(0), VadEvent.SpeechEnded(speechEnd)), events)
        assertEquals(speechEnd + SILENCE_FOR_END_OF_TURN, end)
        assertEquals("open the", silence.lastTranscript)
    }
}
