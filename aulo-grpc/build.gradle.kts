import aulo.buildlogic.ModuleRole

// gRPC over HTTP/2 on every target (spec §11): the one module that links nghttp2. It knows no aulo
// service; aulo-server binds the generated interfaces of aulo-proto to it.
plugins {
    id("aulo.kmp")
    id("aulo.conan")
}

aulo {
    role = ModuleRole.LIBRARY
}

conan {
    cinterop("nghttp2", pkg = "libnghttp2", "libnghttp2.a")
    // TLS for TCP listeners and clients (T3.8); the shim in mbedtls.def is the only caller.
    cinterop("mbedtls", pkg = "mbedtls", "libmbedtls.a", "libmbedx509.a", "libmbedcrypto.a")
}

kotlin {
    sourceSets {
        commonMain.dependencies {
            // Handlers and calls are coroutines; ktor-network gives TCP and Unix sockets on every target.
            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.ktor.network)
        }
    }
}
