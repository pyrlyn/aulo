package aulo.app.daemon

import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.staticCFunction
import kotlinx.coroutines.CompletableDeferred
import platform.windows.CTRL_BREAK_EVENT
import platform.windows.CTRL_CLOSE_EVENT
import platform.windows.CTRL_C_EVENT
import platform.windows.DWORD
import platform.windows.FALSE
import platform.windows.SetConsoleCtrlHandler
import platform.windows.TRUE

private val received = CompletableDeferred<String>()

// Windows runs console control handlers on a thread of its own, not in signal context, so the
// handler may complete the deferred directly.
@OptIn(ExperimentalForeignApi::class)
actual fun installShutdownSignals() {
    val handler =
        staticCFunction { type: DWORD ->
            val name =
                when (type.toInt()) {
                    CTRL_C_EVENT -> "Ctrl+C"
                    CTRL_BREAK_EVENT -> "Ctrl+Break"
                    CTRL_CLOSE_EVENT -> "console close"
                    else -> null
                }
            if (name == null) {
                FALSE
            } else {
                received.complete(name)
                TRUE
            }
        }
    check(SetConsoleCtrlHandler(handler, TRUE) != 0) { "SetConsoleCtrlHandler failed" }
}

actual suspend fun awaitShutdownSignal(): String = received.await()
