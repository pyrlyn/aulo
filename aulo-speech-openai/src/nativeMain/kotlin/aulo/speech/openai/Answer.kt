package aulo.speech.openai

import aulo.speech.SpeechException
import http.HttpBody
import http.HttpHead
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

// The server's answer, read as it arrives and capped while it is read: a server can be anyone's,
// so nothing it sends is trusted to be small, well formed or honest about its type.

/** What the server heard, and the language it says it was in (a BCP 47 tag, or null). */
internal class Answer(
    val text: String,
    val language: String?,
)

private const val READ_CHUNK = 16 * 1024
private const val DONE_SENTINEL = "[DONE]"
private val LANGUAGE_TAG = Regex("[A-Za-z]{2,3}(-[A-Za-z0-9]{1,8}){0,3}")

/**
 * Reads the answer of one request: server-sent events when the server sends them, JSON
 * otherwise. Each delta of a stream goes to [onPartial] as the whole text so far. Throws
 * [SpeechException] for an error status, a broken or oversized answer, or a stream that ends
 * without its final event.
 */
internal suspend fun readAnswer(
    head: HttpHead,
    body: HttpBody,
    options: OpenAiSttOptions,
    onPartial: (String) -> Unit,
): Answer {
    if (!head.isSuccess) throw statusFailure(head, body, options.maxErrorBytes)
    return if (head.mediaType() == "text/event-stream") {
        EventStream(options, onPartial).read(body)
    } else {
        jsonAnswer(readAll(body, options.maxAnswerBytes))
    }
}

/** The type of the body without its parameters, lowercased; null when the server sent none. */
internal fun HttpHead.mediaType(): String? =
    header("content-type")
        ?.substringBefore(';')
        ?.trim()
        ?.lowercase()

/** The error an unsuccessful answer stands for, with the message from the first [maxErrorBytes] of its body. */
internal suspend fun statusFailure(
    head: HttpHead,
    body: HttpBody,
    maxErrorBytes: Int,
): SpeechException = statusError(head.status, readUpTo(body, maxErrorBytes))

/** An answer that should have been audio but is JSON or text: an error some servers send with a 200. */
internal suspend fun bodyFailure(
    body: HttpBody,
    maxErrorBytes: Int,
): SpeechException {
    val text = readUpTo(body, maxErrorBytes)
    val message =
        try {
            (Json.parseToJsonElement(text) as? JsonObject)?.let { errorOf(it) }
        } catch (e: SerializationException) {
            null
        }
    return SpeechException.Failed(message ?: "the answer is not audio: $text")
}

/** The first [max] bytes of [body] as text; the rest is not read. */
private suspend fun readUpTo(
    body: HttpBody,
    max: Int,
): String {
    val out = ByteArray(max)
    val buf = ByteArray(minOf(max, READ_CHUNK))
    var size = 0
    while (size < max) {
        val n = body.read(buf)
        if (n < 0) break
        val take = minOf(n, max - size)
        buf.copyInto(out, size, 0, take)
        size += take
    }
    return out.decodeToString(0, size)
}

/** All of [body], or [SpeechException.Failed] once it passes [max] bytes. */
private suspend fun readAll(
    body: HttpBody,
    max: Int,
): String {
    var out = ByteArray(minOf(max, READ_CHUNK))
    var size = 0
    val buf = ByteArray(READ_CHUNK)
    while (true) {
        val n = body.read(buf)
        if (n < 0) break
        if (size + n > max) throw SpeechException.Failed("answer is over $max bytes")
        if (size + n > out.size) out = out.copyOf(minOf(max, maxOf(out.size * 2, size + n)))
        buf.copyInto(out, size, 0, n)
        size += n
    }
    return out.decodeToString(0, size)
}

private fun jsonAnswer(text: String): Answer {
    val obj = parseObject(text, "answer")
    errorOf(obj)?.let { throw SpeechException.Failed(it) }
    val said = (obj["text"] as? JsonPrimitive)?.takeIf { it.isString }?.content ?: throw SpeechException.Failed("answer has no text")
    return Answer(said, languageOf(obj))
}

private fun parseObject(
    text: String,
    what: String,
): JsonObject =
    try {
        Json.parseToJsonElement(text) as? JsonObject
    } catch (e: SerializationException) {
        null
    } ?: throw SpeechException.Failed("$what is not a JSON object")

private fun JsonElement?.string(): String? = (this as? JsonPrimitive)?.takeIf { it.isString }?.content

