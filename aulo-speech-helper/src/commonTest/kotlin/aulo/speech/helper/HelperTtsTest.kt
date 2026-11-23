@file:OptIn(ExperimentalAtomicApi::class)

package aulo.speech.helper

import aulo.speech.AudioFormat
import aulo.speech.EngineId
import aulo.speech.EngineKind
import aulo.speech.SpeechException
import aulo.speech.SpeechRate
import aulo.speech.TtsPoll
import aulo.speech.TtsRequest
import aulo.speech.Voice
import aulo.types.TurnId
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.sync.Mutex
import okio.BufferedSink
import okio.BufferedSource
import kotlin.concurrent.atomics.AtomicInt
import kotlin.concurrent.atomics.ExperimentalAtomicApi
import kotlin.test.AfterTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TestTimeSource
import kotlin.time.TimeSource

/** Speaks each text as [chunks] buffers of [chunkSamples] samples, every sample the text's length. */
private class FakeSynth(
    override val voices: List<HelperVoice>,
    private val chunks: Int = 3,
    private val chunkSamples: Int = 100,
    private val gate: Mutex? = null,
) : SpeechSynth {
    val handed = AtomicInt(0)
    val spoken = mutableListOf<String>()

    override fun speak(
        text: String,
        voice: String,
        rate: SpeechRate,
        audio: (FloatArray, Int) -> Unit,
    ) {
        require(voices.any { it.voice.id == voice }) { "no voice $voice" }
        if (text == "boom") error("synth crashed")
        if (text == "first") runBlocking { gate?.lock() }
        spoken += "$voice@${rate.value}:$text"
        val buf = FloatArray(chunkSamples)
        repeat(chunks) {
            buf.fill(text.length.toFloat())
            audio(buf, chunkSamples)
            handed.addAndFetch(chunkSamples)
        }
    }
}

class HelperTtsTest {
    private val ruDefault = HelperVoice(Voice("milena", "Milena", "ru-RU"), 22_050, languageDefault = true)
    private val ru = HelperVoice(Voice("yuri", "Yuri", "ru-RU"), 16_000)
    private val enUs = HelperVoice(Voice("samantha", "Samantha", "en-US"), 22_050, languageDefault = true, systemDefault = true)
    private val enGb = HelperVoice(Voice("daniel", "Daniel", "en-GB"), 48_000, languageDefault = true)
    private val fr = HelperVoice(Voice("thomas", "Thomas", "fr-FR"), 24_000)
    private val all = listOf(ruDefault, ru, enUs, enGb, fr)
    private val turn = TurnId.new()
    private val engines = mutableListOf<AutoCloseable>()

    @AfterTest
    fun closeEngines() = engines.forEach { it.close() }

    private fun link(
        chunks: Int = 64,
        body: (BufferedSource, BufferedSink) -> Unit,
    ) = MemoryLink(chunks, body).also { engines += it }

    private fun engine(
        synth: SpeechSynth = FakeSynth(all),
        options: HelperTtsOptions = HelperTtsOptions(),
        serve: TtsServeOptions = TtsServeOptions(),
    ) = HelperTts
        .start(EngineId("system-test"), "test voices", link { i, o -> serveTts(i, o, synth, serve) }, options)
        .also { engines += it }

    private fun unavailable(body: (BufferedSource, BufferedSink) -> Unit): SpeechException.Unavailable =
        assertFailsWith<SpeechException.Unavailable> {
            HelperTts.start(EngineId("system-test"), "test voices", link(body = body), HelperTtsOptions(startTimeout = 300.milliseconds))
        }

    /** Polls the reply to its end and returns every sample. */
    private fun HelperTts.drain(): List<Float> {
        val got = mutableListOf<Float>()
        val out = FloatArray(4096)
        val start = TimeSource.Monotonic.markNow()
        while (true) {
            val poll = poll(out)
            if (poll.isDone) return got
            for (i in 0 until poll.samples) got += out[i]
            if (poll.isPending) {
                check(start.elapsedNow() < 10.seconds) { "the reply did not end; ${got.size} samples so far" }
                runBlocking { delay(1) }
            }
        }
    }

    private fun HelperTts.failure(): SpeechException =
        eventually {
            try {
                poll(FloatArray(64))
                null
            } catch (e: SpeechException) {
                e
            }
        }

