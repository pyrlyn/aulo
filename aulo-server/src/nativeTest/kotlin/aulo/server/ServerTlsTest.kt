package aulo.server

import aulo.grpc.Endpoint
import aulo.grpc.GrpcClient
import aulo.grpc.StatusCode
import aulo.grpc.Tls
import aulo.grpc.TlsClient
import fs.TempDir
import fs.WriteOptions
import fs.mode
import fs.removeFile
import io.grpc.health.v1.HealthCheckResponse.ServingStatus
import kotlinx.io.files.Path
import kotlin.experimental.ExperimentalNativeApi
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.time.Clock
import kotlin.time.Duration.Companion.seconds

@OptIn(ExperimentalNativeApi::class)
private val posix = Platform.osFamily != OsFamily.WINDOWS

class ServerTlsTest {
    @Test
    fun the_self_signed_certificate_is_made_once_owner_only_and_reused() =
        TempDir.create("aulo-tls").use { tmp ->
            val home = tmp.path.toString()
            val first = serverTls(null, home)
            val again = serverTls(null, home)
            assertContentEquals(first.fingerprint, again.fingerprint, "the pin clients hold stays valid")
            val dir = SelfSignedTls.dir(home)
            if (posix) {
                assertEquals(0b111_000_000, mode(dir)!! and 0b111_111_111)
                assertEquals(0b110_000_000, mode(Path(dir, SelfSignedTls.KEY))!! and 0b111_111_111)
            }

            removeFile(Path(dir, SelfSignedTls.CERT))
            val half = assertFailsWith<ListenerConfigException> { serverTls(null, home) }
            assertEquals("$dir holds only one of cert and key; remove $dir to make a new certificate", half.message)
        }

    @Test
    fun configured_files_load_relative_to_the_home_and_are_checked() =
        TempDir.create("aulo-tls").use { tmp ->
            val home = tmp.path.toString()
            val made = Tls.selfSigned("aulo-test", Clock.System.now())
            val other = Tls.selfSigned("aulo-test", Clock.System.now())
            val write = { name: String, bytes: ByteArray -> WriteOptions().write(Path(home, name), bytes) }
            write("cert.pem", made.certPem)
            write("key.pem", made.keyPem)
            write("other.pem", other.keyPem)
            write("huge.pem", ByteArray(64 * 1024 + 1) { 'A'.code.toByte() })

            assertContentEquals(made.fingerprint, serverTls(TlsFiles("cert.pem", "key.pem"), home).fingerprint)
            assertContentEquals(made.fingerprint, serverTls(TlsFiles(Path(home, "cert.pem").toString(), "key.pem"), home).fingerprint)

            val missing = assertFailsWith<ListenerConfigException> { serverTls(TlsFiles("nope.pem", "key.pem"), home) }
            assertContains(missing.message!!, "daemon.tls_cert")
            val huge = assertFailsWith<ListenerConfigException> { serverTls(TlsFiles("cert.pem", "huge.pem"), home) }
            assertContains(huge.message!!, "daemon.tls_key: ")
            assertContains(huge.message!!, "is larger than 65536 bytes")
            assertFailsWith<ListenerConfigException> { serverTls(TlsFiles("cert.pem", "other.pem"), home) }
            assertFailsWith<ListenerConfigException> { serverTls(TlsFiles("key.pem", "key.pem"), home) }
            Unit
        }

    @Test
    fun tokens_are_still_required_over_tls() =
        transport {
            TempDir.create("aulo-tls").use { tmp ->
                val tls = serverTls(null, tmp.path.toString())
                val token = BearerTokens.generate()
                val server = AuloServer(emptyMap(), tokenHashes = listOf(BearerTokens.hash(token)))
                val endpoint = server.bind(Endpoint.Tcp("127.0.0.1", 0), tls)
                val client = GrpcClient.connect(endpoint, tls = TlsClient(tls.fingerprint))
                try {
                    assertStatus(StatusCode.UNAUTHENTICATED) { client.check("") }
                    assertEquals(ServingStatus.SERVING, client.check("", listOf("authorization" to "Bearer $token")))
                } finally {
                    client.close()
                    server.shutdown(1.seconds)
                }
            }
        }
}
