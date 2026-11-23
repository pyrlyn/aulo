package aulo.chat.presentation

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import aulo.chat.data.SettingsStore
import aulo.chat.domain.Attachment
import aulo.chat.domain.AttachmentKind
import aulo.chat.domain.AttachmentPicker
import aulo.chat.domain.ChatMessage
import aulo.chat.domain.ChatRepository
import aulo.chat.domain.ChatSession
import aulo.chat.domain.ChatSummary
import aulo.chat.domain.ConnectionState
import aulo.chat.domain.LoadModels
import aulo.chat.domain.MessageHit
import aulo.chat.domain.MessageRole
import aulo.chat.domain.MessageTooLongException
import aulo.chat.domain.ModelId
import aulo.chat.domain.ModelInfo
import aulo.chat.domain.Notice
import aulo.chat.domain.NoticeLevel
import aulo.chat.domain.OfflineException
import aulo.chat.domain.OpenChat
import aulo.chat.domain.SendMessage
import aulo.chat.domain.ServerException
import aulo.chat.domain.StopGeneration
import aulo.chat.domain.SwitchModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class ChatUiState(
    val chat: ChatSummary? = null,
    val chats: List<ChatSummary> = emptyList(),
    val messages: List<ChatMessage> = emptyList(),
    val input: String = "",
    val attachments: List<Attachment> = emptyList(),
    val models: List<ModelInfo> = emptyList(),
    val connection: ConnectionState = ConnectionState.Connecting,
    val generating: Boolean = false,
    val notice: Notice? = null,
    val loading: Boolean = true,
    val search: SearchState = SearchState(),
) {
    val online: Boolean get() = connection is ConnectionState.Online

    val canSend: Boolean
        get() = online && chat != null && !generating && (input.isNotBlank() || attachments.isNotEmpty())

    /** Nothing to show yet and nothing on the way: the screen offers suggestions instead. */
    val empty: Boolean get() = !loading && messages.isEmpty()

    /** Never reached a server and nothing cached: the Connect screen is all there is to show. */
    val needsServer: Boolean get() = !loading && chat == null && connection is ConnectionState.Offline

    /** Chats whose title holds every word of the query, from the cache, so they show offline too. */
    val titleMatches: List<ChatSummary>
        get() {
            val words = search.query.lowercase().split(' ').filter { it.isNotBlank() }
            if (words.isEmpty()) return emptyList()
            return chats.filter { chat -> words.all { it in chat.title.lowercase() } }
        }
}

/** The sidebar's search. [hits] are message matches from the server; [error] is why there are none. */
data class SearchState(
    val query: String = "",
    val hits: List<MessageHit> = emptyList(),
    val searching: Boolean = false,
    val error: String? = null,
) {
    val active: Boolean get() = query.isNotBlank()
}

const val MAX_ATTACHMENTS = 6

