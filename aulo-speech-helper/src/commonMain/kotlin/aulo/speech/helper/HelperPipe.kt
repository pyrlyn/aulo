@file:OptIn(ExperimentalAtomicApi::class)

package aulo.speech.helper

import aulo.speech.SpeechException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.launch
import okio.BufferedSink
import okio.BufferedSource
import okio.IOException
import kotlin.concurrent.atomics.AtomicBoolean
import kotlin.concurrent.atomics.AtomicReference
import kotlin.concurrent.atomics.ExperimentalAtomicApi

/** Where closed engines stop their helpers, off the caller's thread. */
private val releaser = CoroutineScope(SupervisorJob() + Dispatchers.IO)

/**
 * The aulod end of a helper link that an engine shares between its caller's thread and two I/O
 * threads: a bounded queue of messages a writer sends to the helper, a reader of its replies, and
 * the first failure, which every later call throws. The caller never blocks on the pipe.
 *
 * Anything wrong with the helper (exit, a frame that breaks the protocol, a pipe that stops)
 * fails the engine with a [SpeechException] that falls back: [SpeechException.Unavailable] while
 * [isReady] is false, as the helper never started, and [SpeechException.Failed] after.
 */
internal class HelperPipe<Out : Any>(
    private val link: HelperLink,
    capacity: Int,
    private val isReady: () -> Boolean,
    /** Writes one message; called on the writer's thread only. */
    private val write: (BufferedSink, Out) -> Unit,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val queue = Channel<Out>(capacity)
    private val failure = AtomicReference<SpeechException?>(null)
    private val closed = AtomicBoolean(false)

    /** Starts the writer and [read], which handles replies until the helper's stdout ends. */
    fun start(read: suspend (BufferedSource) -> Unit) {
        scope.launch { readLoop(read) }
        scope.launch { writeLoop() }
    }

    /** Queues [message]; false when the queue is full. */
    fun offer(message: Out): Boolean = queue.trySend(message).isSuccess

    /** Queues [message]; a full queue means the helper stopped reading, which fails the engine. */
    fun send(message: Out) {
        if (!offer(message)) {
            fail(SpeechException.Failed("helper is not reading its input"))
            close()
            checkHealthy()
        }
    }

    /** Throws the engine's failure, if any. */
    fun checkHealthy() {
        failure.load()?.let { throw it }
        if (closed.load()) throw SpeechException.Unavailable("engine is closed")
    }

    /** Records [error] unless the engine already failed; the first cause is the one reported. */
    fun fail(error: SpeechException) {
        failure.compareAndSet(null, error)
    }

    /** Stops both loops and the helper. Never blocks. */
    fun close() {
        if (!closed.compareAndSet(expectedValue = false, newValue = true)) return
        queue.close()
        scope.cancel()
        // Stopping a process waits for it to be reaped, and close may come from poll, which never waits.
        releaser.launch { link.close() }
    }

    fun exited(how: String): SpeechException {
        val detail = "helper $how" + (link.exitDetail()?.let { ": $it" } ?: "")
        return if (isReady()) SpeechException.Failed(detail) else SpeechException.Unavailable(detail)
    }

    private suspend fun readLoop(read: suspend (BufferedSource) -> Unit) {
        try {
            read(link.input)
            fail(exited("exited"))
        } catch (e: WireException) {
            fail(SpeechException.Failed("helper broke the protocol: ${e.message}"))
        } catch (e: IOException) {
            fail(exited("pipe failed (${e.message})"))
        }
        // The helper is gone or untrusted from here on; stop it and free its pipes.
        close()
    }

    private suspend fun writeLoop() {
        try {
            for (first in queue) {
                // Write what is queued, then flush once: one pipe write per burst, not per message.
                var message: Out? = first
                while (message != null) {
                    write(link.output, message)
                    message = queue.tryReceive().getOrNull()
                }
                link.output.flush()
            }
        } catch (e: IOException) {
            fail(exited("stopped reading (${e.message})"))
            close()
        }
    }
}
