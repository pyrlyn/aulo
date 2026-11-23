package aulo.grpc

import aulo.grpc.nghttp2.NGHTTP2_DATA
import aulo.grpc.nghttp2.NGHTTP2_DATA_FLAG_EOF
import aulo.grpc.nghttp2.NGHTTP2_DATA_FLAG_NO_END_STREAM
import aulo.grpc.nghttp2.NGHTTP2_ERR_CALLBACK_FAILURE
import aulo.grpc.nghttp2.NGHTTP2_ERR_DEFERRED
import aulo.grpc.nghttp2.NGHTTP2_ERR_TEMPORAL_CALLBACK_FAILURE
import aulo.grpc.nghttp2.NGHTTP2_FLAG_END_STREAM
import aulo.grpc.nghttp2.NGHTTP2_FLAG_NONE
import aulo.grpc.nghttp2.NGHTTP2_HEADERS
import aulo.grpc.nghttp2.NGHTTP2_NO_ERROR
import aulo.grpc.nghttp2.NGHTTP2_NV_FLAG_NONE
import aulo.grpc.nghttp2.NGHTTP2_SETTINGS_INITIAL_WINDOW_SIZE
import aulo.grpc.nghttp2.NGHTTP2_SETTINGS_MAX_CONCURRENT_STREAMS
import aulo.grpc.nghttp2.NGHTTP2_SETTINGS_MAX_HEADER_LIST_SIZE
import aulo.grpc.nghttp2.nghttp2_data_provider2
import aulo.grpc.nghttp2.nghttp2_data_source
import aulo.grpc.nghttp2.nghttp2_frame
import aulo.grpc.nghttp2.nghttp2_nv
import aulo.grpc.nghttp2.nghttp2_option_del
import aulo.grpc.nghttp2.nghttp2_option_new
import aulo.grpc.nghttp2.nghttp2_option_set_no_auto_window_update
import aulo.grpc.nghttp2.nghttp2_session_callbacks_del
import aulo.grpc.nghttp2.nghttp2_session_callbacks_new
import aulo.grpc.nghttp2.nghttp2_session_callbacks_set_on_begin_headers_callback
import aulo.grpc.nghttp2.nghttp2_session_callbacks_set_on_data_chunk_recv_callback
import aulo.grpc.nghttp2.nghttp2_session_callbacks_set_on_frame_recv_callback
import aulo.grpc.nghttp2.nghttp2_session_callbacks_set_on_frame_send_callback
import aulo.grpc.nghttp2.nghttp2_session_callbacks_set_on_header_callback
import aulo.grpc.nghttp2.nghttp2_session_callbacks_set_on_stream_close_callback
import aulo.grpc.nghttp2.nghttp2_session_client_new2
import aulo.grpc.nghttp2.nghttp2_session_consume
import aulo.grpc.nghttp2.nghttp2_session_del
import aulo.grpc.nghttp2.nghttp2_session_get_last_proc_stream_id
import aulo.grpc.nghttp2.nghttp2_session_mem_recv2
import aulo.grpc.nghttp2.nghttp2_session_mem_send2
import aulo.grpc.nghttp2.nghttp2_session_resume_data
import aulo.grpc.nghttp2.nghttp2_session_server_new2
import aulo.grpc.nghttp2.nghttp2_session_set_local_window_size
import aulo.grpc.nghttp2.nghttp2_session_want_read
import aulo.grpc.nghttp2.nghttp2_session_want_write
import aulo.grpc.nghttp2.nghttp2_settings_entry
import aulo.grpc.nghttp2.nghttp2_strerror
import aulo.grpc.nghttp2.nghttp2_submit_goaway
import aulo.grpc.nghttp2.nghttp2_submit_settings
import aulo.grpc.nghttp2.nghttp2_submit_trailer
import cnames.structs.nghttp2_option
import cnames.structs.nghttp2_session
import cnames.structs.nghttp2_session_callbacks
import kotlinx.cinterop.ByteVar
import kotlinx.cinterop.COpaquePointer
import kotlinx.cinterop.CPointer
import kotlinx.cinterop.CPointerVar
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.MemScope
import kotlinx.cinterop.StableRef
import kotlinx.cinterop.UByteVar
import kotlinx.cinterop.UIntVar
import kotlinx.cinterop.addressOf
import kotlinx.cinterop.alloc
import kotlinx.cinterop.allocArray
import kotlinx.cinterop.asStableRef
import kotlinx.cinterop.convert
import kotlinx.cinterop.get
import kotlinx.cinterop.memScoped
import kotlinx.cinterop.plus
import kotlinx.cinterop.pointed
import kotlinx.cinterop.ptr
import kotlinx.cinterop.readBytes
import kotlinx.cinterop.reinterpret
import kotlinx.cinterop.set
import kotlinx.cinterop.staticCFunction
import kotlinx.cinterop.toKString
import kotlinx.cinterop.usePinned
import kotlinx.cinterop.value
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.SendChannel
import kotlinx.coroutines.launch
import kotlinx.coroutines.selects.select
import kotlinx.coroutines.withContext
import platform.posix.memcpy

