package aulo.server

import aulo.grpc.GrpcException
import aulo.grpc.StatusCode
import aulo.store.ChatCursor
import aulo.store.SearchCursor
import aulo.types.Ulid
import kotlin.io.encoding.Base64

// Paging as common.proto states it: page_size 0 is the default and anything above the maximum is
// clamped; page_token is opaque. The tokens hold the keyset cursors aulo-store pages by, so a
// page never skips or repeats rows as others are written.

internal const val DEFAULT_PAGE_SIZE = 50
internal const val MAX_PAGE_SIZE = 200

// The longest cursor (search: a 20-digit rank and a ULID) encodes to about 70 characters.
internal const val MAX_PAGE_TOKEN_CHARS = 128

private const val SEPARATOR = '|'
private val base64 = Base64.UrlSafe.withPadding(Base64.PaddingOption.ABSENT)

/** Wire reads uint32 into an Int, so a size of 2^31 or more arrives negative: clamped like any large one. */
internal fun pageSize(requested: Int): Int =
    when {
        requested == 0 -> DEFAULT_PAGE_SIZE
        requested < 0 || requested > MAX_PAGE_SIZE -> MAX_PAGE_SIZE
        else -> requested
    }

/**
 * One page of [rows] fetched with a limit of `size + 1`: the extra row only says another page
 * exists, and the token points after the last row kept.
 */
internal fun <T, R> page(
    rows: List<T>,
    size: Int,
    token: (T) -> String,
    build: (List<T>, String) -> R,
): R {
    val kept = rows.take(size)
    return build(kept, if (rows.size > size) token(kept.last()) else "")
}

internal object PageTokens {
    private const val CHATS = "c"
    private const val MESSAGES = "m"
    private const val SEARCH = "s"

    fun ofChat(cursor: ChatCursor): String = encode(CHATS, cursor.updatedAt.toString(), cursor.id)

    fun chatsAfter(token: String): ChatCursor? =
        decode(token, CHATS, 2)?.let { (updatedAt, id) -> ChatCursor(updatedAt.toLongOrNull()?.takeIf { it >= 0 } ?: invalid(), id) }

    fun ofMessage(lastId: String): String = encode(MESSAGES, lastId)

    fun messagesAfter(token: String): String? = decode(token, MESSAGES, 1)?.single()

    // The rank's exact bits: a decimal round trip could move the cursor past a hit or before it.
    fun ofHit(cursor: SearchCursor): String = encode(SEARCH, cursor.rank.toRawBits().toString(), cursor.id)

    fun searchAfter(token: String): SearchCursor? =
        decode(token, SEARCH, 2)?.let { (bits, id) ->
            val rank = Double.fromBits(bits.toLongOrNull() ?: invalid())
            SearchCursor(if (rank.isFinite()) rank else invalid(), id)
        }

    private fun encode(vararg parts: String): String = base64.encode(parts.joinToString(SEPARATOR.toString()).encodeToByteArray())

    /** The fields after [kind], the last one a ULID; null for an empty token, the first page. */
    private fun decode(
        token: String,
        kind: String,
        fields: Int,
    ): List<String>? {
        if (token.isEmpty()) return null
        if (token.length > MAX_PAGE_TOKEN_CHARS) invalid()
        val text =
            try {
                base64.decode(token).decodeToString(throwOnInvalidSequence = true)
            } catch (_: IllegalArgumentException) {
                invalid()
            } catch (_: CharacterCodingException) {
                invalid()
            }
        val parts = text.split(SEPARATOR)
        if (parts.size != fields + 1 || parts[0] != kind || !Ulid.isValid(parts.last())) invalid()
        return parts.drop(1)
    }

    // A token from another list, an older build or a client's own edit: nothing of it is echoed.
    private fun invalid(): Nothing = throw GrpcException(StatusCode.INVALID_ARGUMENT, "page_token is not valid")
}
