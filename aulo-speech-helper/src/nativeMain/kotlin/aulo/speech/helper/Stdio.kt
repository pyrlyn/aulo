package aulo.speech.helper

import okio.Buffer
import okio.IOException
import okio.Sink
import okio.Source
import okio.Timeout

private const val CHUNK = 64 * 1024

/** Up to [max] bytes read from stdin into [buf]: positive, 0 at end of file; throws on an error. */
internal expect fun readStdin(
    buf: ByteArray,
    max: Int,
): Int

/** Writes [count] bytes of [buf] to stdout, all of them; throws on an error. */
internal expect fun writeStdout(
    buf: ByteArray,
    count: Int,
)

/** Makes stdin and stdout byte streams: Windows would otherwise translate line ends in the frames. */
expect fun binaryStdio()

/** stdin as an okio source: the frames from aulod. */
class StdinSource : Source {
    private val chunk = ByteArray(CHUNK)

    override fun read(
        sink: Buffer,
        byteCount: Long,
    ): Long {
        val n = readStdin(chunk, minOf(byteCount, CHUNK.toLong()).toInt())
        if (n == 0) return -1
        sink.write(chunk, 0, n)
        return n.toLong()
    }

    override fun timeout() = Timeout.NONE

    override fun close() {}
}

/** stdout as an okio sink: the replies to aulod. */
class StdoutSink : Sink {
    private val chunk = ByteArray(CHUNK)

    override fun write(
        source: Buffer,
        byteCount: Long,
    ) {
        var left = byteCount
        while (left > 0) {
            val n = source.read(chunk, 0, minOf(left, CHUNK.toLong()).toInt())
            if (n < 0) throw IOException("source ended early")
            writeStdout(chunk, n)
            left -= n
        }
    }

    override fun flush() {}

    override fun timeout() = Timeout.NONE

    override fun close() {}
}
