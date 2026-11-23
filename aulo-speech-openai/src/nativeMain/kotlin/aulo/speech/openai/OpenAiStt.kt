@file:OptIn(ExperimentalAtomicApi::class)

package aulo.speech.openai

import aulo.secrets.SecretStore
import aulo.speech.AudioFormat
import aulo.speech.AudioFrame
import aulo.speech.EngineInfo
import aulo.speech.PIPELINE_SAMPLE_RATE_HZ
import aulo.speech.SpeechException
import aulo.speech.SttEngine
import aulo.speech.SttPoll
import aulo.speech.Transcript
import aulo.types.TranscriptKind
import aulo.types.TurnId
import http.HttpClient
import http.HttpRequest
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlin.concurrent.atomics.AtomicBoolean
import kotlin.concurrent.atomics.AtomicReference
import kotlin.concurrent.atomics.ExperimentalAtomicApi
import kotlin.time.TimeMark

/** Audio kept before the buffer first grows: 4 s. */
private const val FIRST_CAPACITY_SAMPLES = 4 * PIPELINE_SAMPLE_RATE_HZ

/**
 * An [SttEngine] over an OpenAI-compatible `/audio/transcriptions` endpoint. The endpoint takes a
 * whole file, so the utterance is kept as 16-bit samples (up to [OpenAiSttOptions.maxAudio]) and
 * sent on [finish]; the answer is a JSON body or, where the server streams, server-sent events
 * whose deltas arrive as partials before the final.
 *
 * `push` and `poll` never block: the request runs on an I/O coroutine and hands its results over
 * through atomic slots. Utterances are numbered, so the result of a cancelled one is dropped. A
 * failed request fails that utterance only: the next may go to a server that is back.
 */
