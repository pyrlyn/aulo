package aulo.chat.ui

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import aulo.chat.ui.theme.LocalChatColors

/**
 * A destructive step, asked once more. Cancel is the default button and holds the focus, so
 * Enter or Space alone never confirms: the destructive button needs its own click or key.
 */
@Composable
internal fun ConfirmDialog(
    title: String,
    body: String,
    confirm: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    val colors = LocalChatColors.current
    val cancelFocus = remember { FocusRequester() }
    LaunchedEffect(Unit) { cancelFocus.requestFocus() }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title, style = MaterialTheme.typography.titleMedium) },
        text = { Text(body, style = MaterialTheme.typography.bodyMedium, color = colors.textMuted) },
        // Material puts its confirm slot where the kits put the default button; that is Cancel.
        dismissButton = {
            TextButton(
                onClick = {
                    onDismiss()
                    onConfirm()
                },
                colors = ButtonDefaults.textButtonColors(contentColor = colors.danger),
            ) { Text(confirm) }
        },
        confirmButton = {
            Button(onClick = onDismiss, modifier = Modifier.focusRequester(cancelFocus)) { Text("Cancel") }
        },
        containerColor = colors.surface,
        titleContentColor = colors.text,
    )
}
