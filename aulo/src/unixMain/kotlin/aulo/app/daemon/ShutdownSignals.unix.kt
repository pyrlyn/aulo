package aulo.app.daemon

import aulo.app.daemon.signals.aulo_signals_install
import aulo.app.daemon.signals.aulo_signals_wait
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.toKString
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.DelicateCoroutinesApi
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.newSingleThreadContext
import platform.posix.SIGINT
import platform.posix.SIGTERM
import platform.posix.strerror

private val received = CompletableDeferred<String>()

// The C side (signals.def) says why the signals arrive through a pipe.
@OptIn(ExperimentalForeignApi::class, DelicateCoroutinesApi::class, ExperimentalCoroutinesApi::class)
actual fun installShutdownSignals() {
    val error = aulo_signals_install()
    check(error == 0) { "cannot handle SIGINT and SIGTERM: ${strerror(error)?.toKString()}" }
    // A thread of its own: the read holds it until the first request; later ones only fill the pipe.
    GlobalScope.launch(newSingleThreadContext("aulod-signals")) {
        val name =
            when (aulo_signals_wait()) {
                SIGINT -> "SIGINT"

                SIGTERM -> "SIGTERM"

                // Only these two are routed here; -1 means the pipe broke, and stopping is the safe answer.
                else -> "a broken signal pipe"
            }
        received.complete(name)
    }
}

actual suspend fun awaitShutdownSignal(): String = received.await()
