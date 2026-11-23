package aulo.speech.helper

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import okio.Buffer
import okio.BufferedSink
import okio.BufferedSource
import okio.IOException
import okio.Sink
import okio.Source
import okio.Timeout
import okio.buffer
import process.ChildOutput
import process.Command
import process.PipedChild
import process.ProcessException
import process.spawnPipes
import kotlin.time.Duration.Companion.milliseconds

private const val PIPE_CHUNK = 64 * 1024

/** Between a helper closing its stdout and the system reporting how it ended. */
private val EXIT_GRACE = 500.milliseconds

/** A helper running as a child process, spoken to over its stdin and stdout (pyrlyn-kit `spawnPipes`). */
class ProcessLink private constructor(
    private val child: PipedChild,
) : HelperLink {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    init {
        // stderr is not part of the protocol and its text is untrusted, so it is read only to keep a chatty helper
        // (onnxruntime warns there) from filling the pipe and stalling.
        scope.launch { drain(child.stderr) }
    }

    override val input: BufferedSource = StdoutSource(child.stdout).buffer()

    override val output: BufferedSink = StdinSink(child).buffer()

    override fun exitDetail(): String? = child.waitFor(EXIT_GRACE)?.toString()

    override fun close() {
        child.close()
        scope.cancel()
    }

    companion object {
        /** Starts [command]; a helper that cannot start is unavailable, so the registry falls back. */
        fun spawn(command: Command): ProcessLink =
            try {
                ProcessLink(command.spawnPipes())
            } catch (e: ProcessException) {
                throw aulo.speech.SpeechException.Unavailable(e.detail)
            }
    }
}

private fun drain(stream: ChildOutput) {
    val buf = ByteArray(PIPE_CHUNK)
    do {
        val n = stream.read(buf)
    } while (n >= 0)
}

private class StdoutSource(
    private val stdout: ChildOutput,
) : Source {
    private val buf = ByteArray(PIPE_CHUNK)

    override fun read(
        sink: Buffer,
        byteCount: Long,
    ): Long {
        val n = stdout.read(buf, 0, minOf(byteCount, buf.size.toLong()).toInt())
        if (n < 0) return -1
        sink.write(buf, 0, n)
        return n.toLong()
    }

    override fun timeout() = Timeout.NONE

    override fun close() = stdout.close()
}

private class StdinSink(
    private val child: PipedChild,
) : Sink {
    private val buf = ByteArray(PIPE_CHUNK)

    override fun write(
        source: Buffer,
        byteCount: Long,
    ) {
        var left = byteCount
        while (left > 0) {
            val n = source.read(buf, 0, minOf(left, buf.size.toLong()).toInt())
            try {
                child.stdin.write(buf, 0, n)
            } catch (e: ProcessException) {
                throw IOException(e.detail, e)
            }
            left -= n
        }
    }

    override fun flush() = Unit

    override fun timeout() = Timeout.NONE

    override fun close() = child.stdin.close()
}
