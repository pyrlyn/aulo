package aulo.config

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class RenderTest {
    private fun overrides(toml: String) = parseLayer(toml, Origin.Override)

    @Test
    fun defaultsRenderOneLinePerLeafWithTheirOrigin() {
        val out = loadConfig(LoadOptions()).render(withOrigin = true)
        assertTrue("policy.approval_timeout_secs = 60  # built-in defaults\n" in out)
        assertTrue("voice.listen = \"push-to-talk\"  # built-in defaults\n" in out)
        assertTrue("log.level = \"info\"  # built-in defaults\n" in out)
        // Unset optional keys have no value and no line.
        assertFalse("daemon.listen" in out)
        assertEquals(out.lines().filter { it.isNotEmpty() }.sorted(), out.lines().filter { it.isNotEmpty() })
    }

    @Test
    fun theOutputIsTomlThatLoadsBackToTheSameConfig() {
        val loaded =
            loadConfig(
                LoadOptions(
                    env = mapOf("AULO_VOICE__LISTEN" to "wake-word"),
                    overrides =
                        overrides(
                            """
                            [mcp.servers."my files"]
                            transport = "stdio"
                            command = "mcp-files"
                            args = ["--root", "."]
                            """.trimIndent(),
                        ),
                ),
            )
        val out = loaded.render(withOrigin = false)
        assertTrue("mcp.servers.\"my files\".args = [\"--root\", \".\"]\n" in out)
        assertTrue("voice.listen = \"wake-word\"\n" in out)
        assertEquals(loaded.config, loadConfig(LoadOptions(overrides = overrides(out))).config)
        val withOrigin = loaded.render(withOrigin = true)
        assertTrue("voice.listen = \"wake-word\"  # AULO_VOICE__LISTEN\n" in withOrigin)
        assertTrue("mcp.servers.\"my files\".command = \"mcp-files\"  # command-line overrides\n" in withOrigin)
    }

    @Test
    fun everyValueThatCouldCarryASecretIsMasked() {
        val loaded =
            loadConfig(
                LoadOptions(
                    overrides =
                        overrides(
                            """
                            providers.endpoints.x = { kind = "openai-compatible", base_url = "https://user:s3cret@host/v1?api_key=abc123" }
                            mcp.servers.s = { transport = "stdio", command = "srv", args = ["--password=hunter2"] }
                            mcp.servers.h = { transport = "http", url = "https://u:p4ss@host/mcp" }
                            policy.allow = ["Bash(curl -H 'Authorization: Bearer abcdefgh12345678')"]
                            plugins.dirs = ["/plugins"]
                            """.trimIndent(),
                        ),
                ),
            )
        val out = loaded.render(withOrigin = true)
        for ((index, fixture) in listOf("s3cret", "abc123", "hunter2", "p4ss", "abcdefgh12345678").withIndex()) {
            // The message names the case by index: an assertion message is a log too.
            assertFalse(fixture in out, "fixture $index leaked")
        }
        assertTrue("[REDACTED]" in out)
        assertTrue("plugins.dirs = [\"/plugins\"]  # command-line overrides\n" in out)
    }
}
