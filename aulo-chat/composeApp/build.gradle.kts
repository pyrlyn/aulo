import org.jetbrains.compose.desktop.application.dsl.TargetFormat

// The pixels: the theme with each platform's tokens, the chat screen, and the entry points for
// desktop and iOS. The Android entry point is the :androidApp module.
plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.android.kmp.library)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.compose)
}

kotlin {
    jvmToolchain(21)
    compilerOptions { allWarningsAsErrors.set(true) }

    android {
        namespace = "aulo.chat.ui"
        compileSdk {
            version = release(libs.versions.android.compileSdk.get().toInt()) {
                minorApiLevel = libs.versions.android.compileSdkMinor.get().toInt()
            }
        }
        minSdk = libs.versions.android.minSdk.get().toInt()
    }
    jvm()
    listOf(iosArm64(), iosSimulatorArm64()).forEach { target ->
        target.binaries.framework {
            baseName = "AuloChat"
            isStatic = true
            // The SQLDelight plugin links SQLite only in the module it is applied to (:shared);
            // the framework is linked here.
            linkerOpts("-lsqlite3")
        }
    }

    sourceSets {
        commonMain.dependencies {
            api(project(":shared"))
            implementation(libs.compose.runtime)
            implementation(libs.compose.foundation)
            implementation(libs.compose.ui)
            implementation(libs.compose.material3)
            implementation(libs.koin.compose)
            implementation(libs.koin.compose.viewmodel)
            implementation(libs.lifecycle.viewmodel.compose)
            implementation(libs.lifecycle.runtime.compose)
        }
        jvmMain.dependencies {
            implementation(compose.desktop.currentOs)
            implementation(libs.kotlinx.coroutines.swing)
        }
        jvmTest.dependencies {
            implementation(kotlin("test"))
            implementation(libs.compose.ui.test)
        }
    }
}

// Skiko loads its native renderer with System.load; JDK 24+ warns about that and will block it.
val nativeAccess = "--enable-native-access=ALL-UNNAMED"

compose.desktop {
    application {
        mainClass = "aulo.chat.ui.MainKt"
        jvmArgs += nativeAccess
        nativeDistributions {
            targetFormats(TargetFormat.Dmg, TargetFormat.Msi, TargetFormat.Deb)
            packageName = "aulo chat"
            // jpackage refuses a macOS version whose first number is 0.
            packageVersion = "1.0.0"
            // The JDBC SQLite driver needs java.sql; Skiko needs jdk.unsupported.
            modules("java.desktop", "java.sql", "jdk.unsupported")
        }
    }
}

tasks.named<Test>("jvmTest") {
    jvmArgs(nativeAccess)
    // The render test writes the chat screen here, one PNG per platform theme.
    systemProperty("aulo.chat.renders", layout.buildDirectory.dir("renders").get().asFile.absolutePath)
}
