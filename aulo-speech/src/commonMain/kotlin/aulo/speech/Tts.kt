package aulo.speech

import aulo.types.TurnId
import kotlin.jvm.JvmInline

/** A voice an engine offers, as `aulo.v1.Voice` lists it. Cloud and plugin voices are untrusted. */
data class Voice(
    /** What `[voice.tts] voice` and `aulo voice use` name. */
    val id: String,
    val name: String,
    /** BCP 47 tag of the voice's language, when the engine knows it. */
    val language: String? = null,
) {
    init {
        requireLabel("voice id", id)
        requireLabel("voice name", name)
        language?.let { requireLanguageTag("voice language", it) }
    }
}

/** Speaking rate as a multiple of the voice's normal pace. */
@JvmInline
value class SpeechRate(
    val value: Float,
) {
    init {
        // The range check also rejects NaN, which is outside every range.
        if (value !in MIN..MAX) throw SpeechException.Invalid("speech rate", "must be within $MIN..$MAX")
    }

    companion object {
        /** Bounds most engines accept; beyond them speech is hard to follow. */
        const val MIN = 0.5f
        const val MAX = 2.0f
        val NORMAL = SpeechRate(1.0f)
    }
}

/** How to speak one reply. */
data class TtsRequest(
    val turnId: TurnId,
    /** A [Voice.id]; null takes the engine's default for [language]. */
    val voice: String? = null,
    /** BCP 47 tag of the text, from the normalizer's language detection. */
    val language: String? = null,
    val rate: SpeechRate = SpeechRate.NORMAL,
)

/**
 * What a poll produced: nothing yet, some audio, or the end. A value class so polling every
 * 20 ms allocates nothing.
 */
@JvmInline
value class TtsPoll private constructor(
    private val raw: Int,
) {
    val isPending: Boolean get() = raw == 0
    val isDone: Boolean get() = raw < 0

    /** How many interleaved samples at the start of the caller's array hold new audio. */
    val samples: Int get() = if (raw > 0) raw else 0

    override fun toString(): String =
        when {
            isPending -> "Pending"
            isDone -> "Done"
            else -> "Audio($raw)"
        }

    companion object {
        /** Nothing ready yet; the array is untouched. */
        val PENDING = TtsPoll(0)

        /** All audio of the reply was delivered, or the reply was cancelled. */
        val DONE = TtsPoll(-1)

        fun audio(samples: Int): TtsPoll {
            if (samples <= 0) throw SpeechException.Invalid("tts poll", "audio must hold at least one sample")
            return TtsPoll(samples)
        }
    }
}

/**
 * Text-to-speech. One reply at a time: [begin], any number of [pushText] and [poll], then
 * [finish], then [poll] until [TtsPoll.isDone]. Text pushed before [finish] may already be
 * speaking, which is how the first sentence plays while the rest is generated. [begin] may be
 * called at any point and implies [cancel]. Audio goes back to the pipeline rather than to the
 * speaker, so barge-in and echo cancellation see every sample.
 */
interface TtsEngine : SpeechEngine {
    /** Voices known when the engine was loaded; cloud engines fetch their list up front so this never blocks. */
    val voices: List<Voice>

    /**
     * Starts a reply and returns the format [poll] writes in. An unknown voice or language throws
     * [SpeechException.Unsupported] so the registry can fall back.
     */
    fun begin(request: TtsRequest): AudioFormat

    /** Queues more text, usually one sentence. Must not block. */
    fun pushText(text: String)

    /** No more text for this reply. */
    fun finish()

    /**
     * Writes ready audio into [out]: at most `out.size` samples, always whole channel groups.
     * Must not block.
     */
    fun poll(out: FloatArray): TtsPoll

    /** Drops queued text and audio; the next poll is done. Must return at once: barge-in has 150 ms. */
    fun cancel()
}
