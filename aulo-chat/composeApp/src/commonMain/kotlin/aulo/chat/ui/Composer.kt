package aulo.chat.ui

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.unit.dp
import aulo.chat.domain.AttachmentKind
import aulo.chat.domain.ConnectionState
import aulo.chat.presentation.ChatUiState
import aulo.chat.ui.theme.LocalChatColors
import aulo.chat.ui.theme.LocalChatShapes

/** The input row: attach, type, then send, or stop while an answer is being written. */
@Composable
internal fun Composer(state: ChatUiState, actions: ChatActions) {
    val colors = LocalChatColors.current
    Surface(color = colors.surface, contentColor = colors.text) {
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            Column(
                Modifier.widthIn(max = ContentWidth).fillMaxWidth().padding(horizontal = 8.dp, vertical = 10.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (state.attachments.isNotEmpty()) {
                    Row(
                        Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        for (attachment in state.attachments) {
                            AttachmentChip(attachment, onRemove = { actions.onRemoveAttachment(attachment) })
                        }
                    }
                }
                Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    AttachButton(enabled = state.online, onAttach = actions.onAttach)
                    TextField(
                        value = state.input,
                        onValueChange = actions.onInputChange,
                        enabled = state.online,
                        placeholder = { Text(placeholder(state.connection)) },
                        maxLines = 6,
                        shape = RoundedCornerShape(LocalChatShapes.current.control),
                        colors = TextFieldDefaults.colors(
                            focusedContainerColor = colors.raised,
                            unfocusedContainerColor = colors.raised,
                            disabledContainerColor = colors.raised,
                            focusedIndicatorColor = Color.Transparent,
                            unfocusedIndicatorColor = Color.Transparent,
                            disabledIndicatorColor = Color.Transparent,
                        ),
                        // A hardware Enter sends, Shift+Enter breaks the line; touch keyboards
                        // send no key events, so there Enter stays a line break.
                        modifier = Modifier.weight(1f).onPreviewKeyEvent { event ->
                            if (event.key != Key.Enter || event.isShiftPressed) return@onPreviewKeyEvent false
                            if (event.type == KeyEventType.KeyDown && state.canSend) actions.onSend()
                            true
                        },
                    )
                    if (state.generating) {
                        FilledIconButton(onClick = actions.onStop, modifier = Modifier.padding(start = 4.dp, bottom = 4.dp)) {
                            Icon(ChatIcons.Stop, contentDescription = "Stop generating")
                        }
                    } else {
                        FilledIconButton(
                            onClick = actions.onSend,
                            enabled = state.canSend,
                            modifier = Modifier.padding(start = 4.dp, bottom = 4.dp),
                        ) {
                            Icon(ChatIcons.Send, contentDescription = "Send")
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun AttachButton(enabled: Boolean, onAttach: (AttachmentKind) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box(Modifier.padding(bottom = 4.dp)) {
        IconButton(onClick = { open = true }, enabled = enabled) {
            Icon(ChatIcons.Add, contentDescription = "Add to message")
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            for (kind in AttachmentKind.entries) {
                DropdownMenuItem(
                    text = { Text(kind.label) },
                    leadingIcon = { Icon(kind.icon, contentDescription = null, modifier = Modifier.size(20.dp)) },
                    onClick = {
                        open = false
                        onAttach(kind)
                    },
                )
            }
        }
    }
}

private fun placeholder(connection: ConnectionState): String = when (connection) {
    ConnectionState.Online -> "Message aulo"
    ConnectionState.Connecting -> "Connecting…"
    is ConnectionState.Offline -> "Reconnect to send a message"
}
