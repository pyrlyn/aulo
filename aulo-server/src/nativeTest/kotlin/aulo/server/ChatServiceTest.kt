package aulo.server

import aulo.config.DaemonConfig
import aulo.grpc.GrpcClient
import aulo.grpc.GrpcException
import aulo.grpc.StatusCode
import aulo.server.generated.grpcHandlers
import aulo.store.Store
import aulo.store.databasePath
import aulo.v1.Chat
import aulo.v1.CreateChatRequest
import aulo.v1.CreateChatResponse
import aulo.v1.DeleteChatRequest
import aulo.v1.GetChatRequest
import aulo.v1.GetChatResponse
import aulo.v1.ListChatsRequest
import aulo.v1.ListChatsResponse
import aulo.v1.ListMessagesRequest
import aulo.v1.ListMessagesResponse
import aulo.v1.MessageRole
import aulo.v1.ModelRef
import aulo.v1.RenameChatRequest
import aulo.v1.RenameChatResponse
import aulo.v1.SearchMessagesRequest
import aulo.v1.SearchMessagesResponse
import aulo.v1.ToolCallStatus
import fs.TempDir
import kotlin.io.encoding.Base64
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.time.Duration.Companion.seconds

private const val SERVICE = "/aulo.v1.ChatService"
private val MAIN = ModelRef(provider = "anthropic", model = "claude-sonnet-5-5")

// A canonical ULID that names nothing.
private const val NOBODY = "01JZZZZZZZZZZZZZZZZZZZZZZZ"

class ChatServiceTest {
    @Test
    fun a_chat_is_created_read_renamed_and_deleted() =
        chats {
            val chat = create(CreateChatRequest(title = "plans"))
            assertEquals("plans", chat.title)
            assertEquals(MAIN, chat.model)
            assertNull(chat.bot_id)
            assertEquals(chat, get(chat.id))

            val renamed = rename(chat.id, "holiday plans")
            assertEquals("holiday plans", renamed.title)
            assertEquals(chat.created_at, renamed.created_at)
            assertEquals(renamed, get(chat.id))

            delete(chat.id)
            assertStatus(StatusCode.NOT_FOUND) { get(chat.id) }
            assertStatus(StatusCode.NOT_FOUND) { delete(chat.id) }
            assertStatus(StatusCode.NOT_FOUND) { rename(chat.id, "x") }
        }

    @Test
    fun create_takes_the_main_tier_unless_a_model_is_named() =
        chats {
            // A model id may hold a slash (OpenRouter's ids do); the provider never does.
            val named = ModelRef(provider = "openrouter", model = "anthropic/claude-opus-5-5")
            val chat = create(CreateChatRequest(model = named))
            assertEquals(named, chat.model)
            assertEquals("openrouter/anthropic/claude-opus-5-5", store.use { getChat(chat.id) }!!.model)
            assertEquals(named, get(chat.id).model)
            assertEquals("", create().title)

            val bot = store.use { createBot("helper") }
            assertEquals(bot.id, create(CreateChatRequest(bot_id = bot.id)).bot_id)
            assertStatus(StatusCode.NOT_FOUND, "no bot has that bot_id") { create(CreateChatRequest(bot_id = NOBODY)) }
        }

    @Test
    fun create_without_a_model_or_a_main_tier_fails() =
        chats(defaultModel = null) {
            assertStatus(StatusCode.FAILED_PRECONDITION) { create() }
            assertEquals(MAIN, create(CreateChatRequest(model = MAIN)).model)
        }

