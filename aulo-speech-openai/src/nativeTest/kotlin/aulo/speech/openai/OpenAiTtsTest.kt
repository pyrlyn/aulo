package aulo.speech.openai

import aulo.speech.AudioFormat
import aulo.speech.EngineId
import aulo.speech.EngineSpec
import aulo.speech.LanguageSupport
import aulo.speech.SpeechException
import aulo.speech.SpeechRate
import aulo.speech.TtsEngine
import aulo.speech.TtsRequest
import aulo.speech.Voice
import aulo.types.TurnId
import aulo.voice.TtsRegistry
import http.HttpException
import http.HttpRequest
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TestTimeSource
import kotlin.time.TimeSource

class OpenAiTtsTest {
    private val secrets = MemorySecrets("openai-key" to KEY)

    private val voices = listOf(Voice("alloy", "Alloy", "en"), Voice("nova", "Nova", "ru"))

    private fun endpoint(
        key: String? = "openai-key",
        maxInputChars: Int = 4096,
        languages: LanguageSupport = LanguageSupport.Multilingual,
    ) = OpenAiTtsEndpoint(
        EngineId("openai-tts"),
        "OpenAI speech",
        "https://api.openai.test/v1",
        "gpt-4o-mini-tts",
        voices,
        key,
        languages,
        maxInputChars,
    )

    private fun engine(
        server: FakeTranscriber,
        endpoint: OpenAiTtsEndpoint = endpoint(),
        options: OpenAiTtsOptions = OpenAiTtsOptions(),
        model: String? = null,
    ): TtsEngine {
        val registry = TtsRegistry()
        registry.registerOpenAi(endpoint, server, secrets, options)
        return registry.build(EngineSpec(endpoint.engine, model))
    }

    private fun pcm(vararg samples: Int): ByteArray =
        ByteArray(samples.size * 2).also { out ->
            samples.forEachIndexed { i, s ->
                out[2 * i] = s.toByte()
                out[2 * i + 1] = (s shr 8).toByte()
            }
        }

    private fun audio(
        samples: ByteArray,
        chunk: Int = 3,
    ) = Reply(contentType = "audio/pcm", body = samples, chunk = chunk)

    private fun HttpRequest.json(): JsonObject = Json.parseToJsonElement(body.decodeToString()) as JsonObject

    private fun HttpRequest.input(): String = (json()["input"] as JsonPrimitive).content

    /** Says [texts] as one reply and returns every sample; whatever the engine throws passes through. */
    private fun TtsEngine.say(
        vararg texts: String,
        request: TtsRequest = TtsRequest(TurnId.new()),
    ): FloatArray =
        runBlocking {
            begin(request)
            texts.forEach { pushText(it) }
            finish()
            drain()
        }

    private suspend fun TtsEngine.drain(): FloatArray {
        val out = FloatArray(500)
        var all = FloatArray(0)
        val giveUp = TimeSource.Monotonic.markNow() + 10.seconds
        while (true) {
            val p = poll(out)
            when {
                p.isDone -> {
                    return all
                }

                p.isPending -> {
                    check(!giveUp.hasPassedNow()) { "the reply never ended" }
                    delay(1)
                }

                else -> {
                    all += out.copyOf(p.samples)
                }
            }
        }
    }

    private suspend fun FakeTranscriber.waitFor(count: Int) {
        val giveUp = TimeSource.Monotonic.markNow() + 10.seconds
        while (requests.size < count) {
            check(!giveUp.hasPassedNow()) { "no request $count" }
            delay(1)
        }
    }

    @Test
    fun aReplyStreamsPcmInOrder() {
        // Three bytes a read, so samples split across reads.
        val server = FakeTranscriber { _, _ -> audio(pcm(0, 16384, -32768, 32767, -1)) }
        val tts = engine(server)
        val turn = TurnId.new()
        val samples =
            runBlocking {
                assertEquals(AudioFormat(24_000, 1), tts.begin(TtsRequest(turn)))
                tts.pushText("Hello there.")
                tts.finish()
                tts.drain()
            }
        assertContentEquals(floatArrayOf(0f, 0.5f, -1f, 32767f / 32768f, -1f / 32768f), samples)
        val req = server.requests.single()
        assertEquals("https://api.openai.test/v1/audio/speech", req.url)
        assertTrue("Authorization" to "Bearer $KEY" in req.headers)
        assertTrue("Content-Type" to "application/json" in req.headers)
        val json = req.json()
        assertEquals(setOf("model", "input", "voice", "response_format"), json.keys)
        assertEquals("gpt-4o-mini-tts", (json["model"] as JsonPrimitive).content)
        assertEquals("Hello there.", req.input())
        assertEquals("alloy", (json["voice"] as JsonPrimitive).content)
        assertEquals("pcm", (json["response_format"] as JsonPrimitive).content)
    }

