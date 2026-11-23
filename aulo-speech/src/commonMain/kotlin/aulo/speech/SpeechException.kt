package aulo.speech

import aulo.types.truncateUtf8

/** Longest engine-supplied detail kept in an error, in UTF-8 bytes. */
const val MAX_ERROR_DETAIL_BYTES = 512

/**
 * The one error every engine throws. It is shared rather than one type per engine because the
 * registry holds engines of different origins side by side, and a gRPC plugin can only report a
 * category and a message anyway. The category decides fallback, which is the fail-open rule for
 * voice (spec §5.2).
 *
 * A full input queue is not an error: `push` reports dropped samples in its return value,
 * because the pipeline counts them and goes on.
 */
sealed class SpeechException(
    message: String,
    /**
     * True when the registry should warn and hand the work to the next engine in the fallback
     * list. Caller bugs and bad values stay with this engine: switching would not fix them.
     */
    val shouldFallBack: Boolean,
) : Exception(message) {
    /** The engine cannot run here: model missing, platform API absent, no key, plugin process down. */
    class Unavailable(
        detail: String,
    ) : SpeechException("engine unavailable: ${engineDetail(detail)}", shouldFallBack = true)

    /** The engine runs, but not for this request: a language, voice or audio format it does not handle. */
    class Unsupported(
        detail: String,
    ) : SpeechException("not supported: ${engineDetail(detail)}", shouldFallBack = true)

    /** The engine broke while working: decode error, network drop, bad reply. */
    class Failed(
        detail: String,
    ) : SpeechException("engine failed: ${engineDetail(detail)}", shouldFallBack = true)

    /** A value failed validation. */
    class Invalid(
        what: String,
        reason: String,
    ) : SpeechException("invalid $what: $reason", shouldFallBack = false)

    /** The caller broke the call order, for example pushing before `begin`. */
    class OutOfOrder(
        what: String,
    ) : SpeechException("called out of order: $what", shouldFallBack = false)
}

/**
 * Details come from cloud services and plugins, which are untrusted, and end up in notices and
 * logs: capping stops an engine flooding them, and flattening to one line stops it forging
 * extra log lines or terminal escapes.
 */
internal fun engineDetail(text: String): String =
    text.truncateUtf8(MAX_ERROR_DETAIL_BYTES).map { if (it.isISOControl()) ' ' else it }.joinToString("")