    @Test
    fun malformed_arguments_are_refused_without_echoing_them() =
        chats {
            val chat = create()
            val long = "x".repeat(10_000)
            val e = assertFailsWith<GrpcException> { get(long) }
            assertEquals(StatusCode.INVALID_ARGUMENT, e.status.code)
            assertFalse(long.take(16) in e.status.message, "the id is not echoed")
            assertStatus(StatusCode.INVALID_ARGUMENT) { get(chat.id.lowercase()) }
            assertStatus(StatusCode.INVALID_ARGUMENT) { delete("") }
            assertStatus(StatusCode.INVALID_ARGUMENT) { create(CreateChatRequest(bot_id = "bot")) }
            assertStatus(StatusCode.INVALID_ARGUMENT) { listChats(ListChatsRequest(bot_id = "bot")) }
            assertStatus(StatusCode.INVALID_ARGUMENT) { messages(ListMessagesRequest(chat_id = "chat")) }
            assertStatus(StatusCode.INVALID_ARGUMENT) { search(SearchMessagesRequest(query = "a", chat_id = "chat")) }

            // 200 bytes is the cap, counted in UTF-8: 100 two-byte letters fit, 101 do not.
            val fits = "я".repeat(100)
            assertEquals(fits, rename(chat.id, fits).title)
            assertStatus(StatusCode.INVALID_ARGUMENT) { rename(chat.id, fits + "я") }
            assertStatus(StatusCode.INVALID_ARGUMENT) { create(CreateChatRequest(title = fits + "я")) }
            assertStatus(StatusCode.INVALID_ARGUMENT) { rename(chat.id, "") }

            for (bad in listOf(
                ModelRef(provider = "", model = "m"),
                ModelRef(provider = "a/b", model = "m"),
                ModelRef(provider = "p".repeat(65), model = "m"),
                ModelRef(provider = "p", model = ""),
                ModelRef(provider = "p", model = "m".repeat(129)),
            )) {
                assertStatus(StatusCode.INVALID_ARGUMENT) { create(CreateChatRequest(model = bad)) }
            }
            create(CreateChatRequest(model = ModelRef(provider = "p".repeat(64), model = "m".repeat(128))))

            search(SearchMessagesRequest(query = "q".repeat(ChatLimits.MAX_QUERY_BYTES)))
            assertStatus(StatusCode.INVALID_ARGUMENT) { search(SearchMessagesRequest(query = "q".repeat(ChatLimits.MAX_QUERY_BYTES + 1))) }
        }

    @Test
    fun page_tokens_from_elsewhere_are_refused() =
        chats {
            create()
            val chat = create()
            repeat(3) { store.use { appendMessage(chat.id, "user", "m$it") } }
            val chatsToken = listChats(ListChatsRequest(page_size = 1)).next_page_token
            val messagesToken = messages(ListMessagesRequest(chat_id = chat.id, page_size = 1)).next_page_token
            assertTrue(chatsToken.isNotEmpty() && messagesToken.isNotEmpty())

            val url = Base64.UrlSafe.withPadding(Base64.PaddingOption.ABSENT)
            val forged =
                listOf(
                    "not base64!",
                    "A".repeat(MAX_PAGE_TOKEN_CHARS + 1),
                    url.encode("m|not-a-ulid".encodeToByteArray()),
                    url.encode("m|${chat.id}|extra".encodeToByteArray()),
                    url.encode("c|-1|${chat.id}".encodeToByteArray()),
                    url.encode("s|${Double.NaN.toRawBits()}|${chat.id}".encodeToByteArray()),
                    url.encode(byteArrayOf(0xC3.toByte(), 0x28)),
                    // Each list's token is refused by the others.
                    chatsToken,
                )
            for (token in forged) {
                assertStatus(StatusCode.INVALID_ARGUMENT, "page_token is not valid") {
                    messages(ListMessagesRequest(chat_id = chat.id, page_token = token))
                }
            }
            assertStatus(StatusCode.INVALID_ARGUMENT) { listChats(ListChatsRequest(page_token = messagesToken)) }
            assertStatus(StatusCode.INVALID_ARGUMENT) { search(SearchMessagesRequest(query = "m0", page_token = chatsToken)) }
        }

    @Test
    fun chats_page_newest_first_without_gaps_or_repeats() =
        chats {
            val bot = store.use { createBot("helper") }
            repeat(4) { create() }
            repeat(3) { create(CreateChatRequest(bot_id = bot.id)) }
            // A message bumps a chat to the front, between the pages too.
            val first = store.use { listChats(100) }.last()
            store.use { appendMessage(first.id, "user", "bump") }
            val want = store.use { listChats(100) }.map { it.id }
            assertEquals(first.id, want.first())

            assertEquals(
                want,
                pageAll { token ->
                    listChats(ListChatsRequest(page_size = 3, page_token = token)).let {
                        it.chats.map(Chat::id) to
                            it.next_page_token
                    }
                },
            )
            val ofBot =
                pageAll { token ->
                    listChats(ListChatsRequest(page_size = 2, page_token = token, bot_id = bot.id)).let {
                        it.chats.map(Chat::id) to
                            it.next_page_token
                    }
                }
            assertEquals(want.filter { id -> store.use { getChat(id) }!!.botId == bot.id }, ofBot)
            assertEquals(3, ofBot.size)
        }