    @Test
    fun theVoiceFollowsTheLanguageUnlessOneIsAsked() {
        val server = FakeTranscriber { _, _ -> audio(pcm(1)) }
        val tts = engine(server, model = "tts-1")
        tts.say("Привет.", request = TtsRequest(TurnId.new(), language = "ru-RU"))
        tts.say("Hello.", request = TtsRequest(TurnId.new(), voice = "nova", language = "en"))
        tts.say("Hallo.", request = TtsRequest(TurnId.new(), language = "de"))
        assertEquals(listOf("nova", "nova", "alloy"), server.requests.map { (it.json()["voice"] as JsonPrimitive).content })
        assertEquals("tts-1", (server.requests[0].json()["model"] as JsonPrimitive).content)
    }

    @Test
    fun anUnknownVoiceOrLanguageIsUnsupported() {
        val english = endpoint(languages = LanguageSupport.Listed(listOf("en")))
        val tts = engine(FakeTranscriber { _, _ -> error("no request expected") }, english)
        assertFailsWith<SpeechException.Unsupported> { tts.begin(TtsRequest(TurnId.new(), voice = "echo")) }
        assertFailsWith<SpeechException.Unsupported> { tts.begin(TtsRequest(TurnId.new(), language = "ru")) }
    }

    @Test
    fun aRateOtherThanNormalIsSentAsSpeed() {
        val server = FakeTranscriber { _, _ -> audio(pcm(1)) }
        engine(server).say("Fast.", request = TtsRequest(TurnId.new(), rate = SpeechRate(1.5f)))
        assertEquals("1.5", (server.requests.single().json()["speed"] as JsonPrimitive).content)
    }

    @Test
    fun sentencesQueuedDuringARequestGoInTheNextOne() =
        runBlocking {
            val server = FakeTranscriber { i, _ -> audio(pcm(i + 1)) }
            val gate = CompletableDeferred<Unit>()
            server.gate = gate
            val tts = engine(server)
            tts.begin(TtsRequest(TurnId.new()))
            tts.pushText("One.")
            server.waitFor(1)
            tts.pushText("Two.")
            tts.pushText(" Three.")
            tts.pushText("Four.")
            tts.finish()
            gate.complete(Unit)
            assertContentEquals(floatArrayOf(1 / 32768f, 2 / 32768f), tts.drain())
            assertEquals(listOf("One.", "Two. Three. Four."), server.requests.map { it.input() })
        }

    @Test
    fun textOverTheInputCapIsCutAtWhitespace() {
        val server = FakeTranscriber { _, _ -> audio(pcm(1)) }
        val words = (1..120).joinToString(" ") { "word$it" }
        engine(server, endpoint(maxInputChars = 256)).say(words)
        val inputs = server.requests.map { it.input() }
        assertTrue(inputs.size > 1)
        assertTrue(inputs.all { it.length <= 256 }, inputs.toString())
        assertEquals(words, inputs.joinToString(" "))
    }

    @Test
    fun aCutNeverSplitsASurrogatePairOrLeavesAScrap() {
        val emoji = charArrayOf(0xD83D.toChar(), 0xDE00.toChar()).concatToString()
        assertEquals(9, cutAt("abcdefghi" + emoji, 10))
        assertEquals(10, cutAt("abcdefghij" + "klm", 10))
        // Whitespace only in the first half is ignored: the cut would leave a scrap.
        assertEquals(10, cutAt("ab cdefghijklm", 10))
        assertEquals(7, cutAt("abcdef ghijklm", 10))
        assertEquals(4, cutAt("abcd", 10))
    }

    @Test
    fun blankTextSendsNothing() {
        val server = FakeTranscriber { _, _ -> error("no request expected") }
        assertEquals(0, engine(server).say("  ", "\n").size)
        assertTrue(server.requests.isEmpty())
    }

