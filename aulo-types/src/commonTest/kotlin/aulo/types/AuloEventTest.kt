package aulo.types

import kotlinx.serialization.json.Json
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class AuloEventTest {
    private val chat = ChatId.new()
    private val turn = TurnId.new()
    private val call = CallId.new()

    private val every: List<AuloEvent> =
        listOf(
            AuloEvent.VoiceStateChanged(VoiceState.LISTENING),
            AuloEvent.SpeechStarted(12u),
            AuloEvent.SpeechEnded(ULong.MAX_VALUE),
            AuloEvent.Transcript(turn, TranscriptKind.PARTIAL, "hello"),
            AuloEvent.Transcript(turn, TranscriptKind.FINAL, "привет", language = "ru"),
            AuloEvent.TtsChunk(turn, seq = 3u, sampleRateHz = 24_000u, channels = 1u, durationMs = 40u, last = true),
            AuloEvent.ApprovalRequired(chat, turn, call, tool = "shell", summary = "rm -rf build", needsClick = true),
            AuloEvent.TakeoverRequested(chat, call, TakeoverReason.TWO_FACTOR),
            AuloEvent.Notice(NoticeLevel.WARN, "engine fell back", source = "sherpa"),
            AuloEvent.Notice(NoticeLevel.INFO, "ready"),
        )

    @Test
    fun everyEventRoundTripsThroughJson() {
        for (event in every) {
            val json = Json.encodeToString(AuloEvent.serializer(), event)
            assertEquals(event, Json.decodeFromString(AuloEvent.serializer(), json), json)
        }
    }

    @Test
    fun theDiscriminatorMatchesTheProtoFieldName() {
        val json = Json.encodeToString(AuloEvent.serializer(), AuloEvent.VoiceStateChanged(VoiceState.IDLE))
        assertEquals("""{"type":"voice_state","state":"IDLE"}""", json)
    }

    @Test
    fun cappedFactoriesTruncateWithoutSplittingCharacters() {
        // Three-byte characters: the cap falls inside one, which must be dropped whole.
        val notice = AuloEvent.Notice.capped(NoticeLevel.ERROR, "€".repeat(1_000), source = "x".repeat(500))
        assertEquals(682, notice.message.length)
        assertTrue(notice.message.encodeToByteArray().size <= AuloEvent.Notice.MAX_MESSAGE_BYTES)
        assertEquals(AuloEvent.Notice.MAX_SOURCE_BYTES, notice.source!!.length)

        val approval = AuloEvent.ApprovalRequired.capped(chat, turn, call, "t".repeat(200), "s".repeat(3_000), false)
        assertEquals(AuloEvent.ApprovalRequired.MAX_TOOL_BYTES, approval.tool.length)
        assertEquals(AuloEvent.ApprovalRequired.MAX_SUMMARY_BYTES, approval.summary.length)

        val transcript = AuloEvent.Transcript.capped(turn, TranscriptKind.FINAL, "a".repeat(70_000), "x".repeat(40))
        assertEquals(AuloEvent.Transcript.MAX_TEXT_BYTES, transcript.text.length)
        assertEquals(AuloEvent.Transcript.MAX_LANGUAGE_BYTES, transcript.language!!.length)
        assertEquals("ab", "ab".truncateUtf8(5))
        assertEquals("", "€".truncateUtf8(2))
    }

    @Test
    fun oversizedTextIsRejectedOnConstructionAndDecode() {
        val long = "x".repeat(AuloEvent.Notice.MAX_MESSAGE_BYTES + 1)
        assertFailsWith<IllegalArgumentException> { AuloEvent.Notice(NoticeLevel.INFO, long) }
        assertFailsWith<IllegalArgumentException> {
            Json.decodeFromString(AuloEvent.serializer(), """{"type":"notice","level":"INFO","message":"$long"}""")
        }
        assertFailsWith<IllegalArgumentException> { AuloEvent.ApprovalRequired(chat, turn, call, "t".repeat(129), "", true) }
    }

    @Test
    fun invalidIdsInsideEventsAreRejectedOnDecode() {
        val bad = """{"type":"takeover_requested","chatId":"nope","callId":"$call","reason":"CAPTCHA"}"""
        assertFailsWith<IllegalArgumentException> { Json.decodeFromString(AuloEvent.serializer(), bad) }
    }
}
