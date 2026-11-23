package aulo.app.service

import kotlinx.io.files.Path

/**
 * Windows: a Task Scheduler task that starts at the installing user's logon. It is created
 * from XML rather than `/SC ONLOGON /TR`: only the XML lifts the default run-time limit, keeps
 * the task alive on battery and restarts it after a crash. A task cannot redirect output, so
 * aulod's own logs are what it leaves.
 */
object TaskScheduler : ServiceBackend {
    /** The definition last handed to `schtasks`, kept to tell a repeat install from a change. */
    fun xmlPath(spec: ServiceSpec) = Path(spec.home, "service", "${ServiceNames.TASK_NAME}.xml")

    override fun location(spec: ServiceSpec) = "the Task Scheduler task ${ServiceNames.TASK_NAME}"

    fun render(
        spec: ServiceSpec,
        user: String,
    ): String {
        val userId = escapeXml(user)
        return """
            |<?xml version="1.0" encoding="UTF-16"?>
            |<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
            |  <RegistrationInfo>
            |    <Description>${escapeXml(ServiceNames.DESCRIPTION)}</Description>
            |  </RegistrationInfo>
            |  <Triggers>
            |    <LogonTrigger>
            |      <Enabled>true</Enabled>
            |      <UserId>$userId</UserId>
            |    </LogonTrigger>
            |  </Triggers>
            |  <Principals>
            |    <Principal id="Author">
            |      <UserId>$userId</UserId>
            |      <LogonType>InteractiveToken</LogonType>
            |      <RunLevel>LeastPrivilege</RunLevel>
            |    </Principal>
            |  </Principals>
            |  <Settings>
            |    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
            |    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
            |    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
            |    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
            |    <RestartOnFailure>
            |      <Interval>PT1M</Interval>
            |      <Count>255</Count>
            |    </RestartOnFailure>
            |    <Enabled>true</Enabled>
            |  </Settings>
            |  <Actions Context="Author">
            |    <Exec>
            |      <Command>${escapeXml(spec.exe)}</Command>
            |      <Arguments>${escapeXml(spec.args.joinToString(" ") { windowsArg(it) })}</Arguments>
            |    </Exec>
            |  </Actions>
            |</Task>
            |
            """.trimMargin().replace("\n", "\r\n")
    }

    override fun install(
        mgr: ServiceManager,
        spec: ServiceSpec,
    ): Installed {
        val path = xmlPath(spec)
        val existed = exists(mgr)
        val user = mgr.must("whoami").stdout.trim()
        if (user.isEmpty() || user.any { it.isISOControl() }) throw ServiceException("whoami printed \"$user\", not a user name")
        val changed = mgr.sync(path, utf16(render(spec, user)))
        if (!changed && existed) {
            // Whether the task runs cannot be read reliably (the state is localised), and with
            // IgnoreNew a run of a running task does nothing, so a repeat install just runs it.
            mgr.must("schtasks", "/Run", "/TN", ServiceNames.TASK_NAME)
            return Installed.Unchanged
        }
        // `/F` replaces an existing task in place.
        mgr.must("schtasks", "/Create", "/XML", path.toString(), "/TN", ServiceNames.TASK_NAME, "/F")
        // A task that is running keeps its old command line until it is restarted.
        if (existed) mgr.query("schtasks", "/End", "/TN", ServiceNames.TASK_NAME)
        // A logon trigger fires only at the next logon; start it now, as the other systems do.
        mgr.must("schtasks", "/Run", "/TN", ServiceNames.TASK_NAME)
        return if (existed) Installed.Updated else Installed.Fresh
    }

    override fun uninstall(
        mgr: ServiceManager,
        spec: ServiceSpec,
    ): Boolean {
        val existed = exists(mgr)
        if (existed) {
            // Ending a task that is not running fails, which is fine.
            mgr.query("schtasks", "/End", "/TN", ServiceNames.TASK_NAME)
            mgr.must("schtasks", "/Delete", "/TN", ServiceNames.TASK_NAME, "/F")
        }
        val kept = mgr.read(xmlPath(spec)) != null
        if (kept) mgr.remove(xmlPath(spec))
        return existed
    }

    override fun status(
        mgr: ServiceManager,
        spec: ServiceSpec,
    ): ServiceStatus {
        val out = mgr.query("schtasks", "/Query", "/TN", ServiceNames.TASK_NAME, "/FO", "LIST")
        if (!out.success) return ServiceStatus.NotInstalled
        // The value is localised; outside English Windows this reports "not running", which errs safe.
        val running = out.stdout.lineSequence().any { it.trimEnd().endsWith("Running") }
        return ServiceStatus.Installed(running)
    }

    private fun exists(mgr: ServiceManager) = mgr.query("schtasks", "/Query", "/TN", ServiceNames.TASK_NAME).success

    /**
     * One argument as the C runtime splits a command line back: quoted when it holds a space, tab
     * or quote; backslashes doubled only before a quote, including the closing one.
     */
    internal fun windowsArg(arg: String): String {
        if (arg.isNotEmpty() && arg.none { it == ' ' || it == '\t' || it == '"' }) return arg
        return buildString {
            append('"')
            var backslashes = 0
            for (c in arg) {
                when (c) {
                    '\\' -> {
                        backslashes++
                        continue
                    }

                    '"' -> {
                        repeat(backslashes * 2 + 1) { append('\\') }
                    }

                    else -> {
                        repeat(backslashes) { append('\\') }
                    }
                }
                backslashes = 0
                append(c)
            }
            repeat(backslashes * 2) { append('\\') }
            append('"')
        }
    }
}

/** UTF-16 little-endian with a byte-order mark, the encoding the XML declares. */
internal fun utf16(text: String): ByteArray {
    val out = ByteArray(2 + text.length * 2)
    out[0] = 0xFF.toByte()
    out[1] = 0xFE.toByte()
    text.forEachIndexed { i, c ->
        out[2 + i * 2] = (c.code and 0xFF).toByte()
        out[3 + i * 2] = (c.code shr 8).toByte()
    }
    return out
}
