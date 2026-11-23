package aulo.app.service

import kotlinx.io.files.Path

/**
 * launchctl, systemctl and schtasks as far as aulod uses them, with their state in memory. Each
 * remembers the definition it runs, so a test can tell a service restarted on a new definition
 * from one left on the old.
 */
class FakeServiceManager : ServiceManager {
    val files = mutableMapOf<String, ByteArray>()
    val dirs = mutableSetOf<String>()

    /** Every command line run, in order. */
    val calls = mutableListOf<String>()

    /** Command lines starting with one of these fail as the tool would. */
    val failing = mutableSetOf<String>()

    // launchd: loaded jobs and the plist each was loaded from.
    val loaded = mutableMapOf<String, ByteArray>()

    // systemd: what daemon-reload last read, and what the running service started from.
    private var reloaded: ByteArray? = null
    var enabled = false
    var active = false
    var runningUnit: ByteArray? = null

    // Task Scheduler: the registered task and the definition its running instance started from.
    var task: ByteArray? = null
    var runningTask: ByteArray? = null

    override fun read(path: Path) = files[path.toString()]

    override fun write(
        path: Path,
        bytes: ByteArray,
    ) {
        files[path.toString()] = bytes
    }

    override fun remove(path: Path) {
        files.remove(path.toString())
    }

    override fun createDirAll(path: Path) {
        dirs += path.toString()
    }

    override fun run(
        program: String,
        args: List<String>,
    ): ToolOutput {
        val line = (listOf(program) + args).joinToString(" ")
        calls += line
        if (failing.any { line.startsWith(it) }) return ToolOutput(false, stderr = "$program: refused by the test\n")
        return when (program) {
            "id" -> ToolOutput(true, "$UID\n")
            "whoami" -> ToolOutput(true, "$USER\r\n")
            "launchctl" -> launchctl(args)
            "systemctl" -> systemctl(args.drop(1).also { check(args.first() == "--user") })
            "schtasks" -> schtasks(args)
            else -> error("unexpected program $program")
        }
    }

    private fun launchctl(args: List<String>): ToolOutput =
        when (args[0]) {
            "print" -> {
                if (args[1] in loaded) ToolOutput(true, "${args[1]} = {\n\tstate = running\n}\n") else fail("Could not find service")
            }

            "bootstrap" -> {
                val plist = files[args[2]] ?: return fail("Bootstrap failed: 2: No such file or directory")
                val target = "${args[1]}/${ServiceNames.LAUNCHD_LABEL}"
                // launchd refuses to load a job that is already loaded.
                if (target in loaded) return fail("Bootstrap failed: 5: Input/output error")
                loaded[target] = plist
                ToolOutput(true)
            }

            "bootout" -> {
                if (loaded.remove(args[1]) != null) ToolOutput(true) else fail("Boot-out failed: 3: No such process")
            }

            else -> {
                error("unexpected launchctl ${args[0]}")
            }
        }

    private fun systemctl(args: List<String>): ToolOutput {
        val unitFile = files.entries.firstOrNull { Path(it.key).name == ServiceNames.SYSTEMD_UNIT }?.value
        return when (args) {
            listOf("is-enabled", ServiceNames.SYSTEMD_UNIT) -> {
                ToolOutput(enabled)
            }

            listOf("is-active", ServiceNames.SYSTEMD_UNIT) -> {
                ToolOutput(active)
            }

            listOf("daemon-reload") -> {
                ToolOutput(true).also { reloaded = unitFile }
            }

            listOf("enable", "--now", ServiceNames.SYSTEMD_UNIT) -> {
                val known = reloaded ?: return fail("Unit ${ServiceNames.SYSTEMD_UNIT} not found.")
                enabled = true
                if (!active) {
                    active = true
                    runningUnit = known
                }
                ToolOutput(true)
            }

            listOf("restart", ServiceNames.SYSTEMD_UNIT) -> {
                runningUnit = reloaded ?: return fail("Unit ${ServiceNames.SYSTEMD_UNIT} not found.")
                active = true
                ToolOutput(true)
            }

            listOf("disable", "--now", ServiceNames.SYSTEMD_UNIT) -> {
                enabled = false
                active = false
                runningUnit = null
                ToolOutput(true)
            }

            else -> {
                error("unexpected systemctl $args")
            }
        }
    }

    private fun schtasks(args: List<String>): ToolOutput {
        check(args[args.indexOf("/TN") + 1] == ServiceNames.TASK_NAME)
        return when (args[0]) {
            "/Query" -> {
                if (task == null) return fail("ERROR: The system cannot find the file specified.")
                ToolOutput(true, "TaskName:      \\aulod\r\nStatus:        ${if (runningTask != null) "Running" else "Ready"}\r\n")
            }

            "/Create" -> {
                check(args.last() == "/F")
                task = files[args[args.indexOf("/XML") + 1]] ?: return fail("ERROR: The system cannot find the file specified.")
                ToolOutput(true)
            }

            "/Run" -> {
                val registered = task ?: return fail("ERROR: The system cannot find the file specified.")
                // MultipleInstancesPolicy IgnoreNew: a running instance stays as it is.
                if (runningTask == null) runningTask = registered
                ToolOutput(true)
            }

            "/End" -> {
                if (runningTask != null) ToolOutput(true).also { runningTask = null } else fail("ERROR: The task is not running.")
            }

            "/Delete" -> {
                if (task !=
                    null
                ) {
                    ToolOutput(true).also { task = null }
                } else {
                    fail("ERROR: The system cannot find the file specified.")
                }
            }

            else -> {
                error("unexpected schtasks ${args[0]}")
            }
        }
    }

    private fun fail(stderr: String) = ToolOutput(false, stderr = "$stderr\n")

    companion object {
        const val UID = "501"
        const val USER = "HOST\\me"
    }
}
