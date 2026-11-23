package aulo.grpc

/** gRPC's length-prefixed message: one compressed-flag byte, then the length as big-endian u32. */
internal const val PREFIX_BYTES = 5

internal fun frame(message: ByteArray): ByteArray {
    val out = ByteArray(PREFIX_BYTES + message.size)
    val n = message.size
    out[1] = (n ushr 24).toByte()
    out[2] = (n ushr 16).toByte()
    out[3] = (n ushr 8).toByte()
    out[4] = n.toByte()
    message.copyInto(out, PREFIX_BYTES)
    return out
}

/**
 * Splits a stream's DATA bytes into messages. The peer is untrusted: a length over
 * [maxMessageBytes] fails before anything is allocated for it.
 */
internal class Deframer(
    private val maxMessageBytes: Int,
) {
    private val prefix = ByteArray(PREFIX_BYTES)
    private var prefixFill = 0
    private var length = 0
    private var body = EMPTY
    private var bodyFill = 0

    private companion object {
        val EMPTY = ByteArray(0)
    }

    /** True while a message has started but not ended. */
    val partial: Boolean get() = prefixFill > 0

    /** The messages [data] completes; throws [GrpcException] on a cap or a compressed message. */
    fun feed(data: ByteArray): List<ByteArray> {
        val out = ArrayList<ByteArray>(1)
        var i = 0
        while (i < data.size) {
            if (prefixFill < PREFIX_BYTES) {
                val n = minOf(PREFIX_BYTES - prefixFill, data.size - i)
                data.copyInto(prefix, prefixFill, i, i + n)
                prefixFill += n
                i += n
                if (prefixFill == PREFIX_BYTES) startBody(out)
                continue
            }
            val n = minOf(length - bodyFill, data.size - i)
            if (bodyFill + n > body.size) body = body.copyOf(minOf(length, maxOf(bodyFill + n, body.size * 2)))
            data.copyInto(body, bodyFill, i, i + n)
            bodyFill += n
            i += n
            if (bodyFill == length) finish(body, out)
        }
        return out
    }

    private fun startBody(out: MutableList<ByteArray>) {
        // aulo advertises no grpc-encoding, so a peer has no business compressing.
        if (prefix[0].toInt() != 0) throw GrpcException(StatusCode.UNIMPLEMENTED, "compressed messages are not supported")
        val declared =
            ((prefix[1].toLong() and 0xFF) shl 24) or ((prefix[2].toLong() and 0xFF) shl 16) or
                ((prefix[3].toLong() and 0xFF) shl 8) or (prefix[4].toLong() and 0xFF)
        if (declared > maxMessageBytes) {
            throw GrpcException(StatusCode.RESOURCE_EXHAUSTED, "message of $declared bytes exceeds the limit of $maxMessageBytes")
        }
        length = declared.toInt()
        // Grown as bytes arrive, not sized from the prefix: a peer that declares large messages on
        // many streams and sends nothing more must not cost memory it never sent.
        body = EMPTY
        if (length == 0) finish(ByteArray(0), out)
    }

    private fun finish(
        message: ByteArray,
        out: MutableList<ByteArray>,
    ) {
        out += message
        prefixFill = 0
        length = 0
        body = EMPTY
        bodyFill = 0
    }
}
