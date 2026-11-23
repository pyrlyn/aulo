import aulo.buildlogic.ModuleRole

// Fake engines and fixtures that tests across modules share. Only test source sets depend on it.
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
    }
}
