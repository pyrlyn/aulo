package aulo.grpc

import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds

/**
 * Caps on what a peer may send. Every peer is untrusted, a local client included, so each cap
 * holds on every listener.
 */
data class GrpcLimits(
    /** The largest message a peer may send; over it the call ends with RESOURCE_EXHAUSTED. */
    val maxMessageBytes: Int = 4 * 1024 * 1024,
    /** The largest header list (RFC 9113 §6.5.2 size); over it the call ends with RESOURCE_EXHAUSTED. */
    val maxHeaderListBytes: Int = 8 * 1024,
    /** Calls one connection may have open at once (SETTINGS_MAX_CONCURRENT_STREAMS). */
    val maxConcurrentStreams: Int = 100,
    /** Connections a server holds at once; one more is closed as soon as it is accepted. */
    val maxConnections: Int = 64,
    /** How long a TLS handshake may take; a connection still in it counts against [maxConnections]. */
    val handshakeTimeout: Duration = 10.seconds,
) {
    init {
        require(maxMessageBytes > 0) { "maxMessageBytes must be positive" }
        require(maxHeaderListBytes > 0) { "maxHeaderListBytes must be positive" }
        require(maxConcurrentStreams > 0) { "maxConcurrentStreams must be positive" }
        require(maxConnections > 0) { "maxConnections must be positive" }
        require(handshakeTimeout.isPositive()) { "handshakeTimeout must be positive" }
    }
}

/** Where a server listens or a client connects. */
sealed interface Endpoint {
    /** A TCP port; port 0 on a server binds a free one, which [GrpcServer.bind] returns. */
    data class Tcp(
        val host: String,
        val port: Int,
    ) : Endpoint

    /** A Unix domain socket at [path]. */
    data class Unix(
        val path: String,
    ) : Endpoint
}
