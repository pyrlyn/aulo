import aulo.buildlogic.ModuleRole

// Which speech engine runs (spec §4.2, §5.1): the registry of engine factories, the candidates
// a chat, bot and config pick, and the engine in use with fallback and mid-chat switching.
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
        }
        commonTest.dependencies {
            implementation(project(":aulo-testkit"))
        }
    }
}
