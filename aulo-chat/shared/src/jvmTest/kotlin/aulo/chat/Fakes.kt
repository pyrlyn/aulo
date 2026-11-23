package aulo.chat

import app.cash.sqldelight.driver.jdbc.sqlite.JdbcSqliteDriver
import aulo.chat.data.ChatCache
import aulo.chat.data.ChatRemote
import aulo.chat.data.Submission
import aulo.chat.data.TurnEnd
import aulo.chat.data.TurnEvent
import aulo.chat.data.cache.ChatDatabase
import aulo.chat.domain.ChatMessage
import aulo.chat.domain.ChatSummary
import aulo.chat.domain.MessageRole
import aulo.chat.domain.ModelId
import aulo.chat.domain.ModelInfo
import aulo.chat.domain.OfflineException
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.channelFlow
import kotlinx.coroutines.launch
import java.util.Properties
import kotlin.coroutines.CoroutineContext

val ECHO = ModelId("stub", "echo")
val SLOW = ModelId("stub", "slow")

fun memoryCache(context: CoroutineContext) =
    ChatCache(JdbcSqliteDriver(JdbcSqliteDriver.IN_MEMORY, Properties(), ChatDatabase.Schema), context)

/**
 * A server the test drives by hand: it records what the client submits and answers only when
 * the test says so, which makes streaming, stopping and losing the connection deterministic.
 */
class FakeRemote : ChatRemote {
    var reachable = true
    val chats = mutableListOf<ChatSummary>()
    val messages = mutableMapOf<String, MutableList<ChatMessage>>()
    val submissions = mutableListOf<Submission>()
    private var events = Channel<TurnEvent>(Channel.UNLIMITED)
    private var created = 0

    fun seedChat(id: String, title: String, vararg texts: String): ChatSummary {
        val chat = ChatSummary(id, title, ECHO, updatedAtMs = 1_000L + chats.size)
        chats.add(0, chat)
        messages[id] = texts.mapIndexed { i, text ->
            ChatMessage("$id-m$i", id, if (i % 2 == 0) MessageRole.User else MessageRole.Assistant, text)
        }.toMutableList()
        return chat
    }

    suspend fun emit(event: TurnEvent) = events.send(event)

    suspend fun answer(turnId: String, vararg tokens: String, end: TurnEnd = TurnEnd.Completed, error: String? = null) {
        emit(TurnEvent.Started(turnId))
        tokens.forEach { emit(TurnEvent.Delta(turnId, it)) }
        emit(TurnEvent.Finished(turnId, end, error))
    }

    /** The connection drops: the open stream fails and later calls find nobody home. */
    fun disconnect() {
        reachable = false
        events.close(OfflineException("The server cannot be reached."))
        events = Channel(Channel.UNLIMITED)
    }

    private fun check() {
        if (!reachable) throw OfflineException("The server cannot be reached.")
    }

    override suspend fun listChats(limit: Int): List<ChatSummary> {
        check()
        return chats.take(limit)
    }

    override suspend fun createChat(): ChatSummary {
        check()
        return seedChat("new-${++created}", "")
    }

    override suspend fun listMessages(chatId: String): List<ChatMessage> {
        check()
        return messages[chatId].orEmpty()
    }

    override suspend fun listModels(): List<ModelInfo> {
        check()
        return listOf(ModelInfo(ECHO, "Stub Echo", true), ModelInfo(SLOW, "Stub Slow", true))
    }

    override fun converse(chatId: String, submissions: Flow<Submission>): Flow<TurnEvent> = channelFlow {
        check()
        val source = events
        launch { submissions.collect { this@FakeRemote.submissions += it } }
        for (event in source) send(event)
    }
}
