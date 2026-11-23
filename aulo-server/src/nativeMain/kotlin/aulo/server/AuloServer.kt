package aulo.server

import aulo.grpc.Endpoint
import aulo.grpc.GrpcLimits
import aulo.grpc.GrpcServer
import aulo.grpc.ServerHandler
import aulo.server.generated.grpcHandlers
import kotlin.time.Duration

/**
 * aulod's gRPC server: the given service handlers (from the generated `grpcHandlers()` of each
 * aulo.v1 service), plus `grpc.health.v1` and `grpc.reflection.v1` for tooling.
 */
class AuloServer(
    handlers: Map<String, ServerHandler>,
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
        server = GrpcServer(handlers + builtIn, limits)
    }

    /** Opens [listeners]; the endpoints as bound. */
    suspend fun start(listeners: Listeners): List<Endpoint> {
        if (listeners.remote != null) {
            // The config parser already demands the TLS files; serving them is T3.8.
            throw ListenerConfigException("TLS for daemon.listen is not supported yet; unset daemon.listen")
        }
        return listOf(bind(listeners.local))
    }

    /** Opens one listener: for tests and tools; aulod opens what [start] gets from config. */
    suspend fun bind(endpoint: Endpoint): Endpoint = server.bind(endpoint)

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
