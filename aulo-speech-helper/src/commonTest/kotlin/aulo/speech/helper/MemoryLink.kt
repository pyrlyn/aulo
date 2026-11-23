package aulo.speech.helper

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ClosedSendChannelException
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import okio.Buffer
import okio.BufferedSink
import okio.BufferedSource
import okio.IOException
import okio.Sink
import okio.Source
import okio.Timeout
import okio.buffer
import kotlin.concurrent.atomics.AtomicReference
import kotlin.concurrent.atomics.ExperimentalAtomicApi
import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TimeSource

/** A blocking in-memory pipe of bounded size, so a test helper can stall the writer like a full OS pipe. */
class MemoryPipe(
    chunks: Int = 64,
) {
    private val channel = Channel<ByteArray>(chunks)

    val sink: Sink =
        object : Sink {
            override fun write(
                source: Buffer,
                byteCount: Long,
            ) {
                val bytes = source.readByteArray(byteCount)
                try {
                    runBlocking { channel.send(bytes) }
                } catch (e: ClosedSendChannelException) {
                    throw IOException("pipe closed")
                } catch (e: kotlinx.coroutines.CancellationException) {
                    throw IOException("pipe closed")
                }
            }

            override fun flush() {}

            override fun timeout() = Timeout.NONE

            override fun close() {
                channel.close()
            }
        }

    val source: Source =
        object : Source {
            private val pending = Buffer()

            override fun read(
                sink: Buffer,
                byteCount: Long,
            ): Long {
                if (pending.size == 0L) {
                    val chunk = runBlocking { channel.receiveCatching() }.getOrNull() ?: return -1
                    pending.write(chunk)
                }
                return pending.read(sink, minOf(byteCount, pending.size))
            }

            override fun timeout() = Timeout.NONE

            override fun close() {}
        }

    /** Like a killed process: what is buffered is lost and both ends see the pipe close. */
    fun kill() {
        channel.cancel()
    }
}

/** A helper "process" running [body] on its own thread, over two memory pipes. */
@OptIn(ExperimentalAtomicApi::class)
class MemoryLink(
    chunks: Int = 64,
    body: (BufferedSource, BufferedSink) -> Unit,
) : HelperLink {
    private val toHelper = MemoryPipe(chunks)
    private val fromHelper = MemoryPipe(chunks)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val exit = AtomicReference<String?>(null)
    override val input = fromHelper.source.buffer()
    override val output = toHelper.sink.buffer()

    init {
        scope.launch {
            val out = fromHelper.sink.buffer()
            val status =
                try {
                    body(toHelper.source.buffer(), out)
                    out.flush()
                    "exit code 0"
                } catch (e: Exception) {
                    "exit code 1 (${e.message})"
                }
            exit.store(status)
            fromHelper.sink.close()
        }
    }

    override fun exitDetail(): String? = exit.load()

    override fun close() {
        toHelper.kill()
        fromHelper.kill()
        scope.cancel()
    }
}

/** Polls until [block] gives something other than null, or fails the test after [timeout]. */
fun <T : Any> eventually(
    timeout: Duration = 5.seconds,
    block: () -> T?,
): T {
    val start = TimeSource.Monotonic.markNow()
    while (true) {
        block()?.let { return it }
        check(start.elapsedNow() < timeout) { "nothing happened in $timeout" }
        runBlocking { delay(1) }
    }
}
