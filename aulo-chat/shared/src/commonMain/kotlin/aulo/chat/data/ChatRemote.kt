package aulo.chat.data

import aulo.chat.domain.ChatMessage
import aulo.chat.domain.ChatSummary
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
import aulo.v1.Event
import aulo.v1.Interrupt
import aulo.v1.ListChatsRequest
import aulo.v1.ListMessagesRequest
import aulo.v1.ListModelsRequest
import aulo.v1.ModelRef
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

    suspend fun createChat(): ChatSummary

    suspend fun listMessages(chatId: String): List<ChatMessage>

    suspend fun listModels(): List<ModelInfo>

    /** One `Converse` stream attached to [chatId]; it ends when [submissions] ends. */
    fun converse(chatId: String, submissions: Flow<Submission>): Flow<TurnEvent>
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

    override suspend fun createChat(): ChatSummary = call {
        chats.createChat(CreateChatRequest { }).chat.toSummary()
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

    /** Drops the connection. The app keeps one for its whole life; tests open many. */
    fun shutdown() {
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
        aulo.v1.SwitchModel {
            model = ModelRef {
                provider = this@toProto.model.provider
                model = this@toProto.model.model
            }
        },
    )
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
