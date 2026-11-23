package aulo.chat.ui

import androidx.compose.runtime.Composable
import aulo.chat.ui.theme.AuloTheme

/** The whole app: one screen. Koin must be started before this is composed. */
@Composable
fun App() {
    AuloTheme { ChatRoute() }
}
