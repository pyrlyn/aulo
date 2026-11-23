package aulo.server

import aulo.config.DaemonConfig
import aulo.grpc.Endpoint

/** The daemon config names listeners aulod cannot open. */
class ListenerConfigException(
    message: String,
) : Exception(message)

/**
 * A TCP listener for remote clients; it always serves TLS (spec §11): [tls] names the files, or
 * is null for aulod's own self-signed certificate ([SelfSignedTls]).
 */
data class RemoteListener(
    val endpoint: Endpoint.Tcp,
    val tls: TlsFiles?,
)

/** Where aulod listens: always the owner-only local socket, and TCP when `daemon.listen` is set. */
data class Listeners(
    val local: Endpoint.Unix,
    val remote: RemoteListener? = null,
) {
    companion object {
        /** The owner-only directory in the aulo home that holds the socket. */
        const val RUN_DIR = "run"

        /** The socket file in [RUN_DIR]. */
        const val SOCKET_NAME = "aulod.sock"

        // sun_path is 104 bytes on macOS and 108 on Linux and Windows, with the terminating NUL.
        const val MAX_SOCKET_PATH_BYTES = 103

        fun from(
            config: DaemonConfig,
            home: String,
        ): Listeners {
            val separator = if ('\\' in home && '/' !in home) "\\" else "/"
            val socket = home.trimEnd('/', '\\') + separator + RUN_DIR + separator + SOCKET_NAME
            if (socket.encodeToByteArray().size > MAX_SOCKET_PATH_BYTES) {
                throw ListenerConfigException(
                    "the socket path $socket is longer than $MAX_SOCKET_PATH_BYTES bytes; set AULO_HOME to a shorter directory",
                )
            }
            val listen = config.listen ?: return Listeners(Endpoint.Unix(socket))
            val endpoint = parseListen(listen)
            // TLS is served on every TCP listener, loopback included (spec §11, §13); without
            // files of its own the listener uses aulod's self-signed certificate.
            val cert = config.tlsCert
            val key = config.tlsKey
            val files =
                when {
                    cert != null && key != null -> TlsFiles(cert, key)

                    cert == null && key == null -> null

                    else -> throw ListenerConfigException(
                        "daemon.tls_cert and daemon.tls_key go together: set both, or neither for a self-signed certificate",
                    )
                }
            return Listeners(Endpoint.Unix(socket), RemoteListener(endpoint, files))
        }

        /** `host:port` or `[ipv6]:port`, port 1 to 65535. */
        fun parseListen(value: String): Endpoint.Tcp {
            val bad = { ListenerConfigException("daemon.listen must be host:port or [ipv6]:port, not \"$value\"") }
            val colon = value.lastIndexOf(':')
            if (colon <= 0) throw bad()
            var host = value.substring(0, colon)
            if (host.startsWith('[')) {
                if (!host.endsWith(']') || host.length < 3) throw bad()
                host = host.substring(1, host.length - 1)
            } else if (':' in host) {
                throw bad()
            }
            if (host.any { it.isWhitespace() || it == '[' || it == ']' }) throw bad()
            val digits = value.substring(colon + 1)
            if (digits.isEmpty() || digits.length > 5 || !digits.all { it in '0'..'9' }) throw bad()
            val port = digits.toInt()
            if (port !in 1..65535) throw bad()
            return Endpoint.Tcp(host, port)
        }
    }
}
