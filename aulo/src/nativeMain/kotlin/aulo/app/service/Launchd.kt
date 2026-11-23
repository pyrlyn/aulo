package aulo.app.service

import kotlinx.io.files.Path

/** macOS: a LaunchAgent plist, loaded into the user's login session (`gui/<uid>`). */
object Launchd : ServiceBackend {
    fun plistPath(spec: ServiceSpec) = Path(spec.userHome, "Library", "LaunchAgents", "${ServiceNames.LAUNCHD_LABEL}.plist")

    override fun location(spec: ServiceSpec) = plistPath(spec).toString()

    override fun logFile(spec: ServiceSpec) = spec.logFile

    fun render(spec: ServiceSpec): String {
        val argv = (listOf(spec.exe) + spec.args).joinToString("") { "    ${xmlString(it)}\n" }
        val log = xmlString(spec.logFile.toString())
        return """
            |<?xml version="1.0" encoding="UTF-8"?>
            |<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
            |<plist version="1.0">
            |<dict>
            |  <key>Label</key>
            |  ${xmlString(ServiceNames.LAUNCHD_LABEL)}
            |  <key>ProgramArguments</key>
            |  <array>
            |${argv.trimEnd('\n')}
            |  </array>
            |  <key>RunAtLoad</key>
            |  <true/>
            |  <key>KeepAlive</key>
            |  <dict>
            |    <key>SuccessfulExit</key>
            |    <false/>
            |  </dict>
            |  <key>StandardOutPath</key>
            |  $log
            |  <key>StandardErrorPath</key>
            |  $log
            |</dict>
            |</plist>
            |
            """.trimMargin()
    }

    override fun install(
        mgr: ServiceManager,
        spec: ServiceSpec,
    ): Installed {
        val path = plistPath(spec)
        val existed = mgr.read(path) != null
        val (domain, target) = target(mgr)
        val wasLoaded = loaded(mgr, target) != null
        mgr.createDirAll(spec.logDir)
        val changed = mgr.sync(path, render(spec).encodeToByteArray())
        if (!changed && wasLoaded) return Installed.Unchanged
        // A loaded job keeps its old definition until it is booted out.
        if (wasLoaded) mgr.must("launchctl", "bootout", target)
        mgr.must("launchctl", "bootstrap", domain, path.toString())
        return if (existed || wasLoaded) Installed.Updated else Installed.Fresh
    }

    override fun uninstall(
        mgr: ServiceManager,
        spec: ServiceSpec,
    ): Boolean {
        val path = plistPath(spec)
        val (_, target) = target(mgr)
        val wasLoaded = loaded(mgr, target) != null
        if (mgr.read(path) == null && !wasLoaded) return false
        if (wasLoaded) mgr.must("launchctl", "bootout", target)
        mgr.remove(path)
        return true
    }

    override fun status(
        mgr: ServiceManager,
        spec: ServiceSpec,
    ): ServiceStatus {
        if (mgr.read(plistPath(spec)) == null) return ServiceStatus.NotInstalled
        val running = loaded(mgr, target(mgr).second)?.contains("state = running") == true
        return ServiceStatus.Installed(running)
    }

    /** The user's login session and the job in it. */
    private fun target(mgr: ServiceManager): Pair<String, String> {
        val uid = mgr.must("id", "-u").stdout.trim()
        if (uid.isEmpty() || !uid.all { it.isDigit() }) throw ServiceException("id -u printed \"$uid\", not a user id")
        val domain = "gui/$uid"
        return domain to "$domain/${ServiceNames.LAUNCHD_LABEL}"
    }

    /** `launchctl print` of the job, or null when it is not loaded. */
    private fun loaded(
        mgr: ServiceManager,
        target: String,
    ): String? = mgr.query("launchctl", "print", target).takeIf { it.success }?.stdout
}

private fun xmlString(value: String) = "<string>${escapeXml(value)}</string>"

internal fun escapeXml(value: String) =
    // `&` first, or the entities after it would be escaped again.
    value
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&apos;")