    @Test
    fun page_size_zero_is_fifty_and_large_sizes_are_clamped() =
        chats {
            store.use { repeat(MAX_PAGE_SIZE + 1) { createChat(null, "c$it", "p/m") } }
            assertEquals(DEFAULT_PAGE_SIZE, listChats(ListChatsRequest()).chats.size)
            val clamped = listChats(ListChatsRequest(page_size = 1_000))
            assertEquals(MAX_PAGE_SIZE, clamped.chats.size)
            assertTrue(clamped.next_page_token.isNotEmpty())
            // A uint32 above 2^31 reaches Kotlin as a negative Int.
            assertEquals(MAX_PAGE_SIZE, listChats(ListChatsRequest(page_size = -1)).chats.size)
            val last = listChats(ListChatsRequest(page_size = 1_000, page_token = clamped.next_page_token))
            assertEquals(1, last.chats.size)
            assertEquals("", last.next_page_token)
        }

    @Test
    fun messages_page_oldest_first_with_their_tool_calls() =
        chats {
            val chat = create()
            val other = create()
            store.use { appendMessage(other.id, "user", "elsewhere") }
            val ids = store.use { (0 until 5).map { appendMessage(chat.id, if (it % 2 == 0) "user" else "assistant", "m$it").id } }
            val huge = "o".repeat(ChatLimits.MAX_TOOL_OUTPUT_BYTES + 100)
            store.use {
                recordToolCall(ids[1], "shell", "{\"cmd\":\"ls\"}", huge, "succeeded")
                recordToolCall(ids[1], "browse", "{}", null, "running")
                recordToolCall(ids[3], "odd", "{}", "x", "exploded")
            }

            val pages = mutableListOf<ListMessagesResponse>()
            var token = ""
            do {
                val page = messages(ListMessagesRequest(chat_id = chat.id, page_size = 2, page_token = token))
                pages += page
                token = page.next_page_token
            } while (token.isNotEmpty())
            assertEquals(listOf(2, 2, 1), pages.map { it.messages.size })
            val all = pages.flatMap { it.messages }
            assertEquals(ids, all.map { it.id })
            assertEquals(listOf("m0", "m1", "m2", "m3", "m4"), all.map { it.content })
            assertEquals(MessageRole.MESSAGE_ROLE_ASSISTANT, all[1].role)
            assertTrue(all.all { it.chat_id == chat.id })

            val (shell, browse) = all[1].tool_calls
            assertEquals("shell", shell.name)
            assertEquals(ToolCallStatus.TOOL_CALL_STATUS_SUCCEEDED, shell.status)
            assertEquals(ChatLimits.MAX_TOOL_OUTPUT_BYTES, shell.output!!.length)
            assertEquals(ToolCallStatus.TOOL_CALL_STATUS_RUNNING, browse.status)
            assertNull(browse.output)
            assertEquals(ToolCallStatus.TOOL_CALL_STATUS_UNSPECIFIED, all[3].tool_calls.single().status)
            assertTrue(all[0].tool_calls.isEmpty())

            assertStatus(StatusCode.NOT_FOUND) { messages(ListMessagesRequest(chat_id = NOBODY)) }
        }

    @Test
    fun deleting_a_chat_takes_its_messages_and_tool_calls() =
        chats {
            val chat = create()
            val message =
                store.use {
                    appendMessage(
                        chat.id,
                        "assistant",
                        "doomed",
                    ).also { recordToolCall(it.id, "t", "{}", "o", "succeeded") }
                }
            delete(chat.id)
            assertTrue(store.use { listMessages(chat.id, null, 10) }.isEmpty())
            assertTrue(store.use { listToolCalls(message.id) }.isEmpty())
            assertTrue(search(SearchMessagesRequest(query = "doomed")).messages.isEmpty())
        }

