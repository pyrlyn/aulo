package aulo.grpc

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeout
import kotlinx.io.files.Path
import kotlinx.io.files.SystemFileSystem
import kotlin.random.Random
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

private const val ECHO = "/test.Echo/Echo"

/** Sends every request back. */
private val echo =
    ServerHandler { call ->
        while (true) call.send(call.receive() ?: break)
    }

class TransportTest {
    @Test
    fun unary_call_over_a_unix_socket() =
        transport {
            serving(mapOf(ECHO to echo), endpoint = unixSocket()) { endpoint ->
                connected(endpoint) { client ->
                    assertEquals("ping", client.unary(ECHO, "ping".encodeToByteArray()).decodeToString())
                }
            }
        }

    @Test
    fun unary_call_over_tcp() =
        transport {
            serving(mapOf(ECHO to echo)) { endpoint ->
                assertTrue((endpoint as Endpoint.Tcp).port > 0)
                connected(endpoint) { client ->
                    assertEquals("ping", client.unary(ECHO, "ping".encodeToByteArray()).decodeToString())
                }
            }
        }

    @Test
    fun streaming_both_ways_moves_more_than_the_flow_control_windows() =
        transport {
            for (endpoint in listOf(unixSocket(), tcp())) {
                serving(mapOf(ECHO to echo), endpoint = endpoint) { bound ->
                    connected(bound) { client ->
                        // 4 MiB each way: well past the 1 MiB stream window, so window updates must flow.
                        val messages = List(256) { i -> ByteArray(16 * 1024) { (i + it).toByte() } }
                        val call = client.call(ECHO)
                        val sender =
                            async {
                                messages.forEach { call.send(it) }
                                call.closeSend()
                            }
                        val received = mutableListOf<ByteArray>()
                        while (true) received += call.receive() ?: break
                        sender.await()
                        assertEquals(Status.OK, call.status())
                        assertEquals(messages.size, received.size)
                        messages.zip(received).forEach { (sent, back) -> assertContentEquals(sent, back) }
                    }
                }
            }
        }

    @Test
    fun metadata_reaches_the_handler_and_statuses_reach_the_client() =
        transport {
            val handlers =
                mapOf(
                    "/test.S/Meta" to
                        ServerHandler { call -> call.send(call.metadata.joinToString { "${it.first}=${it.second}" }.encodeToByteArray()) },
                    "/test.S/Fail" to ServerHandler { throw GrpcException(StatusCode.NOT_FOUND, "nö 100%") },
                    "/test.S/Crash" to ServerHandler { error("a secret detail") },
                )
            serving(handlers) { endpoint ->
                connected(endpoint) { client ->
                    val meta = client.unary("/test.S/Meta", ByteArray(0), metadata = listOf("x-trace" to "abc")).decodeToString()
                    assertTrue("x-trace=abc" in meta, meta)
                    assertStatus(StatusCode.NOT_FOUND, "nö 100%") { client.unary("/test.S/Fail", ByteArray(0)) }
                    // An unexpected exception is UNKNOWN with no message: internals stay on the server.
                    assertStatus(StatusCode.UNKNOWN, "") { client.unary("/test.S/Crash", ByteArray(0)) }
                    assertStatus(StatusCode.UNIMPLEMENTED) { client.unary("/test.S/Missing", ByteArray(0)) }
                }
            }
        }

    @Test
    fun a_request_over_the_message_cap_is_resource_exhausted() =
        transport {
            serving(mapOf(ECHO to echo), GrpcLimits(maxMessageBytes = 1024)) { endpoint ->
                connected(endpoint) { client ->
                    assertEquals(1024, client.unary(ECHO, ByteArray(1024)).size)
                    assertStatus(
                        StatusCode.RESOURCE_EXHAUSTED,
                        "message of 1025 bytes exceeds the limit of 1024",
                    ) { client.unary(ECHO, ByteArray(1025)) }
                    // The connection survives a capped call.
                    assertEquals(1, client.unary(ECHO, ByteArray(1)).size)
                }
            }
        }

    @Test
    fun a_response_over_the_client_cap_is_resource_exhausted() =
        transport {
            serving(mapOf(ECHO to echo)) { endpoint ->
                connected(endpoint, GrpcLimits(maxMessageBytes = 1024)) { client ->
                    assertStatus(
                        StatusCode.RESOURCE_EXHAUSTED,
                        "message of 2048 bytes exceeds the limit of 1024",
                    ) { client.unary(ECHO, ByteArray(2048)) }
                }
            }
        }

    @Test
    fun request_metadata_over_the_header_cap_is_resource_exhausted() =
        transport {
            serving(mapOf(ECHO to echo), GrpcLimits(maxHeaderListBytes = 1024)) { endpoint ->
                connected(endpoint) { client ->
                    assertEquals(1, client.unary(ECHO, ByteArray(1)).size)
                    val big = listOf("x-big" to "a".repeat(4096))
                    assertStatus(
                        StatusCode.RESOURCE_EXHAUSTED,
                        "request metadata exceeds 1024 bytes",
                    ) { client.unary(ECHO, ByteArray(1), metadata = big) }
                    assertEquals(1, client.unary(ECHO, ByteArray(1)).size)
                }
            }
        }

