package aulo.chat.data

import aulo.chat.domain.ChatMessage
import aulo.chat.domain.ChatSummary
import aulo.chat.domain.MessageHit
import aulo.chat.domain.MessageRole
import aulo.chat.domain.ModelId
import aulo.chat.domain.ModelInfo
import aulo.chat.domain.Notice
import aulo.chat.domain.NoticeLevel
import aulo.chat.domain.OfflineException
import aulo.chat.domain.ServerException
import aulo.v1.Attach
import aulo.v1.ChatService
import aulo.v1.ConfigService
import aulo.v1.ConverseRequest
import aulo.v1.CreateChatRequest
import aulo.v1.DeleteChatRequest
import aulo.v1.Event
import aulo.v1.Interrupt
import aulo.v1.ListChatsRequest
import aulo.v1.ListMessagesRequest
import aulo.v1.ListModelsRequest
import aulo.v1.ModelRef
import aulo.v1.RenameChatRequest
import aulo.v1.SearchMessagesRequest
import aulo.v1.SessionService
import aulo.v1.TurnEndReason
import aulo.v1.UserTurn
import aulo.v1.invoke
import com.google.protobuf.kotlin.Timestamp
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.emitAll
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.mapNotNull
import kotlinx.rpc.grpc.GrpcStatusCode
import kotlinx.rpc.grpc.GrpcStatusException
import kotlinx.rpc.grpc.client.GrpcClient
import kotlinx.rpc.grpc.status
import kotlinx.rpc.grpc.statusCode
import kotlinx.rpc.withService

/** Where the server listens. The development server speaks plaintext gRPC. */
data class ServerEndpoint(val host: String = "127.0.0.1", val port: Int = 50051) {
    companion object {
        /**
         * The endpoint a launcher was configured with (environment, Info.plist); a blank value
         * keeps the default. Null when [port] is not a port number.
         */
        fun parse(host: String?, port: String?): ServerEndpoint? {
            val default = ServerEndpoint()
            val number = when (val raw = port?.trim()?.ifEmpty { null }) {
                null -> default.port
                else -> raw.toIntOrNull()?.takeIf { it in 1..65535 } ?: return null
            }
            return ServerEndpoint(host?.trim()?.ifEmpty { null } ?: default.host, number)
        }
    }
}

sealed interface Submission {
    data class UserTurn(val text: String) : Submission

    data object Interrupt : Submission

    data class SwitchModel(val model: ModelId) : Submission
}

enum class TurnEnd { Completed, Interrupted, Failed }

sealed interface TurnEvent {
    data class Started(val turnId: String) : TurnEvent

    data class Delta(val turnId: String, val text: String) : TurnEvent

    data class Finished(val turnId: String, val end: TurnEnd, val error: String?) : TurnEvent

    data class Noticed(val notice: Notice) : TurnEvent
}

/**
 * The server, in the app's own types. Every call throws [OfflineException] when the server
 * cannot be reached and [ServerException] when it refuses.
 */
interface ChatRemote {
    suspend fun listChats(limit: Int): List<ChatSummary>

    /** [model] null leaves the choice to the server. */
    suspend fun createChat(model: ModelId? = null): ChatSummary

    suspend fun renameChat(chatId: String, title: String): ChatSummary

    suspend fun deleteChat(chatId: String)

    suspend fun listMessages(chatId: String): List<ChatMessage>

    /** The newest [limit] messages holding every word of [query]. */
    suspend fun searchMessages(query: String, limit: Int): List<MessageHit>

    suspend fun listModels(): List<ModelInfo>

    /** One `Converse` stream attached to [chatId]; it ends when [submissions] ends. */
    fun converse(chatId: String, submissions: Flow<Submission>): Flow<TurnEvent>

    /** Drops the connection; the remote is not used afterwards. */
    fun close() {}
}

/**
 * The remote the app talks to, which Settings can point at another server. Calls go to the
 * current server; [switchTo] replaces it only once the new one has answered, so a typo in the
 * address never cuts the app off from the server it has.
 */
