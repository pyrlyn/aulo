import aulo.buildlogic.ModuleRole
import org.jetbrains.kotlin.gradle.targets.native.tasks.KotlinNativeTest

// Typed config, its layers and provenance, and the committed JSON Schema (spec §14). The only
// module that reads or writes aulo's TOML config.
plugins {
    id("aulo.kmp")
    id("org.jetbrains.kotlin.plugin.serialization")
}

aulo {
    role = ModuleRole.LIBRARY
}

// config/default.toml is documentation users copy from, and the binary prints it; one file
// serves both, embedded as a Kotlin string because Kotlin/Native has no resources.
val defaultToml = layout.projectDirectory.file("config/default.toml")
val embedDefaultToml =
    tasks.register("embedDefaultToml") {
        val source = defaultToml
        val outDir = layout.buildDirectory.dir("generated/defaultToml/kotlin")
        inputs.file(source)
        outputs.dir(outDir)
        doLast {
            val text = source.asFile.readText()
            check("\"\"\"" !in text) { "config/default.toml must not contain three double quotes" }
            val file = outDir.get().file("aulo/config/DefaultToml.kt").asFile
            file.parentFile.mkdirs()
            file.writeText(
                "package aulo.config\n\n" +
                    "/** `config/default.toml`: every key with its default, all commented out. */\n" +
                    "val DEFAULT_TOML: String = \"\"\"" + text.replace("$", "\${'$'}") + "\"\"\"\n",
            )
        }
    }

kotlin {
    sourceSets {
        nativeMain {
            kotlin.srcDir(embedDefaultToml)
        }
        nativeMain.dependencies {
            implementation(libs.kotlinx.serialization.json)
            // Config exposes LogLevel.
            api(project(":aulo-telemetry"))
            implementation("pyrlyn.kit:toml")
            implementation("pyrlyn.kit:fs")
        }
    }
}

// The stale-schema test compares the generator's output with the committed file;
// AULO_UPDATE_SCHEMA=1 makes it rewrite the file instead (`just schema`).
val schemaFile = rootProject.layout.projectDirectory.file("schemas/aulo.schema.json")
val updateSchema = providers.environmentVariable("AULO_UPDATE_SCHEMA").orElse("")
tasks.withType<KotlinNativeTest>().configureEach {
    inputs.file(schemaFile)
    inputs.property("updateSchema", updateSchema)
    environment("AULO_SCHEMA_FILE", schemaFile.asFile.path)
    environment("AULO_UPDATE_SCHEMA", updateSchema.get())
}
