package aulo.store

import aulo.types.BotId
import aulo.types.ChatId
import fs.TempDir
import fs.entryKind
import kotlinx.io.files.Path
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

internal fun withStore(block: (Store, TempDir) -> Unit) =
    TempDir.create(".aulo-store-test").use { dir -> Store.open(databasePath(dir.path).toString()).use { block(it, dir) } }

class StoreTest {
    @Test
    fun theDatabaseLivesInTheHome() {
        assertEquals(Path("/h", "aulo.db"), databasePath(Path("/h")))
    }

    @Test
    fun idsAreTheContractsUlids() =
        withStore { s, _ ->
            val bot = s.createBot("b")
            val chat = s.createChat(bot.id, "t", "m")
            // The typed ids reject anything but a canonical ULID.
            assertEquals(bot.id, BotId(bot.id).value)
            assertEquals(chat.id, ChatId(chat.id).value)
        }

    @Test
    fun reopeningKeepsData() {
        TempDir.create(".aulo-store-test").use { dir ->
            val path = databasePath(dir.path).toString()
            val id = Store.open(path).use { it.createChat(null, "t", "m").id }
            assertNotNull(Store.open(path).use { it.getChat(id) })
        }
    }

    @Test
    fun chatCrud() =
        withStore { s, _ ->
            val bot = s.createBot("helper")
            val chat = s.createChat(bot.id, "first", "model-a")
            assertEquals(chat, s.getChat(chat.id))
            assertNull(s.getChat("missing"))

            assertTrue(s.setChatModel(chat.id, "anthropic/claude-sonnet-5"))
            assertFalse(s.setChatModel("missing", "openai/gpt-5.5"))
            val switched = s.getChat(chat.id)!!
            assertEquals("anthropic/claude-sonnet-5", switched.model)
            assertTrue(switched.updatedAt >= chat.updatedAt)

            assertTrue(s.renameChat(chat.id, "renamed"))
            assertFalse(s.renameChat("missing", "x"))
            assertEquals("renamed", s.getChat(chat.id)!!.title)

            val other = s.createChat(null, "second", "model-b")
            s.appendMessage(chat.id, "user", "bump")
            val ids = s.listChats(10).map { it.id }
            assertEquals(2, ids.size)
            assertTrue(other.id in ids)
            assertEquals(1, s.listChats(1, 1).size)

            assertTrue(s.deleteChat(chat.id))
            assertFalse(s.deleteChat(chat.id))
            assertNull(s.getChat(chat.id))
        }

    @Test
    fun aNegativeLimitIsRefusedRatherThanReadingEverything() =
        withStore { s, _ ->
            // SQLite reads a negative LIMIT as no limit at all.
            assertFailsWith<IllegalArgumentException> { s.listChats(-1) }
            assertFailsWith<IllegalArgumentException> { s.listChatsPage(null, null, -1) }
            assertFailsWith<IllegalArgumentException> { s.listMessages("c", null, -1) }
        }

    @Test
    fun messagesPaginateInInsertionOrder() =
        withStore { s, _ ->
            val chat = s.createChat(null, "t", "m")
            // Five inserts land in the same millisecond more often than not; ids must still sort.
            val want = (0 until 5).map { s.appendMessage(chat.id, "user", "m$it").id }

            val page1 = s.listMessages(chat.id, null, 2)
            val page2 = s.listMessages(chat.id, page1[1].id, 2)
            val page3 = s.listMessages(chat.id, page2[1].id, 2)
            assertEquals(want, (page1 + page2 + page3).map { it.id })
            assertEquals(1, page3.size)
            assertTrue(s.listMessages(chat.id, want[4], 2).isEmpty())
            assertEquals(listOf("m0", "m1"), page1.map { it.content })
        }

    @Test
    fun appendMessageRejectsAnUnknownChat() =
        withStore { s, _ ->
            // Fails only if foreign_keys is on for this connection.
            val e = assertFailsWith<StoreException> { s.appendMessage("missing", "user", "x") }
            assertTrue(e.isMissingReference, e.message)
        }

    @Test
    fun deletingAChatCascades() =
        withStore { s, _ ->
            val chat = s.createChat(null, "t", "m")
            val msg = s.appendMessage(chat.id, "assistant", "hi")
            s.recordToolCall(msg.id, "search", "{}", "ok", "ok")
            s.recordUsage(chat.id, "p", "m", 10, 20)
            assertEquals(1, s.listToolCalls(msg.id).size)
            assertEquals(20, s.listUsage(chat.id).single().outputTokens)

            s.deleteChat(chat.id)
            assertTrue(s.listMessages(chat.id, null, 10).isEmpty())
            assertTrue(s.listToolCalls(msg.id).isEmpty())
            assertTrue(s.listUsage(chat.id).isEmpty())
        }

