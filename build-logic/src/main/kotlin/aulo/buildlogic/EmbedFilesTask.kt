package aulo.buildlogic

import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.CacheableTask
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.InputDirectory
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction

/**
 * Embeds every file under [source] in a generated Kotlin source as `internal val <name>:
 * Map<String, String>`, keyed by the path relative to [source] with `/` separators. Kotlin/Native
 * has no resources, and a file people read or edit (the default config, SQL migrations) stays
 * one file rather than a copy pasted into a string.
 */
@CacheableTask
abstract class EmbedFilesTask : DefaultTask() {
    @get:InputDirectory
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val source: DirectoryProperty

    @get:Input
    abstract val packageName: Property<String>

    @get:Input
    abstract val valName: Property<String>

    @get:Input
    abstract val doc: Property<String>

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    @TaskAction
    fun embed() {
        val root = source.get().asFile
        val entries =
            root
                .walkTopDown()
                .filter { it.isFile }
                .map { it.relativeTo(root).invariantSeparatorsPath to it }
                .sortedBy { it.first }
                .map { (key, file) -> "        \"$key\" to \"\"\"${embeddable(key, file.readText())}\"\"\"," }
                .toList()
        val out = outputDir.get().asFile
        out.deleteRecursively()
        val file = out.resolve(packageName.get().replace('.', '/')).resolve("${valName.get()}.kt")
        file.parentFile.mkdirs()
        file.writeText(
            "package ${packageName.get()}\n\n" +
                "/** ${doc.get()} */\n" +
                "internal val ${valName.get()}: Map<String, String> =\n" +
                "    mapOf(\n" + entries.joinToString("") { "$it\n" } + "    )\n",
        )
    }

    // The key lands in a plain string literal and the text in a raw one: neither has an escape
    // for everything, so refuse what would break out of them.
    private fun embeddable(
        key: String,
        text: String,
    ): String {
        check(KEY.matches(key)) { "$key: an embedded file's path may only use letters, digits, '.', '_', '-' and '/'" }
        check("\"\"\"" !in text) { "$key must not contain three double quotes" }
        return text.replace("$", "\${'$'}")
    }

    private companion object {
        val KEY = Regex("[A-Za-z0-9._/-]+")
    }
}
