package aulo.grpc

import kotlin.time.Duration

/** Serves one method: reads the call's requests, sends its responses, returns or throws. */
fun interface ServerHandler {
    suspend fun handle(call: ServerCall)
}

/**
 * One call on the server side. A handler that returns ends the call with OK; one that throws
 * [GrpcException] ends it with that status; any other exception ends it with UNKNOWN and no
 * message, so internals never reach the client.
 */
interface ServerCall {
    /** The method, `/package.Service/Method`. */
    val path: String

    /** The client's custom metadata: untrusted, already capped by [GrpcLimits.maxHeaderListBytes]. */
    val metadata: List<Pair<String, String>>

    /** The client's deadline, if it sent one; the handler is cancelled when it passes. */
    val timeout: Duration?

    /** The next request, or null once the client has sent them all. */
    suspend fun receive(): ByteArray?

    /** Sends one response; waits while the client is slow to read. */
    suspend fun send(message: ByteArray)
}
