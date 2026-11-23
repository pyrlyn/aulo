package aulo.telemetry

import aulo.telemetry.Redact.REDACTED
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class RedactTest {
    @Test
    fun rulesCompileAndLeavePlainTextAlone() {
        // A rule that failed to compile would mask everything, so plain text surviving proves they built.
        assertEquals("started turn 4 for bot b1", Redact.scrubText("started turn 4 for bot b1"))
    }

    @Test
    fun namesThatLookSecretAreDetected() {
        for (name in listOf("token", "API_KEY", "access_token", "Authorization", "db.password", "client_secret")) {
            assertTrue(Redact.isSecretName(name), name)
        }
        for (name in listOf("message", "level", "target", "turn_id", "bot")) {
            assertFalse(Redact.isSecretName(name), name)
        }
    }

    @Test
    fun keyLikeValuesAreMaskedAnywhere() {
        val secrets =
            listOf(
                "Bearer abcdef0123456789",
                "sk-ant-api03-abcdefghijklmnop",
                "ghp_abcdefghijklmnopqrstuvwxyz",
                "github_pat_11ABCDEFG0123456789abc",
                "aulo_0123456789abcdef0123456789abcdef",
                "xoxb-1234567890-abcdef",
                "AKIAIOSFODNN7EXAMPLE",
                "eyJhbGciOiJIUzI1.eyJzdWIiOiIxMjM0.SflKxwRJSMeKKF2QT4",
            )
        for ((index, secret) in secrets.withIndex()) {
            val out = Redact.scrubText("calling upstream with $secret now")
            // The message names the case by index: an assertion message is a log too.
            assertFalse(secret in out, "case $index leaked")
            assertEquals("calling upstream with $REDACTED now", out, "case $index")
        }
    }

    @Test
    fun secretNamedPairsLoseOnlyTheValue() {
        assertEquals(
            "connect host=db password=$REDACTED port=5432",
            Redact.scrubText("connect host=db password=hunter2 port=5432"),
        )
        assertEquals("login user=a token=$REDACTED ok", Redact.scrubText("""login user=a token="two words" ok"""))
        assertEquals("Authorization: $REDACTED", Redact.scrubText("Authorization: Bearer abcdef0123456789"))
    }

    @Test
    fun scrubbingIsIdempotent() {
        val once = Redact.scrubText("password=hunter2 Bearer abcdef0123456789")
        assertEquals(once, Redact.scrubText(once))
    }

    @Test
    fun jsonMasksSecretKeysOfAnyTypeAndNestedStrings() {
        val record =
            Json.parseToJsonElement(
                """{"level":"info","fields":{"message":"sent Bearer abcdef0123456789",""" +
                    """"token":{"inner":1},"api_key":42,"turn":7,"args":["--password=hunter2"]}}""",
            )
        val fields =
            Redact
                .scrubJson(record)
                .jsonObject
                .getValue("fields")
                .jsonObject
        assertEquals("sent $REDACTED", fields.getValue("message").jsonPrimitive.content)
        assertEquals(REDACTED, fields.getValue("token").jsonPrimitive.content)
        assertEquals(REDACTED, fields.getValue("api_key").jsonPrimitive.content)
        assertEquals("7", fields.getValue("turn").toString())
        assertEquals("""["--password=$REDACTED"]""", fields.getValue("args").toString())
        assertTrue(Redact.scrubJson(JsonObject(emptyMap())) is JsonObject)
    }
}
