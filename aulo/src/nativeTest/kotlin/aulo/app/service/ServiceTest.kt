package aulo.app.service

import kotlinx.io.files.Path
import kotlin.experimental.ExperimentalNativeApi
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

@OptIn(ExperimentalNativeApi::class)
private val windows = Platform.osFamily == OsFamily.WINDOWS

private val root = if (windows) "C:\\" else "/"

private fun spec(exe: String = "aulo bin/aulod") =
    ServiceSpec(Path(root, exe).toString(), Path(root, "me", ".aulo").toString(), Path(root, "me").toString())

private val backends = listOf(Launchd, Systemd, TaskScheduler)

/** The definition the running service started from, as each fake manager keeps it. */
private fun FakeServiceManager.running(backend: ServiceBackend): ByteArray? =
    when (backend) {
        Launchd -> loaded["gui/${FakeServiceManager.UID}/${ServiceNames.LAUNCHD_LABEL}"]
        Systemd -> runningUnit
        else -> runningTask
    }

/** The definition on disk that the backend handed to its manager. */
private fun FakeServiceManager.definition(
    backend: ServiceBackend,
    spec: ServiceSpec,
): ByteArray? =
    when (backend) {
        Launchd -> files[Launchd.plistPath(spec).toString()]
        Systemd -> files[Systemd.unitPath(spec).toString()]
        else -> files[TaskScheduler.xmlPath(spec).toString()]
    }

// Calls that only ask, or start what already runs: a repeat install that makes none of the others
// changed nothing.
private fun String.asks() =
    startsWith("id ") ||
        this == "whoami" ||
        startsWith("launchctl print") ||
        startsWith("systemctl --user is-") ||
        startsWith("schtasks /Query") ||
        startsWith("schtasks /Run")

class ServiceTest {
    @Test
    fun install_starts_the_service_and_a_repeat_changes_nothing() {
        for (backend in backends) {
            val mgr = FakeServiceManager()
            val spec = spec()
            assertEquals(ServiceStatus.NotInstalled, backend.status(mgr, spec), "$backend")
            assertEquals(Installed.Fresh, backend.install(mgr, spec), "$backend")
            assertContentEquals(mgr.definition(backend, spec), mgr.running(backend), "$backend runs what it installed")
            assertEquals(ServiceStatus.Installed(running = true), backend.status(mgr, spec), "$backend")

            mgr.calls.clear()
            assertEquals(Installed.Unchanged, backend.install(mgr, spec), "$backend")
            assertEquals(emptyList(), mgr.calls.filterNot { it.asks() }, "$backend repeat install")
        }
    }

    @Test
    fun a_new_definition_replaces_the_running_one() {
        for (backend in backends) {
            val mgr = FakeServiceManager()
            backend.install(mgr, spec())
            val moved = spec(exe = "opt/aulo/aulod")
            assertEquals(Installed.Updated, backend.install(mgr, moved), "$backend")
            val now = mgr.definition(backend, moved)!!
            assertContains(now.decode(), "opt", message = "$backend")
            assertContentEquals(now, mgr.running(backend), "$backend restarted on the new definition")
        }
    }

    @Test
    fun install_starts_a_service_that_was_defined_but_stopped() {
        for (backend in backends) {
            val mgr = FakeServiceManager()
            val spec = spec()
            backend.install(mgr, spec)
            when (backend) {
                Launchd -> mgr.loaded.clear()
                Systemd -> mgr.active = false
                else -> mgr.runningTask = null
            }
            assertEquals(ServiceStatus.Installed(running = false), backend.status(mgr, spec), "$backend")
            assertEquals(if (backend == TaskScheduler) Installed.Unchanged else Installed.Updated, backend.install(mgr, spec), "$backend")
            val running = if (backend == TaskScheduler) mgr.task else mgr.definition(backend, spec)
            assertContentEquals(running, mgr.running(backend), "$backend")
        }
    }

    @Test
    fun uninstall_removes_the_service_and_a_repeat_says_there_was_none() {
        for (backend in backends) {
            val mgr = FakeServiceManager()
            val spec = spec()
            backend.install(mgr, spec)
            assertTrue(backend.uninstall(mgr, spec), "$backend")
            assertNull(mgr.running(backend), "$backend stopped")
            assertNull(mgr.definition(backend, spec), "$backend definition removed")
            assertEquals(ServiceStatus.NotInstalled, backend.status(mgr, spec), "$backend")
            assertFalse(backend.uninstall(mgr, spec), "$backend")
            if (backend == Systemd) assertFalse(mgr.enabled)
            if (backend == TaskScheduler) assertNull(mgr.task)
        }
    }