/** A header list in nghttp2's terms: lowercase names, in order, repeats allowed. */
internal typealias Headers = List<Pair<String, String>>

/** Headers the transport sets and reads itself; neither side passes them as metadata. */
internal val RESERVED_HEADERS = setOf("content-type", "te", "grpc-timeout", "grpc-encoding", "grpc-accept-encoding")

/** Unconsumed bytes a stream may hold before its peer must wait (SETTINGS_INITIAL_WINDOW_SIZE). */
internal const val STREAM_WINDOW = 1 shl 20

/** The connection's share: enough for many busy streams before one slow reader stalls the rest. */
private const val CONNECTION_WINDOW = 16 shl 20

/** Buffered response bytes per stream before a sender waits for the peer to read. */
private const val SEND_HIGH_WATER = 64 * 1024

internal const val READ_CHUNK = 16 * 1024

/** Reads in flight between the socket and the loop; the socket waits beyond that. */
private const val READ_AHEAD = 4

/** HTTP/2 frame fields the callbacks need, copied out of nghttp2's frame. */
@OptIn(ExperimentalForeignApi::class)
internal class FrameInfo(
    val type: Int,
    val flags: Int,
    val streamId: Int,
    /** The header category (request, response, trailers) of a HEADERS frame; -1 otherwise. */
    val category: Int,
) {
    val isHeaders: Boolean get() = type == NGHTTP2_HEADERS.toInt()
    val isData: Boolean get() = type == NGHTTP2_DATA.toInt()
    val endStream: Boolean get() = flags and NGHTTP2_FLAG_END_STREAM.toInt() != 0
}

/** Messages a stream has received and the flow-control debt behind them. */
internal class Inbound(
    maxMessageBytes: Int,
) {
    val deframer = Deframer(maxMessageBytes)

    // Unlimited in type only: the peer can never have more than STREAM_WINDOW bytes plus one
    // partial message unconsumed, because consume() waits for the reader (see deliver()).
    val messages = Channel<ByteArray>(Channel.UNLIMITED)
    var queued = 0L
    var owed = 0L
}

/** Messages waiting for a stream's data provider, and the senders waiting for room. */
internal class Outbound {
    private val queue = ArrayDeque<ByteArray>()
    private var offset = 0
    private var bytes = 0L
    private val waiters = ArrayDeque<CompletableDeferred<Unit>>()

    /** No more messages will be added. */
    var ended = false

    /** Sent after the last message; null ends the stream with the last DATA frame. */
    var trailers: Headers? = null

    /** nghttp2 was told there is nothing to send and waits for resume_data. */
    var deferred = false

    val isEmpty: Boolean get() = queue.isEmpty()

    fun add(
        chunk: ByteArray,
        ack: CompletableDeferred<Unit>,
    ) {
        queue.addLast(chunk)
        bytes += chunk.size
        if (bytes <= SEND_HIGH_WATER) ack.complete(Unit) else waiters.addLast(ack)
    }

