package aulo.speech

import aulo.types.AuloEvent
import aulo.types.TranscriptKind
import aulo.types.TurnId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class TypesTest {
    @Test
    fun engine_details_are_capped_on_a_character_boundary() {
        val error = SpeechException.Failed("я".repeat(MAX_ERROR_DETAIL_BYTES))
        val detail = error.message!!.removePrefix("engine failed: ")
        assertTrue(detail.encodeToByteArray().size <= MAX_ERROR_DETAIL_BYTES)
        assertTrue(detail.all { it == 'я' })
    }

    @Test
    fun engine_details_cannot_forge_log_lines() {
        assertEquals("not supported: bad INFO forged [2J", SpeechException.Unsupported("bad\nINFO forged\u001b[2J").message)
    }

    @Test
    fun only_engine_faults_trigger_fallback() {
        assertTrue(SpeechException.Unavailable("no model").shouldFallBack)
        assertTrue(SpeechException.Unsupported("ru").shouldFallBack)
        assertTrue(SpeechException.Failed("socket closed").shouldFallBack)
        assertFalse(SpeechException.OutOfOrder("push before begin").shouldFallBack)
        assertFalse(SpeechException.Invalid("audio frame", "odd length").shouldFallBack)
    }

    @Test
    fun zero_rate_or_channels_is_rejected() {
        assertFailsWith<SpeechException.Invalid> { AudioFormat(0, 1) }
        assertFailsWith<SpeechException.Invalid> { AudioFormat(PIPELINE_SAMPLE_RATE_HZ, 0) }
        assertFailsWith<SpeechException.Invalid> { AudioFormat(PIPELINE_SAMPLE_RATE_HZ, MAX_CHANNELS + 1) }
    }

    @Test
    fun a_frame_holds_whole_sample_groups_inside_its_array() {
        val stereo = AudioFormat(48_000, 2)
        assertFailsWith<SpeechException.Invalid> { AudioFrame(FloatArray(3), stereo, 0) }
        assertEquals(2, AudioFrame(FloatArray(4), stereo, 0).length)
        assertFailsWith<SpeechException.Invalid> { AudioFrame(FloatArray(4), stereo, 0, offset = 2, size = 4) }
        assertFailsWith<SpeechException.Invalid> { AudioFrame(FloatArray(4), stereo, 0, offset = 5, size = 0) }
        assertFailsWith<SpeechException.Invalid> { AudioFrame(FloatArray(4), stereo, -1) }
    }

    @Test
    fun a_frame_is_a_window_into_the_callers_array() {
        val samples = floatArrayOf(0f, 1f, 2f, 3f, 4f)
        val frame = AudioFrame(samples, AudioFormat.PIPELINE, 0, offset = 1, size = 3)
        assertEquals(2f, frame[1])
        assertFailsWith<IndexOutOfBoundsException> { frame[3] }
        val copy = FloatArray(4)
        frame.copyInto(copy, 1)
        assertEquals(listOf(0f, 1f, 2f, 3f), copy.toList())
    }

    @Test
    fun positions_convert_to_milliseconds_at_the_frame_rate() {
        val frame = AudioFrame(FloatArray(320), AudioFormat.PIPELINE, 16_000)
        assertEquals(1_000, frame.atMs)
        assertEquals(16_320, frame.endPosition)
        assertEquals(1_020, AudioFormat.PIPELINE.positionToMs(frame.endPosition))
        // No overflow, even at a position no stream reaches.
        assertEquals(576_460_752_303_423_487, AudioFormat.PIPELINE.positionToMs(Long.MAX_VALUE))
    }

    @Test
    fun engine_ids_take_the_config_alphabet_only() {
        for (id in listOf("sherpa-kokoro", "openai.tts", "whisper_cpp", "eleven2")) assertEquals(id, EngineId(id).value)
        for (id in listOf("", "Kokoro", "1engine", "a b", "a/b", "tts\n", "ъ", "a".repeat(MAX_ENGINE_ID_BYTES + 1))) {
            assertFailsWith<SpeechException.Invalid>(id) { EngineId(id) }
        }
    }

    @Test
    fun languages_match_on_the_primary_subtag() {
        val listed = LanguageSupport.Listed(listOf("en", "ru-RU"))
        assertTrue(listed.supports("en-US"))
        assertTrue(listed.supports("RU"))
        assertFalse(listed.supports("uk"))
        assertTrue(LanguageSupport.Multilingual.supports("uk"))
    }

    @Test
    fun language_lists_from_engines_are_bounded() {
        assertFailsWith<SpeechException.Invalid> { LanguageSupport.Listed(emptyList()) }
        assertFailsWith<SpeechException.Invalid> { LanguageSupport.Listed(List(MAX_LANGUAGES + 1) { "en" }) }
        assertFailsWith<SpeechException.Invalid> { LanguageSupport.Listed(listOf("en\n")) }
        assertFailsWith<SpeechException.Invalid> { LanguageSupport.Listed(listOf("x".repeat(36))) }
    }

    @Test
    fun names_and_voice_ids_are_one_bounded_line() {
        val ok = "в".repeat(MAX_LABEL_BYTES / 2)
        assertEquals(ok, Voice(ok, ok).id)
        assertFailsWith<SpeechException.Invalid> { Voice(ok + "x", "Anna") }
        assertFailsWith<SpeechException.Invalid> { Voice("anna", "") }
        assertFailsWith<SpeechException.Invalid> { Voice("anna", "Anna\r\nINFO forged") }
        assertFailsWith<SpeechException.Invalid> { Voice("anna", "Anna", "en us") }
        val capabilities = Capabilities(true, LanguageSupport.Multilingual, offline = true, needsNetwork = false)
        assertFailsWith<SpeechException.Invalid> {
            EngineInfo(
                EngineId("x"),
                EngineKind.TTS,
                "x".repeat(MAX_LABEL_BYTES + 1),
                capabilities,
            )
        }
    }

    @Test
    fun rate_outside_the_range_or_nan_is_rejected() {
        assertEquals(1.5f, SpeechRate(1.5f).value)
        for (rate in listOf(0f, 0.49f, 2.01f, Float.NaN, Float.POSITIVE_INFINITY)) {
            assertFailsWith<SpeechException.Invalid>("$rate") { SpeechRate(rate) }
        }
    }

    @Test
    fun tts_poll_tells_pending_audio_and_done_apart() {
        assertTrue(TtsPoll.PENDING.isPending)
        assertTrue(TtsPoll.DONE.isDone)
        val audio = TtsPoll.audio(320)
        assertEquals(320, audio.samples)
        assertFalse(audio.isPending || audio.isDone)
        assertEquals(0, TtsPoll.DONE.samples)
        assertFailsWith<SpeechException.Invalid> { TtsPoll.audio(0) }
    }

    @Test
    fun transcript_text_is_capped_so_the_event_always_builds() {
        val turn = TurnId.new()
        val out = Transcript(turn)
        // Three bytes per character: the cap falls inside a character, which must not be split.
        out.set(turn, TranscriptKind.FINAL, "€".repeat(AuloEvent.Transcript.MAX_TEXT_BYTES), "x".repeat(100))
        assertEquals(AuloEvent.Transcript.MAX_TEXT_BYTES / 3, out.text.length)
        val event = out.toEvent()
        assertEquals(AuloEvent.Transcript.MAX_LANGUAGE_BYTES, event.language!!.length)
        assertEquals(TranscriptKind.FINAL, event.kind)
    }

    @Test
    fun transcript_cap_never_splits_a_surrogate_pair() {
        val turn = TurnId.new()
        val out = Transcript(turn)
        val emoji = "😀"
        out.set(turn, TranscriptKind.PARTIAL, "a".repeat(AuloEvent.Transcript.MAX_TEXT_BYTES - 3) + emoji, null)
        assertEquals(AuloEvent.Transcript.MAX_TEXT_BYTES - 3, out.text.length)
        out.set(turn, TranscriptKind.PARTIAL, "open $emoji", "en")
        assertEquals("open $emoji", out.text.toString())
        assertEquals("en", out.language)
        out.set(turn, TranscriptKind.FINAL, "", null)
        assertEquals(null, out.language)
    }
}
