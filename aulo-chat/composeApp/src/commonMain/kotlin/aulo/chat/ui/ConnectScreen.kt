package aulo.chat.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import aulo.chat.presentation.ServerStatus
import aulo.chat.presentation.SettingsUiState
import aulo.chat.ui.theme.LocalChatColors

/** The first start: no server has answered yet and nothing is cached to show. */
@Composable
fun ConnectScreen(state: SettingsUiState, actions: SettingsActions, modifier: Modifier = Modifier) {
    val colors = LocalChatColors.current
    Surface(modifier.fillMaxSize(), color = colors.background, contentColor = colors.text) {
        Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState()), contentAlignment = Alignment.Center) {
            Column(
                Modifier.width(360.dp).padding(24.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Mark()
                Text("Connect to aulo", style = MaterialTheme.typography.headlineSmall)
                Text(
                    "Start the aulo-chat server, then enter its address.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.textMuted,
                    textAlign = TextAlign.Center,
                )
                AddressField(state, actions.onAddressChange, actions.onConnect, Modifier.fillMaxWidth())
                Button(onClick = actions.onConnect, enabled = state.canConnect, modifier = Modifier.fillMaxWidth()) {
                    Text("Connect")
                }
                // Before the first try there is nothing to report; "Connected" never shows here.
                if (state.status != ServerStatus.Connected) ServerStatusLine(state.status)
                Text(
                    "./gradlew :server:run",
                    style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                    color = colors.textMuted,
                )
            }
        }
    }
}

/** The aulo mark: a rounded accent square with an "a". */
@Composable
private fun Mark() {
    val colors = LocalChatColors.current
    Surface(Modifier.size(48.dp), shape = RoundedCornerShape(12.dp), color = colors.accent, contentColor = colors.onAccent) {
        Box(contentAlignment = Alignment.Center) {
            Text("a", style = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.Bold))
        }
    }
}
