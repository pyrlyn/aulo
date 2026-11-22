package aulo.app.cli

import aulo.app.versionText
import aulo.config.DEFAULT_TOML
import cli.Settings
import fs.TempDir
import fs.WriteOptions
import fs.atomic
import fs.bytesIfExists
import fs.createDirAll
import fs.listDir
import fs.readText
import fs.removeFile
import kotlinx.io.files.Path
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonPrimitive
import process.envVars
import toml.TomlParser
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Runs every fixture in `aulo/tests/cmd` (see its README) and the cases a fixture cannot express. */
class CmdTest {
    private val hostEnv = envVars().toMap()
    private val fixtures = Path(hostEnv["AULO_CMD_FIXTURES"] ?: error("AULO_CMD_FIXTURES is set by the Gradle test task"))
    private val update = hostEnv["AULO_UPDATE_FIXTURES"] == "1"
    private val settings = Settings("aulo")

    @Test
    fun everyFixturePasses() {
        val cases = listDir(fixtures).filter { it.endsWith(".toml") }.sorted()
        assertTrue(cases.size >= 4, "fixtures not found in $fixtures")
        val failures = cases.mapNotNull { runCase(it.removeSuffix(".toml")) }
        assertEquals(emptyList(), failures)
    }

    private fun runCase(name: String): String? {
        val case = TomlParser.parse(readText(Path(fixtures, "$name.toml")))
        TempDir.create(".aulo-cmd-test").use { tmp ->
            val home = Path(tmp.path, "home")
            val project = Path(tmp.path, "project")
            createDirAll(home)
            createDirAll(project)
            writeFiles(case["home"] as JsonObject?, home)
            writeFiles(case["project"] as JsonObject?, project)
            val env =
                (case["env"] as JsonObject?).orEmpty().mapValues { it.value.jsonPrimitive.content } +
                    ("AULO_HOME" to home.toString())
            val args = case["args"]?.jsonArray?.map { it.jsonPrimitive.content }.orEmpty()
            val result = runCli(args, env, project.toString(), settings)
            val portable = { text: String ->
                text
                    .replace(home.toString(), "[HOME]")
                    .replace(project.toString(), "[PROJECT]")
                    .replace('\\', '/')
            }
            val stdout = if (result.stderr) "" else portable(result.text)
            val stderr = if (result.stderr) portable(result.text) else ""
            val status = case["status"]?.jsonPrimitive?.int ?: 0
            if (update) {
                save(Path(fixtures, "$name.stdout"), stdout)
                save(Path(fixtures, "$name.stderr"), stderr)
                return null
            }
            return listOfNotNull(
                "status ${result.code}, expected $status".takeIf { result.code != status },
                "stdout differs".takeIf { stdout != expected("$name.stdout") },
                "stderr differs".takeIf { stderr != expected("$name.stderr") },
            ).takeIf { it.isNotEmpty() }?.joinToString(", ", "$name: ")
        }
    }

    private fun writeFiles(
        files: JsonObject?,
        dir: Path,
    ) {
        val options = WriteOptions(createParent = true)
        files?.forEach { (file, text) ->
            options.write(Path(dir, file), text.jsonPrimitive.content.encodeToByteArray())
        }
    }

    private fun expected(file: String) = bytesIfExists(Path(fixtures, file))?.decodeToString().orEmpty()

    private fun save(
        path: Path,
        text: String,
    ) {
        if (text.isEmpty()) {
            if (bytesIfExists(path) != null) removeFile(path)
        } else {
            atomic(path, text.encodeToByteArray())
        }
    }

    @Test
    fun versionMatchesTheVersionText() {
        val result = runCli(listOf("--version"), emptyMap(), null, settings)
        assertEquals(versionText("aulo") + "\n", result.text)
        assertEquals(0, result.code)
    }

    @Test
    fun configDefaultPrintsTheEmbeddedFile() {
        val result = runCli(listOf("config", "default"), emptyMap(), null, settings)
        assertEquals(DEFAULT_TOML, result.text)
        assertEquals(0, result.code)
    }
}
