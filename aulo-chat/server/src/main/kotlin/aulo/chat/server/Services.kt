package aulo.chat.server

import aulo.v1.Chat
import aulo.v1.ChatService
import aulo.v1.ConfigService
import aulo.v1.CreateChatRequest
import aulo.v1.CreateChatResponse
import aulo.v1.DeleteChatRequest
import aulo.v1.DeleteChatResponse
import aulo.v1.GetChatRequest
import aulo.v1.GetChatResponse
import aulo.v1.GetConfigRequest
import aulo.v1.GetConfigResponse
import aulo.v1.ListAudioDevicesRequest
import aulo.v1.ListAudioDevicesResponse
import aulo.v1.ListChatsRequest
import aulo.v1.ListChatsResponse
import aulo.v1.ListMessagesRequest
import aulo.v1.ListMessagesResponse
import aulo.v1.ListModelsRequest
import aulo.v1.ListModelsResponse
import aulo.v1.ListProvidersRequest
import aulo.v1.ListProvidersResponse
import aulo.v1.Message
import aulo.v1.MessageRole
import aulo.v1.Model
import aulo.v1.ModelRef
import aulo.v1.RenameChatRequest
import aulo.v1.RenameChatResponse
import aulo.v1.SearchMessagesRequest
import aulo.v1.SearchMessagesResponse
import aulo.v1.SetConfigRequest
import aulo.v1.SetConfigResponse
import aulo.v1.SetDeviceRequest
import aulo.v1.SetDeviceResponse
import aulo.v1.SetProviderKeyRequest
import aulo.v1.SetProviderKeyResponse
import aulo.v1.invoke
import aulo.v1.presence
import com.google.protobuf.kotlin.Timestamp
import com.google.protobuf.kotlin.invoke
import kotlinx.rpc.grpc.GrpcStatus
import kotlinx.rpc.grpc.GrpcStatusCode
import kotlinx.rpc.grpc.GrpcStatusException

internal fun status(code: GrpcStatusCode, message: String) = GrpcStatusException(GrpcStatus(code, message))

internal fun invalid(message: String) = status(GrpcStatusCode.INVALID_ARGUMENT, message)

private fun notFound(chatId: String) = status(GrpcStatusCode.NOT_FOUND, "no chat $chatId")

private fun notServed(method: String) =
    status(GrpcStatusCode.UNIMPLEMENTED, "$method is not served by the aulo-chat development server")

internal fun ModelId.toProto() = ModelRef {
    provider = this@toProto.provider
    model = this@toProto.model
}

internal fun ModelRef.toId() = ModelId(provider, model)

private fun timestamp(ms: Long) = Timestamp {
    seconds = Math.floorDiv(ms, 1000)
    nanos = Math.floorMod(ms, 1000) * 1_000_000
}

private fun StoredChat.toProto() = Chat {
    id = this@toProto.id
    title = this@toProto.title
    model = this@toProto.model.toProto()
    createdAt = timestamp(createdAtMs)
    updatedAt = timestamp(updatedAtMs)
}

private fun StoredMessage.toProto() = Message {
    id = this@toProto.id
    chatId = this@toProto.chatId
    role = when (this@toProto.role) {
        Role.USER -> MessageRole.USER
        Role.ASSISTANT -> MessageRole.ASSISTANT
    }
    content = this@toProto.content
    createdAt = timestamp(createdAtMs)
}

private const val DEFAULT_PAGE = 50
private const val MAX_PAGE = 200

/** The page token is the offset of the next item; anything else is refused, not guessed at. */
private fun <T> List<T>.page(pageSize: UInt, pageToken: String): Pair<List<T>, String> {
    val offset = if (pageToken.isEmpty()) 0 else pageToken.toIntOrNull()?.takeIf { it in 0..size }
        ?: throw invalid("unknown page_token")
    val limit = if (pageSize == 0u) DEFAULT_PAGE else pageSize.coerceAtMost(MAX_PAGE.toUInt()).toInt()
    val end = minOf(size, offset + limit)
    return subList(offset, end) to if (end < size) end.toString() else ""
}