    @OptIn(ExperimentalForeignApi::class)
    fun copyTo(
        buf: CPointer<UByteVar>,
        length: Int,
    ): Int {
        val dst = buf.reinterpret<ByteVar>()
        var written = 0
        while (written < length && queue.isNotEmpty()) {
            val head = queue.first()
            val n = minOf(length - written, head.size - offset)
            head.usePinned { memcpy(dst + written, it.addressOf(offset), n.convert()) }
            written += n
            offset += n
            if (offset == head.size) {
                queue.removeFirst()
                offset = 0
            }
        }
        bytes -= written
        while (waiters.isNotEmpty() && bytes <= SEND_HIGH_WATER) waiters.removeFirst().complete(Unit)
        return written
    }

    fun fail(cause: Throwable) {
        queue.clear()
        bytes = 0
        ended = true
        while (waiters.isNotEmpty()) waiters.removeFirst().completeExceptionally(cause)
    }
}

/**
 * One HTTP/2 connection over nghttp2. A single loop coroutine owns the session: it feeds
 * [stream] bytes in, runs the commands other coroutines [post], and writes what nghttp2 produces. nghttp2
 * is never touched outside that loop, so it needs no lock.
 */
@OptIn(ExperimentalForeignApi::class)
internal abstract class Http2Connection(
    private val stream: ByteStream,
    protected val limits: GrpcLimits,
    parent: Job?,
    server: Boolean,
) {
    /** The calls' coroutines; cancelled when the connection ends. */
    protected val scope = CoroutineScope(SupervisorJob(parent) + Dispatchers.Default)

    // Unlimited in type only: every poster waits for its command's result before posting another
    // one for the same call, so the queue holds at most a few commands per open stream.
    private val commands = Channel<() -> Unit>(Channel.UNLIMITED)
    private val ref = StableRef.create(this)
    protected val session: CPointer<nghttp2_session> = newSession(server, ref.asCPointer())

    /** Set once the loop has ended; commands still queued then must fail, not touch nghttp2. */
    protected var closing = false
        private set

    /** Runs [action] on the loop; false once the connection is gone. */
    fun post(action: () -> Unit): Boolean = commands.trySend(action).isSuccess

    /** Refuses new streams and ends the connection once the open ones finish. */
    fun goAway(): Boolean =
        post {
            if (!closing) {
                nghttp2_submit_goaway(
                    session,
                    NGHTTP2_FLAG_NONE.convert(),
                    nghttp2_session_get_last_proc_stream_id(session),
                    NGHTTP2_NO_ERROR,
                    null,
                    0u,
                )
            }
        }

    /** Serves the connection until either side ends it. */
    suspend fun run() {
        val incoming = Channel<ByteArray>(READ_AHEAD)
        val reader = scope.launch { readLoop(incoming) }
        try {
            start()
            flush()
            while (wantsIo()) {
                val ok =
                    select {
                        incoming.onReceiveCatching { r -> r.getOrNull()?.let { receive(it) } ?: false }
                        commands.onReceive { command ->
                            command()
                            true
                        }
                    }
                // A protocol error still leaves a GOAWAY for the peer; send it if the socket lets us.
                if (!ok) {
                    runCatching { flush() }
                    break
                }
                flush()
            }
        } finally {
            closing = true
            commands.close()
            onClosed()
            while (true) {
                val command = commands.tryReceive().getOrNull() ?: break
                runCatching { command() }
            }
            scope.cancel()
            // A TLS stream frees its state on close, so the reader must be out of it first.
            withContext(NonCancellable) { reader.join() }
            stream.close()
            nghttp2_session_del(session)
            ref.dispose()
        }
    }

    // Hooks, called on the loop from nghttp2's callbacks.

    open fun onBeginHeaders(frame: FrameInfo) {}

    /** [size] is the field's RFC 9113 §6.5.2 size: name and value bytes plus 32. */
    abstract fun onHeader(
        frame: FrameInfo,
        name: String,
        value: String,
        size: Int,
    )

    abstract fun onFrameReceived(frame: FrameInfo)

    open fun onFrameSent(frame: FrameInfo) {}

    abstract fun onData(
        streamId: Int,
        data: ByteArray,
    )

    abstract fun onStreamClose(
        streamId: Int,
        errorCode: UInt,
    )

    abstract fun outbound(streamId: Int): Outbound?

    /** The connection is gone: fail every call that is still open. */
    abstract fun onClosed()

    // Helpers for the hooks; loop only.

    /** Hands [data] to the stream's deframer; window is returned only while the reader keeps up. */
    protected fun deliver(
        streamId: Int,
        inbound: Inbound,
        data: ByteArray,
    ) {
        for (message in inbound.deframer.feed(data)) {
            inbound.queued += message.size
            inbound.messages.trySend(message)
        }
        if (inbound.queued <= STREAM_WINDOW) consume(streamId, data.size.toLong()) else inbound.owed += data.size
    }

    /** The reader took a message of [size] bytes. */
    protected fun taken(
        streamId: Int,
        inbound: Inbound,
        size: Int,
    ) {
        inbound.queued -= size
        if (inbound.owed > 0 && inbound.queued <= STREAM_WINDOW) {
            consume(streamId, inbound.owed)
            inbound.owed = 0
        }
    }

    /** Returns a closed stream's unread window to the connection; nghttp2 leaves that to us. */
    protected fun release(
        streamId: Int,
        inbound: Inbound,
    ) {
        inbound.messages.close()
        if (inbound.owed > 0) consume(streamId, inbound.owed)
        inbound.owed = 0
    }

    protected fun consume(
        streamId: Int,
        size: Long,
    ) {
        if (size > 0) nghttp2_session_consume(session, streamId, size.convert())
    }

    protected fun resume(
        streamId: Int,
        out: Outbound,
    ) {
        if (out.deferred) {
            out.deferred = false
            nghttp2_session_resume_data(session, streamId)
        }
    }

    protected fun <T> withHeaders(
        headers: Headers,
        block: (CPointer<nghttp2_nv>, ULong) -> T,
    ): T =
        memScoped {
            val nva = allocArray<nghttp2_nv>(headers.size.coerceAtLeast(1))
            headers.forEachIndexed { i, (name, value) ->
                val n = name.encodeToByteArray()
                val v = value.encodeToByteArray()
                nva[i].name = bytes(n)
                nva[i].namelen = n.size.convert()
                nva[i].value = bytes(v)
                nva[i].valuelen = v.size.convert()
                nva[i].flags = NGHTTP2_NV_FLAG_NONE.convert()
            }
            block(nva, headers.size.convert())
        }

    /** A data provider that reads from [outbound]; nghttp2 copies it, so it lives only for [block]. */
    protected fun <T> withProvider(block: (CPointer<nghttp2_data_provider2>) -> T): T =
        memScoped {
            val provider = alloc<nghttp2_data_provider2>()
            provider.source.ptr = null
            provider.read_callback = readCallback
            block(provider.ptr)
        }

    internal fun read(
        streamId: Int,
        buf: CPointer<UByteVar>,
        length: Int,
        flags: CPointer<UIntVar>,
    ): Long {
        val out = outbound(streamId) ?: return NGHTTP2_ERR_TEMPORAL_CALLBACK_FAILURE.toLong()
        val n = out.copyTo(buf, length)
        if (out.isEmpty && out.ended) {
            var f = NGHTTP2_DATA_FLAG_EOF
            val trailers = out.trailers
            if (trailers != null) {
                f = f or NGHTTP2_DATA_FLAG_NO_END_STREAM
                val rv = withHeaders(trailers) { nva, count -> nghttp2_submit_trailer(session, streamId, nva, count) }
                if (rv != 0) return NGHTTP2_ERR_TEMPORAL_CALLBACK_FAILURE.toLong()
            }
            flags.pointed.value = flags.pointed.value or f
            return n.toLong()
        }
        if (n == 0) {
            out.deferred = true
            return NGHTTP2_ERR_DEFERRED.toLong()
        }
        return n.toLong()
    }

    private fun start() {
        memScoped {
            val iv = allocArray<nghttp2_settings_entry>(3)
            iv[0].settings_id = NGHTTP2_SETTINGS_MAX_CONCURRENT_STREAMS.convert()
            iv[0].value = limits.maxConcurrentStreams.convert()
            iv[1].settings_id = NGHTTP2_SETTINGS_INITIAL_WINDOW_SIZE.convert()
            iv[1].value = STREAM_WINDOW.convert()
            iv[2].settings_id = NGHTTP2_SETTINGS_MAX_HEADER_LIST_SIZE.convert()
            iv[2].value = limits.maxHeaderListBytes.convert()
            check(nghttp2_submit_settings(session, NGHTTP2_FLAG_NONE.convert(), iv, 3u) == 0) { "nghttp2 refused the SETTINGS" }
        }
        nghttp2_session_set_local_window_size(session, NGHTTP2_FLAG_NONE.convert(), 0, CONNECTION_WINDOW)
    }

    private fun wantsIo(): Boolean = nghttp2_session_want_read(session) != 0 || nghttp2_session_want_write(session) != 0

    /** False when nghttp2 found the peer's bytes fatal; it has queued a GOAWAY saying why. */
    private fun receive(bytes: ByteArray): Boolean {
        val rv = bytes.usePinned { nghttp2_session_mem_recv2(session, it.addressOf(0).reinterpret(), bytes.size.convert()) }
        return rv >= 0
    }

    private suspend fun flush() {
        while (true) {
            val chunk =
                memScoped {
                    val data = alloc<CPointerVar<UByteVar>>()
                    val n = nghttp2_session_mem_send2(session, data.ptr)
                    check(n >= 0) { "nghttp2: ${nghttp2_strerror(n.toInt())?.toKString()}" }
                    if (n == 0L) null else checkNotNull(data.value).reinterpret<ByteVar>().readBytes(n.toInt())
                } ?: break
            stream.write(chunk)
        }
        stream.flush()
    }

    private suspend fun readLoop(incoming: SendChannel<ByteArray>) {
        val buf = ByteArray(READ_CHUNK)
        try {
            while (true) {
                val n = stream.read(buf)
                if (n < 0) break
                if (n > 0) incoming.send(buf.copyOf(n))
            }
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            // A reset socket or a TLS alert ends the connection the same way a clean close does.
        } finally {
            incoming.close()
        }
    }
}

