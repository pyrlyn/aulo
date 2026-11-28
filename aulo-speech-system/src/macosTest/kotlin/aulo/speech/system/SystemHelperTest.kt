@file:OptIn(ExperimentalForeignApi::class)

package aulo.speech.system

import aulo.speech.EngineSpec
import aulo.speech.SpeechException
import aulo.speech.TtsEngine
import aulo.speech.TtsRequest
import aulo.speech.helper.MAC_VOICES
import aulo.speech.helper.registerMacVoices
import aulo.types.TurnId
import aulo.voice.TtsRegistry
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.toKString
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import platform.posix.getenv
import kotlin.math.abs
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TimeSource

/** The engine aulod builds, over the helper this module links: Gradle sets AULO_SYSTEM_HELPER for the test task. */
class SystemHelperTest {
    private val helper = getenv("AULO_SYSTEM_HELPER")?.toKString()?.takeIf { it.isNotEmpty() }

    private fun build(helper: String): TtsEngine = TtsRegistry().apply { registerMacVoices(helper) }.build(EngineSpec(MAC_VOICES))

    /** Every sample of the reply, polled as the pipeline would. */
    private fun TtsEngine.drain(): FloatArray {
        val out = FloatArray(4096)
        var got = FloatArray(0)
        val start = TimeSource.Monotonic.markNow()
        while (true) {
            val poll = poll(out)
            if (poll.isDone) return got
            if (poll.samples > 0) got += out.copyOf(poll.samples)
            if (poll.isPending) {
                check(start.elapsedNow() < 60.seconds) { "the reply did not end" }
                runBlocking { delay(5) }
            }
        }
    }

    @Test
    fun theHelperListsTheInstalledVoicesAndSpeaks() {
        val helper = helper ?: return
        build(helper).use { tts ->
            assertTrue(tts.voices.isNotEmpty(), "no voices")
            assertTrue(
                tts.info.capabilities.languages
                    .supports("en"),
                "no English voice",
            )
            val format = tts.begin(TtsRequest(TurnId.new(), language = "en-US"))
            assertEquals(1, format.channels)
            tts.pushText("Hello from aulo.")
            tts.finish()
            val audio = tts.drain()
            val seconds = audio.size.toDouble() / format.sampleRateHz
            assertTrue(seconds in 0.3..10.0, "$seconds s of audio")
            assertTrue(audio.any { abs(it) > 0.05f }, "the audio is silent")
            assertTrue(audio.all { it in -1f..1f }, "samples out of range")
        }
    }

    @Test
    fun aCancelledReplyIsDoneAndTheNextOneSpeaks() {
        val helper = helper ?: return
        build(helper).use { tts ->
            tts.begin(TtsRequest(TurnId.new(), language = "en"))
            tts.pushText("This sentence is cut short before it is over.")
            tts.cancel()
            assertTrue(tts.poll(FloatArray(16)).isDone)
            tts.begin(TtsRequest(TurnId.new(), language = "en"))
            tts.pushText("Yes.")
            tts.finish()
            assertTrue(tts.drain().isNotEmpty())
        }
    }

    @Test
    fun anUnknownVoiceIsUnsupported() {
        val helper = helper ?: return
        build(helper).use { tts ->
            assertFailsWith<SpeechException.Unsupported> { tts.begin(TtsRequest(TurnId.new(), voice = "com.example.nobody")) }
        }
    }

    @Test
    fun aHelperThatIsNotThereIsUnavailable() {
        assertFailsWith<SpeechException.Unavailable> { build("/nonexistent/aulo-speech-system") }
    }
}
