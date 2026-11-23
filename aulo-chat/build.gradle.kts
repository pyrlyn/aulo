plugins {
    alias(libs.plugins.kotlin.multiplatform) apply false
    alias(libs.plugins.kotlin.jvm) apply false
    alias(libs.plugins.kotlin.compose) apply false
    alias(libs.plugins.compose) apply false
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.android.kmp.library) apply false
    alias(libs.plugins.kotlinx.rpc) apply false
    alias(libs.plugins.sqldelight) apply false
}

// kotlinx-rpc 0.11.0-grpc-190 ships its compiler plugin up to Kotlin 2.4.20, and 2.4.21 is a
// patch release with the same compiler API; drop this once a gRPC build for our Kotlin is
// published.
subprojects {
    configurations.configureEach {
        if (name.contains("CompilerPluginClasspath", ignoreCase = true)) {
            resolutionStrategy.eachDependency {
                if (requested.group == "org.jetbrains.kotlinx" && requested.name.startsWith("kotlinx-rpc-compiler-plugin")) {
                    useVersion("2.4.20-${rootProject.libs.versions.kotlinx.rpc.get()}")
                }
            }
        }
    }
}
