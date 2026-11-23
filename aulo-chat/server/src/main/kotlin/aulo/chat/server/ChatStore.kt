package aulo.chat.server

import kotlin.random.Random

data class ModelId(val provider: String, val model: String) {
    override fun toString(): String = "$provider/$model"
}

enum class Role { USER, ASSISTANT }

data class StoredChat(
    val id: String,
    val title: String,
    val model: ModelId,
    val createdAtMs: Long,
    val updatedAtMs: Long,
)

data class StoredMessage(
    val id: String,
    val chatId: String,
    val role: Role,
    val content: String,
    val createdAtMs: Long,
)

/**
 * Chats and their messages, kept in memory: this is the dialog memory the stub model reads.
 * Nothing survives a restart, which is what a development stand-in should do.
 */
class ChatStore(
    private val clock: () -> Long = System::currentTimeMillis,
    private val random: Random = Random.Default,
) {
    private val lock = Any()
    private val chats = LinkedHashMap<String, StoredChat>()
    private val messages = HashMap<String, MutableList<StoredMessage>>()

    fun newId(): String = ulid(clock(), random)

    fun createChat(title: String, model: ModelId): StoredChat = synchronized(lock) {
        val now = clock()
        val chat = StoredChat(newId(), title, model, now, now)
        chats[chat.id] = chat
        messages[chat.id] = mutableListOf()
        chat
    }

    /** Most recently updated first, as a conversation list shows them. */
    fun listChats(): List<StoredChat> = synchronized(lock) {
        chats.values.sortedByDescending { it.updatedAtMs }
    }

    fun chat(id: String): StoredChat? = synchronized(lock) { chats[id] }

    fun rename(id: String, title: String): StoredChat? = update(id) { it.copy(title = title) }

    fun setModel(id: String, model: ModelId): StoredChat? = update(id) { it.copy(model = model) }

    fun delete(id: String): Boolean = synchronized(lock) {
        messages.remove(id)
        chats.remove(id) != null
    }

    fun messages(chatId: String): List<StoredMessage> = synchronized(lock) {
        messages[chatId]?.toList().orEmpty()
    }

    /** Returns null when the chat was deleted while the turn was running. */
    fun append(chatId: String, role: Role, content: String): StoredMessage? = synchronized(lock) {
        val list = messages[chatId] ?: return null
        val message = StoredMessage(newId(), chatId, role, content, clock())
        list += message
        chats.computeIfPresent(chatId) { _, chat ->
            // The daemon's cheap tier names an untitled chat after the first turn; the stand-in
            // uses the start of the first user message instead.
            val title = chat.title.ifEmpty { if (role == Role.USER) content.titleFromText() else "" }
            chat.copy(title = title, updatedAtMs = message.createdAtMs)
        }
        message
    }

    private fun update(id: String, change: (StoredChat) -> StoredChat): StoredChat? = synchronized(lock) {
        chats.computeIfPresent(id) { _, chat -> change(chat).copy(updatedAtMs = clock()) }
    }
}

const val MAX_TITLE_BYTES = 200

private fun String.titleFromText(): String {
    val line = lineSequence().firstOrNull { it.isNotBlank() }?.trim().orEmpty()
    return line.take(48).truncateUtf8(MAX_TITLE_BYTES)
}

/** Cuts on a character boundary so a multi-byte title never ends in half a code point. */
internal fun String.truncateUtf8(maxBytes: Int): String {
    var end = length
    while (end > 0 && substring(0, end).toByteArray().size > maxBytes) end--
    if (end in 1 until length && Character.isHighSurrogate(this[end - 1])) end--
    return substring(0, end)
}

private const val CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"

/**
 * A ULID: 48 bits of time and 80 random bits in Crockford base32. Written here because the
 * stand-in needs twenty lines of it and no shared Kotlin module has one yet.
 */
internal fun ulid(timeMs: Long, random: Random): String {
    val out = CharArray(26)
    var time = timeMs
    for (i in 9 downTo 0) {
        out[i] = CROCKFORD[(time and 31).toInt()]
        time = time ushr 5
    }
    for (i in 10 until 26) out[i] = CROCKFORD[random.nextInt(32)]
    return String(out)
}
