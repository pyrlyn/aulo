package aulo.chat.server

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlin.random.Random
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TestTimeSource

@OptIn(ExperimentalCoroutinesApi::class)
class PipelineTest {
    @Test
    fun `rate limiter allows the burst then refills at its rate`() {
        val time = TestTimeSource()
        val limiter = RateLimiter(burst = 2, perMinute = 60, time = time)
        assertNull(limiter.tryAcquire("a"))
        assertNull(limiter.tryAcquire("a"))
        val wait = assertNotNull(limiter.tryAcquire("a"))
        assertEquals(1.seconds, wait)
        assertNull(limiter.tryAcquire("b"), "another chat has its own bucket")

        time += 1.seconds
        assertNull(limiter.tryAcquire("a"))
        assertNotNull(limiter.tryAcquire("a"))

        time += 1.seconds * 60
        assertNull(limiter.tryAcquire("a"))
        assertNull(limiter.tryAcquire("a"))
        assertNotNull(limiter.tryAcquire("a"), "the bucket never holds more than the burst")
    }

    @Test
    fun `queue runs in arrival order and refuses what does not fit`() = runTest {
        val queue = RequestQueue(concurrency = 1, capacity = 1)
        val gate = CompletableDeferred<Unit>()
        val order = mutableListOf<String>()
        val positions = mutableListOf<Int>()

        val first = launch { queue.run { gate.await(); order += "first" } }
        runCurrent()
        val second = launch { queue.run(onQueued = { positions += it }) { order += "second" } }
        runCurrent()
        assertFailsWith<QueueFullException> { queue.run { order += "third" } }

        gate.complete(Unit)
        first.join()
        second.join()
        assertEquals(listOf("first", "second"), order)
        assertEquals(listOf(1), positions)
        // The refused turn must not leave the queue looking full.
        queue.run { order += "fourth" }
        assertEquals("fourth", order.last())
    }

    @Test
    fun `a cancelled waiter gives its place back`() = runTest {
        val queue = RequestQueue(concurrency = 1, capacity = 1)
        val gate = CompletableDeferred<Unit>()
        val running = launch { queue.run { gate.await() } }
        runCurrent()
        val waiter = launch { queue.run { } }
        runCurrent()
        waiter.cancel()
        runCurrent()

        val next = async { queue.run { "ran" } }
        runCurrent()
        gate.complete(Unit)
        running.join()
        assertEquals("ran", next.await())
    }

    @Test
    fun `retry backs off exponentially until the flow answers`() = runTest {
        var calls = 0
        val waits = mutableListOf<Long>()
        val source = flow {
            if (++calls < 3) throw TransientModelException("overloaded")
            emit("a")
            emit("b")
        }
        val policy = RetryPolicy(maxAttempts = 3, baseDelay = 100.milliseconds, maxDelay = 150.milliseconds)
        val out = source.retryTransient(policy) { _, wait, _ -> waits += wait.inWholeMilliseconds }.toList()
        assertEquals(listOf("a", "b"), out)
        assertEquals(listOf(100L, 150L), waits)
        assertEquals(250L, testScheduler.currentTime)
    }

    @Test
    fun `retry gives up after the last attempt and never after a token`() = runTest {
        var calls = 0
        val alwaysDown = flow<String> {
            calls++
            throw TransientModelException("down")
        }
        assertFailsWith<TransientModelException> { alwaysDown.retryTransient(RetryPolicy(maxAttempts = 3)).toList() }
        assertEquals(3, calls)

        calls = 0
        val seen = mutableListOf<String>()
        val breaksMidway = flow {
            calls++
            emit("partial")
            throw TransientModelException("dropped")
        }
        assertFailsWith<TransientModelException> {
            breaksMidway.retryTransient(RetryPolicy()).collect { seen += it }
        }
        assertEquals(1, calls, "an answer that started is not asked for again")
        assertEquals(listOf("partial"), seen)

        calls = 0
        val refuses = flow<String> {
            calls++
            throw ModelException("refused")
        }
        assertFailsWith<ModelException> { refuses.retryTransient(RetryPolicy()).toList() }
        assertEquals(1, calls, "only transient failures are retried")
    }

    @Test
    fun `ulids are 26 crockford characters that sort by time`() {
        val random = Random(1)
        val early = ulid(1_000, random)
        val late = ulid(2_000, random)
        assertEquals(26, early.length)
        assertTrue(early.all { it in "0123456789ABCDEFGHJKMNPQRSTVWXYZ" })
        assertTrue(early < late)
    }

    @Test
    fun `stub tokens concatenate back to the text`() {
        val text = "one  two\nthree"
        assertEquals(text, tokens(text).joinToString(""))
        assertEquals(3, tokens(text).size)
    }

    @Test
    fun `titles are cut on a character boundary`() {
        assertEquals("ab", "abc".truncateUtf8(2))
        assertEquals("é", "éé".truncateUtf8(3))
        assertEquals("a", "a😀".truncateUtf8(3))
    }
}
