package aulo.grpc

import io.ktor.network.selector.SelectorManager
import io.ktor.network.sockets.InetSocketAddress
import io.ktor.network.sockets.ServerSocket
import io.ktor.network.sockets.Socket
import io.ktor.network.sockets.SocketAddress
import io.ktor.network.sockets.UnixSocketAddress
import io.ktor.network.sockets.aSocket
import io.ktor.network.sockets.isClosed
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.job
import kotlinx.coroutines.joinAll
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.io.files.Path
import kotlinx.io.files.SystemFileSystem
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds

/** A pause after a failed accept (out of file descriptors, say), so the loop does not spin. */
private val ACCEPT_BACKOFF = 100.milliseconds

/**
 * A gRPC server: [handlers] by method path (`/package.Service/Method`), served on every endpoint
 * passed to [bind], over TLS where one is given. It knows no aulo service; aulo-server supplies
 * the handlers.
 */
class GrpcServer(
    handlers: Map<String, ServerHandler>,
    private val limits: GrpcLimits = GrpcLimits(),
) {
    private val handlers = handlers.toMap()
    private val job = SupervisorJob()
    private val scope = CoroutineScope(job + Dispatchers.Default)
    private val selector = SelectorManager(Dispatchers.Default)
    private val mutex = Mutex()
    private val listeners = ArrayList<Pair<ServerSocket, Endpoint>>()

    // By the coroutine serving each; null while its TLS handshake runs.
    private val connections = HashMap<Job, ServerConnection?>()
    private var closing = false

    /**
     * Starts listening on [endpoint], with [tls] on every connection when given; returns the
     * endpoint with the port a TCP port 0 was given.
     */
    suspend fun bind(
        endpoint: Endpoint,
        tls: TlsServer? = null,
    ): Endpoint {
        val listener = aSocket(selector).tcp().bind(endpoint.address())
        val bound =
            when (endpoint) {
                is Endpoint.Tcp -> endpoint.copy(port = (listener.localAddress as InetSocketAddress).port)
                is Endpoint.Unix -> endpoint
            }
        mutex.withLock {
            if (closing) {
                listener.close()
                throw IllegalStateException("the server is shutting down")
            }
            listeners += listener to bound
        }
        scope.launch { accept(listener, bound, tls) }
        return bound
    }

    /**
     * Stops accepting, lets open calls finish for up to [grace], then cancels what is left and
     * removes the Unix socket files.
     */
    suspend fun shutdown(grace: Duration) {
        val (open, bound) =
            mutex.withLock {
                closing = true
                connections.toMap() to listeners.toList()
            }
        for ((listener, endpoint) in bound) {
            listener.close()
            if (endpoint is Endpoint.Unix) SystemFileSystem.delete(Path(endpoint.path), mustExist = false)
        }
        open.values.forEach { it?.goAway() }
        withTimeoutOrNull(grace) { open.keys.joinAll() }
        job.cancelAndJoin()
        selector.close()
    }

    private suspend fun accept(
        listener: ServerSocket,
        endpoint: Endpoint,
        tls: TlsServer?,
    ) {
        while (true) {
            val socket =
                try {
                    listener.accept()
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    if (listener.isClosed || mutex.withLock { closing }) return
                    delay(ACCEPT_BACKOFF)
                    continue
                }
            val admitted =
                mutex.withLock {
                    if (closing || connections.size >= limits.maxConnections) return@withLock false
                    // serve() takes the lock before it touches the map, so the entry is there first.
                    connections[scope.launch { serve(socket, endpoint, tls) }] = null
                    true
                }
            // Over the cap the peer gets a closed socket before any of its bytes are read.
            if (!admitted) socket.close()
        }
    }

    private suspend fun serve(
        socket: Socket,
        endpoint: Endpoint,
        tls: TlsServer?,
    ) {
        val self = currentCoroutineContext().job
        try {
            val raw = SocketStream(socket)
            // A failed handshake has closed the socket already.
            val stream = if (tls == null) raw else withTimeoutOrNull(limits.handshakeTimeout) { TlsStream.accept(raw, tls) } ?: return
            val connection =
                mutex.withLock {
                    if (closing) return@withLock null
                    ServerConnection(stream, endpoint, limits, job, handlers).also { connections[self] = it }
                } ?: return stream.close()
            connection.run()
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            // A peer that breaks the protocol, fails TLS or drops the socket ends only its own connection.
        } finally {
            withContext(NonCancellable) { mutex.withLock { connections.remove(self) } }
        }
    }
}

internal fun Endpoint.address(): SocketAddress =
    when (this) {
        is Endpoint.Tcp -> {
            InetSocketAddress(host, port)
        }

        is Endpoint.Unix -> {
            if (!UnixSocketAddress.isSupported()) throw UnsupportedOperationException("Unix sockets are not supported on this system")
            UnixSocketAddress(path)
        }
    }
