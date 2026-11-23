@file:OptIn(ExperimentalAtomicApi::class)

package aulo.voice

import kotlinx.coroutines.channels.Channel
import kotlin.concurrent.atomics.AtomicBoolean
import kotlin.concurrent.atomics.AtomicLong
import kotlin.concurrent.atomics.ExperimentalAtomicApi

/**
 * A fixed ring of samples between one writer coroutine and one reader, the audio thread. The
 * reader never blocks or allocates; the writer suspends while the ring is full, which is what
 * keeps a TTS engine from producing speech faster than the speaker plays it, with no buffer that
 * grows. [close] is the reader walking away (a cancelled reply): a waiting writer is let go and
 * everything it writes from then on is dropped.
 */
class AudioRing(
    capacity: Int,
) {
    init {
        require(capacity > 0) { "capacity must be positive" }
    }

    private val buf = FloatArray(capacity)

    // Totals, never wrapped: their difference is the fill, so full and empty cannot be confused.
    private val written = AtomicLong(0)
    private val read = AtomicLong(0)
    private val closed = AtomicBoolean(false)

    // Conflated, so a read or close that comes before the writer starts waiting is not lost.
    private val space = Channel<Unit>(Channel.CONFLATED)

    val capacity: Int get() = buf.size

    /** Samples written and not read yet. */
    val available: Int get() = (written.load() - read.load()).toInt()

    val isClosed: Boolean get() = closed.load()

    /**
     * Writes [count] samples of [src] from [offset], waiting for room as often as it takes.
     * Returns false, with the rest dropped, once the ring is closed.
     */
    suspend fun write(
        src: FloatArray,
        offset: Int = 0,
        count: Int = src.size - offset,
    ): Boolean {
        require(offset >= 0 && count >= 0 && offset + count <= src.size) { "range outside the array" }
        var off = offset
        val end = offset + count
        while (off < end) {
            if (closed.load()) return false
            val w = written.load()
            val free = buf.size - (w - read.load()).toInt()
            if (free == 0) {
                space.receive()
                continue
            }
            val n = minOf(free, end - off)
            val at = (w % buf.size).toInt()
            val first = minOf(n, buf.size - at)
            src.copyInto(buf, at, off, off + first)
            src.copyInto(buf, 0, off + first, off + n)
            written.store(w + n)
            off += n
        }
        return !closed.load()
    }

    /** Moves up to `out.size` samples into [out] and returns how many; never blocks. */
    fun read(out: FloatArray): Int {
        val r = read.load()
        val n = minOf(out.size.toLong(), written.load() - r).toInt()
        if (n == 0) return 0
        val at = (r % buf.size).toInt()
        val first = minOf(n, buf.size - at)
        buf.copyInto(out, 0, at, at + first)
        buf.copyInto(out, first, 0, n - first)
        read.store(r + n)
        space.trySend(Unit)
        return n
    }

    /** Lets a waiting writer go; later writes are dropped. Never blocks. */
    fun close() {
        closed.store(true)
        space.trySend(Unit)
    }
}
