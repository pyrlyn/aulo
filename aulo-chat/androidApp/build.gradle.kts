// The Android entry point: an activity around the shared chat screen.
plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

// The emulator reaches the machine it runs on at 10.0.2.2; a phone needs that machine's LAN
// address: ./gradlew :androidApp:installDebug -Paulo.chat.host=192.168.1.20
val serverHost = providers.gradleProperty("aulo.chat.host").getOrElse("10.0.2.2")
val serverPort = providers.gradleProperty("aulo.chat.port").getOrElse("50051")
require(Regex("[A-Za-z0-9.:-]+").matches(serverHost)) { "aulo.chat.host is not a host name or address: $serverHost" }
require(serverPort.toIntOrNull() in 1..65535) { "aulo.chat.port is not a port number: $serverPort" }

android {
    namespace = "aulo.chat.android"
    compileSdk {
        version = release(libs.versions.android.compileSdk.get().toInt()) {
            minorApiLevel = libs.versions.android.compileSdkMinor.get().toInt()
        }
    }

    defaultConfig {
        applicationId = "aulo.chat"
        minSdk = libs.versions.android.minSdk.get().toInt()
        targetSdk = libs.versions.android.targetSdk.get().toInt()
        versionCode = 1
        versionName = "0.1.0"
        buildConfigField("String", "SERVER_HOST", "\"$serverHost\"")
        buildConfigField("int", "SERVER_PORT", serverPort)
    }
    buildTypes {
        release {
            // What ships is shrunk and obfuscated. Most keep rules come with the libraries;
            // proguard-rules.pro holds the ones kotlinx-rpc does not ship.
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    buildFeatures { buildConfig = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_21
        targetCompatibility = JavaVersion.VERSION_21
    }
}

kotlin {
    jvmToolchain(21)
    compilerOptions { allWarningsAsErrors.set(true) }
}

dependencies {
    implementation(project(":composeApp"))
    implementation(libs.androidx.activity.compose)
}
