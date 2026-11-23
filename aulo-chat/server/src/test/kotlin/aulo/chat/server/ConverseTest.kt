package aulo.chat.server

import aulo.v1.Attach
import aulo.v1.ChatService
import aulo.v1.ConfigService
import aulo.v1.ConverseRequest
import aulo.v1.CreateChatRequest
import aulo.v1.Event
import aulo.v1.GetChatRequest
import aulo.v1.Interrupt
import aulo.v1.ListMessagesRequest
import aulo.v1.ListModelsRequest
import aulo.v1.MessageRole
import aulo.v1.NoticeLevel
import aulo.v1.SessionService
import aulo.v1.SwitchModel
import aulo.v1.TurnEndReason
import aulo.v1.TurnFinished
import aulo.v1.UserTurn
import aulo.v1.invoke
import aulo.v1.presence
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ReceiveChannel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.produceIn
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.rpc.grpc.GrpcStatusCode
import kotlinx.rpc.grpc.GrpcStatusException
import kotlinx.rpc.grpc.client.GrpcClient
import kotlinx.rpc.grpc.status
import kotlinx.rpc.grpc.statusCode
import kotlinx.rpc.withService
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

/** The contract over a real gRPC connection, the way the client will use it. */
class ConverseTest {
    private class Harness(client: GrpcClient, val dev: DevServer) {
        val chats = client.withService<ChatService>()
        val sessions = client.withService<SessionService>()
        val config = client.withService<ConfigService>()

        suspend fun newChat(model: ModelId? = null): String = chats.createChat(
            CreateChatRequest { if (model != null) this.model = model.toProto() },
        ).chat.id
    }

    private class Session(private val out: Channel<ConverseRequest>, private val events: ReceiveChannel<Event.Kind>) {
        suspend fun send(submission: ConverseRequest.Submission) = out.send(ConverseRequest { this.submission = submission })

        suspend fun say(text: String) = send(ConverseRequest.Submission.UserTurn(UserTurn { this.text = text }))

        suspend fun next(): Event.Kind = events.receive()

        /** Every event of the running turn, ending with its TurnFinished. */
        suspend fun turn(): List<Event.Kind> = buildList {
            do {
                add(next())
            } while (last() !is Event.Kind.TurnFinished)
        }
    }

    private fun fastServer(
        limiter: RateLimiter = RateLimiter(),
        queue: RequestQueue = RequestQueue(),
        retry: RetryPolicy = RetryPolicy(baseDelay = 1.milliseconds),
        tokenDelay: Duration = 1.milliseconds,
    ) = DevServer(model = StubModel(tokenDelay = tokenDelay), limiter = limiter, queue = queue, retry = retry)

    // Slow enough that an Interrupt sent after the first token arrives long before the last.
    private fun slowServer(queue: RequestQueue = RequestQueue()) = fastServer(queue = queue, tokenDelay = 20.milliseconds)

    private fun withServer(dev: DevServer = fastServer(), block: suspend Harness.() -> Unit) = runBlocking {
        val server = dev.grpc(0).start()
        val client = GrpcClient("localhost", server.port) { credentials = plaintext() }
        try {
            withTimeout(30.seconds) { Harness(client, dev).block() }
        } finally {
            client.shutdownNow()
            server.shutdownNow()
        }
    }

    private suspend fun Harness.converse(chatId: String?, script: suspend Session.() -> Unit) = coroutineScope {
        val out = Channel<ConverseRequest>(Channel.UNLIMITED)
        val events = sessions.converse(out.receiveAsFlow()).map { it.event.kind!! }.produceIn(this)
        val session = Session(out, events)
        if (chatId != null) session.send(ConverseRequest.Submission.Attach(Attach { this.chatId = chatId }))
        try {
            session.script()
        } finally {
            out.close()
            events.cancel()
        }
    }

    private fun List<Event.Kind>.text(): String =
        filterIsInstance<Event.Kind.TextDelta>().joinToString("") { it.value.text }

    private fun List<Event.Kind>.finished(): TurnFinished = (last() as Event.Kind.TurnFinished).value

    @Test
    fun `a turn streams tokens and the chat remembers it`() = withServer {
        val chatId = newChat()
        converse(chatId) {
            say("The capital of France?")
            val first = turn()
            val started = assertIs<Event.Kind.TurnStarted>(first.first()).value
            assertEquals(26, started.turnId.length)
            assertTrue(first.count { it is Event.Kind.TextDelta } > 5, "the answer arrives in pieces")
            assertEquals(TurnEndReason.COMPLETED, first.finished().reason)
            assertEquals(started.turnId, first.finished().turnId)

            say("And of Spain?")
            val second = turn()
            assertContains(second.text(), "The capital of France?", message = "the model saw the earlier turn")

            val stored = chats.listMessages(ListMessagesRequest { this.chatId = chatId }).messages
            assertEquals(
                listOf(MessageRole.USER, MessageRole.ASSISTANT, MessageRole.USER, MessageRole.ASSISTANT),
                stored.map { it.role },
            )
            assertEquals(first.text(), stored[1].content)
            assertEquals(second.text(), stored[3].content)
        }
        val chat = chats.getChat(GetChatRequest { this.chatId = chatId }).chat
        assertEquals("The capital of France?", chat.title, "an untitled chat is named after its first message")
    }

