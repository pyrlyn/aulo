package aulo.buildlogic

import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputFiles
import org.gradle.api.tasks.Optional
import org.gradle.api.tasks.OutputFile
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction

/** The `aulo { }` block of a module build file. */
abstract class AuloModuleExtension {
    abstract val role: Property<ModuleRole>
}

/** Line format of a module manifest; each module writes one, the root check reads them all. */
object ModuleManifest {
    fun render(node: ModuleNode): String =
        buildString {
            appendLine("path=${node.path}")
            node.role?.let { appendLine("role=${it.name}") }
            node.externals.sorted().forEach { appendLine("external=$it") }
            node.projects.sorted().forEach { appendLine("project=$it") }
            node.cinterops.sorted().forEach { appendLine("cinterop=$it") }
        }

    fun parse(text: String): ModuleNode {
        val pairs = text.lines().filter { it.isNotBlank() }.map { it.substringBefore('=') to it.substringAfter('=') }

        fun all(key: String) = pairs.filter { it.first == key }.map { it.second }.toSet()
        return ModuleNode(
            path = all("path").single(),
            role = all("role").singleOrNull()?.let(ModuleRole::valueOf),
            externals = all("external"),
            projects = all("project"),
            cinterops = all("cinterop"),
        )
    }
}

/** Writes this module's role and declared (non-test) dependencies for [CheckModuleGraphTask]. */
abstract class ModuleManifestTask : DefaultTask() {
    @get:Input
    abstract val modulePath: Property<String>

    @get:Input
    @get:Optional
    abstract val role: Property<ModuleRole>

    @get:Input
    abstract val externals: ListProperty<String>

    @get:Input
    abstract val projects: ListProperty<String>

    @get:Input
    abstract val cinterops: ListProperty<String>

    @get:OutputFile
    abstract val manifest: RegularFileProperty

    @TaskAction
    fun write() {
        val node =
            ModuleNode(
                path = modulePath.get(),
                role = role.orNull,
                externals = externals.get().toSet(),
                projects = projects.get().toSet(),
                cinterops = cinterops.get().toSet(),
            )
        manifest.get().asFile.writeText(ModuleManifest.render(node))
    }
}

/** Fails the build when the module graph breaks a rule of [ModuleGraph]. */
abstract class CheckModuleGraphTask : DefaultTask() {
    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val manifests: ConfigurableFileCollection

    // An output makes the check up to date until a manifest changes.
    @get:OutputFile
    abstract val report: RegularFileProperty

    @TaskAction
    fun check() {
        val modules = manifests.files.sortedBy { it.path }.map { ModuleManifest.parse(it.readText()) }
        val violations = ModuleGraph.violations(modules)
        report.get().asFile.writeText(violations.joinToString("\n", postfix = "\n"))
        if (violations.isNotEmpty()) {
            throw GradleException("Module graph rules broken (T2.2):\n" + violations.joinToString("\n") { "  - $it" })
        }
    }
}