@OptIn(ExperimentalForeignApi::class)
private fun newSession(
    server: Boolean,
    userData: COpaquePointer,
): CPointer<nghttp2_session> =
    memScoped {
        val cbsVar = alloc<CPointerVar<nghttp2_session_callbacks>>()
        check(nghttp2_session_callbacks_new(cbsVar.ptr) == 0) { "nghttp2: out of memory" }
        val cbs = cbsVar.value
        val optVar = alloc<CPointerVar<nghttp2_option>>()
        check(nghttp2_option_new(optVar.ptr) == 0) { "nghttp2: out of memory" }
        val opt = optVar.value
        try {
            nghttp2_session_callbacks_set_on_begin_headers_callback(cbs, onBeginHeadersCallback)
            nghttp2_session_callbacks_set_on_header_callback(cbs, onHeaderCallback)
            nghttp2_session_callbacks_set_on_frame_recv_callback(cbs, onFrameRecvCallback)
            nghttp2_session_callbacks_set_on_frame_send_callback(cbs, onFrameSendCallback)
            nghttp2_session_callbacks_set_on_data_chunk_recv_callback(cbs, onDataChunkCallback)
            nghttp2_session_callbacks_set_on_stream_close_callback(cbs, onStreamCloseCallback)
            // Window comes back only as the application reads (Http2Connection.deliver), so a
            // slow reader slows its peer instead of growing a buffer.
            nghttp2_option_set_no_auto_window_update(opt, 1)
            val sessionVar = alloc<CPointerVar<nghttp2_session>>()
            val rv =
                if (server) {
                    nghttp2_session_server_new2(sessionVar.ptr, cbs, userData, opt)
                } else {
                    nghttp2_session_client_new2(sessionVar.ptr, cbs, userData, opt)
                }
            check(rv == 0) { "nghttp2: ${nghttp2_strerror(rv)?.toKString()}" }
            checkNotNull(sessionVar.value)
        } finally {
            nghttp2_option_del(opt)
            nghttp2_session_callbacks_del(cbs)
        }
    }

