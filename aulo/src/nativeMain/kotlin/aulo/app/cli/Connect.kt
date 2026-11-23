package aulo.app.cli

import aulo.grpc.GrpcClient
import aulo.grpc.GrpcException
import aulo.grpc.StatusCode
import aulo.grpc.Tls
import aulo.grpc.TlsClient
import aulo.server.ListenerConfigException
import aulo.server.Listeners
import cli.Parsed
import io.grpc.health.v1.HealthCheckRequest
import io.grpc.health.v1.HealthCheckResponse
import kotlinx.coroutines.CancellationException
import kotlin.time.Duration.Companion.seconds

/** The environment variable that holds the bearer token for a remote aulod. */
const val TOKEN_ENV = "AULO_TOKEN"

private val CHECK_TIMEOUT = 10.seconds

/**
 * `aulo connect`: a TLS handshake with the aulod at [address] that accepts only the certificate
 * with [pin], then a health check, with the bearer token from [TOKEN_ENV] in [env] when it is
 * set. Nothing is saved.
 */
suspend fun connect(
    address: String,
    pin: String,
    env: Map<String, String>,
): Parsed.Exit {
    val endpoint =
        try {
            Listeners.parseListen(address)
        } catch (_: ListenerConfigException) {
            return failure("ADDR must be host:port or [ipv6]:port, not \"$address\"")
        }
    val pinBytes = Tls.parsePin(pin) ?: return failure("--pin must be the SHA-256 aulod logs: 64 hex digits, colons allowed")
    // Never echoed: a token in an error message would land in terminals and logs.
    val token = env[TOKEN_ENV]?.takeIf { it.isNotEmpty() }
    val metadata = token?.let { listOf("authorization" to "Bearer $it") } ?: emptyList()
    return try {
        val client = GrpcClient.connect(endpoint, tls = TlsClient(pinBytes))
        try {
            val response = client.unary("/grpc.health.v1.Health/Check", HealthCheckRequest().encode(), metadata, CHECK_TIMEOUT)
            val status = HealthCheckResponse.ADAPTER.decode(response).status
            Parsed.Exit("aulod at $address: $status, certificate matches the pin\n", stderr = false, code = 0)
        } finally {
            client.close()
        }
    } catch (e: GrpcException) {
        when {
            e.status.code == StatusCode.UNAUTHENTICATED && token == null -> {
                failure(
                    "aulod at $address wants a bearer token: set $TOKEN_ENV",
                )
            }

            e.status.code == StatusCode.UNAUTHENTICATED -> {
                failure("aulod at $address refused the token in $TOKEN_ENV")
            }

            else -> {
                failure(e.status.message.ifEmpty { e.status.code.name })
            }
        }
    } catch (e: CancellationException) {
        throw e
    } catch (e: Exception) {
        // A response that is not a health message: whatever answers is not aulod.
        failure("aulod at $address answered with something that is not a health check: ${e.message}")
    }
}

private fun failure(message: String) = Parsed.Exit("error: $message\n", stderr = true, code = 1)
