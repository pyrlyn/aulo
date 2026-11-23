package aulo.chat.ui

import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.isSecondaryPressed
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import aulo.chat.data.CACHED_CHATS
import aulo.chat.domain.ChatSummary
import aulo.chat.domain.MessageHit
import aulo.chat.presentation.ChatUiState
import aulo.chat.ui.theme.LocalChatColors
import aulo.chat.ui.theme.LocalChatShapes
import aulo.chat.ui.theme.LocalUiPlatform
import aulo.chat.ui.theme.UiPlatform

/** The sidebar's width on a wide window, as in the desktop frames. */
internal val SidebarWidth = 280.dp

/** What the conversation list can ask for; [ChatWindow] wires these to the view model. */
class ChatListActions(
    val onOpen: (ChatSummary) -> Unit = {},
    val onNewChat: () -> Unit = {},
    val onStartRename: (ChatSummary) -> Unit = {},
    val onRename: (ChatSummary, String) -> Unit = { _, _ -> },
    val onCancelRename: () -> Unit = {},
    val onAskDelete: (ChatSummary) -> Unit = {},
    val onQueryChange: (String) -> Unit = {},
    val onOpenHit: (MessageHit) -> Unit = {},
    val onOpenSettings: () -> Unit = {},
)

/**
 * Search, New chat, the conversations (or the search results) and Settings. [renaming] is the
 * id of the chat whose title is being edited in place; [nowMs] dates the rows.
 */