    @Test
    fun `interrupt stops the turn and keeps what was shown`() = withServer(slowServer()) {
        val chatId = newChat(StubModel.SLOW)
        converse(chatId) {
            say("Tell me everything")
            val seen = mutableListOf<Event.Kind>()
            do {
                seen += next()
            } while (seen.last() !is Event.Kind.TextDelta)
            send(ConverseRequest.Submission.Interrupt(Interrupt { }))
            seen += turn()
            assertEquals(TurnEndReason.INTERRUPTED, seen.finished().reason)

            val stored = chats.listMessages(ListMessagesRequest { this.chatId = chatId }).messages
            assertEquals(2, stored.size)
            assertEquals(seen.text(), stored[1].content)
            assertTrue(stored[1].content.length < 100, "the answer was cut short")

            say("Again")
            assertEquals(TurnEndReason.COMPLETED, turn().finished().reason, "the stream stays usable")
        }
    }

    @Test
    fun `transient failures are retried and reported`() = withServer {
        val chatId = newChat(StubModel.FLAKY)
        converse(chatId) {
            say("hello")
            val events = turn()
            val notices = events.filterIsInstance<Event.Kind.Notice>().map { it.value }
            assertEquals(2, notices.size)
            assertTrue(notices.all { it.level == NoticeLevel.WARN && "retry" in it.message })
            assertEquals(TurnEndReason.COMPLETED, events.finished().reason)
        }
    }

    @Test
    fun `a turn the model refuses fails with its reason`() = withServer {
        val chatId = newChat()
        converse(chatId) {
            say("please /fail")
            val finished = turn().finished()
            assertEquals(TurnEndReason.FAILED, finished.reason)
            assertTrue(finished.presence.hasError)
            assertContains(finished.error, "/fail")
        }
    }

    @Test
    fun `a turn over the rate limit is refused and not stored`() = withServer(
        fastServer(limiter = RateLimiter(burst = 1, perMinute = 1)),
    ) {
        val chatId = newChat()
        converse(chatId) {
            say("one")
            assertEquals(TurnEndReason.COMPLETED, turn().finished().reason)
            say("two")
            val refused = turn()
            assertEquals(TurnEndReason.FAILED, refused.finished().reason)
            assertContains(refused.finished().error, "Rate limit")
            assertEquals(0, refused.count { it is Event.Kind.TextDelta })
        }
        assertEquals(2, chats.listMessages(ListMessagesRequest { this.chatId = chatId }).messages.size)
    }

    @Test
    fun `a full queue refuses the turn instead of holding it`() = withServer(
        slowServer(queue = RequestQueue(concurrency = 1, capacity = 0)),
    ) {
        val busy = newChat(StubModel.SLOW)
        val other = newChat()
        converse(busy) {
            say("occupy the only slot")
            do {
                val event = next()
            } while (event !is Event.Kind.TextDelta)
            converse(other) {
                say("no room")
                val refused = turn().finished()
                assertEquals(TurnEndReason.FAILED, refused.reason)
                assertContains(refused.error, "busy")
            }
            send(ConverseRequest.Submission.Interrupt(Interrupt { }))
            turn()
        }
    }

    @Test
    fun `switching the model applies from the next turn`() = withServer {
        val chatId = newChat()
        converse(chatId) {
            send(ConverseRequest.Submission.SwitchModel(SwitchModel { model = StubModel.SLOW.toProto() }))
            val notice = assertIs<Event.Kind.Notice>(next()).value
            assertEquals(NoticeLevel.INFO, notice.level)
            say("hi")
            assertContains(turn().text(), "[stub/slow]")

            send(ConverseRequest.Submission.SwitchModel(SwitchModel { model = ModelId("stub", "missing").toProto() }))
            assertEquals(NoticeLevel.ERROR, assertIs<Event.Kind.Notice>(next()).value.level)
        }
        assertEquals(StubModel.SLOW, chats.getChat(GetChatRequest { this.chatId = chatId }).chat.model.toId())
    }

    @Test
    fun `a stream must attach to an existing chat first`() = withServer {
        val unattached = assertFailsWith<GrpcStatusException> {
            converse(chatId = null) {
                say("hello")
                next()
            }
        }
        assertEquals(GrpcStatusCode.FAILED_PRECONDITION, unattached.status.statusCode)

        val missing = assertFailsWith<GrpcStatusException> {
            converse("01ARZ3NDEKTSV4RRFFQ69G5FAV") { next() }
        }
        assertEquals(GrpcStatusCode.NOT_FOUND, missing.status.statusCode)
    }

    @Test
    fun `an oversized turn is rejected, not truncated`() = withServer {
        val chatId = newChat()
        val error = assertFailsWith<GrpcStatusException> {
            converse(chatId) {
                say("x".repeat(MAX_TURN_BYTES + 1))
                next()
            }
        }
        assertEquals(GrpcStatusCode.INVALID_ARGUMENT, error.status.statusCode)
        assertTrue(chats.listMessages(ListMessagesRequest { this.chatId = chatId }).messages.isEmpty())
    }

    @Test
    fun `the model list and chat pages come from the server`() = withServer {
        val models = config.listModels(ListModelsRequest { }).models
        assertEquals(listOf("Stub Echo", "Stub Slow", "Stub Flaky"), models.map { it.displayName })
        assertTrue(models.all { it.served })

        val unknown = assertFailsWith<GrpcStatusException> {
            chats.createChat(CreateChatRequest { model = ModelId("stub", "missing").toProto() })
        }
        assertEquals(GrpcStatusCode.INVALID_ARGUMENT, unknown.status.statusCode)

        repeat(3) { newChat() }
        val firstPage = chats.listChats(aulo.v1.ListChatsRequest { pageSize = 2u })
        assertEquals(2, firstPage.chats.size)
        val rest = chats.listChats(aulo.v1.ListChatsRequest { pageToken = firstPage.nextPageToken })
        assertEquals(1, rest.chats.size)
        assertEquals("", rest.nextPageToken)
        assertEquals(3, (firstPage.chats + rest.chats).map { it.id }.toSet().size)
    }
}
