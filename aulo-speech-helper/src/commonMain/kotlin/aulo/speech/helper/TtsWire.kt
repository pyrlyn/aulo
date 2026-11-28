package aulo.speech.helper

import aulo.speech.MAX_LABEL_BYTES
import aulo.speech.SpeechException
import aulo.speech.SpeechRate
import aulo.speech.Voice
import aulo.types.AuloEvent
import okio.Buffer
import okio.BufferedSink
import okio.EOFException

/*
 * The payloads of the TTS frames. A string is a one-byte length then its UTF-8 bytes; every
 * length is bounded by the field's own cap, so the frame caps below hold whatever a helper sends.
 */

/** Rate (`f32`) and voice id. */
internal const val SPEAK_MAX_BYTES = 4 + 1 + MAX_LABEL_BYTES

/** Sample rate (`u32`), flags, id, name and language. */
internal const val VOICE_MAX_BYTES = 4 + 1 + (1 + MAX_LABEL_BYTES) * 2 + 1 + AuloEvent.Transcript.MAX_LANGUAGE_BYTES

/** Slowest and fastest voice a TTS helper may announce; anything else is a broken helper. */
const val MIN_VOICE_RATE_HZ = 8_000
const val MAX_VOICE_RATE_HZ = 192_000

private const val LANGUAGE_DEFAULT = 1
private const val SYSTEM_DEFAULT = 2

/** A voice of a TTS helper and the rate its audio comes in. */
data class HelperVoice(
    val voice: Voice,
    val sampleRateHz: Int,
    /** The voice the system speaks [Voice.language] in when no voice is named. */
    val languageDefault: Boolean = false,
    /** The voice of the system's own language: the pick when neither voice nor language is known. */
    val systemDefault: Boolean = false,
) {
    init {
        require(sampleRateHz in MIN_VOICE_RATE_HZ..MAX_VOICE_RATE_HZ) { "voice rate $sampleRateHz Hz" }
    }
}

/** What a [FrameType.SPEAK] frame asks for. */
data class Speak(
    val voice: String,
    val rate: SpeechRate,
)

fun BufferedSink.writeSpeak(
    seq: Int,
    voice: String,
    rate: SpeechRate,
) {
    val payload = Buffer()
    payload.writeIntLe(rate.value.toRawBits())
    payload.writeLabel(voice)
    writeFrame(FrameType.SPEAK, seq, payload.readByteArray())
}

/** The request of a [FrameType.SPEAK] frame; a malformed one throws [WireException]. */
fun Frame.speak(): Speak =
    decode(FrameType.SPEAK) {
        val rate = Float.fromBits(readIntLe())
        Speak(readLabel(MAX_LABEL_BYTES), SpeechRate(rate)).also { requireExhausted() }
    }

fun BufferedSink.writeVoice(voice: HelperVoice) {
    val payload = Buffer()
    payload.writeIntLe(voice.sampleRateHz)
    var flags = 0
    if (voice.languageDefault) flags = flags or LANGUAGE_DEFAULT
    if (voice.systemDefault) flags = flags or SYSTEM_DEFAULT
    payload.writeByte(flags)
    payload.writeLabel(voice.voice.id)
    payload.writeLabel(voice.voice.name)
    payload.writeLabel(voice.voice.language.orEmpty())
    writeFrame(FrameType.VOICE, 0, payload.readByteArray())
}

/**
 * The voice of a [FrameType.VOICE] frame. A helper is untrusted, so a voice the contract would
 * refuse (a label too long, a bad language tag, a rate out of range) throws [WireException].
 */
fun Frame.voice(): HelperVoice =
    decode(FrameType.VOICE) {
        val rate = readIntLe()
        if (rate !in MIN_VOICE_RATE_HZ..MAX_VOICE_RATE_HZ) throw WireException("VOICE: rate of $rate Hz")
        val flags = readByte().toInt()
        val id = readLabel(MAX_LABEL_BYTES)
        val name = readLabel(MAX_LABEL_BYTES)
        val language = readLabel(AuloEvent.Transcript.MAX_LANGUAGE_BYTES).ifEmpty { null }
        requireExhausted()
        HelperVoice(Voice(id, name, language), rate, flags and LANGUAGE_DEFAULT != 0, flags and SYSTEM_DEFAULT != 0)
    }

private fun Buffer.writeLabel(value: String) {
    val bytes = value.encodeToByteArray()
    require(bytes.size <= MAX_LABEL_BYTES) { "label of ${bytes.size} bytes" }
    writeByte(bytes.size)
    write(bytes)
}

private fun Buffer.readLabel(max: Int): String {
    val size = readByte().toInt() and 0xff
    if (size > max) throw WireException("a field of $size bytes, the cap is $max")
    return readUtf8(size.toLong())
}

private fun Buffer.requireExhausted() {
    if (!exhausted()) throw WireException("$size bytes after the last field")
}

private inline fun <T> Frame.decode(
    expected: FrameType,
    block: Buffer.() -> T,
): T {
    check(type == expected) { "not a ${expected.name} frame: ${type.name}" }
    try {
        return Buffer().write(payload).block()
    } catch (e: EOFException) {
        throw WireException("${type.name}: payload ends inside a field")
    } catch (e: SpeechException) {
        throw WireException("${type.name}: ${e.message}")
    }
}
