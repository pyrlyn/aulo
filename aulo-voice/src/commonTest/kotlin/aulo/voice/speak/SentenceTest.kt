package aulo.voice.speak

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Sentences come out as soon as they are certain, however the text is cut into deltas. */
class SentenceTest {
    private fun splitBy(
        text: String,
        chunk: Int,
        maxChunk: Int,
    ): List<String> {
        val splitter = SentenceSplitter(maxChunk)
        return text.chunked(chunk).flatMap(splitter::push) + listOfNotNull(splitter.finish())
    }

    @Test fun splitsTheSameWayForAnyDeltaSize() {
        val text =
            "Hello there. How are you? Fine! Dr. Smith said 3.14 is pi. " +
                "Ну что… Привет, мир! J. Doe (a \"quote.\") left. Last one"
        val whole = splitBy(text, Int.MAX_VALUE, 500)
        assertEquals(
            listOf(
                "Hello there.",
                "How are you?",
                "Fine!",
                "Dr. Smith said 3.14 is pi.",
                "Ну что…",
                "Привет, мир!",
                "J. Doe (a \"quote.\")",
                "left.",
                "Last one",
            ),
            whole,
        )
        for (chunk in listOf(1, 2, 3, 7)) assertEquals(whole, splitBy(text, chunk, 500), "delta size $chunk")
    }

    @Test fun aTerminatorAtTheEndWaitsForTheNextDelta() {
        val splitter = SentenceSplitter(100)
        assertTrue(splitter.push("Pi is 3.").isEmpty())
        assertTrue(splitter.push("14 roughly.").isEmpty())
        assertEquals(listOf("Pi is 3.14 roughly."), splitter.push(" Next"))
        assertEquals("Next", splitter.finish())
        assertNull(splitter.finish())
    }

    @Test fun runOnsAreCutAtASpaceAndLongTokensHard() {
        val words = "word ".repeat(40)
        val chunks = splitBy(words, 9, 30)
        assertTrue(chunks.size > 5)
        assertTrue(chunks.all { it.length <= 31 }, chunks.toString())
        assertEquals(words.trim(), chunks.joinToString(" "))

        val token = "x".repeat(100)
        val hard = splitBy(token, 100, 30)
        assertTrue(hard.all { it.length <= 31 }, hard.toString())
        assertEquals(token, hard.joinToString(""))

        // A hard cut never splits a surrogate pair.
        val emoji = "😀".repeat(30)
        val pairs = splitBy(emoji, 7, 21)
        assertEquals(emoji, pairs.joinToString(""))
        assertTrue(pairs.all { !it.first().isLowSurrogate() && !it.last().isHighSurrogate() }, pairs.toString())
    }

    private val reply =
        "# Result\n\nI ran the tests. All **42** passed.\n\n```text\nok\nok\n```\n\n" +
            "```text\nmore\n```\n\nSee https://example.com/report for details"

    @Test fun streamMatchesThePureNormalizerAndStartsEarly() {
        val options = SpeakOptions(Language.EN)
        val stream = SpeakableStream(options)
        var firstAt: Int? = null
        val sentences = mutableListOf<String>()
        reply.chunked(5).forEachIndexed { i, delta ->
            val ready = stream.push(delta)
            if (firstAt == null && ready.isNotEmpty()) firstAt = i
            sentences += ready
        }
        sentences += stream.finish()
        // The first sentence is out long before the reply ends.
        assertTrue((firstAt ?: Int.MAX_VALUE) < 8, "first at $firstAt")
        assertEquals(
            listOf("Result.", "I ran the tests.", "All 42 passed.", "It's on the screen.", "See example.com for details."),
            sentences,
        )
        // Block-wise streaming says what the whole-text call says.
        assertEquals(speakable(reply, options), sentences.joinToString(" "))
    }

    @Test fun streamIgnoresInputPastTheCapAndFencesHoldBlankLines() {
        val capped = SpeakableStream(SpeakOptions(Language.EN, maxInputBytes = 20))
        val out = capped.push("Short one. And a very long second sentence.\n\n") + capped.push("ignored.\n\n") + capped.finish()
        assertEquals(listOf("Short one.", "And a ver."), out)

        val fenced = SpeakableStream(SpeakOptions(Language.RU))
        assertEquals(listOf("Это на экране.", "Готово."), fenced.push("```\na\n\nb\n```\n\nГотово.\n\n") + fenced.finish())
    }

    @Test fun aLongFenceStreamedInSmallDeltasStaysCheap() {
        val stream = SpeakableStream(SpeakOptions(Language.EN))
        val body = "```\n" + "line of code\n".repeat(MAX_INPUT_BYTES / 13) + "```\n\nDone.\n\n"
        val out = body.chunked(3).flatMap(stream::push) + stream.finish()
        assertEquals(listOf("It's on the screen."), out)
    }
}
