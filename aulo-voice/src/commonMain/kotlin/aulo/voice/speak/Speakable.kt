package aulo.voice.speak

import org.intellij.markdown.IElementType
import org.intellij.markdown.MarkdownElementTypes
import org.intellij.markdown.MarkdownTokenTypes
import org.intellij.markdown.ast.ASTNode
import org.intellij.markdown.flavours.gfm.GFMElementTypes
import org.intellij.markdown.flavours.gfm.GFMFlavourDescriptor
import org.intellij.markdown.flavours.gfm.GFMTokenTypes
import org.intellij.markdown.html.entities.Entities
import org.intellij.markdown.parser.CancellationToken
import org.intellij.markdown.parser.MarkdownParser

/**
 * Normalizes one markdown reply for speech: markdown stripped, code and tables replaced by [Language.onScreen], URLs
 * shortened to the host, numbers and units spelled out, whitespace collapsed. Bounded by the options' caps. Returns
 * an empty string when nothing is speakable.
 *
 * Limits: no number-to-words (engines read digits), no negative numbers; dates, times and versions stay as written;
 * only the single units in `Expand.kt` (no `km/h`); only `http(s)` URLs; no per-sentence language switching.
 */
fun speakable(
    markdown: String,
    options: SpeakOptions,
): String {
    val lang = options.language
    val clean = capUtf8(markdown, options.maxInputBytes).filterNot(::isUnspeakable)
    // Filtered again: an entity such as `&#x202E;` decodes to a character the first pass never saw.
    val plain = stripMarkdown(clean, lang).filterNot(::isUnspeakable)
    val text = expand(shortenUrls(plain, lang), lang)
    return limitSpoken(text.trim(), options)
}

/**
 * Characters that make spoken text unsafe or confusing: controls (terminal escapes), soft hyphens, zero-width and
 * bidirectional marks. Bidi overrides can make a reply read differently from what is shown, so they go before the
 * text is parsed. Ordinary whitespace is kept.
 */
private fun isUnspeakable(c: Char): Boolean = (c.isISOControl() && c !in "\t\n\u000B\u000C\r\u0085") || UNSPEAKABLE.any { c.code in it }

// Code points rather than character literals: the formatter would turn escapes into the invisible characters themselves.
private val UNSPEAKABLE =
    listOf(
        0x00AD..0x00AD,
        0x061C..0x061C,
        0x200B..0x200F,
        0x202A..0x202E,
        0x2060..0x2064,
        0x2066..0x2069,
        0xFEFF..0xFEFF,
        0xFFF9..0xFFFB,
    )

// Assertions off: a reply is untrusted text, and a parser self-check must not take the voice loop down.
private val parser = MarkdownParser(GFMFlavourDescriptor(), false, CancellationToken.NonCancellable)

private fun stripMarkdown(
    src: String,
    lang: Language,
): String {
    // The tree's offsets point into the limited text, so the walker reads that text too.
    val limited = limitNesting(src)
    val tree =
        try {
            parser.buildMarkdownTreeFromString(limited as CharSequence)
        } catch (_: RuntimeException) {
            // The parser is third-party code fed untrusted text; a bug in it means "do not read this aloud", not a crash.
            return lang.onScreen
        }
    return Stripper(limited, lang).apply { walk(tree) }.out.toString()
}

/**
 * Most block quote and list markers kept at the start of a line. The parser's time grows with the square of the
 * nesting depth (64 KiB of `>` takes minutes), and no reply nests this deep on purpose.
 */
private const val MAX_NESTING = 16

/** Indentation the parser allows before a marker without turning the line into code. */
private const val MAX_MARKER_INDENT = 3

private const val MAX_ORDERED_DIGITS = 9

/** [src] with block quote and list markers past [MAX_NESTING] removed from the start of every line. */
internal fun limitNesting(src: String): String {
    val out = StringBuilder(src.length)
    var lineStart = 0
    while (lineStart <= src.length) {
        val lineEnd = src.indexOf('\n', lineStart).let { if (it < 0) src.length else it }
        var at = lineStart
        var depth = 0
        var keep = lineStart
        while (true) {
            var marker = at
            while (marker < lineEnd && marker - at < MAX_MARKER_INDENT && (src[marker] == ' ' || src[marker] == '\t')) marker++
            at = markerEnd(src, marker, lineEnd) ?: break
            if (++depth == MAX_NESTING) keep = at
        }
        if (depth > MAX_NESTING) {
            out.append(src, lineStart, keep).append(src, at, lineEnd)
        } else {
            out.append(src, lineStart, lineEnd)
        }
        if (lineEnd < src.length) out.append('\n')
        lineStart = lineEnd + 1
    }
    return out.toString()
}

