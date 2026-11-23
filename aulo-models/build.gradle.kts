import aulo.buildlogic.EmbedFilesTask
import aulo.buildlogic.ModuleRole

// Speech models (spec §4.2): the pinned catalog and the manager that downloads, verifies, lists and
// removes them under $AULO_HOME/models. It speaks the kit's HTTP interface; the composition root
// picks the engine.
plugins {
    id("aulo.kmp")
    id("org.jetbrains.kotlin.plugin.serialization")
}

aulo {
    role = ModuleRole.LIBRARY
}

// data/models.json ships inside the binary, so a pull never trusts a catalog fetched at run time.
val embedCatalog =
    tasks.register<EmbedFilesTask>("embedCatalog") {
        source.set(layout.projectDirectory.dir("data"))
        packageName.set("aulo.models")
        valName.set("CATALOG_FILES")
        doc.set("The files under `aulo-models/data/`, by name.")
        outputDir.set(layout.buildDirectory.dir("generated/catalog/kotlin"))
    }

kotlin {
    sourceSets {
        nativeMain {
            kotlin.srcDir(embedCatalog)
        }
        nativeMain.dependencies {
            api("pyrlyn.kit:http")
            implementation("pyrlyn.kit:fs")
            implementation(libs.kotlinx.serialization.json)
            implementation(libs.okio)
        }
        nativeTest.dependencies {
            implementation(libs.kotlinx.coroutines.core)
        }
    }
}
