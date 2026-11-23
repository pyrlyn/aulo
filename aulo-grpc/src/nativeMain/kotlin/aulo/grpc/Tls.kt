package aulo.grpc

import aulo.grpc.tls.AULO_TLS_ERR_ALPN
import aulo.grpc.tls.AULO_TLS_ERR_MEMORY
import aulo.grpc.tls.AULO_TLS_ERR_OVERFLOW
import aulo.grpc.tls.AULO_TLS_ERR_PIN
import aulo.grpc.tls.AULO_TLS_ERR_PSA
import aulo.grpc.tls.AULO_TLS_PIN_BYTES
import aulo.grpc.tls.AULO_TLS_WANT_READ
import aulo.grpc.tls.AULO_TLS_WANT_WRITE
import aulo.grpc.tls.aulo_tls_client
import aulo.grpc.tls.aulo_tls_feed
import aulo.grpc.tls.aulo_tls_fingerprint
import aulo.grpc.tls.aulo_tls_free
import aulo.grpc.tls.aulo_tls_handshake
import aulo.grpc.tls.aulo_tls_read
import aulo.grpc.tls.aulo_tls_self_signed
import aulo.grpc.tls.aulo_tls_server
import aulo.grpc.tls.aulo_tls_strerror
import aulo.grpc.tls.aulo_tls_take
import aulo.grpc.tls.aulo_tls_write
import kotlinx.cinterop.ByteVar
import kotlinx.cinterop.COpaquePointer
import kotlinx.cinterop.CPointer
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.IntVar
import kotlinx.cinterop.UByteVar
import kotlinx.cinterop.addressOf
import kotlinx.cinterop.alloc
import kotlinx.cinterop.allocArray
import kotlinx.cinterop.convert
import kotlinx.cinterop.memScoped
import kotlinx.cinterop.pointed
import kotlinx.cinterop.ptr
import kotlinx.cinterop.reinterpret
import kotlinx.cinterop.toKString
import kotlinx.cinterop.usePinned
import kotlinx.cinterop.value
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlin.time.Instant

/** A TLS certificate or key that does not load, or a handshake that failed. */
class TlsException(
    message: String,
) : Exception(message)

/**
 * TLS for a server: a PEM certificate chain (leaf first) and its PEM private key. The pair is
 * loaded once here, so a bad one fails before anything listens.
 */
@OptIn(ExperimentalForeignApi::class)
class TlsServer(
    certPem: ByteArray,
    keyPem: ByteArray,
) {
    val certPem: ByteArray = certPem.copyOf()
    val keyPem: ByteArray = keyPem.copyOf()

    /** What clients pin: the SHA-256 of the leaf certificate's DER. */
    val fingerprint: ByteArray = Tls.fingerprint(this.certPem)

    init {
        if (this.certPem.isEmpty() || this.keyPem.isEmpty()) throw TlsException("the TLS certificate and key must not be empty")
        aulo_tls_free(newHandle(this))
    }
}

/**
 * TLS for a client that trusts exactly one server certificate: the one whose DER has the SHA-256
 * [pin]. No certificate authority is consulted.
 */
class TlsClient(
    pin: ByteArray,
) {
    val pin: ByteArray = pin.copyOf()

    init {
        require(pin.size == Tls.PIN_BYTES) { "a pin is ${Tls.PIN_BYTES} bytes, not ${pin.size}" }
    }
}

@OptIn(ExperimentalForeignApi::class)
object Tls {
    const val PIN_BYTES: Int = AULO_TLS_PIN_BYTES

    // RFC 5280 §4.1.2.5: a certificate with no well-defined expiry. Clients pin the certificate
    // itself, so a date by which it must be replaced would only break working setups.
    private const val NO_EXPIRY = "99991231235959"

    // A P-256 certificate or key in PEM is well under 1 KiB.
    private const val PEM_CAP = 4096

    /** The SHA-256 of the DER of the first certificate in [certPem]. */
    fun fingerprint(certPem: ByteArray): ByteArray {
        if (certPem.isEmpty()) throw TlsException("the TLS certificate is empty")
        val digest = ByteArray(PIN_BYTES)
        val rc =
            certPem.usePinned { pem ->
                digest.usePinned { out -> aulo_tls_fingerprint(pem.ubytes(), certPem.size.convert(), out.ubytes()) }
            }
        if (rc != 0) throw TlsException("the TLS certificate does not load: ${describeTls(rc)}")
        return digest
    }