/** End of a block quote or list marker at [at], or `null` when there is none. */
private fun markerEnd(
    src: String,
    at: Int,
    lineEnd: Int,
): Int? {
    if (at >= lineEnd) return null
    val spaceAfter = { i: Int -> i == lineEnd || src[i] == ' ' || src[i] == '\t' }
    if (src[at] == '>') return at + 1
    if (src[at] in "-+*") return if (spaceAfter(at + 1)) at + 1 else null
    var digit = at
    while (digit < lineEnd && digit - at < MAX_ORDERED_DIGITS && src[digit] in '0'..'9') digit++
    val ordered = digit > at && digit < lineEnd && src[digit] in ".)" && spaceAfter(digit + 1)
    return if (ordered) digit + 1 else null
}

/** Blocks whose contents are not spoken: [Language.onScreen] is said instead. */
private val SCREEN_BLOCKS: Set<IElementType> =
    setOf(MarkdownElementTypes.CODE_FENCE, MarkdownElementTypes.CODE_BLOCK, GFMElementTypes.TABLE, GFMElementTypes.BLOCK_MATH)

/** Blocks that end a sentence when they close, so headings and list items are not glued to the next block. */
private val SENTENCE_BLOCKS: Set<IElementType> =
    setOf(
        MarkdownElementTypes.PARAGRAPH,
        MarkdownElementTypes.LIST_ITEM,
        MarkdownElementTypes.SETEXT_1,
        MarkdownElementTypes.SETEXT_2,
        MarkdownElementTypes.ATX_1,
        MarkdownElementTypes.ATX_2,
        MarkdownElementTypes.ATX_3,
        MarkdownElementTypes.ATX_4,
        MarkdownElementTypes.ATX_5,
        MarkdownElementTypes.ATX_6,
    )

/** Markup with no spoken form: raw HTML, rules, link definitions, list and quote markers, heading markers. */
private val SILENT: Set<IElementType> =
    setOf(
        MarkdownElementTypes.HTML_BLOCK,
        MarkdownElementTypes.LINK_DEFINITION,
        MarkdownTokenTypes.HTML_TAG,
        MarkdownTokenTypes.HORIZONTAL_RULE,
        MarkdownTokenTypes.LIST_BULLET,
        MarkdownTokenTypes.LIST_NUMBER,
        MarkdownTokenTypes.BLOCK_QUOTE,
        MarkdownTokenTypes.ATX_HEADER,
        MarkdownTokenTypes.SETEXT_1,
        MarkdownTokenTypes.SETEXT_2,
        GFMTokenTypes.CHECK_BOX,
        GFMTokenTypes.ALERT_TITLE,
    )

private val LINKS: Set<IElementType> =
    setOf(MarkdownElementTypes.INLINE_LINK, MarkdownElementTypes.FULL_REFERENCE_LINK, MarkdownElementTypes.SHORT_REFERENCE_LINK)

/** Inline spans whose leading and trailing delimiter runs are markup and the rest is text. */
private val DELIMITED: Set<IElementType> =
    setOf(MarkdownElementTypes.EMPH, MarkdownElementTypes.STRONG, GFMElementTypes.STRIKETHROUGH, GFMElementTypes.HIGHLIGHT)

private val DELIMITERS: Set<IElementType> = setOf(MarkdownTokenTypes.EMPH, GFMTokenTypes.TILDE, GFMTokenTypes.EQUALS)

