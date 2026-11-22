package aulo.store

// Row types. Ids are canonical ULIDs (aulo-types) and times are Unix milliseconds; `role` and `status` stay
// plain strings until aulo-types owns those enums.

data class Bot(
    val id: String,
    val name: String,
    val createdAt: Long,
)

data class Chat(
    val id: String,
    val botId: String?,
    val title: String,
    /** `provider/model`; the next request of this chat is built from it. */
    val model: String,
    val createdAt: Long,
    val updatedAt: Long,
)

/** Where a keyset page of chats continues: the last chat of the previous page. */
data class ChatCursor(
    val updatedAt: Long,
    val id: String,
) {
    constructor(chat: Chat) : this(chat.updatedAt, chat.id)
}

data class Message(
    val id: String,
    val chatId: String,
    val role: String,
    /** The text a person reads; listings and search serve this. */
    val content: String,
    val createdAt: Long,
)

/** A message as a resumed session reads it back; [wire] is null for rows written without one. */
data class HistoryMessage(
    val id: String,
    val role: String,
    val content: String,
    /** The provider-form message as JSON. */
    val wire: String?,
)

/** One message of a finished turn, with the tool calls it made. */
data class NewMessage(
    val role: String,
    val content: String = "",
    val wire: String? = null,
    val toolCalls: List<NewToolCall> = emptyList(),
)

data class NewToolCall(
    val name: String,
    val arguments: String,
    val output: String?,
    val status: String,
)

data class ToolCall(
    val id: String,
    val messageId: String,
    val name: String,
    val arguments: String,
    val output: String?,
    val status: String,
    val createdAt: Long,
)

data class Usage(
    val id: String,
    val chatId: String,
    val provider: String,
    val model: String,
    val inputTokens: Long,
    val outputTokens: Long,
    val createdAt: Long,
)

/** A standing decision for a subject and scope. Times are Unix milliseconds; a null [expiresAt] never expires. */
data class Grant(
    val id: String,
    val subject: String,
    val scope: String,
    val decision: String,
    val expiresAt: Long?,
    val createdAt: Long,
)

/** One audit row; [seq] is assigned by the database. */
data class AuditRow(
    val seq: Long,
    val prevHash: String,
    val hash: String,
    val kind: String,
    val payload: String,
    val createdAt: Long,
)
