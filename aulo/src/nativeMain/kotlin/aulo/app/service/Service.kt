package aulo.app.service

import kotlinx.io.files.Path

/**
 * What installing a login service asks of the machine, and nothing else: files and programs.
 * Backends talk only to this, so tests drive every one of them against a fake on any host.
 */
interface ServiceManager {
    /** The file's bytes, or null when it does not exist. */
    fun read(path: Path): ByteArray?

    /** Writes the file whole, creating missing parent directories. */
    fun write(
        path: Path,
        bytes: ByteArray,
    )

    /** A file that is already gone is not an error. */
    fun remove(path: Path)

    fun createDirAll(path: Path)

    /** Runs [program] with [args], no shell. A program that ran and failed is a [ToolOutput] with [ToolOutput.success] false. */
    fun run(
        program: String,
        args: List<String>,
    ): ToolOutput
}

class ToolOutput(
    val success: Boolean,
    val stdout: String = "",
    val stderr: String = "",
)

/** Why a login service could not be installed, removed or queried. */
class ServiceException(
    message: String,
) : Exception(message)

/** The names that tell aulod's login service apart from any other program's. */
object ServiceNames {
    const val LAUNCHD_LABEL = "dev.aulo.aulod"
    const val SYSTEMD_UNIT = "aulod.service"
    const val TASK_NAME = "aulod"
    const val DESCRIPTION = "aulo daemon"
    const val LOG_FILE = "aulod.service.log"
}

/**
 * The login service for the aulod at [exe] with [home]: it runs `<exe> run --home <home>`, so it
 * needs no environment of its own. [userHome] is where launchd and systemd keep their units.
 */
class ServiceSpec(
    val exe: String,
    val home: String,
    val userHome: String,
) {
    init {
        for ((what, value) in listOf("aulod path" to exe, "aulo home" to home, "user home" to userHome)) {
            if (value.isEmpty()) throw ServiceException("the $what is empty")
            // A line break would start a new directive in a unit file or a plist line.
            if (value.any { it.isISOControl() }) throw ServiceException("the $what holds a control character: ${value.escaped()}")
            if (!Path(value).isAbsolute) throw ServiceException("the $what is not an absolute path: $value")
        }
    }

    /** The command line after [exe]. */
    val args: List<String> get() = listOf("run", "--home", home)

    val logDir: Path get() = Path(home, "logs")

    val logFile: Path get() = Path(logDir, ServiceNames.LOG_FILE)
}

enum class Installed { Fresh, Updated, Unchanged }

sealed interface ServiceStatus {
    data object NotInstalled : ServiceStatus

    data class Installed(
        val running: Boolean,
    ) : ServiceStatus
}

/** One operating system's login service manager. */
interface ServiceBackend {
    /** Where the service manager is told about aulod, for messages. */
    fun location(spec: ServiceSpec): String

    /** Where the service manager writes aulod's output, when it keeps it. */
    fun logFile(spec: ServiceSpec): Path? = null

    /** What the user should know after an install on this system. */
    val note: String? get() = null

    /** Installs and starts the service; a repeat with the same definition changes nothing. */
    fun install(
        mgr: ServiceManager,
        spec: ServiceSpec,
    ): Installed

    /** Stops and removes the service: false when there was none. */
    fun uninstall(
        mgr: ServiceManager,
        spec: ServiceSpec,
    ): Boolean

    fun status(
        mgr: ServiceManager,
        spec: ServiceSpec,
    ): ServiceStatus
}

/** Runs [program]; a failure is the caller's to judge. */
internal fun ServiceManager.query(
    program: String,
    vararg args: String,
): ToolOutput = run(program, args.toList())

/** Runs [program] and turns a failure into a [ServiceException] that carries the tool's own words. */
internal fun ServiceManager.must(
    program: String,
    vararg args: String,
): ToolOutput {
    val out = run(program, args.toList())
    if (!out.success) {
        val said = out.stderr.trim().ifEmpty { out.stdout.trim() }
        throw ServiceException("$program ${args.joinToString(" ")} failed" + if (said.isEmpty()) "" else ": $said")
    }
    return out
}

/** Writes [bytes] only when they differ from the file, and says whether they did. */
internal fun ServiceManager.sync(
    path: Path,
    bytes: ByteArray,
): Boolean {
    if (read(path)?.contentEquals(bytes) == true) return false
    write(path, bytes)
    return true
}

private fun String.escaped() =
    buildString {
        this@escaped.forEach { if (it.isISOControl()) append("\\u").append(it.code.toString(16).padStart(4, '0')) else append(it) }
    }
