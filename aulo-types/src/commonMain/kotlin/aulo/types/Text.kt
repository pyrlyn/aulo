package aulo.types

/**
 * Cuts [this] to at most [maxBytes] of UTF-8 without splitting a character. Untrusted text
 * (STT output, plugin and MCP names, model-built summaries) goes through it before it is
 * put into an event, so the event caps below hold.
 */
fun String.truncateUtf8(maxBytes: Int): String {
    require(maxBytes >= 0) { "maxBytes must not be negative" }
    val bytes = encodeToByteArray()
    if (bytes.size <= maxBytes) return this
    var end = maxBytes
    // A continuation byte (10xxxxxx) at the cut means the character started before it.
    while (end > 0 && (bytes[end].toInt() and 0xC0) == 0x80) end--
    return bytes.decodeToString(0, end)
}

internal fun requireUtf8AtMost(
    field: String,
    value: String,
    maxBytes: Int,
) {
    require(value.encodeToByteArray().size <= maxBytes) { "$field is longer than $maxBytes bytes" }
}
