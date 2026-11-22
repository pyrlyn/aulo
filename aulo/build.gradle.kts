// The surface module: links the `aulo` CLI and the `aulod` daemon executables.
plugins {
    id("aulo.kmp")
}

val gitSha: Provider<String> =
    providers
        .exec {
            commandLine("git", "rev-parse", "--short=12", "HEAD")
            // A source tarball has no .git; the build must still work there.
            isIgnoreExitValue = true
        }.standardOutput.asText
        .map { it.trim().ifEmpty { "unknown" } }

val buildInfoDir = layout.buildDirectory.dir("generated/buildInfo/kotlin")

val generateBuildInfo =
    tasks.register("generateBuildInfo") {
        // Locals, not script properties: the configuration cache cannot serialize the script object.
        val version = project.version.toString()
        val license = providers.gradleProperty("aulo.license")
        val sha = gitSha
        val outDir = buildInfoDir
        inputs.property("version", version)
        inputs.property("license", license)
        inputs.property("gitSha", sha)
        outputs.dir(outDir)
        doLast {
            val file = outDir.get().file("aulo/app/BuildInfo.kt").asFile
            file.parentFile.mkdirs()
            file.writeText(
                """
            |package aulo.app
            |
            |internal object BuildInfo {
            |    const val VERSION: String = "$version"
            |    const val GIT_SHA: String = "${sha.get()}"
            |    const val LICENSE: String = "${license.get()}"
            |}
            |
                """.trimMargin(),
            )
        }
    }

kotlin {
    targets.withType<org.jetbrains.kotlin.gradle.plugin.mpp.KotlinNativeTarget>().configureEach {
        binaries {
            executable("aulo") {
                baseName = "aulo"
                entryPoint = "aulo.app.cli.main"
            }
            executable("aulod") {
                baseName = "aulod"
                entryPoint = "aulo.app.daemon.main"
            }
        }
    }

    sourceSets {
        commonMain {
            kotlin.srcDir(generateBuildInfo)
        }
    }
}
