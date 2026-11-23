package aulo.speech.openai

import aulo.secrets.SecretException
import aulo.secrets.SecretStore
import aulo.speech.SpeechException
import http.HttpException

/**
 * The headers every request carries: the API key, read from [secrets] for this request only so
 * its value never goes into a message, and the body's type.
 */
internal fun requestHeaders(
    secrets: SecretStore,
    apiKeySecret: String?,
    contentType: String,
): MutableList<Pair<String, String>> {
    val headers = mutableListOf<Pair<String, String>>()
    if (apiKeySecret != null) {
        val key =
            try {
                secrets.load(apiKeySecret)
            } catch (e: SecretException) {
                throw SpeechException.Unavailable("API key `$apiKeySecret` cannot be read: ${e.message}")
            } ?: throw SpeechException.Unavailable("API key `$apiKeySecret` is not in ${secrets.description}")
        headers += "Authorization" to "Bearer $key"
    }
    headers += "Content-Type" to contentType
    return headers
}

/** What a request that threw becomes; the caller rethrows cancellation before asking. */
internal fun requestFailure(e: Exception): SpeechException =
    when (e) {
        is SpeechException -> e

        is HttpException -> SpeechException.Failed("${e.kind}: ${e.message}")

        // The transport is the caller's code; anything else it throws is still this engine failing.
        else -> SpeechException.Failed(e.message ?: e::class.simpleName ?: "request failed")
    }
