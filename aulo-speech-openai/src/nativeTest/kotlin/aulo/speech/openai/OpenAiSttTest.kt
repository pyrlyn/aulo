package aulo.speech.openai

import aulo.speech.EngineId
import aulo.speech.EngineSpec
import aulo.speech.SpeechException
import aulo.speech.SttPoll
import aulo.speech.Transcript
import aulo.types.TranscriptKind
import aulo.types.TurnId
import aulo.voice.SttRegistry
import http.HttpException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TestTimeSource

private const val KEY = "sk-test-0123456789"

class OpenAiSttTest {
    private val secrets = MemorySecrets("openai-key" to KEY)

    private fun endpoint(
        stream: Boolean = true,
        model: String = "gpt-transcribe",
        key: String? = "openai-key",
    ) = OpenAiEndpoint(EngineId("openai"), "OpenAI", "https://api.openai.test/v1", model, key, stream)

    private fun engine(
        server: FakeTranscriber,
        endpoint: OpenAiEndpoint = endpoint(),
        options: OpenAiSttOptions = OpenAiSttOptions(),
        model: String? = null,
    ): OpenAiStt {
        val registry = SttRegistry()
        registry.registerOpenAi(endpoint, server, secrets, options)
        return registry.build(EngineSpec(endpoint.engine, model)) as OpenAiStt
    }

    private fun sse(vararg events: String) = events.joinToString("") { "data: $it\n\n" }

    /** One utterance of [samples] samples; the transcript it ended with and every update on the way. */
    private fun OpenAiStt.say(
        samples: Int = 1600,
        language: String? = null,
        turn: TurnId = TurnId.new(),
    ): Pair<Transcript, List<Pair<TranscriptKind, String>>> =
        runBlocking {
            val out = Transcript(turn)
            begin(turn, language)
            if (samples > 0) push(frame(samples))
            finish()
            out to updates(out)
        }

    @Test
    fun aJsonAnswerIsTheFinal() {
        val server = FakeTranscriber { _, _ -> Reply(text = """{"text":"hello there","usage":{"type":"duration","seconds":1}}""") }
        val (out, seen) = engine(server, endpoint(stream = false, model = "whisper-large-v3")).say(language = "en-US")
        assertEquals(listOf(TranscriptKind.FINAL to "hello there"), seen)
        assertEquals("en-US", out.language)
        val req = server.requests.single()
        assertEquals("https://api.openai.test/v1/audio/transcriptions", req.url)
        assertTrue("Authorization" to "Bearer $KEY" in req.headers)
        assertEquals(
            listOf("model" to "whisper-large-v3", "response_format" to "json", "language" to "en"),
            req.formFields(),
        )
        val wav = req.wavAt()
        assertEquals(16_000, req.body.u32(wav + 24))
        assertEquals(1600 * 2, req.body.u32(wav + 40))
        // 0.5 as 16-bit PCM, little endian.
        assertContentEquals(byteArrayOf(0xff.toByte(), 0x3f), req.body.copyOfRange(wav + 44, wav + 46))
    }

    @Test
    fun aStreamGivesPartialsThenTheFinal() {
        val body =
            ": keep-alive\r\n\r\n" +
                "data: {\"type\":\"transcript.text.delta\",\"delta\":\"Bon\"}\r\n\r\n" +
                sse(
                    """{"type":"transcript.text.delta","delta":"jour à"}""",
                    """{"type":"transcript.text.segment","id":"s1"}""",
                    """{"type":"transcript.text.done","text":"Bonjour à tous.","languages":[{"code":"fr"}]}""",
                )
        // Three-byte reads split lines, events and the two bytes of à.
        val server = FakeTranscriber { _, _ -> Reply(contentType = "text/event-stream; charset=utf-8", text = body, chunk = 3) }
        val (out, seen) = engine(server).say(language = "fr-CA")
        assertEquals(TranscriptKind.FINAL to "Bonjour à tous.", seen.last())
        assertEquals("fr", out.language)
        seen.dropLast(1).forEach { (kind, text) ->
            assertEquals(TranscriptKind.PARTIAL, kind)
            assertTrue(text == "Bon" || text == "Bonjour à", text)
        }
        // GPT Transcribe takes the language as a list.
        assertEquals(
            listOf("model" to "gpt-transcribe", "response_format" to "json", "languages[]" to "fr", "stream" to "true"),
            server.requests.single().formFields(),
        )
    }