private class Stripper(
    private val src: String,
    private val lang: Language,
) {
    val out = StringBuilder()

    // An explicit stack instead of recursion: inline markup can still nest deeply, and a stack overflow kills the
    // process. A null entry closes the sentence block whose children were pushed above it.
    private val stack = ArrayDeque<ASTNode?>()

    fun walk(root: ASTNode) {
        stack.addLast(root)
        while (stack.isNotEmpty()) {
            val node = stack.removeLast()
            if (node == null) endBlock() else visit(node)
        }
    }

    private fun push(nodes: List<ASTNode>) {
        for (i in nodes.indices.reversed()) stack.addLast(nodes[i])
    }

    private fun visit(node: ASTNode) {
        val type = node.type
        when {
            type in SCREEN_BLOCKS -> {
                onScreen()
            }

            type in SILENT -> {}

            type in SENTENCE_BLOCKS -> {
                stack.addLast(null)
                push(node.children)
            }

            type in LINKS -> {
                node.children.firstOrNull { it.type == MarkdownElementTypes.LINK_TEXT }?.let { text ->
                    push(text.children.trim(MarkdownTokenTypes.LBRACKET, MarkdownTokenTypes.RBRACKET))
                }
            }

            type == MarkdownElementTypes.IMAGE -> {
                push(node.children.filter { it.type != MarkdownTokenTypes.EXCLAMATION_MARK })
            }

            type == MarkdownElementTypes.AUTOLINK -> {
                push(node.children.trim(MarkdownTokenTypes.LT, MarkdownTokenTypes.GT))
            }

            type == MarkdownElementTypes.CODE_SPAN -> {
                codeSpan(node)
            }

            type in DELIMITED -> {
                push(node.children.trimDelimiters())
            }

            type == MarkdownTokenTypes.EOL || type == MarkdownTokenTypes.HARD_LINE_BREAK -> {
                out.append(' ')
            }

            node.children.isEmpty() -> {
                out.append(decodeEntities(text(node)))
            }

            else -> {
                push(node.children)
            }
        }
    }

    private fun text(node: ASTNode): String = src.substring(node.startOffset, node.endOffset)

    private fun codeSpan(node: ASTNode) {
        val ticks = node.children.filter { it.type == MarkdownTokenTypes.BACKTICK }
        if (ticks.size < 2) {
            out.append(text(node))
            return
        }
        out.append(src, ticks.first().endOffset, ticks.last().startOffset)
    }

    private fun onScreen() {
        endBlock()
        // Adjacent blocks say it once.
        if (!out.trimEnd().endsWith(lang.onScreen)) out.append(lang.onScreen).append(' ')
    }

    /** Closes a block with a sentence terminator, so the sentence splitter sees a boundary. */
    private fun endBlock() {
        out.setLength(out.trimEnd().length)
        if (out.isEmpty()) return
        if (out.last() !in BLOCK_ENDS) out.append('.')
        out.append(' ')
    }
}

private const val BLOCK_ENDS = ".!?…:;"

// The library's EntityConverter escapes for HTML (it turns `&` back into `&amp;`), so plain text needs its own pass.
private val ENTITY = Regex("""&(?:[a-zA-Z0-9]+|#[0-9]{1,7}|#[xX][a-fA-F0-9]{1,6});|\\[!-/:-@\[-`{-~]""")

private const val MAX_CODE_POINT = 0x10FFFF
private const val SUPPLEMENTARY = 0x10000
private const val SURROGATE_BITS = 10
private const val LOW_MASK = 0x3FF
private const val HEX = 16
private val SURROGATES = Char.MIN_SURROGATE.code..Char.MAX_SURROGATE.code

/** Text as a renderer shows it: entities (`&amp;`, `&#38;`) and backslash escapes decoded, unknown entities kept. */
internal fun decodeEntities(text: String): String =
    ENTITY.replace(text) { match ->
        val value = match.value
        when {
            value[0] == '\\' -> {
                value.substring(1)
            }

            value[1] == '#' -> {
                val hex = value[2] == 'x' || value[2] == 'X'
                val digits = value.substring(if (hex) 3 else 2, value.length - 1)
                codePoint(digits.toInt(if (hex) HEX else 10))
            }

            else -> {
                Entities.map[value]?.let(::codePoint) ?: value
            }
        }
    }

/** One code point as a string; NUL, surrogates and values past Unicode become U+FFFD, as CommonMark says. */
private fun codePoint(code: Int): String =
    when {
        code == 0 || code > MAX_CODE_POINT || code in SURROGATES -> {
            "\uFFFD"
        }

        code < SUPPLEMENTARY -> {
            Char(code).toString()
        }

        else -> {
            val v = code - SUPPLEMENTARY
            charArrayOf(Char(Char.MIN_HIGH_SURROGATE.code + (v shr SURROGATE_BITS)), Char(Char.MIN_LOW_SURROGATE.code + (v and LOW_MASK)))
                .concatToString()
        }
    }

