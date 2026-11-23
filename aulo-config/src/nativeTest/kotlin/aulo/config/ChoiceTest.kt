package aulo.config

import aulo.speech.EngineChoice
import aulo.speech.EngineId
import aulo.speech.MAX_FALLBACK_ENGINES
import aulo.speech.SpeechRate
import fs.TempDir
import kotlinx.io.files.Path
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull

class ChoiceTest {
    @Test
    fun aSectionBecomesAValidatedChoice() {
        val raw = EngineConfig(engine = "kokoro", model = "v1", voice = "anna", rate = 1.25f, fallback = listOf("piper", "say"))
        assertEquals(
            EngineChoice(EngineId("kokoro"), "v1", "anna", SpeechRate(1.25f), listOf(EngineId("piper"), EngineId("say"))),
            raw.choice("voice.tts"),
        )
        assertEquals(EngineChoice(), EngineConfig().choice("voice.stt"))
    }

    @Test
    fun aBadValueNamesItsKeyButNotTheValue() {
        val secret = "sk-live-Token"
        val error = assertFailsWith<ConfigException> { EngineConfig(engine = secret).choice("voice.tts") }
        assertEquals("voice.tts.engine", error.key)
        assertEquals("invalid engine id: only a-z, 0-9, '-', '_' and '.' for key `voice.tts.engine`", error.detail)
        assertFalse(secret in error.message.orEmpty())
        assertNull(error.origin)
    }

    @Test
    fun eachFieldIsReportedUnderItsOwnKey() {
        val cases =
            mapOf(
                "voice.stt.model" to EngineConfig(model = "a\u0000b"),
                "voice.stt.voice" to EngineConfig(voice = ""),
                "voice.stt.rate" to EngineConfig(rate = 9f),
                "voice.stt.fallback" to EngineConfig(fallback = listOf("ok", "Bad")),
            )
        for ((key, raw) in cases) {
            assertEquals(key, assertFailsWith<ConfigException> { raw.choice("voice.stt") }.key)
        }
    }

    @Test
    fun theFallbackListIsCapped() {
        val raw = EngineConfig(fallback = List(MAX_FALLBACK_ENGINES + 1) { "engine$it" })
        val error = assertFailsWith<ConfigException> { raw.choice("voice.tts") }
        assertEquals("invalid fallback: lists more than $MAX_FALLBACK_ENGINES engines for key `voice.tts.fallback`", error.detail)
        assertEquals(
            MAX_FALLBACK_ENGINES,
            EngineConfig(fallback = List(MAX_FALLBACK_ENGINES) { "engine$it" }).choice("voice.tts").fallback.size,
        )
    }

    @Test
    fun theErrorNamesTheLayerThatSetTheValue() {
        val tmp = TempDir.create(".aulo-choice-test")
        try {
            val variable = "AULO_VOICE__TTS__RATE"
            val loaded = loadConfig(LoadOptions(env = mapOf(variable to "5"), home = Path(tmp.path, "home").toString()))
            val error =
                assertFailsWith<ConfigException> {
                    loaded.config.voice.tts
                        .choice("voice.tts", loaded.provenance)
                }
            assertEquals(Origin.Env(variable), error.origin)
        } finally {
            tmp.close()
        }
    }
}