@OptIn(ExperimentalForeignApi::class)
private fun MemScope.bytes(b: ByteArray): CPointer<UByteVar> {
    val p = allocArray<UByteVar>(b.size.coerceAtLeast(1))
    for (i in b.indices) p[i] = b[i].toUByte()
    return p
}

@OptIn(ExperimentalForeignApi::class)
private fun CPointer<nghttp2_frame>.info(): FrameInfo {
    val f = pointed
    val type = f.hd.type.toInt()
    val category = if (type == NGHTTP2_HEADERS.toInt()) f.headers.cat.toInt() else -1
    return FrameInfo(type, f.hd.flags.toInt(), f.hd.stream_id, category)
}

@OptIn(ExperimentalForeignApi::class)
private fun CPointer<UByteVar>?.string(length: ULong): String =
    if (this == null || length == 0uL) "" else reinterpret<ByteVar>().readBytes(length.toInt()).decodeToString()

/**
 * Runs a hook for nghttp2. An exception must not unwind through C frames, so it becomes
 * NGHTTP2_ERR_CALLBACK_FAILURE, which ends the session.
 */
@OptIn(ExperimentalForeignApi::class)
private inline fun hook(
    userData: COpaquePointer?,
    block: Http2Connection.() -> Unit,
): Int =
    try {
        checkNotNull(userData).asStableRef<Http2Connection>().get().block()
        0
    } catch (e: Throwable) {
        NGHTTP2_ERR_CALLBACK_FAILURE
    }