    @Test
    fun theEngineOffersTheHelpersVoices() {
        val tts = engine()
        assertEquals(all.map { it.voice }, tts.voices)
        assertEquals(EngineKind.TTS, tts.info.kind)
        assertTrue(tts.info.capabilities.streaming && tts.info.capabilities.offline && !tts.info.capabilities.needsNetwork)
        assertTrue(
            tts.info.capabilities.languages
                .supports("ru"),
        )
        assertTrue(
            tts.info.capabilities.languages
                .supports("en-AU"),
        )
        assertTrue(
            !tts.info.capabilities.languages
                .supports("de"),
        )
    }

    @Test
    fun aReplyPlaysItsTextsInOrderThenEnds() {
        val synth = FakeSynth(all)
        val tts = engine(synth)
        assertEquals(AudioFormat(22_050, 1), tts.begin(TtsRequest(turn, voice = "milena", rate = SpeechRate(1.5f))))
        tts.pushText("ab")
        tts.pushText("cde")
        tts.finish()
        assertEquals(List(300) { 2f } + List(300) { 3f }, tts.drain())
        assertEquals(listOf("milena@1.5:ab", "milena@1.5:cde"), synth.spoken)
        assertEquals(TtsPoll.DONE, tts.poll(FloatArray(8)))

        // The engine speaks the next reply too.
        tts.begin(TtsRequest(turn, voice = "thomas"))
        tts.pushText("x")
        tts.finish()
        assertEquals(List(300) { 1f }, tts.drain())
    }

    @Test
    fun aVoiceIsPickedByIdThenByLanguage() {
        val tts = engine()

        fun rate(request: TtsRequest) = tts.begin(request).sampleRateHz
        assertEquals(48_000, rate(TtsRequest(turn, voice = "daniel", language = "ru")))
        assertEquals(22_050, rate(TtsRequest(turn, language = "ru-RU")))
        assertEquals(48_000, rate(TtsRequest(turn, language = "en-GB")))
        // No exact tag: the system's own voice among the language's, then the language default.
        assertEquals(22_050, rate(TtsRequest(turn, language = "en")))
        assertEquals(22_050, rate(TtsRequest(turn, language = "ru")))
        assertEquals(24_000, rate(TtsRequest(turn, language = "fr-CA")))
        assertEquals(22_050, rate(TtsRequest(turn)))
        assertFailsWith<SpeechException.Unsupported> { tts.begin(TtsRequest(turn, voice = "nobody")) }
        assertFailsWith<SpeechException.Unsupported> { tts.begin(TtsRequest(turn, language = "de")) }
    }

    @Test
    fun aFullRingHoldsTheHelperBack() {
        val synth = FakeSynth(all, chunks = 1_000, chunkSamples = 100)
        // 10 ms at 16 kHz: a ring of 160 samples.
        val tts = engine(synth, HelperTtsOptions(bufferMs = 10))
        tts.begin(TtsRequest(turn, voice = "yuri"))
        tts.pushText("go")
        tts.finish()
        runBlocking { delay(300) }
        // Without polls the ring and the pipe fill up and the synth is made to wait.
        assertTrue(synth.handed.load() < 100_000, "handed ${synth.handed.load()}")
        val got = tts.drain()
        assertEquals(100_000, got.size)
        assertTrue(got.all { it == 2f })
    }

    @Test
    fun aCancelledReplyNeverPlaysLate() {
        val gate = Mutex(locked = true)
        val tts = engine(FakeSynth(all, gate = gate))
        tts.begin(TtsRequest(turn, voice = "milena"))
        tts.pushText("first")
        tts.pushText("never")
        tts.cancel()
        assertEquals(TtsPoll.DONE, tts.poll(FloatArray(8)))

        tts.begin(TtsRequest(turn, voice = "milena"))
        tts.pushText("xy")
        tts.finish()
        gate.unlock()
        assertEquals(List(300) { 2f }, tts.drain())
    }

    @Test
    fun beginImpliesCancel() {
        val gate = Mutex(locked = true)
        val tts = engine(FakeSynth(all, gate = gate))
        tts.begin(TtsRequest(turn, voice = "milena"))
        tts.pushText("first")
        tts.begin(TtsRequest(turn, voice = "milena"))
        tts.pushText("xyz")
        tts.finish()
        gate.unlock()
        assertEquals(List(300) { 3f }, tts.drain())
    }

