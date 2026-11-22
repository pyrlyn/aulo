package aulo.config

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.descriptors.PolymorphicKind
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.StructureKind
import kotlinx.serialization.descriptors.elementNames
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

/**
 * `config/default.toml` is documentation, so the only way to keep it honest is to read it back:
 * its settings must parse into `Config()`, and a key added to `Config` without a line in the
 * file must fail here. `##` lines are prose; a single-`#` line is a commented-out setting or
 * table header; a trailing `# unset by default` marks an example, not the default.
 */
class DefaultTomlTest {
    private val unset = "# unset by default"

    private fun uncommented(withExamples: Boolean): String {
        var inUnsetTable = false
        return buildString {
            for (line in DEFAULT_TOML.lines()) {
                val code = line.removePrefix("# ").takeIf { line.startsWith("# ") } ?: continue
                val isUnset = code.endsWith(unset)
                if (code.startsWith("[")) inUnsetTable = isUnset
                if (withExamples || !(isUnset || inUnsetTable)) append(code).append('\n')
            }
        }
    }

    private fun parse(text: String) = loadConfig(LoadOptions(overrides = parseLayer(text, Origin.Override))).config

    @Test
    fun shippedFileIsInertAndEqualsTheDefaults() {
        for ((index, line) in DEFAULT_TOML.lines().withIndex()) {
            assertTrue(line.isEmpty() || line.startsWith("#"), "active line ${index + 1} in default.toml")
        }
        assertEquals(Config(), parse(DEFAULT_TOML))
    }

    @Test
    fun uncommentingEveryDefaultChangesNothing() {
        assertEquals(Config(), parse(uncommented(withExamples = false)))
    }

    @Test
    fun theExamplesAreValidSettings() {
        val parsed = parse(uncommented(withExamples = true))
        assertNotEquals(Config(), parsed)
        assertEquals(1, parsed.providers.endpoints.size)
        assertEquals(2, parsed.mcp.servers.size)
    }

    /** Every leaf key path of the config; map entries are the wildcard `*`. */
    @OptIn(ExperimentalSerializationApi::class)
    private fun leafPaths(
        descriptor: SerialDescriptor,
        path: String,
        out: MutableList<String>,
    ) {
        when (descriptor.kind) {
            StructureKind.CLASS -> {
                descriptor.elementNames.forEachIndexed { i, name ->
                    leafPaths(descriptor.getElementDescriptor(i), join(path, name), out)
                }
            }

            StructureKind.MAP -> {
                leafPaths(descriptor.getElementDescriptor(1), join(path, "*"), out)
            }

            PolymorphicKind.SEALED -> {
                out += join(path, descriptor.discriminator())
                descriptor.variants().forEach { leafPaths(it, path, out) }
            }

            else -> {
                out += path
            }
        }
    }

    /** `(table, key)` of every setting line, with commented tables tracked. */
    private fun documented(): List<Pair<String, String>> {
        var table = ""
        val found = mutableListOf<Pair<String, String>>()
        for (line in DEFAULT_TOML.lines()) {
            val code = line.removePrefix("# ").takeIf { line.startsWith("# ") } ?: continue
            if (code.startsWith("[")) {
                table = code.removePrefix("[").substringBefore("]")
            } else if (" = " in code) {
                found += table to code.substringBefore(" = ")
            }
        }
        return found
    }

    private fun tableMatches(
        documented: String,
        expected: String,
    ): Boolean {
        val doc = documented.split(".")
        val exp = expected.split(".")
        return doc.size == exp.size && doc.zip(exp).all { (d, e) -> e == "*" || d == e }
    }

    private fun missingFrom(lines: List<Pair<String, String>>): List<String> {
        val paths = mutableListOf<String>()
        leafPaths(Config.serializer().descriptor, "", paths)
        assertTrue("mcp.servers.*.command" in paths, "the walk must reach sealed variants")
        return paths.filter { path ->
            val table = path.substringBeforeLast(".", "")
            val key = path.substringAfterLast(".")
            lines.none { (t, k) -> k == key && tableMatches(t, table) }
        }
    }

    @Test
    fun everyConfigKeyIsInTheFile() {
        assertEquals(emptyList(), missingFrom(documented()), "keys missing from default.toml")
    }

    @Test
    fun theCompletenessCheckCatchesAMissingKey() {
        val without = documented().filter { (_, key) -> key != "approval_timeout_secs" }
        assertEquals(listOf("policy.approval_timeout_secs"), missingFrom(without))
    }
}
