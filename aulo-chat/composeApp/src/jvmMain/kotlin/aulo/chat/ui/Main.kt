package aulo.chat.ui

import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import aulo.chat.data.ServerEndpoint
import aulo.chat.di.chatModules
import aulo.chat.di.desktopPlatformModule
import org.koin.core.context.startKoin
import java.io.File
import kotlin.system.exitProcess

fun main() {
    val endpoint = endpointFromEnv(System.getenv()) ?: run {
        System.err.println("AULO_CHAT_PORT must be a port number between 1 and 65535.")
        exitProcess(2)
    }
    startKoin { modules(chatModules(endpoint, desktopPlatformModule(dataDir()))) }
    application {
        Window(
            onCloseRequest = ::exitApplication,
            title = "aulo chat",
            state = rememberWindowState(width = 1120.dp, height = 720.dp),
        ) {
            App()
        }
    }
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
