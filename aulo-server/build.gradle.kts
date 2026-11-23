import aulo.buildlogic.GrpcTablesFactory
import aulo.buildlogic.ModuleRole

// The aulod gRPC server (spec §11): binds the aulo.v1 services, health and reflection to the
// transport in aulo-grpc, with listeners from config.
plugins {
    id("aulo.kmp")
    id("com.squareup.wire")
}

aulo {
    role = ModuleRole.LIBRARY
}

wire {
    sourcePath {
        srcDir(rootProject.layout.projectDirectory.dir("proto"))
    }
    // grpc.health.v1 and grpc.reflection.v1, vendored from grpc/grpc-proto (src/proto/README.md).
    sourcePath {
        srcDir("src/proto")
    }
    // First and not exclusive, so the Kotlin target below still sees every type.
    custom {
        schemaHandlerFactory.set(GrpcTablesFactory())
        options.set(mapOf("package" to "aulo.server.generated", "runtime" to "aulo.server"))
        exclusive.set(false)
    }
    kotlin {
        // aulo.v1 is generated once, in aulo-proto; only the grpc protos are generated here.
        includes = listOf("grpc.health.v1.*", "grpc.reflection.v1.*")
        rpcRole = "server"
        rpcCallStyle = "suspending"
    }
}

kotlin {
    compilerOptions {
        // Wire 7.2.2 writes a needless `!!` when it decodes packed repeated scalars (reflection's
        // ExtensionNumberResponse); every other warning still fails the build.
        freeCompilerArgs.add("-Xwarning-level=UNNECESSARY_NOT_NULL_ASSERTION:disabled")
    }
    sourceSets {
        commonMain.dependencies {
            api(project(":aulo-proto"))
            api(project(":aulo-grpc"))
            implementation(project(":aulo-config"))
            implementation(libs.kotlinx.coroutines.core)
            implementation(libs.crypto.rand)
        }
        nativeMain.dependencies {
            // The owner-only socket directory and where the first token is kept; its hash is stored.
            implementation(project(":aulo-secrets"))
            implementation(project(":aulo-store"))
        }
        nativeTest.dependencies {
            // Temporary home directories for the local socket; grpcurl as a child process.
            implementation("pyrlyn.kit:fs")
            implementation("pyrlyn.kit:process")
        }
    }
}
