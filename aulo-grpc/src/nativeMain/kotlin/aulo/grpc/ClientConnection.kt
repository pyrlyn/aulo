package aulo.grpc

import aulo.grpc.nghttp2.NGHTTP2_CANCEL
import aulo.grpc.nghttp2.NGHTTP2_FLAG_NONE
import aulo.grpc.nghttp2.NGHTTP2_NO_ERROR
import aulo.grpc.nghttp2.NGHTTP2_REFUSED_STREAM
import aulo.grpc.nghttp2.nghttp2_strerror
import aulo.grpc.nghttp2.nghttp2_submit_request2
import aulo.grpc.nghttp2.nghttp2_submit_rst_stream
import io.ktor.network.sockets.Socket
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.convert
import kotlinx.cinterop.toKString
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Job

/** A send on a call that has already ended; the caller turns it into the call's status. */
internal class CallEndedException : Exception("the call has ended")

/**
 * One call on the client side. Requests go out with [send] and [closeSend]; responses come in
 * with [receive]; [status] waits for the server's verdict. A status message from the server is
 * untrusted text.
 */
class ClientCall internal constructor(
    private val connection: ClientConnection,
    private val stream: ClientConnection.Stream,
) {
    /** Sends one request; waits while the server is slow to read. Throws the call's status once it has ended. */
    suspend fun send(message: ByteArray) {
        val ack = CompletableDeferred<Unit>()
        val framed = frame(message)
        cancelling {
            if (!connection.post { connection.enqueue(stream, framed, ack) }) throw GrpcException(stream.status.await())
            try {
                ack.await()
            } catch (e: CallEndedException) {
                throw GrpcException(stream.status.await())
            }
        }
    }

    /** No more requests: the server sees the end of the request stream. */
    fun closeSend() {
        connection.post { connection.closeSend(stream) }
    }

    /** The next response, or null once the server has sent them all; then see [status]. */
    suspend fun receive(): ByteArray? =
        cancelling {
            val message =
                stream.inbound.messages
                    .receiveCatching()
                    .getOrNull()
            if (message != null) connection.post { connection.taken(stream, message.size) }
            message
        }

    /** The call's status, once it has ended. */
    suspend fun status(): Status = cancelling { stream.status.await() }

    /** Ends the call with CANCELLED; the server's handler is cancelled too. */
    fun cancel() {
        connection.post { connection.cancel(stream, Status(StatusCode.CANCELLED, "cancelled by the client")) }
    }

    /** A caller that stops waiting stops the call, so the server does not work for nobody. */
    private suspend fun <T> cancelling(block: suspend () -> T): T =
        try {
            block()
        } catch (e: CancellationException) {
            cancel()
            throw e
        }
}

