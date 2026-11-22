package aulo.types

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * aulo's own events: voice and safety. They mirror the first block of `Event` in
 * proto/aulo/v1/event.proto field for field. Turn, text and tool-call events belong to the
 * agent loop (T1.15) and are not repeated here.
 *
 * Text fields that carry untrusted content are capped as the proto documents. The
 * constructors reject longer values, so oversized input decoded from the wire fails closed;
 * the `capped` factories truncate instead, for producers that own the text.
 */
@Serializable
sealed interface AuloEvent {
    @Serializable
    @SerialName("voice_state")
    data class VoiceStateChanged(
        val state: VoiceState,
    ) : AuloEvent

    @Serializable
    @SerialName("speech_started")
    data class SpeechStarted(
        // Offset into the capture stream, so clients can align it with audio.
        val atMs: ULong,
    ) : AuloEvent

    @Serializable
    @SerialName("speech_ended")
    data class SpeechEnded(
        val atMs: ULong,
    ) : AuloEvent

    @Serializable
    @SerialName("transcript")
    data class Transcript(
        val turnId: TurnId,
        val kind: TranscriptKind,
        val text: String,
        val language: String? = null,
    ) : AuloEvent {
        init {
            requireUtf8AtMost("text", text, MAX_TEXT_BYTES)
            language?.let { requireUtf8AtMost("language", it, MAX_LANGUAGE_BYTES) }
        }

        companion object {
            // STT output of whatever was said or played near the microphone.
            const val MAX_TEXT_BYTES = 64 * 1024

            // A BCP 47 tag; RFC 5646 §4.4.1 advises supporting at least 35 characters.
            const val MAX_LANGUAGE_BYTES = 35

            fun capped(
                turnId: TurnId,
                kind: TranscriptKind,
                text: String,
                language: String? = null,
            ) = Transcript(turnId, kind, text.truncateUtf8(MAX_TEXT_BYTES), language?.truncateUtf8(MAX_LANGUAGE_BYTES))
        }
    }

    /** Metadata of one synthesized chunk; the samples travel separately so events stay small. */
    @Serializable
    @SerialName("tts_chunk")
    data class TtsChunk(
        val turnId: TurnId,
        // Position in the reply, so a client can detect a dropped chunk.
        val seq: UInt,
        val sampleRateHz: UInt,
        val channels: UByte,
        val durationMs: UInt,
        val last: Boolean,
    ) : AuloEvent

    /** Policy answered Ask. Voice never widens access: with [needsClick] only a click or key approves. */
    @Serializable
    @SerialName("approval_required")
    data class ApprovalRequired(
        val chatId: ChatId,
        val turnId: TurnId,
        val callId: CallId,
        val tool: String,
        val summary: String,
        val needsClick: Boolean,
    ) : AuloEvent {
        init {
            requireUtf8AtMost("tool", tool, MAX_TOOL_BYTES)
            requireUtf8AtMost("summary", summary, MAX_SUMMARY_BYTES)
        }

        companion object {
            // Plugin and MCP tool names come from third parties.
            const val MAX_TOOL_BYTES = 128

            // Built from model-chosen arguments; rendered as plain text.
            const val MAX_SUMMARY_BYTES = 2 * 1024

            fun capped(
                chatId: ChatId,
                turnId: TurnId,
                callId: CallId,
                tool: String,
                summary: String,
                needsClick: Boolean,
            ) = ApprovalRequired(
                chatId,
                turnId,
                callId,
                tool.truncateUtf8(MAX_TOOL_BYTES),
                summary.truncateUtf8(MAX_SUMMARY_BYTES),
                needsClick,
            )
        }
    }

    @Serializable
    @SerialName("takeover_requested")
    data class TakeoverRequested(
        val chatId: ChatId,
        val callId: CallId,
        val reason: TakeoverReason,
    ) : AuloEvent

    @Serializable
    @SerialName("notice")
    data class Notice(
        val level: NoticeLevel,
        val message: String,
        // The engine, server or plugin the notice is about.
        val source: String? = null,
    ) : AuloEvent {
        init {
            requireUtf8AtMost("message", message, MAX_MESSAGE_BYTES)
            source?.let { requireUtf8AtMost("source", it, MAX_SOURCE_BYTES) }
        }

        companion object {
            // Untrusted when it quotes a plugin, MCP server or engine.
            const val MAX_MESSAGE_BYTES = 2 * 1024

            // Same bound as a tool name: it is a third-party name too.
            const val MAX_SOURCE_BYTES = 128

            fun capped(
                level: NoticeLevel,
                message: String,
                source: String? = null,
            ) = Notice(level, message.truncateUtf8(MAX_MESSAGE_BYTES), source?.truncateUtf8(MAX_SOURCE_BYTES))
        }
    }
}

@Serializable
enum class VoiceState { IDLE, LISTENING, THINKING, SPEAKING }

@Serializable
enum class TranscriptKind {
    // A streaming hypothesis the engine may revise.
    PARTIAL,

    // The engine's committed text for the utterance.
    FINAL,
}

/** What a human has to do themselves because the agent must not. */
@Serializable
enum class TakeoverReason { PASSWORD, TWO_FACTOR, CAPTCHA, OTHER }

@Serializable
enum class NoticeLevel { INFO, WARN, ERROR }
