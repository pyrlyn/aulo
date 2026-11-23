// Everything of the client that is not pixels: the domain model and use cases, the gRPC gateway
// and the SQLDelight cache behind the repository, the view model and the Koin modules.
plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.android.kmp.library)
    alias(libs.plugins.sqldelight)
}

kotlin {
    jvmToolchain(21)
    compilerOptions { allWarningsAsErrors.set(true) }

    android {
        namespace = "aulo.chat.shared"
        compileSdk {
            version = release(libs.versions.android.compileSdk.get().toInt()) {
                minorApiLevel = libs.versions.android.compileSdkMinor.get().toInt()
            }
        }
        minSdk = libs.versions.android.minSdk.get().toInt()
    }
    jvm()
    iosArm64()
    iosSimulatorArm64()

    sourceSets {
        commonMain.dependencies {
            implementation(project(":proto"))
            api(libs.kotlinx.coroutines.core)
            api(libs.koin.core)
            api(libs.koin.core.viewmodel)
            api(libs.lifecycle.viewmodel)
            implementation(libs.sqldelight.runtime)
            implementation(libs.sqldelight.coroutines)
        }
        commonTest.dependencies {
            implementation(kotlin("test"))
            implementation(libs.kotlinx.coroutines.test)
        }
        androidMain.dependencies {
            implementation(libs.sqldelight.driver.android)
            // kotlinx-rpc brings the gRPC API only; each JVM platform adds its own transport.
            implementation(libs.grpc.okhttp)
        }
        jvmMain.dependencies {
            implementation(libs.sqldelight.driver.jvm)
            implementation(libs.grpc.netty.shaded)
        }
        jvmTest.dependencies {
            // The end-to-end test runs the client against the real development server.
            implementation(project(":server"))
            implementation(libs.kotlinx.rpc.grpc.server)
        }
        iosMain.dependencies {
            implementation(libs.sqldelight.driver.native)
        }
    }
}

sqldelight {
    databases {
        create("ChatDatabase") {
            packageName.set("aulo.chat.data.cache")
        }
    }
}
