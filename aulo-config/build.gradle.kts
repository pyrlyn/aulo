import aulo.buildlogic.EmbedFilesTask
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

// config/default.toml is documentation users copy from, and the binary prints it (DEFAULT_TOML).
val embedConfigFiles =
    tasks.register<EmbedFilesTask>("embedConfigFiles") {
        source.set(layout.projectDirectory.dir("config"))
        packageName.set("aulo.config")
        valName.set("CONFIG_FILES")
        doc.set("The files under `aulo-config/config/`, by name.")
        outputDir.set(layout.buildDirectory.dir("generated/configFiles/kotlin"))
    }

kotlin {
    sourceSets {
        nativeMain {
            kotlin.srcDir(embedConfigFiles)
        }
        nativeMain.dependencies {
            implementation(libs.kotlinx.serialization.json)
            // Config exposes LogLevel.
            api(project(":aulo-telemetry"))
            // EngineConfig.choice returns the speech contract's EngineChoice.
            api(project(":aulo-speech"))
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
