import aulo.buildlogic.ModuleRole

// What every speech engine implements (spec §4.2, §5.1): STT, TTS, VAD, wake word and end of
// turn. Built-in engines, cloud clients and plugin proxies all fit the same interfaces.
plugins {
    id("aulo.kmp")
}

aulo {
    role = ModuleRole.CONTRACT
}

kotlin {
    sourceSets {
        commonMain.dependencies {
            // TurnId and TranscriptKind appear in the engine interfaces.
            api(project(":aulo-types"))
        }
        commonTest.dependencies {
            implementation(project(":aulo-testkit"))
        }
    }
}
