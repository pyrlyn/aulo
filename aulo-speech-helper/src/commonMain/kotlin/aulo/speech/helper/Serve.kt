package aulo.speech.helper

import aulo.speech.PIPELINE_SAMPLE_RATE_HZ
import okio.BufferedSink
import okio.BufferedSource

/** Turns the audio of one utterance into text: the helper's model. */
fun interface UtteranceDecoder {
    /** Text of the first [count] samples of [samples] (mono, 16 kHz); [language] is the hint, if any. */
    fun decode(
        samples: FloatArray,
        count: Int,
        language: String?,
    ): String
}

data class ServeOptions(
    /** Longest utterance kept; audio past it is dropped and the final covers what was kept. */
    val maxUtteranceSamples: Int = PIPELINE_SAMPLE_RATE_HZ * 60,
    /** Decode a partial after every this many new samples; null sends only finals. */
    val partialEverySamples: Int? = null,
    /**
     * No partials once the utterance is this long: an offline model decodes the whole buffer
     * each time, so the cost of partials grows with the square of the length.
     */
    val maxPartialSamples: Int = PIPELINE_SAMPLE_RATE_HZ * 15,
) {
    init {
        require(maxUtteranceSamples > 0) { "maxUtteranceSamples must be positive" }
        require(partialEverySamples == null || partialEverySamples > 0) { "partialEverySamples must be positive" }
    }
}

/**
 * The helper's side of the pipe protocol: tells aulod the model is [FrameType.READY], then
 * answers frames until aulod closes the pipe. Only the utterance begun last is decoded; frames
 * of any other utterance are ignored, which is what makes cancel cheap on aulod's side. A
 * failing decode answers [FrameType.ERROR] for that utterance and keeps serving.
 *
 * Throws [WireException] when aulod's side breaks the protocol, so the helper exits.
 */
fun serveStt(
    input: BufferedSource,
    output: BufferedSink,
    decoder: UtteranceDecoder,
    options: ServeOptions = ServeOptions(),
) {
    val buffer = FloatArray(options.maxUtteranceSamples)
    var seq = -1
    var count = 0
    var language: String? = null
    var partialAt = 0
    output.writeFrame(FrameType.READY, 0)
    output.flush()

    fun answer(
        type: FrameType,
        text: () -> String,
    ) {
        val reply =
            try {
                type to text()
            } catch (e: Exception) {
                FrameType.ERROR to "decode failed: ${e.message ?: e::class.simpleName}"
            }
        output.writeText(reply.first, seq, reply.second)
        output.flush()
    }

    while (true) {
        val frame = input.readFrame() ?: return
        when (frame.type) {
            FrameType.BEGIN -> {
                seq = frame.seq
                count = 0
                partialAt = 0
                language = frame.text.ifEmpty { null }
            }

            FrameType.AUDIO -> {
                if (frame.seq != seq) continue
                val samples = frame.samples()
                val take = minOf(samples.size, buffer.size - count)
                samples.copyInto(buffer, count, 0, take)
                count += take
                val every = options.partialEverySamples ?: continue
                if (count - partialAt >= every && count <= options.maxPartialSamples) {
                    partialAt = count
                    answer(FrameType.PARTIAL) { decoder.decode(buffer, count, language) }
                }
            }

            FrameType.FINISH -> {
                if (frame.seq != seq) continue
                answer(FrameType.FINAL) { if (count == 0) "" else decoder.decode(buffer, count, language) }
                seq = -1
            }

            FrameType.CANCEL -> {
                if (frame.seq == seq) seq = -1
            }

            else -> {
                throw WireException("${frame.type.name} is not a request")
            }
        }
    }
}
