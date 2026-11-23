// A development stand-in for aulod: the aulo.v1 chat contract over gRPC with a stub model.
// It runs on the JVM so the client can be built and tested before the Kotlin/Native daemon
// (S3, S4) exists; it is not the daemon and ships nowhere.
plugins {
    alias(libs.plugins.kotlin.jvm)
    application
}

kotlin {
    jvmToolchain(21)
    compilerOptions { allWarningsAsErrors.set(true) }
}

dependencies {
    implementation(project(":proto"))
    implementation(libs.kotlinx.rpc.grpc.server)
    implementation(libs.kotlinx.coroutines.core)
    // grpc-java finds its transport on the classpath; kotlinx-rpc brings only the API.
    runtimeOnly(libs.grpc.netty.shaded)
    testImplementation(kotlin("test"))
    testImplementation(libs.kotlinx.coroutines.test)
}

application {
    mainClass = "aulo.chat.server.MainKt"
}

tasks.test { useJUnitPlatform() }
