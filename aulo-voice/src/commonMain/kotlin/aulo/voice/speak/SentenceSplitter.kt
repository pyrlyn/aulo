package aulo.voice.speak

/** Abbreviations whose period does not end a sentence. Only ones that almost never end one: "etc." is left out. */
private val ABBREVIATIONS =
    setOf("mr", "mrs", "ms", "dr", "prof", "st", "vs", "e.g", "i.e", "г", "гг", "ул", "им", "см", "рис", "проф", "т.е")

private const val TERMINATORS = ".!?…"
private const val CLOSERS = "\"')]»”’"
private const val OPENERS = "(\"'«"

/** Smallest chunk a [SentenceSplitter] cuts, so a chunk can always hold a word. */
private const val MIN_CHUNK = 20

/**
 * Model deltas in, complete sentences out, so TTS can start on the first sentence while the rest is still generated.
 *
 * A sentence is released once its end is certain: a `.`, `!`, `?` or `…` (plus closing quotes and brackets) followed
 * by whitespace. A terminator at the very end of the buffer is not certain ("3." may become "3.14"), so it waits for
 * the next delta or [finish]. A run-on with no boundary is cut at the last space once it passes [maxChunk] characters
 * (at least 20), so the buffer never grows past that bound and a long sentence still starts playing.
 */
class SentenceSplitter(
    maxChunk: Int,
) {
    private val maxChunk = maxOf(maxChunk, MIN_CHUNK)
    private val buf = StringBuilder()

    /** Adds a delta and returns the sentences it completed, in order. */
    fun push(delta: String): List<String> {
        val out = mutableListOf<String>()
        for (c in delta) {
            // Bound the work per character instead of per delta: one huge delta must not grow the buffer past maxChunk.
            buf.append(c)
            if (c.isWhitespace() || buf.length > maxChunk) drainReady(out)
        }
        return out
    }

    /** Ends the text and returns whatever is left as the last sentence. */
    fun finish(): String? {
        val rest = buf.toString().trim()
        buf.clear()
        return rest.ifEmpty { null }
    }

    private fun drainReady(out: MutableList<String>) {
        while (true) {
            val end = sentenceEnd(buf) ?: overflowCut() ?: return
            val sentence = buf.substring(0, end).trim()
            if (sentence.isNotEmpty()) out += sentence
            buf.deleteRange(0, end)
            val skipped = buf.indexOfFirst { !it.isWhitespace() }.let { if (it < 0) buf.length else it }
            buf.deleteRange(0, skipped)
        }
    }

    /** Where to cut a buffer that passed [maxChunk] without a boundary. */
    private fun overflowCut(): Int? {
        if (buf.length <= maxChunk) return null
        // Never split a surrogate pair.
        val limit = if (buf[maxChunk].isLowSurrogate()) maxChunk - 1 else maxChunk
        val space = (limit - 1 downTo 1).firstOrNull { buf[it].isWhitespace() }
        // No space at all (a very long token): hard cut, to keep the bound.
        return space ?: limit
    }
}

/** End (exclusive) of the first confirmed sentence in [text]. */
private fun sentenceEnd(text: CharSequence): Int? {
    var i = 0
    while (i < text.length) {
        val c = text[i]
        if (c !in TERMINATORS) {
            i++
            continue
        }
        var end = i + 1
        while (end < text.length && text[end] in CLOSERS) end++
        val confirmed = end < text.length && text[end].isWhitespace()
        if (confirmed && !(c == '.' && isAbbreviation(text, i))) return end
        i = end
    }
    return null
}

/**
 * Whether the word that ends just before the period at [dot] is an abbreviation or an initial ("J. Smith"). "I", "A"
 * and "Я" are words that end sentences, so they do not count as initials.
 */
private fun isAbbreviation(
    text: CharSequence,
    dot: Int,
): Boolean {
    var start = dot
    while (start > 0 && !text[start - 1].isWhitespace()) start--
    val word = text.subSequence(start, dot).toString().trimStart { it in OPENERS }
    val initial = word.length == 1 && word[0].isUpperCase() && word[0] !in "IAЯ"
    return initial || word.lowercase() in ABBREVIATIONS
}
