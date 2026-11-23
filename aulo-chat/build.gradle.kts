// The plugins run inside the build, so what they resolve to is locked as well
// (buildscript-gradle.lockfile).
buildscript {
    configurations.classpath {
        resolutionStrategy.activateDependencyLocking()
    }
}

plugins {
    alias(libs.plugins.kotlin.multiplatform) apply false
    alias(libs.plugins.kotlin.jvm) apply false
    alias(libs.plugins.kotlin.compose) apply false
    alias(libs.plugins.compose) apply false
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.android.kmp.library) apply false
    alias(libs.plugins.kotlinx.rpc) apply false
    alias(libs.plugins.sqldelight) apply false
}

// The catalog pins what the build asks for; the lock files pin what it gets, transitive modules
// included, so a build does not change under an unchanged catalog; gradle/verification-metadata.xml
// pins the checksum of every file it downloads. After changing a dependency, on macOS (the iOS
// configurations exist only there):
// ./gradlew resolveAndLockAll --write-locks
// ./gradlew --write-verification-metadata sha256 help <tasks>   (the task list is in README.md)
allprojects {
    dependencyLocking {
        lockAllConfigurations()
        // Compose names these after the host OS and CPU, so no single lock file can list them;
        // their version is the Compose and Skiko version, which the lock still pins.
        ignoredDependencies.addAll(
            "org.jetbrains.compose.desktop:desktop-jvm-*",
            "org.jetbrains.skiko:skiko-awt-runtime-*",
        )
    }

    tasks.register("resolveAndLockAll") {
        description = "Resolves every configuration so that --write-locks records all of them."
        notCompatibleWithConfigurationCache("Resolves configurations at execution time")
        doFirst {
            require(gradle.startParameter.isWriteDependencyLocks) { "Run it with --write-locks" }
        }
        doLast {
            // The graph is what a lock records. Asking for the files as well fails where a
            // configuration leaves the choice of artifact to the task that consumes it.
            configurations.filter { it.isCanBeResolved }.forEach { it.incoming.resolutionResult.rootComponent.get() }
        }
    }
}

// kotlinx-rpc 0.11.0-grpc-190 ships its compiler plugin up to Kotlin 2.4.20, and 2.4.21 is a
// patch release with the same compiler API; drop this once a gRPC build for our Kotlin is
// published.
subprojects {
    // A shared-source-set metadata compile warns whenever two dependencies publish klibs with one
    // unique name (Compose's and SQLDelight's `runtime`, JetBrains' and Google's androidx forks),
    // which no build setting can avoid. The same sources compile for every target with warnings
    // as errors, so these compiles may warn.
    tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompilationTask<*>>().configureEach {
        if (name.endsWith("KotlinMetadata")) compilerOptions.allWarningsAsErrors.set(false)
    }

    configurations.configureEach {
        if (name.contains("CompilerPluginClasspath", ignoreCase = true)) {
            resolutionStrategy.eachDependency {
                if (requested.group == "org.jetbrains.kotlinx" && requested.name.startsWith("kotlinx-rpc-compiler-plugin")) {
                    useVersion("2.4.20-${rootProject.libs.versions.kotlinx.rpc.get()}")
                }
            }
        }
    }
}