    /** A new self-signed ECDSA P-256 certificate for [commonName], valid from [now] on. */
    fun selfSigned(
        commonName: String,
        now: Instant,
    ): TlsServer {
        require(commonName.isNotEmpty() && commonName.all { it.isLetterOrDigit() || it in " .-_" }) {
            "a common name is letters, digits, spaces and .-_: $commonName"
        }
        // Instant prints as ISO 8601 (2026-11-23T10:15:30Z); its digits are the ASN.1 form.
        val notBefore = Instant.fromEpochSeconds(now.epochSeconds).toString().filter { it.isDigit() }
        check(notBefore.length == NO_EXPIRY.length) { "a year outside 0000..9999: $now" }
        val cert = ByteArray(PEM_CAP)
        val key = ByteArray(PEM_CAP)
        val rc =
            cert.usePinned { c ->
                key.usePinned { k ->
                    aulo_tls_self_signed(
                        "CN=$commonName",
                        notBefore,
                        NO_EXPIRY,
                        c.ubytes(),
                        PEM_CAP.convert(),
                        k.ubytes(),
                        PEM_CAP.convert(),
                    )
                }
            }
        if (rc != 0) throw TlsException("cannot make a TLS certificate: ${describeTls(rc)}")
        val server = TlsServer(cert.untilNul(), key.untilNul())
        key.fill(0)
        return server
    }

    /** The pin as `openssl x509 -fingerprint -sha256` prints it: upper-case hex pairs and colons. */
    fun formatPin(pin: ByteArray): String = pin.joinToString(":") { (it.toInt() and 0xff).toString(16).padStart(2, '0').uppercase() }

    /** A pin from hex, with or without colons, in either case; null when it is not 32 bytes of hex. */
    fun parsePin(text: String): ByteArray? {
        val hex = text.replace(":", "")
        if (hex.length != PIN_BYTES * 2 || !hex.all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }) return null
        return ByteArray(PIN_BYTES) { hex.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
    }
}

/**
 * HTTP/2 bytes through TLS 1.3. mbedTLS sees only memory: [fill] feeds it what the socket
 * delivered and [drain] writes out what it produced. One coroutine reads while another writes;
 * both may produce records (a read can answer a key update), so [drain] runs under a lock that
 * keeps the records in the order mbedTLS made them.
 */
