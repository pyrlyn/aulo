package aulo.app.daemon

import aulo.app.service.FakeServiceManager
import aulo.app.service.Systemd
import fs.TempDir
import fs.canon
import kotlinx.io.files.Path
import kotlin.test.Test
import kotlin.test.assertEquals

class ServiceCliTest {
    @Test
    fun install_uninstall_and_status_say_what_happened() =
        TempDir.create("aulo-service").use { tmp ->
            // canon resolves the temp directory's links, as the service paths are resolved.
            val dir = canon(tmp.path).toString()
            val home = Path(dir, "home").toString()
            val exe = Path(dir, "aulod").toString()
            val host = ServiceHost(Systemd, FakeServiceManager(), exe, dir)
            val unit = Path(dir, ".config", "systemd", "user", "aulod.service")
            val log = Path(home, "logs", "aulod.service.log")
            val note = "systemd stops user services when you log out; `loginctl enable-linger` keeps aulod running without a session"

            assertEquals("the aulod service is not installed\n", runService("status", home, host).text)
            assertEquals(
                "installed and started the aulod service: $unit\nits output goes to $log\n$note\n",
                runService("install", home, host).text,
            )
            assertEquals(
                "the aulod service is already installed: $unit\nits output goes to $log\n$note\n",
                runService("install", home, host).text,
            )
            assertEquals("the aulod service is installed (running): $unit\n", runService("status", home, host).text)
            assertEquals("removed the aulod service: $unit\n", runService("uninstall", home, host).text)
            assertEquals("the aulod service is not installed\n", runService("uninstall", home, host).text)
        }

    @Test
    fun errors_name_what_is_missing() =
        TempDir.create("aulo-service").use { tmp ->
            val dir = canon(tmp.path).toString()
            val home = Path(dir, ".aulo").toString()
            val exe = Path(dir, "aulod").toString()
            val fake = FakeServiceManager()
            val unsupported = runService("install", home, ServiceHost(null, fake, exe, dir))
            assertEquals(Triple("error: aulod service supports macOS, Linux and Windows only\n", true, 1), unsupported.triple())
            val noHome = runService("install", home, ServiceHost(Systemd, fake, exe, null))
            assertEquals("error: cannot find the user's home: set HOME\n", noHome.text)
            fake.failing += "systemctl --user enable"
            val refused = runService("install", home, ServiceHost(Systemd, fake, exe, dir))
            assertEquals(1, refused.code)
            assertEquals(
                "error: systemctl --user enable --now aulod.service failed: systemctl: refused by the test\n",
                refused.text,
            )
        }
}

private fun cli.Parsed.Exit.triple() = Triple(text, stderr, code)
