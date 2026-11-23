import aulo.buildlogic.ModuleRole
import org.jetbrains.kotlin.gradle.plugin.mpp.KotlinNativeTarget
import org.jetbrains.kotlin.konan.target.Family

// Secrets kept for the user, out of reach of other users and of the model: the OS keychain
// (Keychain, Credential Manager, the Secret Service), a 0600 file where there is none, and the
// owner-only directories both the files and aulod's local socket live in.
plugins {
    id("aulo.kmp")
}

aulo {
    role = ModuleRole.LIBRARY
}

kotlin {
    // libsecret is LGPL and often absent on servers, so it is never linked: a small C shim
    // dlopens the system copy at run time and declares the few types it needs itself.
    targets.withType<KotlinNativeTarget>().matching { it.konanTarget.family == Family.LINUX }.configureEach {
        compilations.getByName("main").cinterops.create("libsecret")
    }
    targets.withType<KotlinNativeTarget>().matching { it.konanTarget.family == Family.MINGW }.configureEach {
        compilations.getByName("main").cinterops.create("wincred")
    }
    sourceSets {
        nativeMain.dependencies {
            // Atomic 0600 writes, modes and entry kinds.
            implementation("pyrlyn.kit:fs")
        }
    }
}
