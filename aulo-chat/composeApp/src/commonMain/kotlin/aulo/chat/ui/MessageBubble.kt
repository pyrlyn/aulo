package aulo.chat.ui

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.StartOffset
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.dp
import aulo.chat.domain.Attachment
import aulo.chat.domain.AttachmentKind
import aulo.chat.domain.ChatMessage
import aulo.chat.domain.MessageRole
import aulo.chat.domain.MessageState
import aulo.chat.ui.theme.LocalChatColors
import aulo.chat.ui.theme.LocalChatShapes

/**
 * One message. The text is drawn as plain text on purpose: what a model writes is untrusted,
 * so nothing in it is interpreted as markup or a link.
 */
@Composable
internal fun MessageBubble(message: ChatMessage, onRetry: (ChatMessage) -> Unit) {
    val colors = LocalChatColors.current
    val user = message.role == MessageRole.User
    Row(Modifier.fillMaxWidth(), horizontalArrangement = if (user) Arrangement.End else Arrangement.Start) {
        // The spacer keeps a long message from touching the far edge.
        if (user) Spacer(Modifier.width(48.dp))
        Column(
            Modifier.weight(1f, fill = false).widthIn(max = 560.dp),
            horizontalAlignment = if (user) Alignment.End else Alignment.Start,
        ) {
            Surface(
                color = if (user) colors.userBubble else colors.raised,
                contentColor = if (user) colors.onUserBubble else colors.text,
                shape = RoundedCornerShape(LocalChatShapes.current.bubble),
            ) {
                Column(
                    Modifier.padding(horizontal = 14.dp, vertical = 10.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    for (attachment in message.attachments) AttachmentChip(attachment, onRemove = null)
                    val streaming = message.state == MessageState.Streaming
                    when {
                        message.text.isNotEmpty() -> MessageText(message.text, cursor = streaming)
                        streaming -> TypingIndicator()
                    }
                    if (message.state == MessageState.Failed) {
                        Column {
                            Text(
                                text = message.error ?: "The answer failed.",
                                style = MaterialTheme.typography.bodySmall,
                                color = colors.danger,
                            )
                            TextButton(
                                onClick = { onRetry(message) },
                                contentPadding = PaddingValues(horizontal = 4.dp),
                            ) { Text("Retry") }
                        }
                    }
                }
            }
            if (message.state == MessageState.Interrupted) {
                Text(
                    "Stopped",
                    style = MaterialTheme.typography.labelSmall,
                    color = colors.textMuted,
                    modifier = Modifier.padding(top = 4.dp, start = 6.dp),
                )
            }
        }
        if (!user) Spacer(Modifier.width(48.dp))
    }
}

/** The text of a message; while it streams, a blinking bar follows its last character. */
@Composable
private fun MessageText(text: String, cursor: Boolean) {
    if (!cursor) {
        Text(text, style = MaterialTheme.typography.bodyLarge)
        return
    }
    val color = LocalContentColor.current
    val blink by rememberInfiniteTransition().animateFloat(
        initialValue = 1f,
        targetValue = 0f,
        animationSpec = infiniteRepeatable(tween(530, easing = LinearEasing), RepeatMode.Reverse),
    )
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    Text(
        text = text,
        style = MaterialTheme.typography.bodyLarge,
        onTextLayout = { layout = it },
        // The end padding is the room the bar needs after the last character of a full line.
        modifier = Modifier.padding(end = 6.dp).drawWithContent {
            drawContent()
            val measured = layout ?: return@drawWithContent
            // The bar is drawn rather than typed: fonts differ on having a block glyph.
            val caret = measured.getCursorRect(measured.layoutInput.text.length)
            drawRect(
                color = color.copy(alpha = blink),
                topLeft = Offset(caret.left + 2.dp.toPx(), caret.top + 2.dp.toPx()),
                size = Size(2.dp.toPx(), caret.height - 4.dp.toPx()),
            )
        },
    )
}

@Composable
private fun TypingIndicator() {
    val transition = rememberInfiniteTransition()
    Row(
        Modifier.padding(vertical = 6.dp).semantics { contentDescription = "The answer is being written" },
        horizontalArrangement = Arrangement.spacedBy(5.dp),
    ) {
        repeat(3) { index ->
            val alpha by transition.animateFloat(
                initialValue = 0.25f,
                targetValue = 1f,
                animationSpec = infiniteRepeatable(
                    animation = tween(450, easing = LinearEasing),
                    repeatMode = RepeatMode.Reverse,
                    initialStartOffset = StartOffset(index * 150),
                ),
            )
            Box(Modifier.size(7.dp).graphicsLayer { this.alpha = alpha }.background(LocalContentColor.current, CircleShape))
        }
    }
}

/** An attachment's label. With [onRemove] it is a staged chip in the composer. */
@Composable
internal fun AttachmentChip(attachment: Attachment, onRemove: (() -> Unit)?) {
    val content = LocalContentColor.current
    Surface(
        shape = RoundedCornerShape(LocalChatShapes.current.chip),
        // A tint of the text colour reads on the user bubble and on the composer alike.
        color = content.copy(alpha = 0.12f),
        contentColor = content,
    ) {
        Row(
            Modifier.padding(start = 8.dp, end = if (onRemove == null) 10.dp else 2.dp, top = 4.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Icon(attachment.kind.icon, contentDescription = attachment.kind.label, modifier = Modifier.size(16.dp))
            Text("${attachment.name} · ${attachment.detail}", style = MaterialTheme.typography.labelMedium, maxLines = 1)
            if (onRemove != null) {
                Surface(onClick = onRemove, shape = CircleShape, color = Color.Transparent, contentColor = content) {
                    Icon(
                        ChatIcons.Close,
                        contentDescription = "Remove ${attachment.name}",
                        modifier = Modifier.padding(6.dp).size(14.dp),
                    )
                }
            }
        }
    }
}

internal val AttachmentKind.icon
    get() = when (this) {
        AttachmentKind.Text -> ChatIcons.Document
        AttachmentKind.Image -> ChatIcons.Image
        AttachmentKind.Voice -> ChatIcons.Mic
    }

internal val AttachmentKind.label
    get() = when (this) {
        AttachmentKind.Text -> "Text file"
        AttachmentKind.Image -> "Image"
        AttachmentKind.Voice -> "Voice note"
    }
