package aulo.voice.speak

/** Longest chunk handed to TTS in characters; a run-on is cut at a space. */
private const val MAX_SENTENCE_CHARS = 240

/** A block with no blank line is normalized once it passes this many UTF-8 bytes, so one long paragraph still starts. */
private const val MAX_BLOCK_BYTES = 1024

/**
 * Raw reply deltas in, speakable sentences out.
 *
 * Markdown cannot be stripped delta by delta (a code fence spans many), so text is collected into blocks: a block
 * ends at a blank line outside a code fence. Each block is normalized with [speakable] and split into sentences, so
 * TTS starts after the first paragraph, not the whole reply.
 */
class SpeakableStream(
    private val options: SpeakOptions,
) {
    private val pending = StringBuilder()
    private var seen = 0
    private val splitter = SentenceSplitter(MAX_SENTENCE_CHARS)
    private var lastWasPhrase = false

    /** Adds a delta and returns the sentences now ready to speak. Input past the total cap is ignored. */
    fun push(delta: String): List<String> {
        val kept = capUtf8(delta, options.maxInputBytes - seen)
        seen += utf8Size(kept)
        pending.append(kept)
        val out = mutableListOf<String>()
        while (true) {
            val end = blockEnd() ?: break
            val block = pending.substring(0, end)
            pending.deleteRange(0, end)
            emit(block, out)
        }
        return out
    }

    /** Ends the reply and returns the remaining sentences. */
    fun finish(): List<String> {
        val out = mutableListOf<String>()
        val block = pending.toString()
        pending.clear()
        scanned = 0
        scannedBytes = 0
        inFence = false
        emit(block, out)
        return out
    }

    // Where the block scan stopped: whole lines before it are already counted, so an open fence is not rescanned on
    // every delta.
    private var scanned = 0
    private var scannedBytes = 0
    private var inFence = false

    /** End of the first complete block in [pending]. Blocks are cut only outside a fence, so each starts outside one. */
    private fun blockEnd(): Int? {
        while (true) {
            val newline = pending.indexOf('\n', scanned)
            if (newline < 0) return null
            val line = pending.substring(scanned, newline + 1)
            scanned = newline + 1
            scannedBytes += utf8Size(line)
            val trimmed = line.trimStart()
            if (trimmed.startsWith("```") || trimmed.startsWith("~~~")) inFence = !inFence
            if (!inFence && (trimmed.isEmpty() || scannedBytes >= MAX_BLOCK_BYTES)) {
                val end = scanned
                scanned = 0
                scannedBytes = 0
                return end
            }
        }
    }

    private fun emit(
        block: String,
        out: MutableList<String>,
    ) {
        val text = speakable(block, options)
        val phrase = options.language.onScreen
        // Blocks end on a terminator; finish so sentences never merge across them.
        val sentences = splitter.push(text) + listOfNotNull(splitter.finish())
        for (sentence in sentences) {
            val isPhrase = sentence == phrase
            if (!(isPhrase && lastWasPhrase)) out += sentence
            lastWasPhrase = isPhrase
        }
    }
}
