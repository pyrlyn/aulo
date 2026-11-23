package aulo.telemetry

import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlin.concurrent.atomics.AtomicLong
import kotlin.concurrent.atomics.ExperimentalAtomicApi
import kotlin.concurrent.atomics.incrementAndFetch
import kotlin.time.Clock

/** Where finished, already masked log lines go; one call per line, without the newline. */
fun interface LogSink {
    fun write(line: String)
}

/**
 * Writes one JSON object per line: `ts`, `level`, `target`, `message` and `fields`. The whole
 * record is masked through [Redact] before the sink sees it, so no caller can forget to.
 */
@OptIn(ExperimentalAtomicApi::class)
class Logger(
    val level: LogLevel,
    private val sink: LogSink,
    private val clock: Clock = Clock.System,
) {
    private val droppedLines = AtomicLong(0)

    /** Lines the sink failed to take. Logging never fails the caller; this is how it shows. */
    val dropped: Long get() = droppedLines.load()

    fun enabled(at: LogLevel): Boolean = at >= level

    fun log(
        at: LogLevel,
        target: String,
        message: String,
        fields: Map<String, Any?> = emptyMap(),
    ) {
        if (!enabled(at)) return
        val record =
            JsonObject(
                mapOf(
                    "ts" to JsonPrimitive(clock.now().toString()),
                    "level" to JsonPrimitive(at.label),
                    "target" to JsonPrimitive(target),
                    "message" to JsonPrimitive(message),
                    "fields" to JsonObject(fields.mapValues { (_, v) -> toJson(v) }),
                ),
            )
        val line = Redact.scrubJson(record).toString()
        try {
            sink.write(line)
        } catch (e: Exception) {
            droppedLines.incrementAndFetch()
        }
    }

    fun trace(
        target: String,
        message: String,
        fields: Map<String, Any?> = emptyMap(),
    ) = log(LogLevel.TRACE, target, message, fields)

    fun debug(
        target: String,
        message: String,
        fields: Map<String, Any?> = emptyMap(),
    ) = log(LogLevel.DEBUG, target, message, fields)

    fun info(
        target: String,
        message: String,
        fields: Map<String, Any?> = emptyMap(),
    ) = log(LogLevel.INFO, target, message, fields)

    fun warn(
        target: String,
        message: String,
        fields: Map<String, Any?> = emptyMap(),
    ) = log(LogLevel.WARN, target, message, fields)

    fun error(
        target: String,
        message: String,
        fields: Map<String, Any?> = emptyMap(),
    ) = log(LogLevel.ERROR, target, message, fields)

    private fun toJson(value: Any?): JsonElement =
        when (value) {
            null -> JsonNull
            is JsonElement -> value
            is String -> JsonPrimitive(value)
            is Boolean -> JsonPrimitive(value)
            is Number -> JsonPrimitive(value)
            else -> JsonPrimitive(value.toString())
        }
}