@OptIn(ExperimentalForeignApi::class)
private val onBeginHeadersCallback =
    staticCFunction { _: CPointer<nghttp2_session>?, frame: CPointer<nghttp2_frame>?, user: COpaquePointer? ->
        hook(user) { onBeginHeaders(checkNotNull(frame).info()) }
    }

@OptIn(ExperimentalForeignApi::class)
private val onHeaderCallback =
    staticCFunction {
        _: CPointer<nghttp2_session>?,
        frame: CPointer<nghttp2_frame>?,
        name: CPointer<UByteVar>?,
        nameLength: ULong,
        value: CPointer<UByteVar>?,
        valueLength: ULong,
        _: UByte,
        user: COpaquePointer?,
        ->
        hook(user) {
            val size = (nameLength + valueLength).toLong() + 32
            onHeader(
                checkNotNull(frame).info(),
                name.string(nameLength),
                value.string(valueLength),
                size.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
            )
        }
    }

@OptIn(ExperimentalForeignApi::class)
private val onFrameRecvCallback =
    staticCFunction { _: CPointer<nghttp2_session>?, frame: CPointer<nghttp2_frame>?, user: COpaquePointer? ->
        hook(user) { onFrameReceived(checkNotNull(frame).info()) }
    }

@OptIn(ExperimentalForeignApi::class)
private val onFrameSendCallback =
    staticCFunction { _: CPointer<nghttp2_session>?, frame: CPointer<nghttp2_frame>?, user: COpaquePointer? ->
        hook(user) { onFrameSent(checkNotNull(frame).info()) }
    }

@OptIn(ExperimentalForeignApi::class)
private val onDataChunkCallback =
    staticCFunction {
        _: CPointer<nghttp2_session>?,
        _: UByte,
        streamId: Int,
        data: CPointer<UByteVar>?,
        length: ULong,
        user: COpaquePointer?,
        ->
        hook(user) { onData(streamId, checkNotNull(data).reinterpret<ByteVar>().readBytes(length.toInt())) }
    }

@OptIn(ExperimentalForeignApi::class)
private val onStreamCloseCallback =
    staticCFunction { _: CPointer<nghttp2_session>?, streamId: Int, errorCode: UInt, user: COpaquePointer? ->
        hook(user) { onStreamClose(streamId, errorCode) }
    }

@OptIn(ExperimentalForeignApi::class)
private val readCallback =
    staticCFunction {
        _: CPointer<nghttp2_session>?,
        streamId: Int,
        buf: CPointer<UByteVar>?,
        length: ULong,
        flags: CPointer<UIntVar>?,
        _: CPointer<nghttp2_data_source>?,
        user: COpaquePointer?,
        ->
        try {
            checkNotNull(user).asStableRef<Http2Connection>().get().read(streamId, checkNotNull(buf), length.toInt(), checkNotNull(flags))
        } catch (e: Throwable) {
            // Resets this stream only; the connection and its other calls go on.
            NGHTTP2_ERR_TEMPORAL_CALLBACK_FAILURE.toLong()
        }
    }
