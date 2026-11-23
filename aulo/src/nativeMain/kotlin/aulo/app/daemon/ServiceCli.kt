package aulo.app.daemon

import aulo.app.service.Installed
import aulo.app.service.Launchd
import aulo.app.service.OsServiceManager
import aulo.app.service.ServiceBackend
import aulo.app.service.ServiceException
import aulo.app.service.ServiceManager
import aulo.app.service.ServiceSpec
import aulo.app.service.ServiceStatus
import aulo.app.service.Systemd
import aulo.app.service.TaskScheduler
import cli.Parsed
import fs.canon
import kotlinx.io.files.Path
import process.currentExe
import kotlin.experimental.ExperimentalNativeApi

/** What `aulod service` works with; a parameter so tests use a fake manager. */
class ServiceHost(
    /** Null on a system without a supported service manager. */
    val backend: ServiceBackend?,
    val manager: ServiceManager,
    /** This aulod's own path, which the service runs. */
    val exe: String?,
    val userHome: String?,
) {
    companion object {
        @OptIn(ExperimentalNativeApi::class)
        fun os(env: Map<String, String>) =
            ServiceHost(
                when (Platform.osFamily) {
                    OsFamily.MACOSX -> Launchd
                    OsFamily.LINUX -> Systemd
                    OsFamily.WINDOWS -> TaskScheduler
                    else -> null
                },
                OsServiceManager,
                currentExe(),
                env["HOME"]?.takeIf { it.isNotEmpty() } ?: env["USERPROFILE"]?.takeIf { it.isNotEmpty() },
            )
    }
}

/** `aulod service <action>` for the aulod at [home]. */
internal fun runService(
    action: String,
    home: String,
    host: ServiceHost,
): Parsed.Exit {
    val backend = host.backend ?: return failure("aulod service supports macOS, Linux and Windows only")
    val exe = host.exe ?: return failure("cannot find the path of this aulod")
    val userHome = host.userHome ?: return failure("cannot find the user's home: set HOME")
    return try {
        // The service starts with no working directory of ours, so every path in it is absolute.
        val spec = ServiceSpec(canon(Path(exe)).toString(), canon(Path(home)).toString(), userHome)
        val where = backend.location(spec)
        val text =
            when (action) {
                "install" -> {
                    val said =
                        when (backend.install(host.manager, spec)) {
                            Installed.Fresh -> "installed and started the aulod service: $where"
                            Installed.Updated -> "updated and restarted the aulod service: $where"
                            Installed.Unchanged -> "the aulod service is already installed: $where"
                        }
                    listOfNotNull(said, backend.logFile(spec)?.let { "its output goes to $it" }, backend.note).joinToString("\n")
                }

                "uninstall" -> {
                    if (backend.uninstall(host.manager, spec)) "removed the aulod service: $where" else "the aulod service is not installed"
                }

                "status" -> {
                    when (val status = backend.status(host.manager, spec)) {
                        ServiceStatus.NotInstalled -> {
                            "the aulod service is not installed"
                        }

                        is ServiceStatus.Installed -> {
                            val state = if (status.running) "running" else "not running"
                            "the aulod service is installed ($state): $where"
                        }
                    }
                }

                else -> {
                    error("unhandled service subcommand $action")
                }
            }
        Parsed.Exit("$text\n", stderr = false, code = 0)
    } catch (e: ServiceException) {
        failure(e.message)
    }
}

private fun failure(message: String?) = Parsed.Exit("error: $message\n", stderr = true, code = 1)
