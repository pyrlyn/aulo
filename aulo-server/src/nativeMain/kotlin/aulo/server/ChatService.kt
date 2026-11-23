package aulo.server

import aulo.grpc.GrpcException
import aulo.grpc.StatusCode
import aulo.store.ChatCursor
import aulo.store.SearchCursor
import aulo.store.Store
import aulo.store.StoreException
import aulo.types.Ulid
import aulo.types.truncateUtf8
import aulo.v1.Chat
import aulo.v1.ChatServiceServer
import aulo.v1.CreateChatRequest
import aulo.v1.CreateChatResponse
import aulo.v1.DeleteChatRequest
import aulo.v1.DeleteChatResponse
import aulo.v1.GetChatRequest
import aulo.v1.GetChatResponse
import aulo.v1.ListChatsRequest
import aulo.v1.ListChatsResponse
import aulo.v1.ListMessagesRequest
import aulo.v1.ListMessagesResponse
import aulo.v1.Message
import aulo.v1.MessageRole
import aulo.v1.ModelRef
import aulo.v1.RenameChatRequest
import aulo.v1.RenameChatResponse
import aulo.v1.SearchMessagesRequest
import aulo.v1.SearchMessagesResponse
import aulo.v1.ToolCall
import aulo.v1.ToolCallStatus
import com.squareup.wire.Instant
import com.squareup.wire.ofEpochSecond
import aulo.store.Chat as StoredChat
import aulo.store.Message as StoredMessage
import aulo.store.ToolCall as StoredToolCall

/** Caps from chat.proto and common.proto, in UTF-8 bytes. */
object ChatLimits {
    const val MAX_TITLE_BYTES = 200
    const val MAX_QUERY_BYTES = 1024
    const val MAX_PROVIDER_BYTES = 64
    const val MAX_MODEL_BYTES = 128
    const val MAX_TOOL_OUTPUT_BYTES = 64 * 1024
}

/**
 * `aulo.v1.ChatService` over the daemon's store. [defaultModel] is read on every CreateChat that
 * names no model, so a reloaded config takes effect: the config's `tiers.main`, or null when it
 * is unset.
 */
class ChatService(
    private val store: SharedStore,
    private val defaultModel: () -> ModelRef?,
) : ChatServiceServer {
    override suspend fun CreateChat(request: CreateChatRequest): CreateChatResponse {
        val botId = request.bot_id?.let { ulid("bot_id", it) }
        val title = request.title.orEmpty().also { checkTitle(it) }
        val model =
            request.model?.also(::checkModel)
                ?: defaultModel()
                ?: throw GrpcException(StatusCode.FAILED_PRECONDITION, "no model given and tiers.main is not set")
        val chat =
            try {
                store.use { createChat(botId, title, "${model.provider}/${model.model}") }
            } catch (e: StoreException) {
                if (e.isMissingReference) throw GrpcException(StatusCode.NOT_FOUND, "no bot has that bot_id")
                throw e
            }
        return CreateChatResponse(chat = chat.toProto())
    }

    override suspend fun ListChats(request: ListChatsRequest): ListChatsResponse {
        val size = pageSize(request.page_size)
        val after = PageTokens.chatsAfter(request.page_token)
        val botId = request.bot_id?.let { ulid("bot_id", it) }
        val rows = store.use { listChatsPage(botId, after, size + 1) }
        return page(rows, size, { PageTokens.ofChat(ChatCursor(it)) }) { chats, next ->
            ListChatsResponse(chats = chats.map { it.toProto() }, next_page_token = next)
        }
    }

    override suspend fun GetChat(request: GetChatRequest): GetChatResponse {
        val id = ulid("chat_id", request.chat_id)
        return GetChatResponse(chat = (store.use { getChat(id) } ?: noChat()).toProto())
    }

    override suspend fun RenameChat(request: RenameChatRequest): RenameChatResponse {
        val id = ulid("chat_id", request.chat_id)
        checkTitle(request.title)
        if (request.title.isEmpty()) invalid("title must not be empty")
        // One turn on the store: the chat read back is the one just renamed.
        val chat = store.use { if (renameChat(id, request.title)) getChat(id) else null } ?: noChat()
        return RenameChatResponse(chat = chat.toProto())
    }

    override suspend fun DeleteChat(request: DeleteChatRequest): DeleteChatResponse {
        val id = ulid("chat_id", request.chat_id)
        if (!store.use { deleteChat(id) }) noChat()
        return DeleteChatResponse()
    }

    override suspend fun ListMessages(request: ListMessagesRequest): ListMessagesResponse {
        val chatId = ulid("chat_id", request.chat_id)
        val size = pageSize(request.page_size)
        val after = PageTokens.messagesAfter(request.page_token)
        // A missing chat is NOT_FOUND rather than an empty history, which would read as a new chat.
        val rows =
            store.use { getChat(chatId)?.let { withToolCalls(listMessages(chatId, after, size + 1)) } } ?: noChat()
        return page(rows, size, { PageTokens.ofMessage(it.id) }) { messages, next ->
            ListMessagesResponse(messages = messages, next_page_token = next)
        }
    }

    override suspend fun SearchMessages(request: SearchMessagesRequest): SearchMessagesResponse {
        if (request.query.encodeToByteArray().size > ChatLimits.MAX_QUERY_BYTES) {
            invalid("query is longer than ${ChatLimits.MAX_QUERY_BYTES} bytes")
        }
        val size = pageSize(request.page_size)
        val after = PageTokens.searchAfter(request.page_token)
        val chatId = request.chat_id?.let { ulid("chat_id", it) }
        val (hits, messages) =
            store.use {
                val hits = searchMessages(request.query, chatId, after, size + 1)
                hits to withToolCalls(hits.map { it.message })
            }
        val next = if (hits.size > size) PageTokens.ofHit(SearchCursor(hits[size - 1])) else ""
        return SearchMessagesResponse(messages = messages.take(size), next_page_token = next)
    }
}

