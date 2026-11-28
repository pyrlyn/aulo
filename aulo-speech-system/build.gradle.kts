import aulo.buildlogic.ModuleRole
import org.jetbrains.kotlin.gradle.plugin.mpp.KotlinNativeTarget
import org.jetbrains.kotlin.gradle.plugin.mpp.NativeBuildType
import org.jetbrains.kotlin.gradle.targets.native.tasks.KotlinNativeTest

// The system-voices helper (spec §5.1): the `aulo-speech-system` executable, which speaks with the
// operating system's own voices. aulod runs it as a child process and talks to it through
// :aulo-speech-helper. On macOS it drives AVSpeechSynthesizer, which delivers audio only through
// the main run loop, something aulod's main thread cannot give it. Other systems have no executable
// yet, so the engine is unavailable there and the registry falls back.
plugins {
    id("aulo.kmp")
}

aulo {
    role = ModuleRole.SURFACE
}

kotlin {
    sourceSets {
        commonMain.dependencies {
            implementation(project(":aulo-speech-helper"))
        }
        macosMain.dependencies {
            // The protocol runs on an I/O thread while the main thread runs the run loop.
            implementation(libs.kotlinx.coroutines.core)
        }
        macosTest.dependencies {
            implementation(libs.kotlinx.coroutines.core)
        }
    }
    targets.withType<KotlinNativeTarget>().matching { it.name == "macosArm64" }.configureEach {
        binaries {
            executable("helper") {
                baseName = "aulo-speech-system"
                entryPoint = "aulo.speech.system.main"
            }
        }
    }
}

// SystemHelperTest starts the linked helper, as aulod would.
kotlin.targets.withType<KotlinNativeTarget>().matching { it.name == "macosArm64" }.configureEach {
    val helper = binaries.getExecutable("helper", NativeBuildType.DEBUG)
    tasks.withType<KotlinNativeTest>().matching { it.name == "macosArm64Test" }.configureEach {
        dependsOn(helper.linkTaskProvider)
        environment("AULO_SYSTEM_HELPER", helper.outputFile.path)
    }
}
