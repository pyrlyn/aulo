package aulo.chat.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import aulo.chat.domain.Attachment
import aulo.chat.domain.AttachmentKind
import aulo.chat.domain.ChatMessage
import aulo.chat.domain.ConnectionState
import aulo.chat.domain.ModelId
import aulo.chat.domain.ModelInfo
import aulo.chat.domain.Notice
import aulo.chat.domain.NoticeLevel
import aulo.chat.presentation.ChatUiState
import aulo.chat.ui.theme.LocalChatColors
import aulo.chat.ui.theme.LocalChatShapes

/** The widest the messages and the composer grow on a large window. */
internal val ContentWidth = 760.dp

/** What the chat screen can ask for; [chatActions] wires these to the view model. */
class ChatActions(
    val onInputChange: (String) -> Unit = {},
    val onSend: () -> Unit = {},
    val onStop: () -> Unit = {},
    val onRetry: (ChatMessage) -> Unit = {},
    val onSelectModel: (ModelId) -> Unit = {},
    val onAttach: (AttachmentKind) -> Unit = {},
    val onRemoveAttachment: (Attachment) -> Unit = {},
    val onReconnect: () -> Unit = {},
    val onDismissNotice: () -> Unit = {},
    val onNewChat: () -> Unit = {},
)

/** The chat screen as a function of its state: everything it shows is in [state]. */
@Composable
fun ChatScreen(state: ChatUiState, actions: ChatActions, modifier: Modifier = Modifier) {
    val colors = LocalChatColors.current
    Surface(modifier.fillMaxSize(), color = colors.background, contentColor = colors.text) {
        Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            TopBar(state, actions)
            ChatBody(state, actions, Modifier.weight(1f))
        }
    }
}

/** Everything under the header: progress, banners, the messages and the composer. */
@Composable
internal fun ChatBody(state: ChatUiState, actions: ChatActions, modifier: Modifier = Modifier) {
    val colors = LocalChatColors.current
    Column(modifier.fillMaxWidth()) {
        if (state.loading || state.connection is ConnectionState.Connecting) {
            LinearProgressIndicator(Modifier.fillMaxWidth().height(2.dp))
        }
        if (state.connection is ConnectionState.Offline) {
            Banner(
                text = "Offline — showing chats saved on this device.",
                container = colors.warningContainer,
                content = colors.onWarningContainer,
                action = "Retry",
                onAction = actions.onReconnect,
            )
        }
        state.notice?.let { NoticeBanner(it, actions.onDismissNotice) }
        Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
            when {
                state.loading -> Unit
                state.empty -> EmptyState(onSuggestion = actions.onInputChange)
                else -> MessageList(state.messages, actions.onRetry)
            }
        }
        Composer(state, actions)
    }
}

@Composable
private fun TopBar(state: ChatUiState, actions: ChatActions) {
    val colors = LocalChatColors.current
    Surface(color = colors.surface, contentColor = colors.text) {
        Row(
            Modifier.fillMaxWidth().height(60.dp).padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Column(Modifier.weight(1f)) {
                Text(
                    text = state.chat?.title?.ifBlank { null } ?: "New chat",
                    style = MaterialTheme.typography.titleMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                ConnectionLabel(state.connection)
            }
            ModelPicker(state.chat?.model, state.models, enabled = state.online, onSelect = actions.onSelectModel)
            IconButton(onClick = actions.onNewChat, enabled = state.online) {
                Icon(ChatIcons.Add, contentDescription = "New chat")
            }
        }
    }
}

@Composable
internal fun ConnectionLabel(connection: ConnectionState) {
    val colors = LocalChatColors.current
    val (dot, label) = when (connection) {
        ConnectionState.Online -> colors.ok to "Online"
        ConnectionState.Connecting -> colors.textMuted to "Connecting…"
        is ConnectionState.Offline -> colors.textMuted to "Offline"
    }
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Box(Modifier.size(8.dp).background(dot, CircleShape))
        Text(label, style = MaterialTheme.typography.labelSmall, color = colors.textMuted)
    }
}

