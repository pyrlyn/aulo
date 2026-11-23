package aulo.proto

import aulo.v1.ConverseRequest
import aulo.v1.ConverseResponse
import aulo.v1.CreateChatRequest
import aulo.v1.Event
import aulo.v1.ModelRef
import aulo.v1.SessionServiceServer
import aulo.v1.TextDelta
import aulo.v1.UserTurn
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ReceiveChannel
import kotlinx.coroutines.channels.SendChannel
import kotlinx.coroutines.channels.toList
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNull

class GeneratedCodeTest {
    private fun bytes(vararg values: Int): ByteArray = ByteArray(values.size) { values[it].toByte() }

    // Field 2 (user_turn), length 4, holding field 1 (text), length 2, "hi".
    private val userTurnHi = bytes(0x12, 0x04, 0x0A, 0x02, 0x68, 0x69)

    @Test
    fun a_message_is_encoded_with_the_field_numbers_of_the_proto() {
        assertContentEquals(userTurnHi, ConverseRequest(user_turn = UserTurn(text = "hi")).encode())
    }

    @Test
    fun a_message_survives_a_round_trip() {
        val request = CreateChatRequest(title = "Trip to Örebro", model = ModelRef(provider = "local", model = "m"))
        val decoded = CreateChatRequest.ADAPTER.decode(request.encode())
        assertEquals(request, decoded)
        assertNull(decoded.bot_id, "an optional field that was not set stays unset")
    }

    @Test
    fun a_oneof_holds_the_one_case_that_was_sent() {
        val decoded = ConverseRequest.ADAPTER.decode(userTurnHi)
        assertEquals("hi", decoded.user_turn?.text)
        assertNull(decoded.attach)
    }

    @Test
    fun a_field_this_build_does_not_know_is_kept() {
        // Field 999, a varint 1: what a newer peer may send.
        val newer = userTurnHi + bytes(0xB8, 0x3E, 0x01)
        val decoded = ConverseRequest.ADAPTER.decode(newer)
        assertEquals("hi", decoded.user_turn?.text)
        assertContentEquals(newer, decoded.encode())
    }

    @Test
    fun a_streaming_service_can_be_implemented_and_called() {
        val echo =
            object : SessionServiceServer {
                override suspend fun Converse(
                    request: ReceiveChannel<ConverseRequest>,
                    response: SendChannel<ConverseResponse>,
                ) {
                    for (submission in request) {
                        val text = submission.user_turn?.text.orEmpty()
                        response.send(ConverseResponse(event = Event(text_delta = TextDelta(text = text))))
                    }
                    response.close()
                }
            }
        val replies =
            runBlocking {
                val requests = Channel<ConverseRequest>(Channel.UNLIMITED)
                val responses = Channel<ConverseResponse>(Channel.UNLIMITED)
                launch { echo.Converse(requests, responses) }
                requests.send(ConverseRequest(user_turn = UserTurn(text = "one")))
                requests.send(ConverseRequest(user_turn = UserTurn(text = "two")))
                requests.close()
                responses.toList().map { it.event?.text_delta?.text }
            }
        assertEquals(listOf("one", "two"), replies)
    }
}