@OptIn(ExperimentalForeignApi::class)
internal class TlsStream private constructor(
    private val raw: ByteStream,
    private val handle: COpaquePointer,
) : ByteStream {
    private val drainLock = Mutex()
    private val cipher = ByteArray(READ_CHUNK)
    private val pending = ByteArray(READ_CHUNK)
    private var freed = false

    override suspend fun read(buf: ByteArray): Int {
        while (true) {
            val n = buf.usePinned { aulo_tls_read(handle, it.ubytes(), buf.size.convert()) }
            when {
                n > 0 -> {
                    return n
                }

                n == 0 -> {
                    return -1
                }

                n == AULO_TLS_WANT_READ -> {
                    drain()
                    if (!fill()) return -1
                }

                n == AULO_TLS_WANT_WRITE -> {
                    drain()
                }

                else -> {
                    // A fatal alert may be queued for the peer; it is a courtesy, not a must.
                    runCatching { drain() }
                    throw TlsException(describeTls(n))
                }
            }
        }
    }

    override suspend fun write(bytes: ByteArray) {
        var offset = 0
        while (offset < bytes.size) {
            val n = bytes.usePinned { aulo_tls_write(handle, it.ubytes(offset), (bytes.size - offset).convert()) }
            when {
                n > 0 -> offset += n
                n == AULO_TLS_WANT_WRITE -> drain()
                else -> throw TlsException(describeTls(n))
            }
        }
    }

    override suspend fun flush() = drain()

    override fun close() {
        if (!freed) {
            freed = true
            aulo_tls_free(handle)
        }
        raw.close()
    }

    private suspend fun handshake(): TlsStream {
        while (true) {
            val rc = aulo_tls_handshake(handle)
            // Sent even when the handshake failed: it carries the alert that says why.
            drain()
            when (rc) {
                0 -> {
                    return this
                }

                AULO_TLS_WANT_READ -> {
                    if (!fill()) throw TlsException("the peer closed the connection during the TLS handshake")
                }

                AULO_TLS_WANT_WRITE -> {}

                else -> {
                    throw TlsException(describeTls(rc))
                }
            }
        }
    }

    /** Feeds the next bytes from the socket to mbedTLS; false once the peer has closed. */
    private suspend fun fill(): Boolean {
        val n = raw.read(cipher)
        if (n < 0) return false
        val rc = cipher.usePinned { aulo_tls_feed(handle, it.ubytes(), n.convert()) }
        if (rc != 0) throw TlsException(describeTls(rc))
        return true
    }

    private suspend fun drain() =
        drainLock.withLock {
            var wrote = false
            while (true) {
                val n = pending.usePinned { aulo_tls_take(handle, it.ubytes(), pending.size.convert()) }
                if (n == 0) break
                raw.write(pending.copyOf(n))
                wrote = true
            }
            if (wrote) raw.flush()
        }

    companion object {
        /** The server side of a handshake on [raw]; closes [raw] when it fails. */
        suspend fun accept(
            raw: ByteStream,
            tls: TlsServer,
        ): TlsStream = start(raw) { error -> newHandle(tls, error) }

        /** The client side of a handshake on [raw], checking the pin; closes [raw] when it fails. */
        suspend fun connect(
            raw: ByteStream,
            tls: TlsClient,
        ): TlsStream =
            start(raw) { error ->
                tls.pin.usePinned { aulo_tls_client(it.ubytes(), error) }
                    ?: throw TlsException("cannot start TLS: ${describeTls(error.pointed.value)}")
            }

        private suspend fun start(
            raw: ByteStream,
            open: (CPointer<IntVar>) -> COpaquePointer,
        ): TlsStream {
            val stream =
                try {
                    TlsStream(raw, memScoped { open(alloc<IntVar>().ptr) })
                } catch (e: Throwable) {
                    raw.close()
                    throw e
                }
            try {
                return stream.handshake()
            } catch (e: Throwable) {
                stream.close()
                throw e
            }
        }
    }
}

@OptIn(ExperimentalForeignApi::class)
private fun newHandle(
    tls: TlsServer,
    error: CPointer<IntVar>? = null,
): COpaquePointer =
    memScoped {
        val code = error ?: alloc<IntVar>().ptr
        tls.certPem.usePinned { c ->
            tls.keyPem.usePinned { k ->
                aulo_tls_server(c.ubytes(), tls.certPem.size.convert(), k.ubytes(), tls.keyPem.size.convert(), code)
            }
        } ?: throw TlsException("the TLS certificate or key does not load: ${describeTls(code.pointed.value)}")
    }

@OptIn(ExperimentalForeignApi::class)
private fun kotlinx.cinterop.Pinned<ByteArray>.ubytes(offset: Int = 0): CPointer<UByteVar> = addressOf(offset).reinterpret()

private fun ByteArray.untilNul(): ByteArray = copyOf(indexOf(0).let { if (it < 0) size else it })

/** Words for a shim or mbedTLS error code. */
@OptIn(ExperimentalForeignApi::class)
internal fun describeTls(code: Int): String =
    when (code) {
        AULO_TLS_ERR_PIN -> {
            "the server's certificate does not match the pin"
        }

        AULO_TLS_ERR_ALPN -> {
            "the peer does not offer HTTP/2 (ALPN h2)"
        }

        AULO_TLS_ERR_OVERFLOW -> {
            "the peer sent more than a TLS record holds"
        }

        AULO_TLS_ERR_MEMORY -> {
            "out of memory"
        }

        AULO_TLS_ERR_PSA -> {
            "the crypto library did not start"
        }

        else -> {
            memScoped {
                val buf = allocArray<ByteVar>(STRERROR_CAP)
                aulo_tls_strerror(code, buf, STRERROR_CAP.convert())
                buf.toKString()
            }
        }
    }

private const val STRERROR_CAP = 160