    @Test
    fun aServerThatCannotStreamAnswersWithJson() {
        val server = FakeTranscriber { _, _ -> Reply(text = """{"text":"plain","language":"de"}""") }
        val (out, seen) = engine(server).say()
        assertEquals(listOf(TranscriptKind.FINAL to "plain"), seen)
        assertEquals("de", out.language)
    }

    @Test
    fun theDoneSentinelOfChatStyleServersEndsTheStream() {
        val server =
            FakeTranscriber { _, _ ->
                Reply(contentType = "text/event-stream", text = sse("""{"type":"transcript.text.done","text":"ok"}""", "[DONE]"))
            }
        assertEquals(TranscriptKind.FINAL to "ok", engine(server).say().second.last())
    }

    @Test
    fun aStreamThatEndsWithoutItsFinalEventFails() {
        val server =
            FakeTranscriber { _, _ ->
                Reply(contentType = "text/event-stream", text = sse("""{"type":"transcript.text.delta","delta":"half"}""", "[DONE]"))
            }
        val e = assertFailsWith<SpeechException.Failed> { engine(server).say() }
        assertTrue("ended before transcript.text.done" in e.message!!, e.message)
    }

    @Test
    fun aBadKeyIsUnavailableWithTheServersMessage() {
        val server =
            FakeTranscriber { _, _ ->
                Reply(status = 401, text = """{"error":{"message":"Incorrect API key provided","type":"invalid_request_error"}}""")
            }
        val e = assertFailsWith<SpeechException.Unavailable> { engine(server).say() }
        assertTrue("HTTP 401: Incorrect API key provided" in e.message!!, e.message)
        assertFalse(KEY in e.message!!)
    }

    @Test
    fun aServerErrorFailsTheUtteranceAndTheNextOneTriesAgain() {
        val server =
            FakeTranscriber { i, _ ->
                if (i ==
                    0
                ) {
                    Reply(status = 503, contentType = "text/plain", text = "overloaded")
                } else {
                    Reply(text = """{"text":"back"}""")
                }
            }
        val stt = engine(server)
        val e = assertFailsWith<SpeechException.Failed> { stt.say() }
        assertTrue("HTTP 503: overloaded" in e.message!!, e.message)
        assertEquals(listOf(TranscriptKind.FINAL to "back"), stt.say().second)
    }

    @Test
    fun anErrorBodyIsReadOnlyUpToItsCap() {
        val server =
            FakeTranscriber {
                _,
                _,
                ->
                Reply(status = 500, contentType = "text/plain", body = ByteArray(1 shl 20) { 'x'.code.toByte() }, chunk = 1000)
            }
        assertFailsWith<SpeechException.Failed> { engine(server, options = OpenAiSttOptions(maxErrorBytes = 2048)).say() }
        assertTrue(server.served <= 2048 + 1000, "read ${server.served} bytes")
    }

    @Test
    fun aJsonAnswerOverItsCapFails() {
        val server = FakeTranscriber { _, _ -> Reply(text = """{"text":"${"a".repeat(2000)}"}""", chunk = 100) }
        val e =
            assertFailsWith<SpeechException.Failed> {
                engine(
                    server,
                    options = OpenAiSttOptions(maxAnswerBytes = 1024, maxEventBytes = 256),
                ).say()
            }
        assertTrue("answer is over 1024 bytes" in e.message!!, e.message)
        assertTrue(server.served <= 1024 + 100)
    }

    @Test
    fun anEventLineOverItsCapFails() {
        val server =
            FakeTranscriber { _, _ ->
                Reply(contentType = "text/event-stream", text = sse("""{"type":"transcript.text.delta","delta":"${"a".repeat(300)}"}"""))
            }
        val e =
            assertFailsWith<SpeechException.Failed> {
                engine(
                    server,
                    options = OpenAiSttOptions(maxAnswerBytes = 1024, maxEventBytes = 256),
                ).say()
            }
        assertTrue("event line is over 256 bytes" in e.message!!, e.message)
    }

    @Test
    fun anEventSpreadOverManyLinesIsCappedToo() {
        val lines = (1..20).joinToString("") { "data: ${"b".repeat(20)}\n" } + "\n"
        val server = FakeTranscriber { _, _ -> Reply(contentType = "text/event-stream", text = lines) }
        val e =
            assertFailsWith<SpeechException.Failed> {
                engine(
                    server,
                    options = OpenAiSttOptions(maxAnswerBytes = 1024, maxEventBytes = 256),
                ).say()
            }
        assertTrue("event is over 256 bytes" in e.message!!, e.message)
    }