    @Test
    fun anErrorStatusFailsWithItsMessage() {
        val server =
            FakeTranscriber { i, _ ->
                val status = if (i == 0) 401 else 500
                Reply(status, text = """{"error":{"message":"bad key $status"}}""")
            }
        val tts = engine(server)
        val bad = assertFailsWith<SpeechException.Unavailable> { tts.say("Hi.") }
        assertTrue("HTTP 401: bad key 401" in bad.message!!, bad.message)
        val down = assertFailsWith<SpeechException.Failed> { tts.say("Hi.") }
        assertTrue("HTTP 500: bad key 500" in down.message!!, down.message)
    }

    @Test
    fun aJsonAnswerWithA200Fails() {
        val server = FakeTranscriber { _, _ -> Reply(text = """{"error":{"message":"unknown voice"}}""") }
        val e = assertFailsWith<SpeechException.Failed> { engine(server).say("Hi.") }
        assertTrue("unknown voice" in e.message!!, e.message)
    }

    @Test
    fun anAnswerInAnotherAudioFormatFails() {
        val server = FakeTranscriber { _, _ -> Reply(contentType = "audio/mpeg", body = ByteArray(10)) }
        val e = assertFailsWith<SpeechException.Failed> { engine(server).say("Hi.") }
        assertTrue("audio/mpeg, not raw PCM" in e.message!!, e.message)
    }

    @Test
    fun aTransportFailureFails() {
        val server = FakeTranscriber { _, _ -> Reply(failure = HttpException(HttpException.Kind.Connect, "connection refused")) }
        val e = assertFailsWith<SpeechException.Failed> { engine(server).say("Hi.") }
        assertTrue("Connect: connection refused" in e.message!!, e.message)
    }

    @Test
    fun aMissingKeyIsUnavailableBeforeAnyRequest() {
        val server = FakeTranscriber { _, _ -> error("no request expected") }
        val e = assertFailsWith<SpeechException.Unavailable> { engine(server, endpoint(key = "other-key")).say("Hi.") }
        assertTrue("API key `other-key` is not in the test store" in e.message!!, e.message)
        assertTrue(server.requests.isEmpty())
    }

    @Test
    fun anAnswerPastItsCapFails() {
        // 100 ms at 24 kHz is 2400 samples, 4800 bytes.
        val server = FakeTranscriber { _, _ -> audio(ByteArray(10_000), chunk = 1000) }
        val tts = engine(server, options = OpenAiTtsOptions(maxAudioPerRequest = 100.milliseconds))
        val e = assertFailsWith<SpeechException.Failed> { tts.say("Hi.") }
        assertTrue("over 100ms of audio" in e.message!!, e.message)
    }

    @Test
    fun aFullRingStopsTheReadUntilPollDrainsIt() =
        runBlocking {
            // 100 ms of ring is 2400 samples; the answer is 1 s.
            val server = FakeTranscriber { _, _ -> audio(pcm(*IntArray(24_000) { it % 100 }), chunk = 1000) }
            val tts = engine(server, options = OpenAiTtsOptions(bufferedAudio = 100.milliseconds))
            tts.begin(TtsRequest(TurnId.new()))
            tts.pushText("A long sentence.")
            tts.finish()
            server.waitFor(1)
            delay(200)
            assertTrue(server.served <= 2400 * 2 + 1000, "read ${server.served} bytes into a 4800-byte ring")
            val all = tts.drain()
            assertEquals(24_000, all.size)
            assertEquals(99 / 32768f, all[23_999])
        }

    @Test
    fun aCancelledRepliesAudioNeverReachesTheNextOne() =
        runBlocking {
            val server = FakeTranscriber { i, _ -> audio(pcm(*IntArray(100) { i + 1 })) }
            val gate = CompletableDeferred<Unit>()
            server.gate = gate
            val tts = engine(server)
            tts.begin(TtsRequest(TurnId.new()))
            tts.pushText("First.")
            tts.finish()
            server.waitFor(1)
            tts.cancel()
            assertTrue(tts.poll(FloatArray(10)).isDone)
            tts.begin(TtsRequest(TurnId.new()))
            tts.pushText("Second.")
            tts.finish()
            gate.complete(Unit)
            val all = tts.drain()
            assertEquals(100, all.size)
            assertTrue(all.all { it == 2 / 32768f })
        }

