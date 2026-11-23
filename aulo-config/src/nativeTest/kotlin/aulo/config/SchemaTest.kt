package aulo.config

import fs.atomic
import fs.readText
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.toKString
import kotlinx.io.files.Path
import platform.posix.getenv
import kotlin.test.Test
import kotlin.test.assertEquals

class SchemaTest {
    @OptIn(ExperimentalForeignApi::class)
    private fun env(name: String) = getenv(name)?.toKString().orEmpty()

    @Test
    fun committedSchemaMatchesTheTypes() {
        val file = Path(env("AULO_SCHEMA_FILE").ifEmpty { error("AULO_SCHEMA_FILE is set by the Gradle test task") })
        val generated = ConfigSchema.generate()
        if (env("AULO_UPDATE_SCHEMA") == "1") {
            atomic(file, generated.encodeToByteArray())
            return
        }
        assertEquals(readText(file), generated, "schemas/aulo.schema.json is stale: run `just schema`")
    }
}