/** The chat screen's state and the things the user can do on it. */
class ChatViewModel(
    private val repository: ChatRepository,
    private val openChat: OpenChat,
    private val loadModels: LoadModels,
    private val sendMessage: SendMessage,
    private val stopGeneration: StopGeneration,
    private val switchModel: SwitchModel,
    private val picker: AttachmentPicker,
    private val settings: SettingsStore,
) : ViewModel() {
    private val mutable = MutableStateFlow(ChatUiState())
    val state: StateFlow<ChatUiState> = mutable.asStateFlow()

    private var session: ChatSession? = null
    private var sessionJob: Job? = null
    private var searchJob: Job? = null

    init {
        viewModelScope.launch {
            repository.connection.collect { connection -> mutable.update { it.copy(connection = connection) } }
        }
        viewModelScope.launch {
            // A chat gets its title with its first message; the cache is where that shows up.
            repository.chats.collect { chats ->
                mutable.update { state ->
                    state.copy(chats = chats, chat = chats.firstOrNull { it.id == state.chat?.id } ?: state.chat)
                }
            }
        }
        connect()
    }

    /** Also the banner's Retry: tries the server again and reopens the chat. */
    fun connect() {
        viewModelScope.launch {
            mutable.update { it.copy(loading = it.chat == null) }
            val chat = openChat()
            show(chat)
            if (repository.connection.value is ConnectionState.Online) {
                val models = loadModels()
                mutable.update { it.copy(models = models) }
            }
        }
    }

    fun newChat() {
        viewModelScope.launch {
            try {
                show(repository.newChat(settings.settings.first().defaultModel))
            } catch (e: OfflineException) {
                tell(NoticeLevel.Warning, e.message)
            } catch (e: ServerException) {
                // A default model this server does not know: start with the server's own.
                tell(NoticeLevel.Warning, "${e.message}; the chat starts with the server's default model.")
                runCatching { show(repository.newChat()) }
            }
        }
    }

    /** Opens a chat from the list; offline it opens what the cache has. */
    fun openChat(chat: ChatSummary) {
        if (chat.id == mutable.value.chat?.id) return
        viewModelScope.launch {
            repository.refresh(chat)
            show(chat)
        }
    }

    fun rename(chat: ChatSummary, title: String) {
        if (title.isBlank()) return
        viewModelScope.launch {
            try {
                repository.rename(chat.id, title)
            } catch (e: OfflineException) {
                tell(NoticeLevel.Warning, e.message)
            } catch (e: ServerException) {
                tell(NoticeLevel.Error, "The chat was not renamed: ${e.message}")
            }
        }
    }

    /** Only after the confirmation dialog: deleting is never one click. */
    fun delete(chat: ChatSummary) {
        viewModelScope.launch {
            try {
                repository.delete(chat.id)
            } catch (e: OfflineException) {
                tell(NoticeLevel.Warning, e.message)
                return@launch
            } catch (e: ServerException) {
                tell(NoticeLevel.Error, "The chat was not deleted: ${e.message}")
                return@launch
            }
            if (mutable.value.chat?.id != chat.id) return@launch
            val next = mutable.value.chats.firstOrNull { it.id != chat.id }
            if (next != null) {
                repository.refresh(next)
                show(next)
            } else {
                newChat()
            }
        }
    }

    fun onQueryChange(query: String) {
        mutable.update { it.copy(search = it.search.copy(query = query, error = null)) }
        searchJob?.cancel()
        if (query.isBlank()) {
            mutable.update { it.copy(search = SearchState()) }
            return
        }
        searchJob = viewModelScope.launch {
            // Waits for a pause in typing so the server sees one query, not one per key.
            delay(SEARCH_DEBOUNCE_MS)
            if (!mutable.value.online) {
                mutable.update { it.copy(search = it.search.copy(hits = emptyList(), error = OFFLINE_SEARCH)) }
                return@launch
            }
            mutable.update { it.copy(search = it.search.copy(searching = true)) }
            val outcome = try {
                Result.success(repository.search(query))
            } catch (e: OfflineException) {
                Result.failure(e)
            } catch (e: ServerException) {
                Result.failure(e)
            }
            mutable.update { state ->
                if (state.search.query != query) return@update state
                state.copy(
                    search = state.search.copy(
                        hits = outcome.getOrDefault(emptyList()),
                        searching = false,
                        error = outcome.exceptionOrNull()?.let { "Message search failed: ${it.message}" },
                    ),
                )
            }
        }
    }

    fun clearQuery() = onQueryChange("")

    /** A message hit opens its chat. */
    fun openHit(hit: MessageHit) {
        val chat = mutable.value.chats.firstOrNull { it.id == hit.chatId } ?: return
        clearQuery()
        openChat(chat)
    }

    private fun show(chat: ChatSummary?) {
        session?.close()
        sessionJob?.cancel()
        session = null
        mutable.update { it.copy(chat = chat, messages = emptyList(), generating = false, loading = chat != null) }
        if (chat == null) return
        val opened = repository.open(chat, viewModelScope)
        session = opened
        sessionJob = viewModelScope.launch {
            launch { opened.messages.collect { list -> mutable.update { it.copy(messages = list, loading = false) } } }
            launch { opened.generating.collect { busy -> mutable.update { it.copy(generating = busy) } } }
            launch { opened.notices.collect { notice -> mutable.update { it.copy(notice = notice) } } }
        }
    }

    fun onInputChange(text: String) = mutable.update { it.copy(input = text) }

    fun send() {
        val current = mutable.value
        val active = session
        if (!current.canSend || active == null) return
        // Cleared at once so a second tap cannot send the same text twice.
        mutable.update { it.copy(input = "", attachments = emptyList(), notice = null) }
        viewModelScope.launch {
            try {
                val result = sendMessage(active, current.input, current.attachments)
                if (result.notSent.isNotEmpty()) {
                    tell(NoticeLevel.Info, "Images and voice notes are not sent yet; the text was.")
                }
            } catch (e: MessageTooLongException) {
                restore(current, NoticeLevel.Error, e.message)
            } catch (e: OfflineException) {
                restore(current, NoticeLevel.Warning, e.message)
            }
        }
    }

    /** Sends the user message that a failed or stopped answer belonged to once more. */
    fun retry(message: ChatMessage) {
        val messages = mutable.value.messages
        val failedAt = messages.indexOfFirst { it.id == message.id }
        val question = messages.take(failedAt.coerceAtLeast(0)).lastOrNull { it.role == MessageRole.User } ?: return
        mutable.update { it.copy(input = question.text) }
        send()
    }

    fun stop() {
        val active = session ?: return
        viewModelScope.launch { stopGeneration(active) }
    }

    fun selectModel(model: ModelId) {
        val active = session ?: return
        viewModelScope.launch {
            try {
                switchModel(active, model)
                mutable.update { it.copy(chat = it.chat?.copy(model = model)) }
            } catch (e: OfflineException) {
                tell(NoticeLevel.Warning, e.message)
            }
        }
    }

    fun attach(kind: AttachmentKind) {
        viewModelScope.launch {
            val picked = picker.pick(kind) ?: return@launch
            mutable.update {
                if (it.attachments.size >= MAX_ATTACHMENTS) {
                    it.copy(notice = Notice(NoticeLevel.Warning, "A message takes at most $MAX_ATTACHMENTS attachments."))
                } else {
                    it.copy(attachments = it.attachments + picked)
                }
            }
        }
    }

    fun removeAttachment(attachment: Attachment) =
        mutable.update { it.copy(attachments = it.attachments - attachment) }

    fun dismissNotice() = mutable.update { it.copy(notice = null) }

    private fun restore(before: ChatUiState, level: NoticeLevel, message: String?) = mutable.update {
        it.copy(input = before.input, attachments = before.attachments, notice = Notice(level, message ?: "Not sent."))
    }

    private fun tell(level: NoticeLevel, message: String?) =
        mutable.update { it.copy(notice = Notice(level, message ?: "Something went wrong.")) }

    override fun onCleared() {
        session?.close()
    }
}

const val SEARCH_DEBOUNCE_MS = 300L

const val OFFLINE_SEARCH = "Message search needs the server."
