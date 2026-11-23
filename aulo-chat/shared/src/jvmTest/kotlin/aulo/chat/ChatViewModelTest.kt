package aulo.chat

import aulo.chat.data.DefaultChatRepository
import aulo.chat.data.MAX_MESSAGE_CHARS
import aulo.chat.data.SampleAttachmentPicker
import aulo.chat.data.Submission
import aulo.chat.data.TurnEnd
import aulo.chat.data.TurnEvent
import aulo.chat.domain.AttachmentKind
import aulo.chat.domain.ConnectionState
import aulo.chat.domain.LoadModels
import aulo.chat.domain.MAX_TURN_BYTES
import aulo.chat.domain.MessageRole
import aulo.chat.domain.MessageState
import aulo.chat.domain.Notice
import aulo.chat.domain.NoticeLevel
import aulo.chat.domain.OpenChat
import aulo.chat.domain.SendMessage
import aulo.chat.domain.StopGeneration
import aulo.chat.domain.SwitchModel
import aulo.chat.presentation.ChatViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import kotlin.test.AfterTest
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** The view model over the real repository and cache, with only the server faked. */
@OptIn(ExperimentalCoroutinesApi::class)
class ChatViewModelTest {
    private val dispatcher = StandardTestDispatcher()
    private val remote = FakeRemote()

    @BeforeTest
    fun setUp() = Dispatchers.setMain(dispatcher)

    @AfterTest
    fun tearDown() = Dispatchers.resetMain()

    private fun TestScope.viewModel(): ChatViewModel {
        val repository = DefaultChatRepository(remote, memoryCache(dispatcher), nowMs = { testScheduler.currentTime })
        return ChatViewModel(
            repository,
            OpenChat(repository),
            LoadModels(repository),
            SendMessage(),
            StopGeneration(),
            SwitchModel(),
            SampleAttachmentPicker(),
        ).also { advanceUntilIdle() }
    }

    private fun test(block: suspend TestScope.() -> Unit) = runTest(dispatcher) { block() }

    @Test
    fun `opens the most recent chat with its history and models`() = test {
        remote.seedChat("old", "Old chat", "hi", "hello")
        remote.seedChat("lisbon", "Trip to Lisbon", "Plan a trip", "Day 1: Alfama")
        val state = viewModel().state.value

        assertEquals("Trip to Lisbon", state.chat?.title)
        assertEquals(listOf("Plan a trip", "Day 1: Alfama"), state.messages.map { it.text })
        assertEquals(ConnectionState.Online, state.connection)
        assertEquals(listOf("Stub Echo", "Stub Slow"), state.models.map { it.displayName })
        assertFalse(state.loading)
        assertFalse(state.canSend, "nothing typed yet")
    }

    @Test
    fun `a first run creates a chat and shows the empty state`() = test {
        val state = viewModel().state.value
        assertEquals("new-1", state.chat?.id)
        assertTrue(state.empty)
    }

    @Test
    fun `tokens appear as they arrive and the answer is cached when the turn ends`() = test {
        remote.seedChat("c", "")
        val vm = viewModel()
        vm.onInputChange("  Plan a trip  ")
        assertTrue(vm.state.value.canSend)
        vm.send()
        advanceUntilIdle()

        assertEquals(listOf<Submission>(Submission.UserTurn("Plan a trip")), remote.submissions)
        assertEquals("", vm.state.value.input)
        assertTrue(vm.state.value.generating)
        assertEquals(MessageState.Streaming, vm.state.value.messages.last().state, "a placeholder shows at once")

        remote.emit(TurnEvent.Started("t1"))
        remote.emit(TurnEvent.Delta("t1", "Day 1: "))
        advanceUntilIdle()
        assertEquals("Day 1: ", vm.state.value.messages.last().text)
        remote.emit(TurnEvent.Delta("t1", "Alfama"))
        remote.emit(TurnEvent.Finished("t1", TurnEnd.Completed, null))
        advanceUntilIdle()

        val state = vm.state.value
        assertFalse(state.generating)
        assertEquals(listOf(MessageRole.User, MessageRole.Assistant), state.messages.map { it.role })
        assertEquals("Day 1: Alfama", state.messages.last().text)
        assertEquals(MessageState.Complete, state.messages.last().state)
        assertEquals("Plan a trip", state.chat?.title.orEmpty().ifEmpty { "Plan a trip" })
    }

    @Test
    fun `stop interrupts the turn and keeps the partial answer`() = test {
        remote.seedChat("c", "Chat")
        val vm = viewModel()
        vm.onInputChange("Tell me everything")
        vm.send()
        advanceUntilIdle()
        remote.emit(TurnEvent.Started("t1"))
        remote.emit(TurnEvent.Delta("t1", "Well, "))
        advanceUntilIdle()

        vm.stop()
        advanceUntilIdle()
        assertEquals(Submission.Interrupt, remote.submissions.last())
        remote.emit(TurnEvent.Finished("t1", TurnEnd.Interrupted, null))
        advanceUntilIdle()

        val last = vm.state.value.messages.last()
        assertEquals("Well, ", last.text)
        assertEquals(MessageState.Interrupted, last.state)
        assertFalse(vm.state.value.generating)
    }