/** `languages[0].code` (GPT Transcribe) or `language`, when it is a language tag at all. */
private fun languageOf(obj: JsonObject): String? {
    val listed = ((obj["languages"] as? JsonArray)?.firstOrNull() as? JsonObject)?.get("code").string()
    return (listed ?: obj["language"].string())?.takeIf { LANGUAGE_TAG.matches(it) }
}

/** The message of an OpenAI-style `{"error": {"message": ...}}`, or of an `error` event. */
private fun errorOf(obj: JsonObject): String? {
    val error = obj["error"] ?: return if (obj["type"].string() == "error") "error event" else null
    return (error as? JsonObject)?.get("message").string() ?: error.string() ?: "error"
}

private fun statusError(
    status: Int,
    body: String,
): SpeechException {
    val message =
        try {
            (Json.parseToJsonElement(body) as? JsonObject)?.let { errorOf(it) }
        } catch (e: SerializationException) {
            null
        } ?: body
    val detail = "HTTP $status: $message"
    // A bad key or a wrong URL will not mend on the next utterance; the rest might.
    return when (status) {
        401, 403, 404 -> SpeechException.Unavailable(detail)
        else -> SpeechException.Failed(detail)
    }
}

/**
 * A `text/event-stream` body (the WHATWG server-sent events format) cut into lines and events
 * over byte chunks, so a line split across reads or a multi-byte character split across chunks
 * still decodes whole.
 */
private class EventStream(
    private val options: OpenAiSttOptions,
    private val onPartial: (String) -> Unit,
) {
    private val line = ByteArray(options.maxEventBytes)
    private var lineSize = 0
    private val data = StringBuilder()
    private var dataBytes = 0
    private var total = 0
    private val text = StringBuilder()
    private var answer: Answer? = null
    private var ended = false

    suspend fun read(body: HttpBody): Answer {
        val buf = ByteArray(READ_CHUNK)
        while (answer == null && !ended) {
            val n = body.read(buf)
            if (n < 0) break
            total += n
            if (total > options.maxAnswerBytes) throw SpeechException.Failed("answer is over ${options.maxAnswerBytes} bytes")
            for (i in 0 until n) {
                accept(buf[i])
                if (answer != null || ended) break
            }
        }
        // An event the stream did not finish with a blank line is dropped, as the format says.
        return answer ?: throw SpeechException.Failed("the stream ended before transcript.text.done")
    }

    private fun accept(b: Byte) {
        if (b == '\n'.code.toByte()) {
            val end = if (lineSize > 0 && line[lineSize - 1] == '\r'.code.toByte()) lineSize - 1 else lineSize
            val text = line.decodeToString(0, end)
            lineSize = 0
            onLine(text, end)
            return
        }
        if (lineSize == line.size) throw SpeechException.Failed("an event line is over ${options.maxEventBytes} bytes")
        line[lineSize++] = b
    }

    private fun onLine(
        text: String,
        bytes: Int,
    ) {
        if (text.isEmpty()) return dispatch()
        if (text.startsWith(':')) return
        val field = text.substringBefore(':')
        if (field != "data") return
        val value = text.substringAfter(':', "").removePrefix(" ")
        dataBytes += bytes + 1
        if (dataBytes > options.maxEventBytes) throw SpeechException.Failed("an event is over ${options.maxEventBytes} bytes")
        if (data.isNotEmpty()) data.append('\n')
        data.append(value)
    }

    private fun dispatch() {
        if (data.isEmpty()) return
        val event = data.toString()
        data.clear()
        dataBytes = 0
        // Not OpenAI's (its stream ends with transcript.text.done), but servers built on the chat API send it.
        if (event == DONE_SENTINEL) {
            ended = true
            return
        }
        val obj = parseObject(event, "an event")
        errorOf(obj)?.let { throw SpeechException.Failed(it) }
        when (obj["type"].string()) {
            "transcript.text.delta" -> {
                val delta = obj["delta"].string() ?: throw SpeechException.Failed("a delta event has no delta")
                text.append(delta)
                onPartial(text.toString())
            }

            "transcript.text.done" -> {
                val said = obj["text"].string() ?: throw SpeechException.Failed("the done event has no text")
                answer = Answer(said, languageOf(obj))
            }

            // Segments of a diarized stream and anything newer carry nothing this engine needs.
            else -> {}
        }
    }
}
