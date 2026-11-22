package aulo.config

import fs.FsException
import fs.textOrEmpty
import kotlinx.io.files.Path
import kotlinx.serialization.SerializationException
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.SerialKind
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import toml.TomlException
import toml.TomlParser

/** Origin of every leaf value, keyed by dotted path (`voice.stt.engine`). */
class Provenance internal constructor(
    private val entries: Map<String, Ranked>,
) {
    internal data class Ranked(
        val rank: Int,
        val origin: Origin,
    )

    operator fun get(key: String): Origin? = entries[key]?.origin

    /** Leaves in key order. */
    fun entries(): List<Pair<String, Origin>> = entries.entries.sortedBy { it.key }.map { it.key to it.value.origin }

    /** The layer behind [key], or for a section the highest layer that set something inside it. */
    internal fun originOf(key: String): Origin? =
        entries[key]?.origin
            ?: entries
                .filterKeys { it.startsWith("$key.") || it.startsWith("$key[") || key.isEmpty() }
                .values
                .maxByOrNull { it.rank }
                ?.origin
}

class Loaded(
    val config: Config,
    val provenance: Provenance,
)

/**
 * Inputs of [loadConfig]. [env] is the process environment, passed in so tests and the daemon
 * decide what the loader sees; [overrides] is the command-line layer as a nested object.
 */
class LoadOptions(
    val env: Map<String, String> = emptyMap(),
    // Directory holding config.toml. Unset: $AULO_HOME, then ~/.aulo.
    val home: String? = null,
    // Directory holding the project's .aulo.toml.
    val projectDir: String? = null,
    val overrides: JsonObject? = null,
)

internal val configJson =
    Json {
        encodeDefaults = true
        explicitNulls = false
    }

private const val ENV_PREFIX = "AULO_"
private const val ENV_NESTING = "__"

/** `$AULO_HOME`, else `~/.aulo`; null when the environment names no home directory. */
fun auloHome(env: Map<String, String>): String? =
    env["AULO_HOME"]?.takeIf { it.isNotEmpty() }
        ?: (env["HOME"]?.takeIf { it.isNotEmpty() } ?: env["USERPROFILE"]?.takeIf { it.isNotEmpty() })
            ?.let { Path(it, ".aulo").toString() }

/**
 * Merges defaults < `config.toml` < `.aulo.toml` < `AULO_*` variables < overrides. A missing
 * file is skipped; a present layer with an unknown key, a wrong type or bad TOML is an error
 * that names the layer.
 */
fun loadConfig(options: LoadOptions): Loaded {
    val layers = mutableListOf<Pair<Origin, JsonObject>>()
    layers += Origin.Default to (configJson.encodeToJsonElement(Config.serializer(), Config()) as JsonObject)
    (options.home ?: auloHome(options.env))?.let { home -> fileLayer(Path(home, "config.toml"))?.let(layers::add) }
    options.projectDir?.let { dir -> fileLayer(Path(dir, ".aulo.toml"))?.let(layers::add) }
    layers += envLayers(options.env)
    options.overrides?.let { layers += Origin.Override to it }

    val descriptor = Config.serializer().descriptor
    var merged = JsonObject(emptyMap())
    val provenance = mutableMapOf<String, Provenance.Ranked>()
    layers.forEachIndexed { rank, (origin, tree) ->
        ShapeCheck(complete = false) { origin }.check(tree, descriptor, "")
        merged = merge(merged, tree)
        recordLeaves(tree, "") { key -> provenance[key] = Provenance.Ranked(rank, origin) }
    }
    val result = Provenance(provenance)
    // Merging can still combine layers into something invalid (a required key nobody set).
    ShapeCheck(complete = true, originOf = result::originOf).check(merged, descriptor, "")
    val config =
        try {
            configJson.decodeFromJsonElement(Config.serializer(), merged)
        } catch (e: SerializationException) {
            throw ConfigException(null, null, "configuration does not decode (${e::class.simpleName})")
        } catch (e: IllegalArgumentException) {
            throw ConfigException(null, null, "configuration does not decode (${e::class.simpleName})")
        }
    return Loaded(config, result)
}

/** Parses one TOML layer; [origin] names it in errors. */
internal fun parseLayer(
    text: String,
    origin: Origin,
): JsonObject =
    try {
        TomlParser.parse(text)
    } catch (e: TomlException) {
        throw ConfigException(origin, null, e.message ?: "invalid TOML")
    }

private fun fileLayer(path: Path): Pair<Origin, JsonObject>? {
    val origin = Origin.File(path.toString())
    val text =
        try {
            textOrEmpty(path)
        } catch (e: FsException) {
            throw ConfigException(origin, null, "cannot read: ${e.detail}")
        }
    if (text.isEmpty()) return null
    return origin to parseLayer(text, origin)
}

/**
 * One layer per variable, in name order, so each value keeps its own origin. Only names with
 * `__` are config (`AULO_VOICE__STT__ENGINE`): field names contain `_`, and this keeps
 * `AULO_HOME` and credential variables out of the strict check.
 */
private fun envLayers(env: Map<String, String>): List<Pair<Origin, JsonObject>> {
    val descriptor = Config.serializer().descriptor
    return env.keys
        .filter { it.startsWith(ENV_PREFIX) && ENV_NESTING in it.removePrefix(ENV_PREFIX) }
        .sorted()
        .map { name ->
            val origin = Origin.Env(name)
            val segments = name.removePrefix(ENV_PREFIX).lowercase().split(ENV_NESTING)
            val key = segments.joinToString(".")
            val target = descriptor.at(segments) ?: throw ConfigException(origin, key, "unknown key `$key`")
            val raw = env.getValue(name)
            val value: JsonElement =
                if (target.kind == PrimitiveKind.STRING || target.kind == SerialKind.ENUM) {
                    JsonPrimitive(raw)
                } else {
                    // Numbers, booleans, arrays and inline tables use TOML syntax.
                    try {
                        TomlParser.parse("v = $raw").getValue("v")
                    } catch (e: TomlException) {
                        throw ConfigException(origin, key, "`$key` is not a TOML value")
                    }
                }
            origin to segments.foldRight(value) { segment, inner -> JsonObject(mapOf(segment to inner)) } as JsonObject
        }
}

/** Tables merge key by key; anything else in [over] replaces what [base] had. */
internal fun merge(
    base: JsonObject,
    over: JsonObject,
): JsonObject {
    val out = base.toMutableMap()
    for ((key, value) in over) {
        val old = out[key]
        out[key] = if (old is JsonObject && value is JsonObject) merge(old, value) else value
    }
    return JsonObject(out)
}

private fun recordLeaves(
    element: JsonElement,
    path: String,
    record: (String) -> Unit,
) {
    if (element is JsonObject) {
        element.forEach { (key, value) -> recordLeaves(value, join(path, key), record) }
    } else {
        record(path)
    }
}
