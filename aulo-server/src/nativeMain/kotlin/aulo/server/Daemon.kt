package aulo.server

import aulo.config.Config
import aulo.grpc.Endpoint
import aulo.grpc.ServerHandler
import aulo.secrets.OwnerOnlyException
import aulo.secrets.SecretStore
import aulo.secrets.ensureOwnerOnlyDir
import aulo.server.generated.grpcHandlers
import aulo.store.Store
import aulo.store.databasePath
import aulo.v1.ModelRef
import fs.FileLock
import fs.FsException
import fs.atomic
import fs.bytesIfExists
import fs.createDirAll
import fs.removeFile
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.withContext
import kotlinx.io.files.Path
import platform.posix.getpid
import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds

/** Another aulod already serves this home; [pid] is what its pidfile says, when it is readable. */
class AlreadyRunningException(
    val home: String,
    val pid: Long?,
) : Exception("aulod is already running for $home" + (pid?.let { " (pid $it)" } ?: ""))

/** The run directory holds the socket, the single-instance lock and the pidfile. */
object RunFiles {
    const val LOCK = "aulod.lock"
    const val PID = "aulod.pid"

    fun dir(home: String): Path = Path(home, Listeners.RUN_DIR)

    /** The pid in [home]'s pidfile, or null when there is none or it is not a pid. */
    fun readPid(home: String): Long? =
        bytesIfExists(Path(dir(home), PID))
            // A pidfile is ours to write but anyone's to corrupt: only a short plain number is read.
            ?.takeIf { it.size <= 24 }
            ?.decodeToString()
            ?.trim()
            ?.toLongOrNull()
            ?.takeIf { it > 0 }
}

/** The aulo.v1 services aulod serves, bound for [AuloServer]. */
fun serviceHandlers(
    store: SharedStore,
    defaultModel: () -> ModelRef?,
): Map<String, ServerHandler> = ChatService(store, defaultModel).grpcHandlers()

/**
 * Runs aulod for [home] until [stop] returns, then shuts down gracefully, giving open calls
 * [grace]. Only one aulod runs per home: the lock in [RunFiles] is held for the whole run, and
 * a second one fails with [AlreadyRunningException] before it touches anything else. Problems
 * a person can fix (listeners, the run directory) fail with [ListenerConfigException] before the
 * lock is taken.
 */
suspend fun runDaemon(
    home: String,
    config: Config,
    secrets: SecretStore,
    log: (String) -> Unit,
    stop: suspend () -> Unit,
    grace: Duration = 5.seconds,
) {
    val listeners = Listeners.from(config.daemon, home)
    val run = RunFiles.dir(home)
    try {
        createDirAll(Path(home))
        ensureOwnerOnlyDir(run.toString())
    } catch (e: FsException) {
        throw ListenerConfigException("cannot create $run: ${e.detail}")
    } catch (e: OwnerOnlyException) {
        throw ListenerConfigException(e.message ?: "$run is not owner-only")
    }
    val lock = FileLock.tryExclusive(Path(run, RunFiles.LOCK).toString()) ?: throw AlreadyRunningException(home, RunFiles.readPid(home))
    val pidFile = Path(run, RunFiles.PID)
    try {
        // Written under the lock, so a pidfile names the live aulod or one that died holding it.
        atomic(pidFile, "${getpid()}\n".encodeToByteArray())
        Store.open(databasePath(Path(home)).toString()).use { store ->
            val tokens = ensureFirstToken(store, secrets)
            val main = config.providers.tiers.main
            val server = AuloServer(serviceHandlers(SharedStore(store)) { main?.let { ModelRef(it.provider, it.model) } }, tokens)
            val endpoints = server.start(listeners)
            try {
                log("aulod listening on ${endpoints.joinToString { it.display() }}")
                stop()
            } finally {
                log("aulod shutting down")
                withContext(NonCancellable) { server.shutdown(grace) }
            }
        }
    } finally {
        try {
            removeFile(pidFile)
        } catch (_: FsException) {
            // Already gone; the lock, not the pidfile, decides whether aulod runs.
        }
        lock.close()
    }
}

private fun Endpoint.display(): String =
    when (this) {
        is Endpoint.Unix -> "unix:$path"
        is Endpoint.Tcp -> if (':' in host) "[$host]:$port" else "$host:$port"
    }
