package aulo.telemetry

import aulo.telemetry.Redact.REDACTED
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlin.time.Clock
import kotlin.time.Instant

class LoggerTest {
    private val lines = mutableListOf<String>()
    private val fixed =
        object : Clock {
            override fun now() = Instant.parse("2026-11-23T10:00:00Z")
        }

    @Test
    fun writesOneJsonObjectPerLine() {
        Logger(LogLevel.INFO, lines::add, fixed).info("aulod", "listening", mapOf("port" to 7878, "tls" to true))
        val record = Json.parseToJsonElement(lines.single()).jsonObject
        assertEquals("2026-11-23T10:00:00Z", record.getValue("ts").jsonPrimitive.content)
        assertEquals("info", record.getValue("level").jsonPrimitive.content)
        assertEquals("aulod", record.getValue("target").jsonPrimitive.content)
        assertEquals("listening", record.getValue("message").jsonPrimitive.content)
        assertEquals("""{"port":7878,"tls":true}""", record.getValue("fields").toString())
    }

    @Test
    fun levelsBelowTheThresholdAreDropped() {
        val logger = Logger(LogLevel.WARN, lines::add, fixed)
        logger.debug("t", "noise")
        logger.info("t", "noise")
        logger.warn("t", "kept")
        logger.error("t", "kept")
        assertEquals(2, lines.size)
    }

    @Test
    fun aKeyLikeValueNeverReachesTheSink() {
        val key = "sk-proj-0123456789abcdefghij"
        Logger(LogLevel.TRACE, lines::add, fixed).info(
            "provider",
            "request failed with key $key",
            mapOf("api_key" to key, "url" to "https://host/v1?token=$key", "status" to 401),
        )
        val line = lines.single()
        assertFalse(key in line, "the key reached the log line")
        val record = Json.parseToJsonElement(line).jsonObject
        assertEquals("request failed with key $REDACTED", record.getValue("message").jsonPrimitive.content)
        val fields = record.getValue("fields").jsonObject
        assertEquals(REDACTED, fields.getValue("api_key").jsonPrimitive.content)
        assertEquals("https://host/v1?token=$REDACTED", fields.getValue("url").jsonPrimitive.content)
    }

    @Test
    fun aFailingSinkIsCountedNotThrown() {
        val logger = Logger(LogLevel.INFO, { error("disk full") }, fixed)
        logger.info("t", "one")
        logger.info("t", "two")
        assertEquals(2, logger.dropped)
        assertTrue(logger.enabled(LogLevel.ERROR))
    }
}
