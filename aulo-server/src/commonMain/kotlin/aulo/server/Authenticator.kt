package aulo.server

import aulo.grpc.Endpoint
import aulo.grpc.GrpcException
import aulo.grpc.ServerCall
import aulo.grpc.StatusCode
import okio.ByteString.Companion.decodeHex
import okio.ByteString.Companion.encodeUtf8
import org.kotlincrypto.random.CryptoRand
import kotlin.io.encoding.Base64

/** aulo's API tokens: [PREFIX] and 32 random bytes in base64url. Only their SHA-256 is stored. */
object BearerTokens {
    /** Marks a string as an aulo token, so a secret scanner can find a leaked one. */
    const val PREFIX = "aulo_"

    /** A longer `authorization` value is refused before anything else is done with it. */
    const val MAX_HEADER_CHARS = 256

    private const val RANDOM_BYTES = 32

    private val encoding = Base64.UrlSafe.withPadding(Base64.PaddingOption.ABSENT)

    fun generate(): String = PREFIX + encoding.encode(CryptoRand.Default.nextBytes(ByteArray(RANDOM_BYTES)))

    /** The lowercase hex SHA-256 that `api_tokens.hash` keeps. */
    fun hash(token: String): String = token.encodeUtf8().sha256().hex()
}

/**
 * Admits every call on the local socket, whose owner-only directory already keeps others out,
 * and on TCP only a call with `authorization: Bearer <token>` whose hash is in [tokenHashes].
 * An empty [tokenHashes] refuses every TCP call.
 */
class Authenticator(
    tokenHashes: Collection<String>,
) {
    private val digests =
        tokenHashes.map { hash ->
            hash.decodeHex().toByteArray().also { require(it.size == DIGEST_BYTES) { "a token hash is a hex SHA-256" } }
        }

    /** Throws UNAUTHENTICATED, with one message for every reason so a client learns nothing more. */
    fun check(call: ServerCall) {
        if (call.endpoint is Endpoint.Unix) return
        val token = bearerToken(call.metadata) ?: throw refused()
        val digest = token.encodeUtf8().sha256().toByteArray()
        // Every stored digest is compared in full: the time taken says nothing about which one,
        // or how much of one, matched.
        var matches = 0
        for (known in digests) matches = matches or same(known, digest)
        if (matches == 0) throw refused()
    }

    private fun bearerToken(metadata: List<Pair<String, String>>): String? {
        val values = metadata.filter { (name, _) -> name == AUTHORIZATION }
        if (values.size != 1) return null
        val value = values.single().second
        if (value.length > BearerTokens.MAX_HEADER_CHARS) return null
        val space = value.indexOf(' ')
        if (space < 0 || !value.substring(0, space).equals(SCHEME, ignoreCase = true)) return null
        return value.substring(space + 1).takeIf { it.isNotEmpty() && it.none(Char::isWhitespace) }
    }

    private companion object {
        const val AUTHORIZATION = "authorization"
        const val SCHEME = "Bearer"
        const val DIGEST_BYTES = 32

        fun refused() = GrpcException(StatusCode.UNAUTHENTICATED, "a valid bearer token is required")

        /** 1 when equal, else 0, without a branch on the bytes. Both are [DIGEST_BYTES] long. */
        fun same(
            a: ByteArray,
            b: ByteArray,
        ): Int {
            var diff = 0
            // Masked: a Byte widens with its sign, and a negative diff would read as equal below.
            for (i in 0 until DIGEST_BYTES) diff = diff or ((a[i].toInt() xor b[i].toInt()) and 0xFF)
            // diff is 0..255, so diff - 1 is negative, its sign bit set, only when diff is 0.
            return (diff - 1) ushr 31
        }
    }
}
