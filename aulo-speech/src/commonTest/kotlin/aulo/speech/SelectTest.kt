package aulo.speech

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

// Overrides arrive from gRPC clients, so every label in a choice is bounded where it is built.
class SelectTest {
    private val tooLong = "m".repeat(MAX_LABEL_BYTES + 1)

    @Test
    fun specLabelsAreCapped() {
        assertFailsWith<SpeechException.Invalid> { EngineSpec(EngineId("kokoro"), model = tooLong) }
        assertFailsWith<SpeechException.Invalid> { EngineSpec(EngineId("kokoro"), voice = "a\nb") }
        assertEquals("v1", EngineSpec(EngineId("kokoro"), model = "v1").model)
    }

    @Test
    fun choiceLabelsAndFallbackAreCapped() {
        assertFailsWith<SpeechException.Invalid> { EngineChoice(model = tooLong) }
        assertFailsWith<SpeechException.Invalid> { EngineChoice(voice = "") }
        val most = List(MAX_FALLBACK_ENGINES) { EngineId("e$it") }
        assertEquals(most, EngineChoice(fallback = most).fallback)
        val error = assertFailsWith<SpeechException.Invalid> { EngineChoice(fallback = most + EngineId("one-more")) }
        assertEquals(false, error.shouldFallBack)
    }
}