    @Test
    fun manySmallEventsOverTheAnswerCapFail() {
        val deltas = Array(100) { """{"type":"transcript.text.delta","delta":"word "}""" }
        val server = FakeTranscriber { _, _ -> Reply(contentType = "text/event-stream", text = sse(*deltas)) }
        val e =
            assertFailsWith<SpeechException.Failed> {
                engine(
                    server,
                    options = OpenAiSttOptions(maxAnswerBytes = 1024, maxEventBytes = 256),
                ).say()
            }
        assertTrue("answer is over 1024 bytes" in e.message!!, e.message)
    }

    @Test
    fun anErrorEventFails() {
        val server =
            FakeTranscriber {
                _,
                _,
                ->
                Reply(contentType = "text/event-stream", text = sse("""{"type":"error","error":{"message":"audio too short"}}"""))
            }
        val e = assertFailsWith<SpeechException.Failed> { engine(server).say() }
        assertTrue("audio too short" in e.message!!, e.message)
    }

    @Test
    fun anAnswerThatIsNotJsonFails() {
        val server = FakeTranscriber { _, _ -> Reply(contentType = "text/html", text = "<html>proxy login</html>") }
        val e = assertFailsWith<SpeechException.Failed> { engine(server).say() }
        assertTrue("answer is not a JSON object" in e.message!!, e.message)
    }

    @Test
    fun aMissingKeyIsUnavailableBeforeAnyRequest() {
        val server = FakeTranscriber { _, _ -> error("no request expected") }
        val e = assertFailsWith<SpeechException.Unavailable> { engine(server, endpoint(key = "other-key")).say() }
        assertTrue("API key `other-key` is not in the test store" in e.message!!, e.message)
        assertTrue(server.requests.isEmpty())
    }

    @Test
    fun aServerWithoutAKeyGetsNoAuthorizationHeader() {
        val server = FakeTranscriber { _, _ -> Reply(text = """{"text":"local"}""") }
        val local = OpenAiEndpoint(EngineId("local-stt"), "Local", "http://127.0.0.1:8080/v1", "whisper", stream = false)
        engine(server, local).say()
        assertTrue(
            server.requests
                .single()
                .headers
                .none { it.first == "Authorization" },
        )
    }

    @Test
    fun aTransportFailureFails() {
        val server = FakeTranscriber { _, _ -> Reply(failure = HttpException(HttpException.Kind.Connect, "connection refused")) }
        val e = assertFailsWith<SpeechException.Failed> { engine(server).say() }
        assertTrue("Connect: connection refused" in e.message!!, e.message)
    }

    @Test
    fun audioPastItsCapIsDroppedAndCounted() {
        val server = FakeTranscriber { _, _ -> Reply(text = """{"text":"capped"}""") }
        val stt = engine(server, options = OpenAiSttOptions(maxAudio = 100.milliseconds))
        val turn = TurnId.new()
        stt.begin(turn, null)
        assertEquals(0, stt.push(frame(1000)))
        assertEquals(400, stt.push(frame(1000)))
        assertEquals(1000, stt.push(frame(1000)))
        stt.finish()
        runBlocking { stt.updates(Transcript(turn)) }
        val req = server.requests.single()
        assertEquals(1600 * 2, req.body.u32(req.wavAt() + 40))
    }

    @Test
    fun silenceIsAnEmptyFinalWithoutARequest() {
        val server = FakeTranscriber { _, _ -> error("no request expected") }
        assertEquals(listOf(TranscriptKind.FINAL to ""), engine(server).say(samples = 0).second)
        assertTrue(server.requests.isEmpty())
    }

    @Test
    fun aCancelledUtterancesAnswerNeverReachesTheNextOne() =
        runBlocking {
            val server = FakeTranscriber { i, _ -> Reply(text = """{"text":"${if (i == 0) "first" else "second"}"}""") }
            val gate = CompletableDeferred<Unit>()
            server.gate = gate
            val stt = engine(server)
            stt.begin(TurnId.new(), null)
            stt.push(frame(1600))
            stt.finish()
            while (server.requests.isEmpty()) delay(1)
            stt.cancel()
            val second = TurnId.new()
            val out = Transcript(second)
            stt.begin(second, null)
            stt.push(frame(1600))
            stt.finish()
            gate.complete(Unit)
            assertEquals(listOf(TranscriptKind.FINAL to "second"), stt.updates(out))
            assertEquals(second, out.turnId)
        }

