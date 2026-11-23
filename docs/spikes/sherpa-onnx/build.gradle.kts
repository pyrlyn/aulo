import aulo.buildlogic.ModuleRole

plugins {
    id("aulo.kmp")
    id("aulo.conan")
}

aulo {
    role = ModuleRole.SURFACE
}

conan {
    cinterop("sherpa", pkg = "sherpa-onnx")
}

kotlin {
    sourceSets {
        nativeMain.dependencies { implementation("pyrlyn.kit:fs") }
    }
    targets.withType<org.jetbrains.kotlin.gradle.plugin.mpp.KotlinNativeTarget>().configureEach {
        binaries {
            executable("spike") {
                baseName = "sherpa-spike"
                entryPoint = "spike.main"
            }
        }
    }
}
