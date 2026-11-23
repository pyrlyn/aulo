package aulo.chat.domain

data class ModelId(val provider: String, val model: String) {
    override fun toString(): String = "$provider/$model"
}

data class ModelInfo(val id: ModelId, val displayName: String, val served: Boolean)

data class ChatSummary(
    val id: String,
    val title: String,
    val model: ModelId,
    val updatedAtMs: Long,
)

enum class MessageRole { User, Assistant }

enum class MessageState {
    Complete,

    /** The answer is still arriving. Never stored: a cached message is always settled. */
    Streaming,

    /** The user stopped the answer; what had arrived is kept. */
    Interrupted,
    Failed,
}

enum class AttachmentKind { Text, Image, Voice }

/**
 * Something added to a message. Only [AttachmentKind.Text] carries [content], because only text
 * can travel today: `aulo.v1.UserTurn` has a text field and nothing else.
 */
data class Attachment(
    val kind: AttachmentKind,
    val name: String,
    val detail: String,
    val content: String? = null,
)

data class ChatMessage(
    val id: String,
    val chatId: String,
    val role: MessageRole,
    val text: String,
    val state: MessageState = MessageState.Complete,
    val error: String? = null,
    val attachments: List<Attachment> = emptyList(),
    val createdAtMs: Long = 0,
)

/** One message that matched a search. [excerpt] is server text: one line, drawn as plain text. */
data class MessageHit(val chatId: String, val messageId: String, val excerpt: String, val createdAtMs: Long)

/** Settings → Appearance. [System] follows the operating system and changes with it. */
enum class Appearance { System, Light, Dark }

sealed interface ConnectionState {
    data object Connecting : ConnectionState

    data object Online : ConnectionState

    data class Offline(val reason: String) : ConnectionState
}

enum class NoticeLevel { Info, Warning, Error }

/** A line for the banner: from the server (queued, retrying, rate limited) or from the client. */
data class Notice(val level: NoticeLevel, val message: String)

/** The server cannot be reached; the cache is all there is until it can. */
class OfflineException(message: String, cause: Throwable? = null) : Exception(message, cause)

/** The server answered and said no. */
class ServerException(message: String, cause: Throwable? = null) : Exception(message, cause)
