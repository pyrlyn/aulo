package aulo.voice

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class AudioRingTest {
    private fun ramp(
        from: Int,
        count: Int,
    ) = FloatArray(count) { (from + it).toFloat() }

    @Test
    fun samplesComeOutInOrderAcrossTheWrap() =
        runBlocking {
            val ring = AudioRing(5)
            val out = FloatArray(3)
            assertTrue(ring.write(ramp(0, 4)))
            assertEquals(3, ring.read(out))
            assertContentEquals(ramp(0, 3), out)
            // Starts at slot 4 and wraps to slot 0..2.
            assertTrue(ring.write(ramp(4, 4)))
            assertEquals(5, ring.available)
            val all = FloatArray(8)
            assertEquals(5, ring.read(all))
            assertContentEquals(ramp(3, 5), all.copyOf(5))
            assertEquals(0, ring.read(all))
        }

    @Test
    fun aWriteHonoursItsRange() =
        runBlocking<Unit> {
            val ring = AudioRing(4)
            ring.write(ramp(0, 6), offset = 2, count = 3)
            val out = FloatArray(4)
            assertEquals(3, ring.read(out))
            assertContentEquals(ramp(2, 3), out.copyOf(3))
            assertFailsWith<IllegalArgumentException> { ring.write(ramp(0, 2), offset = 1, count = 2) }
        }

    @Test
    fun aFullRingMakesTheWriterWaitForTheReader() =
        runBlocking {
            val ring = AudioRing(4)
            val writer = async(Dispatchers.IO) { ring.write(ramp(0, 10)) }
            delay(50)
            // The writer filled the ring and is waiting; nothing grew past the capacity.
            assertFalse(writer.isCompleted)
            assertEquals(4, ring.available)
            val got = mutableListOf<Float>()
            val out = FloatArray(3)
            withTimeout(5_000) {
                while (got.size < 10) {
                    val n = ring.read(out)
                    for (i in 0 until n) got += out[i]
                    if (n == 0) delay(1)
                }
            }
            assertTrue(writer.await())
            assertEquals(ramp(0, 10).toList(), got)
        }

    @Test
    fun closingLetsAWaitingWriterGoAndDropsWhatFollows() =
        runBlocking {
            val ring = AudioRing(2)
            val writer = async(Dispatchers.IO) { ring.write(ramp(0, 10)) }
            delay(50)
            assertFalse(writer.isCompleted)
            ring.close()
            assertFalse(withTimeout(5_000) { writer.await() })
            assertTrue(ring.isClosed)
            assertFalse(ring.write(ramp(0, 1)))
        }

    @Test
    fun capacityMustBePositive() {
        assertFailsWith<IllegalArgumentException> { AudioRing(0) }
    }
}
