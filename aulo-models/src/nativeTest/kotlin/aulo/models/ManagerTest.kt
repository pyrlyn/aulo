package aulo.models

import fs.FileLock
import fs.TempDir
import fs.WriteOptions
import fs.bytesIfExists
import fs.createDirAll
import fs.entryKind
import fs.joinPath
import kotlinx.coroutines.runBlocking
import kotlinx.io.files.Path
import kotlin.test.AfterTest
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class ManagerTest {
    private val home = TempDir.create("aulo-models")
    private val server = FakeServer()
    private val voice = content(5_000, seed = 1)
    private val tokens = content(1_200, seed = 2)
    private val nested = content(900, seed = 3)
    private val catalog = Catalog(listOf(model("voice-1", "voice.onnx" to voice, "tokens.txt" to tokens, "espeak/data.bin" to nested)))
    private val manager = ModelManager(home.path, catalog)
    private val dir get() = joinPath(home.path, "models", "voice-1")

    init {
        server.serve("voice.onnx", voice)
        server.serve("tokens.txt", tokens)
        server.serve("espeak/data.bin", nested)
    }

    @AfterTest fun cleanUp() = home.close()

    private fun file(name: String): Path = joinPath(dir, name)

    private fun partial(name: String): Path = Path("${file(name)}$PART_SUFFIX")

    private fun write(
        path: Path,
        bytes: ByteArray,
    ) = WriteOptions(createParent = true).write(path, bytes)

    private fun pull(onProgress: (Progress) -> Unit = {}) = runBlocking { manager.pull("voice-1", server, onProgress) }

    private fun nothingKept(name: String) {
        assertNull(entryKind(file(name)), "$name was kept")
        assertNull(entryKind(partial(name)), "$name.part was kept")
    }

    @Test fun pullInstallsEveryFileAndReportsProgress() {
        assertFalse(manager.list().single().installed)
        assertNull(manager.path("voice-1"))
        val seen = mutableListOf<Progress>()
        val path = pull { seen += it }
        assertEquals(dir, path)
        assertContentEquals(voice, bytesIfExists(file("voice.onnx")))
        assertContentEquals(tokens, bytesIfExists(file("tokens.txt")))
        assertContentEquals(nested, bytesIfExists(file("espeak/data.bin")))
        assertEquals(catalog.models[0].totalSize, seen.last().bytes)
        assertEquals(seen.last().totalBytes, seen.last().bytes)
        assertTrue(seen.zipWithNext().all { (a, b) -> a.bytes <= b.bytes })
        assertEquals(dir, manager.path("voice-1"))
        assertEquals(3, manager.list().single().filesPresent)
        assertTrue(manager.list().single().installed)
    }

    @Test fun installedFilesAreNotFetchedAgain() {
        pull()
        val before = server.requests.size
        pull()
        assertEquals(before, server.requests.size)
    }

    @Test fun aCorruptedDownloadIsRejectedAndNothingIsKept() {
        server.serve("voice.onnx", content(voice.size, seed = 9))
        val e = assertFailsWith<ModelException.HashMismatch> { pull() }
        assertEquals("voice.onnx", e.file)
        assertEquals(sha256(voice), e.expected)
        nothingKept("voice.onnx")
        assertNull(manager.path("voice-1"))
    }

    @Test fun aBodyLongerThanThePinIsCutOff() {
        server.announce = -1
        server.extra = ByteArray(10_000)
        val e = assertFailsWith<ModelException.TooLarge> { pull() }
        assertEquals(voice.size.toLong(), e.limit)
        nothingKept("voice.onnx")
    }

    @Test fun anAnnouncedOversizeIsRefusedBeforeTheBody() {
        server.announce = voice.size + 1L
        assertFailsWith<ModelException.TooLarge> { pull() }
        nothingKept("voice.onnx")
    }

    @Test fun anHttpErrorNamesTheUrlAndStatus() {
        server.status = 503
        val e = assertFailsWith<ModelException.Http> { pull() }
        assertEquals(503, e.status)
        assertEquals(BASE + "voice.onnx", e.url)
        server.files.remove(BASE + "voice.onnx")
        server.status = null
        assertEquals(404, assertFailsWith<ModelException.Http> { pull() }.status)
    }

    @Test fun aDroppedConnectionKeepsThePartialAndTheNextPullResumes() {
        server.dropAfter = 2_100
        val e = assertFailsWith<ModelException.Network> { pull() }
        assertTrue("connection reset" in e.message.orEmpty())
        assertEquals(2_100, bytesIfExists(partial("voice.onnx"))?.size)
        server.dropAfter = null
        pull()
        assertEquals("bytes=2100-", server.range(1))
        assertContentEquals(voice, bytesIfExists(file("voice.onnx")))
        assertNull(entryKind(partial("voice.onnx")))
    }

    @Test fun aServerThatIgnoresTheRangeRestartsTheFile() {
        write(partial("voice.onnx"), voice.copyOf(1_000))
        server.ignoreRange = true
        pull()
        assertEquals("bytes=1000-", server.range(0))
        assertContentEquals(voice, bytesIfExists(file("voice.onnx")))
    }

    @Test fun theWrongRangeIsRefusedAndThePartialDropped() {
        write(partial("voice.onnx"), voice.copyOf(1_000))
        server.claimedStart = 0
        assertFailsWith<ModelException.BadRange> { pull() }
        nothingKept("voice.onnx")
    }

    @Test fun aCorruptPartialFailsTheHashOnceAndThenDownloadsClean() {
        write(partial("voice.onnx"), content(1_000, seed = 7))
        assertFailsWith<ModelException.HashMismatch> { pull() }
        nothingKept("voice.onnx")
        pull()
        assertContentEquals(voice, bytesIfExists(file("voice.onnx")))
    }

    @Test fun aPartialLongerThanThePinIsDiscarded() {
        write(partial("voice.onnx"), ByteArray(voice.size + 1))
        pull()
        assertNull(server.range(0))
        assertContentEquals(voice, bytesIfExists(file("voice.onnx")))
    }

    @Test fun aCompletePartialIsVerifiedWithoutARequest() {
        write(partial("voice.onnx"), voice)
        pull()
        assertTrue(server.requests.none { it.url.endsWith("voice.onnx") })
        assertContentEquals(voice, bytesIfExists(file("voice.onnx")))
    }

    @Test fun aShortBodyIsTruncatedAndKeptForResume() {
        server.announce = 100
        server.files[BASE + "voice.onnx"] = voice.copyOf(100)
        val e = assertFailsWith<ModelException.Truncated> { pull() }
        assertEquals(100, e.got)
        assertEquals(100, bytesIfExists(partial("voice.onnx"))?.size)
    }

    @Test fun aDamagedInstalledFileIsNotCountedAndIsFetchedAgain() {
        pull()
        write(file("tokens.txt"), tokens.copyOf(10))
        assertEquals(2, manager.list().single().filesPresent)
        assertNull(manager.path("voice-1"))
        pull()
        assertContentEquals(tokens, bytesIfExists(file("tokens.txt")))
    }

    @Test fun removeDeletesTheModelWithItsPartials() {
        server.dropAfter = 500
        assertFailsWith<ModelException.Network> { pull() }
        assertNotNull(entryKind(partial("voice.onnx")))
        assertTrue(manager.remove("voice-1"))
        assertNull(entryKind(dir))
        assertFalse(manager.remove("voice-1"))
        assertFalse(manager.remove("never-pulled"))
    }

    @Test fun removeReachesModelsTheCatalogNoLongerLists() {
        write(joinPath(home.path, "models", "old-model", "x.bin"), ByteArray(3))
        assertTrue(manager.remove("old-model"))
    }

    @Test fun unknownAndUnsafeIdsAreRefused() {
        assertEquals(
            "unknown model `nope`",
            assertFailsWith<ModelException.UnknownModel> { runBlocking { manager.pull("nope", server) } }.message,
        )
        for (id in listOf("../escape", "a/b", "", "UP", "x".repeat(100))) {
            val pull = assertFailsWith<ModelException.UnsafeName> { runBlocking { manager.pull(id, server) } }
            assertFalse(id.isNotEmpty() && id in pull.message.orEmpty(), "the id was echoed")
            assertFailsWith<ModelException.UnsafeName> { manager.remove(id) }
        }
        assertEquals(0, server.requests.size)
    }

    @Test fun aSecondWriterOfTheSameModelIsTurnedAway() {
        val models = joinPath(home.path, "models")
        createDirAll(models)
        FileLock.tryExclusive(joinPath(models, "voice-1.lock").toString())!!.use {
            assertFailsWith<ModelException.Busy> { pull() }
            assertFailsWith<ModelException.Busy> { manager.remove("voice-1") }
        }
        pull()
    }

    @Test fun theManagerRefusesAnInvalidCatalog() {
        val bad = Catalog(listOf(model("BAD", "a" to ByteArray(1))))
        assertFailsWith<ModelException.BadCatalog> { ModelManager(home.path, bad) }
    }
}
