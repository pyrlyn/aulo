package aulo.speech.openai

import kotlin.random.Random

private const val WAV_HEADER_BYTES = 44
private const val BOUNDARY_CHARS = 32
private const val ALNUM = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"

/** Model ids of OpenAI's GPT Transcribe family, which takes `languages[]` instead of `language`. */
private const val LANGUAGES_MODEL_PREFIX = "gpt-transcribe"

/** One `multipart/form-data` body and the content type that names its boundary. */
internal class Form(
    val contentType: String,
    val body: ByteArray,
)

/**
 * The transcription request: the fields, then the audio as a 16-bit mono WAV, written straight
 * into one array sized up front, since the audio can be tens of megabytes.
 *
 * Every value is an engine label or a language tag, both validated free of control characters,
 * so none can break out of its part.
 */
internal fun transcriptionForm(
    model: String,
    language: String?,
    stream: Boolean,
    samples: ShortArray,
    count: Int,
    sampleRateHz: Int,
    random: Random = Random.Default,
): Form {
    // Random rather than checked against the audio: 32 alphanumerics leave a collision at about 2^-190.
    val boundary = "aulo" + CharArray(BOUNDARY_CHARS) { ALNUM[random.nextInt(ALNUM.length)] }.concatToString()
    val fields = mutableListOf("model" to model, "response_format" to "json")
    if (language != null) {
        // Both are ISO 639-1 codes; the region is the voice's business, not the recognizer's.
        val code = language.substringBefore('-').lowercase()
        fields += (if (model.startsWith(LANGUAGES_MODEL_PREFIX)) "languages[]" else "language") to code
    }
    if (stream) fields += "stream" to "true"
    val head = StringBuilder()
    for ((name, value) in fields) {
        head.append("--").append(boundary).append("\r\n")
        head.append("Content-Disposition: form-data; name=\"").append(name).append("\"\r\n\r\n")
        head.append(value).append("\r\n")
    }
    head.append("--").append(boundary).append("\r\n")
    head.append("Content-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\n")
    head.append("Content-Type: audio/wav\r\n\r\n")
    val headBytes = head.toString().encodeToByteArray()
    val tail = "\r\n--$boundary--\r\n".encodeToByteArray()
    val body = ByteArray(headBytes.size + WAV_HEADER_BYTES + count * 2 + tail.size)
    headBytes.copyInto(body)
    var at = writeWavHeader(body, headBytes.size, count, sampleRateHz)
    for (i in 0 until count) {
        val s = samples[i].toInt()
        body[at++] = s.toByte()
        body[at++] = (s shr 8).toByte()
    }
    tail.copyInto(body, at)
    return Form("multipart/form-data; boundary=$boundary", body)
}

/** The RIFF header of a 16-bit mono PCM WAV of [count] samples; returns where the samples start. */
private fun writeWavHeader(
    out: ByteArray,
    from: Int,
    count: Int,
    sampleRateHz: Int,
): Int {
    var at = from

    fun ascii(s: String) = s.forEach { out[at++] = it.code.toByte() }

    fun u32(v: Int) = repeat(4) { out[at++] = (v shr (8 * it)).toByte() }

    fun u16(v: Int) = repeat(2) { out[at++] = (v shr (8 * it)).toByte() }
    val data = count * 2
    ascii("RIFF")
    u32(WAV_HEADER_BYTES - 8 + data)
    ascii("WAVEfmt ")
    u32(16)
    u16(1) // PCM
    u16(1) // mono
    u32(sampleRateHz)
    u32(sampleRateHz * 2)
    u16(2)
    u16(16)
    ascii("data")
    u32(data)
    return at
}

/** One float sample as 16-bit PCM, clipped rather than wrapped. */
internal fun pcm16(sample: Float): Short = (sample.coerceIn(-1f, 1f) * Short.MAX_VALUE).toInt().toShort()
