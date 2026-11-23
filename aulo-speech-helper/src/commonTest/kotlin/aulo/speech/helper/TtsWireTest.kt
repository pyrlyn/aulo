package aulo.speech.helper

import aulo.speech.MAX_LABEL_BYTES
import aulo.speech.SpeechRate
import aulo.speech.Voice
import okio.Buffer
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class TtsWireTest {
    private fun Buffer.frame(): Frame = readFrame()!!

    private fun voiceFrame(
        rate: Int,
        fields: List<ByteArray>,
        trailing: Int = 0,
    ): Frame {
        val payload = Buffer().writeIntLe(rate).writeByte(0)
        for (f in fields) payload.writeByte(f.size).write(f)
        payload.write(ByteArray(trailing))
        return Frame(FrameType.VOICE, 0, payload.readByteArray())
    }

    @Test
    fun speakRoundTrips() {
        val buf = Buffer()
        buf.writeSpeak(7, "com.apple.voice.compact.ru-RU.Milena", SpeechRate(1.5f))
        val frame = buf.frame()
        assertEquals(FrameType.SPEAK, frame.type)
        assertEquals(7, frame.seq)
        assertEquals(Speak("com.apple.voice.compact.ru-RU.Milena", SpeechRate(1.5f)), frame.speak())
    }

    @Test
    fun voicesRoundTripWithTheirFlags() {
        val voices =
            listOf(
                HelperVoice(Voice("v1", "Milena", "ru-RU"), 22_050, languageDefault = true),
                HelperVoice(Voice("v2", "Samantha", "en-US"), 16_000, systemDefault = true),
                HelperVoice(Voice("v3", "No language"), 48_000),
                HelperVoice(Voice("x".repeat(MAX_LABEL_BYTES), "y".repeat(MAX_LABEL_BYTES), "zh-Hant-TW"), 24_000),
            )
        val buf = Buffer()
        voices.forEach { buf.writeVoice(it) }
        assertEquals(voices, voices.map { buf.frame().voice() })
    }

    @Test
    fun aVoiceTheContractRefusesBreaksTheProtocol() {
        val ok = "ok".encodeToByteArray()
        // A rate out of range, a label over its cap, an empty id, a bad language tag.
        assertFailsWith<WireException> { voiceFrame(4_000, listOf(ok, ok, ByteArray(0))).voice() }
        assertFailsWith<WireException> { voiceFrame(16_000, listOf(ByteArray(MAX_LABEL_BYTES + 1), ok, ByteArray(0))).voice() }
        assertFailsWith<WireException> { voiceFrame(16_000, listOf(ByteArray(0), ok, ByteArray(0))).voice() }
        assertFailsWith<WireException> { voiceFrame(16_000, listOf(ok, ok, "not a tag!".encodeToByteArray())).voice() }
        // Cut inside a field, or bytes after the last one.
        assertFailsWith<WireException> { Frame(FrameType.VOICE, 0, byteArrayOf(0x80.toByte(), 0x3e, 0, 0, 0, 5)).voice() }
        assertFailsWith<WireException> { voiceFrame(16_000, listOf(ok, ok, ByteArray(0)), trailing = 1).voice() }
    }

    @Test
    fun aSpeakOutsideTheRateRangeBreaksTheProtocol() {
        val payload = Buffer().writeIntLe(Float.NaN.toRawBits()).writeByte(2).writeUtf8("v1")
        assertFailsWith<WireException> { Frame(FrameType.SPEAK, 1, payload.readByteArray()).speak() }
    }

    @Test
    fun theVoiceCapHoldsTheLargestVoice() {
        val buf = Buffer()
        buf.writeVoice(HelperVoice(Voice("x".repeat(MAX_LABEL_BYTES), "y".repeat(MAX_LABEL_BYTES), "zh-Hant-TW"), 16_000))
        assertEquals(FrameType.VOICE.maxPayload - (35 - "zh-Hant-TW".length), buf.frame().payload.size)
    }
}
