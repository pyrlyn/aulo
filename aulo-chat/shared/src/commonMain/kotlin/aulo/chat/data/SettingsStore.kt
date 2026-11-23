package aulo.chat.data

import app.cash.sqldelight.coroutines.asFlow
import app.cash.sqldelight.coroutines.mapToList
import app.cash.sqldelight.db.SqlDriver
import aulo.chat.data.cache.ChatDatabase
import aulo.chat.domain.Appearance
import aulo.chat.domain.ModelId
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withContext
import kotlin.coroutines.CoroutineContext

/** What Settings holds. Null means "not chosen": the launcher's server, the server's model. */
data class AppSettings(
    val endpoint: ServerEndpoint? = null,
    val appearance: Appearance = Appearance.System,
    val defaultModel: ModelId? = null,
)

/** Settings on this device, in the cache database but outside the cache: clearing it keeps them. */
class SettingsStore(driver: SqlDriver, private val context: CoroutineContext = Dispatchers.Default) {
    private val queries = ChatDatabase(driver).settingQueries

    val settings: Flow<AppSettings> =
        queries.selectSettings().asFlow().mapToList(context).map { rows -> decode(rows.associate { it.key to it.setting_value }) }

    /** Read once at start, before anything is shown, to pick the server to connect to. */
    fun current(): AppSettings = decode(queries.selectSettings().executeAsList().associate { it.key to it.setting_value })

    suspend fun setEndpoint(endpoint: ServerEndpoint) = put(ENDPOINT, endpoint.address())

    suspend fun setAppearance(appearance: Appearance) = put(APPEARANCE, appearance.name)

    suspend fun setDefaultModel(model: ModelId?) = withContext(context) {
        if (model == null) queries.deleteSetting(DEFAULT_MODEL) else queries.upsertSetting(DEFAULT_MODEL, model.toString())
    }

    private suspend fun put(key: String, value: String) = withContext(context) { queries.upsertSetting(key, value) }

    // Rows are read back leniently: one a newer build wrote, or a damaged one, falls back to
    // the default instead of stopping the app at start.
    private fun decode(values: Map<String, String>) = AppSettings(
        endpoint = values[ENDPOINT]?.let(::parseAddress),
        appearance = Appearance.entries.firstOrNull { it.name == values[APPEARANCE] } ?: Appearance.System,
        defaultModel = values[DEFAULT_MODEL]?.split('/', limit = 2)?.takeIf { it.size == 2 && it.none(String::isBlank) }
            ?.let { ModelId(it[0], it[1]) },
    )

    private companion object {
        const val ENDPOINT = "endpoint"
        const val APPEARANCE = "appearance"
        const val DEFAULT_MODEL = "default_model"
    }
}

/**
 * `host:port` as typed in Settings or on the Connect screen; null when it is not one. IPv6
 * literals take brackets (`[::1]:50051`), as in a URL.
 */
fun parseAddress(text: String): ServerEndpoint? {
    val trimmed = text.trim()
    val colon = trimmed.lastIndexOf(':')
    if (colon <= 0 || colon == trimmed.lastIndex) return null
    val host = trimmed.substring(0, colon).removeSurrounding("[", "]")
    if (host.isBlank() || host.any { it.isWhitespace() || it == '/' }) return null
    val port = trimmed.substring(colon + 1).toIntOrNull()?.takeIf { it in 1..65535 } ?: return null
    return ServerEndpoint(host, port)
}

/** The inverse of [parseAddress]. */
fun ServerEndpoint.address(): String = if (':' in host) "[$host]:$port" else "$host:$port"
