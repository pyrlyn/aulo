package aulo.grpc

import io.ktor.network.sockets.Socket
import io.ktor.network.sockets.openReadChannel
import io.ktor.network.sockets.openWriteChannel
import io.ktor.utils.io.readAvailable
import io.ktor.utils.io.writeFully

/**
 * The bytes HTTP/2 runs over: a socket as it is, or wrapped in TLS. One coroutine reads while
 * another writes and flushes.
 */
internal interface ByteStream {
    /** Up to `buf.size` bytes into [buf]; -1 once the peer has closed. */
    suspend fun read(buf: ByteArray): Int

    /** Queues [bytes]; [flush] sends them. */
    suspend fun write(bytes: ByteArray)

    suspend fun flush()

    /** Closes the socket; call it once neither side is reading or writing. */
    fun close()
}

internal class SocketStream(
    private val socket: Socket,
) : ByteStream {
    private val input = socket.openReadChannel()
    private val output = socket.openWriteChannel(autoFlush = false)

    override suspend fun read(buf: ByteArray): Int = input.readAvailable(buf, 0, buf.size)

    override suspend fun write(bytes: ByteArray) = output.writeFully(bytes, 0, bytes.size)

    override suspend fun flush() = output.flush()

    override fun close() = socket.close()
}