    @Test
    fun deletingABotKeepsItsChats() =
        withStore { s, _ ->
            val bot = s.createBot("b")
            val chat = s.createChat(bot.id, "t", "m")
            assertTrue(s.deleteBot(bot.id))
            assertFalse(s.deleteBot(bot.id))
            assertNull(s.getChat(chat.id)!!.botId)
            assertTrue(s.listBots().isEmpty())
        }

    @Test
    fun botNamesAreUnique() =
        withStore { s, _ ->
            s.createBot("b")
            val e = assertFailsWith<StoreException> { s.createBot("b") }
            assertFalse(e.isMissingReference)
            assertEquals(listOf("b"), s.listBots().map { it.name })
        }

    @Test
    fun theDatabaseRunsInWalMode() =
        withStore { _, dir ->
            // The -wal sidecar exists only while a WAL-mode connection is open.
            assertNotNull(entryKind(Path(dir.path, "aulo.db-wal")))
        }

    @Test
    fun chatsPaginateByKeysetEvenWhenActivityMoves() =
        withStore { s, _ ->
            val bot = s.createBot("b")
            val ids = (0 until 5).map { s.createChat(if (it % 2 == 0) bot.id else null, "t", "m").id }
            val all = s.listChats(10)

            val page1 = s.listChatsPage(null, null, 2)
            assertEquals(all.subList(0, 2), page1)
            // A newer chat appearing mid-pagination must not shift the next page.
            s.createChat(null, "late", "m")
            val page2 = s.listChatsPage(null, ChatCursor(page1[1]), 2)
            assertEquals(all.subList(2, 4), page2)
            val page3 = s.listChatsPage(null, ChatCursor(page2[1]), 2)
            assertEquals(all.subList(4, 5), page3)

            val ofBot = s.listChatsPage(bot.id, null, 10).map { it.id }.toSet()
            assertEquals(setOf(ids[0], ids[2], ids[4]), ofBot)
        }

    @Test
    fun toolCallsLoadForManyMessagesAtOnce() =
        withStore { s, _ ->
            val chat = s.createChat(null, "t", "m")
            val (a, b, c) = listOf("a", "b", "c").map { s.appendMessage(chat.id, "assistant", it) }
            s.recordToolCall(a.id, "one", "{}", null, "running")
            s.recordToolCall(b.id, "two", "{}", null, "running")
            s.recordToolCall(c.id, "three", "{}", null, "running")
            assertEquals(listOf("one", "two"), s.listToolCallsFor(listOf(a.id, b.id)).map { it.name })
            assertTrue(s.listToolCallsFor(emptyList()).isEmpty())
        }

    @Test
    fun anUnknownBotIsAMissingReference() =
        withStore { s, _ ->
            val e = assertFailsWith<StoreException> { s.createChat("missing", "t", "m") }
            assertTrue(e.isMissingReference, e.message)
        }

    @Test
    fun aTurnAppendsItsMessagesAndCallsTogether() =
        withStore { s, _ ->
            val chat = s.createChat(null, "t", "m")
            s.appendMessage(chat.id, "user", "old")
            val turn =
                listOf(
                    NewMessage(
                        role = "assistant",
                        content = "looking",
                        wire = "{}",
                        toolCalls = listOf(NewToolCall("shell", "{}", "a.txt", "succeeded")),
                    ),
                    NewMessage(role = "tool"),
                )
            s.appendTurn(chat.id, turn)

            val history = s.chatHistory(chat.id)
            assertEquals(listOf("user" to null, "assistant" to "{}", "tool" to null), history.map { it.role to it.wire })
            val calls = s.listToolCalls(history[1].id)
            assertEquals("a.txt", calls.single().output)
        }

    @Test
    fun aTurnForAnUnknownChatWritesNothing() =
        withStore { s, _ ->
            val e = assertFailsWith<StoreException> { s.appendTurn("missing", listOf(NewMessage("user", "hi"))) }
            assertTrue(e.isMissingReference, e.message)
            assertTrue(s.chatHistory("missing").isEmpty())
        }
}
