package aulo.speech

import aulo.types.AuloEvent
import aulo.types.TranscriptKind
import aulo.types.TurnId
import aulo.types.truncateUtf8

private const val DEFAULT_TRANSCRIPT_CAPACITY = 256

/**
 * The latest transcript of an utterance, kept by the caller across polls. The engine overwrites
 * it in place, so streaming partials reuses one buffer for the whole utterance.
 *
 * Engine text is untrusted (a cloud service or a plugin can send anything), so [set] caps it at
 * the limits of [AuloEvent.Transcript] and [toEvent] can never fail on it.
 */
class Transcript(
    turnId: TurnId,
    capacity: Int = DEFAULT_TRANSCRIPT_CAPACITY,
) {
    /**
     * The turn given to [SttEngine.begin]. A network engine can deliver a result after the
     * pipeline moved on (barge-in, cancel); the stamp lets the pipeline drop such stale text
     * instead of giving it to the new turn.
     */
    var turnId: TurnId = turnId
        private set

    var kind: TranscriptKind = TranscriptKind.PARTIAL
        private set

    private val buffer = StringBuilder(capacity)

    val text: CharSequence get() = buffer

    /** BCP 47 tag when the engine detected or was told the language. */
    var language: String? = null
        private set

    /** Overwrites the transcript; engines update through this rather than building new strings. */
    fun set(
        turnId: TurnId,
        kind: TranscriptKind,
        text: CharSequence,
        language: String?,
    ) {
        this.turnId = turnId
        this.kind = kind
        buffer.clear()
        buffer.appendUtf8Capped(text, AuloEvent.Transcript.MAX_TEXT_BYTES)
        // Engines pass the same short tag every time, so this rarely allocates.
        this.language = language?.truncateUtf8(AuloEvent.Transcript.MAX_LANGUAGE_BYTES)
    }

    fun toEvent(): AuloEvent.Transcript = AuloEvent.Transcript(turnId, kind, buffer.toString(), language)
}

/**
 * Appends the longest prefix of [text] that fits [maxBytes] of UTF-8, without encoding it, so a
 * partial transcript costs no garbage. A lone surrogate counts as the three bytes of the
 * replacement character it encodes to.
 */
internal fun StringBuilder.appendUtf8Capped(
    text: CharSequence,
    maxBytes: Int,
) {
    var bytes = 0
    var i = 0
    while (i < text.length) {
        val c = text[i]
        val pair = c.isHighSurrogate() && i + 1 < text.length && text[i + 1].isLowSurrogate()
        val size =
            when {
                pair -> 4
                c.code < 0x80 -> 1
                c.code < 0x800 -> 2
                else -> 3
            }
        if (bytes + size > maxBytes) return
        bytes += size
        val end = if (pair) i + 2 else i + 1
        append(text, i, end)
        i = end
    }
}

/** What a poll produced. */
enum class SttPoll {
    /** Nothing new; the transcript is untouched. */
    PENDING,

    /** The transcript holds a newer result; check its kind. */
    UPDATED,

    /** The final transcript was already delivered; the utterance is over. */
    DONE,
}

/**
 * Speech-to-text. One utterance at a time: [begin], any number of [push] and [poll], then
 * [finish], then [poll] until [SttPoll.DONE]. Exactly one final transcript is delivered per
 * finished utterance (its text may be empty), and only after [finish]: end of turn is the
 * [TurnDetector]'s call, not the STT engine's. [begin] may be called at any point and implies
 * [cancel].
 */
interface SttEngine : SpeechEngine {
    /**
     * Starts an utterance. [language] is a BCP 47 hint; null asks the engine to detect it. A
     * language the engine does not handle throws [SpeechException.Unsupported] so the registry
     * can fall back.
     */
    fun begin(
        turnId: TurnId,
        language: String?,
    )

    /**
     * Hands the engine one frame and returns how many per-channel samples it dropped because its
     * bounded queue was full: 0 when it took them all. Must not block.
     */
    fun push(frame: AudioFrame): Int

    /** No more audio for this utterance; the final transcript follows. */
    fun finish()

    /** Writes the newest transcript into [out] if there is one. Must not block. */
    fun poll(out: Transcript): SttPoll

    /** Drops the utterance and anything queued. Must return at once: barge-in has 150 ms. */
    fun cancel()
}