    @Test
    fun callsOutOfOrderAreRefused() {
        val tts = engine()
        assertEquals(TtsPoll.DONE, tts.poll(FloatArray(8)))
        assertFailsWith<SpeechException.OutOfOrder> { tts.pushText("hi") }
        assertFailsWith<SpeechException.OutOfOrder> { tts.finish() }
        tts.begin(TtsRequest(turn))
        tts.finish()
        assertFailsWith<SpeechException.OutOfOrder> { tts.pushText("late") }
    }

    @Test
    fun aFailingSynthFailsTheEngine() {
        val tts = engine()
        tts.begin(TtsRequest(turn, voice = "milena"))
        tts.pushText("boom")
        val e = tts.failure()
        assertTrue(e is SpeechException.Failed && "synth crashed" in e.message.orEmpty(), e.toString())
    }

    @Test
    fun replyTextIsCappedInTheHelper() {
        val gate = Mutex(locked = true)
        val tts = engine(FakeSynth(all, gate = gate), serve = TtsServeOptions(maxQueuedTextBytes = 16))
        tts.begin(TtsRequest(turn, voice = "milena"))
        tts.pushText("first")
        repeat(4) { tts.pushText("0123456789") }
        // The error goes out once the synth is free again.
        gate.unlock()
        val e = tts.failure()
        assertTrue(e is SpeechException.Failed && "cap" in e.message.orEmpty(), e.toString())
    }

    @Test
    fun aHelperThatDiesMidReplyFailsTheEngine() {
        val tts =
            HelperTts
                .start(
                    EngineId("system-test"),
                    "test voices",
                    link { input, output ->
                        output.writeVoice(enUs)
                        output.writeFrame(FrameType.READY, 0)
                        output.flush()
                        input.readFrame()
                        error("segfault")
                    },
                ).also { engines += it }
        tts.begin(TtsRequest(turn))
        val e = tts.failure()
        assertTrue(e is SpeechException.Failed && "segfault" in e.message.orEmpty(), e.toString())
    }

    @Test
    fun aHungHelperIsCaughtAfterFinish() {
        val time = TestTimeSource()
        val tts =
            HelperTts
                .start(
                    EngineId("system-test"),
                    "test voices",
                    link { input, output ->
                        output.writeVoice(enUs)
                        output.writeFrame(FrameType.READY, 0)
                        output.flush()
                        // Reads every request and never answers.
                        while (input.readFrame() != null) continue
                    },
                    HelperTtsOptions(stallTimeout = 5.seconds, time = time),
                ).also { engines += it }
        tts.begin(TtsRequest(turn))
        tts.pushText("hello")
        tts.finish()
        assertEquals(TtsPoll.PENDING, tts.poll(FloatArray(8)))
        time += 6.seconds
        assertFailsWith<SpeechException.Failed> { tts.poll(FloatArray(8)) }
    }

    @Test
    fun aHelperThatCannotStartIsUnavailable() {
        unavailable { _, _ -> error("cannot load the framework") }
        unavailable { _, output ->
            output.writeText(FrameType.ERROR, 0, "speech framework missing")
            output.flush()
        }
        unavailable { _, output ->
            output.writeFrame(FrameType.READY, 0)
            output.flush()
        }
        unavailable { _, output ->
            output.writeFrame(FrameType.END, 1)
            output.flush()
        }
        unavailable { input, _ -> input.readFrame() }
        val e =
            unavailable { _, output ->
                output.writeFrame(FrameType.VOICE, 0, byteArrayOf(1, 2, 3))
                output.flush()
            }
        assertTrue("protocol" in e.message.orEmpty(), e.toString())
    }

    @Test
    fun aHelperListingTooManyVoicesIsUnavailable() {
        unavailable { _, output ->
            repeat(MAX_HELPER_VOICES + 1) { output.writeVoice(HelperVoice(Voice("v$it", "Voice $it", "en"), 16_000)) }
            output.writeFrame(FrameType.READY, 0)
            output.flush()
        }
    }

    @Test
    fun longTextGoesOutInPiecesUnderTheCap() {
        val synth = FakeSynth(all, chunks = 1, chunkSamples = 1)
        val tts = engine(synth)
        tts.begin(TtsRequest(turn, voice = "milena"))
        tts.pushText("ж".repeat(40_000))
        tts.finish()
        assertEquals(2, tts.drain().size)
        assertEquals(listOf(32_768, 7_232), synth.spoken.map { it.substringAfter(':').length })
    }
}
