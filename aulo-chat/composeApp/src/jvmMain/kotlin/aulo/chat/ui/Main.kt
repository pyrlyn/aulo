package aulo.chat.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.type
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import aulo.chat.data.ServerEndpoint
import aulo.chat.di.chatModules
import aulo.chat.di.desktopPlatformModule
import aulo.chat.presentation.ChatViewModel
import aulo.chat.presentation.SettingsViewModel
import aulo.chat.ui.theme.AuloTheme
import aulo.chat.ui.theme.UiPlatform
import aulo.chat.ui.theme.currentUiPlatform
import aulo.chat.ui.theme.isDark
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import org.koin.compose.viewmodel.koinViewModel
import org.koin.core.context.startKoin
import java.io.File
import kotlin.system.exitProcess

fun main() {
    val endpoint = endpointFromEnv(System.getenv()) ?: run {
        System.err.println("AULO_CHAT_PORT must be a port number between 1 and 65535.")
        exitProcess(2)
    }
    // Set explicitly for this run, the environment wins over the address saved in Settings.
    val pinned = listOf("AULO_CHAT_HOST", "AULO_CHAT_PORT").any { !System.getenv(it).isNullOrBlank() }
    startKoin { modules(chatModules(endpoint, desktopPlatformModule(dataDir()), pinned)) }
    // jpackage sets this in an installed app; a run from Gradle has no version.
    val version = System.getProperty("jpackage.app-version") ?: "development build"
    application {
        val keys = remember { KeyRouter() }
        var settingsOpen by remember { mutableStateOf(false) }
        Window(
            onCloseRequest = ::exitApplication,
            title = "aulo chat",
            state = rememberWindowState(width = 1120.dp, height = 720.dp),
            onPreviewKeyEvent = { event ->
                if (isSettingsShortcut(event)) {
                    settingsOpen = true
                    true
                } else {
                    keys.dispatch(event)
                }
            },
        ) {
            val chatVm = koinViewModel<ChatViewModel>()
            val settingsVm = koinViewModel<SettingsViewModel>()
            App(version, keys, onOpenSettings = { settingsOpen = true }, chatVm = chatVm, settingsVm = settingsVm)
            if (settingsOpen) {
                val settings by settingsVm.state.collectAsStateWithLifecycle()
                Window(
                    onCloseRequest = { settingsOpen = false },
                    title = "Settings",
                    state = rememberWindowState(width = 560.dp, height = 520.dp),
                    resizable = false,
                    onPreviewKeyEvent = { event ->
                        val close = event.type == KeyEventType.KeyDown && event.key == Key.Escape
                        if (close) settingsOpen = false
                        close
                    },
                ) {
                    AuloTheme(dark = settings.appearance.isDark()) {
                        SettingsScreen(settings, version, settingsActions(settingsVm, chatVm))
                    }
                }
            }
        }
    }
}

/** Cmd+, on macOS and Ctrl+, elsewhere, as every desktop app opens its settings. */
private fun isSettingsShortcut(event: KeyEvent): Boolean {
    val command = if (currentUiPlatform() == UiPlatform.MacOs) event.isMetaPressed else event.isCtrlPressed
    return event.type == KeyEventType.KeyDown && command && event.key == Key.Comma
}

/** `AULO_CHAT_HOST` and `AULO_CHAT_PORT` override the default server; null when the port is not one. */
internal fun endpointFromEnv(env: Map<String, String>): ServerEndpoint? =
    ServerEndpoint.parse(env["AULO_CHAT_HOST"], env["AULO_CHAT_PORT"])

// Where each desktop keeps an app's own data. Three lines per system did not earn a dependency.
private fun dataDir(): File {
    val home = File(System.getProperty("user.home"))
    val os = System.getProperty("os.name").orEmpty().lowercase()
    val base = when {
        "mac" in os -> File(home, "Library/Application Support")
        "win" in os -> System.getenv("LOCALAPPDATA")?.let(::File) ?: File(home, "AppData/Local")
        else -> System.getenv("XDG_DATA_HOME")?.takeIf { it.isNotBlank() }?.let(::File) ?: File(home, ".local/share")
    }
    return File(base, "aulo-chat")
}
