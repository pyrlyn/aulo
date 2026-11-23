import aulo.buildlogic.ConanExtension
import aulo.buildlogic.ConanInstallTask
import aulo.buildlogic.ConanLock
import aulo.buildlogic.conanDirectory
import aulo.buildlogic.conanInstallTaskName
import org.jetbrains.kotlin.gradle.plugin.mpp.KotlinNativeTarget
import org.jetbrains.kotlin.gradle.targets.native.internal.KotlinNativeDownloadTask

// For a module that links C libraries (workspace rule: native code only through a package manager).
// Conan installs what the module's `conanfile.txt` lists, pinned by its `conan.lock`, once per
// target with the profiles in `conan/`; the module's `conan { }` block binds them.
plugins {
    id("org.jetbrains.kotlin.multiplatform")
}

val conanLock = gradle.sharedServices.registerIfAbsent("conan", ConanLock::class) { maxParallelUsages.set(1) }

// Kotlin's own task: it installs Kotlin/Native and unpacks the toolchain of every target of the
// module. The compile tasks that would do so otherwise run after Conan, which needs it first.
val konanDownload = tasks.withType<KotlinNativeDownloadTask>()

kotlin.targets.withType<KotlinNativeTarget>().configureEach {
    val native = this
    tasks.register<ConanInstallTask>(conanInstallTaskName(name)) {
        group = "build"
        description = "Installs the C libraries of conanfile.txt for ${native.name} with Conan."
        target.set(native.name)
        konanTarget.set(native.konanTarget.name)
        // Conan Center has binaries for macOS; Linux and Windows need Kotlin/Native's own sysroots.
        fromSource.set(!native.konanTarget.family.isAppleFamily)
        conanfile.set(layout.projectDirectory.file("conanfile.txt"))
        lockfile.set(layout.projectDirectory.file("conan.lock"))
        profiles.set(rootProject.layout.projectDirectory.dir("conan"))
        dependsOn(konanDownload)
        konanHome.set(
            provider {
                val download = konanDownload.singleOrNull() ?: error("Kotlin no longer has one task that installs Kotlin/Native")
                download.konanHome
                    .get()
                    .asFile.absolutePath
            },
        )
        konanDataDir.set(providers.gradleProperty("konan.data.dir").orElse(providers.environmentVariable("KONAN_DATA_DIR")))
        conanHome.set(rootProject.layout.buildDirectory.dir("conan-home"))
        outputDir.set(conanDirectory(native.name))
        usesService(conanLock)
    }
}

extensions.create<ConanExtension>("conan")