private fun Store.withToolCalls(messages: List<StoredMessage>): List<Message> {
    val calls = listToolCallsFor(messages.map { it.id }).groupBy { it.messageId }
    return messages.map { it.toProto(calls[it.id].orEmpty()) }
}

private fun ulid(
    field: String,
    value: String,
): String = value.takeIf(Ulid::isValid) ?: invalid("$field is not a valid ULID")

private fun checkTitle(title: String) {
    if (title.encodeToByteArray().size > ChatLimits.MAX_TITLE_BYTES) invalid("title is longer than ${ChatLimits.MAX_TITLE_BYTES} bytes")
}

private fun checkModel(model: ModelRef) {
    val provider = model.provider.encodeToByteArray().size
    val name = model.model.encodeToByteArray().size
    // The store keeps `provider/model` and splits at the first slash, so a provider has none.
    if (provider == 0 || provider > ChatLimits.MAX_PROVIDER_BYTES || '/' in model.provider) {
        invalid("model.provider must be 1 to ${ChatLimits.MAX_PROVIDER_BYTES} bytes without '/'")
    }
    if (name == 0 || name > ChatLimits.MAX_MODEL_BYTES) invalid("model.model must be 1 to ${ChatLimits.MAX_MODEL_BYTES} bytes")
}

private fun invalid(message: String): Nothing = throw GrpcException(StatusCode.INVALID_ARGUMENT, message)

private fun noChat(): Nothing = throw GrpcException(StatusCode.NOT_FOUND, "no chat has that chat_id")

private fun StoredChat.toProto() =
    Chat(
        id = id,
        bot_id = botId,
        // Titles may come from the model later (the cheap tier names chats): the cap holds whatever wrote it.
        title = title.truncateUtf8(ChatLimits.MAX_TITLE_BYTES),
        model =
            model.indexOf('/').let {
                if (it <
                    0
                ) {
                    ModelRef(model = model)
                } else {
                    ModelRef(model.substring(0, it), model.substring(it + 1))
                }
            },
        created_at = instant(createdAt),
        updated_at = instant(updatedAt),
    )

private fun StoredMessage.toProto(calls: List<StoredToolCall>) =
    Message(
        id = id,
        chat_id = chatId,
        role = ROLES[role] ?: MessageRole.MESSAGE_ROLE_UNSPECIFIED,
        content = content,
        tool_calls = calls.map { it.toProto() },
        created_at = instant(createdAt),
    )

private fun StoredToolCall.toProto() =
    ToolCall(
        id = id,
        name = name,
        arguments_json = arguments,
        output = output?.truncateUtf8(ChatLimits.MAX_TOOL_OUTPUT_BYTES),
        status = STATUSES[status] ?: ToolCallStatus.TOOL_CALL_STATUS_UNSPECIFIED,
        created_at = instant(createdAt),
    )

// The store keeps roles and statuses as lowercase words until aulo-types owns those enums.
private val ROLES =
    mapOf(
        "user" to MessageRole.MESSAGE_ROLE_USER,
        "assistant" to MessageRole.MESSAGE_ROLE_ASSISTANT,
        "tool" to MessageRole.MESSAGE_ROLE_TOOL,
    )

private val STATUSES =
    mapOf(
        "running" to ToolCallStatus.TOOL_CALL_STATUS_RUNNING,
        "succeeded" to ToolCallStatus.TOOL_CALL_STATUS_SUCCEEDED,
        "failed" to ToolCallStatus.TOOL_CALL_STATUS_FAILED,
        "denied" to ToolCallStatus.TOOL_CALL_STATUS_DENIED,
        "cancelled" to ToolCallStatus.TOOL_CALL_STATUS_CANCELLED,
    )

private fun instant(epochMillis: Long): Instant = ofEpochSecond(epochMillis.floorDiv(1000L), epochMillis.mod(1000L) * 1_000_000L)
