package aulo.speech

// Detectors watch the capture stream frame by frame. They are cheap enough to answer on every
// push, so unlike STT and TTS they return their result directly instead of being polled.

/** A change in voice activity, stamped with the stream position ([AudioFrame.position]) where it happened. */
sealed interface VadEvent {
    val position: Long

    data class SpeechStarted(
        override val position: Long,
    ) : VadEvent

    data class SpeechEnded(
        override val position: Long,
    ) : VadEvent
}

/**
 * Voice activity detection. At most one event per pushed frame: engines with minimum speech
 * and silence durations never flip twice inside one 20 ms frame.
 */
interface Vad : SpeechEngine {
    /** Feeds one frame; frames must be contiguous. Engines with another window size buffer in fixed storage. */
    fun push(frame: AudioFrame): VadEvent?

    /** Forgets all state, for example after the microphone changed owner. */
    fun reset()
}

data class KeywordHit(
    /** Index into the list given to [KeywordSpotter.setKeywords]; the pipeline maps it to the bot or chat it selects. */
    val keyword: Int,
    /** Stream position where the keyword ended. */
    val position: Long,
)

/** Wake-word detection over an open vocabulary. */
interface KeywordSpotter : SpeechEngine {
    /** Replaces the keywords. Called on config changes, never on the audio path, so engines may rebuild models here. */
    fun setKeywords(phrases: List<String>)

    fun push(frame: AudioFrame): KeywordHit?

    fun reset()
}

/** The detector's verdict after a frame. */
sealed interface TurnDecision {
    /** The user may still be talking. */
    data object Continue : TurnDecision

    /** The turn ended at this stream position; the pipeline calls [SttEngine.finish]. */
    data class EndOfTurn(
        val position: Long,
    ) : TurnDecision
}

/** End of turn: a silence timeout, optionally refined by a semantic model on the audio or the transcript. */
interface TurnDetector : SpeechEngine {
    /** Feeds one frame with the VAD's verdict, so detectors build on the selected VAD instead of running their own. */
    fun push(
        frame: AudioFrame,
        inSpeech: Boolean,
    ): TurnDecision

    /** The latest partial transcript, for detectors that judge text; audio-only detectors ignore it. */
    fun observeTranscript(text: CharSequence) {}

    /** Starts over for the next turn. */
    fun reset()
}