class OpenAiStt internal constructor(
    private val endpoint: OpenAiEndpoint,
    private val model: String,
    private val http: HttpClient,
    private val secrets: SecretStore,
    private val options: OpenAiSttOptions,
) : SttEngine {
    override val info: EngineInfo = endpoint.info

    init {
        // A model from a gRPC override is checked like any engine label before it is sent.
        aulo.speech.EngineSpec(endpoint.engine, model = model)
    }

    private class Result(
        val seq: Int,
        val final: Boolean,
        val text: String,
        val language: String?,
    )

    private class Failure(
        val seq: Int,
        val error: SpeechException,
    )

    private enum class Phase { IDLE, OPEN, FINISHED, DONE }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val latest = AtomicReference<Result?>(null)
    private val failure = AtomicReference<Failure?>(null)
    private val closed = AtomicBoolean(false)
    private val maxSamples = (options.maxAudio.inWholeMilliseconds * PIPELINE_SAMPLE_RATE_HZ / 1000).toInt()

    // The caller's thread only.
    private var seq = 0
    private var phase = Phase.IDLE
    private var turn: TurnId? = null
    private var language: String? = null
    private var samples = ShortArray(0)
    private var count = 0
    private var delivered: Result? = null
    private var finishedAt: TimeMark? = null
    private var request: Job? = null

    override fun begin(
        turnId: TurnId,
        language: String?,
    ) {
        checkOpen()
        if (language != null && !info.capabilities.languages.supports(language)) {
            throw SpeechException.Unsupported("language $language")
        }
        if (phase == Phase.OPEN || phase == Phase.FINISHED) cancel()
        seq++
        turn = turnId
        this.language = language
        // The last utterance's buffer went to its request; this one starts small and grows to the cap.
        samples = ShortArray(minOf(FIRST_CAPACITY_SAMPLES, maxSamples))
        count = 0
        delivered = null
        finishedAt = null
        phase = Phase.OPEN
    }

    override fun push(frame: AudioFrame): Int {
        checkOpen()
        if (phase != Phase.OPEN) throw SpeechException.OutOfOrder("push outside an utterance")
        if (frame.format != AudioFormat.PIPELINE) throw SpeechException.Unsupported("audio format ${frame.format}")
        val take = minOf(frame.length, maxSamples - count)
        if (count + take > samples.size) samples = samples.copyOf(minOf(maxSamples, maxOf(samples.size * 2, count + take)))
        for (i in 0 until take) samples[count + i] = pcm16(frame[i])
        count += take
        return frame.length - take
    }

    override fun finish() {
        checkOpen()
        if (phase != Phase.OPEN) throw SpeechException.OutOfOrder("finish outside an utterance")
        phase = Phase.FINISHED
        finishedAt = options.time.markNow()
        val mine = seq
        if (count == 0) {
            // Nothing was said; a request would only cost a round trip to hear so.
            publish(Result(mine, final = true, text = "", language = language))
            return
        }
        val audio = samples
        val size = count
        val hint = language
        samples = ShortArray(0)
        request = scope.launch { transcribe(mine, audio, size, hint) }
    }

    override fun poll(out: Transcript): SttPoll {
        checkOpen()
        val turn = turn ?: return SttPoll.PENDING
        if (phase == Phase.DONE) return SttPoll.DONE
        if (phase == Phase.IDLE) return SttPoll.PENDING
        val result = latest.load()
        if (result != null && result.seq == seq && result !== delivered) {
            delivered = result
            if (result.final) phase = Phase.DONE
            // Partials after finish are the server reading the file: worth showing, never final.
            val kind = if (result.final) TranscriptKind.FINAL else TranscriptKind.PARTIAL
            out.set(turn, kind, result.text, result.language ?: language)
            return SttPoll.UPDATED
        }
        failure.load()?.takeIf { it.seq == seq }?.let {
            cancel()
            throw it.error
        }
        val since = finishedAt
        if (phase == Phase.FINISHED && since != null && since.elapsedNow() > options.finalTimeout) {
            cancel()
            throw SpeechException.Failed("no transcript within ${options.finalTimeout}")
        }
        return SttPoll.PENDING
    }

    override fun cancel() {
        // Cancelling only asks; the seq check in publish is what keeps a late answer out.
        request?.cancel()
        request = null
        turn = null
        phase = Phase.IDLE
        samples = ShortArray(0)
        count = 0
        delivered = null
        finishedAt = null
    }

    override fun close() {
        if (!closed.compareAndSet(expectedValue = false, newValue = true)) return
        scope.cancel()
    }

    private fun checkOpen() {
        if (closed.load()) throw SpeechException.Unavailable("engine is closed")
    }

    private suspend fun transcribe(
        mine: Int,
        audio: ShortArray,
        size: Int,
        hint: String?,
    ) {
        try {
            val form = transcriptionForm(model, hint, endpoint.stream, audio, size, AudioFormat.PIPELINE.sampleRateHz)
            val headers = requestHeaders(secrets, endpoint.apiKeySecret, form.contentType)
            if (endpoint.stream) headers += "Accept" to "text/event-stream, application/json"
            val answer =
                http.streamPost(HttpRequest(endpoint.url, headers, form.body)) { head, body ->
                    readAnswer(head, body, options) { text -> publish(Result(mine, final = false, text = text, language = null)) }
                }
            publish(Result(mine, final = true, text = answer.text, language = answer.language))
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            newest(failure, Failure(mine, requestFailure(e)), Failure::seq)
        }
    }

    private fun publish(result: Result) = newest(latest, result, Result::seq)

    /**
     * Stores [value] unless the slot already holds a later utterance's. A cancelled request can
     * still be inside a chunk when the next utterance's answer lands; it must not overwrite it.
     */
    private fun <T : Any> newest(
        slot: AtomicReference<T?>,
        value: T,
        seqOf: (T) -> Int,
    ) {
        while (true) {
            val prev = slot.load()
            if (prev != null && seqOf(prev) > seqOf(value)) return
            if (slot.compareAndSet(prev, value)) return
        }
    }
}
