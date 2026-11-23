package aulo.app.daemon

import aulo.app.BuildInfo
import aulo.config.ConfigException
import aulo.config.LoadOptions
import aulo.config.auloHome
import aulo.config.loadConfig
import aulo.secrets.SecretException
import aulo.secrets.SecretStore
import aulo.server.AlreadyRunningException
import aulo.server.ListenerConfigException
import aulo.server.runDaemon
import aulo.store.StoreException
import cli.Parsed
import cli.Settings
import cli.command
import cli.parse
import fs.FsException
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

private val tree =
    command("aulod") {
        doc("The aulo daemon: serves the aulo.v1 API to the local socket and, when configured, TCP.")
        // clap prints `<name> <version>`, so the licence line rides along to match `versionText`.
        version = "${BuildInfo.VERSION} (${BuildInfo.GIT_SHA})\nLicense: ${BuildInfo.LICENSE}"
        subcommandRequired = true
        subcommand("run") {
            doc("Run in the foreground until SIGINT or SIGTERM (Ctrl+C on Windows).")
            option("config") {
                long = "config"
                valueName = "FILE"
                doc("Read this file instead of <home>/config.toml; the home itself stays where it is")
            }
            option("listen") {
                long = "listen"
                valueName = "ADDR"
                doc("TCP address for remote clients (host:port), overriding daemon.listen; needs TLS")
            }
        }
    }

/**
 * Runs one `aulod` command line. [secrets] opens the secret store of a home, [log] hears progress
 * lines and [stop] returns when the daemon should shut down; all are parameters so tests run
 * the daemon in-process.
 */
suspend fun runDaemonCli(
    args: List<String>,
    env: Map<String, String>,
    settings: Settings,
    secrets: (home: String, warn: (String) -> Unit) -> SecretStore,
    log: (String) -> Unit,
    stop: suspend () -> Unit,
): Parsed.Exit {
    val matches =
        when (val parsed = parse(tree, args, settings)) {
            is Parsed.Exit -> return parsed
            is Parsed.Run -> parsed.matches
        }
    // subcommandRequired makes clap's parser answer an empty command line itself.
    val run = matches.subcommand ?: error("no subcommand")
    check(run.name == "run") { "unhandled subcommand ${run.name}" }
    val home = auloHome(env) ?: return failure("cannot find the aulo home: set AULO_HOME or HOME")
    val overrides =
        run.matches.value("listen")?.let { listen ->
            JsonObject(mapOf("daemon" to JsonObject(mapOf("listen" to JsonPrimitive(listen)))))
        }
    return try {
        val config = loadConfig(LoadOptions(env = env, home = home, overrides = overrides, configFile = run.matches.value("config"))).config
        runDaemon(home, config, secrets(home) { log("warning: $it") }, log, stop)
        Parsed.Exit("", stderr = false, code = 0)
    } catch (e: ConfigException) {
        failure(e.message)
    } catch (e: ListenerConfigException) {
        failure(e.message)
    } catch (e: AlreadyRunningException) {
        failure(e.message)
    } catch (e: StoreException) {
        failure(e.message)
    } catch (e: SecretException) {
        failure(e.message)
    } catch (e: FsException) {
        failure(e.message)
    }
}

private fun failure(message: String?) = Parsed.Exit("error: $message\n", stderr = true, code = 1)
