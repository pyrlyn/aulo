package aulo.chat.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.InternalComposeUiApi
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.pressKey
import androidx.compose.ui.test.v2.runComposeUiTest
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.use
import aulo.chat.domain.Appearance
import aulo.chat.domain.ChatMessage
import aulo.chat.domain.ChatSummary
import aulo.chat.domain.ConnectionState
import aulo.chat.domain.MessageHit
import aulo.chat.domain.MessageRole
import aulo.chat.domain.MessageState
import aulo.chat.domain.ModelId
import aulo.chat.domain.ModelInfo
import aulo.chat.presentation.ChatUiState
import aulo.chat.presentation.SearchState
import aulo.chat.presentation.ServerStatus
import aulo.chat.presentation.SettingsUiState
import aulo.chat.ui.theme.AuloTheme
import aulo.chat.ui.theme.UiPlatform
import aulo.chat.ui.theme.tokensFor
import org.jetbrains.skia.EncodedImageFormat
import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

private val ECHO = ModelId("stub", "echo")
private const val NOW = 10 * 24 * 60 * 60_000L
private const val MINUTE = 60_000L

private val CHATS = listOf(
    ChatSummary("lisbon", "Trip to Lisbon", ECHO, NOW),
    ChatSummary("notes", "Release notes draft", ECHO, NOW - 12 * MINUTE),
    ChatSummary("kotlin", "Kotlin flow question", ECHO, NOW - 60 * MINUTE),
    ChatSummary("weekly", "Weekly review", ECHO, NOW - 26 * 60 * MINUTE),
    ChatSummary("recipes", "Recipe ideas", ECHO, NOW - 5 * 24 * 60 * MINUTE),
)

private val MESSAGES = listOf(
    ChatMessage("m1", "lisbon", MessageRole.User, "Plan a three-day trip to Lisbon in May.", MessageState.Complete),
    ChatMessage("m2", "lisbon", MessageRole.Assistant, "Day 1: Alfama and the castle.", MessageState.Complete),
)

private fun window(
    connection: ConnectionState = ConnectionState.Online,
    search: SearchState = SearchState(),
) = ChatUiState(
    chat = CHATS[0],
    chats = CHATS,
    messages = MESSAGES,
    models = listOf(ModelInfo(ECHO, "Stub Echo", served = true)),
    connection = connection,
    loading = false,
    search = search,
)

private val SEARCH = window(
    search = SearchState(
        query = "lisbon",
        hits = listOf(MessageHit("lisbon", "m9", "…stay near Lisbon's Alfama for the first night", NOW - 3 * MINUTE)),
    ),
)

private val SETTINGS = SettingsUiState(
    address = "127.0.0.1:50051",
    connectedAddress = "127.0.0.1:50051",
    status = ServerStatus.Connected,
    appearance = Appearance.Dark,
    defaultModel = ECHO,
    models = listOf(ModelInfo(ECHO, "Stub Echo", served = true)),
)

@OptIn(ExperimentalTestApi::class)
class ChatWindowTest {
    @Composable
    private fun Window(
        state: ChatUiState,
        list: ChatListActions = ChatListActions(),
        renaming: String? = null,
        deleting: ChatSummary? = null,
        onConfirmDelete: (ChatSummary) -> Unit = {},
        onCancelDelete: () -> Unit = {},
    ) = ChatWindow(state, ChatActions(), list, renaming, deleting, onConfirmDelete, onCancelDelete, NOW)

    @Test
    fun `the sidebar lists the chats with their age and opens the one clicked`() = runComposeUiTest {
        var opened: ChatSummary? = null
        setContent { AuloTheme(UiPlatform.MacOs) { Window(window(), ChatListActions(onOpen = { opened = it })) } }

        onNodeWithText("12 min").assertExists()
        onNodeWithText("1 h").assertExists()
        onNodeWithText("Yesterday").assertExists()
        onNodeWithText("5 d").assertExists()
        onNodeWithText("Kotlin flow question").performClick()
        assertEquals("kotlin", opened?.id)
        onNodeWithText("The 20 most recent chats are kept on this device.").assertExists()
    }

    @Test
    fun `the header menu renames and asks before deleting`() = runComposeUiTest {
        var renaming: ChatSummary? = null
        var asked: ChatSummary? = null
        val list = ChatListActions(onStartRename = { renaming = it }, onAskDelete = { asked = it })
        setContent { AuloTheme(UiPlatform.Windows) { Window(window(), list) } }

        onNodeWithContentDescription("Chat actions").performClick()
        onNodeWithText("Rename").performClick()
        assertEquals("lisbon", renaming?.id)
        onNodeWithContentDescription("Chat actions").performClick()
        onNodeWithText("Delete").performClick()
        assertEquals("lisbon", asked?.id)
    }

