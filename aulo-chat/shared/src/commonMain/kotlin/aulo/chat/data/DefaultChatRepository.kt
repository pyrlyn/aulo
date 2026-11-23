package aulo.chat.data

import aulo.chat.domain.Attachment
import aulo.chat.domain.AttachmentKind
import aulo.chat.domain.AttachmentPicker
import aulo.chat.domain.ChatMessage
import aulo.chat.domain.ChatRepository
import aulo.chat.domain.ChatSession
import aulo.chat.domain.ChatSummary
import aulo.chat.domain.ConnectionState
import aulo.chat.domain.MessageRole
import aulo.chat.domain.MessageState
import aulo.chat.domain.ModelId
import aulo.chat.domain.ModelInfo
import aulo.chat.domain.Notice
import aulo.chat.domain.OfflineException
import aulo.chat.domain.ServerException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.random.Random
import kotlin.time.Clock
import kotlin.time.ExperimentalTime

@OptIn(ExperimentalTime::class)
private fun systemNowMs(): Long = Clock.System.now().toEpochMilliseconds()

class DefaultChatRepository(
    private val remote: ChatRemote,
    private val cache: ChatCache,
    private val nowMs: () -> Long = ::systemNowMs,
) : ChatRepository {
    private val state = MutableStateFlow<ConnectionState>(ConnectionState.Connecting)
    override val connection: StateFlow<ConnectionState> = state.asStateFlow()

    override val chats: Flow<List<ChatSummary>> = cache.chats

    override suspend fun connect(): ChatSummary? {
        state.value = ConnectionState.Connecting
        return try {
            val recent = remote.listChats(CACHED_CHATS).ifEmpty { listOf(remote.createChat()) }
            val current = recent.first()
            val messages = remote.listMessages(current.id)
            cache.replaceChats(recent)
            cache.replaceMessages(current.id, messages)
            state.value = ConnectionState.Online
            current
        } catch (e: OfflineException) {
            offline(e)
        } catch (e: ServerException) {
            offline(e)
        }
    }

    private suspend fun offline(cause: Exception): ChatSummary? {
        state.value = ConnectionState.Offline(cause.message ?: "The server cannot be reached.")
        return cache.latestChat()
    }

    override suspend fun models(): List<ModelInfo> = guarded { remote.listModels() }

    override suspend fun newChat(): ChatSummary = guarded { remote.createChat().also { cache.upsertChat(it) } }

    override fun open(chat: ChatSummary, scope: CoroutineScope): ChatSession =
        DefaultChatSession(chat, remote, cache, scope, nowMs) { reason -> state.value = ConnectionState.Offline(reason) }

    private suspend fun <T> guarded(block: suspend () -> T): T = try {
        block()
    } catch (e: OfflineException) {
        state.value = ConnectionState.Offline(e.message ?: "The server cannot be reached.")
        throw e
    }
}

/**
 * One chat's stream. Settled messages live in the cache; the answer in flight lives in memory
 * and is written once, when its turn ends, so a token never costs a disk write.
 */
