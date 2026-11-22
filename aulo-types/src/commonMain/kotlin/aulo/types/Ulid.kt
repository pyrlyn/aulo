package aulo.types

import kotlin.time.Clock
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

/**
 * ULIDs (https://github.com/ulid/spec): 48-bit millisecond timestamp, then 80 random bits,
 * as 26 Crockford base32 characters. They sort by creation time, which the store relies on.
 * Only the canonical upper-case form is valid, so one id has exactly one spelling.
 */
object Ulid {
    const val LENGTH = 26
    private const val ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
    private const val RANDOM_BYTES = 10
    private const val MAX_TIME_MS = (1L shl 48) - 1

    /** A new ULID for the current time. */
    fun next(): String = encode(Clock.System.now().toEpochMilliseconds(), secureRandomBytes())

    /** Encodes [timeMs] and the first ten bytes of [random]; tests pass both to get fixed ids. */
    fun encode(
        timeMs: Long,
        random: ByteArray,
    ): String {
        require(timeMs in 0..MAX_TIME_MS) { "ULID time out of range: $timeMs" }
        require(random.size >= RANDOM_BYTES) { "ULID needs $RANDOM_BYTES random bytes" }
        val bytes = ByteArray(16)
        for (i in 0 until 6) bytes[i] = (timeMs shr (8 * (5 - i))).toByte()
        random.copyInto(bytes, destinationOffset = 6, endIndex = RANDOM_BYTES)
        // 26 characters hold 130 bits; the two missing leading bits are zero.
        return buildString(LENGTH) {
            for (char in 0 until LENGTH) {
                var value = 0
                for (offset in 0 until 5) {
                    val bit = char * 5 + offset - 2
                    value = value shl 1
                    if (bit >= 0) value = value or ((bytes[bit / 8].toInt() shr (7 - bit % 8)) and 1)
                }
                append(ALPHABET[value])
            }
        }
    }

    fun isValid(value: String): Boolean = value.length == LENGTH && value[0] <= '7' && value.all { it in ALPHABET }

    /** Milliseconds since the Unix epoch encoded in a valid ULID. */
    fun timestampMs(value: String): Long {
        require(isValid(value)) { "not a ULID" }
        return value.take(10).fold(0L) { acc, c -> (acc shl 5) or ALPHABET.indexOf(c).toLong() }
    }

    // Uuid.random() is the stdlib's cryptographically secure source on every native target;
    // 80 of its 122 random bits are enough and need no extra dependency.
    @OptIn(ExperimentalUuidApi::class)
    private fun secureRandomBytes(): ByteArray = Uuid.random().toByteArray().copyOfRange(6, 16)
}

/** Rejects anything but a canonical ULID; the value itself is never echoed, it may be untrusted. */
internal fun requireUlid(value: String) {
    require(Ulid.isValid(value)) { "not a canonical ULID (${value.length} chars)" }
}