    @Test
    fun no_more_calls_run_at_once_than_the_stream_cap() =
        transport {
            val lock = Mutex()
            var running = 0
            var peak = 0
            val gate = CompletableDeferred<Unit>()
            val slow =
                ServerHandler { call ->
                    lock.withLock { peak = maxOf(peak, ++running) }
                    try {
                        gate.await()
                        call.send(call.receive() ?: ByteArray(0))
                    } finally {
                        lock.withLock { running-- }
                    }
                }
            serving(mapOf(ECHO to echo, "/test.S/Slow" to slow), GrpcLimits(maxConcurrentStreams = 2)) { endpoint ->
                connected(endpoint) { client ->
                    // One call first, so the client has the server's SETTINGS before the burst.
                    client.unary(ECHO, ByteArray(1))
                    val calls = List(5) { async { client.unary("/test.S/Slow", byteArrayOf(it.toByte())) } }
                    while (lock.withLock { running } < 2) delay(10.milliseconds)
                    delay(200.milliseconds)
                    assertEquals(2, lock.withLock { running })
                    gate.complete(Unit)
                    assertEquals((0 until 5).toList(), calls.awaitAll().map { it.single().toInt() })
                    assertEquals(2, peak)
                }
            }
        }

    @Test
    fun a_connection_over_the_cap_is_closed() =
        transport {
            serving(mapOf(ECHO to echo), GrpcLimits(maxConnections = 1)) { endpoint ->
                val first = GrpcClient.connect(endpoint)
                assertEquals(1, first.unary(ECHO, ByteArray(1)).size)
                connected(endpoint) { second -> assertStatus(StatusCode.UNAVAILABLE) { second.unary(ECHO, ByteArray(1)) } }
                first.close()
                // The slot frees once the server sees the first connection end.
                var served = false
                repeat(50) {
                    if (!served) {
                        served = runCatching { connected(endpoint) { it.unary(ECHO, ByteArray(1)) } }.isSuccess
                        if (!served) delay(50.milliseconds)
                    }
                }
                assertTrue(served)
            }
        }

    @Test
    fun a_passed_deadline_ends_the_call_and_cancels_the_handler() =
        transport {
            val cancelled = CompletableDeferred<Unit>()
            val stuck =
                ServerHandler {
                    try {
                        awaitCancellation()
                    } finally {
                        cancelled.complete(Unit)
                    }
                }
            serving(mapOf("/test.S/Stuck" to stuck)) { endpoint ->
                connected(endpoint) { client ->
                    assertStatus(StatusCode.DEADLINE_EXCEEDED) { client.unary("/test.S/Stuck", ByteArray(0), timeout = 200.milliseconds) }
                    cancelled.await()
                }
            }
        }

    @Test
    fun a_cancelled_call_cancels_the_handler() =
        transport {
            val started = CompletableDeferred<Unit>()
            val cancelled = CompletableDeferred<Unit>()
            val stuck =
                ServerHandler {
                    started.complete(Unit)
                    try {
                        awaitCancellation()
                    } finally {
                        cancelled.complete(Unit)
                    }
                }
            serving(mapOf("/test.S/Stuck" to stuck)) { endpoint ->
                connected(endpoint) { client ->
                    val call = client.call("/test.S/Stuck")
                    call.send(ByteArray(0))
                    started.await()
                    call.cancel()
                    assertEquals(StatusCode.CANCELLED, call.status().code)
                    cancelled.await()
                }
            }
        }

    @Test
    fun shutdown_lets_open_calls_finish_and_refuses_new_ones() =
        transport {
            val path = unixSocket()
            val started = CompletableDeferred<Unit>()
            val gate = CompletableDeferred<Unit>()
            val slow =
                ServerHandler { call ->
                    started.complete(Unit)
                    gate.await()
                    call.send("done".encodeToByteArray())
                }
            val server = GrpcServer(mapOf("/test.S/Slow" to slow))
            val endpoint = server.bind(path)
            val client = GrpcClient.connect(endpoint)
            try {
                val inFlight = async { client.unary("/test.S/Slow", ByteArray(0)) }
                started.await()
                val shutdown = async { server.shutdown(10.seconds) }
                delay(100.milliseconds)
                assertFalse(SystemFileSystem.exists(Path(path.path)), "the socket file outlives shutdown")
                assertStatus(StatusCode.UNAVAILABLE) { client.unary("/test.S/Slow", ByteArray(0)) }
                gate.complete(Unit)
                assertEquals("done", inFlight.await().decodeToString())
                shutdown.await()
            } finally {
                client.close()
            }
            assertFailsWith<GrpcException> { GrpcClient.connect(endpoint) }
        }
}

private fun transport(block: suspend CoroutineScope.() -> Unit) = runBlocking { withTimeout(60.seconds) { block() } }

private fun tcp() = Endpoint.Tcp("127.0.0.1", 0)

// macOS caps a socket path at 104 bytes, so not under the long per-user temporary directory.
private fun unixSocket() = Endpoint.Unix("/tmp/aulo-grpc-${Random.nextLong().toULong().toString(16)}.sock")

private suspend fun <T> serving(
    handlers: Map<String, ServerHandler>,
    limits: GrpcLimits = GrpcLimits(),
    endpoint: Endpoint = tcp(),
    block: suspend (Endpoint) -> T,
): T {
    val server = GrpcServer(handlers, limits)
    try {
        return block(server.bind(endpoint))
    } finally {
        server.shutdown(1.seconds)
    }
}

private suspend fun <T> connected(
    endpoint: Endpoint,
    limits: GrpcLimits = GrpcLimits(),
    block: suspend (GrpcClient) -> T,
): T {
    val client = GrpcClient.connect(endpoint, limits)
    try {
        return block(client)
    } finally {
        client.close()
    }
}

private inline fun assertStatus(
    code: StatusCode,
    message: String? = null,
    block: () -> Unit,
) {
    val e = assertFailsWith<GrpcException> { block() }
    assertEquals(code, e.status.code, e.status.message)
    if (message != null) assertEquals(message, e.status.message)
}
