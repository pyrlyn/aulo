package aulo.grpc

import io.ktor.network.selector.SelectorManager
import io.ktor.network.sockets.aSocket
import io.ktor.network.sockets.openReadChannel
import io.ktor.utils.io.readAvailable
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.time.Clock
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

private const val ECHO = "/test.Echo/Echo"

private val echo =
    ServerHandler { call ->
        while (true) call.send(call.receive() ?: break)
    }

class TlsTest {
    private val server = Tls.selfSigned("aulo-test", Clock.System.now())

    @Test
    fun a_client_with_the_right_pin_streams_both_ways() =
        tls {
            serving(server) { endpoint ->
                val client = GrpcClient.connect(endpoint, tls = TlsClient(server.fingerprint))
                try {
                    assertEquals("ping", client.unary(ECHO, "ping".encodeToByteArray()).decodeToString())
                    // 4 MiB each way: hundreds of records, and reads and writes on both sides at once.
                    val messages = List(256) { i -> ByteArray(16 * 1024) { (i * 7 + it).toByte() } }
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
                    messages.zip(received).forEach { (sent, back) -> assertContentEquals(sent, back) }
                    assertEquals(messages.size, received.size)
                } finally {
                    client.close()
                }
            }
        }

    @Test
    fun a_client_with_a_wrong_pin_is_refused_before_any_call() =
        tls {
            var calls = 0
            val counting =
                ServerHandler { call ->
                    calls++
                    echo.handle(call)
                }
            serving(server, mapOf(ECHO to counting)) { endpoint ->
                val other = Tls.selfSigned("aulo-test", Clock.System.now())
                val e = assertFailsWith<GrpcException> { GrpcClient.connect(endpoint, tls = TlsClient(other.fingerprint)) }
                assertEquals(StatusCode.UNAVAILABLE, e.status.code)
                assertContains(e.status.message, "the server's certificate does not match the pin")
                assertEquals(0, calls)
                // The refusal ends only that connection.
                val client = GrpcClient.connect(endpoint, tls = TlsClient(server.fingerprint))
                try {
                    assertEquals(1, client.unary(ECHO, ByteArray(1)).size)
                } finally {
                    client.close()
                }
            }
        }

    @Test
    fun a_plain_client_on_a_tls_port_and_tls_on_a_plain_port_both_fail() =
        tls {
            serving(server) { endpoint ->
                val plain = GrpcClient.connect(endpoint)
                try {
                    val e = assertFailsWith<GrpcException> { plain.unary(ECHO, ByteArray(1)) }
                    assertEquals(StatusCode.UNAVAILABLE, e.status.code)
                } finally {
                    plain.close()
                }
            }
            serving(null) { endpoint ->
                val e = assertFailsWith<GrpcException> { GrpcClient.connect(endpoint, tls = TlsClient(server.fingerprint)) }
                assertEquals(StatusCode.UNAVAILABLE, e.status.code)
            }
        }

    @Test
    fun a_silent_peer_loses_its_slot_at_the_handshake_deadline() =
        tls {
            val limits = GrpcLimits(maxConnections = 1, handshakeTimeout = 300.milliseconds)
            serving(server, limits = limits) { endpoint ->
                endpoint as Endpoint.Tcp
                val selector = SelectorManager(Dispatchers.Default)
                try {
                    // Connects and never says a word: the slot is taken until the deadline.
                    val silent = aSocket(selector).tcp().connect(endpoint.host, endpoint.port)
                    val closed = async { silent.openReadChannel().readAvailable(ByteArray(1), 0, 1) }
                    assertEquals(-1, withTimeout(5.seconds) { closed.await() }, "the server closes a silent peer")
                    silent.close()
                    val client = GrpcClient.connect(endpoint, limits, tls = TlsClient(server.fingerprint))
                    try {
                        assertEquals(1, client.unary(ECHO, ByteArray(1)).size)
                    } finally {
                        client.close()
                    }
                } finally {
                    selector.close()
                }
            }
        }

    @Test
    fun a_client_handshake_has_a_deadline_too() =
        tls {
            val selector = SelectorManager(Dispatchers.Default)
            // A listener that accepts and never answers.
            val listener = aSocket(selector).tcp().bind("127.0.0.1", 0)
            try {
                val port = (listener.localAddress as io.ktor.network.sockets.InetSocketAddress).port
                val held = async { listener.accept() }
                val e =
                    assertFailsWith<GrpcException> {
                        GrpcClient.connect(
                            Endpoint.Tcp("127.0.0.1", port),
                            GrpcLimits(handshakeTimeout = 300.milliseconds),
                            tls = TlsClient(server.fingerprint),
                        )
                    }
                assertEquals(StatusCode.UNAVAILABLE, e.status.code)
                assertContains(e.status.message, "timed out")
                held.await().close()
            } finally {
                listener.close()
                selector.close()
            }
        }

    @Test
    fun certificates_load_only_with_their_own_key() {
        val other = Tls.selfSigned("other", Clock.System.now())
        assertContentEquals(server.fingerprint, TlsServer(server.certPem, server.keyPem).fingerprint)
        assertFalse(server.fingerprint.contentEquals(other.fingerprint))
        assertFailsWith<TlsException> { TlsServer(server.certPem, other.keyPem) }
        assertFailsWith<TlsException> { TlsServer("not a certificate".encodeToByteArray(), server.keyPem) }
        assertFailsWith<TlsException> { TlsServer(server.certPem, ByteArray(0)) }
        assertFailsWith<IllegalArgumentException> { Tls.selfSigned("CN=x,O=y", Clock.System.now()) }
        assertFailsWith<IllegalArgumentException> { TlsClient(ByteArray(31)) }
    }

    @Test
    fun pins_print_like_openssl_and_parse_leniently() {
        val pin = ByteArray(Tls.PIN_BYTES) { (it * 9).toByte() }
        val text = Tls.formatPin(pin)
        assertEquals(Tls.PIN_BYTES * 3 - 1, text.length)
        assertEquals("00:09:12:1B", text.take(11))
        assertContentEquals(pin, Tls.parsePin(text))
        assertContentEquals(pin, Tls.parsePin(text.replace(":", "").lowercase()))
        for (bad in listOf("", "00", text + ":00", text.dropLast(1), text.replace('B', 'G'))) assertNull(Tls.parsePin(bad), bad)
    }
}

private fun tls(block: suspend CoroutineScope.() -> Unit) = runBlocking { withTimeout(60.seconds) { block() } }

private suspend fun <T> serving(
    tls: TlsServer?,
    handlers: Map<String, ServerHandler> = mapOf(ECHO to echo),
    limits: GrpcLimits = GrpcLimits(),
    block: suspend (Endpoint) -> T,
): T {
    val server = GrpcServer(handlers, limits)
    try {
        return block(server.bind(Endpoint.Tcp("127.0.0.1", 0), tls))
    } finally {
        server.shutdown(1.seconds)
    }
}
