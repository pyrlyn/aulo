package aulo.chat.data

import app.cash.sqldelight.coroutines.asFlow
import app.cash.sqldelight.coroutines.mapToList
import app.cash.sqldelight.db.SqlDriver
import aulo.chat.data.cache.Chat
import aulo.chat.data.cache.ChatDatabase
import aulo.chat.data.cache.Message
import aulo.chat.domain.Attachment
import aulo.chat.domain.AttachmentKind
import aulo.chat.domain.ChatMessage
import aulo.chat.domain.ChatSummary
import aulo.chat.domain.MessageRole
import aulo.chat.domain.MessageState
import aulo.chat.domain.ModelId
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withContext
import kotlin.coroutines.CoroutineContext

/** How many chats stay readable offline. */
const val CACHED_CHATS = 20

/**
 * The offline copy of the recent chats, in SQLite through SQLDelight. The server stays the
 * source of truth: [replaceChats] and [replaceMessages] overwrite what is here.
 */
class ChatCache(driver: SqlDriver, private val context: CoroutineContext = Dispatchers.Default) {
    private val queries = ChatDatabase(driver).chatQueries

    val chats: Flow<List<ChatSummary>> =
        queries.selectChats().asFlow().mapToList(context).map { rows -> rows.map { it.toSummary() } }

    fun messages(chatId: String): Flow<List<ChatMessage>> =
        queries.selectMessages(chatId).asFlow().mapToList(context).map { rows -> rows.map { it.toMessage() } }

    suspend fun latestChat(): ChatSummary? = withContext(context) {
        queries.selectChats().executeAsList().firstOrNull()?.toSummary()
    }

    suspend fun chat(id: String): ChatSummary? = withContext(context) {
        queries.selectChat(id).executeAsOneOrNull()?.toSummary()
    }

    /** Makes the cache hold exactly [recent]; chats that fell out take their messages along. */
    suspend fun replaceChats(recent: List<ChatSummary>) = withContext(context) {
        queries.transaction {
            queries.deleteAllChats()
            recent.take(CACHED_CHATS).forEach(::upsert)
            queries.deleteMessagesWithoutChat()
        }
    }

    suspend fun upsertChat(chat: ChatSummary) = withContext(context) { upsert(chat) }

    suspend fun replaceMessages(chatId: String, messages: List<ChatMessage>) = withContext(context) {
        queries.transaction {
            queries.deleteMessages(chatId)
            messages.forEach(::insert)
        }
    }

    suspend fun appendMessage(message: ChatMessage) = withContext(context) { insert(message) }

    private fun upsert(chat: ChatSummary) =
        queries.upsertChat(chat.id, chat.title, chat.model.provider, chat.model.model, chat.updatedAtMs)

    private fun insert(message: ChatMessage) {
        // A message still streaming has no place on disk: after a crash it would stream forever.
        val state = if (message.state == MessageState.Streaming) MessageState.Interrupted else message.state
        queries.insertMessage(
            id = message.id,
            chat_id = message.chatId,
            role = message.role.name,
            content = message.text,
            state = state.name,
            error = message.error,
            attachments = message.attachments.encode(),
            created_at_ms = message.createdAtMs,
        )
    }
}

private fun Chat.toSummary() = ChatSummary(id, title, ModelId(model_provider, model_name), updated_at_ms)

private fun Message.toMessage() = ChatMessage(
    id = id,
    chatId = chat_id,
    role = enumOr(role, MessageRole.Assistant),
    text = content,
    state = enumOr(state, MessageState.Complete),
    error = error,
    attachments = decodeAttachments(attachments),
    createdAtMs = created_at_ms,
)

// A row written by a newer build may hold a name this build does not know.
private inline fun <reified T : Enum<T>> enumOr(name: String, fallback: T): T =
    enumValues<T>().firstOrNull { it.name == name } ?: fallback

private const val FIELD = '\u001F'
private const val RECORD = '\u001E'

// Only labels are cached (kind, name, detail), so two control characters that cannot occur in a
// file name are enough of a format.
internal fun List<Attachment>.encode(): String = joinToString(RECORD.toString()) {
    listOf(it.kind.name, it.name.clean(), it.detail.clean()).joinToString(FIELD.toString())
}

internal fun decodeAttachments(encoded: String): List<Attachment> =
    encoded.split(RECORD).filter { it.isNotEmpty() }.mapNotNull { record ->
        val parts = record.split(FIELD)
        if (parts.size != 3) return@mapNotNull null
        Attachment(enumOr(parts[0], AttachmentKind.Text), parts[1], parts[2])
    }

private fun String.clean() = filterNot { it == FIELD || it == RECORD }
