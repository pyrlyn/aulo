package aulo.speech

import aulo.types.AuloEvent
import kotlin.jvm.JvmInline

/*
 * Why the engines are push, then poll, and never suspend:
 *
 * - It matches the engines. sherpa-onnx streams are push-then-decode, VADs and keyword spotters
 *   answer per frame, and TTS engines produce audio in chunks.
 * - Network engines still fit. A cloud or plugin engine owns a background coroutine and two
 *   bounded queues: `push` is a non-blocking offer that reports dropped samples when the queue is
 *   full, and `poll` a non-blocking take. The bounded queues are the backpressure, so nothing on
 *   the audio path grows without limit.
 * - Output goes into caller buffers: STT into a reused Transcript, TTS into a caller FloatArray,
 *   so steady-state polling allocates nothing on the caller side.
 *
 * Local inference can still take tens of milliseconds of CPU in `poll` or `finish`, so the
 * pipeline drives engines from its own speech worker, never from the audio device callback, which
 * only fills a ring buffer.
 */

/** Longest engine id, in bytes. Ids appear in config keys, CLI arguments and notices, so they stay short. */
const val MAX_ENGINE_ID_BYTES = 64

/** Longest engine or voice name and voice id, in UTF-8 bytes; `aulo.v1.Voice` caps voice ids the same. */
const val MAX_LABEL_BYTES = 128

/** Most languages an engine may list; plugin engines supply the list. */
const val MAX_LANGUAGES = 256

private const val ID_PUNCTUATION = "-_."

/**
 * The id config and the CLI use to pick an engine (`engine = "sherpa-kokoro"`).
 *
 * Plugins choose their own ids, so ids are untrusted: lowercase ASCII letters, digits, `-`, `_`
 * and `.`, starting with a letter, which keeps them safe to print and to use as TOML keys.
 */
@JvmInline
value class EngineId(
    val value: String,
) {
    init {
        if (value.isEmpty() || value.length > MAX_ENGINE_ID_BYTES) {
            throw SpeechException.Invalid("engine id", "must be 1 to $MAX_ENGINE_ID_BYTES bytes")
        }
        if (value[0] !in 'a'..'z') throw SpeechException.Invalid("engine id", "must start with a-z")
        if (!value.all { it in 'a'..'z' || it in '0'..'9' || it in ID_PUNCTUATION }) {
            throw SpeechException.Invalid("engine id", "only a-z, 0-9, '-', '_' and '.'")
        }
    }

    override fun toString(): String = value
}

/** Which interface the engine implements; the registry keeps one list per kind. */
enum class EngineKind { STT, TTS, VAD, KEYWORD_SPOTTER, TURN_DETECTOR }

/** Languages an engine handles, as BCP 47 tags. */
sealed interface LanguageSupport {
    /**
     * Whether the engine handles [tag]. Primary subtags are compared, ignoring case, so a request
     * for `en-US` is served by an engine listing `en` and the other way round: the region is the
     * voice's choice, not the engine's.
     */
    fun supports(tag: String): Boolean

    /** Detects or accepts any language. */
    data object Multilingual : LanguageSupport {
        override fun supports(tag: String) = true
    }

    data class Listed(
        val tags: List<String>,
    ) : LanguageSupport {
        init {
            if (tags.isEmpty() || tags.size > MAX_LANGUAGES) {
                throw SpeechException.Invalid("language list", "must hold 1 to $MAX_LANGUAGES tags")
            }
            tags.forEach { requireLanguageTag("language", it) }
        }

        override fun supports(tag: String) = tags.any { primarySubtag(it).equals(primarySubtag(tag), ignoreCase = true) }
    }
}

private fun primarySubtag(tag: String) = tag.substringBefore('-')

data class Capabilities(
    /**
     * STT: partial transcripts arrive while audio is still coming in. TTS: the first audio
     * arrives before all text is synthesized. Other engines answer only after `finish`.
     */
    val streaming: Boolean,
    val languages: LanguageSupport,
    /** Works with no network at all once its models are on disk. */
    val offline: Boolean,
    /**
     * Sends audio or text over a socket to another process or host. This is the privacy flag,
     * and it is not `!offline`: an OpenAI-compatible server on localhost needs the network stack
     * but works offline.
     */
    val needsNetwork: Boolean,
)

/**
 * What an engine says about itself. The registry selects and falls back on it without calling
 * the engine, and clients show it in the engine picker.
 */
data class EngineInfo(
    val id: EngineId,
    val kind: EngineKind,
    /** For pickers only; never used to select an engine. Untrusted when a plugin supplies it. */
    val name: String,
    val capabilities: Capabilities,
) {
    init {
        requireLabel("engine name", name)
    }
}

/**
 * What every engine shares. Engines may hold native models, sockets or a plugin process, and
 * Kotlin/Native has no destructors, so the registry closes an engine it drops.
 */
interface SpeechEngine : AutoCloseable {
    val info: EngineInfo

    override fun close() {}
}

/** Untrusted names and ids are shown in pickers and logs: bounded, one line. */
internal fun requireLabel(
    what: String,
    value: String,
) {
    if (value.isEmpty() || value.encodeToByteArray().size > MAX_LABEL_BYTES) {
        throw SpeechException.Invalid(what, "must be 1 to $MAX_LABEL_BYTES bytes")
    }
    if (value.any { it.isISOControl() }) throw SpeechException.Invalid(what, "contains a control character")
}

/** A BCP 47 tag: ASCII letters, digits and `-`, no longer than an aulo event allows. */
internal fun requireLanguageTag(
    what: String,
    tag: String,
) {
    if (tag.isEmpty() || tag.length > AuloEvent.Transcript.MAX_LANGUAGE_BYTES) {
        throw SpeechException.Invalid(what, "must be 1 to ${AuloEvent.Transcript.MAX_LANGUAGE_BYTES} bytes")
    }
    if (!tag.all { it in 'a'..'z' || it in 'A'..'Z' || it in '0'..'9' || it == '-' }) {
        throw SpeechException.Invalid(what, "is not a BCP 47 tag")
    }
}
