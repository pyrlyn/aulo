package aulo.speech.openai

import aulo.secrets.SecretException
import aulo.secrets.SecretStore
import aulo.speech.Capabilities
import aulo.speech.EngineId
import aulo.speech.EngineInfo
import aulo.speech.EngineKind
import aulo.speech.LanguageSupport
import aulo.speech.SpeechException
import aulo.voice.SttRegistry
import http.HttpClient
import kotlin.time.Duration
import kotlin.time.Duration.Companion.minutes
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TimeSource

/** Longest base URL; it shows up in errors and logs. */
const val MAX_BASE_URL_BYTES = 2048

/**
 * One OpenAI-compatible transcription server. Each gets its own engine id, so OpenAI and a local
 * server can both be configured and one can fall back to the other.
 */
data class OpenAiEndpoint(
    val engine: EngineId,
    /** For pickers. */
    val name: String,
    /**
     * The API root, `/v1` included, as OpenAI's SDKs take it: `https://api.openai.com/v1` or
     * `http://127.0.0.1:8080/v1`. The engine posts to `<baseUrl>/audio/transcriptions`.
     */
    val baseUrl: String,
    /** The model a spec without one gets, e.g. `gpt-transcribe`. */
    val defaultModel: String,
    /** Name of the API key in the [SecretStore]; null for a server that takes none. */
    val apiKeySecret: String? = null,
    /** Ask for server-sent events; a server that cannot stream answers with JSON, which is read just the same. */
    val stream: Boolean = true,
    val languages: LanguageSupport = LanguageSupport.Multilingual,
) {
    /** Whether the server is on this machine: then the engine works without a network. */
    val isLoopback: Boolean

    init {
        isLoopback = checkServer(baseUrl, apiKeySecret)
        // Validated like any engine label, so the spec built from it cannot fail later.
        aulo.speech.EngineSpec(engine, model = defaultModel)
    }

    val url: String get() = baseUrl.trimEnd('/') + "/audio/transcriptions"

    val info: EngineInfo
        get() =
            EngineInfo(
                id = engine,
                kind = EngineKind.STT,
                name = name,
                capabilities =
                    Capabilities(
                        // Audio goes up as one file after finish; partials only follow that.
                        streaming = false,
                        languages = languages,
                        offline = isLoopback,
                        needsNetwork = true,
                    ),
            )
}

data class OpenAiSttOptions(
    /**
     * Most audio one utterance keeps; `push` drops the rest. 10 minutes of 16 kHz 16-bit mono is
     * 19.2 MB, under the 25 MB OpenAI takes per file.
     */
    val maxAudio: Duration = 10.minutes,
    /** Most bytes of a whole answer, JSON or every event of a stream together. */
    val maxAnswerBytes: Int = 1 shl 20,
    /** Most bytes of one line or one event of a stream. */
    val maxEventBytes: Int = 64 * 1024,
    /** Most bytes of an error body read for its message. */
    val maxErrorBytes: Int = 4 * 1024,
    /** How long the final transcript may take after `finish`, on top of the HTTP client's own timeouts. */
    val finalTimeout: Duration = 120.seconds,
    val time: TimeSource = TimeSource.Monotonic,
) {
    init {
        require(maxAudio.isPositive()) { "maxAudio must be positive" }
        require(maxAnswerBytes > 0 && maxEventBytes > 0 && maxErrorBytes > 0) { "caps must be positive" }
        require(maxEventBytes <= maxAnswerBytes) { "an event cannot be larger than the answer" }
    }
}

/**
 * Registers [endpoint] under its engine id. [secrets] holds its API key, which is read for each
 * request and only into the request header.
 */
fun SttRegistry.registerOpenAi(
    endpoint: OpenAiEndpoint,
    http: HttpClient,
    secrets: SecretStore,
    options: OpenAiSttOptions = OpenAiSttOptions(),
) {
    register(endpoint.engine) { spec -> OpenAiStt(endpoint, spec.model ?: endpoint.defaultModel, http, secrets, options) }
}

/**
 * Checks the API root and the key's secret name every endpoint shares; returns whether the server
 * is on this machine.
 */
internal fun checkServer(
    baseUrl: String,
    apiKeySecret: String?,
): Boolean {
    if (baseUrl.encodeToByteArray().size > MAX_BASE_URL_BYTES) {
        throw SpeechException.Invalid("base URL", "is longer than $MAX_BASE_URL_BYTES bytes")
    }
    val scheme = baseUrl.substringBefore("://", "")
    if (scheme != "https" && scheme != "http") throw SpeechException.Invalid("base URL", "must start with https:// or http://")
    // A key or token in the URL itself would end up in errors and logs; it belongs in the secret store.
    if (baseUrl.any { it.isWhitespace() || it.isISOControl() || it in "?#@" }) {
        throw SpeechException.Invalid("base URL", "takes no spaces, query, fragment or user name")
    }
    val authority = baseUrl.substringAfter("://").substringBefore('/')
    if (authority.isEmpty()) throw SpeechException.Invalid("base URL", "has no host")
    val loopback = isLoopbackHost(hostOf(authority))
    if (apiKeySecret != null) {
        try {
            SecretStore.checkName(apiKeySecret)
        } catch (e: SecretException) {
            throw SpeechException.Invalid("API key secret", e.message ?: "bad name")
        }
        // Plain HTTP to another machine would hand the key to anyone on the path.
        if (scheme == "http" && !loopback) throw SpeechException.Invalid("base URL", "sends the API key only over https")
    }
    return loopback
}

/** The host of `host[:port]` or `[v6]:port`. */
private fun hostOf(authority: String): String =
    if (authority.startsWith("[")) authority.substringBefore(']') + "]" else authority.substringBefore(':')

private fun isLoopbackHost(host: String): Boolean {
    val h = host.lowercase()
    if (h == "localhost" || h == "[::1]") return true
    val octets = h.split('.')
    return octets.size == 4 && octets[0] == "127" && octets.all { o -> o.toIntOrNull()?.let { it in 0..255 } == true }
}
