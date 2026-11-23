import aulo.buildlogic.ModuleRole

// Ids and aulo's own events (spec §5): the contract every other module may depend on.
plugins {
    id("aulo.kmp")
    id("org.jetbrains.kotlin.plugin.serialization")
}

aulo {
    role = ModuleRole.CONTRACT
}

kotlin {
    sourceSets {
        commonMain.dependencies {
            implementation(libs.kotlinx.serialization.core)
        }
        commonTest.dependencies {
            implementation(libs.kotlinx.serialization.json)
        }
    }
}
