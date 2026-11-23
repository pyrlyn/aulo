package aulo.grpc

import io.ktor.network.selector.SelectorManager
import io.ktor.network.sockets.aSocket
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.time.Duration

/**
 * A gRPC client over one HTTP/2 connection. When the connection ends, every call fails with
 * UNAVAILABLE; connect again for a new one.
 */
class GrpcClient private constructor(
    private val connection: ClientConnection,
    private val limits: GrpcLimits,
    private val authority: String,
    private val job: Job,
    private val selector: SelectorManager,
) {
    private val scope = CoroutineScope(job + Dispatchers.Default)

    /** Starts a call to [path]; with [timeout], the call ends with DEADLINE_EXCEEDED when it passes. */
    fun call(
        path: String,
        metadata: List<Pair<String, String>> = emptyList(),
        timeout: Duration? = null,
    ): ClientCall {
        require(path.startsWith("/")) { "a method path starts with /: $path" }
        for ((name, _) in metadata) {
            require(name.isNotEmpty() && name == name.lowercase() && !name.startsWith(":") && name !in RESERVED_HEADERS) {
                "metadata name $name is empty, not lowercase, or reserved"
            }
        }
        val stream = ClientConnection.Stream(limits.maxMessageBytes)
        val call = ClientCall(connection, stream)
        if (timeout != null && !timeout.isPositive()) {
            stream.status.complete(Status(StatusCode.DEADLINE_EXCEEDED, "deadline exceeded"))
            return call
        }
        val headers =
            buildList {
                add(":method" to "POST")
                add(":scheme" to "http")
                add(":path" to path)
                add(":authority" to authority)
                add("content-type" to "application/grpc")
                add("te" to "trailers")
                if (timeout != null) add("grpc-timeout" to GrpcTimeout.format(timeout))
                addAll(metadata)
            }
        if (!connection.post { connection.start(stream, headers) }) {
            stream.status.complete(Status(StatusCode.UNAVAILABLE, "connection closed"))
            return call
        }
        if (timeout != null) {
            val watchdog =
                scope.launch {
                    delay(timeout)
                    connection.post { connection.cancel(stream, Status(StatusCode.DEADLINE_EXCEEDED, "deadline exceeded")) }
                }
            stream.status.invokeOnCompletion { watchdog.cancel() }
        }
        return call
    }

    /** One request, one response; a status other than OK is thrown as [GrpcException]. */
    suspend fun unary(
        path: String,
        request: ByteArray,
        metadata: List<Pair<String, String>> = emptyList(),
        timeout: Duration? = null,
    ): ByteArray {
        val call = call(path, metadata, timeout)
        call.send(request)
        call.closeSend()
        val response = call.receive()
        if (response != null && call.receive() != null) {
            call.cancel()
            throw GrpcException(StatusCode.INTERNAL, "a unary call got more than one response")
        }
        val status = call.status()
        if (!status.isOk) throw GrpcException(status)
        return response ?: throw GrpcException(StatusCode.INTERNAL, "a unary call got no response")
    }

    /** Ends the connection and every call still on it. */
    suspend fun close() {
        job.cancelAndJoin()
        selector.close()
    }

    companion object {
        /** Connects to [endpoint]; a failure is UNAVAILABLE, as for any call on a dead connection. */
        suspend fun connect(
            endpoint: Endpoint,
            limits: GrpcLimits = GrpcLimits(),
            authority: String = "localhost",
        ): GrpcClient {
            val selector = SelectorManager(Dispatchers.Default)
            val socket =
                try {
                    aSocket(selector).tcp().connect(endpoint.address())
                } catch (e: CancellationException) {
                    selector.close()
                    throw e
                } catch (e: Exception) {
                    selector.close()
                    throw GrpcException(StatusCode.UNAVAILABLE, "cannot connect to $endpoint: ${e.message}")
                }
            val job = SupervisorJob()
            val connection = ClientConnection(socket, limits, job)
            CoroutineScope(job + Dispatchers.Default).launch {
                try {
                    connection.run()
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    // The calls on it have already failed with UNAVAILABLE (onClosed).
                }
            }
            return GrpcClient(connection, limits, authority, job, selector)
        }
    }
}
