package aulo.chat.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import aulo.chat.data.CACHED_CHATS
import aulo.chat.domain.Appearance
import aulo.chat.domain.ModelId
import aulo.chat.presentation.ServerStatus
import aulo.chat.presentation.SettingsUiState
import aulo.chat.ui.theme.LocalChatColors
import aulo.chat.ui.theme.LocalChatShapes

/** What Settings can ask for; the caller wires these to [aulo.chat.presentation.SettingsViewModel]. */
class SettingsActions(
    val onAddressChange: (String) -> Unit = {},
    val onConnect: () -> Unit = {},
    val onAppearance: (Appearance) -> Unit = {},
    val onDefaultModel: (ModelId?) -> Unit = {},
    val onClearCache: () -> Unit = {},
)

internal const val ADDRESS_HINT = "Use host:port, port 1–65535."

/** Server, appearance, the model new chats start with, the local cache, and the version. */
@Composable
fun SettingsScreen(state: SettingsUiState, version: String, actions: SettingsActions, modifier: Modifier = Modifier) {
    val colors = LocalChatColors.current
    var confirmClear by remember { mutableStateOf(false) }
    Surface(modifier.fillMaxSize(), color = colors.surface, contentColor = colors.text) {
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            Text("Settings", style = MaterialTheme.typography.titleLarge)
            Group("Server") {
                AddressField(state, actions.onAddressChange, actions.onConnect, Modifier.fillMaxWidth())
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    ServerStatusLine(state.status, Modifier.weight(1f))
                    Button(onClick = actions.onConnect, enabled = state.canConnect) { Text("Connect") }
                }
            }
            Group("Appearance") {
                Segmented(
                    options = Appearance.entries,
                    selected = state.appearance,
                    label = { it.name },
                    onSelect = actions.onAppearance,
                )
            }
            Group("New chats") {
                DefaultModelPicker(state, actions.onDefaultModel)
            }
            Group("Device") {
                Text(
                    "The $CACHED_CHATS most recent chats are kept on this device.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.textMuted,
                )
                TextButton(
                    onClick = { confirmClear = true },
                    colors = ButtonDefaults.textButtonColors(contentColor = colors.danger),
                ) { Text("Clear local cache") }
            }
            Text("aulo chat $version", style = MaterialTheme.typography.bodySmall, color = colors.textMuted)
        }
    }
    if (confirmClear) {
        ConfirmDialog(
            title = "Clear the local cache?",
            body = "The chats saved on this device are removed. The server keeps them, and they come back on the next connection.",
            confirm = "Clear",
            onConfirm = actions.onClearCache,
            onDismiss = { confirmClear = false },
        )
    }
}

@Composable
private fun Group(title: String, content: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(title, style = MaterialTheme.typography.labelLarge, color = LocalChatColors.current.textMuted)
        content()
    }
}

/** The server field of Settings and of the Connect screen; Enter connects. */
@Composable
internal fun AddressField(
    state: SettingsUiState,
    onChange: (String) -> Unit,
    onConnect: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val invalid = state.address.isNotBlank() && !state.addressValid
    OutlinedTextField(
        value = state.address,
        onValueChange = onChange,
        label = { Text("Address") },
        placeholder = { Text("127.0.0.1:50051") },
        singleLine = true,
        enabled = state.status != ServerStatus.Connecting,
        isError = invalid,
        supportingText = if (invalid) {
            { Text(ADDRESS_HINT) }
        } else {
            null
        },
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Go),
        keyboardActions = KeyboardActions(onGo = { if (state.canConnect) onConnect() }),
        modifier = modifier,
    )
}

@Composable
internal fun ServerStatusLine(status: ServerStatus, modifier: Modifier = Modifier) {
    val colors = LocalChatColors.current
    Row(modifier, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        when (status) {
            ServerStatus.Connected -> {
                Box(Modifier.size(8.dp).background(colors.ok, CircleShape))
                Text("Connected", style = MaterialTheme.typography.bodySmall, color = colors.textMuted)
            }
            ServerStatus.Connecting -> {
                CircularProgressIndicator(Modifier.size(14.dp), strokeWidth = 2.dp)
                Text("Connecting…", style = MaterialTheme.typography.bodySmall, color = colors.textMuted)
            }
            // The reason is the transport's message: shown as written, capped by the line count.
            is ServerStatus.Failed -> Text(status.reason, style = MaterialTheme.typography.bodySmall, color = colors.danger, maxLines = 3)
        }
    }
}

@Composable
private fun <T> Segmented(options: List<T>, selected: T, label: (T) -> String, onSelect: (T) -> Unit) {
    val colors = LocalChatColors.current
    val shape = RoundedCornerShape(LocalChatShapes.current.control)
    Row(Modifier.background(colors.raised, shape).padding(2.dp), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
        for (option in options) {
            val on = option == selected
            Surface(
                onClick = { onSelect(option) },
                shape = shape,
                color = if (on) colors.surface else colors.raised,
                contentColor = if (on) colors.text else colors.textMuted,
                shadowElevation = if (on) 1.dp else 0.dp,
            ) {
                Text(label(option), style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(horizontal = 16.dp, vertical = 6.dp))
            }
        }
    }
}

@Composable
private fun DefaultModelPicker(state: SettingsUiState, onSelect: (ModelId?) -> Unit) {
    val colors = LocalChatColors.current
    var open by remember { mutableStateOf(false) }
    // Offline the list is empty: the stored choice is shown and cannot be changed.
    val name = state.defaultModel?.let { id -> state.models.firstOrNull { it.id == id }?.displayName ?: id.toString() }
        ?: "Server default"
    Box {
        Surface(
            onClick = { open = true },
            enabled = state.models.isNotEmpty(),
            shape = RoundedCornerShape(LocalChatShapes.current.control),
            color = colors.raised,
            contentColor = colors.text,
        ) {
            Row(
                Modifier.padding(start = 12.dp, end = 8.dp, top = 8.dp, bottom = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Text("Default model:", style = MaterialTheme.typography.labelLarge, color = colors.textMuted)
                Text(name, style = MaterialTheme.typography.labelLarge)
                Icon(ChatIcons.ChevronDown, contentDescription = "Choose the default model", modifier = Modifier.size(16.dp))
            }
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            DropdownMenuItem(text = { Text("Server default") }, onClick = {
                open = false
                onSelect(null)
            })
            for (model in state.models.filter { it.served }) {
                DropdownMenuItem(text = { Text(model.displayName) }, onClick = {
                    open = false
                    onSelect(model.id)
                })
            }
        }
    }
}