    @Test
    fun noFinalInTimeFails() {
        val server = FakeTranscriber { _, _ -> Reply(text = """{"text":"late"}""") }
        server.gate = CompletableDeferred()
        val time = TestTimeSource()
        val stt = engine(server, options = OpenAiSttOptions(finalTimeout = 30.seconds, time = time))
        val turn = TurnId.new()
        stt.begin(turn, null)
        stt.push(frame(160))
        stt.finish()
        val out = Transcript(turn)
        assertEquals(SttPoll.PENDING, stt.poll(out))
        time += 31.seconds
        val e = assertFailsWith<SpeechException.Failed> { stt.poll(out) }
        assertTrue("no transcript within 30s" in e.message!!, e.message)
        stt.close()
    }

    @Test
    fun aSpecModelReplacesTheDefault() {
        val server = FakeTranscriber { _, _ -> Reply(text = """{"text":"x"}""") }
        engine(server, model = "gpt-4o-mini-transcribe").say()
        assertEquals(
            "gpt-4o-mini-transcribe",
            server.requests
                .single()
                .formFields()
                .first()
                .second,
        )
    }

    @Test
    fun aClosedEngineIsUnavailable() {
        val stt = engine(FakeTranscriber { _, _ -> Reply(text = "{}") })
        stt.close()
        assertFailsWith<SpeechException.Unavailable> { stt.begin(TurnId.new(), null) }
    }

    @Test
    fun aLanguageOutsideTheListIsUnsupported() {
        val listed = endpoint().copy(languages = aulo.speech.LanguageSupport.Listed(listOf("en")))
        val stt = engine(FakeTranscriber { _, _ -> Reply(text = "{}") }, listed)
        assertFailsWith<SpeechException.Unsupported> { stt.begin(TurnId.new(), "ru") }
    }
}

class OpenAiEndpointTest {
    private fun endpoint(
        url: String,
        key: String? = "k",
    ) = OpenAiEndpoint(EngineId("e"), "E", url, "m", key)

    @Test
    fun aKeyGoesOverHttpsOrToThisMachineOnly() {
        assertFalse(endpoint("https://api.openai.com/v1").isLoopback)
        assertTrue(endpoint("http://127.0.0.1:8080/v1").isLoopback)
        assertTrue(endpoint("http://localhost/v1").isLoopback)
        assertTrue(endpoint("http://[::1]:9000/v1").isLoopback)
        // 192.0.2.0/24 is reserved for documentation (RFC 5737): another machine that is never real.
        val e = assertFailsWith<SpeechException.Invalid> { endpoint("http://192.0.2.5:8080/v1") }
        assertTrue("only over https" in e.message!!, e.message)
        // Without a key, plain HTTP to another machine is the user's call.
        assertFalse(endpoint("http://192.0.2.5:8080/v1", key = null).isLoopback)
    }

    @Test
    fun aLookalikeOfLocalhostIsNotLoopback() {
        assertFailsWith<SpeechException.Invalid> { endpoint("http://127.0.0.1.example.com/v1") }
        assertFailsWith<SpeechException.Invalid> { endpoint("http://localhost.example.com/v1") }
    }

    @Test
    fun theUrlCarriesNoCredentialsQueryOrOddScheme() {
        listOf(
            "https://user:pass@api.example.com/v1",
            "https://api.example.com/v1?key=sk",
            "https://api.example.com/v1#x",
            "ftp://api.example.com/v1",
            "https://api.example.com/v 1",
            "https:///v1",
            "https://api.example.com/" + "a".repeat(MAX_BASE_URL_BYTES),
        ).forEach { url -> assertFailsWith<SpeechException.Invalid>(url) { endpoint(url) } }
    }

    @Test
    fun aBadSecretNameOrModelIsInvalid() {
        assertFailsWith<SpeechException.Invalid> { endpoint("https://x/v1", key = "Bad Name") }
        assertFailsWith<SpeechException.Invalid> { OpenAiEndpoint(EngineId("e"), "E", "https://x/v1", "m\nx") }
    }

    @Test
    fun theInfoSaysItUsesTheNetworkAndWhetherItIsOffline() {
        val remote = endpoint("https://api.openai.com/v1/")
        assertEquals("https://api.openai.com/v1/audio/transcriptions", remote.url)
        assertTrue(remote.info.capabilities.needsNetwork)
        assertFalse(remote.info.capabilities.offline)
        assertTrue(endpoint("http://127.0.0.1:8080/v1").info.capabilities.offline)
        assertIs<aulo.speech.LanguageSupport.Multilingual>(remote.info.capabilities.languages)
    }
}
