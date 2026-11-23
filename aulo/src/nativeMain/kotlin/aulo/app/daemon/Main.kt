package aulo.app.daemon

import aulo.secrets.systemSecretStore
import cli.terminalSettings
import cli.write
import fs.canon
import fs.createDirAll
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.coroutines.runBlocking
import kotlinx.io.files.Path
import platform.posix.fflush
import platform.posix.fprintf
import platform.posix.stderr
import process.envVars
import kotlin.system.exitProcess

fun main(args: Array<String>) {
    // First, while this is the only thread: the threads started later inherit the signal mask.
    installShutdownSignals()
    val result =
        runBlocking {
            runDaemonCli(
                args.asList(),
                envVars().toMap(),
                terminalSettings("aulod"),
                secrets = { home, warn ->
                    // The keychain namespace is the absolute path, which keeps two homes apart.
                    createDirAll(Path(home))
                    systemSecretStore(canon(Path(home)).toString(), Path(home, "secrets").toString(), warn)
                },
                log = ::logLine,
                stop = { logLine("aulod received ${awaitShutdownSignal()}") },
            )
        }
    result.write()
    exitProcess(result.code)
}

// Progress goes to stderr, flushed per line, so a supervisor sees it as it happens.
@OptIn(ExperimentalForeignApi::class)
private fun logLine(line: String) {
    fprintf(stderr, "%s\n", line)
    fflush(stderr)
}
