package aulo.models

import fs.FsException
import fs.appendSink
import fs.entryKind
import fs.linkSize
import fs.removeAny
import fs.rename
import http.HttpBody
import http.HttpClient
import http.HttpException
import http.HttpHead
import http.HttpRequest
import kotlinx.io.Sink
import kotlinx.io.buffered
import kotlinx.io.files.Path
import kotlinx.io.files.SystemFileSystem
import okio.HashingSink
import okio.blackholeSink
import fs.EntryKind as Kind

private const val CHUNK = 64 * 1024
private const val HTTP_OK = 200
private const val HTTP_PARTIAL = 206
private const val HTTP_RANGE_NOT_SATISFIABLE = 416

/** What one file must be once it is downloaded. */
internal data class Expected(
    val url: String,
    /** The catalog path, used in messages. */
    val name: String,
    val size: Long,
    val sha256: String,
)

/**
 * Downloads [want] to [dest] and returns only once the bytes match the pinned SHA-256.
 *
 * The body streams into `<dest>.part` and is hashed on the way. A hash, size or HTTP failure deletes the partial, so
 * the final name never holds bytes that were not checked. A dropped connection or a cancelled coroutine keeps the
 * partial, and the next call resumes it with a `Range` request. [onBytes] receives the bytes this file has so far.
 *
 * pyrlyn-kit's append sink cannot fsync, so a power loss right after the rename may leave a short file under the
 * final name; [ModelManager] tells it apart by size and downloads it again.
 */
internal suspend fun fetchVerified(
    http: HttpClient,
    want: Expected,
    dest: Path,
    onBytes: (Long) -> Unit,
) {
    val part = Path("$dest$PART_SUFFIX")
    var hasher = Sha256()
    var have = resumableLength(part, want.size)
    if (have > 0) {
        hashFile(part, hasher)
        onBytes(have)
    }

    if (have < want.size) {
        val headers = if (have > 0) listOf("Range" to "bytes=$have-") else emptyList()
        val resumed = have
        have =
            try {
                http.stream(HttpRequest(want.url, headers)) { head, body ->
                    var start = resumed
                    when {
                        head.status == HTTP_OK && start > 0 -> {
                            // The server ignored the range; what we hold is of no use.
                            start = 0
                            hasher = Sha256()
                        }

                        head.status == HTTP_PARTIAL && start > 0 -> {
                            if (rangeStart(head) != start) {
                                discard(part)
                                throw ModelException.BadRange(want.url)
                            }
                        }

                        head.status != HTTP_OK -> {
                            if (head.status == HTTP_RANGE_NOT_SATISFIABLE) discard(part)
                            throw ModelException.Http(want.url, head.status)
                        }
                    }
                    // Refuse early when the server announces more than the pin allows.
                    val announced = head.header("Content-Length")?.toLongOrNull()
                    if (announced != null && start + announced > want.size) {
                        discard(part)
                        throw ModelException.TooLarge(want.name, want.size)
                    }
                    streamTo(body, part, start, want, hasher, onBytes)
                }
            } catch (e: HttpException) {
                throw ModelException.Network(want.url, e)
            }
    }

    if (have != want.size) throw ModelException.Truncated(want.name, have, want.size)
    val actual = hasher.hex()
    if (actual != want.sha256) {
        discard(part)
        throw ModelException.HashMismatch(want.name, want.sha256, actual)
    }
    io(dest) { rename(part, dest) }
}

/**
 * Appends [body] to the partial (or restarts it when [have] is 0) and returns the new length. The sink is closed even
 * when the stream fails, so a later resume sees every byte that was hashed.
 */
private suspend fun streamTo(
    body: HttpBody,
    part: Path,
    have: Long,
    want: Expected,
    hasher: Sha256,
    onBytes: (Long) -> Unit,
): Long {
    if (have == 0L) io(part) { removeAny(part) }
    val sink: Sink = io(part) { appendSink(part) }
    var total = have
    val buffer = ByteArray(CHUNK)
    try {
        while (true) {
            val n = body.read(buffer)
            if (n < 0) break
            if (n == 0) continue
            total += n
            if (total > want.size) {
                discard(part)
                throw ModelException.TooLarge(want.name, want.size)
            }
            hasher.update(buffer, n)
            try {
                sink.write(buffer, 0, n)
            } catch (e: kotlinx.io.IOException) {
                discard(part)
                throw ModelException.Io(part.toString(), e)
            }
            onBytes(total)
        }
    } finally {
        runCatching { sink.close() }
    }
    return total
}

/** Length of a usable partial; anything else (none, not a file, longer than the file) is removed so the download starts clean. */
private fun resumableLength(
    part: Path,
    size: Long,
): Long {
    val length = if (entryKind(part) == Kind.File) linkSize(part) else null
    if (length != null && length <= size) return length
    discard(part)
    return 0
}

private fun hashFile(
    path: Path,
    hasher: Sha256,
) {
    io(path) {
        SystemFileSystem.source(path).buffered().use { source ->
            val buffer = ByteArray(CHUNK)
            while (true) {
                val n = source.readAtMostTo(buffer)
                if (n < 0) break
                hasher.update(buffer, n)
            }
        }
    }
}

/** `Content-Range: bytes <start>-<end>/<total>` → `start`. */
private fun rangeStart(head: HttpHead): Long? =
    head
        .header("Content-Range")
        ?.removePrefix("bytes ")
        ?.substringBefore('-')
        ?.toLongOrNull()

/**
 * Removing a partial that is already gone, or cannot be removed, changes nothing for the caller: the error that led
 * here is the one to report.
 */
private fun discard(part: Path) {
    runCatching { removeAny(part) }
}

internal inline fun <T> io(
    path: Path,
    block: () -> T,
): T =
    try {
        block()
    } catch (e: kotlinx.io.IOException) {
        throw ModelException.Io(path.toString(), e)
    }

/** Streaming SHA-256 through okio, which the build already carries for Wire. */
private class Sha256 {
    private val sink = HashingSink.sha256(blackholeSink())
    private val buffer = okio.Buffer()

    fun update(
        bytes: ByteArray,
        count: Int,
    ) {
        buffer.write(bytes, 0, count)
        sink.write(buffer, count.toLong())
    }

    fun hex(): String = sink.hash.hex()
}

/** The OS message without the path, which [ModelException.Io] already names. */
internal fun ioDetail(cause: Throwable): String = (cause as? FsException)?.detail ?: cause.message ?: cause::class.simpleName.orEmpty()
