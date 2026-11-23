package aulo.chat.ui

import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import aulo.chat.domain.ChatSummary
import aulo.chat.presentation.ChatViewModel
import aulo.chat.presentation.SettingsViewModel
import aulo.chat.ui.theme.AuloTheme
import aulo.chat.ui.theme.LocalUiPlatform
import aulo.chat.ui.theme.isDark
import kotlinx.coroutines.delay
import org.koin.compose.viewmodel.koinViewModel
import kotlin.time.Clock

/** Below this width the chat takes the whole screen and the conversation list is hidden. */
internal val WideLayout = 720.dp

/**
 * Key events of the whole window. A desktop window hands each one to [dispatch] before its
 * content sees it, so the shortcuts work wherever the focus is.
 */
class KeyRouter {
    internal var handle: (KeyEvent) -> Boolean = { false }

    fun dispatch(event: KeyEvent): Boolean = handle(event)
}

/**
 * The whole app. Koin must be started before this is composed. [onOpenSettings] opens Settings
 * in a window of its own (desktop); without it Settings opens as a dialog over the app.
 */
@Composable
fun App(
    version: String,
    keys: KeyRouter? = null,
    onOpenSettings: (() -> Unit)? = null,
    chatVm: ChatViewModel = koinViewModel(),
    settingsVm: SettingsViewModel = koinViewModel(),
) {
    val settings by settingsVm.state.collectAsStateWithLifecycle()
    AuloTheme(dark = settings.appearance.isDark()) {
        val state by chatVm.state.collectAsStateWithLifecycle()
        var settingsDialog by remember { mutableStateOf(false) }
        val openSettings = onOpenSettings ?: { settingsDialog = true }
        if (state.needsServer) {
            ConnectScreen(settings, settingsActions(settingsVm, chatVm))
        } else {
            BoxWithConstraints {
                if (maxWidth >= WideLayout) {
                    WideChat(chatVm, state = state, keys = keys, onOpenSettings = openSettings)
                } else {
                    ChatScreen(state, chatActions(chatVm))
                }
            }
        }
        if (settingsDialog) {
            Dialog(onDismissRequest = { settingsDialog = false }) {
                SettingsScreen(settings, version, settingsActions(settingsVm, chatVm), Modifier.size(560.dp, 520.dp))
            }
        }
    }
}

/** Settings wired to both view models: a new server or a cleared cache reopens the chats. */
fun settingsActions(settingsVm: SettingsViewModel, chatVm: ChatViewModel) = SettingsActions(
    onAddressChange = settingsVm::onAddressChange,
    onConnect = { settingsVm.connect(onConnected = chatVm::connect) },
    onAppearance = settingsVm::setAppearance,
    onDefaultModel = settingsVm::setDefaultModel,
    onClearCache = { settingsVm.clearCache(onCleared = chatVm::connect) },
)

internal fun chatActions(vm: ChatViewModel) = ChatActions(
    onInputChange = vm::onInputChange,
    onSend = vm::send,
    onStop = vm::stop,
    onRetry = vm::retry,
    onSelectModel = vm::selectModel,
    onAttach = vm::attach,
    onRemoveAttachment = vm::removeAttachment,
    onReconnect = vm::connect,
    onDismissNotice = vm::dismissNotice,
    onNewChat = vm::newChat,
)

@Composable
private fun WideChat(
    vm: ChatViewModel,
    state: aulo.chat.presentation.ChatUiState,
    keys: KeyRouter?,
    onOpenSettings: () -> Unit,
) {
    var renaming by remember { mutableStateOf<String?>(null) }
    var deleting by remember { mutableStateOf<ChatSummary?>(null) }
    val searchFocus = remember { FocusRequester() }
    val platform = LocalUiPlatform.current
    var nowMs by remember { mutableLongStateOf(Clock.System.now().toEpochMilliseconds()) }
    LaunchedEffect(Unit) {
        // The rows say "12 min"; a minute is as fine as they get.
        while (true) {
            delay(60_000)
            nowMs = Clock.System.now().toEpochMilliseconds()
        }
    }
    val list = remember(vm) {
        ChatListActions(
            onOpen = vm::openChat,
            onNewChat = vm::newChat,
            onStartRename = { renaming = it.id },
            onRename = { chat, title ->
                renaming = null
                vm.rename(chat, title)
            },
            onCancelRename = { renaming = null },
            onAskDelete = { deleting = it },
            onQueryChange = vm::onQueryChange,
            onOpenHit = vm::openHit,
            onOpenSettings = onOpenSettings,
        )
    }
    if (keys != null) {
        DisposableEffect(keys, state, platform) {
            keys.handle = { event ->
                windowShortcut(
                    event,
                    platform,
                    state,
                    onFind = { searchFocus.requestFocus() },
                    onNewChat = vm::newChat,
                    onRename = { renaming = it.id },
                )
            }
            onDispose { keys.handle = { false } }
        }
    }
    ChatWindow(
        state = state,
        actions = remember(vm) { chatActions(vm) },
        list = list,
        renaming = renaming,
        deleting = deleting,
        onConfirmDelete = vm::delete,
        onCancelDelete = { deleting = null },
        nowMs = nowMs,
        searchFocus = searchFocus,
    )
}
