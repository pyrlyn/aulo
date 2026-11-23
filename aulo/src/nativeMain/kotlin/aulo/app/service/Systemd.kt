package aulo.app.service

import kotlinx.io.files.Path

/** Linux: a systemd user unit, enabled for the user's default target. */
object Systemd : ServiceBackend {
    fun unitPath(spec: ServiceSpec) = Path(spec.userHome, ".config", "systemd", "user", ServiceNames.SYSTEMD_UNIT)

    override fun location(spec: ServiceSpec) = unitPath(spec).toString()

    override fun logFile(spec: ServiceSpec) = spec.logFile

    override val note =
        "systemd stops user services when you log out; `loginctl enable-linger` keeps aulod running without a session"

    fun render(spec: ServiceSpec): String {
        val exec = (listOf(spec.exe) + spec.args).joinToString(" ") { quote(it) }
        // `append:` takes the rest of the line as it is, so only a specifier `%` needs care.
        val log = spec.logFile.toString().replace("%", "%%")
        return """
            |[Unit]
            |Description=${ServiceNames.DESCRIPTION}
            |
            |[Service]
            |ExecStart=$exec
            |Restart=on-failure
            |RestartSec=5
            |StandardOutput=append:$log
            |StandardError=append:$log
            |
            |[Install]
            |WantedBy=default.target
            |
            """.trimMargin()
    }

    /**
     * One `ExecStart=` word in double quotes: `%` starts a specifier and `$` a variable anywhere
     * in it, so both are doubled.
     */
    internal fun quote(value: String) =
        "\"" +
            value
                .replace("\\", "\\\\")
                .replace("\"", "\\\"")
                .replace("%", "%%")
                .replace("$", "$$") +
            "\""

    override fun install(
        mgr: ServiceManager,
        spec: ServiceSpec,
    ): Installed {
        val path = unitPath(spec)
        val existed = mgr.read(path) != null
        mgr.createDirAll(spec.logDir)
        val changed = mgr.sync(path, render(spec).encodeToByteArray())
        val enabled = ctl("is-enabled").success(mgr)
        val active = ctl("is-active").success(mgr)
        if (!changed && enabled && active) return Installed.Unchanged
        if (changed) mgr.must("systemctl", "--user", "daemon-reload")
        mgr.must("systemctl", "--user", "enable", "--now", ServiceNames.SYSTEMD_UNIT)
        // `enable --now` leaves a running service on its old definition.
        if (changed && active) mgr.must("systemctl", "--user", "restart", ServiceNames.SYSTEMD_UNIT)
        return if (existed) Installed.Updated else Installed.Fresh
    }

    override fun uninstall(
        mgr: ServiceManager,
        spec: ServiceSpec,
    ): Boolean {
        val path = unitPath(spec)
        // Without the unit there is nothing to ask systemctl about, which also keeps uninstall
        // working on a host without a user systemd.
        if (mgr.read(path) == null) return false
        mgr.must("systemctl", "--user", "disable", "--now", ServiceNames.SYSTEMD_UNIT)
        mgr.remove(path)
        mgr.must("systemctl", "--user", "daemon-reload")
        return true
    }

    override fun status(
        mgr: ServiceManager,
        spec: ServiceSpec,
    ): ServiceStatus {
        if (mgr.read(unitPath(spec)) == null) return ServiceStatus.NotInstalled
        return ServiceStatus.Installed(running = ctl("is-active").success(mgr))
    }

    private fun ctl(verb: String) = listOf("--user", verb, ServiceNames.SYSTEMD_UNIT)

    private fun List<String>.success(mgr: ServiceManager) = mgr.run("systemctl", this).success
}