internal class DefaultChatSession(
    private var chat: ChatSummary,
    remote: ChatRemote,
    private val cache: ChatCache,
    scope: CoroutineScope,
    private val nowMs: () -> Long,
    private val onDisconnected: (reason: String) -> Unit,
) : ChatSession {
    override val chatId: String get() = chat.id

    private val out = Channel<Submission>(Channel.UNLIMITED)
    private val live = MutableStateFlow<ChatMessage?>(null)
    private val busy = MutableStateFlow(false)
    private val noticed = MutableSharedFlow<Notice>(extraBufferCapacity = 16)

    override val generating: StateFlow<Boolean> = busy.asStateFlow()
    override val notices: Flow<Notice> = noticed
    override val messages: Flow<List<ChatMessage>> =
        combine(cache.messages(chat.id), live) { saved, streaming ->
            // While an answer settles it is, for a moment, both saved and live; it is shown once.
            if (streaming == null || saved.any { it.id == streaming.id }) saved else saved + streaming
        }

    private val stream = scope.launch {
        val reason = try {
            remote.converse(chat.id, out.receiveAsFlow()).collect(::onEvent)
            "The server closed the conversation."
        } catch (e: OfflineException) {
            e.message
        } catch (e: ServerException) {
            e.message
        }
        // Reached only when the stream ended without this session being closed.
        settle(MessageState.Failed, "The connection was lost before the answer finished.")
        onDisconnected(reason ?: "The server cannot be reached.")
    }

    private suspend fun onEvent(event: TurnEvent) {
        when (event) {
            is TurnEvent.Started -> live.value = placeholder(event.turnId)
            is TurnEvent.Delta -> live.update { current ->
                val message = current ?: placeholder(event.turnId)
                // Model output is untrusted: it is capped here and drawn as plain text.
                message.copy(text = (message.text + event.text).take(MAX_MESSAGE_CHARS))
            }

            is TurnEvent.Finished -> when (event.end) {
                TurnEnd.Completed -> settle(MessageState.Complete, null)
                TurnEnd.Interrupted -> settle(MessageState.Interrupted, null)
                TurnEnd.Failed -> settle(MessageState.Failed, event.error ?: "The turn failed.")
            }

            is TurnEvent.Noticed -> noticed.emit(event.notice)
        }
    }

    private fun placeholder(id: String) = ChatMessage(
        id = id,
        chatId = chat.id,
        role = MessageRole.Assistant,
        text = "",
        state = MessageState.Streaming,
        createdAtMs = nowMs(),
    )

    /** Moves the answer in flight into the cache with its final [state]. */
    private suspend fun settle(state: MessageState, error: String?) = withContext(NonCancellable) {
        val message = live.value
        if (message != null) {
            // Written before the live copy is dropped, so the list never shows a gap between them.
            cache.appendMessage(message.copy(state = state, error = error))
            live.value = null
        }
        // Last, so Send is not offered while the answer is still on its way into the cache.
        busy.value = false
    }

    override suspend fun send(text: String, attachments: List<Attachment>) {
        if (!stream.isActive) throw OfflineException("The server cannot be reached.")
        val now = nowMs()
        cache.appendMessage(
            ChatMessage(localId(), chat.id, MessageRole.User, text, attachments = attachments, createdAtMs = now),
        )
        if (chat.title.isEmpty()) {
            // The server titles an untitled chat after its first turn; mirror that for the cache.
            chat = chat.copy(title = text.lineSequence().firstOrNull { it.isNotBlank() }.orEmpty().trim().take(48))
        }
        chat = chat.copy(updatedAtMs = now)
        cache.upsertChat(chat)
        busy.value = true
        live.value = placeholder(localId())
        out.send(Submission.UserTurn(text))
    }

    override suspend fun stop() {
        if (busy.value) out.send(Submission.Interrupt)
    }

    override suspend fun switchModel(model: ModelId) {
        if (!stream.isActive) throw OfflineException("The server cannot be reached.")
        out.send(Submission.SwitchModel(model))
        chat = chat.copy(model = model)
        cache.upsertChat(chat)
    }

    override fun close() {
        out.close()
        stream.cancel()
    }

    private fun localId() = "local-" + Random.nextLong().toULong().toString(16)
}

/**
 * Stands in for the platform pickers (file dialog, photo picker, microphone), which are not
 * wired yet: it hands back a small built-in sample of the requested kind so the whole
 * attachment flow can be exercised on every platform.
 */
class SampleAttachmentPicker : AttachmentPicker {
    private var picked = 0

    override suspend fun pick(kind: AttachmentKind): Attachment {
        val n = ++picked
        return when (kind) {
            AttachmentKind.Text -> Attachment(
                kind = kind,
                name = "sample-$n.txt",
                detail = "1 KB",
                content = "Sample note $n: the budget is 600 EUR for three days, flights excluded.",
            )

            AttachmentKind.Image -> Attachment(kind, "sample-$n.png", "340 KB")
            AttachmentKind.Voice -> Attachment(kind, "voice-$n.m4a", "0:07")
        }
    }
}