/** The client side of one HTTP/2 connection. */
@OptIn(ExperimentalForeignApi::class)
internal class ClientConnection(
    socket: Socket,
    limits: GrpcLimits,
    parent: Job,
) : Http2Connection(socket, limits, parent, server = false) {
    internal class Stream(
        maxMessageBytes: Int,
    ) {
        var id = -1
        val inbound = Inbound(maxMessageBytes)
        val outbound = Outbound()
        var sendClosed = false
        val status = CompletableDeferred<Status>()
        val headers = ArrayList<Pair<String, String>>()
        var headerBytes = 0
        var headersTooLarge = false
        var responded = false
    }

    private val streams = HashMap<Int, Stream>()

    override fun outbound(streamId: Int): Outbound? = streams[streamId]?.outbound

    fun start(
        stream: Stream,
        headers: Headers,
    ) {
        if (closing) return complete(stream, CONNECTION_CLOSED)
        val id =
            withProvider { provider ->
                withHeaders(headers) { nva, count -> nghttp2_submit_request2(session, null, nva, count, provider, null) }
            }
        if (id < 0) return complete(stream, Status(StatusCode.UNAVAILABLE, "cannot start the call: ${nghttp2_strerror(id)?.toKString()}"))
        stream.id = id
        streams[id] = stream
    }

    fun enqueue(
        stream: Stream,
        framed: ByteArray,
        ack: CompletableDeferred<Unit>,
    ) {
        when {
            stream.sendClosed -> {
                ack.completeExceptionally(IllegalStateException("send after closeSend"))
            }

            closing || streams[stream.id] !== stream || stream.status.isCompleted -> {
                ack.completeExceptionally(CallEndedException())
            }

            else -> {
                stream.outbound.add(framed, ack)
                resume(stream.id, stream.outbound)
            }
        }
    }

    fun closeSend(stream: Stream) {
        if (stream.sendClosed) return
        stream.sendClosed = true
        if (closing || streams[stream.id] !== stream) return
        stream.outbound.ended = true
        resume(stream.id, stream.outbound)
    }

    fun taken(
        stream: Stream,
        size: Int,
    ) {
        if (!closing && streams[stream.id] === stream) taken(stream.id, stream.inbound, size)
    }

    fun cancel(
        stream: Stream,
        status: Status,
    ) {
        if (stream.status.isCompleted) return
        val open = !closing && streams[stream.id] === stream
        complete(stream, status, resetIfSending = false)
        if (open) nghttp2_submit_rst_stream(session, NGHTTP2_FLAG_NONE.convert(), stream.id, NGHTTP2_CANCEL)
    }

    override fun onBeginHeaders(frame: FrameInfo) {
        val stream = streams[frame.streamId] ?: return
        stream.headers.clear()
        stream.headerBytes = 0
        stream.headersTooLarge = false
    }

    override fun onHeader(
        frame: FrameInfo,
        name: String,
        value: String,
        size: Int,
    ) {
        val stream = streams[frame.streamId] ?: return
        if (stream.headersTooLarge) return
        stream.headerBytes += size
        if (stream.headerBytes > limits.maxHeaderListBytes) {
            stream.headersTooLarge = true
            stream.headers.clear()
            return
        }
        stream.headers += name to value
    }

    override fun onFrameReceived(frame: FrameInfo) {
        val stream = streams[frame.streamId] ?: return
        if (stream.status.isCompleted) return
        if (frame.isHeaders) {
            if (stream.headersTooLarge) {
                return cancel(stream, Status(StatusCode.RESOURCE_EXHAUSTED, "response metadata exceeds ${limits.maxHeaderListBytes} bytes"))
            }
            val headers = stream.headers
            if (!stream.responded) {
                stream.responded = true
                val http = headers.value(":status")
                if (http != "200") return cancel(stream, Status(httpToGrpc(http), "HTTP status $http"))
                // Trailers-only: the status rides on the response headers.
                if (frame.endStream) return complete(stream, statusOf(headers))
                val contentType = headers.value("content-type")
                if (contentType == null || !contentType.startsWith("application/grpc")) {
                    return cancel(stream, Status(StatusCode.UNKNOWN, "not a gRPC response"))
                }
            } else {
                if (stream.inbound.deframer.partial) {
                    return complete(stream, Status(StatusCode.INTERNAL, "the response ended inside a message"))
                }
                complete(stream, statusOf(headers))
            }
        } else if (frame.isData && frame.endStream) {
            complete(stream, Status(StatusCode.INTERNAL, "the response ended without a status"))
        }
    }

    override fun onData(
        streamId: Int,
        data: ByteArray,
    ) {
        val stream = streams[streamId]
        if (stream == null || stream.status.isCompleted) {
            consume(streamId, data.size.toLong())
            return
        }
        try {
            deliver(streamId, stream.inbound, data)
        } catch (e: GrpcException) {
            consume(streamId, data.size.toLong())
            cancel(stream, e.status)
        }
    }

    override fun onStreamClose(
        streamId: Int,
        errorCode: UInt,
    ) {
        val stream = streams.remove(streamId) ?: return
        release(streamId, stream.inbound)
        val status =
            when (errorCode) {
                NGHTTP2_REFUSED_STREAM -> Status(StatusCode.UNAVAILABLE, "the server refused the stream")
                NGHTTP2_CANCEL -> Status(StatusCode.CANCELLED, "the server cancelled the stream")
                NGHTTP2_NO_ERROR -> Status(StatusCode.INTERNAL, "the stream closed without a status")
                else -> Status(StatusCode.INTERNAL, "the stream was reset (HTTP/2 error $errorCode)")
            }
        complete(stream, status)
    }

    override fun onClosed() {
        for (stream in streams.values) complete(stream, CONNECTION_CLOSED)
        streams.clear()
    }

    /** Settles the call; queued responses stay readable, unsent requests are dropped. */
    private fun complete(
        stream: Stream,
        status: Status,
        resetIfSending: Boolean = true,
    ) {
        stream.status.complete(status)
        stream.inbound.messages.close()
        val sending = !stream.outbound.ended
        stream.outbound.fail(CallEndedException())
        // The server has answered while requests were still going out: tell it to expect no more.
        if (resetIfSending && sending && !closing && streams[stream.id] === stream) {
            nghttp2_submit_rst_stream(session, NGHTTP2_FLAG_NONE.convert(), stream.id, NGHTTP2_NO_ERROR)
        }
    }

    private companion object {
        val CONNECTION_CLOSED = Status(StatusCode.UNAVAILABLE, "connection closed")
    }
}

private fun statusOf(headers: Headers): Status {
    val code = headers.value("grpc-status")?.toIntOrNull() ?: return Status(StatusCode.UNKNOWN, "the response carries no grpc-status")
    return Status(StatusCode.of(code), headers.value("grpc-message")?.let(StatusMessage::decode).orEmpty())
}

/** gRPC's mapping for a response that is not HTTP 200 (doc/http-grpc-status-mapping.md). */
private fun httpToGrpc(http: String?): StatusCode =
    when (http) {
        "400" -> StatusCode.INTERNAL
        "401" -> StatusCode.UNAUTHENTICATED
        "403" -> StatusCode.PERMISSION_DENIED
        "404" -> StatusCode.UNIMPLEMENTED
        "429", "502", "503", "504" -> StatusCode.UNAVAILABLE
        else -> StatusCode.UNKNOWN
    }
