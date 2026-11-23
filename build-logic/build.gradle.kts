plugins {
    `kotlin-dsl`
    alias(libs.plugins.ktlint)
}

dependencies {
    implementation(libs.kotlin.gradle.plugin)
    implementation(libs.ktlint.gradle.plugin)
    implementation(libs.kotlin.serialization.plugin)
    implementation(libs.sqldelight.gradle.plugin)
    implementation(libs.wire.gradle.plugin)
    implementation(libs.wire.schema)
    testImplementation(kotlin("test-junit5"))
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

tasks.test {
    useJUnitPlatform()
}

ktlint {
    version.set(libs.versions.ktlint.asProvider())
}
