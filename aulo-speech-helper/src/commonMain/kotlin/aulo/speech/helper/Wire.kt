package aulo.speech.helper

import aulo.speech.MAX_ERROR_DETAIL_BYTES
import aulo.speech.PIPELINE_SAMPLE_RATE_HZ
import aulo.types.AuloEvent
import okio.BufferedSink
import okio.BufferedSource
import okio.EOFException

/*
 * The pipe protocol between aulod and a speech helper: aulod writes to the helper's stdin, the
 * helper answers on its stdout and logs to stderr. Each frame is a type byte, the utterance
 * number and the payload length, both little-endian u32, then the payload.
 *
 * The helper runs native inference code over model files, so aulod treats what it says as
 * untrusted: every payload has a cap per type and the length is checked before anything is
 * read, so a broken helper cannot make aulod allocate more than one capped frame.
 */

/** Most samples in one audio frame: one second of pipeline audio. */
const val MAX_AUDIO_SAMPLES = PIPELINE_SAMPLE_RATE_HZ

private const val HEADER_BYTES = 9
private const val FLOAT_BYTES = 4

/** What one frame is; the code is the first byte on the wire. */
enum class FrameType(
    val code: Byte,
    /** Longest payload in bytes. */
    val maxPayload: Int,
) {
    // aulod to the helper.

    /** Starts utterance `seq` of an STT helper; the payload is the BCP 47 language hint, empty to detect. */
    BEGIN(1, AuloEvent.Transcript.MAX_LANGUAGE_BYTES),

    /**
     * Mono samples of utterance or reply `seq`, little-endian `f32`: 16 kHz from aulod to an STT
     * helper, the voice's rate from a TTS helper to aulod. Both ways.
     */
    AUDIO(2, MAX_AUDIO_SAMPLES * FLOAT_BYTES),

    /** No more input for `seq`: an STT helper answers [FINAL], a TTS helper [END] once its audio is out. */
    FINISH(3, 0),

    /** Drop `seq` and everything queued for it; nothing more follows for it. */
    CANCEL(4, 0),

    /** Starts reply `seq` of a TTS helper; the payload is the rate and the voice ([writeSpeak]). */
    SPEAK(5, SPEAK_MAX_BYTES),

    /** Text of reply `seq`, usually one sentence. */
    TEXT(6, AuloEvent.Transcript.MAX_TEXT_BYTES),

    // The helper to aulod.

    /** The model is loaded, or a TTS helper's [VOICE] list is complete; `seq` is 0. */
    READY(16, 0),

    /** The newest partial transcript of utterance `seq`. */
    PARTIAL(17, AuloEvent.Transcript.MAX_TEXT_BYTES),

    /** The final transcript of utterance `seq`. */
    FINAL(18, AuloEvent.Transcript.MAX_TEXT_BYTES),

    /** Utterance `seq` failed, or the helper cannot start when `seq` is 0; the payload says why. */
    ERROR(19, MAX_ERROR_DETAIL_BYTES),

    /** One voice a TTS helper offers, sent before [READY]; `seq` is 0 ([writeVoice]). */
    VOICE(20, VOICE_MAX_BYTES),

    /** All audio of reply `seq` was sent. */
    END(21, 0),
    ;

    companion object {
        fun of(code: Byte): FrameType? = entries.firstOrNull { it.code == code }
    }
}

/** A frame as read; [payload] is at most the type's cap. */
class Frame(
    val type: FrameType,
    /** The utterance; aulod numbers them from 1, and 0 is the helper itself. */
    val seq: Int,
    val payload: ByteArray,
) {
    val text: String get() = payload.decodeToString()

    /** The samples of an [FrameType.AUDIO] frame. */
    fun samples(): FloatArray {
        val out = FloatArray(payload.size / FLOAT_BYTES)
        for (i in out.indices) {
            val o = i * FLOAT_BYTES
            val bits =
                (payload[o].toInt() and 0xff) or
                    ((payload[o + 1].toInt() and 0xff) shl 8) or
                    ((payload[o + 2].toInt() and 0xff) shl 16) or
                    (payload[o + 3].toInt() shl 24)
            out[i] = Float.fromBits(bits)
        }
        return out
    }
}

