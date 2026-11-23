import aulo.buildlogic.ModuleRole

// Speech-to-text over an OpenAI-compatible `/v1/audio/transcriptions` endpoint (T7.6): OpenAI
// itself, a local server, or any compatible one. It speaks the kit's HTTP interface; the
// composition root picks the transport and the secret store.
plugins {
    id("aulo.kmp")
}

aulo {
    role = ModuleRole.LIBRARY
}

kotlin {
    sourceSets {
        nativeMain.dependencies {
            // The engine registers in aulo-voice's registry, part of registerOpenAi's signature.
            api(project(":aulo-voice"))
            api(project(":aulo-secrets"))
            api("pyrlyn.kit:http")
            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.kotlinx.serialization.json)
        }
    }
}
