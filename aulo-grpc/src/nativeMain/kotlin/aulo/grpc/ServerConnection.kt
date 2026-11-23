package aulo.grpc

import aulo.grpc.nghttp2.NGHTTP2_FLAG_NONE
import aulo.grpc.nghttp2.NGHTTP2_HCAT_REQUEST
import aulo.grpc.nghttp2.NGHTTP2_NO_ERROR
import aulo.grpc.nghttp2.nghttp2_submit_response2
import aulo.grpc.nghttp2.nghttp2_submit_rst_stream
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.convert
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Job
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeout
import kotlin.time.Duration

internal class StreamServerCall(
    override val path: String,
    override val endpoint: Endpoint,
    override val metadata: List<Pair<String, String>>,
    override val timeout: Duration?,
    private val connection: ServerConnection,
    private val stream: ServerConnection.Stream,
) : ServerCall {
    override suspend fun receive(): ByteArray? {
        val message =
            stream.inbound.messages
                .receiveCatching()
                .getOrNull() ?: return null
        connection.post { connection.taken(stream, message.size) }
        return message
    }

    override suspend fun send(message: ByteArray) {
        val ack = CompletableDeferred<Unit>()
        val framed = frame(message)
        if (!connection.post { connection.enqueue(stream, framed, ack) }) throw CancellationException("connection closed")
        ack.await()
    }
}

