package aulo.models

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class CatalogTest {
    private val sha = "a".repeat(64)

    private fun file(path: String) = ModelFile(path, 1, sha)

    private fun entry(
        id: String = "m1",
        baseUrl: String = BASE,
        files: List<ModelFile> = listOf(file("a.onnx")),
    ) = Model(id, ModelKind.TTS, "M", "MIT", baseUrl, files)

    private fun rejects(
        vararg models: Model,
        reason: String,
    ) {
        val e = assertFailsWith<ModelException.BadCatalog> { Catalog(models.toList()).validate() }
        assertTrue(reason in e.message.orEmpty(), e.message)
    }

    @Test fun theEmbeddedCatalogIsValidAndPinnedToCommits() {
        val catalog = Catalog.embedded()
        assertTrue(catalog.models.isNotEmpty())
        val commit = Regex("^https://huggingface\\.co/[^/]+/[^/]+/resolve/[0-9a-f]{40}/$")
        for (model in catalog.models) {
            assertTrue(commit.matches(model.baseUrl), "${model.id} is not pinned to a commit: ${model.baseUrl}")
            assertTrue(model.license.isNotBlank())
        }
        val parakeet = assertNotNull(catalog["parakeet-tdt-0.6b-v3-int8"])
        assertEquals(ModelKind.STT, parakeet.kind)
        assertEquals(
            setOf("decoder.int8.onnx", "encoder.int8.onnx", "joiner.int8.onnx", "tokens.txt"),
            parakeet.files.map { it.path }.toSet(),
        )
        assertNull(catalog["nope"])
    }

    @Test fun parsingNamesWhatIsWrong() {
        assertFailsWith<ModelException.BadCatalog> { Catalog.parse("{") }
        assertFailsWith<ModelException.BadCatalog> { Catalog.parse("""{"models":[],"extra":1}""") }
        assertFailsWith<ModelException.BadCatalog> {
            Catalog.parse("""{"models":[{"id":"m","kind":"music","name":"n","license":"MIT","base_url":"$BASE","files":[]}]}""")
        }
    }

    @Test fun idsMustBePlainDirectoryNames() {
        for (id in listOf("", "Upper", "-dash", ".hidden", "a/b", "a\\b", "..", "x".repeat(65))) {
            rejects(entry(id = id), reason = "safe directory name")
        }
        rejects(entry(), entry(), reason = "duplicate model id")
    }

    @Test fun filePathsStayInsideTheModelDirectory() {
        for (path in listOf("", "/abs", "../up", "a/../b", "a//b", "./a", "c:x", "a\\b", "a.part", "a/")) {
            rejects(entry(files = listOf(file(path))), reason = "unsafe file path")
        }
        rejects(entry(files = listOf(file("a"), file("a"))), reason = "duplicate file")
        rejects(entry(files = listOf(file("a"), file("a/b"))), reason = "both a file and a directory")
        Catalog(listOf(entry(files = listOf(file("dir/a.onnx"), file("dir/b.txt"))))).validate()
    }

    @Test fun everyFileNeedsASizeAndALowercaseHash() {
        rejects(entry(files = listOf(ModelFile("a", 0, sha))), reason = "needs a size")
        rejects(entry(files = listOf(ModelFile("a", 1, "A".repeat(64)))), reason = "lowercase SHA-256")
        rejects(entry(files = listOf(ModelFile("a", 1, "a".repeat(63)))), reason = "lowercase SHA-256")
        rejects(entry(files = emptyList()), reason = "no files")
    }

    @Test fun theBaseUrlIsAnHttpDirectory() {
        rejects(entry(baseUrl = "https://h/x"), reason = "base_url")
        rejects(entry(baseUrl = "file:///etc/"), reason = "base_url")
    }
}
