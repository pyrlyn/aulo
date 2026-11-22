package aulo.config

import fs.TempDir
import kotlinx.io.files.Path
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlin.test.AfterTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class LoadTest {
    private val tmp = TempDir.create(".aulo-config-test")
    private val home = Path(tmp.path, "home").toString()
    private val project = Path(tmp.path, "project").toString()
    private val homeFile = Path(home, "config.toml").toString()
    private val projectFile = Path(project, ".aulo.toml").toString()

    @AfterTest
    fun cleanup() = tmp.close()

    private fun write(
        path: String,
        text: String,
    ) = fs.WriteOptions(createParent = true).write(Path(path), text.encodeToByteArray())

    private fun load(
        env: Map<String, String> = emptyMap(),
        overrides: JsonObject? = null,
    ) = loadConfig(LoadOptions(env = env, home = home, projectDir = project, overrides = overrides))

    private fun failure(
        env: Map<String, String> = emptyMap(),
        overrides: JsonObject? = null,
    ) = assertFailsWith<ConfigException> { load(env, overrides) }

    @Test
    fun missingFilesGiveTheDefaults() {
        val loaded = load()
        assertEquals(Config(), loaded.config)
        assertEquals(Origin.Default, loaded.provenance["policy.approval_timeout_secs"])
        // Unset optional keys have no value and so no origin.
        assertNull(loaded.provenance["daemon.listen"])
    }

    @Test
    fun layersApplyInOrderAndRecordWhereEachValueCameFrom() {
        write(homeFile, "[voice]\nlisten = \"wake-word\"\n[policy]\napproval_timeout_secs = 30\ndeny = [\"shell(rm *)\"]\n")
        write(projectFile, "[policy]\napproval_timeout_secs = 10\n")
        val overrides = JsonObject(mapOf("voice" to JsonObject(mapOf("barge_in" to JsonPrimitive("continue")))))
        val loaded = load(mapOf("AULO_POLICY__APPROVAL_TIMEOUT_SECS" to "5"), overrides)

        assertEquals(ListenMode.WAKE_WORD, loaded.config.voice.listen)
        assertEquals(BargeIn.CONTINUE, loaded.config.voice.bargeIn)
        assertEquals(5, loaded.config.policy.approvalTimeoutSecs)
        assertEquals(listOf("shell(rm *)"), loaded.config.policy.deny)
        assertEquals(Origin.File(homeFile), loaded.provenance["voice.listen"])
        assertEquals(Origin.File(homeFile), loaded.provenance["policy.deny"])
        assertEquals(Origin.Env("AULO_POLICY__APPROVAL_TIMEOUT_SECS"), loaded.provenance["policy.approval_timeout_secs"])
        assertEquals(Origin.Override, loaded.provenance["voice.barge_in"])
        assertEquals(Origin.Default, loaded.provenance["voice.stt.fallback"])
    }

    @Test
    fun unknownKeysAreRejectedWithTheFileName() {
        write(homeFile, "[voice]\nlisten = \"push-to-talk\"\nlisen = \"wake-word\"\n")
        val e = failure()
        assertEquals(Origin.File(homeFile), e.origin)
        assertEquals("voice.lisen", e.key)
        assertTrue(homeFile in e.message!! && "unknown key `voice.lisen`" in e.message!!, e.message)

        write(homeFile, "")
        write(projectFile, "[nope]\n")
        assertEquals(Origin.File(projectFile), failure().origin)
    }

    @Test
    fun unknownVariablesAreRejectedButOtherAuloVariablesAreIgnored() {
        val e = failure(mapOf("AULO_VOICE__NOPE" to "1"))
        assertEquals(Origin.Env("AULO_VOICE__NOPE"), e.origin)
        assertEquals("voice.nope", e.key)
        // No `__`: not configuration (AULO_HOME, credential variables).
        assertEquals(Config(), load(mapOf("AULO_OPENAI_API_KEY" to "x", "AULO_HOME" to "/elsewhere")).config)
    }

    @Test
    fun invalidValuesAreReportedWithoutEchoingThem() {
        val secret = "sk-live-0123456789abcdef"
        write(homeFile, "[policy]\napproval_timeout_secs = \"$secret\"\n")
        val e = failure()
        assertEquals("`policy.approval_timeout_secs` must be an integer", e.detail)
        assertFalse(secret in e.message!!)

        write(homeFile, "[voice]\nlisten = \"$secret\"\n")
        assertFalse(secret in failure().message!!)

        write(homeFile, "[voice.stt]\nengine = 1\n")
        assertEquals("`voice.stt.engine` must be a string", failure().detail)
    }

    @Test
    fun minimumsAreEnforced() {
        write(homeFile, "[policy]\napproval_timeout_secs = 0\n")
        assertEquals("`policy.approval_timeout_secs` must be at least 1", failure().detail)
    }

    @Test
    fun badTomlNamesTheFile() {
        write(homeFile, "[voice\n")
        val e = failure()
        assertEquals(Origin.File(homeFile), e.origin)
        assertTrue(e.message!!.startsWith("$homeFile: TOML parse error at line 1"), e.message)
    }

    @Test
    fun mapEntriesMergeAcrossLayersAndMustEndUpComplete() {
        write(homeFile, "[providers.endpoints.local]\nkind = \"openai-compatible\"\n")
        write(
            projectFile,
            "[providers.endpoints.local]\nbase_url = \"http://127.0.0.1:8080/v1\"\n[providers.tiers.main]\nprovider = \"local\"\nmodel = \"qwen\"\n",
        )
        val providers = load().config.providers
        assertEquals(ProviderConfig(ProviderKind.OPENAI_COMPATIBLE, "http://127.0.0.1:8080/v1"), providers.endpoints["local"])
        assertEquals(TierConfig("local", "qwen"), providers.tiers.main)

        write(homeFile, "")
        val e = failure()
        assertEquals("missing key `providers.endpoints.local.kind`", e.detail)
        assertEquals(Origin.File(projectFile), e.origin)
    }

    @Test
    fun mcpServersPickTheirShapeByTransport() {
        write(
            homeFile,
            "[mcp.servers.files]\ntransport = \"stdio\"\ncommand = \"mcp-files\"\nargs = [\"--root\", \".\"]\n" +
                "[mcp.servers.search]\ntransport = \"http\"\nurl = \"https://example.test/mcp\"\n",
        )
        val servers = load().config.mcp.servers
        assertEquals(McpServer.Stdio("mcp-files", listOf("--root", ".")), servers["files"])
        assertEquals(McpServer.Http("https://example.test/mcp"), servers["search"])

        write(homeFile, "[mcp.servers.x]\ntransport = \"http\"\ncommand = \"y\"\nurl = \"z\"\n")
        assertEquals("unknown key `mcp.servers.x.command`", failure().detail)
        write(homeFile, "[mcp.servers.x]\ntransport = \"pipe\"\n")
        assertEquals("`mcp.servers.x.transport` must be one of \"http\", \"stdio\"", failure().detail)
    }

    @Test
    fun variablesUseTomlSyntaxExceptForStrings() {
        val loaded =
            load(
                mapOf(
                    "AULO_VOICE__STT__ENGINE" to "whisper",
                    "AULO_VOICE__STT__RATE" to "1.5",
                    "AULO_VOICE__LISTEN" to "wake-word",
                    "AULO_PLUGINS__DISABLED" to "[\"a\", \"b\"]",
                    "AULO_PROVIDERS__ENDPOINTS__LOCAL__KIND" to "ollama",
                ),
            )
        assertEquals("whisper", loaded.config.voice.stt.engine)
        assertEquals(1.5f, loaded.config.voice.stt.rate)
        assertEquals(ListenMode.WAKE_WORD, loaded.config.voice.listen)
        assertEquals(listOf("a", "b"), loaded.config.plugins.disabled)
        assertEquals(
            ProviderKind.OLLAMA,
            loaded.config.providers.endpoints["local"]
                ?.kind,
        )
        assertEquals("`voice.stt.rate` is not a TOML value", failure(mapOf("AULO_VOICE__STT__RATE" to "fast")).detail)
    }

    @Test
    fun homeComesFromAuloHomeThenTheUserDirectory() {
        assertEquals("/a", auloHome(mapOf("AULO_HOME" to "/a", "HOME" to "/h")))
        assertEquals(Path("/h", ".aulo").toString(), auloHome(mapOf("AULO_HOME" to "", "HOME" to "/h")))
        assertEquals(Path("C:/Users/u", ".aulo").toString(), auloHome(mapOf("USERPROFILE" to "C:/Users/u")))
        assertNull(auloHome(emptyMap()))

        write(homeFile, "[voice]\nbarge_in = \"continue\"\n")
        val viaEnv = loadConfig(LoadOptions(env = mapOf("AULO_HOME" to home)))
        assertEquals(BargeIn.CONTINUE, viaEnv.config.voice.bargeIn)
    }

    @Test
    fun anEmptyFileIsNoLayer() {
        write(homeFile, "")
        assertEquals(Config(), load().config)
    }
}
