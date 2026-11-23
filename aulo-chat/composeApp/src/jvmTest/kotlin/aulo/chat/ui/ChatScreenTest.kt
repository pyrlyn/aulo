package aulo.chat.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.v2.runComposeUiTest
import androidx.compose.ui.unit.Density
import androidx.compose.ui.use
import aulo.chat.data.ServerEndpoint
import aulo.chat.domain.Attachment
import aulo.chat.domain.AttachmentKind
import aulo.chat.domain.ChatMessage
import aulo.chat.domain.ChatSummary
import aulo.chat.domain.ConnectionState
import aulo.chat.domain.MessageRole
import aulo.chat.domain.MessageState
import aulo.chat.domain.ModelId
import aulo.chat.domain.ModelInfo
import aulo.chat.domain.Notice
import aulo.chat.domain.NoticeLevel
import aulo.chat.presentation.ChatUiState
import aulo.chat.ui.theme.AuloTheme
import aulo.chat.ui.theme.UiPlatform
import org.jetbrains.skia.EncodedImageFormat
import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

private val ECHO = ModelId("stub", "echo")
private val SLOW = ModelId("stub", "slow")
private val LLAMA = ModelId("local", "llama-3")

private val MODELS = listOf(
    ModelInfo(ECHO, "Stub Echo", served = true),
    ModelInfo(SLOW, "Stub Slow", served = true),
    ModelInfo(LLAMA, "Local Llama", served = false),
)

private fun message(id: String, role: MessageRole, text: String, state: MessageState = MessageState.Complete) =
    ChatMessage(id, "c", role, text, state)

// The conversation the mockups show.
private val M1 = message("m1", MessageRole.User, "Plan a three-day trip to Lisbon in May.")
private val M2 = message(
    "m2",
    MessageRole.Assistant,
    "Day 1: Alfama and the castle, then sunset at Miradouro da Graça. Day 2: Belém — the tower, the monastery " +
        "and pastéis de nata. Day 3: a day trip to Sintra.",
)
private val M3 = message("m3", MessageRole.User, "Keep it within this budget.")
    .copy(attachments = listOf(Attachment(AttachmentKind.Text, "budget.txt", "2 KB")))
private const val M4_TEXT = "With that budget, stay in Graça or Alfama and use the 24-hour tram and metro pass"

private fun state(
    messages: List<ChatMessage> = listOf(M1, M2),
    connection: ConnectionState = ConnectionState.Online,
    generating: Boolean = false,
    input: String = "",
    notice: Notice? = null,
    title: String = "Trip to Lisbon",
) = ChatUiState(
    chat = ChatSummary("c", title, ECHO, 0),
    messages = messages,
    input = input,
    models = if (connection is ConnectionState.Online) MODELS else emptyList(),
    connection = connection,
    generating = generating,
    notice = notice,
    loading = false,
)

private val STREAMING = state(
    messages = listOf(M1, M2, M3, message("m4", MessageRole.Assistant, M4_TEXT, MessageState.Streaming)),
    generating = true,
)

private val FAILED = message("m2f", MessageRole.Assistant, "", MessageState.Failed)
    .copy(error = "The model is unavailable after 3 attempts: stub provider overloaded")

private val ERROR = state(
    messages = listOf(M1, FAILED, M3, message("m4", MessageRole.Assistant, M4_TEXT, MessageState.Interrupted)),
    notice = Notice(NoticeLevel.Error, "Rate limit reached for this chat; try again in 12 s."),
)

private val OFFLINE = state(connection = ConnectionState.Offline("The server cannot be reached."))

private val EMPTY = state(messages = emptyList(), title = "")

@OptIn(ExperimentalTestApi::class)
class ChatScreenTest {
    @Test
    fun `while an answer streams the composer offers Stop, not Send`() = runComposeUiTest {
        var stopped = 0
        setContent { AuloTheme(UiPlatform.MacOs) { ChatScreen(STREAMING, ChatActions(onStop = { stopped++ })) } }

        onNodeWithText(M4_TEXT).assertExists()
        onNodeWithContentDescription("Send").assertDoesNotExist()
        onNodeWithContentDescription("Stop generating").performClick()
        assertEquals(1, stopped)
    }

    @Test
    fun `send is offered only when there is something to send`() = runComposeUiTest {
        var sent = 0
        var current by mutableStateOf(state())
        setContent { AuloTheme(UiPlatform.Windows) { ChatScreen(current, ChatActions(onSend = { sent++ })) } }
        onNodeWithContentDescription("Send").assertIsNotEnabled()
        onNodeWithText("Message aulo").assertExists()

        current = state(input = "And day 4?")
        onNodeWithContentDescription("Send").assertIsEnabled().performClick()
        assertEquals(1, sent)
    }

    @Test
    fun `offline shows the banner, retries on demand and locks the composer`() = runComposeUiTest {
        var reconnects = 0
        setContent { AuloTheme(UiPlatform.Android) { ChatScreen(OFFLINE, ChatActions(onReconnect = { reconnects++ })) } }

        onNodeWithText("Offline — showing chats saved on this device.").assertExists()
        onNodeWithText(M2.text).assertExists()
        onNodeWithText("Reconnect to send a message").assertExists()
        onNodeWithContentDescription("Send").assertIsNotEnabled()
        onNodeWithContentDescription("Add to message").assertIsNotEnabled()
        onNodeWithText("Retry").performClick()
        assertEquals(1, reconnects)
    }

