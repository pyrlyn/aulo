package aulo.chat

import androidx.lifecycle.viewModelScope
import aulo.chat.data.DefaultChatRepository
import aulo.chat.data.GrpcChatRemote
import aulo.chat.data.SampleAttachmentPicker
import aulo.chat.data.ServerEndpoint
import aulo.chat.domain.ConnectionState
import aulo.chat.domain.LoadModels
import aulo.chat.domain.MessageRole
import aulo.chat.domain.MessageState
import aulo.chat.domain.OpenChat
import aulo.chat.domain.SendMessage
import aulo.chat.domain.StopGeneration
import aulo.chat.domain.SwitchModel
import aulo.chat.presentation.ChatUiState
import aulo.chat.presentation.ChatViewModel
import aulo.chat.server.DevServer
import aulo.chat.server.StubModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.job
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.rpc.grpc.server.GrpcServer
import kotlin.test.AfterTest
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue
import kotlin.test.fail
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

/**
 * The client against the development server over a real gRPC connection: nothing is faked, so
 * this is the proof that the two halves speak the same contract.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class EndToEndTest {
    // One thread, like a UI thread: the view model is not written for concurrent callers.
    private val main = Dispatchers.Default.limitedParallelism(1)
    private val dev = DevServer(model = StubModel(tokenDelay = 10.milliseconds))
    private lateinit var server: GrpcServer
    private val remotes = mutableListOf<GrpcChatRemote>()
    private val viewModels = mutableListOf<ChatViewModel>()

    @BeforeTest
    fun setUp() {
        Dispatchers.setMain(main)
        server = dev.grpc(0).start()
    }

    @AfterTest
    fun tearDown() {
        // The view models go first: Main must be idle before it is reset.
        runBlocking { viewModels.forEach { it.viewModelScope.coroutineContext.job.cancelAndJoin() } }
        remotes.forEach { it.shutdown() }
        server.shutdownNow()
        Dispatchers.resetMain()
    }

    private fun viewModel(): ChatViewModel {
        val remote = GrpcChatRemote(ServerEndpoint("localhost", server.port)).also { remotes += it }
        val repository = DefaultChatRepository(remote, memoryCache(main))
        return ChatViewModel(
            repository,
            OpenChat(repository),
            LoadModels(repository),
            SendMessage(),
            StopGeneration(),
            SwitchModel(),
            SampleAttachmentPicker(),
        ).also { viewModels += it }
    }

    private suspend fun ChatViewModel.await(what: String, reached: (ChatUiState) -> Boolean): ChatUiState =
        withTimeoutOrNull(20.seconds) { state.first(reached) }
            ?: fail("timed out waiting for: $what; the state was ${state.value}")

    private suspend fun ChatViewModel.ui(action: ChatViewModel.() -> Unit) = withContext(main) { action() }

    private suspend fun ChatViewModel.say(text: String) = ui {
        onInputChange(text)
        send()
    }

    private suspend fun ChatViewModel.ready() =
        await("the first connection") { it.online && it.chat != null && it.models.isNotEmpty() && !it.loading }

    private val ChatUiState.answer get() = messages.last { it.role == MessageRole.Assistant }

    /** The chat holds [count] messages and its last one is an answer in [state]. */
    private fun ChatUiState.answered(count: Int, state: MessageState) = messages.size == count &&
        messages.last().let { it.role == MessageRole.Assistant && it.state == state } &&
        generating == (state == MessageState.Streaming)

    private val ChatUiState.streamingText get() = messages.lastOrNull()
        ?.takeIf { it.role == MessageRole.Assistant && it.state == MessageState.Streaming }?.text.orEmpty()

    @Test
    fun `an answer streams in and the next one remembers the dialog`() = runBlocking {
        val vm = viewModel()
        val start = vm.ready()
        assertEquals(listOf("Stub Echo", "Stub Slow", "Stub Flaky"), start.models.map { it.displayName })
        assertTrue(start.empty)

        vm.say("Plan a trip to Lisbon")
        val partial = vm.await("the first tokens") { it.generating && it.streamingText.isNotEmpty() }
        val done = vm.await("the end of the turn") { it.answered(2, MessageState.Complete) }
        assertContains(done.answer.text, "[stub/echo] You wrote: \"Plan a trip to Lisbon\"")
        assertTrue(partial.answer.text.length < done.answer.text.length, "the answer arrived in pieces")
        assertEquals("Plan a trip to Lisbon", vm.await("the chat's title") { it.chat?.title?.isNotEmpty() == true }.chat?.title)

        vm.say("And the budget?")
        val second = vm.await("the second answer") { it.answered(4, MessageState.Complete) }
        assertContains(second.answer.text, "before it you wrote: \"Plan a trip to Lisbon\"")
    }

    @Test
    fun `stop interrupts the answer and a switched model answers the next turn`() = runBlocking {
        val vm = viewModel()
        vm.ready()
        vm.ui { selectModel(SLOW) }
        vm.await("the model switch") { it.chat?.model == SLOW }

        vm.say("Write something long")
        vm.await("the first tokens") { it.generating && it.streamingText.isNotEmpty() }
        vm.ui { stop() }
        val stopped = vm.await("the stop") { it.answered(2, MessageState.Interrupted) }

        assertTrue(stopped.answer.text.startsWith("[stub/slow]"), "the slow model answered: ${stopped.answer.text}")
        assertFalse("one token at a time" in stopped.answer.text, "the tail of the answer never arrived")
    }

    @Test
    fun `a refused turn is shown as failed and retry sends the question again`() = runBlocking {
        val vm = viewModel()
        vm.ready()
        vm.say("please /fail")
        val failed = vm.await("the failure") { it.answered(2, MessageState.Failed) }
        assertContains(failed.answer.error.orEmpty(), "/fail")

        vm.ui { retry(failed.answer) }
        val again = vm.await("the retried turn") { it.answered(4, MessageState.Failed) }
        assertEquals("please /fail", again.messages[2].text)
    }

    @Test
    fun `losing the server goes offline with the chat still shown and reconnect brings it back`() = runBlocking {
        val vm = viewModel()
        vm.ready()
        vm.say("Remember this")
        vm.await("the answer") { it.answered(2, MessageState.Complete) }

        val port = server.port
        server.shutdownNow()
        server.awaitTermination()
        val offline = vm.await("the offline state") { it.connection is ConnectionState.Offline }
        assertEquals(2, offline.messages.size, "the cached chat stays on screen")
        vm.ui { onInputChange("typed while offline") }
        assertFalse(vm.state.value.canSend)

        // The same store behind a new listener: what a restarted connection to the same server sees.
        server = dev.grpc(port).start()
        vm.ui { connect() }
        val back = vm.await("the reconnect") { it.online }
        assertIs<ConnectionState.Online>(back.connection)
        assertEquals("Remember this", vm.await("the history") { it.messages.size == 2 }.messages.first().text)
        assertTrue(vm.await("a sendable composer") { it.canSend }.canSend)
    }
}
