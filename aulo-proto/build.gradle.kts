import aulo.buildlogic.ModuleRole

// The aulo.v1 API as Kotlin (spec §11): messages and service interfaces generated from
// `proto/aulo/v1`. No transport lives here; aulo-server binds the interfaces to one (T3.3).
plugins {
    id("aulo.kmp")
    id("com.squareup.wire")
}

aulo {
    role = ModuleRole.CONTRACT
}

wire {
    sourcePath {
        srcDir(rootProject.layout.projectDirectory.dir("proto"))
    }
    kotlin {
        // One interface per service with `suspend` methods and channels for streams. The client
        // role would need wire-grpc-client, which has no linuxArm64 build.
        rpcRole = "server"
        rpcCallStyle = "suspending"
    }
}

kotlin {
    sourceSets {
        commonMain.dependencies {
            // Generated messages extend Wire's types, and service streams are coroutine channels.
            api(libs.wire.runtime)
            api(libs.kotlinx.coroutines.core)
        }
    }
}