/** The other side broke the protocol; the link cannot be trusted any more. */
class WireException(
    message: String,
) : Exception(message)

/**
 * Reads the next frame, or null at a clean end of stream (the other side closed before a new
 * frame). A frame cut short, an unknown type or a payload over its cap throws [WireException].
 */
fun BufferedSource.readFrame(): Frame? {
    if (exhausted()) return null
    try {
        val code = readByte()
        val type = FrameType.of(code) ?: throw WireException("unknown frame type $code")
        val seq = readIntLe()
        val size = readIntLe()
        if (seq < 0) throw WireException("${type.name}: negative utterance number")
        if (size < 0 || size > type.maxPayload) {
            throw WireException("${type.name}: payload of $size bytes, the cap is ${type.maxPayload}")
        }
        if (type == FrameType.AUDIO && size % FLOAT_BYTES != 0) throw WireException("AUDIO: payload is not whole samples")
        return Frame(type, seq, readByteArray(size.toLong()))
    } catch (e: EOFException) {
        throw WireException("stream ended inside a frame")
    }
}

/** Writes a frame whose payload is [payload]; a payload over the type's cap is a caller bug. */
fun BufferedSink.writeFrame(
    type: FrameType,
    seq: Int,
    payload: ByteArray = EMPTY,
) {
    require(payload.size <= type.maxPayload) { "${type.name} payload of ${payload.size} bytes, the cap is ${type.maxPayload}" }
    writeByte(type.code.toInt())
    writeIntLe(seq)
    writeIntLe(payload.size)
    write(payload)
}

/** Writes an [FrameType.AUDIO] frame of [count] samples of [samples] from [offset], without an extra copy. */
fun BufferedSink.writeAudio(
    seq: Int,
    samples: FloatArray,
    offset: Int = 0,
    count: Int = samples.size - offset,
) {
    require(count in 0..MAX_AUDIO_SAMPLES) { "AUDIO of $count samples, the cap is $MAX_AUDIO_SAMPLES" }
    writeByte(FrameType.AUDIO.code.toInt())
    writeIntLe(seq)
    writeIntLe(count * FLOAT_BYTES)
    for (i in offset until offset + count) writeIntLe(samples[i].toRawBits())
}

/** Writes text, cut to the type's cap on a character boundary: model output can be any length. */
fun BufferedSink.writeText(
    type: FrameType,
    seq: Int,
    text: String,
) = writeFrame(type, seq, text.utf8Prefix(type.maxPayload))

internal fun String.utf8Prefix(maxBytes: Int): ByteArray {
    val bytes = encodeToByteArray()
    if (bytes.size <= maxBytes) return bytes
    return bytes.copyOf(bytes.utf8Cut(0, maxBytes))
}

/** The text in pieces of at most [maxBytes] of UTF-8 each, cut on character boundaries. */
internal fun String.utf8Chunks(maxBytes: Int): List<String> {
    val bytes = encodeToByteArray()
    val chunks = mutableListOf<String>()
    var from = 0
    while (from < bytes.size) {
        val end = bytes.utf8Cut(from, maxBytes)
        chunks += bytes.decodeToString(from, end)
        from = end
    }
    return chunks
}

/** Where a piece of at most [maxBytes] that starts at [from] ends without splitting a character. */
private fun ByteArray.utf8Cut(
    from: Int,
    maxBytes: Int,
): Int {
    require(maxBytes >= 4) { "a piece must hold any character" }
    var end = from + maxBytes
    if (end >= size) return size
    // Back off continuation bytes (10xxxxxx) so the cut never splits a character.
    while (end > from && (this[end].toInt() and 0xC0) == 0x80) end--
    return end
}

private val EMPTY = ByteArray(0)

/** Header size, for tests that build frames by hand. */
internal const val FRAME_HEADER_BYTES = HEADER_BYTES
