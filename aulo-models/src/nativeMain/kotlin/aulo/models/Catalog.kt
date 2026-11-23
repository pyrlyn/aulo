package aulo.models

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json

/** Suffix of in-progress downloads; a catalog path may not end in it, or a finished file could pass for a partial one. */
internal const val PART_SUFFIX = ".part"

private const val MAX_ID_LENGTH = 64
private const val MAX_PATH_LENGTH = 200
private const val SHA256_HEX_LENGTH = 64

/** Which speech role a model fills. */
@Serializable
enum class ModelKind {
    @SerialName("stt")
    STT,

    @SerialName("tts")
    TTS,

    @SerialName("vad")
    VAD,

    @SerialName("kws")
    KWS,
}

/** One downloadable file. The size is also the download cap. */
@Serializable
data class ModelFile(
    /** Path relative to the model directory and to [Model.baseUrl]. */
    val path: String,
    /** Exact size in bytes. */
    val size: Long,
    /** Lowercase hex SHA-256 of the file. */
    val sha256: String,
)

/** A model: a directory of files installed together. */
@Serializable
data class Model(
    /** Directory name under `$AULO_HOME/models`. */
    val id: String,
    val kind: ModelKind,
    val name: String,
    /** SPDX expression; shown before a download because some licences need attribution. */
    val license: String,
    /** Immutable location (a Hugging Face commit or a release tag) ending in `/`. */
    @SerialName("base_url")
    val baseUrl: String,
    val files: List<ModelFile>,
) {
    val totalSize: Long get() = files.sumOf { it.size }
}

/** The models aulo knows how to download. */
@Serializable
data class Catalog(
    val models: List<Model>,
) {
    operator fun get(id: String): Model? = models.firstOrNull { it.id == id }

    /**
     * Checks what the types cannot. Ids and paths become names on disk, so they are held to plain
     * names even though the catalog is ours.
     */
    fun validate() {
        val ids = mutableSetOf<String>()
        for (model in models) {
            fun bad(message: String): Nothing = throw ModelException.BadCatalog("${model.id}: $message")
            if (!isSafeId(model.id)) bad("not a safe directory name")
            if (!ids.add(model.id)) bad("duplicate model id")
            val http = model.baseUrl.startsWith("https://") || model.baseUrl.startsWith("http://")
            if (!http || !model.baseUrl.endsWith('/')) bad("base_url must be an http(s) URL ending in `/`")
            if (model.files.isEmpty()) bad("no files")
            val paths = mutableSetOf<String>()
            for (file in model.files) {
                if (!isSafeRelative(file.path)) bad("unsafe file path `${file.path}`")
                if (!paths.add(file.path)) bad("duplicate file `${file.path}`")
                if (file.size <= 0 || !isSha256(file.sha256)) bad("`${file.path}` needs a size and a lowercase SHA-256")
            }
            // `a` next to `a/b` cannot both exist on disk.
            model.files.firstOrNull { f -> paths.any { it.startsWith("${f.path}/") } }?.let {
                bad("`${it.path}` is both a file and a directory")
            }
        }
    }

    companion object {
        private val json = Json { ignoreUnknownKeys = false }

        /** The catalog shipped in the binary, pinned to Hugging Face commits. */
        fun embedded(): Catalog = parse(checkNotNull(CATALOG_FILES["models.json"]) { "models.json is not embedded" })

        fun parse(text: String): Catalog {
            val catalog =
                try {
                    json.decodeFromString(serializer(), text)
                } catch (e: SerializationException) {
                    throw ModelException.BadCatalog(e.message ?: "not a catalog")
                } catch (e: IllegalArgumentException) {
                    throw ModelException.BadCatalog(e.message ?: "not a catalog")
                }
            catalog.validate()
            return catalog
        }
    }
}

/** A directory name: lowercase letters and digits first, then also `.`, `_` and `-`. */
internal fun isSafeId(id: String): Boolean =
    id.length in 1..MAX_ID_LENGTH &&
        id[0].let { it in 'a'..'z' || it in '0'..'9' } &&
        id.all { it in 'a'..'z' || it in '0'..'9' || it == '.' || it == '_' || it == '-' }

/** A relative path of plain components. Backslashes and colons are out: Windows reads them as separators and drives. */
private fun isSafeRelative(path: String): Boolean =
    path.length in 1..MAX_PATH_LENGTH &&
        !path.endsWith(PART_SUFFIX) &&
        path.none { it == '\\' || it == ':' || it == '\u0000' } &&
        path.split('/').none { it.isEmpty() || it == "." || it == ".." }

private fun isSha256(text: String): Boolean = text.length == SHA256_HEX_LENGTH && text.all { it in '0'..'9' || it in 'a'..'f' }
