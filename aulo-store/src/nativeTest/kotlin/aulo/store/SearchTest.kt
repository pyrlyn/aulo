package aulo.store

import fs.TempDir
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

// Raw SQL here only stands in for writes the store does not offer (editing or deleting one
// message), to prove the index triggers follow them.
class SearchTest {
    @Test
    fun findsAMessageByAWordWithCaseAndDiacriticsFolded() =
        withStore { s, _ ->
            val chat = s.createChat(null, "t", "m")
            val hit = s.appendMessage(chat.id, "user", "Plan the Café trip")
            s.appendMessage(chat.id, "user", "unrelated")

            val found = s.search("cafe")
            assertEquals(listOf(hit), found.map { it.message })
            assertEquals(emptyList(), s.search("absent"))
        }

    @Test
    fun everyWordMustMatch() =
        withStore { s, _ ->
            val chat = s.createChat(null, "t", "m")
            val both = s.appendMessage(chat.id, "user", "red apple pie")
            s.appendMessage(chat.id, "user", "red car")
            assertEquals(listOf(both.id), s.search("red apple").ids())
        }

    @Test
    fun anEditedMessageIsFoundByItsNewWordOnly() =
        withRawAccess { s, raw ->
            val chat = s.createChat(null, "t", "m")
            val msg = s.appendMessage(chat.id, "user", "alpha")
            raw("UPDATE messages SET content = 'beta' WHERE id = ?", msg.id)
            assertEquals(emptyList(), s.search("alpha"))
            assertEquals(listOf(msg.id), s.search("beta").ids())
        }

    @Test
    fun deletedMessagesAndChatsVanish() =
        withRawAccess { s, raw ->
            val keep = s.createChat(null, "t", "m")
            val gone = s.createChat(null, "t", "m")
            val kept = s.appendMessage(keep.id, "user", "needle one")
            val single = s.appendMessage(keep.id, "user", "needle two")
            s.appendMessage(gone.id, "user", "needle three")

            raw("DELETE FROM messages WHERE id = ?", single.id)
            assertEquals(2, s.search("needle").size)
            assertTrue(s.deleteChat(gone.id))
            assertEquals(listOf(kept.id), s.search("needle").ids())
        }

    @Test
    fun aChatFilterLimitsResults() =
        withStore { s, _ ->
            val a = s.createChat(null, "t", "m")
            val b = s.createChat(null, "t", "m")
            val inA = s.appendMessage(a.id, "user", "shared word")
            s.appendMessage(b.id, "user", "shared word")
            assertEquals(listOf(inA.id), s.searchMessages("shared", a.id, null, 10).ids())
            assertEquals(2, s.search("shared").size)
        }

    @Test
    fun theMigrationIndexesMessagesThatAlreadyExist() =
        withStore { s, _ ->
            val chat = s.createChat(null, "t", "m")
            val old = s.appendMessage(chat.id, "user", "legacy words")
            assertEquals(1, s.migrator.down(1).size)
            // While the index is gone, no trigger indexes new rows.
            val late = s.appendMessage(chat.id, "user", "legacy late")
            s.migrator.up()
            assertEquals(listOf(old.id, late.id).sorted(), s.search("legacy").ids().sorted())
        }

    @Test
    fun hostileQueriesArePlainWordsNotErrors() =
        withStore { s, _ ->
            val chat = s.createChat(null, "t", "m")
            s.appendMessage(chat.id, "user", "this OR that and NEAR things content")
            val hostile =
                listOf(
                    "\"",
                    "\"\"\"",
                    "*",
                    "NEAR(",
                    "NEAR(a b)",
                    "OR",
                    "AND",
                    "NOT",
                    "-",
                    "- x",
                    "content:",
                    "col:",
                    "^x",
                    "(",
                    ")",
                    "a OR",
                    "{content}: x",
                    "\"unterminated",
                    "x*",
                    "\u0000",
                    "'; DROP TABLE messages; --",
                    "‮",
                    "",
                )
            for (q in hostile) s.search(q)
            // Operators are literal words: "OR" finds the message that contains the word.
            assertEquals(1, s.search("OR").size)
            assertEquals(1, s.search("content:").size)
            assertEquals(emptyList(), s.search("\""))
            assertEquals(1, s.listChats(10).size)
        }

    @Test
    fun theMatchExpressionQuotesEveryWordAndIsCapped() {
        assertEquals("\"a\" \"OR\" \"b\"", matchExpression("a OR b"))
        assertEquals("\"say\" \"\"\"hi\"\"\"", matchExpression("say \"hi\""))
        assertEquals("\"日本\" \"語\"", matchExpression("日本　語"))
        assertNull(matchExpression("\" - *"))
        assertNull(matchExpression("   "))
        assertEquals(MAX_TERMS, assertNotNull(matchExpression("w ".repeat(500))).split(' ').size)
        // A cut inside a multi-byte character keeps whole characters only.
        val cut = assertNotNull(matchExpression("é".repeat(MAX_QUERY_BYTES)))
        assertEquals("\"${"é".repeat(MAX_QUERY_BYTES / 2)}\"", cut)
        assertEquals("\"${"語".repeat(MAX_QUERY_BYTES / 3)}\"", matchExpression("語".repeat(MAX_QUERY_BYTES)))
    }

    @Test
    fun pagesCoverEveryHitOnceBestFirst() =
        withStore { s, _ ->
            val chat = s.createChat(null, "t", "m")
            // Different lengths give different bm25 ranks; equal ones exercise the id tie-break.
            repeat(7) { i -> s.appendMessage(chat.id, "user", "page " + "pad ".repeat(i % 4)) }
            val all = s.search("page", limit = 100)
            assertEquals(7, all.size)
            assertTrue(all.zipWithNext().all { (a, b) -> a.rank < b.rank || (a.rank == b.rank && a.message.id < b.message.id) })

            val seen = mutableListOf<String>()
            var after: SearchCursor? = null
            while (true) {
                val page = s.searchMessages("page", null, after, 3)
                if (page.isEmpty()) break
                after = SearchCursor(page.last())
                seen += page.ids()
            }
            assertEquals(all.ids(), seen)
            assertFailsWith<IllegalArgumentException> { s.searchMessages("page", null, null, -1) }
        }

    private fun Store.search(
        query: String,
        limit: Int = 10,
    ) = searchMessages(query, null, null, limit)

    private fun List<SearchHit>.ids() = map { it.message.id }

    /** A store plus a second connection to the same file for writes the store does not offer. */
    private fun withRawAccess(block: (Store, (String, String) -> Unit) -> Unit) =
        TempDir.create(".aulo-store-test").use { dir ->
            val path = databasePath(dir.path).toString()
            Store.open(path).use { s ->
                sqlite.open(path).use { db -> block(s) { sql, arg -> db.execute(sql, arg) } }
            }
        }
}
