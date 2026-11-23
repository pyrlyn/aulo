package aulo.speech.helper

import aulo.speech.AudioFormat
import aulo.speech.AudioFrame
import aulo.speech.Capabilities
import aulo.speech.EngineId
import aulo.speech.EngineInfo
import aulo.speech.EngineKind
import aulo.speech.LanguageSupport
import aulo.speech.SpeechException
import aulo.speech.SttPoll
import aulo.speech.Transcript
import aulo.testkit.FRAME_SAMPLES
import aulo.testkit.frame
import aulo.testkit.loud
import aulo.types.TranscriptKind
import aulo.types.TurnId
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.sync.Mutex
import kotlin.test.AfterTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TestTimeSource

class HelperSttTest {
    private val info =
        EngineInfo(
            EngineId("sherpa-test"),
            EngineKind.STT,
            "test",
            Capabilities(streaming = true, languages = LanguageSupport.Listed(listOf("en", "ru")), offline = true, needsNetwork = false),
        )
    private val turn = TurnId.new()
    private val turn2 = TurnId.new()
    private val engines = mutableListOf<HelperStt>()

    @AfterTest
    fun closeEngines() = engines.forEach { it.close() }

    private fun engine(
        options: HelperOptions = HelperOptions(),
        chunks: Int = 64,
        body: (okio.BufferedSource, okio.BufferedSink) -> Unit,
    ) = HelperStt(info, MemoryLink(chunks, body), options).also { engines += it }

    private fun serving(
        serve: ServeOptions = ServeOptions(),
        decoder: UtteranceDecoder = UtteranceDecoder { _, count, language -> "heard $count $language" },
    ) = engine { input, output -> serveStt(input, output, decoder, serve) }

    private fun HelperStt.pollUntilChanged(out: Transcript): SttPoll = eventually { poll(out).takeIf { it != SttPoll.PENDING } }

    private fun HelperStt.failure(): SpeechException =
        eventually {
            try {
                poll(Transcript(turn))
                null
            } catch (e: SpeechException) {
                e
            }
        }

    @Test
    fun anUtteranceEndsInOneFinal() {
        val stt = serving()
        val out = Transcript(turn)
        stt.begin(turn, "ru")
        repeat(3) { assertEquals(0, stt.push(frame(loud, it))) }
        stt.finish()
        assertEquals(SttPoll.UPDATED, stt.pollUntilChanged(out))
        assertEquals(TranscriptKind.FINAL, out.kind)
        assertEquals("heard ${3 * FRAME_SAMPLES} ru", out.text.toString())
        assertEquals("ru", out.language)
        assertEquals(turn, out.turnId)
        assertEquals(SttPoll.DONE, stt.poll(out))
        assertTrue(stt.isReady)

        // The engine serves the next utterance too.
        stt.begin(turn2, null)
        stt.push(frame(loud, 0))
        stt.finish()
        stt.pollUntilChanged(out)
        assertEquals("heard $FRAME_SAMPLES null", out.text.toString())
    }

    @Test
    fun partialsArriveBeforeFinish() {
        val stt = serving(ServeOptions(partialEverySamples = FRAME_SAMPLES))
        val out = Transcript(turn)
        stt.begin(turn, "en")
        stt.push(frame(loud, 0))
        assertEquals(SttPoll.UPDATED, stt.pollUntilChanged(out))
        assertEquals(TranscriptKind.PARTIAL, out.kind)
        stt.finish()
        eventually { out.takeIf { stt.poll(it) == SttPoll.UPDATED && it.kind == TranscriptKind.FINAL } }
    }

    @Test
    fun aCancelledUtteranceNeverReportsLate() {
        // The decoder holds the first utterance's final until the test lets it go.
        val gate = Mutex(locked = true)
        val stt =
            serving(
                decoder = { _, count, _ ->
                    if (count == FRAME_SAMPLES) runBlocking { gate.lock() }
                    "count $count"
                },
            )
        val out = Transcript(turn)
        stt.begin(turn, "en")
        stt.push(frame(loud, 0))
        stt.finish()
        stt.cancel()
        stt.begin(turn2, "en")
        stt.push(frame(loud, 0))
        stt.push(frame(loud, 1))
        gate.unlock()
        stt.finish()
        assertEquals(SttPoll.UPDATED, stt.pollUntilChanged(out))
        assertEquals("count ${2 * FRAME_SAMPLES}", out.text.toString())
        assertEquals(turn2, out.turnId)
    }

