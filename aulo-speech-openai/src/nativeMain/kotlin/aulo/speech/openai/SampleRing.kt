@file:OptIn(ExperimentalAtomicApi::class)

package aulo.speech.openai

import kotlinx.coroutines.channels.Channel
import kotlin.concurrent.atomics.AtomicLong
import kotlin.concurrent.atomics.ExperimentalAtomicApi

/**
 * A fixed ring of 16-bit samples between one writer coroutine and one reader thread. The reader
 * never blocks; the writer suspends while the ring is full, which is what stops it reading the
 * server's answer faster than the speaker plays it.
 */
internal class SampleRing(
    capacity: Int,
) {
    private val buf = ShortArray(capacity)

    // Totals, never wrapped: their difference is the fill, so full and empty cannot be confused.
    private val written = AtomicLong(0)
    private val read = AtomicLong(0)

    // Conflated, so a read that frees space before the writer starts waiting is not lost.
    private val space = Channel<Unit>(Channel.CONFLATED)

    val available: Int get() = (written.load() - read.load()).toInt()

    /** Writes the first [count] samples of [src], waiting for room as often as it takes. */
    suspend fun write(
        src: ShortArray,
        count: Int,
    ) {
        var off = 0
        while (off < count) {
            val w = written.load()
            val free = buf.size - (w - read.load()).toInt()
            if (free == 0) {
                space.receive()
                continue
            }
            val n = minOf(free, count - off)
            for (i in 0 until n) buf[((w + i) % buf.size).toInt()] = src[off + i]
            written.store(w + n)
            off += n
        }
    }

    /** Moves up to `out.size` samples into [out] as floats in [-1, 1); returns how many. */
    fun read(out: FloatArray): Int {
        val r = read.load()
        val n = minOf(out.size.toLong(), written.load() - r).toInt()
        for (i in 0 until n) out[i] = buf[((r + i) % buf.size).toInt()] / PCM16_SCALE
        if (n > 0) {
            read.store(r + n)
            space.trySend(Unit)
        }
        return n
    }

    private companion object {
        const val PCM16_SCALE = 32768f
    }
}
