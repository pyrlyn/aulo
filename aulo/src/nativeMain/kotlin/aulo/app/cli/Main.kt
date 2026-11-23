package aulo.app.cli

import cli.Parsed
import cli.terminalSettings
import cli.write
import fs.canon
import kotlinx.io.files.Path
import process.envVars
import kotlin.system.exitProcess

fun main(args: Array<String>) {
    val result =
        runCli(
            args.asList(),
            envVars().toMap(),
            canon(Path(".")).toString(),
            terminalSettings("aulo"),
            progress = { Parsed.Exit(it, stderr = true, code = 0).write() },
        )
    result.write()
    exitProcess(result.code)
}
