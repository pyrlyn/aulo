package aulo.voice.speak

/**
 * Largest reply the normalizer reads, in UTF-8 bytes. Model text is untrusted; a runaway reply must not turn into
 * minutes of speech or unbounded work.
 */
const val MAX_INPUT_BYTES: Int = 64 * 1024

/** Longest text spoken from one reply; the rest is replaced by [Language.onScreen], since nobody listens to a page. */
const val MAX_SPOKEN_CHARS: Int = 2_000

/** Languages with spoken-form rules. */
enum class Language(
    /** The primary BCP 47 subtag. */
    val tag: String,
    /** Spoken in place of code, tables and output that was cut for length. */
    val onScreen: String,
    internal val link: String,
) {
    EN("en", "It's on the screen.", "link"),
    RU("ru", "Это на экране.", "ссылка"),
    ;

    companion object {
        /** Matches the primary subtag of a BCP 47 tag, so `ru-RU` is Russian. */
        fun fromTag(tag: String): Language? =
            when (tag.split('-', '_').first().lowercase()) {
                "en" -> EN
                "ru" -> RU
                else -> null
            }
    }
}

/** How [speakable] reads one reply. The caps are what keep untrusted model text bounded. */
data class SpeakOptions(
    val language: Language,
    val maxInputBytes: Int = MAX_INPUT_BYTES,
    val maxSpokenChars: Int = MAX_SPOKEN_CHARS,
) {
    init {
        require(maxInputBytes >= 0) { "maxInputBytes must not be negative" }
        require(maxSpokenChars >= 0) { "maxSpokenChars must not be negative" }
    }
}

private val CYRILLIC = 0x0400..0x04FF

/**
 * Guesses the language of [text] from its letters, so the voice can follow what the model wrote. `null` when there
 * are no Latin or Cyrillic letters; the caller then keeps the chat's configured language. Cyrillic needs only half as
 * many letters as Latin because Russian replies often quote English commands and names.
 */
fun detectLanguage(text: String): Language? {
    var cyrillic = 0
    var latin = 0
    for (c in text) {
        if (c.code in CYRILLIC) {
            cyrillic++
        } else if (c in 'a'..'z' || c in 'A'..'Z') {
            latin++
        }
    }
    return when {
        cyrillic == 0 && latin == 0 -> null
        cyrillic * 2 >= latin -> Language.RU
        else -> Language.EN
    }
}

private const val ONE_BYTE = 0x80
private const val TWO_BYTES = 0x800

/** The longest prefix of [s] that fits in [maxBytes] of UTF-8, never splitting a character. */
internal fun capUtf8(
    s: String,
    maxBytes: Int,
): String {
    var bytes = 0
    var i = 0
    while (i < s.length) {
        val c = s[i]
        val pair = c.isHighSurrogate() && i + 1 < s.length && s[i + 1].isLowSurrogate()
        val size =
            when {
                pair -> 4
                c.code < ONE_BYTE -> 1
                c.code < TWO_BYTES -> 2
                else -> 3
            }
        if (bytes + size > maxBytes) break
        bytes += size
        i += if (pair) 2 else 1
    }
    return s.substring(0, i)
}

internal fun utf8Size(s: String): Int = s.encodeToByteArray().size
