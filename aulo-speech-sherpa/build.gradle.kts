import aulo.buildlogic.ModuleRole

// The sherpa-onnx helper (spec §5.1): the `aulo-speech-sherpa` executable, the one module that
// links sherpa-onnx. aulod runs it as a child process and talks to it through :aulo-speech-helper,
// so a crash in native inference code is an engine failure, not the end of the daemon.
plugins {
    id("aulo.kmp")
    id("aulo.conan")
}

aulo {
    role = ModuleRole.SURFACE
}

conan {
    // Static libraries on macOS, shared ones on Linux and Windows: sherpa.def names them per platform.
    cinterop("sherpa", pkg = "sherpa-onnx")
    bundleSharedLibraries("sherpa-onnx")
}

kotlin {
    sourceSets {
        nativeMain.dependencies {
            implementation(project(":aulo-speech-helper"))
            // kotlinx-io paths, to check the model files before sherpa-onnx sees them.
            implementation("pyrlyn.kit:fs")
        }
        nativeTest.dependencies {
            implementation(libs.kotlinx.coroutines.core)
        }
    }
    targets.withType<org.jetbrains.kotlin.gradle.plugin.mpp.KotlinNativeTarget>().configureEach {
        binaries {
            executable("helper") {
                baseName = "aulo-speech-sherpa"
                entryPoint = "aulo.speech.sherpa.main"
            }
        }
    }
}

// HelperProcessTest starts the linked helper of the target under test, as aulod would.
kotlin.targets.withType<org.jetbrains.kotlin.gradle.plugin.mpp.KotlinNativeTarget>().configureEach {
    val helper = binaries.getExecutable("helper", org.jetbrains.kotlin.gradle.plugin.mpp.NativeBuildType.DEBUG)
    val testName = "${name}Test"
    tasks.withType<org.jetbrains.kotlin.gradle.targets.native.tasks.KotlinNativeTest>().matching { it.name == testName }.configureEach {
        dependsOn(helper.linkTaskProvider)
        environment("AULO_SHERPA_HELPER", helper.outputFile.path)
    }
}
