package aulo.chat.presentation

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import aulo.chat.data.SettingsStore
import aulo.chat.data.SwitchableChatRemote
import aulo.chat.data.address
import aulo.chat.data.parseAddress
import aulo.chat.domain.Appearance
import aulo.chat.domain.ChatRepository
import aulo.chat.domain.ConnectionState
import aulo.chat.domain.LoadModels
import aulo.chat.domain.ModelId
import aulo.chat.domain.ModelInfo
import aulo.chat.domain.OfflineException
import aulo.chat.domain.ServerException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Where the server field stands. [Failed.reason] is shown under it in the danger colour. */
sealed interface ServerStatus {
    data object Connected : ServerStatus

    data object Connecting : ServerStatus

    data class Failed(val reason: String) : ServerStatus
}

data class SettingsUiState(
    /** What the user typed; the app keeps using [connectedAddress] until this one answers. */
    val address: String = "",
    val connectedAddress: String = "",
    val status: ServerStatus = ServerStatus.Connecting,
    val appearance: Appearance = Appearance.System,
    val defaultModel: ModelId? = null,
    val models: List<ModelInfo> = emptyList(),
) {
    val addressValid: Boolean get() = parseAddress(address) != null

    val canConnect: Boolean
        get() = addressValid && status != ServerStatus.Connecting &&
            (address.trim() != connectedAddress || status is ServerStatus.Failed)
}

/** Settings and the Connect screen: which server, which theme, which model new chats get. */
class SettingsViewModel(
    private val store: SettingsStore,
    private val remote: SwitchableChatRemote,
    private val repository: ChatRepository,
    private val loadModels: LoadModels,
) : ViewModel() {
    private val mutable = MutableStateFlow(
        remote.endpoint.address().let { SettingsUiState(address = it, connectedAddress = it) },
    )
    val state: StateFlow<SettingsUiState> = mutable.asStateFlow()

    init {
        viewModelScope.launch {
            store.settings.collect { saved ->
                mutable.update { it.copy(appearance = saved.appearance, defaultModel = saved.defaultModel) }
            }
        }
        viewModelScope.launch {
            repository.connection.collect { connection ->
                mutable.update {
                    it.copy(
                        status = when (connection) {
                            ConnectionState.Online -> ServerStatus.Connected
                            ConnectionState.Connecting -> ServerStatus.Connecting
                            is ConnectionState.Offline -> ServerStatus.Failed(connection.reason)
                        },
                    )
                }
                if (connection is ConnectionState.Online) mutable.update { it.copy(models = loadModels()) }
            }
        }
    }

    fun onAddressChange(text: String) = mutable.update { it.copy(address = text) }

    /**
     * Tries the typed server and switches to it only if it answers; [onConnected] then reopens
     * the chats from there.
     */
    fun connect(onConnected: () -> Unit) {
        val endpoint = parseAddress(mutable.value.address) ?: return
        mutable.update { it.copy(status = ServerStatus.Connecting) }
        viewModelScope.launch {
            try {
                remote.switchTo(endpoint)
            } catch (e: OfflineException) {
                return@launch fail(endpoint.address(), e.message)
            } catch (e: ServerException) {
                return@launch fail(endpoint.address(), e.message)
            }
            store.setEndpoint(endpoint)
            mutable.update { it.copy(address = endpoint.address(), connectedAddress = endpoint.address()) }
            onConnected()
        }
    }

    private fun fail(address: String, reason: String?) = mutable.update {
        it.copy(status = ServerStatus.Failed("Cannot reach $address: ${reason ?: "no answer"}"))
    }

    fun setAppearance(appearance: Appearance) {
        viewModelScope.launch { store.setAppearance(appearance) }
    }

    fun setDefaultModel(model: ModelId?) {
        viewModelScope.launch { store.setDefaultModel(model) }
    }

    /** Only after the confirmation dialog; [onCleared] reloads what the server has. */
    fun clearCache(onCleared: () -> Unit) {
        viewModelScope.launch {
            repository.clearCache()
            onCleared()
        }
    }
}
