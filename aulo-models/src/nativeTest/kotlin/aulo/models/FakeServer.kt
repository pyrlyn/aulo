package aulo.models

import http.HttpBody
import http.HttpClient
import http.HttpException
import http.HttpHead
import http.HttpRequest
import http.HttpResponse
import okio.ByteString.Companion.toByteString

const val BASE = "https://models.test/m/"

/** An in-process server that honours `Range` unless told otherwise, so downloads are tested without a network. */
class FakeServer : HttpClient {
    val files = mutableMapOf<String, ByteArray>()
    val requests = mutableListOf<HttpRequest>()

    /** Answer every request with this status and an empty body. */
    var status: Int? = null

    /** Send the whole file with 200 even when a range was asked for. */
    var ignoreRange = false

    /** Claim this start in `Content-Range` instead of the real one. */
    var claimedStart: Long? = null

    /** Announce this `Content-Length` instead of the real one; a negative value announces none. */
    var announce: Long? = null

    /** Drop the connection once this many body bytes went out. */
    var dropAfter: Int? = null

    /** Bytes sent after the file's own, as a server that serves the wrong file would. */
    var extra = ByteArray(0)

    fun serve(
        path: String,
        bytes: ByteArray,
    ) {
        files[BASE + path] = bytes
    }

    fun range(index: Int): String? = requests[index].headers.firstOrNull { it.first == "Range" }?.second

    override suspend fun post(req: HttpRequest): HttpResponse = error("models only GET")

    override suspend fun <T> stream(
        req: HttpRequest,
        handle: suspend (HttpHead, HttpBody) -> T,
    ): T {
        requests += req
        status?.let { return handle(HttpHead(it, emptyList()), HttpBody { -1 }) }
        val file = files[req.url] ?: return handle(HttpHead(404, emptyList()), HttpBody { -1 })
        val asked =
            req.headers
                .firstOrNull { it.first == "Range" }
                ?.second
                ?.removePrefix("bytes=")
                ?.removeSuffix("-")
                ?.toInt()
        val start = if (asked != null && !ignoreRange) asked else 0
        val body = file.copyOfRange(start, file.size) + extra
        val headers = mutableListOf<Pair<String, String>>()
        val length = announce ?: body.size.toLong()
        if (length >= 0) headers += "Content-Length" to length.toString()
        if (start > 0) headers += "Content-Range" to "bytes ${claimedStart ?: start}-${file.size - 1}/${file.size}"
        var sent = 0
        val reader =
            HttpBody { buffer ->
                val limit = dropAfter
                if (limit != null && sent >= limit) throw HttpException(HttpException.Kind.Io, "connection reset")
                if (sent == body.size) return@HttpBody -1
                // Small chunks, so a drop or a cap lands partway through a file.
                var n = minOf(buffer.size, body.size - sent, CHUNK_OUT)
                if (limit != null) n = minOf(n, limit - sent)
                body.copyInto(buffer, 0, sent, sent + n)
                sent += n
                n
            }
        return handle(HttpHead(if (start > 0) 206 else 200, headers), reader)
    }

    override fun close() = Unit

    private companion object {
        const val CHUNK_OUT = 700
    }
}

fun sha256(bytes: ByteArray): String = bytes.toByteString().sha256().hex()

/** Deterministic test content of [size] bytes. */
fun content(
    size: Int,
    seed: Int = 1,
): ByteArray = ByteArray(size) { ((it * 31 + seed * 7) % 251).toByte() }

fun model(
    id: String,
    vararg files: Pair<String, ByteArray>,
): Model =
    Model(
        id = id,
        kind = ModelKind.STT,
        name = "Test $id",
        license = "MIT",
        baseUrl = BASE,
        files = files.map { (path, bytes) -> ModelFile(path, bytes.size.toLong(), sha256(bytes)) },
    )
