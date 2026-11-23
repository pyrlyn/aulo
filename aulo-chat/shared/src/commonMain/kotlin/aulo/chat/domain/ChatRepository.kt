package aulo.chat.domain

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.StateFlow

/** The chats as the app sees them: the server when it is reachable, the cache when it is not. */
interface ChatRepository {
    val connection: StateFlow<ConnectionState>

    /** The cached chats, most recent first. */
    val chats: Flow<List<ChatSummary>>

    /**
     * Reaches the server, mirrors its recent chats into the cache and returns the chat to show:
     * the most recent one, or a new one when there is none. Offline it returns the most recent
     * cached chat, or null when nothing was ever cached. It reports failure through
     * [connection] instead of throwing.
     */
    suspend fun connect(): ChatSummary?

    /** @throws OfflineException when the server cannot be reached. */
    suspend fun models(): List<ModelInfo>

    /** @throws OfflineException when the server cannot be reached. */
    suspend fun newChat(): ChatSummary

    /** Opens the conversation stream of one chat; it lives until [ChatSession.close] or [scope] ends. */
    fun open(chat: ChatSummary, scope: CoroutineScope): ChatSession
}

interface ChatSession {
    val chatId: String

    /** The cached messages followed by the answer being streamed, if any. */
    val messages: Flow<List<ChatMessage>>

    val generating: StateFlow<Boolean>

    val notices: Flow<Notice>

    /** @throws OfflineException when the stream is down; nothing is stored then. */
    suspend fun send(text: String, attachments: List<Attachment> = emptyList())

    suspend fun stop()

    suspend fun switchModel(model: ModelId)

    fun close()
}

/** Where attachments come from. Platform pickers plug in here. */
interface AttachmentPicker {
    /** Null when the user cancelled. */
    suspend fun pick(kind: AttachmentKind): Attachment?
}
