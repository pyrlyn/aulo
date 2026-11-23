import aulo.buildlogic.AuloModuleExtension
import aulo.buildlogic.ModuleManifestTask
import org.gradle.api.artifacts.ProjectDependency
import org.jetbrains.kotlin.gradle.plugin.mpp.KotlinNativeTarget

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

val aulo = extensions.create<AuloModuleExtension>("aulo")

// Declared dependencies of production source sets; tests may use anything.
val productionDependencies =
    provider {
        configurations
            .filter { c -> listOf("Api", "Implementation", "CompileOnly").any { c.name.endsWith(it) } }
            .filterNot { it.name.contains("Test") }
            .flatMap { it.dependencies }
    }

tasks.register<ModuleManifestTask>("moduleManifest") {
    modulePath.set(project.path)
    role.set(aulo.role)
    externals.set(
        productionDependencies.map { deps ->
            deps.filterNot { it is ProjectDependency }.map { "${it.group}:${it.name}" }.distinct()
        },
    )
    projects.set(productionDependencies.map { deps -> deps.filterIsInstance<ProjectDependency>().map { it.path }.distinct() })
    cinterops.set(
        provider {
            kotlin.targets
                .withType<KotlinNativeTarget>()
                .flatMap { it.compilations.getByName("main").cinterops }
                .map { it.name }
                .distinct()
        },
    )
    manifest.set(layout.buildDirectory.file("aulo/module-manifest.txt"))
}
