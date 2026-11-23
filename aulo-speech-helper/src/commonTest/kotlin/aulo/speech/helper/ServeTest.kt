package aulo.speech.helper

import okio.Buffer
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class ServeTest {
    /** Says how many samples it heard and the language hint. */
    private val counter = UtteranceDecoder { _, count, language -> "$count${language?.let { " $it" }.orEmpty()}" }

    private fun serve(
        options: ServeOptions = ServeOptions(),
        decoder: UtteranceDecoder = counter,
        requests: Buffer.() -> Unit,
    ): List<Pair<FrameType, String>> {
        val input = Buffer().apply(requests)
        val output = Buffer()
        serveStt(input, output, decoder, options)
        return generateSequence { output.readFrame() }.map { it.type to "${it.seq}:${it.text}" }.toList()
    }

    @Test
    fun readyThenOneFinalPerFinishedUtterance() {
        val replies =
            serve {
                writeFrame(FrameType.BEGIN, 1, "ru".encodeToByteArray())
                writeAudio(1, FloatArray(320))
                writeAudio(1, FloatArray(320))
                writeFrame(FrameType.FINISH, 1)
                writeFrame(FrameType.BEGIN, 2)
                writeFrame(FrameType.FINISH, 2)
            }
        assertEquals(listOf(FrameType.READY to "0:", FrameType.FINAL to "1:640 ru", FrameType.FINAL to "2:"), replies)
    }

    @Test
    fun aCancelledOrStaleUtteranceGetsNoReply() {
        val replies =
            serve {
                writeFrame(FrameType.BEGIN, 1)
                writeAudio(1, FloatArray(10))
                writeFrame(FrameType.CANCEL, 1)
                writeFrame(FrameType.FINISH, 1)
                writeFrame(FrameType.BEGIN, 2)
                // Audio of the old utterance arriving late must not join the new one.
                writeAudio(1, FloatArray(10))
                writeAudio(2, FloatArray(5))
                writeFrame(FrameType.FINISH, 2)
            }
        assertEquals(listOf(FrameType.READY to "0:", FrameType.FINAL to "2:5"), replies)
    }

    @Test
    fun audioPastTheCapIsDropped() {
        val replies =
            serve(ServeOptions(maxUtteranceSamples = 100)) {
                writeFrame(FrameType.BEGIN, 1)
                writeAudio(1, FloatArray(80))
                writeAudio(1, FloatArray(80))
                writeFrame(FrameType.FINISH, 1)
            }
        assertEquals(FrameType.FINAL to "1:100", replies.last())
    }

    @Test
    fun partialsComeAtTheIntervalUpToTheirLimit() {
        val replies =
            serve(ServeOptions(partialEverySamples = 100, maxPartialSamples = 300)) {
                writeFrame(FrameType.BEGIN, 1)
                repeat(8) { writeAudio(1, FloatArray(50)) }
                writeFrame(FrameType.FINISH, 1)
            }
        assertEquals(
            listOf(
                FrameType.READY to "0:",
                FrameType.PARTIAL to "1:100",
                FrameType.PARTIAL to "1:200",
                FrameType.PARTIAL to "1:300",
                FrameType.FINAL to "1:400",
            ),
            replies,
        )
    }

    @Test
    fun aFailingDecodeAnswersAnErrorAndServingGoesOn() {
        var calls = 0
        val flaky = UtteranceDecoder { _, _, _ -> if (calls++ == 0) error("model blew up") else "ok" }
        val replies =
            serve(decoder = flaky) {
                writeFrame(FrameType.BEGIN, 1)
                writeAudio(1, FloatArray(1))
                writeFrame(FrameType.FINISH, 1)
                writeFrame(FrameType.BEGIN, 2)
                writeAudio(2, FloatArray(1))
                writeFrame(FrameType.FINISH, 2)
            }
        assertEquals(
            listOf(FrameType.READY to "0:", FrameType.ERROR to "1:decode failed: model blew up", FrameType.FINAL to "2:ok"),
            replies,
        )
    }

    @Test
    fun aReplyTypeFromAuloIsAProtocolError() {
        assertFailsWith<WireException> { serve { writeFrame(FrameType.READY, 0) } }
    }
}