private fun List<ASTNode>.trim(
    open: IElementType,
    close: IElementType,
): List<ASTNode> {
    val from = if (firstOrNull()?.type == open) 1 else 0
    val to = if (size > from && last().type == close) size - 1 else size
    return subList(from, to)
}

private fun List<ASTNode>.trimDelimiters(): List<ASTNode> {
    val isDelimiter = { n: ASTNode -> n.children.isEmpty() && n.type in DELIMITERS }
    val from = indexOfFirst { !isDelimiter(it) }.let { if (it < 0) size else it }
    val to = indexOfLast { !isDelimiter(it) } + 1
    return if (from >= to) emptyList() else subList(from, to)
}

private const val URL_TRAILERS = ".,;:!?)]}>\"'"

/**
 * Replaces every `http(s)` URL with its host; one that does not parse is spoken as [Language.link]. Trailing sentence
 * punctuation stays outside the URL. Whitespace runs become one space.
 */
private fun shortenUrls(
    text: String,
    lang: Language,
): String =
    words(text).joinToString(" ") { word ->
        val start =
            word.indexOf("http://").takeIf { it >= 0 }
                ?: word.indexOf("https://").takeIf { it >= 0 }
                ?: return@joinToString word
        val tail = word.substring(start)
        val url = tail.trimEnd { it in URL_TRAILERS }
        val spoken = urlHost(url)?.removePrefix("www.") ?: lang.link
        word.substring(0, start) + spoken + tail.substring(url.length)
    }

private fun words(text: String): List<String> =
    buildList {
        var start = -1
        for (i in text.indices) {
            if (text[i].isWhitespace()) {
                if (start >= 0) add(text.substring(start, i))
                start = -1
            } else if (start < 0) {
                start = i
            }
        }
        if (start >= 0) add(text.substring(start))
    }

private const val MAX_PORT_DIGITS = 5

/**
 * The host of an `http(s)` URL, lower-cased, or `null` when it has none or holds characters no host can. User info
 * is dropped on purpose: a password in a URL must never be read aloud.
 */
internal fun urlHost(url: String): String? {
    val rest = url.substringAfter("://")
    val authority = rest.substring(0, rest.indexOfAny(charArrayOf('/', '?', '#', '\\')).let { if (it < 0) rest.length else it })
    val hostPort = authority.substringAfterLast('@')
    val (host, port) =
        if (hostPort.startsWith('[')) {
            val close = hostPort.indexOf(']')
            if (close < 0) return null
            val ip = hostPort.substring(1, close)
            if (ip.isEmpty() || !ip.all { it.isHexDigit() || it == ':' || it == '.' }) return null
            hostPort.substring(0, close + 1) to hostPort.substring(close + 1)
        } else {
            val colon = hostPort.indexOf(':')
            if (colon < 0) hostPort to "" else hostPort.substring(0, colon) to hostPort.substring(colon)
        }
    val portOk = port.isEmpty() || (port.length <= MAX_PORT_DIGITS + 1 && port.drop(1).all { it in '0'..'9' } && port[0] == ':')
    val hostOk = host.isNotEmpty() && (host.startsWith('[') || host.all { it.isLetterOrDigit() || it in "-._" })
    return if (portOk && hostOk) host.lowercase() else null
}

private fun Char.isHexDigit(): Boolean = this in '0'..'9' || this in 'a'..'f' || this in 'A'..'F'

/** Cuts text over the spoken limit at the last sentence end (or word) and says the rest is on the screen. */
private fun limitSpoken(
    text: String,
    options: SpeakOptions,
): String {
    if (text.length <= options.maxSpokenChars) return text
    val head = text.substring(0, options.maxSpokenChars)
    val end = head.indexOfLast { it in ".!?…" }
    val cut = if (end >= 0) end + 1 else head.lastIndexOf(' ').coerceAtLeast(0)
    val spoken = head.substring(0, cut).trim()
    val phrase = options.language.onScreen
    return if (spoken.isEmpty()) phrase else "$spoken $phrase"
}
