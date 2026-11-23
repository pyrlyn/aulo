package aulo.app.cli

import aulo.app.BuildInfo
import aulo.config.ConfigException
import aulo.config.DEFAULT_TOML
import aulo.config.LoadOptions
import aulo.config.auloHome
import aulo.config.loadConfig
import aulo.config.render
import aulo.models.MODEL_HTTP_SETTINGS
import cli.Parsed
import cli.Settings
import cli.command
import cli.parse
import http.HttpClient
import http.ktor.KtorHttp
import kotlinx.coroutines.runBlocking

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
        subcommand("models") {
            doc(
                "Download, list and remove speech models\n\n" +
                    "Models come from a catalog built into aulo, pinned to exact files by size and SHA-256, and live in " +
                    "the models directory of the aulo home. A download that stops resumes on the next pull.",
            )
            subcommandRequired = true
            subcommand("ls") { doc("List the catalog with each model's install state.") }
            subcommand("pull") {
                doc("Download a model and check every file against its pinned SHA-256.")
                positional("id") {
                    required = true
                    valueName = "ID"
                    doc("The model id, as `aulo models ls` shows it")
                }
            }
            subcommand("rm") {
                doc("Delete a model, partial downloads included.")
                positional("id") {
                    required = true
                    valueName = "ID"
                    doc("The model id, as `aulo models ls` shows it")
                }
            }
        }
        subcommand("connect") {
            doc(
                "Check that a remote aulod answers with the pinned certificate\n\n" +
                    "A TLS handshake that accepts only the certificate with --pin, then a health check with the " +
                    "bearer token from $TOKEN_ENV when it is set. Nothing is saved.",
            )
            positional("addr") {
                required = true
                valueName = "ADDR"
                doc("The aulod's daemon.listen address: host:port or [ipv6]:port")
            }
            option("pin") {
                long = "pin"
                valueName = "SHA256"
                required = true
                doc("The certificate's SHA-256, as aulod logs it at start; colons optional")
            }
        }
    }

/**
 * Runs one command line. [env] is the process environment and [projectDir] the directory whose
 * `.aulo.toml` applies; both are parameters so tests run commands in-process from fixtures.
 * [http] opens the client a model download uses, and [progress] shows a long command's progress.
 */
fun runCli(
    args: List<String>,
    env: Map<String, String>,
    projectDir: String?,
    settings: Settings,
    http: () -> HttpClient = { KtorHttp(MODEL_HTTP_SETTINGS) },
    progress: (String) -> Unit = {},
): Parsed.Exit {
    val matches =
        when (val parsed = parse(tree, args, settings)) {
            is Parsed.Exit -> return parsed
            is Parsed.Run -> parsed.matches
        }
    // subcommandRequired makes clap's parser answer an empty level itself, so both are set.
    val config = matches.subcommand ?: error("no subcommand")
    if (config.name == "connect") {
        val addr = config.matches.value("addr") ?: error("no ADDR")
        val pin = config.matches.value("pin") ?: error("no --pin")
        return runBlocking { connect(addr, pin, env) }
    }
    if (config.name == "models") {
        val action = config.matches.subcommand ?: error("no models subcommand")
        val home = auloHome(env) ?: return Parsed.Exit("error: no aulo home: set AULO_HOME or HOME\n", stderr = true, code = 1)
        val id = if (action.name == "ls") null else action.matches.value("id")
        return runBlocking { models(action.name, id, modelManager(home), http, progress) }
    }
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
