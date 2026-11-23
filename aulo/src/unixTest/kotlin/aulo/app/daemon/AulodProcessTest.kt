package aulo.app.daemon

import aulo.server.FIRST_TOKEN
import aulo.server.RunFiles
import aulo.store.Store
import aulo.store.databasePath
import fs.TempDir
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.IO
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.io.files.Path
import kotlinx.io.files.SystemFileSystem
import platform.posix.SIGINT
import platform.posix.SIGTERM
import platform.posix.kill
import process.Command
import process.ExitStatus
import process.envVars
import process.runBounded
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

/** The linked `aulod` binary: one aulod per home, and SIGINT or SIGTERM stops it cleanly. */
class AulodProcessTest {
    private val aulod = envVars().toMap()["AULOD_BIN"] ?: error("AULOD_BIN is set by the Gradle test task")

    @Test
    fun sigterm_stops_aulod_and_a_second_one_is_refused_meanwhile() = stopsOn(SIGTERM, "SIGTERM", checkSecond = true)

    @Test
    fun sigint_stops_aulod() = stopsOn(SIGINT, "SIGINT", checkSecond = false)

    private fun stopsOn(
        signal: Int,
        name: String,
        checkSecond: Boolean,
    ) = runBlocking {
        TempDir.create(".aulod-test").use { tmp ->
            val home = tmp.path.toString()
            // A token already in the store: the first-run token would otherwise go to the login keychain.
            Store.open(databasePath(tmp.path).toString()).use { it.putApiToken(FIRST_TOKEN, "admin", "0".repeat(64)) }
            val command = Command(aulod, listOf("run")).env("AULO_HOME", home)
            val first = async(Dispatchers.IO) { command.runBounded(30.seconds, 64 * 1024) }
            val pid = withTimeout(20.seconds) { waitForPid(home) }
            waitForSocket(home)

            if (checkSecond) {
                val second = command.runBounded(20.seconds, 64 * 1024)
                assertEquals(ExitStatus.Exited(1), second.status)
                assertEquals("error: aulod is already running for $home (pid $pid)\n", second.stderr.decodeToString())
            }

            assertEquals(0, kill(pid.toInt(), signal))
            val output = first.await()
            val stderr = output.stderr.decodeToString()
            assertEquals(ExitStatus.Exited(0), output.status, stderr)
            assertEquals(
                listOf("aulod listening on unix:${Path(RunFiles.dir(home), "aulod.sock")}", "aulod received $name", "aulod shutting down"),
                stderr.lines().filter { it.isNotEmpty() },
            )
            assertFalse(SystemFileSystem.exists(Path(RunFiles.dir(home), RunFiles.PID)), "the pidfile is removed")
        }
    }

    private suspend fun waitForPid(home: String): Long {
        while (true) {
            RunFiles.readPid(home)?.let { return it }
            delay(20.milliseconds)
        }
    }

    // The pidfile comes first; the socket appears once aulod serves.
    private suspend fun waitForSocket(home: String) {
        withTimeout(20.seconds) {
            while (!SystemFileSystem.exists(Path(RunFiles.dir(home), "aulod.sock"))) delay(20.milliseconds)
        }
    }
}
