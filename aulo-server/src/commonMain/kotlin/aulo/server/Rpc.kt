package aulo.server

import aulo.grpc.GrpcException
import aulo.grpc.ServerCall
import aulo.grpc.ServerHandler
import aulo.grpc.StatusCode
import com.squareup.wire.ProtoAdapter
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ReceiveChannel
import kotlinx.coroutines.channels.SendChannel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch

// The four call shapes the generated method tables (build-logic GrpcTables) bind Wire's service
// interfaces with. Wire's interfaces take channels; these adapt them to a transport ServerCall.

internal fun <I : Any, O : Any> unary(
    request: ProtoAdapter<I>,
    response: ProtoAdapter<O>,
    method: suspend (I) -> O,
) = ServerHandler { call -> call.send(response.encode(method(call.single(request)))) }

internal fun <I : Any, O : Any> serverStreaming(
    request: ProtoAdapter<I>,
    response: ProtoAdapter<O>,
    method: suspend (I, SendChannel<O>) -> Unit,
) = ServerHandler { call ->
    val message = call.single(request)
    call.respond(response) { method(message, it) }
}

internal fun <I : Any, O : Any> clientStreaming(
    request: ProtoAdapter<I>,
    response: ProtoAdapter<O>,
    method: suspend (ReceiveChannel<I>) -> O,
) = ServerHandler { call ->
    coroutineScope {
        val requests = call.requests(this, request)
        val result = method(requests)
        requests.cancel()
        call.send(response.encode(result))
    }
}

internal fun <I : Any, O : Any> bidiStreaming(
    request: ProtoAdapter<I>,
    response: ProtoAdapter<O>,
    method: suspend (ReceiveChannel<I>, SendChannel<O>) -> Unit,
) = ServerHandler { call ->
    coroutineScope {
        val requests = call.requests(this, request)
        call.respond(response) { method(requests, it) }
        requests.cancel()
    }
}

/** The one request of a unary or server-streaming call; waits for the client to finish sending. */
private suspend fun <I : Any> ServerCall.single(adapter: ProtoAdapter<I>): I {
    val first = receive() ?: throw GrpcException(StatusCode.INTERNAL, "the call sent no request")
    if (receive() != null) throw GrpcException(StatusCode.INTERNAL, "the call sent more than one request")
    return adapter.decodeRequest(first)
}

/**
 * The call's requests, decoded. Capacity 1: the client's flow control, not a queue here, holds
 * requests the method has not read. Cancelling the channel stops the reader, which may be
 * waiting on a client that never finishes sending.
 */
private fun <I : Any> ServerCall.requests(
    scope: CoroutineScope,
    adapter: ProtoAdapter<I>,
): Channel<I> {
    val requests = Channel<I>(capacity = 1)
    val reader =
        scope.launch {
            try {
                while (true) requests.send(adapter.decodeRequest(receive() ?: break))
                requests.close()
            } catch (e: GrpcException) {
                // The method sees the bad request as the end of its channel; the call fails with it.
                requests.close(e)
                throw e
            }
        }
    requests.invokeOnClose { reader.cancel() }
    return requests
}

/**
 * Runs [method] with a channel whose messages go to the client. The method may close the channel
 * itself; returning closes it too, and a close with a cause fails the call with that cause.
 */
private suspend fun <O : Any> ServerCall.respond(
    adapter: ProtoAdapter<O>,
    method: suspend (SendChannel<O>) -> Unit,
) = coroutineScope {
    val responses = Channel<O>(capacity = 1)
    val writer = launch { for (message in responses) send(adapter.encode(message)) }
    method(responses)
    responses.close()
    writer.join()
}

// The bytes come from the client: a message Wire cannot read is the client's error, not ours.
private fun <I : Any> ProtoAdapter<I>.decodeRequest(bytes: ByteArray): I =
    try {
        decode(bytes)
    } catch (_: Exception) {
        throw GrpcException(StatusCode.INTERNAL, "the request is not a valid message")
    }
