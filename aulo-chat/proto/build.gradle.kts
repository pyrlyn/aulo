import kotlinx.rpc.protoc.proto

// The aulo.v1 contract as Kotlin: messages and gRPC service interfaces generated from the
// repository's proto/ directory, so the client and the dev server compile against the same
// files aulod will serve. T3.2 owns the daemon's generated module; this one exists because
// aulo-chat is a separate build.
plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.kotlinx.rpc)
}

rpc { protoc() }

kotlin {
    jvmToolchain(21)
    jvm()
    iosArm64()
    iosSimulatorArm64()

    sourceSets {
        commonMain {
            proto {
                srcDir(rootDir.resolve("../proto"))
            }
            dependencies {
                api(libs.kotlinx.rpc.grpc.core)
                api(libs.kotlinx.rpc.grpc.client)
                api(libs.kotlinx.rpc.protobuf)
                api(libs.kotlinx.coroutines.core)
            }
        }
    }
}
