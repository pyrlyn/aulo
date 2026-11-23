pluginManagement {
    includeBuild("build-logic")
    repositories {
        gradlePluginPortal()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositoriesMode = RepositoriesMode.FAIL_ON_PROJECT_REPOS
    repositories {
        mavenCentral()
        // androidx.sqlite (kotlin-kit's db-room) is published only to Google's repository; nothing
        // else is taken from it.
        google {
            content { includeGroupByRegex("androidx\\..*") }
        }
    }
}

rootProject.name = "aulo"

// Shared Kotlin/Native libraries (TOML, files, SQLite) from the workspace's packages/: the first
// ancestor with packages/kotlin-kit, so the main checkout and nested worktrees both work, or
// KOTLIN_KIT_DIR. CI checks out pyrlyn/packages at a pinned tag and sets the variable.
val kotlinKitDir: String =
    System.getenv("KOTLIN_KIT_DIR")?.let { settingsDir.resolve(it).path }
        ?: generateSequence(settingsDir.parentFile) { it.parentFile }
            .map { it.resolve("packages/kotlin-kit") }
            .firstOrNull { it.isDirectory }
            ?.path
        ?: error("packages/kotlin-kit not found above $settingsDir; set KOTLIN_KIT_DIR")
includeBuild(kotlinKitDir)

// Modules are named aulo-<role> (AGENTS.md, Layout); S2+ tasks add them here.
include(":aulo", ":aulo-types", ":aulo-speech", ":aulo-telemetry", ":aulo-config", ":aulo-store", ":aulo-proto", ":aulo-grpc", ":aulo-server")
