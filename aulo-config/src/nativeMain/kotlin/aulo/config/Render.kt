package aulo.config

import aulo.telemetry.Redact
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

private val bareKey = Regex("[A-Za-z0-9_-]+")

/**
 * The effective config for a person: one `key = value` line per set leaf, keys sorted, which is
 * valid TOML with dotted keys. [withOrigin] appends `  # <layer>` to each line. Values go through
 * [Redact] first: a URL with a password or a token in an MCP argument must not reach a terminal.
 */
fun Loaded.render(withOrigin: Boolean): String {
    val tree = configJson.encodeToJsonElement(Config.serializer(), config)
    val leaves = mutableListOf<Pair<List<String>, JsonElement>>()
    flatten(Redact.scrubJson(tree), emptyList(), leaves)
    return buildString {
        for ((segments, value) in leaves.sortedBy { it.first.joinToString(".") }) {
            append(segments.joinToString(".") { if (bareKey.matches(it)) it else quoted(it) })
            append(" = ")
            append(tomlValue(value))
            // A masked table has no leaf of its own in the provenance; its section's origin stands in.
            val origin = provenance.originOf(segments.joinToString("."))
            if (withOrigin && origin != null) append("  # ").append(origin)
            append('\n')
        }
    }
}

private fun flatten(
    element: JsonElement,
    path: List<String>,
    out: MutableList<Pair<List<String>, JsonElement>>,
) {
    if (element is JsonObject) {
        element.forEach { (key, value) -> flatten(value, path + key, out) }
    } else {
        out += path to element
    }
}

// JSON's string and number syntax is valid TOML; only arrays need their own spacing. Tables never
// get here: flatten splits them into dotted keys.
private fun tomlValue(value: JsonElement): String =
    when (value) {
        is JsonArray -> value.joinToString(", ", "[", "]", transform = ::tomlValue)
        else -> value.toString()
    }

private fun quoted(key: String) = JsonPrimitive(key).toString()
