package aulo.speech.openai

import aulo.secrets.SecretStore
import aulo.speech.AudioFormat
import aulo.speech.Capabilities
import aulo.speech.EngineId
import aulo.speech.EngineInfo
import aulo.speech.EngineKind
import aulo.speech.LanguageSupport
import aulo.speech.SpeechException
import aulo.speech.Voice
import aulo.voice.TtsRegistry
import http.HttpClient
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.minutes
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TimeSource

/** Most voices one endpoint lists; they come from config and end up in pickers. */
const val MAX_TTS_VOICES = 64

/** What `response_format=pcm` answers with: 24 kHz 16-bit signed little-endian mono, no header. */
val OPENAI_PCM_FORMAT = AudioFormat(24_000, 1)

/**
 * One OpenAI-compatible speech server. Each gets its own engine id, so OpenAI and a local server
 * can both be configured and one can fall back to the other.
 */
data class OpenAiTtsEndpoint(
    val engine: EngineId,
    /** For pickers. */
    val name: String,
    /** The API root, `/v1` included, as for [OpenAiEndpoint]; the engine posts to `<baseUrl>/audio/speech`. */
    val baseUrl: String,
    /** The model a spec without one gets, e.g. `gpt-4o-mini-tts`. */
    val defaultModel: String,
    /** The voices from config, the default first; the API has no call that lists them. */
    val voices: List<Voice>,
    /** Name of the API key in the [SecretStore]; null for a server that takes none. */
    val apiKeySecret: String? = null,
    val languages: LanguageSupport = LanguageSupport.Multilingual,
    /** Most characters of `input` the server takes in one request; OpenAI takes 4096. */
    val maxInputChars: Int = 4096,
) {
    /** Whether the server is on this machine: then the engine works without a network. */
    val isLoopback: Boolean = checkServer(baseUrl, apiKeySecret)

    init {
        aulo.speech.EngineSpec(engine, model = defaultModel)
        if (voices.isEmpty() || voices.size > MAX_TTS_VOICES) {
            throw SpeechException.Invalid("voice list", "must hold 1 to $MAX_TTS_VOICES voices")
        }
        if (voices.distinctBy { it.id }.size != voices.size) throw SpeechException.Invalid("voice list", "names a voice twice")
        // Room for a sentence or two at least; below that every sentence would be cut mid-word.
        if (maxInputChars < MIN_INPUT_CHARS) throw SpeechException.Invalid("input cap", "must be at least $MIN_INPUT_CHARS characters")
    }

    val defaultVoice: String get() = voices.first().id

    val url: String get() = baseUrl.trimEnd('/') + "/audio/speech"

    val info: EngineInfo
        get() =
            EngineInfo(
                id = engine,
                kind = EngineKind.TTS,
                name = name,
                capabilities =
                    Capabilities(
                        // Each sentence is sent as it arrives and its audio streams back.
                        streaming = true,
                        languages = languages,
                        offline = isLoopback,
                        needsNetwork = true,
                    ),
            )

    private companion object {
        const val MIN_INPUT_CHARS = 256
    }
}

data class OpenAiTtsOptions(
    /** Audio held between the server and `poll`; when it is full the engine stops reading the answer. */
    val bufferedAudio: Duration = 10.seconds,
    /** Most characters one reply may push; the normalizer already cuts speech to 2000. */
    val maxReplyChars: Int = 16 * 1024,
    /** Most audio one request may answer with: a server that never stops is a broken one. */
    val maxAudioPerRequest: Duration = 10.minutes,
    /** Most bytes of an error body read for its message. */
    val maxErrorBytes: Int = 4 * 1024,
    /** How long a request may go without a byte of audio, on top of the HTTP client's own timeouts. */
    val stallTimeout: Duration = 60.seconds,
    val time: TimeSource = TimeSource.Monotonic,
) {
    init {
        require(bufferedAudio >= 100.milliseconds) { "bufferedAudio must hold at least 100 ms" }
        require(maxReplyChars > 0 && maxErrorBytes > 0) { "caps must be positive" }
        require(maxAudioPerRequest.isPositive() && stallTimeout.isPositive()) { "durations must be positive" }
    }

    internal val ringSamples: Int get() = samplesIn(bufferedAudio)

    internal val maxBytesPerRequest: Long get() = samplesIn(maxAudioPerRequest).toLong() * 2

    private fun samplesIn(d: Duration): Int = (d.inWholeMilliseconds * OPENAI_PCM_FORMAT.sampleRateHz / 1000).toInt()
}

/**
 * Registers [endpoint] under its engine id. [secrets] holds its API key, which is read for each
 * request and only into the request header.
 */
fun TtsRegistry.registerOpenAi(
    endpoint: OpenAiTtsEndpoint,
    http: HttpClient,
    secrets: SecretStore,
    options: OpenAiTtsOptions = OpenAiTtsOptions(),
) {
    register(endpoint.engine) { spec -> OpenAiTts(endpoint, spec.model ?: endpoint.defaultModel, http, secrets, options) }
}
