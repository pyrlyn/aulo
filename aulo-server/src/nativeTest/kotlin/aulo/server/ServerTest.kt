package aulo.server

import aulo.config.DaemonConfig
import aulo.grpc.Endpoint
import aulo.grpc.GrpcClient
import aulo.grpc.GrpcException
import aulo.grpc.ServerHandler
import aulo.grpc.StatusCode
import aulo.secrets.FileSecretStore
import aulo.store.Store
import aulo.store.databasePath
import fs.TempDir
import fs.createDir
import fs.mode
import fs.setMode
import fs.symlink
import io.grpc.health.v1.HealthCheckRequest
import io.grpc.health.v1.HealthCheckResponse
import io.grpc.health.v1.HealthCheckResponse.ServingStatus
import io.grpc.health.v1.HealthListRequest
import io.grpc.health.v1.HealthListResponse
import io.grpc.reflection.v1.ServerReflectionRequest
import io.grpc.reflection.v1.ServerReflectionResponse
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.toKString
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import kotlinx.io.files.Path
import kotlinx.io.files.SystemFileSystem
import okio.ByteString
import platform.posix.getenv
import process.Command
import process.ExitStatus
import process.runBounded
import kotlin.experimental.ExperimentalNativeApi
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.time.Duration.Companion.seconds

private const val CHECK = "/grpc.health.v1.Health/Check"
private const val REFLECT = "/grpc.reflection.v1.ServerReflection/ServerReflectionInfo"

@OptIn(ExperimentalNativeApi::class)
private val posix = Platform.osFamily != OsFamily.WINDOWS

private val echo = ServerHandler { call -> while (true) call.send(call.receive() ?: break) }

class ServerTest {
    @Test
    fun health_answers_over_the_local_socket_from_config() =
        serving { client, _ ->
            assertEquals(ServingStatus.SERVING, client.check(""))
            assertEquals(ServingStatus.SERVING, client.check(AuloServer.HEALTH))
            assertStatus(StatusCode.NOT_FOUND) { client.check("aulo.v1.Nope") }
            val list = HealthListResponse.ADAPTER.decode(client.unary("/grpc.health.v1.Health/List", HealthListRequest().encode()))
            assertEquals(setOf("", AuloServer.HEALTH, AuloServer.REFLECTION, "test.Echo"), list.statuses.keys)
        }

    @Test
    fun watch_reports_not_serving_and_ends_on_shutdown() =
        transport {
            TempDir.create("aulo-server").use { home ->
                val server = AuloServer(emptyMap())
                val endpoint = server.start(Listeners.from(DaemonConfig(), home.path.toString())).single()
                val client = GrpcClient.connect(endpoint)
                try {
                    val watch = client.call("/grpc.health.v1.Health/Watch")
                    watch.send(HealthCheckRequest(service = "").encode())
                    watch.closeSend()
                    assertEquals(ServingStatus.SERVING, HealthCheckResponse.ADAPTER.decode(watch.receive()!!).status)
                    server.shutdown(5.seconds)
                    assertEquals(ServingStatus.NOT_SERVING, HealthCheckResponse.ADAPTER.decode(watch.receive()!!).status)
                    assertNull(watch.receive())
                    assertTrue(watch.status().isOk)
                } finally {
                    client.close()
                }
            }
        }

    @Test
    fun reflection_lists_the_served_services_and_returns_files_with_their_imports() =
        serving { client, _ ->
            val call = client.call(REFLECT)
            val requests =
                listOf(
                    ServerReflectionRequest(list_services = ""),
                    ServerReflectionRequest(file_containing_symbol = "grpc.health.v1.Health.Check"),
                    ServerReflectionRequest(file_containing_symbol = "aulo.v1.ChatService"),
                    ServerReflectionRequest(file_by_filename = "nope.proto"),
                )
            requests.forEach { call.send(it.encode()) }
            call.closeSend()
            val responses = List(requests.size) { ServerReflectionResponse.ADAPTER.decode(call.receive()!!) }
            assertNull(call.receive())

            val services = responses[0].list_services_response!!.service.map { it.name }
            // Only what the server serves: the generated aulo.v1 services have no handlers here.
            assertEquals(listOf(AuloServer.HEALTH, AuloServer.REFLECTION, "test.Echo"), services)
            assertEquals(listOf("grpc/health/v1/health.proto"), responses[1].files())
            // Imports come along, so a client can resolve every type.
            val chat = responses[2].files()
            assertEquals("aulo/v1/chat.proto", chat.first())
            assertTrue(chat.containsAll(listOf("aulo/v1/common.proto", "google/protobuf/timestamp.proto")), chat.toString())
            assertEquals(StatusCode.NOT_FOUND.value, responses[3].error_response!!.error_code)
            assertEquals(requests[3], responses[3].original_request)
        }

