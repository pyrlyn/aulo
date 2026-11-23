package aulo.server

import aulo.grpc.Tls
import aulo.grpc.TlsException
import aulo.grpc.TlsServer
import aulo.secrets.FileSecretStore
import aulo.secrets.SecretException
import fs.FsException
import fs.bytesIfExists
import fs.canonicalize
import fs.linkSize
import kotlinx.io.files.Path
import kotlin.time.Clock
import kotlin.time.Instant

/** The certificate and key files a TCP listener serves, as `daemon.tls_cert` and `daemon.tls_key` name them. */
data class TlsFiles(
    val cert: String,
    val key: String,
)

/**
 * aulod's own certificate, for a TCP listener configured without one: made once, kept in the
 * owner-only [DIR] of the aulo home, and reused, so the pin clients hold stays valid.
 */
object SelfSignedTls {
    const val DIR = "tls"
    const val CERT = "cert"
    const val KEY = "key"
    const val COMMON_NAME = "aulod"

    fun dir(home: String): Path = Path(home, DIR)
}

// A certificate chain or a key in PEM is a few KiB; anything far larger is not one.
private const val MAX_PEM_BYTES = 64 * 1024

/**
 * The TLS a TCP listener serves: [files] when the config names them (relative paths are in
 * [home]), otherwise [SelfSignedTls], made on the first run. Every problem is a
 * [ListenerConfigException].
 */
fun serverTls(
    files: TlsFiles?,
    home: String,
    now: Instant = Clock.System.now(),
): TlsServer =
    try {
        if (files != null) {
            TlsServer(readPem(resolve(files.cert, home), "daemon.tls_cert"), readPem(resolve(files.key, home), "daemon.tls_key"))
        } else {
            selfSigned(home, now)
        }
    } catch (e: TlsException) {
        throw ListenerConfigException(e.message ?: "the TLS certificate does not load")
    }

private fun selfSigned(
    home: String,
    now: Instant,
): TlsServer {
    val dir = SelfSignedTls.dir(home)
    // The secret store's files are what this needs: an owner-only directory, 0600 files, links
    // not followed and a size cap on what is read back.
    val store = FileSecretStore(dir.toString())
    try {
        val cert = store.load(SelfSignedTls.CERT)
        val key = store.load(SelfSignedTls.KEY)
        if (cert != null && key != null) {
            return try {
                TlsServer(cert.encodeToByteArray(), key.encodeToByteArray())
            } catch (e: TlsException) {
                throw ListenerConfigException("$dir: ${e.message}; remove $dir to make a new certificate")
            }
        }
        if (cert != null || key != null) {
            throw ListenerConfigException(
                "$dir holds only one of ${SelfSignedTls.CERT} and ${SelfSignedTls.KEY}; remove $dir to make a new certificate",
            )
        }
        val made = Tls.selfSigned(SelfSignedTls.COMMON_NAME, now)
        // The key first: a crash in between leaves a key without a certificate, which is refused above.
        store.save(SelfSignedTls.KEY, made.keyPem.decodeToString())
        store.save(SelfSignedTls.CERT, made.certPem.decodeToString())
        return made
    } catch (e: SecretException) {
        throw ListenerConfigException(e.message ?: "cannot keep the TLS certificate in $dir")
    }
}

private fun resolve(
    file: String,
    home: String,
): Path = Path(file).let { if (it.isAbsolute) it else Path(home, file) }

private fun readPem(
    path: Path,
    setting: String,
): ByteArray =
    try {
        // Followed on purpose: certificates are often links into a renewal tool's directory.
        val target = canonicalize(path)
        val missing = { ListenerConfigException("$setting: $path does not exist") }
        val size = linkSize(target) ?: throw missing()
        if (size > MAX_PEM_BYTES) throw ListenerConfigException("$setting: $path is larger than $MAX_PEM_BYTES bytes")
        bytesIfExists(target) ?: throw missing()
    } catch (e: FsException) {
        throw ListenerConfigException("$setting: cannot read $path: ${e.detail}")
    }
