package aulo.buildlogic

/** What a module is allowed to depend on (T2.2). Every module declares one. */
enum class ModuleRole {
    /** Types and interfaces only: no I/O, HTTP, storage or native code; depends only on contracts. */
    CONTRACT,

    /** Ordinary module: may do I/O, may not use the CLI libraries. */
    LIBRARY,

    /** Links an executable or the FFI library; the only place for the CLI libraries. */
    SURFACE,
}

/**
 * One module as the graph check sees it. Coordinates are `group:name`; native libraries are
 * `cinterop:<name>`, because they have no Maven coordinate.
 */
data class ModuleNode(
    val path: String,
    val role: ModuleRole?,
    val externals: Set<String>,
    val projects: Set<String>,
    val cinterops: Set<String>,
)

/**
 * The module-role rules. The pattern lists are the single place to extend when a task wires a
 * new library of one of these kinds; a pattern without `:` matches a whole group and its
 * subgroups, one with `:` matches a coordinate prefix.
 */
object ModuleGraph {
    /** Heavy dependencies that must live in exactly one owner module. */
    val heavy: Map<String, List<String>> =
        mapOf(
            "SQLite" to listOf("app.cash.sqldelight", "co.touchlab:sqliter", "cinterop:sqlite"),
            "the gRPC stack" to listOf("io.grpc", "com.squareup.wire:wire-grpc", "cinterop:grpc"),
            "sherpa-onnx" to listOf("cinterop:sherpa"),
            "a WASM runtime" to listOf("io.github.charlietap.chasm", "cinterop:wasm"),
        )

    /** I/O and HTTP client libraries a contract must not pull, on top of every heavy one. */
    val io: List<String> =
        listOf(
            "io.ktor",
            "com.squareup.okio",
            "com.squareup.okhttp3",
            "org.jetbrains.kotlinx:kotlinx-io",
        ) + heavy.values.flatten()

    /** CLI and terminal libraries only a surface may use. */
    val cli: List<String> = listOf("com.github.ajalt.clikt", "com.github.ajalt.mordant")

    fun matches(
        coordinate: String,
        pattern: String,
    ): Boolean {
        if (':' in pattern) return coordinate.startsWith(pattern)
        val group = coordinate.substringBefore(':')
        return group == pattern || group.startsWith("$pattern.")
    }

    private fun ModuleNode.deps(): Set<String> = externals + cinterops.map { "cinterop:$it" }

    private fun Collection<String>.matching(patterns: List<String>): List<String> =
        filter { dep -> patterns.any { matches(dep, it) } }.sorted()

    /** Every broken rule, one line each; empty when the graph is fine. */
    fun violations(modules: List<ModuleNode>): List<String> {
        val roles = modules.associate { it.path to it.role }
        val out = mutableListOf<String>()
        for (m in modules.sortedBy { it.path }) {
            if (m.role == null) {
                out += "${m.path}: declares no role (set aulo { role = ... } in its build file)"
            }
            if (m.role == ModuleRole.CONTRACT) {
                m.externals.matching(io).forEach { out += "${m.path}: contract module depends on I/O library $it" }
                m.cinterops.sorted().forEach { out += "${m.path}: contract module links native library $it" }
                m.projects
                    .filter { roles[it] != ModuleRole.CONTRACT }
                    .sorted()
                    .forEach { out += "${m.path}: contract module depends on non-contract module $it" }
            }
            if (m.role != ModuleRole.SURFACE) {
                m.externals.matching(cli).forEach { out += "${m.path}: only surface modules may use CLI library $it" }
            }
            m.projects
                .filter { roles[it] == ModuleRole.SURFACE }
                .sorted()
                .forEach { out += "${m.path}: depends on surface module $it" }
        }
        for ((name, patterns) in heavy) {
            val owners = modules.filter { it.deps().matching(patterns).isNotEmpty() }.map { it.path }.sorted()
            if (owners.size > 1) out += "$name has more than one owner module: ${owners.joinToString()}"
        }
        return out
    }
}
