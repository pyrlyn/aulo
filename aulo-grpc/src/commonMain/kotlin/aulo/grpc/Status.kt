package aulo.grpc

/** The gRPC status codes (https://grpc.github.io/grpc/core/md_doc_statuscodes.html). */
enum class StatusCode(
    val value: Int,
) {
    OK(0),
    CANCELLED(1),
    UNKNOWN(2),
    INVALID_ARGUMENT(3),
    DEADLINE_EXCEEDED(4),
    NOT_FOUND(5),
    ALREADY_EXISTS(6),
    PERMISSION_DENIED(7),
    RESOURCE_EXHAUSTED(8),
    FAILED_PRECONDITION(9),
    ABORTED(10),
    OUT_OF_RANGE(11),
    UNIMPLEMENTED(12),
    INTERNAL(13),
    UNAVAILABLE(14),
    DATA_LOSS(15),
    UNAUTHENTICATED(16),
    ;

    companion object {
        /** The code for [value]; the gRPC spec maps codes it does not know to [UNKNOWN]. */
        fun of(value: Int): StatusCode = entries.firstOrNull { it.value == value } ?: UNKNOWN
    }
}

/**
 * The outcome of a call. A [message] that came from the peer is untrusted text: mask it before
 * showing it to a person.
 */
data class Status(
    val code: StatusCode,
    val message: String = "",
) {
    val isOk: Boolean get() = code == StatusCode.OK

    companion object {
        val OK = Status(StatusCode.OK)
    }
}

/** A call ended with a status other than OK; a server handler throws it to send that status. */
class GrpcException(
    val status: Status,
) : Exception(if (status.message.isEmpty()) status.code.name else "${status.code.name}: ${status.message}") {
    constructor(code: StatusCode, message: String = "") : this(Status(code, message))
}

/** `grpc-message` is percent-encoded UTF-8 (gRPC over HTTP/2, "Responses"). */
internal object StatusMessage {
    private const val HEX = "0123456789ABCDEF"

    fun encode(message: String): String {
        val out = StringBuilder()
        for (b in message.encodeToByteArray()) {
            val c = b.toInt() and 0xFF
            if (c in 0x20..0x7E && c != '%'.code) {
                out.append(c.toChar())
            } else {
                out.append('%').append(HEX[c shr 4]).append(HEX[c and 0xF])
            }
        }
        return out.toString()
    }

    /** Decodes what a peer sent; a malformed escape is kept as it is, as the spec asks. */
    fun decode(value: String): String {
        if ('%' !in value) return value
        val bytes = ArrayList<Byte>(value.length)
        var i = 0
        while (i < value.length) {
            val c = value[i]
            val hi = if (c == '%' && i + 2 <= value.lastIndex) value[i + 1].digitToIntOrNull(16) else null
            val lo = if (hi != null) value[i + 2].digitToIntOrNull(16) else null
            if (hi != null && lo != null) {
                bytes += ((hi shl 4) or lo).toByte()
                i += 3
            } else {
                // Copy up to the next escape in one piece, so a surrogate pair is never split.
                val end = value.indexOf('%', i + 1).let { if (it < 0) value.length else it }
                value.substring(i, end).encodeToByteArray().forEach { bytes += it }
                i = end
            }
        }
        return bytes.toByteArray().decodeToString()
    }
}