    @Test
    fun callOrderAndInputsAreChecked() {
        val stt = serving()
        assertFailsWith<SpeechException.OutOfOrder> { stt.push(frame(loud, 0)) }
        assertFailsWith<SpeechException.OutOfOrder> { stt.finish() }
        assertEquals(SttPoll.PENDING, stt.poll(Transcript(turn)))
        assertFailsWith<SpeechException.Unsupported> { stt.begin(turn, "de") }
        stt.begin(turn, "en-GB")
        assertFailsWith<SpeechException.Unsupported> { stt.push(AudioFrame(FloatArray(480), AudioFormat(48_000, 1), 0)) }
        stt.finish()
        assertFailsWith<SpeechException.OutOfOrder> { stt.push(frame(loud, 0)) }
    }

    @Test
    fun aFullQueueDropsAudioAndCountsIt() {
        // A helper that never reads: the OS pipe (here one chunk) fills and the writer stalls.
        val stall = Mutex(locked = true)
        val stt = engine(HelperOptions(queueFrames = 4), chunks = 1) { _, _ -> runBlocking { stall.lock() } }
        stt.begin(turn, "en")
        // At most 4 queued, plus two bursts of at most 4 the writer took before it stalled.
        val dropped = (0 until 100).sumOf { stt.push(frame(loud, it)) }
        assertTrue(dropped >= (100 - 12) * FRAME_SAMPLES, "dropped $dropped")
        stall.unlock()
    }

    @Test
    fun aHelperThatCannotLoadItsModelIsUnavailable() {
        val stt =
            engine { _, output ->
                output.writeText(FrameType.ERROR, 0, "no such file: encoder.int8.onnx")
                output.flush()
            }
        val e = stt.failure()
        assertTrue(e is SpeechException.Unavailable && "encoder.int8.onnx" in e.message!!, e.message)
        assertTrue(e.shouldFallBack)
    }

    @Test
    fun aHelperThatExitsBeforeReadyIsUnavailableAndAfterItFailed() {
        assertTrue(engine { _, _ -> error("segfault") }.failure() is SpeechException.Unavailable)

        val stt =
            engine { input, output ->
                output.writeFrame(FrameType.READY, 0)
                output.flush()
                input.readFrame()
                throw IllegalStateException("killed by signal 11")
            }
        eventually { stt.isReady.takeIf { it } }
        stt.begin(turn, "en")
        val e = stt.failure()
        assertTrue(e is SpeechException.Failed && "signal 11" in e.message!!, e.message)
        assertFailsWith<SpeechException.Failed> { stt.begin(turn, "en") }
    }

    @Test
    fun aHelperThatBreaksTheProtocolFails() {
        val stt =
            engine { _, output ->
                output.writeFrame(FrameType.READY, 0)
                // A FINAL whose header claims far more than the cap.
                output.writeByte(FrameType.FINAL.code.toInt())
                output.writeIntLe(1)
                output.writeIntLe(Int.MAX_VALUE)
                output.flush()
            }
        val e = stt.failure()
        assertTrue(e is SpeechException.Failed && "protocol" in e.message!!, e.message)
    }

    @Test
    fun aHelperErrorFailsTheEngine() {
        val stt = serving(decoder = { _, _, _ -> error("onnxruntime: bad alloc") })
        stt.begin(turn, "en")
        stt.push(frame(loud, 0))
        stt.finish()
        val e = stt.failure()
        assertTrue(e is SpeechException.Failed && "bad alloc" in e.message!!, e.message)
    }

    @Test
    fun aHungHelperFailsAfterTheTimeout() {
        val time = TestTimeSource()
        val hang = Mutex(locked = true)
        val stt =
            engine(HelperOptions(finalTimeout = 10.seconds, time = time)) { input, output ->
                serveStt(input, output, { _, _, _ -> runBlocking { hang.lock() }.let { "never" } })
            }
        stt.begin(turn, "en")
        stt.push(frame(loud, 0))
        stt.finish()
        val out = Transcript(turn)
        assertEquals(SttPoll.PENDING, stt.poll(out))
        time += 11.seconds
        assertFailsWith<SpeechException.Failed> { stt.poll(out) }
        assertFailsWith<SpeechException.Failed> { stt.begin(turn, "en") }
        hang.unlock()
    }

    @Test
    fun closeIsFinal() {
        val stt = serving()
        stt.close()
        stt.close()
        assertFailsWith<SpeechException.Unavailable> { stt.begin(turn, "en") }
    }
}