    @Test
    fun a_request_that_is_not_a_message_is_internal() =
        serving { client, _ ->
            assertStatus(StatusCode.INTERNAL, "the request is not a valid message") {
                client.unary(CHECK, byteArrayOf(0xFF.toByte()))
            }
        }

    @Test
    fun a_unary_call_with_two_requests_is_internal() =
        serving { client, _ ->
            val call = client.call(CHECK)
            repeat(2) { call.send(HealthCheckRequest().encode()) }
            call.closeSend()
            assertNull(call.receive())
            assertEquals(StatusCode.INTERNAL, call.status().code)
        }

    @Test
    fun built_in_methods_cannot_be_replaced() {
        assertFailsWith<IllegalArgumentException> { AuloServer(mapOf(CHECK to echo)) }
    }

    @Test
    fun a_tcp_listener_is_refused_until_tls_is_supported() =
        transport {
            TempDir.create("aulo-server").use { home ->
                val config = DaemonConfig(listen = "127.0.0.1:7878", tlsCert = "c.pem", tlsKey = "k.pem")
                val server = AuloServer(emptyMap())
                assertFailsWith<ListenerConfigException> { server.start(Listeners.from(config, home.path.toString())) }
                server.shutdown(1.seconds)
            }
        }
}

class LocalSocketTest {
    @Test
    fun the_socket_directory_is_0700_and_the_socket_0600() =
        serving { client, socket ->
            assertEquals(ServingStatus.SERVING, client.check(""))
            if (!posix) return@serving
            assertEquals(0b111_000_000, mode(Path(socket.path).parent!!)!! and 0b111_111_111)
            assertEquals(0b110_000_000, mode(Path(socket.path))!! and 0b111_111_111)
        }

    @Test
    fun a_loose_or_linked_socket_directory_is_refused() =
        transport {
            if (!posix) return@transport
            TempDir.create("aulo-server").use { home ->
                val run = Path(home.path, Listeners.RUN_DIR)
                createDir(run)
                setMode(run, 0b111_101_101)
                val server = AuloServer(emptyMap())
                val loose = assertFailsWith<ListenerConfigException> { server.start(Listeners.from(DaemonConfig(), home.path.toString())) }
                assertContains(loose.message!!, "chmod 700")
                SystemFileSystem.delete(run)
                createDir(Path(home.path, "elsewhere"))
                setMode(Path(home.path, "elsewhere"), 0b111_000_000)
                symlink(Path(home.path, "elsewhere"), run)
                assertFailsWith<ListenerConfigException> { server.start(Listeners.from(DaemonConfig(), home.path.toString())) }
                server.shutdown(1.seconds)
            }
        }
}

class AuthTest {
    @Test
    fun tcp_needs_a_known_bearer_token_and_the_local_socket_does_not() =
        transport {
            TempDir.create("aulo-server").use { home ->
                val token = BearerTokens.generate()
                val server = AuloServer(mapOf("/test.Echo/Echo" to echo), tokenHashes = listOf(BearerTokens.hash(token)))
                val tcp = server.bind(Endpoint.Tcp("127.0.0.1", 0))
                val local = server.start(Listeners.from(DaemonConfig(), home.path.toString())).single()
                val remote = GrpcClient.connect(tcp)
                val owner = GrpcClient.connect(local)
                try {
                    // The owner on the local socket needs no token.
                    assertEquals(ServingStatus.SERVING, owner.check(""))

                    val refused =
                        listOf(
                            emptyList(),
                            listOf("authorization" to "Bearer ${BearerTokens.generate()}"),
                            listOf("authorization" to token),
                            listOf("authorization" to "Basic $token"),
                            listOf("authorization" to "Bearer "),
                            listOf("authorization" to "Bearer $token x"),
                            listOf("authorization" to "Bearer $token", "authorization" to "Bearer $token"),
                            listOf("authorization" to "Bearer $token" + "x".repeat(BearerTokens.MAX_HEADER_CHARS)),
                        )
                    for ((case, metadata) in refused.withIndex()) {
                        val e = assertFailsWith<GrpcException>("case $case") { remote.check("", metadata) }
                        assertEquals(StatusCode.UNAUTHENTICATED, e.status.code, "case $case")
                        assertEquals("a valid bearer token is required", e.status.message)
                    }
                    // Built-in services are guarded too, and so is a stream before its first message.
                    assertStatus(StatusCode.UNAUTHENTICATED) {
                        remote.unary("/test.Echo/Echo", byteArrayOf(1))
                    }

                    assertEquals(ServingStatus.SERVING, remote.check("", listOf("authorization" to "Bearer $token")))
                    assertEquals(ServingStatus.SERVING, remote.check("", listOf("authorization" to "bearer $token")))
                    val echoed = remote.unary("/test.Echo/Echo", byteArrayOf(7), listOf("authorization" to "Bearer $token"))
                    assertEquals(listOf<Byte>(7), echoed.toList())
                } finally {
                    remote.close()
                    owner.close()
                    server.shutdown(1.seconds)
                }
            }
        }