    @Test
    fun a_failing_tool_is_an_error_with_its_own_words() {
        for ((backend, tool) in listOf(
            Launchd to "launchctl bootstrap",
            Systemd to "systemctl --user enable",
            TaskScheduler to "schtasks /Create",
        )) {
            val mgr = FakeServiceManager()
            mgr.failing += tool
            val e = assertFailsWith<ServiceException> { backend.install(mgr, spec()) }
            assertContains(e.message!!, "$tool")
            assertContains(e.message!!, "refused by the test")
        }
    }

    @Test
    fun launchd_writes_the_home_and_log_and_escapes_xml() {
        // The paths below are POSIX ones, which a Windows host does not take as absolute.
        if (windows) return
        val spec = ServiceSpec("/a&b/<aulod>", "/me/\"x'.aulo", "/me")
        val plist = Launchd.render(spec)
        assertContains(plist, "<string>/a&amp;b/&lt;aulod&gt;</string>\n    <string>run</string>\n    <string>--home</string>\n")
        assertContains(plist, "<string>/me/&quot;x&apos;.aulo</string>\n  </array>")
        assertContains(plist, "<key>StandardErrorPath</key>\n  <string>/me/&quot;x&apos;.aulo/logs/aulod.service.log</string>")
        assertTrue(plist.startsWith("<?xml"), plist)
        assertTrue(plist.endsWith("</plist>\n"), plist)
    }

    @Test
    fun systemd_quotes_every_word() {
        if (windows) return
        assertEquals("\"/a b/%%h\$\$HOME\\\"\\\\\"", Systemd.quote("/a b/%h\$HOME\"\\"))
        val unit = Systemd.render(ServiceSpec("/opt/aulo 1/aulod", "/me/100%", "/me"))
        assertContains(unit, "\nExecStart=\"/opt/aulo 1/aulod\" \"run\" \"--home\" \"/me/100%%\"\n")
        assertContains(unit, "\nStandardOutput=append:/me/100%%/logs/aulod.service.log\n")
        assertTrue(unit.startsWith("[Unit]\n"), unit)
        assertTrue(unit.endsWith("WantedBy=default.target\n"), unit)
    }

    @Test
    fun the_task_runs_for_its_user_without_a_time_limit() {
        val home = Path(root, "me", "a b").toString()
        val xml =
            TaskScheduler.render(
                ServiceSpec(Path(root, "opt", "aulod").toString(), home, Path(root, "me").toString()),
                "HOST\\me & co",
            )
        assertContains(xml, "<UserId>HOST\\me &amp; co</UserId>")
        assertContains(xml, "<LogonType>InteractiveToken</LogonType>")
        assertContains(xml, "<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>")
        assertContains(xml, "<Arguments>run --home &quot;$home&quot;</Arguments>")
        assertFalse(xml.replace("\r\n", "").contains('\n'), "lines end in CRLF")
        val bytes = utf16(xml)
        assertEquals(listOf(0xFF.toByte(), 0xFE.toByte(), '<'.code.toByte(), 0), bytes.take(4))
    }

    @Test
    fun windows_arguments_survive_the_c_runtime() {
        assertEquals("run", TaskScheduler.windowsArg("run"))
        assertEquals("\"\"", TaskScheduler.windowsArg(""))
        assertEquals("\"C:\\a b\\\\\"", TaskScheduler.windowsArg("C:\\a b\\"))
        assertEquals("\"a\\\\\\\"b\"", TaskScheduler.windowsArg("a\\\"b"))
        assertEquals("C:\\a\\b", TaskScheduler.windowsArg("C:\\a\\b"))
    }

    @Test
    fun a_spec_refuses_what_would_break_a_definition() {
        assertFailsWith<ServiceException> { ServiceSpec("/opt/aulod\n[Service]", "/me/.aulo", "/me") }
        assertFailsWith<ServiceException> { ServiceSpec("aulod", "/me/.aulo", "/me") }
        assertFailsWith<ServiceException> { ServiceSpec("/opt/aulod", "", "/me") }
        val e = assertFailsWith<ServiceException> { ServiceSpec("/opt/aulod", "/me/\u0007", "/me") }
        assertContains(e.message!!, "\\u0007")
    }
}

private fun ByteArray.decode() = if (size > 1 && this[0] == 0xFF.toByte()) utf16Decode() else decodeToString()

private fun ByteArray.utf16Decode() =
    CharArray((size - 2) / 2) { i ->
        (
            (this[2 + i * 2].toInt() and 0xFF) or
                (this[3 + i * 2].toInt() shl 8)
        ).toChar()
    }.concatToString()
