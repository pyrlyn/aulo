package aulo.chat.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import aulo.chat.domain.ChatSummary
import aulo.chat.presentation.ChatUiState
import aulo.chat.ui.theme.LocalChatColors
import aulo.chat.ui.theme.UiPlatform

/**
 * The wide layout: the conversation list beside the open chat. [renaming] and [deleting] are
 * the chat whose title is being edited and the one waiting for the delete confirmation.
 */
@Composable
fun ChatWindow(
    state: ChatUiState,
    actions: ChatActions,
    list: ChatListActions,
    renaming: String?,
    deleting: ChatSummary?,
    onConfirmDelete: (ChatSummary) -> Unit,
    onCancelDelete: () -> Unit,
    nowMs: Long,
    searchFocus: FocusRequester = remember { FocusRequester() },
    modifier: Modifier = Modifier,
) {
    val colors = LocalChatColors.current
    Surface(modifier.fillMaxSize(), color = colors.background, contentColor = colors.text) {
        Row(Modifier.fillMaxSize()) {
            Sidebar(state, renaming, nowMs, list, searchFocus)
            VerticalDivider(color = colors.outline)
            Column(Modifier.weight(1f).fillMaxHeight()) {
                ChatHeader(state, actions, list)
                HorizontalDivider(color = colors.outline)
                ChatBody(state, actions, Modifier.weight(1f))
            }
        }
    }
    deleting?.let { chat ->
        ConfirmDialog(
            title = "Delete this chat?",
            body = "“${chatTitle(chat.title)}” and its messages are deleted on the server and on this " +
                "device. This cannot be undone.",
            confirm = "Delete",
            onConfirm = { onConfirmDelete(chat) },
            onDismiss = onCancelDelete,
        )
    }
}

@Composable
private fun ChatHeader(state: ChatUiState, actions: ChatActions, list: ChatListActions) {
    val colors = LocalChatColors.current
    var menuOpen by remember { mutableStateOf(false) }
    Row(
        Modifier.fillMaxWidth().height(52.dp).padding(start = 20.dp, end = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        // One weighted group, not a weighted title beside a weighted spacer: a title that does not
        // fill its share would leave that share empty, and the status would stop short of the edge.
        Row(
            Modifier.weight(1f),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                text = chatTitle(state.chat?.title),
                style = MaterialTheme.typography.titleMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false),
            )
            ModelPicker(state.chat?.model, state.models, enabled = state.online, onSelect = actions.onSelectModel)
        }
        ConnectionLabel(state.connection)
        val chat = state.chat
        Box {
            IconButton(onClick = { menuOpen = true }, enabled = chat != null) {
                Icon(ChatIcons.More, contentDescription = "Chat actions", tint = colors.textMuted, modifier = Modifier.size(18.dp))
            }
            if (chat != null) {
                ChatMenu(
                    expanded = menuOpen,
                    online = state.online,
                    onDismiss = { menuOpen = false },
                    onRename = { list.onStartRename(chat) },
                    onDelete = { list.onAskDelete(chat) },
                )
            }
        }
    }
}

/**
 * The window's keys. Cmd on macOS and Ctrl elsewhere: +F or +K finds, +N starts a chat; F2
 * renames the open chat. Delete has no key of its own: it would fire while typing in the
 * composer, and deleting is never one keystroke anyway.
 */
internal fun windowShortcut(
    event: KeyEvent,
    platform: UiPlatform,
    state: ChatUiState,
    onFind: () -> Unit,
    onNewChat: () -> Unit,
    onRename: (ChatSummary) -> Unit,
): Boolean {
    if (event.type != KeyEventType.KeyDown) return false
    val command = if (platform == UiPlatform.MacOs) event.isMetaPressed else event.isCtrlPressed
    val chat = state.chat
    return when {
        command && (event.key == Key.F || event.key == Key.K) -> onFind().let { true }
        command && event.key == Key.N && state.online -> onNewChat().let { true }
        event.key == Key.F2 && chat != null && state.online -> onRename(chat).let { true }
        else -> false
    }
}