    @Test
    fun without_tokens_tcp_refuses_every_call() =
        transport {
            val server = AuloServer(emptyMap())
            val client = GrpcClient.connect(server.bind(Endpoint.Tcp("127.0.0.1", 0)))
            try {
                assertStatus(StatusCode.UNAUTHENTICATED) {
                    client.check("", listOf("authorization" to "Bearer ${BearerTokens.generate()}"))
                }
            } finally {
                client.close()
                server.shutdown(1.seconds)
            }
        }

    @Test
    fun tokens_are_random_prefixed_and_url_safe() {
        val tokens = List(32) { BearerTokens.generate() }
        assertEquals(tokens.size, tokens.toSet().size)
        for (token in tokens) {
            assertTrue(token.startsWith(BearerTokens.PREFIX), token)
            assertEquals(BearerTokens.PREFIX.length + 43, token.length, token)
            assertTrue(token.all { it.isLetterOrDigit() || it == '-' || it == '_' }, token)
        }
        assertEquals(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            BearerTokens.hash("abc"),
        )
    }

    @Test
    fun the_first_token_is_made_once_and_only_its_hash_is_stored() =
        TempDir.create("aulo-server").use { home ->
            Store.open(databasePath(home.path).toString()).use { store ->
                val secrets = FileSecretStore(Path(home.path, "secrets").toString())
                val first = ensureFirstToken(store, secrets)
                val token = secrets.load(FIRST_TOKEN)!!
                assertEquals(listOf(BearerTokens.hash(token)), first)
                assertEquals(first, ensureFirstToken(store, secrets))
                assertEquals(token, secrets.load(FIRST_TOKEN))
                val stored = store.listApiTokens().single()
                assertEquals(ADMIN_SCOPE, stored.scope)
                assertTrue(stored.toString().contains(token).not(), "the store never holds the token")
            }
        }
}

class GrpcurlTest {
    /**
     * The check a person runs by hand, kept runnable: only with AULO_GRPCURL naming a grpcurl
     * binary, since grpcurl is no part of the build (for example
     * `AULO_GRPCURL=$(mise x aqua:fullstorydev/grpcurl@1.9.4 -- which grpcurl)`).
     */
    @OptIn(ExperimentalForeignApi::class)
    @Test
    fun grpcurl_lists_the_services_and_calls_health() {
        val grpcurl = getenv("AULO_GRPCURL")?.toKString()?.takeIf { it.isNotEmpty() } ?: return
        serving { _, socket ->
            suspend fun run(
                flags: List<String>,
                vararg rest: String,
            ): String {
                val command = Command(grpcurl, listOf("-plaintext", "-unix") + flags + socket.path + rest)
                val out = withContext(Dispatchers.Default) { command.runBounded(30.seconds, maxOutput = 1L shl 20) }
                assertEquals(ExitStatus.Exited(0), out.status, out.stderr.decodeToString())
                return out.stdout.decodeToString()
            }
            assertEquals(
                listOf(AuloServer.HEALTH, AuloServer.REFLECTION, "test.Echo"),
                run(emptyList(), "list").lines().filter { it.isNotBlank() },
            )
            assertContains(run(listOf("-d", "{\"service\": \"\"}"), AuloServer.HEALTH + "/Check"), "\"SERVING\"")
            assertContains(run(emptyList(), "describe", "aulo.v1.ChatService"), "rpc ")
        }
    }
}

