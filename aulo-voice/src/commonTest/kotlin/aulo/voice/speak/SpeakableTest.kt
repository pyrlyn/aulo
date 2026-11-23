package aulo.voice.speak

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.time.Duration.Companion.seconds
import kotlin.time.measureTime

class SpeakableTest {
    private val en = SpeakOptions(Language.EN)

    @Test fun inputIsCappedAndLongOutputGoesToTheScreen() {
        val spoken = speakable("Word. ".repeat(MAX_SPOKEN_CHARS), en)
        assertTrue(spoken.endsWith(Language.EN.onScreen), spoken)
        assertTrue(spoken.length <= MAX_SPOKEN_CHARS + Language.EN.onScreen.length + 1, spoken)

        // Far more input than the cap, two bytes a character, so the cut falls between bytes of one character.
        val huge = "я".repeat(MAX_INPUT_BYTES)
        assertEquals(Language.RU.onScreen, speakable(huge, SpeakOptions(Language.RU)))

        assertEquals("Hello.", speakable("Hello world", en.copy(maxInputBytes = 5)))
        assertEquals("Hello.", speakable("Hello я", en.copy(maxInputBytes = 7)))
        assertEquals("", speakable("Hello", en.copy(maxInputBytes = 0)))
    }

    @Test fun negativeCapsAreRejected() {
        assertFailsWith<IllegalArgumentException> { SpeakOptions(Language.EN, maxInputBytes = -1) }
        assertFailsWith<IllegalArgumentException> { SpeakOptions(Language.EN, maxSpokenChars = -1) }
    }

    @Test fun nothingSpeakableGivesAnEmptyString() {
        for (input in listOf(
            "",
            "   \n\t",
            "<!-- only a comment -->",
            "${Char(0x202E)}${Char(0x200B)}",
            "---",
            "[ref]: https://example.com",
        )) {
            assertEquals("", speakable(input, en), input)
        }
    }

    // Untrusted text built to make a markdown parser recurse or backtrack still finishes and stays bounded. Without
    // the nesting cap the first two take minutes; the bound is loose because CI may run under emulation.
    @Test fun hostileMarkdownStaysBounded() {
        val inputs =
            listOf(
                ">".repeat(MAX_INPUT_BYTES) + " deep",
                "> ".repeat(MAX_INPUT_BYTES / 2) + "deep",
                (0..250).joinToString("") { "> ".repeat(it) + "x\n\n" },
                (0..250).joinToString("") { "  ".repeat(it) + "- x\n" },
                "- ".repeat(MAX_INPUT_BYTES / 2) + "deep",
                "*_".repeat(MAX_INPUT_BYTES / 4) + "x" + "_*".repeat(MAX_INPUT_BYTES / 4),
                "[a](".repeat(MAX_INPUT_BYTES / 4),
                "[".repeat(MAX_INPUT_BYTES / 2) + "]".repeat(MAX_INPUT_BYTES / 2),
                "*a ".repeat(MAX_INPUT_BYTES / 3),
                "`".repeat(MAX_INPUT_BYTES),
                "<".repeat(MAX_INPUT_BYTES),
                "&#".repeat(MAX_INPUT_BYTES / 2),
            )
        for (input in inputs) {
            val spoken: String
            val took = measureTime { spoken = speakable(input, en) }
            assertTrue(took < 30.seconds, "${input.take(10)}: $took")
            assertTrue(spoken.length <= MAX_SPOKEN_CHARS + Language.EN.onScreen.length + 1, input.take(10))
        }
    }

    @Test fun nestingPastTheCapIsFlattened() {
        assertEquals("> ".repeat(16) + "x", limitNesting("> ".repeat(40) + "x"))
        assertEquals("- 1. > ".repeat(5) + "- x\nplain", limitNesting("- 1. > ".repeat(9) + "x\nplain"))
        assertEquals("> > text", limitNesting("> > text"))
        assertEquals("-not a marker", limitNesting("-not a marker"))
        assertEquals("deep quote. after.", speakable("> ".repeat(40) + "deep **quote**\n\nafter", en))
    }

    @Test fun entitiesAndEscapesAreDecoded() {
        assertEquals("Tom & Jerry © * \\ \uFFFD", decodeEntities("Tom &amp; Jerry &copy; \\* \\\\ &#0;"))
        assertEquals("&nosuch; \\q", decodeEntities("&nosuch; \\q"))
        assertEquals("evil.", speakable("&#x202E;evil", en))
    }

    @Test fun hostsAreReadWithoutUserInfoOrPort() {
        assertEquals("example.com", urlHost("https://user:secret@Example.com:8080/a"))
        assertEquals("[::1]", urlHost("http://[::1]:80/"))
        assertEquals("пример.рф", urlHost("https://пример.рф/путь"))
        assertNull(urlHost("https://"))
        assertNull(urlHost("https://[bad"))
        assertNull(urlHost("https://host:port/"))
        assertNull(urlHost("https://a b/"))
    }

    @Test fun languageHelpers() {
        assertEquals(Language.RU, Language.fromTag("ru-RU"))
        assertEquals(Language.EN, Language.fromTag("EN_us"))
        assertNull(Language.fromTag("de"))
        assertNull(Language.fromTag(""))
        assertEquals(Language.EN, detectLanguage("Hello there"))
        assertEquals(Language.RU, detectLanguage("Привет, мир"))
        assertEquals(Language.RU, detectLanguage("Запусти cargo test"))
        assertNull(detectLanguage("12 + 3 = 15"))
    }
}
