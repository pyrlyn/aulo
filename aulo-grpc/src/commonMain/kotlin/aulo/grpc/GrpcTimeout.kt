package aulo.grpc

import kotlin.time.Duration
import kotlin.time.Duration.Companion.hours
import kotlin.time.Duration.Companion.microseconds
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.minutes
import kotlin.time.Duration.Companion.nanoseconds
import kotlin.time.Duration.Companion.seconds

/** The `grpc-timeout` header: at most eight ASCII digits and a unit (gRPC over HTTP/2, "Requests"). */
internal object GrpcTimeout {
    private const val MAX_DIGITS = 8
    private const val MAX_VALUE = 99_999_999L

    private val units: List<Pair<Char, Duration>> =
        listOf('n' to 1.nanoseconds, 'u' to 1.microseconds, 'm' to 1.milliseconds, 'S' to 1.seconds, 'M' to 1.minutes, 'H' to 1.hours)

    /** The timeout [value] names, or null when it is malformed. */
    fun parse(value: String): Duration? {
        if (value.length !in 2..MAX_DIGITS + 1) return null
        val digits = value.dropLast(1)
        if (!digits.all { it in '0'..'9' }) return null
        val unit = units.firstOrNull { it.first == value.last() }?.second ?: return null
        return unit * digits.toInt()
    }

    /** The shortest unit that fits; a timeout is never rounded down, so a call never ends early. */
    fun format(timeout: Duration): String {
        val nanos = timeout.inWholeNanoseconds.coerceAtLeast(0)
        for ((suffix, unit) in units) {
            val size = unit.inWholeNanoseconds
            val count = (nanos + size - 1) / size
            if (count <= MAX_VALUE) return "$count$suffix"
        }
        return "${MAX_VALUE}H"
    }
}
