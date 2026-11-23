@file:OptIn(ExperimentalAtomicApi::class)

package aulo.speech.openai

import aulo.secrets.SecretStore
import aulo.speech.AudioFormat
import aulo.speech.EngineInfo
import aulo.speech.SpeechException
import aulo.speech.SpeechRate
import aulo.speech.TtsEngine
import aulo.speech.TtsPoll
import aulo.speech.TtsRequest
import aulo.speech.Voice
import http.HttpBody
import http.HttpClient
import http.HttpRequest
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.launch
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlin.concurrent.atomics.AtomicBoolean
import kotlin.concurrent.atomics.AtomicReference
import kotlin.concurrent.atomics.ExperimentalAtomicApi
import kotlin.time.TimeMark

private const val READ_CHUNK = 16 * 1024

/** Body types that carry raw PCM; a server that names none is taken at its word too. */
private val PCM_TYPES = setOf("audio/pcm", "audio/l16", "audio/raw", "application/octet-stream")

/**
 * A [TtsEngine] over an OpenAI-compatible `/audio/speech` endpoint: OpenAI, a local server or any
 * compatible one. Each batch of queued text is one request asking for raw PCM, and its answer is
 * read as it arrives into a fixed ring that [poll] drains, so the first sentence plays while the
 * rest is still being spoken by the server.
 *
 * Requests run one at a time on an I/O coroutine, in the order the text came, so the audio stays
 * in order. `pushText` and `poll` never block. Each reply has its own queue, ring and worker, so a
 * cancelled reply's late audio can never reach the next one.
 */