class SwitchableChatRemote(
    endpoint: ServerEndpoint,
    private val connect: (ServerEndpoint) -> ChatRemote,
) : ChatRemote {
    private var current: ChatRemote = connect(endpoint)

    var endpoint: ServerEndpoint = endpoint
        private set

    /** @throws OfflineException or [ServerException] when [next] does not answer; nothing changes then. */
    suspend fun switchTo(next: ServerEndpoint) {
        val candidate = connect(next)
        try {
            candidate.listChats(1)
        } catch (e: Exception) {
            candidate.close()
            throw e
        }
        val old = current
        current = candidate
        endpoint = next
        old.close()
    }

    override suspend fun listChats(limit: Int) = current.listChats(limit)

    override suspend fun createChat(model: ModelId?) = current.createChat(model)

    override suspend fun renameChat(chatId: String, title: String) = current.renameChat(chatId, title)

    override suspend fun deleteChat(chatId: String) = current.deleteChat(chatId)

    override suspend fun listMessages(chatId: String) = current.listMessages(chatId)

    override suspend fun searchMessages(query: String, limit: Int) = current.searchMessages(query, limit)

    override suspend fun listModels() = current.listModels()

    override fun converse(chatId: String, submissions: Flow<Submission>) = current.converse(chatId, submissions)

    override fun close() = current.close()
}

/** `aulo.v1` over gRPC: the API aulod serves and the development server stands in for. */
class GrpcChatRemote(endpoint: ServerEndpoint) : ChatRemote {
    private val client = GrpcClient(endpoint.host, endpoint.port) { credentials = plaintext() }
    private val chats = client.withService<ChatService>()
    private val sessions = client.withService<SessionService>()
    private val config = client.withService<ConfigService>()

    override suspend fun listChats(limit: Int): List<ChatSummary> = call {
        chats.listChats(ListChatsRequest { pageSize = limit.toUInt() }).chats.map { it.toSummary() }
    }

    override suspend fun createChat(model: ModelId?): ChatSummary = call {
        chats.createChat(
            CreateChatRequest {
                if (model != null) this.model = model.toRef()
            },
        ).chat.toSummary()
    }

    override suspend fun renameChat(chatId: String, title: String): ChatSummary = call {
        chats.renameChat(
            RenameChatRequest {
                this.chatId = chatId
                this.title = title
            },
        ).chat.toSummary()
    }

    override suspend fun deleteChat(chatId: String) {
        call { chats.deleteChat(DeleteChatRequest { this.chatId = chatId }) }
    }

    override suspend fun searchMessages(query: String, limit: Int): List<MessageHit> = call {
        chats.searchMessages(
            SearchMessagesRequest {
                this.query = query
                pageSize = limit.toUInt()
            },
        ).messages.map { MessageHit(it.chatId, it.id, excerpt(it.content, query), it.createdAt.toMs()) }
    }

    override suspend fun listMessages(chatId: String): List<ChatMessage> = call {
        val all = mutableListOf<ChatMessage>()
        var token = ""
        do {
            val page = chats.listMessages(
                ListMessagesRequest {
                    this.chatId = chatId
                    pageToken = token
                },
            )
            for (message in page.messages) {
                val role = when (message.role) {
                    aulo.v1.MessageRole.USER -> MessageRole.User
                    aulo.v1.MessageRole.ASSISTANT -> MessageRole.Assistant
                    // Tool rows have no place in this screen yet.
                    else -> continue
                }
                all += ChatMessage(
                    id = message.id,
                    chatId = message.chatId,
                    role = role,
                    text = message.content.take(MAX_MESSAGE_CHARS),
                    createdAtMs = message.createdAt.toMs(),
                )
            }
            token = page.nextPageToken
        } while (token.isNotEmpty() && all.size < MAX_CACHED_MESSAGES)
        all
    }

    override suspend fun listModels(): List<ModelInfo> = call {
        config.listModels(ListModelsRequest { }).models.map {
            ModelInfo(it.ref.toId(), it.displayName.ifBlank { it.ref.toId().toString() }, it.served)
        }
    }

    override fun converse(chatId: String, submissions: Flow<Submission>): Flow<TurnEvent> {
        val requests = flow {
            emit(ConverseRequest.Submission.Attach(Attach { this.chatId = chatId }))
            emitAll(submissions.map { it.toProto() })
        }.map { ConverseRequest { submission = it } }
        return sessions.converse(requests)
            .mapNotNull { it.event.kind?.toDomain() }
            .catch { throw it.translated() }
    }

