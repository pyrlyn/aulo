package aulo.app.cli

import aulo.app.BuildInfo
import aulo.config.ConfigException
import aulo.config.DEFAULT_TOML
import aulo.config.LoadOptions
import aulo.config.loadConfig
import aulo.config.render
import cli.Parsed
import cli.Settings
import cli.command
import cli.parse

private val tree =
    command("aulo") {
        doc("A local-first voice agent that controls your computer.")
        // clap prints `<name> <version>`, so the licence line rides along to match `versionText`.
        version = "${BuildInfo.VERSION} (${BuildInfo.GIT_SHA})\nLicense: ${BuildInfo.LICENSE}"
        subcommandRequired = true
        subcommand("config") {
            doc("Inspect the configuration.")
            subcommandRequired = true
            subcommand("show") {
                doc("Print the effective configuration as TOML, with secrets masked.")
                flag("origin") { doc("Name the layer each value comes from") }
            }
            subcommand("default") {
                doc("Print every key with its default, commented out, to copy from.")
            }
        }
    }

/**
 * Runs one command line. [env] is the process environment and [projectDir] the directory whose
 * `.aulo.toml` applies; both are parameters so tests run commands in-process from fixtures.
 */
fun runCli(
    args: List<String>,
    env: Map<String, String>,
    projectDir: String?,
    settings: Settings,
): Parsed.Exit {
    val matches =
        when (val parsed = parse(tree, args, settings)) {
            is Parsed.Exit -> return parsed
            is Parsed.Run -> parsed.matches
        }
    // subcommandRequired makes clap's parser answer an empty level itself, so both are set.
    val config = matches.subcommand ?: error("no subcommand")
    val action = config.matches.subcommand ?: error("no config subcommand")
    return when (action.name) {
        "show" -> {
            try {
                val loaded = loadConfig(LoadOptions(env = env, projectDir = projectDir))
                Parsed.Exit(loaded.render(withOrigin = action.matches.flag("origin")), stderr = false, code = 0)
            } catch (e: ConfigException) {
                Parsed.Exit("error: ${e.message}\n", stderr = true, code = 1)
            }
        }

        "default" -> {
            Parsed.Exit(DEFAULT_TOML, stderr = false, code = 0)
        }

        else -> {
            error("unhandled subcommand ${action.name}")
        }
    }
}
