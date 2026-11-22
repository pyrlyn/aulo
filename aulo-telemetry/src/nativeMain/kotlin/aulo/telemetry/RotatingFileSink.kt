package aulo.telemetry

import fs.appendSink
import fs.createDirAll
import fs.entryKind
import fs.linkSize
import fs.removeFile
import fs.rename
import kotlinx.io.Sink
import kotlinx.io.files.Path
import kotlin.concurrent.atomics.AtomicBoolean
import kotlin.concurrent.atomics.ExperimentalAtomicApi

/** `<home>/logs`, where aulo and aulod keep their log files. */
fun logDir(home: Path): Path = Path(home, "logs")

/**
 * Appends lines to `<dir>/<name>.log`. When a line would take the file past [maxBytes], the file
 * becomes `<name>.log.1`, older ones shift up, and only [keep] rotated files stay, so the logs
 * never grow without bound. Each line is flushed, so a crash loses at most the line being written.
 */
@OptIn(ExperimentalAtomicApi::class)
class RotatingFileSink(
    private val dir: Path,
    private val name: String = "aulo",
    private val maxBytes: Long = DEFAULT_MAX_BYTES,
    private val keep: Int = DEFAULT_KEEP,
) : LogSink,
    AutoCloseable {
    init {
        require(maxBytes > 0) { "maxBytes must be positive" }
        require(keep >= 0) { "keep must not be negative" }
    }

    val path: Path = Path(dir, "$name.log")

    // Threads share one logger; the critical section is a single write, so a spin is enough.
    private val held = AtomicBoolean(false)
    private var out: Sink? = null
    private var size = 0L

    override fun write(line: String) {
        val bytes = (line + "\n").encodeToByteArray()
        locked {
            val sink = out ?: open()
            val current =
                if (size > 0 && size + bytes.size > maxBytes) {
                    sink.close()
                    out = null
                    rotate()
                    open()
                } else {
                    sink
                }
            current.write(bytes)
            current.flush()
            size += bytes.size
        }
    }

    override fun close() {
        locked {
            out?.close()
            out = null
        }
    }

    private fun open(): Sink {
        createDirAll(dir)
        size = linkSize(path) ?: 0
        return appendSink(path).also { out = it }
    }

    private fun rotated(index: Int) = Path(dir, "$name.log.$index")

    private fun rotate() {
        if (keep == 0) {
            removeFile(path)
            return
        }
        if (entryKind(rotated(keep)) != null) removeFile(rotated(keep))
        for (index in keep - 1 downTo 1) {
            if (entryKind(rotated(index)) != null) rename(rotated(index), rotated(index + 1))
        }
        rename(path, rotated(1))
    }

    private inline fun locked(block: () -> Unit) {
        while (!held.compareAndSet(false, true)) {
            // Spin: another thread is mid-write.
        }
        try {
            block()
        } finally {
            held.store(false)
        }
    }

    companion object {
        const val DEFAULT_MAX_BYTES: Long = 10L * 1024 * 1024
        const val DEFAULT_KEEP: Int = 5
    }
}
