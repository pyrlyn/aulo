import aulo.buildlogic.ModuleRole

// Logs and secret masking (spec §4: aulo-telemetry owns logs, traces and redaction). Every
// place that shows untrusted or configured text to a person or a file masks it through here.
plugins {
    id("aulo.kmp")
    id("org.jetbrains.kotlin.plugin.serialization")
}

aulo {
    role = ModuleRole.LIBRARY
}

kotlin {
    sourceSets {
        nativeMain.dependencies {
            api(libs.kotlinx.serialization.json)
            implementation("pyrlyn.kit:fs")
        }
    }
}
