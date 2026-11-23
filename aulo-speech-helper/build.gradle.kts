import aulo.buildlogic.ModuleRole

// Speech engines whose native code runs in a helper process (spec §5.2, fail open): the pipe
// protocol both sides speak and the aulod side of the engine. A crash or hang in the helper is an
// engine failure the registry falls back from. The sherpa-onnx helper is :aulo-speech-sherpa.
plugins {
    id("aulo.kmp")
}

aulo {
    role = ModuleRole.LIBRARY
}

kotlin {
    sourceSets {
        commonMain.dependencies {
            api(project(":aulo-speech"))
            // The pipes are okio streams on both sides; frames are read and written with it.
            api(libs.okio)
            implementation(libs.kotlinx.coroutines.core)
        }
        nativeMain.dependencies {
            // The engines register in aulo-voice's registry (part of registerSherpa's signature) and start their
            // helper with pyrlyn-kit's process pipes.
            api(project(":aulo-voice"))
            implementation("pyrlyn.kit:process")
        }
        commonTest.dependencies {
            implementation(project(":aulo-testkit"))
        }
    }
}
