package aulo.speech.helper

import aulo.types.AuloEvent
import okio.Buffer
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue

class WireTest {
    private fun header(
        code: Int,
        seq: Int,
        size: Int,
    ) = Buffer().apply {
        writeByte(code)
        writeIntLe(seq)
        writeIntLe(size)
    }

    @Test
    fun framesRoundTrip() {
        val buf = Buffer()
        buf.writeFrame(FrameType.BEGIN, 7, "ru".encodeToByteArray())
        buf.writeAudio(7, floatArrayOf(0f, -1f, 0.25f, Float.MIN_VALUE))
        buf.writeText(FrameType.FINAL, 7, "привет")
        buf.writeFrame(FrameType.READY, 0)

        val begin = buf.readFrame()!!
        assertEquals(FrameType.BEGIN, begin.type)
        assertEquals(7, begin.seq)
        assertEquals("ru", begin.text)
        assertContentEquals(floatArrayOf(0f, -1f, 0.25f, Float.MIN_VALUE), buf.readFrame()!!.samples())
        assertEquals("привет", buf.readFrame()!!.text)
        assertEquals(FrameType.READY, buf.readFrame()!!.type)
        assertNull(buf.readFrame(), "a clean end of stream is not an error")
    }

    @Test
    fun aPayloadOverItsCapIsRefusedBeforeItIsRead() {
        // The header claims 2 GiB and nothing follows: the reader must refuse on the length alone.
        val e = assertFailsWith<WireException> { header(FrameType.FINAL.code.toInt(), 1, Int.MAX_VALUE).readFrame() }
        assertTrue("cap" in e.message!!)
        assertFailsWith<WireException> { header(FrameType.FINISH.code.toInt(), 1, 1).readFrame() }
        assertFailsWith<WireException> { header(FrameType.AUDIO.code.toInt(), 1, MAX_AUDIO_SAMPLES * 4 + 4).readFrame() }
        assertFailsWith<WireException> { header(FrameType.FINAL.code.toInt(), 1, -1).readFrame() }
    }

    @Test
    fun theCapsAreTheEventLimits() {
        assertEquals(AuloEvent.Transcript.MAX_TEXT_BYTES, FrameType.FINAL.maxPayload)
        assertEquals(AuloEvent.Transcript.MAX_TEXT_BYTES, FrameType.PARTIAL.maxPayload)
        assertEquals(AuloEvent.Transcript.MAX_LANGUAGE_BYTES, FrameType.BEGIN.maxPayload)
        // One more byte than the cap is refused, the cap itself is read.
        val atCap = header(FrameType.FINAL.code.toInt(), 1, FrameType.FINAL.maxPayload)
        atCap.write(ByteArray(FrameType.FINAL.maxPayload) { 'a'.code.toByte() })
        assertEquals(FrameType.FINAL.maxPayload, atCap.readFrame()!!.payload.size)
    }

    @Test
    fun brokenFramesAreRefused() {
        assertFailsWith<WireException> { header(99, 1, 0).readFrame() }
        assertFailsWith<WireException> { header(FrameType.FINAL.code.toInt(), -5, 0).readFrame() }
        assertFailsWith<WireException> { header(FrameType.AUDIO.code.toInt(), 1, 6).apply { write(ByteArray(6)) }.readFrame() }
        // Cut inside the header, then inside the payload.
        assertFailsWith<WireException> { Buffer().apply { writeByte(FrameType.FINAL.code.toInt()) }.readFrame() }
        assertFailsWith<WireException> { header(FrameType.FINAL.code.toInt(), 1, 10).apply { write(ByteArray(3)) }.readFrame() }
        assertEquals(FRAME_HEADER_BYTES.toLong(), header(1, 1, 0).size)
    }

    @Test
    fun longTextIsCutOnACharacterBoundary() {
        val cap = FrameType.ERROR.maxPayload
        // Two-byte letters with an odd cap would split the last one if the cut were by bytes.
        val text = "ж".repeat(cap)
        val buf = Buffer()
        buf.writeText(FrameType.ERROR, 3, text)
        val read = buf.readFrame()!!
        assertEquals(cap - cap % 2, read.payload.size)
        assertEquals("ж".repeat(cap / 2), read.text)
        assertFailsWith<IllegalArgumentException> { Buffer().writeFrame(FrameType.ERROR, 1, ByteArray(cap + 1)) }
        assertFailsWith<IllegalArgumentException> { Buffer().writeAudio(1, FloatArray(MAX_AUDIO_SAMPLES + 1)) }
    }
}
