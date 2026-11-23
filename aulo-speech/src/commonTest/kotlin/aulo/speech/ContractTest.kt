package aulo.speech

import aulo.testkit.FRAME_SAMPLES
import aulo.testkit.FakeKeywords
import aulo.testkit.FakeStt
import aulo.testkit.FakeTts
import aulo.testkit.FakeVad
import aulo.testkit.SILENCE_FOR_END_OF_TURN
import aulo.testkit.STT_CAPACITY_SAMPLES
import aulo.testkit.SilenceTurn
import aulo.testkit.TTS_RATE_HZ
import aulo.testkit.TTS_SAMPLES_PER_CHAR
import aulo.testkit.frame
import aulo.testkit.loud
import aulo.testkit.quiet
import aulo.types.TranscriptKind
import aulo.types.TurnId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

// Each fake from aulo-testkit is driven only through its interface, pinning the push/poll
// protocol. Fallback across engines is tested where it lives, in aulo-voice.

class ContractTest {
    @Test
    fun stt_streams_partials_then_exactly_one_final() {
        val stt: SttEngine = FakeStt()
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
        val stt = FakeStt()
        stt.begin(TurnId.new(), null)
        repeat(4) { assertEquals(0, stt.push(frame(loud, it))) }
        assertEquals(FRAME_SAMPLES, stt.push(frame(loud, 4)))
        assertEquals(STT_CAPACITY_SAMPLES, stt.audio.size)
    }

    @Test
    fun stt_refuses_audio_outside_an_utterance() {
        val stt: SttEngine = FakeStt()
        assertFailsWith<SpeechException.OutOfOrder> { stt.push(frame(loud, 0)) }
        stt.begin(TurnId.new(), null)
        stt.finish()
        assertFailsWith<SpeechException.OutOfOrder> { stt.push(frame(loud, 1)) }
    }

    @Test
    fun begin_again_drops_the_previous_utterance() {
        val stt: SttEngine = FakeStt()
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
    fun an_engine_closes_through_the_shared_interface() {
        val stt = FakeStt()
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