/** The server side of one HTTP/2 connection: one handler coroutine per request stream. */
@OptIn(ExperimentalForeignApi::class)
internal class ServerConnection(
    stream: ByteStream,
    private val endpoint: Endpoint,
    limits: GrpcLimits,
    parent: Job,
    private val handlers: Map<String, ServerHandler>,
) : Http2Connection(stream, limits, parent, server = true) {
    internal class Stream(
        val id: Int,
        maxMessageBytes: Int,
    ) {
        val headers = ArrayList<Pair<String, String>>()
        var headerBytes = 0
        var headersTooLarge = false
        val inbound = Inbound(maxMessageBytes)
        var started = false
        var requestEnded = false
        val outbound = Outbound()
        var responded = false
        var finished = false
        var job: Job? = null
    }

    private val streams = HashMap<Int, Stream>()

    override fun outbound(streamId: Int): Outbound? = streams[streamId]?.outbound

    override fun onBeginHeaders(frame: FrameInfo) {
        if (frame.isHeaders && frame.category == NGHTTP2_HCAT_REQUEST.toInt()) {
            streams[frame.streamId] = Stream(frame.streamId, limits.maxMessageBytes)
        }
    }

    override fun onHeader(
        frame: FrameInfo,
        name: String,
        value: String,
        size: Int,
    ) {
        // Request trailers carry nothing gRPC uses; only the request headers are kept.
        if (frame.category != NGHTTP2_HCAT_REQUEST.toInt()) return
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
        if (frame.isHeaders && frame.category == NGHTTP2_HCAT_REQUEST.toInt()) start(stream)
        if ((frame.isHeaders || frame.isData) && frame.endStream) endRequests(stream)
    }

    override fun onFrameSent(frame: FrameInfo) {
        if (!frame.endStream) return
        val stream = streams[frame.streamId] ?: return
        // The response is complete but the client is still sending: ask it to stop, without an
        // error (RFC 9113 §8.1). Only now, so the RST_STREAM cannot overtake the response.
        if (!stream.requestEnded) nghttp2_submit_rst_stream(session, NGHTTP2_FLAG_NONE.convert(), stream.id, NGHTTP2_NO_ERROR)
    }

    override fun onData(
        streamId: Int,
        data: ByteArray,
    ) {
        val stream = streams[streamId]
        if (stream == null || !stream.started || stream.finished) {
            consume(streamId, data.size.toLong())
            return
        }
        try {
            deliver(streamId, stream.inbound, data)
        } catch (e: GrpcException) {
            consume(streamId, data.size.toLong())
            finish(stream, e.status)
        }
    }

    override fun onStreamClose(
        streamId: Int,
        errorCode: UInt,
    ) {
        val stream = streams.remove(streamId) ?: return
        release(streamId, stream.inbound)
        stream.outbound.fail(CancellationException("stream closed"))
        stream.finished = true
        stream.job?.cancel(CancellationException("the client closed the stream"))
    }

    override fun onClosed() {
        for (stream in streams.values) {
            stream.inbound.messages.close()
            stream.outbound.fail(CancellationException("connection closed"))
            stream.finished = true
            stream.job?.cancel(CancellationException("connection closed"))
        }
        streams.clear()
    }

    internal fun taken(
        stream: Stream,
        size: Int,
    ) {
        if (!closing && streams[stream.id] === stream) taken(stream.id, stream.inbound, size)
    }

    internal fun enqueue(
        stream: Stream,
        framed: ByteArray,
        ack: CompletableDeferred<Unit>,
    ) {
        if (closing || stream.finished || streams[stream.id] !== stream) {
            ack.completeExceptionally(CancellationException("the call has ended"))
            return
        }
        if (!stream.responded) {
            stream.responded = true
            val rv =
                withProvider { provider ->
                    withHeaders(RESPONSE_HEADERS) { nva, count -> nghttp2_submit_response2(session, stream.id, nva, count, provider) }
                }
            if (rv != 0) {
                ack.completeExceptionally(CancellationException("nghttp2 refused the response"))
                return
            }
        }
        stream.outbound.add(framed, ack)
        resume(stream.id, stream.outbound)
    }

    internal fun finish(
        stream: Stream,
        status: Status,
    ) {
        if (closing || stream.finished || streams[stream.id] !== stream) return
        stream.finished = true
        stream.job?.cancel(CancellationException("the call has ended"))
        val trailers = statusHeaders(status)
        if (stream.responded) {
            stream.outbound.ended = true
            stream.outbound.trailers = trailers
            resume(stream.id, stream.outbound)
        } else {
            // Trailers-only: one HEADERS frame that ends the stream.
            stream.responded = true
            withHeaders(RESPONSE_HEADERS + trailers) { nva, count -> nghttp2_submit_response2(session, stream.id, nva, count, null) }
        }
    }

    private fun start(stream: Stream) {
        if (stream.started) return
        stream.started = true
        if (stream.headersTooLarge) {
            return finish(stream, Status(StatusCode.RESOURCE_EXHAUSTED, "request metadata exceeds ${limits.maxHeaderListBytes} bytes"))
        }
        val headers = stream.headers
        val contentType = headers.value("content-type")
        if (headers.value(":method") != "POST" || contentType == null || !contentType.startsWith("application/grpc")) {
            return finish(stream, Status(StatusCode.INTERNAL, "not a gRPC request"))
        }
        val path = headers.value(":path").orEmpty()
        val handler = handlers[path] ?: return finish(stream, Status(StatusCode.UNIMPLEMENTED, "unknown method $path"))
        val timeout =
            headers.value("grpc-timeout")?.let {
                GrpcTimeout.parse(it) ?: return finish(stream, Status(StatusCode.INTERNAL, "malformed grpc-timeout"))
            }
        val metadata = headers.filter { (name, _) -> !name.startsWith(":") && name !in RESERVED_HEADERS }
        stream.headers.clear()
        val call = StreamServerCall(path, endpoint, metadata, timeout, this, stream)
        stream.job = scope.launch { serve(stream, handler, call, timeout) }
    }

    private fun endRequests(stream: Stream) {
        stream.requestEnded = true
        if (stream.inbound.deframer.partial && !stream.finished) {
            return finish(stream, Status(StatusCode.INTERNAL, "the request ended inside a message"))
        }
        stream.inbound.messages.close()
    }

    private suspend fun serve(
        stream: Stream,
        handler: ServerHandler,
        call: ServerCall,
        timeout: Duration?,
    ) {
        val status =
            try {
                if (timeout != null) withTimeout(timeout) { handler.handle(call) } else handler.handle(call)
                Status.OK
            } catch (e: TimeoutCancellationException) {
                Status(StatusCode.DEADLINE_EXCEEDED, "deadline exceeded")
            } catch (e: CancellationException) {
                throw e
            } catch (e: GrpcException) {
                e.status
            } catch (e: Exception) {
                Status(StatusCode.UNKNOWN)
            }
        post { finish(stream, status) }
    }

    private companion object {
        val RESPONSE_HEADERS: Headers = listOf(":status" to "200", "content-type" to "application/grpc")
    }
}

internal fun statusHeaders(status: Status): Headers =
    buildList {
        add("grpc-status" to status.code.value.toString())
        if (status.message.isNotEmpty()) add("grpc-message" to StatusMessage.encode(status.message))
    }

internal fun Headers.value(name: String): String? = firstOrNull { it.first == name }?.second
