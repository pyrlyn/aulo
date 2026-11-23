package aulo.config

import aulo.telemetry.LogLevel
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonClassDiscriminator

// Every section has defaults for every key, so a layer names only what it changes. Keys are
// snake_case as in the earlier Rust config, so existing files keep working.

@Serializable
@Doc("Root of `config.toml` and `.aulo.toml`.")
data class Config(
    val daemon: DaemonConfig = DaemonConfig(),
    val providers: ProvidersConfig = ProvidersConfig(),
    val voice: VoiceConfig = VoiceConfig(),
    val mcp: McpConfig = McpConfig(),
    val plugins: PluginsConfig = PluginsConfig(),
    val policy: PolicyConfig = PolicyConfig(),
    val log: LogConfig = LogConfig(),
)

@Serializable
@Doc("`aulod` transport.")
data class DaemonConfig(
    @Doc("TCP address for remote clients. Unset: the owner-only local socket only.")
    val listen: String? = null,
    @Doc(
        "PEM certificate chain for `listen`; a relative path is in the aulo home. `listen` always serves TLS: " +
            "with neither this nor `tls_key` set, aulod makes a self-signed certificate once and logs its SHA-256 to pin.",
    )
    @SerialName("tls_cert")
    val tlsCert: String? = null,
    @Doc("PEM private key for `tls_cert`; set both or neither.")
    @SerialName("tls_key")
    val tlsKey: String? = null,
)

@Serializable
@Doc("Model providers and the tiers that route to them. API keys are not configuration: they live in the OS keychain.")
data class ProvidersConfig(
    @Doc("Named provider endpoints that tiers refer to.")
    val endpoints: Map<String, ProviderConfig> = emptyMap(),
    val tiers: TiersConfig = TiersConfig(),
)

@Serializable
data class ProviderConfig(
    val kind: ProviderKind,
    @Doc("Overrides the provider's default endpoint; required for `openai-compatible`.")
    @SerialName("base_url")
    val baseUrl: String? = null,
)

@Serializable
enum class ProviderKind(
    // The conventional environment variable of a hosted provider; local servers have none.
    val apiKeyEnv: String?,
) {
    @SerialName("runa")
    RUNA(null),

    @SerialName("ollama")
    OLLAMA(null),

    @SerialName("lm-studio")
    LM_STUDIO(null),

    @SerialName("llama-cpp")
    LLAMA_CPP(null),

    @SerialName("openai")
    OPENAI("OPENAI_API_KEY"),

    @SerialName("anthropic")
    ANTHROPIC("ANTHROPIC_API_KEY"),

    @SerialName("gemini")
    GEMINI("GEMINI_API_KEY"),

    @SerialName("xai")
    XAI("XAI_API_KEY"),

    @SerialName("openrouter")
    OPENROUTER("OPENROUTER_API_KEY"),

    @SerialName("deepseek")
    DEEPSEEK("DEEPSEEK_API_KEY"),

    @SerialName("openai-compatible")
    OPENAI_COMPATIBLE(null),
}

@Serializable
@Doc("One provider and model per tier; an unset tier is not configured.")
data class TiersConfig(
    @Doc("Fast short answers, or the realtime front.")
    val voice: TierConfig? = null,
    @Doc("Tool use.")
    val main: TierConfig? = null,
    @Doc("The reviewer of consequential actions.")
    val sentinel: TierConfig? = null,
    @Doc("Titles and summaries.")
    val cheap: TierConfig? = null,
)

@Serializable
data class TierConfig(
    @Doc("Name of an entry in `providers.endpoints`.")
    val provider: String,
    val model: String,
)

@Serializable
data class VoiceConfig(
    val listen: ListenMode = ListenMode.PUSH_TO_TALK,
    @SerialName("barge_in")
    val bargeIn: BargeIn = BargeIn.INTERRUPT,
    val stt: EngineConfig = EngineConfig(),
    val tts: EngineConfig = EngineConfig(),
)

@Serializable
enum class ListenMode {
    @SerialName("push-to-talk")
    PUSH_TO_TALK,

    @SerialName("wake-word")
    WAKE_WORD,
}

@Serializable
@Doc("What speaking over aulo does to the turn in progress.")
enum class BargeIn {
    // Stop playback and cancel the turn.
    @SerialName("interrupt")
    INTERRUPT,

    // Stop playback; the turn keeps running.
    @SerialName("continue")
    CONTINUE,
}

@Serializable
@Doc("Shape shared by `[voice.stt]` and `[voice.tts]`.")
data class EngineConfig(
    val engine: String? = null,
    val model: String? = null,
    val voice: String? = null,
    val rate: Float? = null,
    @Doc("Engines tried in order when the chosen one fails.")
    val fallback: List<String> = emptyList(),
)

@Serializable
data class McpConfig(
    @Doc("MCP servers aulo connects to, by name.")
    val servers: Map<String, McpServer> = emptyMap(),
)

@OptIn(ExperimentalSerializationApi::class)
@Serializable
@JsonClassDiscriminator(McpServer.DISCRIMINATOR)
sealed interface McpServer {
    @Serializable
    @SerialName("stdio")
    data class Stdio(
        val command: String,
        val args: List<String> = emptyList(),
    ) : McpServer

    @Serializable
    @SerialName("http")
    data class Http(
        val url: String,
    ) : McpServer

    companion object {
        const val DISCRIMINATOR = "transport"
    }
}

@Serializable
data class PluginsConfig(
    @Doc("Extra directories scanned for plugins.")
    val dirs: List<String> = emptyList(),
    @Doc("Plugin ids that are never loaded.")
    val disabled: List<String> = emptyList(),
)

@Serializable
data class PolicyConfig(
    @Doc("Rules in the perm-rules grammar. Evaluation order is deny, allow, ask.")
    val deny: List<String> = emptyList(),
    val allow: List<String> = emptyList(),
    val ask: List<String> = emptyList(),
    @Doc("An approval request nobody answers within this time is denied.")
    @SerialName("approval_timeout_secs")
    @Minimum(1)
    val approvalTimeoutSecs: Long = 60,
) {
    init {
        require(approvalTimeoutSecs >= 1) { "policy.approval_timeout_secs must be at least 1" }
    }
}

@Serializable
@Doc("Log files in `\$AULO_HOME/logs`. Secrets are masked before any line is written.")
data class LogConfig(
    @Doc("Least severe level written: trace, debug, info, warn or error.")
    val level: LogLevel = LogLevel.INFO,
)