@Composable
internal fun Sidebar(
    state: ChatUiState,
    renaming: String?,
    nowMs: Long,
    actions: ChatListActions,
    searchFocus: FocusRequester,
    modifier: Modifier = Modifier,
) {
    val colors = LocalChatColors.current
    Surface(modifier.width(SidebarWidth).fillMaxHeight(), color = colors.sidebar, contentColor = colors.text) {
        Column(Modifier.fillMaxSize().padding(horizontal = 12.dp, vertical = 12.dp)) {
            SearchField(state, actions.onQueryChange, searchFocus)
            NewChatButton(enabled = state.online, onClick = actions.onNewChat, modifier = Modifier.padding(top = 10.dp))
            Box(Modifier.weight(1f).fillMaxWidth().padding(top = 10.dp)) {
                if (state.search.active) {
                    SearchResults(state, nowMs, actions)
                } else {
                    LazyColumn(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        items(state.chats, key = { it.id }) { chat ->
                            ChatRow(
                                chat = chat,
                                selected = chat.id == state.chat?.id,
                                renaming = chat.id == renaming,
                                online = state.online,
                                nowMs = nowMs,
                                actions = actions,
                                modelName = state.modelName(chat),
                            )
                        }
                    }
                }
            }
            Text(
                "The $CACHED_CHATS most recent chats are kept on this device.",
                style = MaterialTheme.typography.bodySmall,
                color = colors.textMuted,
                modifier = Modifier.padding(top = 8.dp, start = 4.dp),
            )
            Surface(
                onClick = actions.onOpenSettings,
                shape = RoundedCornerShape(LocalChatShapes.current.control),
                color = colors.sidebar,
                contentColor = colors.text,
                modifier = Modifier.padding(top = 6.dp).fillMaxWidth(),
            ) {
                Row(
                    Modifier.padding(horizontal = 8.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Icon(ChatIcons.Settings, contentDescription = null, modifier = Modifier.size(18.dp))
                    Text("Settings", style = MaterialTheme.typography.bodyMedium)
                }
            }
        }
    }
}

@Composable
private fun SearchField(state: ChatUiState, onQueryChange: (String) -> Unit, focus: FocusRequester) {
    val colors = LocalChatColors.current
    Surface(
        shape = RoundedCornerShape(LocalChatShapes.current.control),
        color = colors.raised,
        contentColor = colors.text,
    ) {
        Row(
            Modifier.fillMaxWidth().height(32.dp).padding(start = 10.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(ChatIcons.Search, contentDescription = null, modifier = Modifier.size(16.dp), tint = colors.textMuted)
            Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
                if (state.search.query.isEmpty()) {
                    Text("Search chats", style = MaterialTheme.typography.bodyMedium, color = colors.textMuted)
                }
                BasicTextField(
                    value = state.search.query,
                    onValueChange = onQueryChange,
                    singleLine = true,
                    textStyle = MaterialTheme.typography.bodyMedium.copy(color = colors.text),
                    cursorBrush = SolidColor(colors.accent),
                    modifier = Modifier
                        .fillMaxWidth()
                        .focusRequester(focus)
                        .semantics { contentDescription = "Search chats" }
                        .onPreviewKeyEvent {
                            val escape = it.type == KeyEventType.KeyDown && it.key == Key.Escape
                            if (escape && state.search.query.isNotEmpty()) onQueryChange("")
                            escape && state.search.query.isNotEmpty()
                        },
                )
            }
            when {
                state.search.searching -> CircularProgressIndicator(Modifier.size(14.dp), strokeWidth = 2.dp)
                state.search.active -> IconButton(onClick = { onQueryChange("") }, modifier = Modifier.size(24.dp)) {
                    Icon(ChatIcons.Close, contentDescription = "Clear search", modifier = Modifier.size(14.dp))
                }
                LocalUiPlatform.current == UiPlatform.MacOs ->
                    Text("⌘F", style = MaterialTheme.typography.labelSmall, color = colors.textMuted, modifier = Modifier.padding(end = 6.dp))
            }
        }
    }
}

@Composable
private fun NewChatButton(enabled: Boolean, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val colors = LocalChatColors.current
    // Windows draws its primary command as an accent button; the other kits use a neutral fill.
    val accent = LocalUiPlatform.current == UiPlatform.Windows
    Surface(
        onClick = onClick,
        enabled = enabled,
        shape = RoundedCornerShape(LocalChatShapes.current.control),
        color = if (accent) colors.accent else colors.raised,
        contentColor = if (accent) colors.onAccent else colors.text,
        modifier = modifier.fillMaxWidth(),
    ) {
        Row(
            Modifier.height(32.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally),
        ) {
            Icon(ChatIcons.Add, contentDescription = null, modifier = Modifier.size(16.dp))
            Text("New chat", style = MaterialTheme.typography.labelLarge)
        }
    }
}

@Composable
private fun ChatRow(
    chat: ChatSummary,
    selected: Boolean,
    renaming: Boolean,
    online: Boolean,
    nowMs: Long,
    actions: ChatListActions,
    modelName: String?,
    query: String = "",
) {
    val colors = LocalChatColors.current
    var menuOpen by remember { mutableStateOf(false) }
    val hover = remember { MutableInteractionSource() }
    val hovered by hover.collectIsHoveredAsState()
    Box {
        Surface(
            onClick = { actions.onOpen(chat) },
            shape = RoundedCornerShape(LocalChatShapes.current.control),
            color = if (selected || menuOpen) colors.selection else colors.sidebar,
            contentColor = colors.text,
            modifier = Modifier
                .fillMaxWidth()
                .hoverable(hover)
                .pointerInput(chat.id) {
                    // A right click opens the same menu as the row's ⋯ button.
                    awaitPointerEventScope {
                        while (true) {
                            val event = awaitPointerEvent()
                            if (event.type == PointerEventType.Press && event.buttons.isSecondaryPressed) {
                                menuOpen = true
                            }
                        }
                    }
                },
        ) {
            Row(
                Modifier.padding(start = 10.dp, end = 4.dp, top = 8.dp, bottom = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    if (renaming) {
                        RenameField(chat, actions)
                    } else {
                        Text(
                            highlight(chatTitle(chat.title), query),
                            style = MaterialTheme.typography.titleSmall,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    // Offline the server's names are unknown; the model id stands in for them.
                    ModelChip(modelName ?: chat.model.model)
                }
                if (selected || hovered || menuOpen) {
                    IconButton(onClick = { menuOpen = true }, modifier = Modifier.size(28.dp)) {
                        Icon(ChatIcons.More, contentDescription = "Actions for ${chatTitle(chat.title)}", modifier = Modifier.size(16.dp))
                    }
                } else {
                    Text(
                        relativeTime(chat.updatedAtMs, nowMs),
                        style = MaterialTheme.typography.labelSmall,
                        color = colors.textMuted,
                        modifier = Modifier.padding(end = 6.dp),
                    )
                }
            }
        }
        ChatMenu(
            expanded = menuOpen,
            online = online,
            onDismiss = { menuOpen = false },
            onRename = { actions.onStartRename(chat) },
            onDelete = { actions.onAskDelete(chat) },
        )
    }
}

/** Rename and Delete, for a row and for the chat header. Offline both wait for the server. */
@Composable
internal fun ChatMenu(
    expanded: Boolean,
    online: Boolean,
    onDismiss: () -> Unit,
    onRename: () -> Unit,
    onDelete: () -> Unit,
) {
    val colors = LocalChatColors.current
    DropdownMenu(expanded = expanded, onDismissRequest = onDismiss) {
        val hint: (@Composable () -> Unit)? = if (online) null else {
            { Text("Needs the server", style = MaterialTheme.typography.labelSmall, color = colors.textMuted) }
        }
        DropdownMenuItem(
            text = { Text("Rename") },
            leadingIcon = { Icon(ChatIcons.Rename, contentDescription = null, modifier = Modifier.size(18.dp)) },
            trailingIcon = hint ?: { Text("F2", style = MaterialTheme.typography.labelSmall, color = colors.textMuted) },
            enabled = online,
            onClick = {
                onDismiss()
                onRename()
            },
        )
        HorizontalDivider(color = colors.outline)
        DropdownMenuItem(
            text = { Text("Delete", color = if (online) colors.danger else colors.textMuted) },
            leadingIcon = {
                Icon(ChatIcons.Delete, contentDescription = null, tint = colors.danger, modifier = Modifier.size(18.dp))
            },
            trailingIcon = hint,
            enabled = online,
            onClick = {
                onDismiss()
                onDelete()
            },
        )
    }
}

@Composable
private fun RenameField(chat: ChatSummary, actions: ChatListActions) {
    val colors = LocalChatColors.current
    val focus = remember { FocusRequester() }
    // Starts with the whole title selected, so typing replaces it.
    var value by remember { mutableStateOf(TextFieldValue(chat.title, TextRange(0, chat.title.length))) }
    var focused by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { focus.requestFocus() }
    BasicTextField(
        value = value,
        onValueChange = { value = it },
        singleLine = true,
        textStyle = MaterialTheme.typography.titleSmall.copy(color = colors.text),
        cursorBrush = SolidColor(colors.accent),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        // A blank title is not saved: the server refuses one.
        keyboardActions = KeyboardActions(onDone = { if (value.text.isNotBlank()) actions.onRename(chat, value.text) }),
        modifier = Modifier
            .fillMaxWidth()
            .focusRequester(focus)
            .semantics { contentDescription = "Chat title" }
            .onFocusChanged {
                // Clicking elsewhere drops the edit, as Escape does.
                if (focused && !it.isFocused) actions.onCancelRename()
                focused = it.isFocused
            }
            .onPreviewKeyEvent {
                if (it.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                when (it.key) {
                    Key.Escape -> actions.onCancelRename().let { true }
                    Key.Enter, Key.NumPadEnter -> {
                        if (value.text.isNotBlank()) actions.onRename(chat, value.text)
                        true
                    }
                    else -> false
                }
            },
    )
}

@Composable
private fun ModelChip(name: String) {
    val colors = LocalChatColors.current
    Surface(shape = RoundedCornerShape(LocalChatShapes.current.chip), color = colors.raised, contentColor = colors.textMuted) {
        Text(name, style = MaterialTheme.typography.labelSmall, maxLines = 1, modifier = Modifier.padding(horizontal = 6.dp, vertical = 1.dp))
    }
}

@Composable
private fun SearchResults(state: ChatUiState, nowMs: Long, actions: ChatListActions) {
    val colors = LocalChatColors.current
    val search = state.search
    val titles = state.titleMatches
    val nothing = titles.isEmpty() && search.hits.isEmpty() && !search.searching && search.error == null
    if (nothing) {
        Text(
            "No chats found",
            style = MaterialTheme.typography.bodyMedium,
            color = colors.textMuted,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth().padding(top = 24.dp),
        )
        return
    }
    val titleOf = state.chats.associate { it.id to it.title }
    LazyColumn(verticalArrangement = Arrangement.spacedBy(2.dp), contentPadding = PaddingValues(bottom = 8.dp)) {
        if (titles.isNotEmpty()) {
            item { SectionHeader("Chats") }
            items(titles, key = { "chat-" + it.id }) { chat ->
                ChatRow(
                    chat,
                    chat.id == state.chat?.id,
                    renaming = false,
                    online = state.online,
                    nowMs = nowMs,
                    actions = actions,
                    modelName = state.modelName(chat),
                    query = search.query,
                )
            }
        }
        if (search.error != null) {
            item {
                Text(
                    search.error.orEmpty(),
                    style = MaterialTheme.typography.bodySmall,
                    color = colors.textMuted,
                    modifier = Modifier.padding(horizontal = 10.dp, vertical = 8.dp),
                )
            }
        } else if (search.hits.isNotEmpty()) {
            item { SectionHeader("Messages") }
            items(search.hits, key = { "hit-" + it.messageId }) { hit ->
                HitRow(hit, titleOf[hit.chatId], search.query, nowMs) { actions.onOpenHit(hit) }
            }
        }
    }
}

@Composable
private fun SectionHeader(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.labelMedium,
        color = LocalChatColors.current.textMuted,
        modifier = Modifier.padding(start = 10.dp, top = 10.dp, bottom = 4.dp),
    )
}

@Composable
private fun HitRow(hit: MessageHit, title: String?, query: String, nowMs: Long, onClick: () -> Unit) {
    val colors = LocalChatColors.current
    Surface(
        onClick = onClick,
        shape = RoundedCornerShape(LocalChatShapes.current.control),
        color = colors.sidebar,
        contentColor = colors.text,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(Modifier.padding(horizontal = 10.dp, vertical = 8.dp)) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(
                    chatTitle(title),
                    style = MaterialTheme.typography.labelSmall,
                    color = colors.textMuted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                // The excerpt comes from the server: plain text, one line, never markup.
                Text(
                    highlight(hit.excerpt, query),
                    style = MaterialTheme.typography.bodySmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Text(
                relativeTime(hit.createdAtMs, nowMs),
                style = MaterialTheme.typography.labelSmall,
                color = colors.textMuted,
                modifier = Modifier.padding(start = 8.dp),
            )
        }
    }
}

private fun ChatUiState.modelName(chat: ChatSummary): String? =
    models.firstOrNull { it.id == chat.model }?.displayName

/** [text] with every word of [query] in bold, matched as the search matches: case ignored. */
internal fun highlight(text: String, query: String): AnnotatedString {
    val words = query.lowercase().split(' ').filter { it.isNotBlank() }
    if (words.isEmpty()) return AnnotatedString(text)
    val lower = text.lowercase()
    val bold = BooleanArray(text.length)
    for (word in words) {
        var at = lower.indexOf(word)
        while (at >= 0) {
            for (i in at until minOf(at + word.length, text.length)) bold[i] = true
            at = lower.indexOf(word, at + word.length)
        }
    }
    return buildAnnotatedString {
        append(text)
        var start = -1
        for (i in 0..text.length) {
            val on = i < text.length && bold[i]
            if (on && start < 0) start = i
            if (!on && start >= 0) {
                addStyle(SpanStyle(fontWeight = FontWeight.Bold), start, i)
                start = -1
            }
        }
    }
}

/** "now", "12 min", "3 h", "Yesterday", "5 d": the sidebar's dates, from the epoch millis. */
internal fun relativeTime(thenMs: Long, nowMs: Long): String {
    val minutes = (nowMs - thenMs).coerceAtLeast(0) / 60_000
    return when {
        minutes < 1 -> "now"
        minutes < 60 -> "$minutes min"
        minutes < 24 * 60 -> "${minutes / 60} h"
        minutes < 48 * 60 -> "Yesterday"
        else -> "${minutes / (24 * 60)} d"
    }
}