    /** Drops the connection. The app keeps one per server it talks to; tests open many. */
    override fun close() {
        client.shutdownNow()
    }

    private suspend fun <T> call(block: suspend () -> T): T = try {
        block()
    } catch (e: GrpcStatusException) {
        throw e.translated()
    }
}

/** A cap on what one message may hold on this device, whatever the server sends. */
const val MAX_MESSAGE_CHARS = 256 * 1024

const val MAX_CACHED_MESSAGES = 500

private fun Throwable.translated(): Throwable {
    if (this !is GrpcStatusException) return this
    val detail = status.detail() ?: status.statusCode.name
    return when (status.statusCode) {
        GrpcStatusCode.UNAVAILABLE, GrpcStatusCode.DEADLINE_EXCEEDED, GrpcStatusCode.CANCELLED, GrpcStatusCode.UNKNOWN ->
            OfflineException("The server cannot be reached.", this)

        else -> ServerException(detail, this)
    }
}

private fun Submission.toProto(): ConverseRequest.Submission = when (this) {
    is Submission.UserTurn -> ConverseRequest.Submission.UserTurn(UserTurn { text = this@toProto.text })
    Submission.Interrupt -> ConverseRequest.Submission.Interrupt(Interrupt { })
    is Submission.SwitchModel -> ConverseRequest.Submission.SwitchModel(
        aulo.v1.SwitchModel { model = this@toProto.model.toRef() },
    )
}

private fun ModelId.toRef() = ModelRef {
    provider = this@toRef.provider
    model = this@toRef.model
}

/** The longest excerpt a search hit shows; the row cuts it further to its width. */
const val MAX_EXCERPT_CHARS = 160

/**
 * One line of [content] around the first word of [query] it holds, so the row shows why the
 * message matched. Server text: whitespace is folded and the result is capped.
 */
internal fun excerpt(content: String, query: String): String {
    val flat = content.take(MAX_MESSAGE_CHARS).replace(Regex("\\s+"), " ").trim()
    val at = query.split(' ').filter { it.isNotBlank() }
        .map { flat.indexOf(it, ignoreCase = true) }
        .filter { it >= 0 }
        .minOrNull() ?: 0
    // A little context before the match, so it does not start the line cold.
    val start = (at - MAX_EXCERPT_CHARS / 4).coerceAtLeast(0)
    val cut = flat.substring(start).take(MAX_EXCERPT_CHARS)
    return (if (start > 0) "…" else "") + cut + (if (start + cut.length < flat.length) "…" else "")
}

private fun Event.Kind.toDomain(): TurnEvent? = when (this) {
    is Event.Kind.TurnStarted -> TurnEvent.Started(value.turnId)
    is Event.Kind.TextDelta -> TurnEvent.Delta(value.turnId, value.text)
    is Event.Kind.TurnFinished -> TurnEvent.Finished(
        turnId = value.turnId,
        end = when (value.reason) {
            TurnEndReason.COMPLETED -> TurnEnd.Completed
            TurnEndReason.INTERRUPTED -> TurnEnd.Interrupted
            else -> TurnEnd.Failed
        },
        error = value.error.takeIf { it.isNotBlank() },
    )

    is Event.Kind.Notice -> TurnEvent.Noticed(
        Notice(
            level = when (value.level) {
                aulo.v1.NoticeLevel.ERROR -> NoticeLevel.Error
                aulo.v1.NoticeLevel.WARN -> NoticeLevel.Warning
                else -> NoticeLevel.Info
            },
            message = value.message.take(MAX_NOTICE_CHARS),
        ),
    )

    // Voice, tools and approvals are other screens' business.
    else -> null
}

private const val MAX_NOTICE_CHARS = 500

private fun aulo.v1.Chat.toSummary() = ChatSummary(id, title, model.toId(), updatedAt.toMs())

private fun ModelRef.toId() = ModelId(provider, model)

private fun Timestamp.toMs(): Long = seconds * 1000 + nanos / 1_000_000
