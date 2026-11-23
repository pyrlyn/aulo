// kotlinx-rpc publishes its gRPC builds and their Gradle plugin to a JetBrains repository of
// their own; only those modules are looked up there.
val kxrpcGrpc = "https://redirector.kotlinlang.org/maven/kxrpc-grpc"

pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
        google()
        maven("https://redirector.kotlinlang.org/maven/kxrpc-grpc") {
            content {
                includeGroup("org.jetbrains.kotlinx.rpc.plugin")
                includeModuleByRegex("org\\.jetbrains\\.kotlinx", "kotlinx-rpc-.*")
            }
        }
    }
}

dependencyResolutionManagement {
    repositories {
        mavenCentral()
        google()
        maven(kxrpcGrpc) {
            content { includeModuleByRegex("org\\.jetbrains\\.kotlinx", "(kotlinx-rpc|protoc-gen)-.*") }
        }
    }
}

rootProject.name = "aulo-chat"

include(":proto", ":shared", ":composeApp", ":androidApp", ":server")
