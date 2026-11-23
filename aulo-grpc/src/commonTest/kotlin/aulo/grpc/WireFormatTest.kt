package aulo.grpc

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.time.Duration.Companion.hours
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.nanoseconds
import kotlin.time.Duration.Companion.seconds

class WireFormatTest {
    @Test
    fun a_message_split_across_chunks_comes_out_whole() {
        val bytes = frame("hello".encodeToByteArray()) + frame(ByteArray(0)) + frame("world".encodeToByteArray())
        val deframer = Deframer(maxMessageBytes = 16)
        val out = bytes.toList().chunked(3).flatMap { deframer.feed(it.toByteArray()) }
        assertEquals(listOf("hello", "", "world"), out.map { it.decodeToString() })
        assertTrue(!deframer.partial)
    }

    @Test
    fun a_large_message_in_small_chunks_comes_out_whole() {
        val message = ByteArray(100_000) { (it * 7).toByte() }
        val deframer = Deframer(maxMessageBytes = 100_000)
        val out = frame(message).toList().chunked(999).flatMap { deframer.feed(it.toByteArray()) }
        assertContentEquals(message, out.single())
    }

    @Test
    fun a_length_over_the_cap_fails_before_the_body_arrives() {
        val deframer = Deframer(maxMessageBytes = 1024)
        // Only the prefix of a 4 GiB message: the cap must hold without allocating or waiting.
        val prefix = byteArrayOf(0, 0xFF.toByte(), 0xFF.toByte(), 0xFF.toByte(), 0xFF.toByte())
        val e = assertFailsWith<GrpcException> { deframer.feed(prefix) }
        assertEquals(StatusCode.RESOURCE_EXHAUSTED, e.status.code)
    }

    @Test
    fun a_message_at_the_cap_passes() {
        val message = ByteArray(1024) { it.toByte() }
        assertContentEquals(message, Deframer(maxMessageBytes = 1024).feed(frame(message)).single())
    }

    @Test
    fun a_compressed_message_is_refused() {
        val compressed = frame("x".encodeToByteArray()).also { it[0] = 1 }
        val e = assertFailsWith<GrpcException> { Deframer(16).feed(compressed) }
        assertEquals(StatusCode.UNIMPLEMENTED, e.status.code)
    }

    @Test
    fun a_cut_message_leaves_the_deframer_partial() {
        val deframer = Deframer(16)
        deframer.feed(frame("hello".encodeToByteArray()).copyOf(6))
        assertTrue(deframer.partial)
    }

    @Test
    fun status_messages_round_trip_through_percent_encoding() {
        val message = "100% done: ünïcode 🚀\nnext line"
        val encoded = StatusMessage.encode(message)
        assertTrue(encoded.all { it.code in 0x20..0x7E })
        assertEquals("100%25 done", encoded.substringBefore(':'))
        assertEquals(message, StatusMessage.decode(encoded))
    }

    @Test
    fun a_malformed_escape_is_kept_as_it_is() {
        assertEquals("50%zz and %4 and 🚀%", StatusMessage.decode("50%zz and %4 and 🚀%"))
    }

    @Test
    fun unknown_status_codes_read_as_unknown() {
        assertEquals(StatusCode.UNKNOWN, StatusCode.of(99))
        assertEquals(StatusCode.UNAUTHENTICATED, StatusCode.of(16))
    }

    @Test
    fun timeouts_parse_every_unit() {
        assertEquals(1.hours, GrpcTimeout.parse("1H"))
        assertEquals(250.milliseconds, GrpcTimeout.parse("250m"))
        assertEquals(99_999_999.nanoseconds, GrpcTimeout.parse("99999999n"))
        assertEquals(30.seconds, GrpcTimeout.parse("30S"))
    }

    @Test
    fun malformed_timeouts_are_rejected() {
        for (bad in listOf("", "S", "1", "123456789S", "1s", "-1S", "1.5S", "1 S")) assertNull(GrpcTimeout.parse(bad), bad)
    }

    @Test
    fun a_formatted_timeout_fits_eight_digits_and_never_shrinks() {
        assertEquals("1500n", GrpcTimeout.format(1500.nanoseconds))
        assertEquals("99999999n", GrpcTimeout.format(99_999_999.nanoseconds))
        assertEquals("100000u", GrpcTimeout.format(100_000_000.nanoseconds))
        for (d in listOf(1.nanoseconds, 1001.nanoseconds, 3.seconds, 1.hours, 10_000.hours)) {
            val formatted = GrpcTimeout.format(d)
            assertTrue(formatted.length <= 9, formatted)
            assertTrue(GrpcTimeout.parse(formatted)!! >= d, formatted)
        }
    }
}