@Composable
internal fun ModelPicker(current: ModelId?, models: List<ModelInfo>, enabled: Boolean, onSelect: (ModelId) -> Unit) {
    val colors = LocalChatColors.current
    var open by remember { mutableStateOf(false) }
    // Offline the list is empty; the chat's own model id is all that is known then.
    val name = models.firstOrNull { it.id == current }?.displayName ?: current?.toString() ?: "No model"
    Box {
        Surface(
            onClick = { open = true },
            enabled = enabled && models.isNotEmpty(),
            shape = RoundedCornerShape(LocalChatShapes.current.control),
            color = colors.raised,
            contentColor = colors.text,
        ) {
            Row(
                Modifier.padding(start = 12.dp, end = 8.dp, top = 8.dp, bottom = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(name, style = MaterialTheme.typography.labelLarge, maxLines = 1)
                Icon(ChatIcons.ChevronDown, contentDescription = "Choose model", modifier = Modifier.size(16.dp))
            }
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            for (model in models) {
                DropdownMenuItem(
                    text = {
                        Column {
                            Text(model.displayName)
                            Text(
                                text = if (model.served) model.id.toString() else "Unavailable",
                                style = MaterialTheme.typography.bodySmall,
                                color = colors.textMuted,
                            )
                        }
                    },
                    trailingIcon = if (model.id == current) {
                        { Icon(ChatIcons.Check, contentDescription = "Selected", modifier = Modifier.size(18.dp)) }
                    } else {
                        null
                    },
                    enabled = model.served,
                    onClick = {
                        open = false
                        if (model.id != current) onSelect(model.id)
                    },
                )
            }
        }
    }
}

@Composable
private fun NoticeBanner(notice: Notice, onDismiss: () -> Unit) {
    val colors = LocalChatColors.current
    val (container, content) = when (notice.level) {
        NoticeLevel.Info -> colors.infoContainer to colors.onInfoContainer
        NoticeLevel.Warning -> colors.warningContainer to colors.onWarningContainer
        NoticeLevel.Error -> colors.dangerContainer to colors.onDangerContainer
    }
    Banner(notice.message, container, content, action = "Dismiss", onAction = onDismiss)
}

@Composable
internal fun Banner(text: String, container: Color, content: Color, action: String, onAction: () -> Unit) {
    Surface(color = container, contentColor = content) {
        Row(
            Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = text,
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f).padding(vertical = 10.dp),
            )
            TextButton(onClick = onAction) { Text(action, color = content) }
        }
    }
}

@Composable
private fun MessageList(messages: List<ChatMessage>, onRetry: (ChatMessage) -> Unit) {
    val listState = rememberLazyListState()
    // The list is laid out from the bottom, so a growing answer stays in view by itself; only a
    // new message needs a scroll.
    LaunchedEffect(messages.lastOrNull()?.id) { listState.animateScrollToItem(0) }
    LazyColumn(
        state = listState,
        reverseLayout = true,
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp, Alignment.Top),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        items(messages.asReversed(), key = { it.id }) { message ->
            // animateItem fades a new message in and slides the others to make room.
            Box(Modifier.animateItem().widthIn(max = ContentWidth).fillMaxWidth()) {
                MessageBubble(message, onRetry)
            }
        }
    }
}

@Composable
private fun EmptyState(onSuggestion: (String) -> Unit) {
    val colors = LocalChatColors.current
    Column(
        // Scrolls, so the suggestions stay reachable when the keyboard takes half the screen.
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterVertically),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text("What can I help with?", style = MaterialTheme.typography.headlineSmall, textAlign = TextAlign.Center)
        Text(
            "Ask anything. Answers stream in as they are written.",
            style = MaterialTheme.typography.bodyMedium,
            color = colors.textMuted,
            textAlign = TextAlign.Center,
        )
        Column(
            Modifier.padding(top = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            for (suggestion in Suggestions) {
                Surface(
                    onClick = { onSuggestion(suggestion) },
                    shape = RoundedCornerShape(LocalChatShapes.current.control),
                    color = colors.raised,
                    contentColor = colors.text,
                ) {
                    Text(
                        suggestion,
                        style = MaterialTheme.typography.labelLarge,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
                    )
                }
            }
        }
    }
}

private val Suggestions = listOf("Summarise a document", "Draft a reply", "Explain an error")