class ListenersTest {
    @Test
    fun the_default_is_the_local_socket_in_the_home_directory() {
        assertEquals(Listeners(Endpoint.Unix("/home/me/.aulo/run/aulod.sock")), Listeners.from(DaemonConfig(), "/home/me/.aulo/"))
        assertEquals(Endpoint.Unix("C:\\Users\\me\\.aulo\\run\\aulod.sock"), Listeners.from(DaemonConfig(), "C:\\Users\\me\\.aulo").local)
    }

    @Test
    fun a_socket_path_over_the_platform_limit_is_refused() {
        val home = "/" + "d".repeat(Listeners.MAX_SOCKET_PATH_BYTES - "/run/aulod.sock".length - 1)
        assertEquals(
            Listeners.MAX_SOCKET_PATH_BYTES,
            Listeners
                .from(DaemonConfig(), home)
                .local.path.length,
        )
        assertFailsWith<ListenerConfigException> { Listeners.from(DaemonConfig(), home + "d") }
    }

    @Test
    fun listen_needs_both_tls_files() {
        val e = assertFailsWith<ListenerConfigException> { Listeners.from(DaemonConfig(listen = "127.0.0.1:7878"), "/h") }
        assertContains(e.message!!, "tls_cert")
        assertFailsWith<ListenerConfigException> { Listeners.from(DaemonConfig(listen = "127.0.0.1:7878", tlsCert = "c"), "/h") }
        val remote = Listeners.from(DaemonConfig(listen = "[::1]:7878", tlsCert = "c", tlsKey = "k"), "/h").remote
        assertEquals(RemoteListener(Endpoint.Tcp("::1", 7878), "c", "k"), remote)
    }

    @Test
    fun listen_addresses_parse_strictly() {
        // IPv6 from the documentation range (RFC 3849), which address scanners know is not a real host.
        assertEquals(Endpoint.Tcp("0.0.0.0", 7443), Listeners.parseListen("0.0.0.0:7443"))
        assertEquals(Endpoint.Tcp("aulo.example", 1), Listeners.parseListen("aulo.example:1"))
        assertEquals(Endpoint.Tcp("2001:db8::1", 65535), Listeners.parseListen("[2001:db8::1]:65535"))
        for (bad in listOf(
            "",
            "7443",
            ":7443",
            "host:",
            "host:0",
            "host:65536",
            "host:123456",
            "2001:db8::1:80",
            "[::1]80",
            "[]:80",
            "a b:80",
            "host:+80",
        )) {
            assertFailsWith<ListenerConfigException>(bad) { Listeners.parseListen(bad) }
        }
    }
}

private fun transport(block: suspend CoroutineScope.() -> Unit) = runBlocking { withTimeout(60.seconds) { block() } }

/** An [AuloServer] with one extra service, on the local socket from the default config. */
private fun serving(block: suspend (GrpcClient, Endpoint.Unix) -> Unit) =
    transport {
        TempDir.create("aulo-server").use { home ->
            val server = AuloServer(mapOf("/test.Echo/Echo" to echo))
            val endpoint = server.start(Listeners.from(DaemonConfig(), home.path.toString())).single()
            val client = GrpcClient.connect(endpoint)
            try {
                block(client, endpoint as Endpoint.Unix)
            } finally {
                client.close()
                server.shutdown(1.seconds)
            }
        }
    }

private suspend fun GrpcClient.check(
    service: String,
    metadata: List<Pair<String, String>> = emptyList(),
): ServingStatus = HealthCheckResponse.ADAPTER.decode(unary(CHECK, HealthCheckRequest(service = service).encode(), metadata)).status

/** The file names in a FileDescriptorResponse: field 1 of each FileDescriptorProto. */
private fun ServerReflectionResponse.files(): List<String> = file_descriptor_response!!.file_descriptor_proto.map { it.fileName() }

private fun ByteString.fileName(): String {
    // aulo's names are short, so the length is one varint byte.
    assertEquals(0x0A, this[0].toInt(), "a FileDescriptorProto starts with its name")
    return substring(2, 2 + this[1].toInt()).utf8()
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
