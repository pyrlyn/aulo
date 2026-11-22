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

kotlin {
    sourceSets {
        nativeMain.dependencies {
            implementation(libs.kotlinx.serialization.json)
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
