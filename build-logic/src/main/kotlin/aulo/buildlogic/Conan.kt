package aulo.buildlogic

import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.Project
import org.gradle.api.file.Directory
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.Property
import org.gradle.api.provider.Provider
import org.gradle.api.services.BuildService
import org.gradle.api.services.BuildServiceParameters
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputDirectory
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.Optional
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.gradle.kotlin.dsl.getByType
import org.gradle.kotlin.dsl.named
import org.gradle.kotlin.dsl.withType
import org.gradle.process.ExecOperations
import org.gradle.work.DisableCachingByDefault
import org.jetbrains.kotlin.gradle.dsl.KotlinMultiplatformExtension
import org.jetbrains.kotlin.gradle.plugin.mpp.KotlinNativeTarget
import java.io.File
import java.util.Properties
import javax.inject.Inject

internal fun conanInstallTaskName(target: String): String = "conanInstall${target.replaceFirstChar(Char::uppercase)}"

/** Where [ConanInstallTask] puts the libraries of one target. */
internal fun Project.conanDirectory(target: String): Provider<Directory> = layout.buildDirectory.dir("conan/$target")

/**
 * Held by every [ConanInstallTask] while it runs, one at a time: Conan's cache does not allow
 * concurrent use (https://docs.conan.io/2/knowledge/guidelines.html).
 */
abstract class ConanLock : BuildService<BuildServiceParameters.None>

/**
 * Installs the C libraries of a module's `conanfile.txt` for one Kotlin/Native target, at the
 * revisions of its `conan.lock` and with the profile `conan/<target>.profile`. Each library lands
 * in `<outputDir>/direct_deploy/<package>/{include,lib}`.
 */
@DisableCachingByDefault(because = "Copies out of Conan's own cache; a build cache entry would only duplicate it")
abstract class ConanInstallTask
    @Inject
    constructor(
        private val exec: ExecOperations,
    ) : DefaultTask() {
        /** The Gradle name of the target, which names its profile: `linuxX64`. */
        @get:Input
        abstract val target: Property<String>

        /** Kotlin/Native's own name of the target, which its `konan.properties` uses: `linux_x64`. */
        @get:Input
        abstract val konanTarget: Property<String>

        /**
         * Whether a library without a matching Conan Center binary is built, with the compiler and
         * sysroot Kotlin/Native links this target with. Otherwise only such a binary is accepted.
         */
        @get:Input
        abstract val fromSource: Property<Boolean>

        @get:InputFile
        @get:PathSensitive(PathSensitivity.NONE)
        abstract val conanfile: RegularFileProperty

        @get:InputFile
        @get:PathSensitive(PathSensitivity.NONE)
        abstract val lockfile: RegularFileProperty

        /** The directory of `<target>.profile`, `build.profile` and what they include. */
        @get:InputDirectory
        @get:PathSensitive(PathSensitivity.RELATIVE)
        abstract val profiles: DirectoryProperty

        /** The Kotlin/Native distribution; its path changes with the Kotlin version. */
        @get:Input
        abstract val konanHome: Property<String>

        /** Kotlin/Native's data directory when it is not `~/.konan`. */
        @get:Input
        @get:Optional
        abstract val konanDataDir: Property<String>

        /**
         * Conan's home, a cache of this build alone. The user's `~/.conan2` may be in use by
         * another Conan at the same moment (pyrlyn-kit's own install task, for one), which
         * [ConanLock] cannot see.
         */
        @get:Internal
        abstract val conanHome: DirectoryProperty

        @get:OutputDirectory
        abstract val outputDir: DirectoryProperty

        @TaskAction
        fun install() {
            val out = outputDir.get().asFile
            // Conan adds to what is there, and a library dropped from the conanfile must not stay linkable.
            out.deleteRecursively()
            val profileDir = profiles.get().asFile
            val log = temporaryDir.resolve("conan.log")
            val exit =
                log.outputStream().use { stream ->
                    try {
                        exec
                            .exec {
                                commandLine(
                                    "conan",
                                    "install",
                                    conanfile.get().asFile.path,
                                    "--lockfile=${lockfile.get().asFile.path}",
                                    "-pr:h=${profileDir.resolve("${target.get()}.profile").path}",
                                    "-pr:b=${profileDir.resolve("build.profile").path}",
                                    if (fromSource.get()) "--build=missing" else "--build=never",
                                    "--deployer=direct_deploy",
                                    "--deployer-folder=${out.path}",
                                    "--output-folder=${out.resolve("generators").path}",
                                )
                                environment("CONAN_HOME", conanHome.get().asFile.path)
                                if (fromSource.get()) environment(toolchain())
                                standardOutput = stream
                                errorOutput = stream
                                isIgnoreExitValue = true
                            }.exitValue
                    } catch (e: GradleException) {
                        throw GradleException("Could not run conan; `mise install` provides it, CMake and Ninja (mise.toml)", e)
                    }
                }
            if (exit != 0) {
                val tail = log.readLines().takeLast(LOG_TAIL).joinToString("\n")
                throw GradleException("conan install failed for ${target.get()} (exit $exit). The end of $log:\n$tail")
            }
        }

        private fun toolchain(): Map<String, String> {
            val properties = Properties()
            File(konanHome.get(), "konan/konan.properties").reader().use(properties::load)
            // Forward slashes: a profile's values are Python literals, where a Windows `\U` is an escape.
            val data = (konanDataDir.orNull ?: "${System.getProperty("user.home")}/.konan").replace('\\', '/')
            val host = KonanToolchain.hostName(System.getProperty("os.name"))
            val paths = KonanToolchain.environment(properties, "$data/dependencies", host, konanTarget.get())
            paths.forEach { (name, path) ->
                if (!File(path).isDirectory) throw GradleException("$name: Kotlin/Native has not unpacked $path")
            }
            return paths
        }

        private companion object {
            const val LOG_TAIL = 40
        }
    }

/** The `conan { }` block of a module that applies `aulo.conan`. */
abstract class ConanExtension
    @Inject
    constructor(
        private val project: Project,
    ) {
        /**
         * Binds the static libraries [staticLibraries] of the Conan package [pkg] on every native
         * target as the cinterop [name], described by `src/nativeInterop/cinterop/<name>.def`. List
         * a library before the ones it uses, as the linker reads them in that order.
         */
        fun cinterop(
            name: String,
            pkg: String,
            vararg staticLibraries: String,
        ) {
            require(staticLibraries.isNotEmpty()) { "cinterop $name links no library" }
            project.extensions.getByType<KotlinMultiplatformExtension>().targets.withType<KotlinNativeTarget>().configureEach {
                val install = project.tasks.named<ConanInstallTask>(conanInstallTaskName(this.name))
                val root =
                    project
                        .conanDirectory(this.name)
                        .get()
                        .asFile
                        .resolve("direct_deploy/$pkg")
                val interop =
                    compilations.getByName("main").cinterops.create(name) {
                        definitionFile.set(project.layout.projectDirectory.file("src/nativeInterop/cinterop/$name.def"))
                        includeDirs(root.resolve("include"))
                        extraOpts(
                            listOf("-libraryPath", root.resolve("lib").path) + staticLibraries.flatMap { listOf("-staticLibrary", it) },
                        )
                    }
                project.tasks.named(interop.interopProcessingTaskName) {
                    // The options above carry paths only: this makes the binding wait for Conan and
                    // rebuild when Conan installs something else.
                    inputs
                        .dir(install.flatMap { it.outputDir })
                        .withPropertyName("conan")
                        .withPathSensitivity(PathSensitivity.RELATIVE)
                }
            }
        }
    }
