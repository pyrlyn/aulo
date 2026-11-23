package aulo.models

import fs.EntryKind
import fs.FileLock
import fs.createDirAll
import fs.entryKind
import fs.isDir
import fs.joinPath
import fs.linkSize
import fs.removeAny
import http.HttpClient
import http.HttpSettings
import kotlinx.io.files.Path

/**
 * Timeouts for model downloads. The hash, not the transport, is the trust anchor, so redirects to a CDN are fine.
 * There is no total timeout because a model is hundreds of megabytes; a stalled socket still ends the transfer.
 */
val MODEL_HTTP_SETTINGS =
    HttpSettings(connectTimeoutMs = 15_000, requestTimeoutMs = Long.MAX_VALUE, socketTimeoutMs = 60_000)

/** Install state of one catalog model, as `aulo models ls` shows it. */
data class ModelStatus(
    val model: Model,
    val filesPresent: Int,
) {
    val installed: Boolean get() = filesPresent == model.files.size
}

/** Download progress; [bytes] and [totalBytes] count the whole model. */
data class Progress(
    val model: String,
    val file: String,
    val fileBytes: Long,
    val fileSize: Long,
    val bytes: Long,
    val totalBytes: Long,
)

/** Installs, lists and removes catalog models under `<aulo home>/models`. */
class ModelManager(
    home: Path,
    val catalog: Catalog,
    private val http: HttpClient,
) {
    val modelsDir: Path = joinPath(home, "models")

    init {
        catalog.validate()
    }

    fun list(): List<ModelStatus> = catalog.models.map { m -> ModelStatus(m, m.files.count { has(m, it) }) }

    /** Directory of an installed model, for the engine that loads it; null until every file is in place. */
    fun path(id: String): Path? {
        val model = catalog[id] ?: return null
        return if (model.files.all { has(model, it) }) joinPath(modelsDir, model.id) else null
    }

    /**
     * Downloads every missing file of [id], verifying each against its pinned SHA-256, and returns the model
     * directory. Files already installed are kept: they only reach their final name after a hash check.
     */
    suspend fun pull(
        id: String,
        onProgress: (Progress) -> Unit = {},
    ): Path {
        val model = known(id)
        val dir = joinPath(modelsDir, model.id)
        val total = model.totalSize
        locked(model.id, create = true) {
            var finished = 0L
            for (file in model.files) {
                val report = { fileBytes: Long ->
                    onProgress(Progress(model.id, file.path, fileBytes, file.size, finished + fileBytes, total))
                }
                if (has(model, file)) {
                    report(file.size)
                } else {
                    val dest = joinPath(dir, file.path)
                    io(dest) { dest.parent?.let(::createDirAll) }
                    val want = Expected(model.baseUrl + file.path, file.path, file.size, file.sha256)
                    fetchVerified(http, want, dest, report)
                }
                finished += file.size
            }
        }
        return dir
    }

    /**
     * Deletes a model directory, partial downloads included; false when there was nothing to delete. An id the
     * catalog no longer lists can still be removed, so an old model does not stay on disk for good.
     */
    fun remove(id: String): Boolean {
        if (!isSafeId(id)) throw ModelException.UnsafeName()
        if (!isDir(modelsDir)) return false
        val dir = joinPath(modelsDir, id)
        return locked(id, create = false) {
            if (entryKind(dir) == null) {
                false
            } else {
                io(dir) { removeAny(dir) }
                true
            }
        }
    }

    /** The catalog entry for [id]; an id that is not a plain name is refused without being echoed. */
    fun known(id: String): Model {
        if (!isSafeId(id)) throw ModelException.UnsafeName()
        return catalog[id] ?: throw ModelException.UnknownModel(id)
    }

    /**
     * One pull or removal per model at a time, across processes: two writers appending to one partial would corrupt
     * it. The lock file sits beside the model directory, so removing the directory leaves the lock in place.
     */
    private inline fun <T> locked(
        id: String,
        create: Boolean,
        block: () -> T,
    ): T {
        if (create) io(modelsDir) { createDirAll(modelsDir) }
        val path = joinPath(modelsDir, "$id.lock")
        val lock = io(path) { FileLock.tryExclusive(path.toString()) } ?: throw ModelException.Busy(id)
        return lock.use { block() }
    }

    /**
     * Files only get their final name after verification, so the pinned size is enough to tell an installed file
     * from damage; hashing hundreds of megabytes on every `ls` would not be.
     */
    private fun has(
        model: Model,
        file: ModelFile,
    ): Boolean {
        val path = joinPath(modelsDir, model.id, file.path)
        return entryKind(path) == EntryKind.File && linkSize(path) == file.size
    }
}
