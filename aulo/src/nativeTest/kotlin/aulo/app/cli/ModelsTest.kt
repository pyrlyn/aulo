package aulo.app.cli

import aulo.models.Catalog
import aulo.models.Model
import aulo.models.ModelFile
import aulo.models.ModelKind
import aulo.models.ModelManager
import fs.TempDir
import http.HttpBody
import http.HttpClient
import http.HttpHead
import http.HttpRequest
import http.HttpResponse
import kotlinx.coroutines.runBlocking
import okio.ByteString.Companion.toByteString
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** `aulo models pull` end to end over a catalog and a server of the test's own; the fixtures cover the rest. */
class ModelsTest {
    private val bytes = ByteArray(3_000_000) { (it % 251).toByte() }
    private val catalog =
        Catalog(
            listOf(
                Model(
                    "tiny",
                    ModelKind.TTS,
                    "Tiny voice",
                    "MIT",
                    "https://models.test/",
                    listOf(ModelFile("voice.bin", bytes.size.toLong(), bytes.toByteString().sha256().hex())),
                ),
            ),
        )

    private class OneFile(
        private val bytes: ByteArray,
    ) : HttpClient {
        var closed = false

        override suspend fun post(req: HttpRequest): HttpResponse = error("GET only")

        override suspend fun <T> stream(
            req: HttpRequest,
            handle: suspend (HttpHead, HttpBody) -> T,
        ): T {
            var sent = 0
            return handle(HttpHead(200, emptyList())) { buffer ->
                if (sent == bytes.size) return@handle -1
                val n = minOf(buffer.size, bytes.size - sent)
                bytes.copyInto(buffer, 0, sent, sent + n)
                sent += n
                n
            }
        }

        override fun close() {
            closed = true
        }
    }

    @Test fun pullReportsProgressInStepsAndInstalls() =
        TempDir.create("aulo-models-cli").use { home ->
            val manager = ModelManager(home.path, catalog)
            val server = OneFile(bytes)
            val lines = mutableListOf<String>()
            val exit = runBlocking { models("pull", "tiny", manager, { server }, { lines += it }) }
            assertEquals(0, exit.code, exit.text)
            assertTrue(exit.text.startsWith("tiny installed in "), exit.text)
            assertEquals("Tiny voice: MIT, 3.0 MB\n", lines.first())
            assertEquals("  voice.bin (3.0 MB)\n", lines[lines.size - 2])
            assertEquals("  100% (3.0 MB of 3.0 MB)\n", lines.last())
            // One line per step, not one per network chunk.
            assertTrue(lines.size <= 2 + 100 / 5 + 1, lines.toString())
            assertTrue(server.closed)
            val ls = runBlocking { models("ls", null, manager, { error("ls needs no network") }, {}) }
            assertTrue("installed" in ls.text, ls.text)
        }

    @Test fun megabytesRoundToOneDigit() {
        assertEquals("0.0 MB", mb(0))
        assertEquals("0.1 MB", mb(93_939))
        assertEquals("670.5 MB", mb(670_478_772))
        assertEquals("1.0 MB", mb(999_999))
    }
}
