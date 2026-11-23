package aulo.server

import aulo.grpc.Endpoint
import aulo.grpc.GrpcLimits
import aulo.grpc.GrpcServer
import aulo.grpc.ServerHandler
import aulo.grpc.TlsServer
import aulo.secrets.OwnerOnlyException
import aulo.secrets.ensureOwnerOnlyDir
import aulo.secrets.restrictToOwner
import aulo.server.generated.grpcHandlers
import kotlin.time.Duration

/**
 * aulod's gRPC server: the given service handlers (from the generated `grpcHandlers()` of each
 * aulo.v1 service), plus `grpc.health.v1` and `grpc.reflection.v1` for tooling. Every method,
 * the built-in ones too, passes [Authenticator] first: TCP calls need one of [tokenHashes].
 */
class AuloServer(
    handlers: Map<String, ServerHandler>,
    tokenHashes: Collection<String> = emptyList(),
    limits: GrpcLimits = GrpcLimits(),
) {
    /** Every service starts SERVING; [shutdown] turns them all NOT_SERVING first. */
    val health: HealthService
    private val server: GrpcServer

    init {
        val services = handlers.keys.map(::serviceOf).toSet() + HEALTH + REFLECTION
        health = HealthService(services)
        val builtIn = health.grpcHandlers() + ReflectionService(services).grpcHandlers()
        val clash = handlers.keys intersect builtIn.keys
        require(clash.isEmpty()) { "these methods are built in: ${clash.joinToString()}" }
        val auth = Authenticator(tokenHashes)
        val guarded =
            (handlers + builtIn).mapValues { (_, handler) ->
                ServerHandler { call ->
                    auth.check(call)
                    handler.handle(call)
                }
            }
        server = GrpcServer(guarded, limits)
    }

    /**
     * Opens [listeners]; the endpoints as bound. The socket's directory is made owner-only
     * before the bind, so nobody else can reach the socket at any moment. The TCP listener, when
     * there is one, serves [remoteTls] ([serverTls] makes it from the config).
     */
    suspend fun start(
        listeners: Listeners,
        remoteTls: TlsServer? = null,
    ): List<Endpoint> {
        require((listeners.remote == null) == (remoteTls == null)) { "a TCP listener needs TLS, and only it takes TLS" }
        val socket = listeners.local.path
        ownerOnly { ensureOwnerOnlyDir(socket.substring(0, maxOf(socket.lastIndexOf('/'), socket.lastIndexOf('\\')))) }
        val bound = bind(listeners.local)
        // The directory is the guard; 0600 keeps the socket closed even if it is loosened later.
        ownerOnly { restrictToOwner(socket) }
        val remote = listeners.remote ?: return listOf(bound)
        return listOf(bound, bind(remote.endpoint, remoteTls))
    }

    private inline fun ownerOnly(block: () -> Unit) {
        try {
            block()
        } catch (e: OwnerOnlyException) {
            throw ListenerConfigException(e.message ?: "the socket directory is not owner-only")
        }
    }

    /** Opens one listener, with [tls] when given: for tests and tools; aulod opens what [start] gets from config. */
    suspend fun bind(
        endpoint: Endpoint,
        tls: TlsServer? = null,
    ): Endpoint = server.bind(endpoint, tls)

    /**
     * Health turns NOT_SERVING (open Watch streams end), listeners close, and open calls get
     * [grace] to finish before they are cancelled.
     */
    suspend fun shutdown(grace: Duration) {
        health.shutDown()
        server.shutdown(grace)
    }

    companion object {
        const val HEALTH = "grpc.health.v1.Health"
        const val REFLECTION = "grpc.reflection.v1.ServerReflection"

        /** `/package.Service/Method` to `package.Service`. */
        private fun serviceOf(path: String) = path.removePrefix("/").substringBefore('/')
    }
}