private fun checkTitle(title: String): String {
    if (title.toByteArray().size > MAX_TITLE_BYTES) throw invalid("title is longer than $MAX_TITLE_BYTES bytes")
    return title
}

class ChatServiceImpl(
    private val store: ChatStore,
    private val model: ChatModel,
    private val limiter: RateLimiter,
) : ChatService {
    override suspend fun createChat(message: CreateChatRequest): CreateChatResponse {
        val chosen = if (message.presence.hasModel) message.model.toId() else model.models.first().id
        if (model.models.none { it.id == chosen }) throw invalid("unknown model $chosen")
        val created = store.createChat(checkTitle(message.title), chosen)
        return CreateChatResponse { chat = created.toProto() }
    }

    override suspend fun listChats(message: ListChatsRequest): ListChatsResponse {
        val (items, next) = store.listChats().page(message.pageSize, message.pageToken)
        return ListChatsResponse {
            chats = items.map { it.toProto() }
            nextPageToken = next
        }
    }

    override suspend fun getChat(message: GetChatRequest): GetChatResponse {
        val found = store.chat(message.chatId) ?: throw notFound(message.chatId)
        return GetChatResponse { chat = found.toProto() }
    }

    override suspend fun renameChat(message: RenameChatRequest): RenameChatResponse {
        if (message.title.isBlank()) throw invalid("title is empty")
        val renamed = store.rename(message.chatId, checkTitle(message.title)) ?: throw notFound(message.chatId)
        return RenameChatResponse { chat = renamed.toProto() }
    }

    override suspend fun deleteChat(message: DeleteChatRequest): DeleteChatResponse {
        if (!store.delete(message.chatId)) throw notFound(message.chatId)
        limiter.forget(message.chatId)
        return DeleteChatResponse { }
    }

    override suspend fun listMessages(message: ListMessagesRequest): ListMessagesResponse {
        store.chat(message.chatId) ?: throw notFound(message.chatId)
        val (items, next) = store.messages(message.chatId).page(message.pageSize, message.pageToken)
        return ListMessagesResponse {
            messages = items.map { it.toProto() }
            nextPageToken = next
        }
    }

    override suspend fun searchMessages(message: SearchMessagesRequest): SearchMessagesResponse =
        throw notServed("SearchMessages")
}

/** Only the model list is real; the rest of the daemon's configuration has no stand-in. */
class ConfigServiceImpl(private val model: ChatModel) : ConfigService {
    override suspend fun listModels(message: ListModelsRequest): ListModelsResponse {
        val wanted = if (message.presence.hasProviderId) message.providerId else null
        val (items, next) = model.models
            .filter { wanted == null || it.id.provider == wanted }
            .page(message.pageSize, message.pageToken)
        return ListModelsResponse {
            models = items.map { spec ->
                Model {
                    ref = spec.id.toProto()
                    displayName = spec.displayName
                    served = true
                }
            }
            nextPageToken = next
        }
    }

    override suspend fun listProviders(message: ListProvidersRequest): ListProvidersResponse =
        throw notServed("ListProviders")

    override suspend fun setProviderKey(message: SetProviderKeyRequest): SetProviderKeyResponse =
        throw notServed("SetProviderKey")

    override suspend fun listAudioDevices(message: ListAudioDevicesRequest): ListAudioDevicesResponse =
        throw notServed("ListAudioDevices")

    override suspend fun setDevice(message: SetDeviceRequest): SetDeviceResponse = throw notServed("SetDevice")

    override suspend fun getConfig(message: GetConfigRequest): GetConfigResponse = throw notServed("GetConfig")

    override suspend fun setConfig(message: SetConfigRequest): SetConfigResponse = throw notServed("SetConfig")
}