    @Test
    fun aServerThatSendsNothingStalls() =
        runBlocking {
            val time = TestTimeSource()
            val server = FakeTranscriber { _, _ -> audio(pcm(1)) }
            server.gate = CompletableDeferred()
            val tts = engine(server, options = OpenAiTtsOptions(stallTimeout = 1.seconds, time = time))
            tts.begin(TtsRequest(TurnId.new()))
            tts.pushText("Hi.")
            tts.finish()
            server.waitFor(1)
            assertTrue(tts.poll(FloatArray(10)).isPending)
            time += 2.seconds
            val e = assertFailsWith<SpeechException.Failed> { tts.poll(FloatArray(10)) }
            assertTrue("no audio for 1s" in e.message!!, e.message)
            assertTrue(tts.poll(FloatArray(10)).isDone)
        }

    @Test
    fun waitingForTheNextTextIsNotAStall() =
        runBlocking {
            val time = TestTimeSource()
            val server = FakeTranscriber { _, _ -> audio(pcm(1, 2)) }
            val tts = engine(server, options = OpenAiTtsOptions(stallTimeout = 1.seconds, time = time))
            tts.begin(TtsRequest(TurnId.new()))
            tts.pushText("First.")
            server.waitFor(1)
            val out = FloatArray(10)
            var got = 0
            val giveUp = TimeSource.Monotonic.markNow() + 10.seconds
            while (got < 2) {
                check(!giveUp.hasPassedNow()) { "the first answer never arrived" }
                val p = tts.poll(out)
                if (p.isPending) delay(1) else got += p.samples
            }
            // The first answer is fully played; the caller takes its time with the next sentence.
            delay(50)
            time += 5.seconds
            assertTrue(tts.poll(out).isPending)
            tts.pushText("Second.")
            tts.finish()
            assertEquals(2, tts.drain().size)
            assertEquals(listOf("First.", "Second."), server.requests.map { it.input() })
        }

    @Test
    fun textOutsideAReplyOrPastItsCapIsRefused() {
        val tts = engine(FakeTranscriber { _, _ -> audio(pcm(1)) }, options = OpenAiTtsOptions(maxReplyChars = 10))
        assertFailsWith<SpeechException.OutOfOrder> { tts.pushText("Hi.") }
        tts.begin(TtsRequest(TurnId.new()))
        tts.pushText("0123456789")
        assertFailsWith<SpeechException.Invalid> { tts.pushText("x") }
        tts.finish()
        assertFailsWith<SpeechException.OutOfOrder> { tts.pushText("Hi.") }
        assertFailsWith<SpeechException.OutOfOrder> { tts.finish() }
    }

    @Test
    fun aClosedEngineIsUnavailable() {
        val tts = engine(FakeTranscriber { _, _ -> audio(pcm(1)) })
        tts.close()
        assertFailsWith<SpeechException.Unavailable> { tts.begin(TtsRequest(TurnId.new())) }
    }

    @Test
    fun anEndpointIsChecked() {
        fun make(
            url: String = "https://api.openai.test/v1",
            voices: List<Voice> = this.voices,
            key: String? = "openai-key",
            maxInputChars: Int = 4096,
        ) = OpenAiTtsEndpoint(EngineId("tts"), "TTS", url, "tts-1", voices, key, maxInputChars = maxInputChars)
        assertFailsWith<SpeechException.Invalid> { make(voices = emptyList()) }
        assertFailsWith<SpeechException.Invalid> { make(voices = List(MAX_TTS_VOICES + 1) { Voice("v$it", "V$it") }) }
        assertFailsWith<SpeechException.Invalid> { make(voices = listOf(Voice("a", "A"), Voice("a", "B"))) }
        assertFailsWith<SpeechException.Invalid> { make(maxInputChars = 10) }
        assertFailsWith<SpeechException.Invalid> { make(url = "http://192.0.2.5/v1") }
        assertFailsWith<SpeechException.Invalid> { make(url = "https://api.test/v1?key=x") }
        val local = make(url = "http://localhost:8880/v1", key = null)
        assertTrue(local.info.capabilities.offline)
        assertTrue(local.info.capabilities.streaming)
        assertEquals("http://localhost:8880/v1/audio/speech", local.url)
        assertFalse(make().info.capabilities.offline)
    }

    private companion object {
        const val KEY = "sk-test-0123456789"
    }
}
