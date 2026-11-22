// Applied by every module, so targets, warnings and style cannot drift between modules.
plugins {
    id("org.jetbrains.kotlin.multiplatform")
    id("org.jlleitschuh.gradle.ktlint")
}

kotlin {
    // spec.md §17 D3. Intel macOS is not supported.
    macosArm64()
    linuxX64()
    linuxArm64()
    mingwX64()

    compilerOptions {
        allWarningsAsErrors.set(true)
    }

    sourceSets {
        commonTest.dependencies {
            implementation(kotlin("test"))
        }
    }
}

// Kotlin Multiplatform has no `test` task; keep the conventional name the docs and CI use.
tasks.register("test") {
    group = "verification"
    description = "Runs the tests of every target this host can run."
    dependsOn("allTests")
}

ktlint {
    version.set(
        extensions
            .getByType<VersionCatalogsExtension>()
            .named("libs")
            .findVersion("ktlint")
            .get()
            .requiredVersion,
    )
    filter {
        // Generated sources (BuildInfo.kt) are not ours to format.
        exclude { it.file.path.contains("/build/generated/") }
    }
}
