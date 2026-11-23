import aulo.buildlogic.ModuleRole

// The surface module: links the `aulo` CLI and the `aulod` daemon executables.
plugins {
    id("aulo.kmp")
}

aulo {
    role = ModuleRole.SURFACE
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
    // aulod's SIGINT and SIGTERM handler; Windows has console control handlers instead.
    targets
        .withType<org.jetbrains.kotlin.gradle.plugin.mpp.KotlinNativeTarget>()
        .matching { it.konanTarget.family != org.jetbrains.kotlin.konan.target.Family.MINGW }
        .configureEach { compilations.getByName("main").cinterops.create("signals") }
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
        nativeMain.dependencies {
            implementation(project(":aulo-config"))
            implementation(project(":aulo-secrets"))
            implementation(project(":aulo-server"))
            implementation(project(":aulo-store"))
            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.kotlinx.serialization.json)
            implementation("pyrlyn.kit:cli")
            implementation("pyrlyn.kit:fs")
            implementation("pyrlyn.kit:process")
        }
        nativeTest.dependencies {
            implementation("pyrlyn.kit:toml")
        }
    }
}

// AulodProcessTest runs the linked aulod of the target under test.
kotlin.targets.withType<org.jetbrains.kotlin.gradle.plugin.mpp.KotlinNativeTarget>().configureEach {
    val aulod = binaries.getExecutable("aulod", org.jetbrains.kotlin.gradle.plugin.mpp.NativeBuildType.DEBUG)
    val testName = "${name}Test"
    tasks.withType<org.jetbrains.kotlin.gradle.targets.native.tasks.KotlinNativeTest>().matching { it.name == testName }.configureEach {
        dependsOn(aulod.linkTaskProvider)
        environment("AULOD_BIN", aulod.outputFile.path)
    }
}

// CmdTest runs the command fixtures in tests/cmd; AULO_UPDATE_FIXTURES=1 rewrites their expected output.
val cmdFixtures = layout.projectDirectory.dir("tests/cmd")
val updateFixtures = providers.environmentVariable("AULO_UPDATE_FIXTURES").orElse("")
tasks.withType<org.jetbrains.kotlin.gradle.targets.native.tasks.KotlinNativeTest>().configureEach {
    inputs.dir(cmdFixtures)
    inputs.property("updateFixtures", updateFixtures)
    environment("AULO_CMD_FIXTURES", cmdFixtures.asFile.path)
    environment("AULO_UPDATE_FIXTURES", updateFixtures.get())
}
