package aulo.chat.server

import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.emitAll
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.retryWhen
import kotlinx.coroutines.sync.Semaphore
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicInteger
import kotlin.math.min
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TimeMark
import kotlin.time.TimeSource

// The three guards in front of the model. They are written here on kotlinx.coroutines rather
// than taken from a resilience library because each needs a rule those do not model: a limit
// per chat with the wait reported back, a queue that tells the caller its position, and a
// retry that stops as soon as one token has reached the client.

/**
 * A token bucket per key: [burst] requests at once, refilled at [perMinute].
 */
class RateLimiter(
    private val burst: Int = 5,
    private val perMinute: Int = 30,
    private val time: TimeSource = TimeSource.Monotonic,
) {
    private class Bucket(var tokens: Double, var mark: TimeMark)

    private val buckets = ConcurrentHashMap<String, Bucket>()

    /** Null when the request may run, otherwise how long until the next one may. */
    fun tryAcquire(key: String): Duration? {
        val bucket = buckets.computeIfAbsent(key) { Bucket(burst.toDouble(), time.markNow()) }
        synchronized(bucket) {
            val perSecond = perMinute / 60.0
            val elapsed = bucket.mark.elapsedNow()
            bucket.tokens = min(burst.toDouble(), bucket.tokens + elapsed.inWholeMilliseconds / 1000.0 * perSecond)
            bucket.mark = time.markNow()
            if (bucket.tokens >= 1.0) {
                bucket.tokens -= 1.0
                return null
            }
            return ((1.0 - bucket.tokens) / perSecond).seconds
        }
    }

    fun forget(key: String) {
        buckets.remove(key)
    }
}

class QueueFullException(val capacity: Int) : Exception("request queue is full ($capacity waiting)")

/**
 * Lets [concurrency] turns reach the model at once and makes up to [capacity] more wait in
 * arrival order. A turn that finds the queue full is refused instead of waiting without bound.
 */
class RequestQueue(concurrency: Int = 2, private val capacity: Int = 8) {
    private val permits = Semaphore(concurrency)
    private val waiting = AtomicInteger(0)

    suspend fun <T> run(onQueued: suspend (position: Int) -> Unit = {}, block: suspend () -> T): T {
        if (!permits.tryAcquire()) {
            val position = waiting.incrementAndGet()
            try {
                if (position > capacity) throw QueueFullException(capacity)
                onQueued(position)
                permits.acquire()
            } finally {
                waiting.decrementAndGet()
            }
        }
        try {
            return block()
        } finally {
            permits.release()
        }
    }
}

data class RetryPolicy(
    val maxAttempts: Int = 3,
    val baseDelay: Duration = 200.milliseconds,
    val maxDelay: Duration = 2.seconds,
) {
    /** Exponential: base, 2×base, 4×base … capped at [maxDelay]. [retry] starts at 1. */
    fun delayBefore(retry: Int): Duration {
        val factor = 1L shl (retry - 1).coerceIn(0, 20)
        return minOf(baseDelay * factor.toDouble(), maxDelay)
    }
}

/**
 * Retries a transient failure with backoff, but only while nothing has been emitted: once a
 * token is on the client's screen a second attempt would print the answer twice.
 */
fun <T> Flow<T>.retryTransient(
    policy: RetryPolicy,
    onRetry: suspend (retry: Int, wait: Duration, cause: Throwable) -> Unit = { _, _, _ -> },
): Flow<T> = flow {
    var emitted = false
    emitAll(
        this@retryTransient.onEach { emitted = true }.retryWhen { cause, attempt ->
            val retry = attempt.toInt() + 1
            if (cause !is TransientModelException || emitted || retry >= policy.maxAttempts) {
                return@retryWhen false
            }
            val wait = policy.delayBefore(retry)
            onRetry(retry, wait, cause)
            delay(wait)
            true
        },
    )
}
