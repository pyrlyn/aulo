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
        // androidx.sqlite (pyrlyn-kit's db-room) is published only to Google's repository; nothing
        // else is taken from it.
        google {
            content { includeGroupByRegex("androidx\\..*") }
        }
    }
}

rootProject.name = "aulo"

// Shared Kotlin/Native libraries (TOML, files, SQLite) from the workspace's packages/: the first
// ancestor with packages/pyrlyn-kit, so the main checkout and nested worktrees both work, or
// KOTLIN_KIT_DIR. CI checks out pyrlyn/packages at a pinned tag and sets the variable.
val pyrlynKitDir: String =
    System.getenv("KOTLIN_KIT_DIR")?.let { settingsDir.resolve(it).path }
        ?: generateSequence(settingsDir.parentFile) { it.parentFile }
            .map { it.resolve("packages/pyrlyn-kit") }
            .firstOrNull { it.isDirectory }
            ?.path
        ?: error("packages/pyrlyn-kit not found above $settingsDir; set KOTLIN_KIT_DIR")
includeBuild(pyrlynKitDir)

// Modules are named aulo-<role> (AGENTS.md, Layout); S2+ tasks add them here.
include(
    ":aulo",
    ":aulo-types",
    ":aulo-telemetry",
    ":aulo-config",
    ":aulo-store",
    ":aulo-proto",
    ":aulo-grpc",
    ":aulo-secrets",
    ":aulo-server",
    ":aulo-models",
    ":aulo-speech",
    ":aulo-speech-helper",
    ":aulo-speech-openai",
    ":aulo-speech-sherpa",
    ":aulo-testkit",
    ":aulo-voice",
)
