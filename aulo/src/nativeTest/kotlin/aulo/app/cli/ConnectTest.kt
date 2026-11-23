package aulo.app.cli

import aulo.grpc.Endpoint
import aulo.grpc.Tls
import aulo.server.AuloServer
import aulo.server.BearerTokens
import cli.Parsed
import kotlinx.coroutines.runBlocking
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.time.Clock
import kotlin.time.Duration.Companion.seconds

class ConnectTest {
    private val tls = Tls.selfSigned("aulo-test", Clock.System.now())
    private val pin = Tls.formatPin(tls.fingerprint)
    private val token = BearerTokens.generate()

    @Test
    fun the_right_pin_and_token_reach_the_health_check() =
        serving { address ->
            val ok = connect(address, pin.replace(":", "").lowercase(), mapOf(TOKEN_ENV to token))
            assertExit(Parsed.Exit("aulod at $address: SERVING, certificate matches the pin\n", stderr = false, code = 0), ok)
        }

    @Test
    fun a_missing_or_wrong_token_says_which_and_never_echoes_it() =
        serving { address ->
            val missing = connect(address, pin, emptyMap())
            assertExit(Parsed.Exit("error: aulod at $address wants a bearer token: set $TOKEN_ENV\n", stderr = true, code = 1), missing)
            val wrong = BearerTokens.generate()
            val refused = connect(address, pin, mapOf(TOKEN_ENV to wrong))
            assertExit(Parsed.Exit("error: aulod at $address refused the token in $TOKEN_ENV\n", stderr = true, code = 1), refused)
            assertFalse(wrong in refused.text)
        }

    @Test
    fun another_certificate_is_refused() =
        serving { address ->
            val other = Tls.formatPin(Tls.selfSigned("aulo-test", Clock.System.now()).fingerprint)
            val refused = connect(address, other, mapOf(TOKEN_ENV to token))
            assertEquals(1, refused.code)
            assertContains(refused.text, "does not match the pin")
            assertFalse(token in refused.text)
        }

    private fun assertExit(
        expected: Parsed.Exit,
        actual: Parsed.Exit,
    ) = assertEquals(Triple(expected.text, expected.stderr, expected.code), Triple(actual.text, actual.stderr, actual.code))

    private fun serving(block: suspend (String) -> Unit) =
        runBlocking {
            val server = AuloServer(emptyMap(), tokenHashes = listOf(BearerTokens.hash(token)))
            try {
                val endpoint = server.bind(Endpoint.Tcp("127.0.0.1", 0), tls) as Endpoint.Tcp
                block("${endpoint.host}:${endpoint.port}")
            } finally {
                server.shutdown(1.seconds)
            }
        }
}
