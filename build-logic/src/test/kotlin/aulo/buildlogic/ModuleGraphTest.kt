package aulo.buildlogic

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ModuleGraphTest {
    private fun node(
        path: String,
        role: ModuleRole?,
        externals: Set<String> = emptySet(),
        projects: Set<String> = emptySet(),
        cinterops: Set<String> = emptySet(),
    ) = ModuleNode(path, role, externals, projects, cinterops)

    private val types = node(":aulo-types", ModuleRole.CONTRACT, setOf("org.jetbrains.kotlinx:kotlinx-serialization-core"))

    @Test
    fun aWellFormedGraphHasNoViolations() {
        val graph =
            listOf(
                types,
                node(":aulo-store", ModuleRole.LIBRARY, setOf("app.cash.sqldelight:native-driver"), setOf(":aulo-types")),
                node(":aulo", ModuleRole.SURFACE, setOf("com.github.ajalt.clikt:clikt"), setOf(":aulo-store", ":aulo-types")),
            )
        assertEquals(emptyList(), ModuleGraph.violations(graph))
    }

    @Test
    fun contractPullingAnHttpClientFails() {
        val graph = listOf(types.copy(externals = types.externals + "io.ktor:ktor-client-core"))
        assertEquals(listOf(":aulo-types: contract module depends on I/O library io.ktor:ktor-client-core"), ModuleGraph.violations(graph))
    }

    @Test
    fun contractPullingFileIoOrStorageFails() {
        val graph = listOf(types.copy(externals = setOf("com.squareup.okio:okio", "co.touchlab:sqliter-driver")))
        assertEquals(
            listOf(
                ":aulo-types: contract module depends on I/O library co.touchlab:sqliter-driver",
                ":aulo-types: contract module depends on I/O library com.squareup.okio:okio",
            ),
            ModuleGraph.violations(graph),
        )
    }

    @Test
    fun contractLinkingNativeCodeOrALibraryModuleFails() {
        val graph =
            listOf(
                types.copy(cinterops = setOf("sqlite3"), projects = setOf(":aulo-store")),
                node(":aulo-store", ModuleRole.LIBRARY),
            )
        assertEquals(
            listOf(
                ":aulo-types: contract module links native library sqlite3",
                ":aulo-types: contract module depends on non-contract module :aulo-store",
            ),
            ModuleGraph.violations(graph),
        )
    }

    @Test
    fun cliLibrariesBelongToSurfacesOnly() {
        val graph = listOf(node(":aulo-agent", ModuleRole.LIBRARY, setOf("com.github.ajalt.mordant:mordant")))
        assertEquals(
            listOf(":aulo-agent: only surface modules may use CLI library com.github.ajalt.mordant:mordant"),
            ModuleGraph.violations(graph),
        )
    }

    @Test
    fun nothingDependsOnASurface() {
        val graph = listOf(node(":aulo", ModuleRole.SURFACE), node(":aulo-agent", ModuleRole.LIBRARY, projects = setOf(":aulo")))
        assertEquals(listOf(":aulo-agent: depends on surface module :aulo"), ModuleGraph.violations(graph))
    }

    @Test
    fun heavyDependencyNeedsOneOwner() {
        val graph =
            listOf(
                node(":aulo-store", ModuleRole.LIBRARY, setOf("app.cash.sqldelight:native-driver")),
                node(":aulo-memory", ModuleRole.LIBRARY, cinterops = setOf("sqlite3")),
                node(":aulo-stt", ModuleRole.LIBRARY, cinterops = setOf("sherpa_onnx")),
            )
        assertEquals(listOf("SQLite has more than one owner module: :aulo-memory, :aulo-store"), ModuleGraph.violations(graph))
    }

    @Test
    fun everyModuleDeclaresARole() {
        assertEquals(
            listOf(":aulo-x: declares no role (set aulo { role = ... } in its build file)"),
            ModuleGraph.violations(listOf(node(":aulo-x", null))),
        )
    }

    @Test
    fun groupPatternsMatchSubgroupsButNotLookalikes() {
        assertTrue(ModuleGraph.matches("io.ktor:ktor-client-core", "io.ktor"))
        assertTrue(ModuleGraph.matches("io.ktor.plugin:x", "io.ktor"))
        assertTrue(!ModuleGraph.matches("io.ktorx:x", "io.ktor"))
        assertTrue(ModuleGraph.matches("org.jetbrains.kotlinx:kotlinx-io-core", "org.jetbrains.kotlinx:kotlinx-io"))
        assertTrue(!ModuleGraph.matches("org.jetbrains.kotlinx:kotlinx-serialization-core", "org.jetbrains.kotlinx:kotlinx-io"))
    }

    @Test
    fun manifestRoundTrips() {
        val node = node(":aulo", ModuleRole.SURFACE, setOf("a.b:c"), setOf(":aulo-types"), setOf("sqlite3"))
        assertEquals(node, ModuleManifest.parse(ModuleManifest.render(node)))
        assertEquals(node.copy(role = null), ModuleManifest.parse(ModuleManifest.render(node.copy(role = null))))
    }
}
