package aulo.app.service

import fs.FsException
import fs.WriteOptions
import fs.bytesIfExists
import fs.createDirAll
import fs.linkSize
import fs.removeFile
import kotlinx.io.files.Path
import process.Command
import process.ProcessException
import process.runBounded
import kotlin.time.Duration.Companion.seconds

// launchctl, systemctl and schtasks answer in well under a second; a hung one must not hang the CLI.
private val TOOL_TIMEOUT = 30.seconds

// Their answers are a few KiB; `launchctl print` is the longest.
private const val TOOL_OUTPUT_BYTES = 256L * 1024

// A plist, a unit or a task definition is a few KiB; anything far larger is not ours.
private const val MAX_FILE_BYTES = 64 * 1024

/** The real file system and the real service tools, run without a shell. */
object OsServiceManager : ServiceManager {
    override fun read(path: Path): ByteArray? =
        fs {
            // Checked before reading, so a huge file never lands in memory.
            if ((linkSize(path) ?: 0) > MAX_FILE_BYTES) throw ServiceException("$path is larger than $MAX_FILE_BYTES bytes")
            bytesIfExists(path)
        }

    override fun write(
        path: Path,
        bytes: ByteArray,
    ) = fs {
        path.parent?.let { createDirAll(it) }
        WriteOptions().write(path, bytes)
    }

    override fun remove(path: Path) =
        fs {
            try {
                removeFile(path)
            } catch (e: FsException) {
                if (e.kind != FsException.Kind.NotFound) throw e
            }
        }

    override fun createDirAll(path: Path) = fs { fs.createDirAll(path) }

    override fun run(
        program: String,
        args: List<String>,
    ): ToolOutput {
        val out =
            try {
                Command(program, args).runBounded(TOOL_TIMEOUT, TOOL_OUTPUT_BYTES)
            } catch (e: ProcessException) {
                throw ServiceException(e.detail)
            }
        return ToolOutput(out.status.success, out.stdout.decodeToString(), out.stderr.decodeToString())
    }

    private inline fun <T> fs(block: () -> T): T =
        try {
            block()
        } catch (e: FsException) {
            throw ServiceException(e.message ?: "file system error")
        }
}
