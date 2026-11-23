package aulo.server

import aulo.config.Config
import aulo.config.DaemonConfig
import aulo.config.ProvidersConfig
import aulo.config.TierConfig
import aulo.config.TiersConfig
import aulo.grpc.Endpoint
import aulo.grpc.GrpcClient
import aulo.grpc.GrpcServer
import aulo.grpc.Tls
import aulo.grpc.TlsClient
import aulo.secrets.FileSecretStore
import aulo.v1.CreateChatRequest
import aulo.v1.CreateChatResponse
import aulo.v1.ModelRef
import fs.FileLock
import fs.TempDir
import fs.WriteOptions
import io.grpc.health.v1.HealthCheckResponse.ServingStatus
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.launch
import kotlinx.io.files.Path
import kotlinx.io.files.SystemFileSystem
import platform.posix.getpid
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds

class DaemonTest {
    @Test
    fun the_daemon_serves_chats_holds_the_lock_and_cleans_up_on_stop() =
        transport {
            TempDir.create("aulo-daemon").use { tmp ->
                val home = tmp.path.toString()
                val config = Config(providers = ProvidersConfig(tiers = TiersConfig(main = TierConfig("anthropic", "claude-sonnet-5-5"))))
                val secrets = FileSecretStore(Path(home, "secrets").toString())
                val listening = CompletableDeferred<Unit>()
                val stop = CompletableDeferred<Unit>()
                val lines = mutableListOf<String>()
                val log = { line: String ->
                    lines += line
                    if (line.startsWith("aulod listening")) listening.complete(Unit)
                }
                val daemon = launch { runDaemon(home, config, secrets, log, { stop.await() }, grace = 1.seconds) }
                listening.await()

                val socket = Listeners.from(DaemonConfig(), home).local
                assertEquals("aulod listening on unix:${socket.path}", lines.single())
                assertEquals(getpid().toLong(), RunFiles.readPid(home))
                assertNotNull(secrets.load(FIRST_TOKEN), "the first run makes the admin token")

                val client = GrpcClient.connect(socket)
                try {
                    val chat =
                        CreateChatResponse.ADAPTER
                            .decode(client.unary("/aulo.v1.ChatService/CreateChat", CreateChatRequest().encode()))
                            .chat!!
                    assertEquals(ModelRef("anthropic", "claude-sonnet-5-5"), chat.model)
                } finally {
                    client.close()
                }

                val second = assertFailsWith<AlreadyRunningException> { runDaemon(home, config, secrets, {}, {}) }
                assertEquals("aulod is already running for $home (pid ${getpid()})", second.message)

                stop.complete(Unit)
                daemon.join()
                assertEquals("aulod shutting down", lines.last())
                assertFalse(SystemFileSystem.exists(Path(RunFiles.dir(home), RunFiles.PID)), "the pidfile is removed")
                FileLock.tryExclusive(Path(RunFiles.dir(home), RunFiles.LOCK).toString()).let { lock ->
                    assertNotNull(lock, "the lock is released").close()
                }
            }
        }

    @Test
    fun a_tcp_listener_serves_the_logged_self_signed_pin_and_wants_the_first_token() =
        transport {
            TempDir.create("aulo-daemon").use { tmp ->
                val home = tmp.path.toString()
                val port = freePort()
                val config = Config(daemon = DaemonConfig(listen = "127.0.0.1:$port"))
                val secrets = FileSecretStore(Path(home, "secrets").toString())
                val listening = CompletableDeferred<Unit>()
                val stop = CompletableDeferred<Unit>()
                val lines = mutableListOf<String>()
                val log = { line: String ->
                    lines += line
                    if (line.startsWith("aulod listening")) listening.complete(Unit)
                }
                val daemon = launch { runDaemon(home, config, secrets, log, { stop.await() }, grace = 1.seconds) }
                listening.await()

                val pinLine = lines.first()
                assertTrue(pinLine.startsWith("aulod TLS certificate SHA-256 "), pinLine)
                val pin = assertNotNull(Tls.parsePin(pinLine.substringAfterLast(' ')))
                assertContentEquals(serverTls(null, home).fingerprint, pin, "the logged pin is the stored certificate's")
                assertTrue(lines[1].endsWith(", 127.0.0.1:$port"), lines[1])

                val client = GrpcClient.connect(Endpoint.Tcp("127.0.0.1", port), tls = TlsClient(pin))
                try {
                    val token = assertNotNull(secrets.load(FIRST_TOKEN))
                    assertEquals(ServingStatus.SERVING, client.check("", listOf("authorization" to "Bearer $token")))
                } finally {
                    client.close()
                }
                stop.complete(Unit)
                daemon.join()
            }
        }

    @Test
    fun a_stale_pidfile_is_named_and_a_corrupt_one_is_not() =
        TempDir.create("aulo-daemon").use { tmp ->
            val home = tmp.path.toString()
            val pidFile = Path(RunFiles.dir(home), RunFiles.PID)
            val write = { text: String -> WriteOptions(createParent = true).write(pidFile, text.encodeToByteArray()) }
            assertNull(RunFiles.readPid(home))
            write("4242\n")
            assertEquals(4242L, RunFiles.readPid(home))
            for (bad in listOf("", "-1\n", "0", "pid", "1".repeat(25))) {
                write(bad)
                assertNull(RunFiles.readPid(home), bad)
            }
        }

    @Test
    fun listener_errors_come_before_the_home_is_touched() =
        transport {
            TempDir.create("aulo-daemon").use { tmp ->
                val home = Path(tmp.path, "home").toString()
                val config = Config(daemon = DaemonConfig(listen = "127.0.0.1:7777", tlsKey = "key.pem"))
                val e =
                    assertFailsWith<ListenerConfigException> {
                        runDaemon(home, config, FileSecretStore(Path(home, "secrets").toString()), {}, {})
                    }
                assertEquals(
                    "daemon.tls_cert and daemon.tls_key go together: set both, or neither for a self-signed certificate",
                    e.message,
                )
                assertFalse(SystemFileSystem.exists(Path(home)))
            }
        }
}

/** A TCP port nothing listens on right now; another process could take it before the test does. */
private suspend fun freePort(): Int {
    val server = GrpcServer(emptyMap())
    val bound = server.bind(Endpoint.Tcp("127.0.0.1", 0)) as Endpoint.Tcp
    server.shutdown(Duration.ZERO)
    return bound.port
}