    @Test
    fun `a failed turn shows its reason and can be retried`() = test {
        remote.seedChat("c", "Chat")
        val vm = viewModel()
        vm.onInputChange("hello")
        vm.send()
        advanceUntilIdle()
        remote.emit(TurnEvent.Noticed(Notice(NoticeLevel.Warning, "Rate limit reached for this chat; try again in 12 s.")))
        remote.answer("t1", end = TurnEnd.Failed, error = "Rate limit reached for this chat; try again in 12 s.")
        advanceUntilIdle()

        val failed = vm.state.value.messages.last()
        assertEquals(MessageState.Failed, failed.state)
        assertEquals("Rate limit reached for this chat; try again in 12 s.", failed.error)
        assertEquals(NoticeLevel.Warning, vm.state.value.notice?.level)

        vm.retry(failed)
        advanceUntilIdle()
        assertEquals(2, remote.submissions.count { it == Submission.UserTurn("hello") })
        assertNull(vm.state.value.notice, "sending again clears the banner")
    }

    @Test
    fun `text attachments travel in the turn, images and voice stay behind`() = test {
        remote.seedChat("c", "Chat")
        val vm = viewModel()
        vm.attach(AttachmentKind.Text)
        vm.attach(AttachmentKind.Image)
        advanceUntilIdle()
        assertEquals(2, vm.state.value.attachments.size)
        vm.removeAttachment(vm.state.value.attachments[1])
        vm.attach(AttachmentKind.Voice)
        vm.onInputChange("Keep it within this budget.")
        advanceUntilIdle()
        vm.send()
        advanceUntilIdle()

        val sent = assertIs<Submission.UserTurn>(remote.submissions.single()).text
        assertTrue(sent.startsWith("Keep it within this budget.\n\n--- sample-1.txt ---\n"))
        assertTrue("600 EUR" in sent)
        assertFalse("voice" in sent)
        val shown = vm.state.value.messages.first()
        assertEquals(listOf(AttachmentKind.Text, AttachmentKind.Voice), shown.attachments.map { it.kind })
        assertTrue(shown.attachments.all { it.content == null }, "the cache keeps labels, not file contents")
        assertEquals(NoticeLevel.Info, vm.state.value.notice?.level)
        assertTrue(vm.state.value.attachments.isEmpty())
    }

    @Test
    fun `an oversized message is refused before it leaves the device`() = test {
        remote.seedChat("c", "Chat")
        val vm = viewModel()
        val huge = "x".repeat(MAX_TURN_BYTES + 1)
        vm.onInputChange(huge)
        vm.send()
        advanceUntilIdle()

        assertTrue(remote.submissions.isEmpty())
        assertEquals(NoticeLevel.Error, vm.state.value.notice?.level)
        assertEquals(huge.length, vm.state.value.input.length, "the text is given back, not lost")
        assertTrue(vm.state.value.messages.isEmpty())
    }

    @Test
    fun `an answer longer than the cap is cut`() = test {
        remote.seedChat("c", "Chat")
        val vm = viewModel()
        vm.onInputChange("go")
        vm.send()
        advanceUntilIdle()
        remote.answer("t1", "y".repeat(MAX_MESSAGE_CHARS), "overflow")
        advanceUntilIdle()
        assertEquals(MAX_MESSAGE_CHARS, vm.state.value.messages.last().text.length)
    }

    @Test
    fun `switching the model is sent to the server and remembered`() = test {
        remote.seedChat("c", "Chat")
        val vm = viewModel()
        vm.selectModel(SLOW)
        advanceUntilIdle()
        assertEquals(Submission.SwitchModel(SLOW), remote.submissions.single())
        assertEquals(SLOW, vm.state.value.chat?.model)
    }

    @Test
    fun `offline the cached chat stays readable and retry reconnects`() = test {
        remote.seedChat("lisbon", "Trip to Lisbon", "Plan a trip", "Day 1: Alfama")
        val vm = viewModel()
        vm.onInputChange("And day 2?")
        vm.send()
        advanceUntilIdle()
        remote.emit(TurnEvent.Started("t1"))
        remote.emit(TurnEvent.Delta("t1", "Day 2: "))
        advanceUntilIdle()

        remote.disconnect()
        advanceUntilIdle()
        var state = vm.state.value
        assertIs<ConnectionState.Offline>(state.connection)
        assertEquals(MessageState.Failed, state.messages.last().state, "the answer in flight is settled, not left spinning")
        assertEquals("Day 2: ", state.messages.last().text)
        assertFalse(state.generating)
        vm.onInputChange("still there?")
        assertFalse(vm.state.value.canSend)

        vm.connect()
        advanceUntilIdle()
        state = vm.state.value
        assertIs<ConnectionState.Offline>(state.connection)
        assertEquals("Trip to Lisbon", state.chat?.title)
        assertEquals(4, state.messages.size, "the cache still shows the conversation")

        remote.reachable = true
        vm.connect()
        advanceUntilIdle()
        state = vm.state.value
        assertEquals(ConnectionState.Online, state.connection)
        assertEquals(listOf("Plan a trip", "Day 1: Alfama"), state.messages.map { it.text }, "the server's copy wins")
    }

    @Test
    fun `a cold start without a server shows the last cached chat`() = test {
        remote.seedChat("lisbon", "Trip to Lisbon", "Plan a trip", "Day 1: Alfama")
        val cache = memoryCache(dispatcher)
        DefaultChatRepository(remote, cache).connect()
        remote.reachable = false

        val repository = DefaultChatRepository(remote, cache)
        val vm = ChatViewModel(
            repository, OpenChat(repository), LoadModels(repository), SendMessage(), StopGeneration(), SwitchModel(),
            SampleAttachmentPicker(),
        )
        advanceUntilIdle()
        val state = vm.state.value
        assertIs<ConnectionState.Offline>(state.connection)
        assertEquals(listOf("Plan a trip", "Day 1: Alfama"), state.messages.map { it.text })
        assertTrue(state.models.isEmpty())
    }
}