    @Test
    fun `a failed answer shows its reason with Retry, a stopped one says so`() = runComposeUiTest {
        var retried: ChatMessage? = null
        var dismissed = 0
        val actions = ChatActions(onRetry = { retried = it }, onDismissNotice = { dismissed++ })
        setContent { AuloTheme(UiPlatform.Linux) { ChatScreen(ERROR, actions) } }

        onNodeWithText("The model is unavailable after 3 attempts: stub provider overloaded").assertExists()
        onNodeWithText("Stopped").assertExists()
        onNodeWithText("Retry").performClick()
        assertEquals("m2f", retried?.id)
        onNodeWithText("Rate limit reached for this chat; try again in 12 s.").assertExists()
        onNodeWithText("Dismiss").performClick()
        assertEquals(1, dismissed)
    }

    @Test
    fun `model output is shown as written, never as markup`() = runComposeUiTest {
        val hostile = "[click](https://example.test) <b>bold</b> **stars** ‮evil"
        val messages = listOf(M1, message("x", MessageRole.Assistant, hostile))
        setContent { AuloTheme(UiPlatform.Ios) { ChatScreen(state(messages = messages), ChatActions()) } }
        onNodeWithText(hostile).assertExists()
    }

    @Test
    fun `the model picker lists the models and selects a served one`() = runComposeUiTest {
        var selected: ModelId? = null
        setContent { AuloTheme(UiPlatform.MacOs) { ChatScreen(state(), ChatActions(onSelectModel = { selected = it })) } }

        onNodeWithText("Stub Echo").performClick()
        onNodeWithText("Local Llama").assertExists()
        onNodeWithText("Unavailable").assertExists()
        onNodeWithText("Stub Slow").performClick()
        assertEquals(SLOW, selected)
    }

    @Test
    fun `a new chat offers suggestions that fill the composer`() = runComposeUiTest {
        var input: String? = null
        setContent { AuloTheme(UiPlatform.Android) { ChatScreen(EMPTY, ChatActions(onInputChange = { input = it })) } }

        onNodeWithText("New chat").assertExists()
        onNodeWithText("What can I help with?").assertExists()
        onNodeWithText("Draft a reply").performClick()
        assertEquals("Draft a reply", input)
    }

    @Test
    fun `the attach menu offers text, image and voice`() = runComposeUiTest {
        var kind: AttachmentKind? = null
        setContent { AuloTheme(UiPlatform.Windows) { ChatScreen(state(), ChatActions(onAttach = { kind = it })) } }

        onNodeWithContentDescription("Add to message").performClick()
        onNodeWithText("Text file").assertExists()
        onNodeWithText("Image").assertExists()
        onNodeWithText("Voice note").performClick()
        assertEquals(AttachmentKind.Voice, kind)
    }

    @Test
    fun `the server address comes from the environment and a bad port is refused`() {
        assertEquals(ServerEndpoint("127.0.0.1", 50051), endpointFromEnv(emptyMap()))
        assertEquals(
            ServerEndpoint("10.0.0.5", 6000),
            endpointFromEnv(mapOf("AULO_CHAT_HOST" to "10.0.0.5", "AULO_CHAT_PORT" to "6000")),
        )
        assertNull(endpointFromEnv(mapOf("AULO_CHAT_PORT" to "70000")))
        assertNull(endpointFromEnv(mapOf("AULO_CHAT_PORT" to "grpc")))
    }

    /** Draws the screen in every platform theme, so a change to the look can be seen in a diff of PNGs. */
    @Test
    fun `renders every platform theme`() {
        val out = File(System.getProperty("aulo.chat.renders") ?: return).apply { mkdirs() }
        val desktop = 1120 to 720
        val shots = listOf(
            Shot("chat-macos", UiPlatform.MacOs, desktop, STREAMING),
            Shot("chat-windows", UiPlatform.Windows, desktop, STREAMING),
            Shot("chat-linux", UiPlatform.Linux, desktop, STREAMING),
            Shot("chat-android", UiPlatform.Android, 412 to 915, STREAMING),
            Shot("chat-ios", UiPlatform.Ios, 393 to 852, STREAMING),
            Shot("error-macos", UiPlatform.MacOs, desktop, ERROR),
            Shot("offline-android", UiPlatform.Android, 412 to 915, OFFLINE),
            Shot("empty-ios", UiPlatform.Ios, 393 to 852, EMPTY),
        )
        for (shot in shots) {
            val (width, height) = shot.size
            val png = ImageComposeScene(width * 2, height * 2, Density(2f)) {
                AuloTheme(shot.platform) { ChatScreen(shot.state, ChatActions()) }
            }.use { scene ->
                // The first frames lay the list out and scroll it to the newest message.
                repeat(4) { scene.render(it * 16_000_000L) }
                scene.render(400_000_000L).encodeToData(EncodedImageFormat.PNG)!!.bytes
            }
            File(out, "${shot.name}.png").writeBytes(png)
            assertTrue(png.size > 10_000, "${shot.name} rendered next to nothing")
        }
    }

    private class Shot(val name: String, val platform: UiPlatform, val size: Pair<Int, Int>, val state: ChatUiState)
}
