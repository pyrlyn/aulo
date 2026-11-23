package aulo.app.cli

import aulo.models.Catalog
import aulo.models.ModelException
import aulo.models.ModelManager
import aulo.models.ModelStatus
import aulo.models.Progress
import cli.Parsed
import http.HttpClient

private const val BYTES_PER_MB = 1_000_000L
private const val TENTHS = 10
private const val PERCENT = 100

/** How often, in percent of the whole model, `aulo models pull` reports progress. */
private const val PROGRESS_STEP = 5

/**
 * `aulo models ls|pull|rm` over [manager]. [http] is opened only for a pull; [progress] gets one line at a time while
 * a pull runs, so the user sees a long download move.
 */
internal suspend fun models(
    action: String,
    id: String?,
    manager: ModelManager,
    http: () -> HttpClient,
    progress: (String) -> Unit,
): Parsed.Exit =
    try {
        when (action) {
            "ls" -> {
                Parsed.Exit(table(manager.list()), stderr = false, code = 0)
            }

            "pull" -> {
                val model = manager.known(checkNotNull(id))
                progress("${model.name}: ${model.license}, ${mb(model.totalSize)}\n")
                val dir = http().use { client -> manager.pull(model.id, client, Reporter(progress)::report) }
                Parsed.Exit("${model.id} installed in $dir\n", stderr = false, code = 0)
            }

            "rm" -> {
                val removed = manager.remove(checkNotNull(id))
                Parsed.Exit(if (removed) "removed $id\n" else "$id is not installed\n", stderr = false, code = 0)
            }

            else -> {
                error("unhandled models subcommand $action")
            }
        }
    } catch (e: ModelException) {
        Parsed.Exit("error: ${e.message}\n", stderr = true, code = 1)
    }

/** The embedded catalog under [home]. */
internal fun modelManager(home: String): ModelManager = ModelManager(kotlinx.io.files.Path(home), Catalog.embedded())

private fun table(models: List<ModelStatus>): String {
    val rows =
        listOf(listOf("ID", "KIND", "SIZE", "STATE", "LICENSE")) +
            models.map { s ->
                val state =
                    when (s.filesPresent) {
                        s.model.files.size -> "installed"
                        0 -> "missing"
                        else -> "partial ${s.filesPresent}/${s.model.files.size}"
                    }
                listOf(
                    s.model.id,
                    s.model.kind.name
                        .lowercase(),
                    mb(s.model.totalSize),
                    state,
                    s.model.license,
                )
            }
    val widths = rows[0].indices.map { col -> rows.maxOf { it[col].length } }
    return rows.joinToString("") { row ->
        row.mapIndexed { col, cell -> if (col == row.lastIndex) cell else cell.padEnd(widths[col]) }.joinToString("  ") + "\n"
    }
}

/** Decimal megabytes with one digit, as download sizes are usually given. */
internal fun mb(bytes: Long): String {
    val tenths = (bytes * TENTHS + BYTES_PER_MB / 2) / BYTES_PER_MB
    return "${tenths / TENTHS}.${tenths % TENTHS} MB"
}

/** One line per finished file and per [PROGRESS_STEP] percent of the model, not one per network chunk. */
private class Reporter(
    private val out: (String) -> Unit,
) {
    private var lastStep = -1L

    fun report(p: Progress) {
        if (p.fileBytes == p.fileSize) out("  ${p.file} (${mb(p.fileSize)})\n")
        val step = if (p.totalBytes == 0L) PERCENT.toLong() else p.bytes * PERCENT / p.totalBytes / PROGRESS_STEP
        if (step != lastStep) {
            lastStep = step
            out("  ${step * PROGRESS_STEP}% (${mb(p.bytes)} of ${mb(p.totalBytes)})\n")
        }
    }
}
