package aulo.chat.server

import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds

/** The provider answered with an error that a later attempt may not hit (overload, timeout). */
class TransientModelException(message: String) : Exception(message)

/** The provider refused the request; trying again cannot help. */
class ModelException(message: String) : Exception(message)

data class ModelSpec(val id: ModelId, val displayName: String)

interface ChatModel {
    val models: List<ModelSpec>

    /** Streams the reply to the last message of [history] token by token. */
    fun generate(model: ModelId, history: List<StoredMessage>): Flow<String>
}

/**
 * Stands in for a language model. It answers from the chat history so the dialog memory is
 * visible, and each of its three models shows one behaviour of the pipeline: `echo` streams
 * at reading speed, `slow` takes its time so Stop is easy to hit, `flaky` fails twice before
 * every answer so the retries show up. A message containing `/fail` is always refused.
 */
class StubModel(
    private val tokenDelay: Duration = 35.milliseconds,
    private val flakyFailures: Int = 2,
) : ChatModel {
    override val models = listOf(
        ModelSpec(ECHO, "Stub Echo"),
        ModelSpec(SLOW, "Stub Slow"),
        ModelSpec(FLAKY, "Stub Flaky"),
    )

    // Keyed by the message being answered, so every turn on the flaky model fails anew.
    private val attempts = HashMap<String, Int>()

    override fun generate(model: ModelId, history: List<StoredMessage>): Flow<String> = flow {
        val last = history.lastOrNull { it.role == Role.USER }
            ?: throw ModelException("nothing to answer")
        if ("/fail" in last.content) throw ModelException("the stub model refuses messages containing /fail")
        if (model == FLAKY && failsThisAttempt(last.id)) {
            throw TransientModelException("stub provider overloaded")
        }
        val pause = if (model == SLOW) tokenDelay * 6 else tokenDelay
        for (token in tokens(reply(model, history, last))) {
            delay(pause)
            emit(token)
        }
    }

    private fun failsThisAttempt(messageId: String): Boolean = synchronized(attempts) {
        val seen = attempts.merge(messageId, 1, Int::plus) ?: 1
        if (seen > flakyFailures) attempts.remove(messageId)
        seen <= flakyFailures
    }

    private fun reply(model: ModelId, history: List<StoredMessage>, last: StoredMessage): String {
        val earlier = history.filter { it.role == Role.USER && it.id != last.id }
        val memory = when {
            earlier.isEmpty() -> "This is the first message of the chat."
            else -> "This is message ${earlier.size + 1} of the chat; before it you wrote: " +
                "\"${earlier.last().content.excerpt()}\"."
        }
        return "[$model] You wrote: \"${last.content.excerpt()}\". $memory " +
            "No language model is connected: this text comes from the development server, " +
            "one token at a time, so the client can show streaming, stopping and retries."
    }

    companion object {
        val ECHO = ModelId("stub", "echo")
        val SLOW = ModelId("stub", "slow")
        val FLAKY = ModelId("stub", "flaky")
    }
}

private fun String.excerpt(): String {
    val flat = trim().replace(Regex("\\s+"), " ")
    return if (flat.length <= 120) flat else flat.take(120) + "…"
}

/** Words with the space that follows them, so concatenating the tokens gives the text back. */
internal fun tokens(text: String): List<String> = Regex("\\S+\\s*").findAll(text).map { it.value }.toList()