class OpenAiTts internal constructor(
    private val endpoint: OpenAiTtsEndpoint,
    private val model: String,
    private val http: HttpClient,
    private val secrets: SecretStore,
    private val options: OpenAiTtsOptions,
) : TtsEngine {
    override val info: EngineInfo = endpoint.info
    override val voices: List<Voice> = endpoint.voices

    init {
        // A model from a gRPC override is checked like any engine label before it is sent.
        aulo.speech.EngineSpec(endpoint.engine, model = model)
    }

    /** One reply, shared by the caller's thread and its worker. */
    private class Reply(
        ringSamples: Int,
    ) {
        val text = Channel<String>(Channel.UNLIMITED)
        val ring = SampleRing(ringSamples)

        /** Set by the worker once every sample of the reply is in the ring. */
        val done = AtomicBoolean(false)
        val failure = AtomicReference<SpeechException?>(null)

        /** When the worker started waiting on the server; null while it waits for text or room. */
        val waiting = AtomicReference<TimeMark?>(null)
        var worker: Job? = null
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val closed = AtomicBoolean(false)

    // The caller's thread only.
    private var reply: Reply? = null
    private var finished = false
    private var pushedChars = 0

    override fun begin(request: TtsRequest): AudioFormat {
        checkOpen()
        val language = request.language
        if (language != null && !info.capabilities.languages.supports(language)) {
            throw SpeechException.Unsupported("language $language")
        }
        val voice = pickVoice(request.voice, language)
        cancel()
        val next = Reply(options.ringSamples)
        next.worker = scope.launch { speak(next, voice, request.rate) }
        reply = next
        finished = false
        pushedChars = 0
        return OPENAI_PCM_FORMAT
    }

    override fun pushText(text: String) {
        checkOpen()
        val current = reply
        if (current == null || finished) throw SpeechException.OutOfOrder("pushText outside a reply")
        if (text.length > options.maxReplyChars - pushedChars) {
            throw SpeechException.Invalid("reply text", "is over ${options.maxReplyChars} characters")
        }
        pushedChars += text.length
        current.text.trySend(text)
    }

    override fun finish() {
        checkOpen()
        val current = reply
        if (current == null || finished) throw SpeechException.OutOfOrder("finish outside a reply")
        finished = true
        current.text.close()
    }

    override fun poll(out: FloatArray): TtsPoll {
        checkOpen()
        val current = reply ?: return TtsPoll.DONE
        // Read before draining: the worker sets it only after its last write.
        val complete = current.done.load()
        val n = current.ring.read(out)
        if (n > 0) return TtsPoll.audio(n)
        current.failure.load()?.let {
            cancel()
            throw it
        }
        if (complete) {
            reply = null
            return TtsPoll.DONE
        }
        val since = current.waiting.load()
        if (since != null && since.elapsedNow() > options.stallTimeout) {
            cancel()
            throw SpeechException.Failed("no audio for ${options.stallTimeout}")
        }
        return TtsPoll.PENDING
    }

    override fun cancel() {
        // Only asks the worker to stop; it writes into its own ring, which nobody reads any more.
        reply?.let {
            it.worker?.cancel()
            it.text.close()
        }
        reply = null
        finished = false
        pushedChars = 0
    }

    override fun close() {
        if (!closed.compareAndSet(expectedValue = false, newValue = true)) return
        reply = null
        scope.cancel()
    }

    private fun checkOpen() {
        if (closed.load()) throw SpeechException.Unavailable("engine is closed")
    }

    /** The asked voice, else the first configured one in [language], else the default. */
    private fun pickVoice(
        asked: String?,
        language: String?,
    ): String {
        if (asked != null) {
            if (voices.none { it.id == asked }) throw SpeechException.Unsupported("voice $asked")
            return asked
        }
        if (language == null) return endpoint.defaultVoice
        val primary = language.substringBefore('-')
        return voices.firstOrNull { it.language?.substringBefore('-').equals(primary, ignoreCase = true) }?.id
            ?: endpoint.defaultVoice
    }

    private suspend fun speak(
        reply: Reply,
        voice: String,
        rate: SpeechRate,
    ) {
        try {
            val pending = StringBuilder()
            val buf = ByteArray(READ_CHUNK)
            val samples = ShortArray(READ_CHUNK / 2 + 1)
            while (true) {
                if (pending.isEmpty()) {
                    val text = reply.text.receiveCatching().getOrNull() ?: break
                    pending.append(text)
                }
                // Whatever else is already queued goes in the same request: fewer round trips.
                while (pending.length < endpoint.maxInputChars) {
                    val more = reply.text.tryReceive().getOrNull() ?: break
                    join(pending, more)
                }
                val cut = cutAt(pending, endpoint.maxInputChars)
                val input = pending.substring(0, cut)
                pending.deleteRange(0, cut)
                if (input.isNotBlank()) {
                    request(reply, input.trim(), voice, rate, buf, samples)
                    // The answer is over: waiting for the caller's next text is not the server stalling.
                    reply.waiting.store(null)
                }
            }
            reply.done.store(true)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            reply.failure.store(requestFailure(e))
        } finally {
            reply.waiting.store(null)
        }
    }

    private suspend fun request(
        reply: Reply,
        input: String,
        voice: String,
        rate: SpeechRate,
        buf: ByteArray,
        samples: ShortArray,
    ) {
        val json =
            buildJsonObject {
                put("model", model)
                put("input", input)
                put("voice", voice)
                put("response_format", "pcm")
                // Left out at the normal pace, for servers that know no `speed`.
                if (rate != SpeechRate.NORMAL) put("speed", rate.value)
            }
        val headers = requestHeaders(secrets, endpoint.apiKeySecret, "application/json")
        reply.waiting.store(options.time.markNow())
        http.streamPost(HttpRequest(endpoint.url, headers, json.toString().encodeToByteArray())) { head, body ->
            if (!head.isSuccess) throw statusFailure(head, body, options.maxErrorBytes)
            val type = head.mediaType()
            when {
                type == null || type in PCM_TYPES -> {}

                type == "application/json" || type.startsWith("text/") -> {
                    throw bodyFailure(body, options.maxErrorBytes)
                }

                else -> {
                    throw SpeechException.Failed("the answer is $type, not raw PCM")
                }
            }
            readPcm(reply, body, buf, samples)
        }
    }

    /** Streams 16-bit little-endian samples into the ring, carrying a byte split across reads. */
    private suspend fun readPcm(
        reply: Reply,
        body: HttpBody,
        buf: ByteArray,
        samples: ShortArray,
    ) {
        var total = 0L
        var carry = -1
        while (true) {
            reply.waiting.store(options.time.markNow())
            val n = body.read(buf)
            if (n < 0) break
            total += n
            if (total > options.maxBytesPerRequest) {
                throw SpeechException.Failed("the answer is over ${options.maxAudioPerRequest} of audio")
            }
            var count = 0
            var i = 0
            if (carry >= 0 && n > 0) {
                samples[count++] = pcm16(carry, buf[0].toInt())
                carry = -1
                i = 1
            }
            while (i + 1 < n) {
                samples[count++] = pcm16(buf[i].toInt(), buf[i + 1].toInt())
                i += 2
            }
            if (i < n) carry = buf[i].toInt() and 0xff
            // Waiting for room is the speaker's pace, not the server stalling.
            reply.waiting.store(null)
            reply.ring.write(samples, count)
        }
    }

    private fun pcm16(
        low: Int,
        high: Int,
    ): Short = ((low and 0xff) or (high shl 8)).toShort()
}

/** Appends [next] to [pending] with a space between, unless one side already has one. */
private fun join(
    pending: StringBuilder,
    next: String,
) {
    if (pending.isNotEmpty() && next.isNotEmpty() && !pending.last().isWhitespace() && !next.first().isWhitespace()) {
        pending.append(' ')
    }
    pending.append(next)
}

/**
 * Where to end one request's input: all of [text] when it fits [max], else after the last
 * whitespace in its second half, else at [max] itself, never between the halves of a surrogate
 * pair. The second half only, so one long word does not leave a request of a few characters.
 */
internal fun cutAt(
    text: CharSequence,
    max: Int,
): Int {
    if (text.length <= max) return text.length
    for (i in max downTo max / 2 + 1) {
        if (text[i - 1].isWhitespace()) return i
    }
    return if (text[max - 1].isHighSurrogate()) max - 1 else max
}
