package aulo.store

/** Longest user query, in UTF-8 bytes, the store turns into a MATCH expression; the rest is dropped. */
const val MAX_QUERY_BYTES: Int = 1024

// Each term is one phrase the index must intersect, so unbounded terms mean unbounded work.
internal const val MAX_TERMS: Int = 32

/** A matching message and its bm25 rank (lower is a better match). */
data class SearchHit(
    val message: Message,
    val rank: Double,
)

/** Where the next page of [Store.searchMessages] starts: the last hit of the previous page. */
data class SearchCursor(
    val rank: Double,
    val id: String,
) {
    constructor(hit: SearchHit) : this(hit.rank, hit.message.id)
}

/**
 * Turns free text into an FTS5 MATCH expression that cannot carry syntax: every
 * whitespace-separated word becomes one quoted phrase (so `OR`, `NEAR(`, `col:`, `*` and `-` are
 * plain words) and the phrases are ANDed. The index tokenizer, not this function, decides what a
 * word is, so queries split exactly as indexed text did. Null when nothing searchable is left,
 * for example `"` alone.
 */
internal fun matchExpression(query: String): String? {
    val terms =
        words(capUtf8(query, MAX_QUERY_BYTES))
            .map { word -> word.filterNot(Char::isISOControl) }
            .filter { word -> word.any(Char::isLetterOrDigit) }
            .take(MAX_TERMS)
            .map { word -> "\"${word.replace("\"", "\"\"")}\"" }
            .toList()
    return terms.takeIf { it.isNotEmpty() }?.joinToString(" ")
}

// Char.isWhitespace also splits on Unicode spaces (U+3000 in CJK text, U+00A0); a regex `\s` need not.
private fun words(text: String): Sequence<String> =
    sequence {
        val word = StringBuilder()
        for (c in text) {
            if (!c.isWhitespace()) {
                word.append(c)
            } else if (word.isNotEmpty()) {
                yield(word.toString())
                word.clear()
            }
        }
        if (word.isNotEmpty()) yield(word.toString())
    }

/** The longest prefix of [text] that fits in [maxBytes] of UTF-8 without splitting a character. */
private fun capUtf8(
    text: String,
    maxBytes: Int,
): String {
    val bytes = text.encodeToByteArray()
    if (bytes.size <= maxBytes) return text
    var end = maxBytes
    // Step back over continuation bytes (10xxxxxx) to the start of the cut character.
    while (end > 0 && bytes[end].toInt() and 0xC0 == 0x80) end--
    return bytes.decodeToString(0, end)
}
