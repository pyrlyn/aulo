package aulo.chat.presentation

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import aulo.chat.domain.Attachment
import aulo.chat.domain.AttachmentKind
import aulo.chat.domain.AttachmentPicker
import aulo.chat.domain.ChatMessage
import aulo.chat.domain.ChatRepository
import aulo.chat.domain.ChatSession
import aulo.chat.domain.ChatSummary
import aulo.chat.domain.ConnectionState
import aulo.chat.domain.LoadModels
import aulo.chat.domain.MessageRole
import aulo.chat.domain.MessageTooLongException
import aulo.chat.domain.ModelId
import aulo.chat.domain.ModelInfo
import aulo.chat.domain.Notice
import aulo.chat.domain.NoticeLevel
import aulo.chat.domain.OfflineException
import aulo.chat.domain.OpenChat
import aulo.chat.domain.SendMessage
import aulo.chat.domain.StopGeneration
import aulo.chat.domain.SwitchModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class ChatUiState(
    val chat: ChatSummary? = null,
    val messages: List<ChatMessage> = emptyList(),
    val input: String = "",
    val attachments: List<Attachment> = emptyList(),
    val models: List<ModelInfo> = emptyList(),
    val connection: ConnectionState = ConnectionState.Connecting,
    val generating: Boolean = false,
    val notice: Notice? = null,
    val loading: Boolean = true,
) {
    val online: Boolean get() = connection is ConnectionState.Online

    val canSend: Boolean
        get() = online && chat != null && !generating && (input.isNotBlank() || attachments.isNotEmpty())

    /** Nothing to show yet and nothing on the way: the screen offers suggestions instead. */
    val empty: Boolean get() = !loading && messages.isEmpty()
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
) : ViewModel() {
    private val mutable = MutableStateFlow(ChatUiState())
    val state: StateFlow<ChatUiState> = mutable.asStateFlow()

    private var session: ChatSession? = null
    private var sessionJob: Job? = null

    init {
        viewModelScope.launch {
            repository.connection.collect { connection -> mutable.update { it.copy(connection = connection) } }
        }
        viewModelScope.launch {
            // A chat gets its title with its first message; the cache is where that shows up.
            repository.chats.collect { chats ->
                mutable.update { state -> state.copy(chat = chats.firstOrNull { it.id == state.chat?.id } ?: state.chat) }
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
                show(repository.newChat())
            } catch (e: OfflineException) {
                tell(NoticeLevel.Warning, e.message)
            }
        }
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
