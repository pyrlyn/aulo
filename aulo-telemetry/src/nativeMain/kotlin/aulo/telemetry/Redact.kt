package aulo.telemetry

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/**
 * Secret masking: the rules of the earlier Rust `telemetry-setup` crate plus the aulod token
 * shape, so logs, `aulo config show` and anything else shown to a person share one rule set.
 */
object Redact {
    const val REDACTED: String = "[REDACTED]"

    // A substring rather than a whole word over-masks `cache_key`, which is the safe side.
    private val secretNameParts =
        listOf("token", "key", "password", "passwd", "secret", "authorization", "credential", "cookie")

    // Masked wherever they appear, even under an innocent name: bearer headers, provider API
    // keys, forge tokens, chat tokens, cloud keys, JWTs and aulod API tokens.
    private val valuePatterns =
        listOf(
            """\bbearer\s+[A-Za-z0-9._~+/=-]{8,}""",
            """\b(?:sk|pk|rk)-[A-Za-z0-9_-]{16,}""",
            """\bgh[pousr]_[A-Za-z0-9]{20,}""",
            """\bgithub_pat_[A-Za-z0-9_]{20,}""",
            """\bxox[abprs]-[A-Za-z0-9-]{10,}""",
            """\bAKIA[0-9A-Z]{16}\b""",
            """\bAIza[0-9A-Za-z_-]{35}""",
            """\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}""",
            """\baulo_[0-9a-f]{16,}""",
        )

    private class Rules(
        val value: Regex,
        val pair: Regex,
    )

    // A rule that fails to compile must not become a crash or a silent pass-through, so callers
    // get everything masked instead; a test pins that the rules compile.
    private val rules: Rules? by lazy {
        runCatching {
            Rules(
                value = Regex("(?i)(?:(?:" + valuePatterns.joinToString(")|(?:") + "))"),
                // `name=value` or `name: value` with a secret-looking name; the value is quoted or one token.
                pair =
                    Regex(
                        """(?i)\b([\w.-]*(?:""" + secretNameParts.joinToString("|") +
                            """)[\w.-]*)(\s*[=:]\s*)("(?:[^"\\]|\\.)*"|'[^']*'|[^\s,;}]+)""",
                    ),
            )
        }.getOrNull()
    }

    fun isSecretName(name: String): Boolean {
        val lower = name.lowercase()
        return secretNameParts.any { it in lower }
    }

    /** [text] with credential-shaped values and the values of secret-named pairs masked. */
    fun scrubText(text: String): String {
        val r = rules ?: return REDACTED
        // Values first, so `Authorization: Bearer abc` is one token when the pair rule runs.
        val afterValues = r.value.replace(text, REDACTED)
        return r.pair.replace(afterValues, "\$1\$2$REDACTED")
    }

    /** Secret-named keys lose their whole value, of any type; every other string is scrubbed. */
    fun scrubJson(element: JsonElement): JsonElement =
        when (element) {
            is JsonObject -> {
                JsonObject(
                    element.mapValues { (key, value) ->
                        if (isSecretName(key)) JsonPrimitive(REDACTED) else scrubJson(value)
                    },
                )
            }

            is JsonArray -> {
                JsonArray(element.map(::scrubJson))
            }

            JsonNull -> {
                element
            }

            is JsonPrimitive -> {
                if (element.isString) JsonPrimitive(scrubText(element.content)) else element
            }
        }
}
