package aulo.speech.openai

import aulo.secrets.SecretStore
import aulo.speech.AudioFormat
import aulo.speech.AudioFrame
import aulo.speech.SttEngine
import aulo.speech.SttPoll
import aulo.speech.Transcript
import aulo.types.TranscriptKind
import http.HttpBody
import http.HttpClient
import http.HttpHead
import http.HttpRequest
import http.HttpResponse
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.delay
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TimeSource

/** What the fake server answers one request with. */
class Reply(
    val status: Int = 200,
    val contentType: String? = "application/json",
    val body: ByteArray = ByteArray(0),
    /** Bytes per read, small so lines, events and characters split across reads. */
    val chunk: Int = 7,
    val failure: Exception? = null,
) {
    constructor(status: Int = 200, contentType: String? = "application/json", text: String, chunk: Int = 7) :
        this(status, contentType, text.encodeToByteArray(), chunk)
}

/** An in-process OpenAI-compatible server, so the engine is tested without a network. */
class FakeTranscriber(
    var answer: (index: Int, HttpRequest) -> Reply,
) : HttpClient {
    val requests = mutableListOf<HttpRequest>()

    /** Requests wait here until it completes; a cancelled utterance's request never gets past it. */
    var gate: CompletableDeferred<Unit>? = null

    /** Body bytes the engine read, over every request. */
    var served = 0

    override suspend fun post(req: HttpRequest): HttpResponse = error("the engine streams every answer")

    override suspend fun <T> streamPost(
        req: HttpRequest,
        handle: suspend (HttpHead, HttpBody) -> T,
    ): T {
        val index = requests.size
        requests += req
        gate?.await()
        val reply = answer(index, req)
        reply.failure?.let { throw it }
        val headers = listOfNotNull(reply.contentType?.let { "Content-Type" to it })
        var sent = 0
        val body =
            HttpBody { buffer ->
                if (sent == reply.body.size) return@HttpBody -1
                val n = minOf(buffer.size, reply.chunk, reply.body.size - sent)
                reply.body.copyInto(buffer, 0, sent, sent + n)
                sent += n
                served += n
                n
            }
        return handle(HttpHead(reply.status, headers), body)
    }

    override fun close() = Unit
}

class MemorySecrets(
    vararg entries: Pair<String, String>,
) : SecretStore {
    private val values = mutableMapOf(*entries)
    override val description = "the test store"

    override fun save(
        name: String,
        value: String,
    ) {
        values[name] = value
    }

    override fun load(name: String): String? = values[name]

    override fun delete(name: String): Boolean = values.remove(name) != null
}

fun frame(
    samples: Int,
    value: Float = 0.5f,
): AudioFrame = AudioFrame(FloatArray(samples) { value }, AudioFormat.PIPELINE, 0)

/** Polls until the utterance is done, collecting every update; whatever the engine throws passes through. */
suspend fun SttEngine.updates(out: Transcript): List<Pair<TranscriptKind, String>> {
    val seen = mutableListOf<Pair<TranscriptKind, String>>()
    val giveUp = TimeSource.Monotonic.markNow() + 10.seconds
    while (true) {
        when (poll(out)) {
            SttPoll.DONE -> {
                return seen
            }

            SttPoll.UPDATED -> {
                seen += out.kind to out.text.toString()
            }

            SttPoll.PENDING -> {
                check(!giveUp.hasPassedNow()) { "no final transcript" }
                delay(1)
            }
        }
    }
}

/** The text fields of a multipart body, by name; the file part is left out. */
fun HttpRequest.formFields(): List<Pair<String, String>> {
    val type = headers.first { it.first == "Content-Type" }.second
    val boundary = type.substringAfter("boundary=")
    return body
        .decodeToString()
        .split("--$boundary")
        .mapNotNull { part ->
            val name = Regex("name=\"([^\"]+)\"\r\n\r\n").find(part)?.groupValues?.get(1) ?: return@mapNotNull null
            name to part.substringAfter("\r\n\r\n").removeSuffix("\r\n")
        }
}

/** Where the WAV starts in a multipart body. */
fun HttpRequest.wavAt(): Int {
    val riff = "RIFF".encodeToByteArray()
    return (0..body.size - riff.size).first { i -> riff.indices.all { body[i + it] == riff[it] } }
}

fun ByteArray.u32(at: Int): Int = (0..3).sumOf { (this[at + it].toInt() and 0xff) shl (8 * it) }