    @Test
    fun `the status and the chat menu sit at the right edge of the header`() = runComposeUiTest {
        setContent { AuloTheme(UiPlatform.Linux) { Window(window()) } }

        val window = onRoot().getUnclippedBoundsInRoot()
        val menu = onNodeWithContentDescription("Chat actions").getUnclippedBoundsInRoot()
        assertTrue(window.right - menu.right < 16.dp, "menu ends ${window.right - menu.right} before the edge")
    }

    @Test
    fun `offline the menu says rename and delete need the server`() = runComposeUiTest {
        setContent { AuloTheme(UiPlatform.Linux) { Window(window(ConnectionState.Offline("down"))) } }

        onNodeWithContentDescription("Chat actions").performClick()
        onNodeWithText("Rename").assertIsNotEnabled()
        onNodeWithText("Delete").assertIsNotEnabled()
    }

    @Test
    fun `rename saves on Enter, refuses a blank title and cancels on Escape`() = runComposeUiTest {
        val saved = mutableListOf<String>()
        var cancelled = 0
        val list = ChatListActions(onRename = { _, title -> saved += title }, onCancelRename = { cancelled++ })
        setContent { AuloTheme(UiPlatform.MacOs) { Window(window(), list, renaming = "lisbon") } }

        val field = onNodeWithContentDescription("Chat title")
        field.assertIsFocused()
        field.performTextReplacement("   ")
        field.performKeyInput { pressKey(Key.Enter) }
        assertTrue(saved.isEmpty())
        field.performTextReplacement("Lisbon in May")
        field.performKeyInput { pressKey(Key.Enter) }
        assertEquals(listOf("Lisbon in May"), saved)
        field.performKeyInput { pressKey(Key.Escape) }
        assertTrue(cancelled >= 1)
    }

    @Test
    fun `deleting waits for the dialog's own button`() = runComposeUiTest {
        var deleted: ChatSummary? = null
        var cancelled = 0
        setContent {
            AuloTheme(UiPlatform.Windows) {
                Window(window(), deleting = CHATS[1], onConfirmDelete = { deleted = it }, onCancelDelete = { cancelled++ })
            }
        }

        onNodeWithText("Delete this chat?").assertExists()
        // Cancel holds the focus, so a stray Enter cancels instead of deleting.
        onNodeWithText("Cancel").assertIsFocused()
        onNodeWithText("Cancel").performKeyInput { pressKey(Key.Enter) }
        assertNull(deleted)
        assertEquals(1, cancelled)
        onNodeWithText("Delete").performClick()
        assertEquals("notes", deleted?.id)
    }

    @Test
    fun `search shows title matches and message hits, the match in bold`() = runComposeUiTest {
        var hit: MessageHit? = null
        setContent { AuloTheme(UiPlatform.MacOs) { Window(SEARCH, ChatListActions(onOpenHit = { hit = it })) } }

        onNodeWithText("Chats").assertExists()
        onNodeWithText("Messages").assertExists()
        onNodeWithText("Release notes draft").assertDoesNotExist()
        onNodeWithText("…stay near Lisbon's Alfama for the first night").performClick()
        assertEquals("m9", hit?.messageId)
    }

    @Test
    fun `search with nothing found says so, and offline keeps the title matches`() = runComposeUiTest {
        var current by mutableStateOf(window(search = SearchState(query = "zzz")))
        setContent { AuloTheme(UiPlatform.Linux) { Window(current) } }
        onNodeWithText("No chats found").assertExists()

        current = window(ConnectionState.Offline("down"), SearchState(query = "trip", error = "Message search needs the server."))
        // The header and the sidebar's title match.
        onAllNodesWithText("Trip to Lisbon").assertCountEquals(2)
        onNodeWithText("Message search needs the server.").assertExists()
    }

    @Test
    fun `highlight bolds every query word wherever it appears`() {
        val text = highlight("Lisbon trip: LISBON again", "lisbon")
        val bold = text.spanStyles.filter { it.item.fontWeight == FontWeight.Bold }.map { it.start to it.end }
        assertEquals(listOf(0 to 6, 13 to 19), bold)
        assertTrue(highlight("plain", " ").spanStyles.isEmpty())
    }