    @Test
    fun search_finds_words_across_chats_and_pages_by_rank() =
        chats {
            val a = create()
            val b = create()
            store.use {
                repeat(3) { appendMessage(a.id, "user", "zebra crossing $it") }
                repeat(2) { appendMessage(b.id, "assistant", "a zebra in chat b $it") }
                appendMessage(b.id, "user", "nothing to see")
            }
            val all = search(SearchMessagesRequest(query = "zebra")).messages
            assertEquals(5, all.size)
            assertEquals(setOf(a.id, b.id), all.map { it.chat_id }.toSet())

            val paged =
                pageAll { token ->
                    search(SearchMessagesRequest(query = "zebra", page_size = 2, page_token = token)).let { r ->
                        r.messages.map { it.id } to
                            r.next_page_token
                    }
                }
            assertEquals(all.map { it.id }, paged)

            assertEquals(2, search(SearchMessagesRequest(query = "zebra", chat_id = b.id)).messages.size)
            // Query syntax is plain words: an FTS operator or a lone quote matches nothing and does not fail.
            assertTrue(search(SearchMessagesRequest(query = "zebra NOT crossing")).messages.isEmpty())
            assertTrue(search(SearchMessagesRequest(query = "\"")).messages.isEmpty())
        }
}

private class Chats(
    val client: GrpcClient,
    val store: SharedStore,
) {
    suspend fun create(request: CreateChatRequest = CreateChatRequest()): Chat =
        CreateChatResponse.ADAPTER.decode(client.unary("$SERVICE/CreateChat", request.encode())).chat!!

    suspend fun get(id: String): Chat =
        GetChatResponse.ADAPTER.decode(client.unary("$SERVICE/GetChat", GetChatRequest(chat_id = id).encode())).chat!!

    suspend fun rename(
        id: String,
        title: String,
    ): Chat =
        RenameChatResponse.ADAPTER
            .decode(
                client.unary("$SERVICE/RenameChat", RenameChatRequest(chat_id = id, title = title).encode()),
            ).chat!!

    suspend fun delete(id: String) {
        client.unary("$SERVICE/DeleteChat", DeleteChatRequest(chat_id = id).encode())
    }

    suspend fun listChats(request: ListChatsRequest): ListChatsResponse =
        ListChatsResponse.ADAPTER.decode(client.unary("$SERVICE/ListChats", request.encode()))

    suspend fun messages(request: ListMessagesRequest): ListMessagesResponse =
        ListMessagesResponse.ADAPTER.decode(client.unary("$SERVICE/ListMessages", request.encode()))

    suspend fun search(request: SearchMessagesRequest): SearchMessagesResponse =
        SearchMessagesResponse.ADAPTER.decode(client.unary("$SERVICE/SearchMessages", request.encode()))

    /** Every id across the pages; fails on a repeat or a runaway token chain. */
    suspend fun pageAll(next: suspend (String) -> Pair<List<String>, String>): List<String> {
        val ids = mutableListOf<String>()
        var token = ""
        repeat(1_000) {
            val (page, following) = next(token)
            ids += page
            if (following.isEmpty()) {
                assertEquals(ids.size, ids.toSet().size, "no id repeats across pages")
                return ids
            }
            token = following
        }
        error("the page tokens never ended")
    }
}

/** A ChatService over a fresh database, served on the local socket from the default config. */
private fun chats(
    defaultModel: ModelRef? = MAIN,
    block: suspend Chats.() -> Unit,
) = transport {
    TempDir.create("aulo-chat").use { home ->
        Store.open(databasePath(home.path).toString()).use { raw ->
            val store = SharedStore(raw)
            val server = AuloServer(ChatService(store) { defaultModel }.grpcHandlers())
            val endpoint = server.start(Listeners.from(DaemonConfig(), home.path.toString())).single()
            val client = GrpcClient.connect(endpoint)
            try {
                Chats(client, store).block()
            } finally {
                client.close()
                server.shutdown(1.seconds)
            }
        }
    }
}
