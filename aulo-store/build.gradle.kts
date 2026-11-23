import aulo.buildlogic.EmbedFilesTask
import aulo.buildlogic.ModuleRole

// SQLite storage of one aulo home (`aulo.db`, spec §14): bots, chats, messages, tool calls and
// usage. The only module that touches SQLite (checkModuleGraph enforces it).
plugins {
    id("aulo.kmp")
    // Typed queries, checked against the schema at build time.
    id("app.cash.sqldelight")
}

aulo {
    role = ModuleRole.LIBRARY
}

// Filename-keyed migrations: migrations/<key>/up.sql and down.sql. A key is a directory name, so
// two branches that each add one never collide the way two numbered versions would.
val embedMigrations =
    tasks.register<EmbedFilesTask>("embedMigrations") {
        source.set(layout.projectDirectory.dir("migrations"))
        packageName.set("aulo.store")
        valName.set("MIGRATION_FILES")
        doc.set("The files under `aulo-store/migrations/`, by `<key>/up.sql` and `<key>/down.sql`.")
        outputDir.set(layout.buildDirectory.dir("generated/migrations/kotlin"))
    }

kotlin {
    sourceSets {
        commonMain.dependencies {
            implementation("pyrlyn.kit:db-sqldelight")
        }
        nativeMain {
            kotlin.srcDir(embedMigrations)
        }
        nativeMain.dependencies {
            implementation(project(":aulo-types"))
            implementation("pyrlyn.kit:fs")
        }
        // SQLite itself: androidx.sqlite's bundled build on macOS and Linux; it has no Windows
        // build, so Windows links SQLite's own C API, built by Conan.
        unixMain.dependencies { implementation("pyrlyn.kit:db-room") }
        mingwMain.dependencies { implementation("pyrlyn.kit:db-native") }
    }
}

sqldelight {
    // SQLite comes from db-room or db-native, never the system's: Windows has no libsqlite3.
    linkSqlite.set(false)
    databases {
        create("AuloDatabase") {
            packageName.set("aulo.store.db")
            srcDirs.setFrom("src/commonMain/sqldelight")
            dialect("app.cash.sqldelight:sqlite-3-38-dialect:${libs.versions.sqldelight.get()}")
        }
        create("LedgerDatabase") {
            packageName.set("aulo.store.ledger")
            srcDirs.setFrom("src/commonMain/ledger")
            dialect("app.cash.sqldelight:sqlite-3-38-dialect:${libs.versions.sqldelight.get()}")
        }
    }
}