    @Test
    fun `window shortcuts follow the platform's command key`() {
        val state = window()
        var found = 0
        var renamed: ChatSummary? = null
        fun press(key: Key, meta: Boolean = false, ctrl: Boolean = false, platform: UiPlatform) = windowShortcut(
            keyDown(key, meta, ctrl),
            platform,
            state,
            onFind = { found++ },
            onNewChat = {},
            onRename = { renamed = it },
        )
        assertTrue(press(Key.F, meta = true, platform = UiPlatform.MacOs))
        assertFalse(press(Key.F, ctrl = true, platform = UiPlatform.MacOs))
        assertTrue(press(Key.K, ctrl = true, platform = UiPlatform.Windows))
        assertEquals(2, found)
        assertTrue(press(Key.F2, platform = UiPlatform.Linux))
        assertEquals("lisbon", renamed?.id)
        assertFalse(press(Key.Delete, platform = UiPlatform.Linux))
    }

    @Test
    fun `settings connect only to a valid new address and confirm clearing the cache`() = runComposeUiTest {
        var current by mutableStateOf(SETTINGS)
        var connects = 0
        var cleared = 0
        val actions = SettingsActions(onConnect = { connects++ }, onClearCache = { cleared++ })
        setContent { AuloTheme(UiPlatform.MacOs) { SettingsScreen(current, "1.0.0", actions) } }

        onNodeWithText("Connected").assertExists()
        onNodeWithText("Connect").assertIsNotEnabled()
        current = SETTINGS.copy(address = "localhost:99999")
        onNodeWithText(ADDRESS_HINT).assertExists()
        onNodeWithText("Connect").assertIsNotEnabled()
        current = SETTINGS.copy(address = "192.0.2.5:6000")
        onNodeWithText("Connect").assertIsEnabled().performClick()
        assertEquals(1, connects)

        onNodeWithText("Clear local cache").performClick()
        assertEquals(0, cleared)
        onNodeWithText("Clear").performClick()
        assertEquals(1, cleared)
        onNodeWithText("aulo chat 1.0.0").assertExists()
    }

    @Test
    fun `the connect screen shows why the server did not answer`() = runComposeUiTest {
        val state = SETTINGS.copy(status = ServerStatus.Failed("Cannot reach 127.0.0.1:50051: connection refused"))
        setContent { AuloTheme(UiPlatform.Windows) { ConnectScreen(state, SettingsActions()) } }

        onNodeWithText("Connect to aulo").assertExists()
        onNodeWithText("Cannot reach 127.0.0.1:50051: connection refused").assertExists()
        onNodeWithText("./gradlew :server:run").assertExists()
    }

    @Test
    fun `every platform has a light mode of its own`() {
        for (platform in UiPlatform.entries) {
            val dark = tokensFor(platform, dark = true).colors
            val light = tokensFor(platform, dark = false).colors
            assertNotEquals(dark.background, light.background, "$platform")
            assertNotEquals(dark.text, light.text, "$platform")
        }
    }

    /** Draws the desktop window in both modes, so a change to the look can be seen in a diff of PNGs. */
    @Test
    fun `renders the desktop window`() {
        val out = File(System.getProperty("aulo.chat.renders") ?: return).apply { mkdirs() }
        val desktops = listOf(UiPlatform.MacOs, UiPlatform.Windows, UiPlatform.Linux)
        for (platform in desktops) {
            for (dark in listOf(true, false)) {
                val mode = if (dark) "dark" else "light"
                val name = platform.name.lowercase()
                render(File(out, "window-$name-$mode.png"), platform, dark) { Window(window()) }
                render(File(out, "search-$name-$mode.png"), platform, dark) { Window(SEARCH) }
                render(File(out, "settings-$name-$mode.png"), platform, dark, 560 to 520) {
                    SettingsScreen(SETTINGS, "1.0.0", SettingsActions())
                }
                render(File(out, "connect-$name-$mode.png"), platform, dark) {
                    ConnectScreen(SETTINGS.copy(status = ServerStatus.Connecting), SettingsActions())
                }
            }
        }
    }

    private fun render(
        file: File,
        platform: UiPlatform,
        dark: Boolean,
        size: Pair<Int, Int> = 1120 to 720,
        content: @Composable () -> Unit,
    ) {
        val (width, height) = size
        val png = ImageComposeScene(width * 2, height * 2, Density(2f)) {
            AuloTheme(platform, dark) { content() }
        }.use { scene ->
            repeat(4) { scene.render(it * 16_000_000L) }
            scene.render(400_000_000L).encodeToData(EncodedImageFormat.PNG)!!.bytes
        }
        file.writeBytes(png)
        assertTrue(png.size > 10_000, "${file.name} rendered next to nothing")
    }
}

// The constructor tests use; a running window builds these from AWT events.
@OptIn(InternalComposeUiApi::class)
private fun keyDown(key: Key, meta: Boolean, ctrl: Boolean) =
    KeyEvent(key, KeyEventType.KeyDown, isCtrlPressed = ctrl, isMetaPressed = meta)
