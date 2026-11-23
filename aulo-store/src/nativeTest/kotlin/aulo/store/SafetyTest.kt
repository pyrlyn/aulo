package aulo.store

import fs.TempDir
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.time.Clock
import kotlin.time.Instant

class SafetyTest {
    @Test
    fun grantsArePutListedPagedAndRevoked() =
        withStore { s, _ ->
            val a = s.putGrant("app:Safari", "browse", "allow", null)
            val b = s.putGrant("app:Mail", "send", "deny", null)
            assertEquals(listOf(a, b), s.listGrants())

            assertTrue(s.revokeGrant(a.id))
            assertTrue(!s.revokeGrant(a.id))
            assertEquals(listOf(b), s.listGrants())

            val c = s.putGrant("app:Notes", "read", "allow", null)
            assertEquals(listOf(b), s.listGrantsPage(null, 1))
            assertEquals(listOf(c), s.listGrantsPage(b.id, 1))
            assertEquals(emptyList(), s.listGrantsPage(c.id, 10))
            assertFailsWith<IllegalArgumentException> { s.listGrantsPage(null, -1) }
        }

    @Test
    fun aRepeatedSubjectAndScopeKeepsTheHistory() =
        withStore { s, _ ->
            val first = s.putGrant("app:Mail", "send", "allow", null)
            val second = s.putGrant("app:Mail", "send", "deny", null)
            assertEquals(listOf(first, second), s.listActiveGrants("app:Mail"))
        }

    @Test
    fun activeGrantsFilterBySubjectAndExpiry() {
        val now = 1_800_000_000_000L
        TempDir.create(".aulo-store-test").use { dir ->
            Store.open(databasePath(dir.path).toString(), FixedClock(now)).use { s ->
                val forever = s.putGrant("app:Safari", "browse", "allow", null)
                val later = s.putGrant("app:Safari", "read", "allow", now + 60_000)
                s.putGrant("app:Safari", "old", "allow", now - 1)
                // Expiry is exclusive: a grant expiring at this very millisecond is already gone.
                s.putGrant("app:Safari", "edge", "allow", now)
                s.putGrant("app:Mail", "send", "allow", null)

                assertEquals(listOf(forever, later), s.listActiveGrants("app:Safari"))
                assertEquals(5, s.listGrants().size)
                assertEquals(emptyList(), s.listActiveGrants("app:Unknown"))
            }
        }
    }

    @Test
    fun auditRowsAppendInSeqOrderAndPage() =
        withStore { s, _ ->
            assertNull(s.lastAudit())
            var prev = "genesis"
            val rows =
                (0 until 5).map { i ->
                    val hash = "h$i"
                    s.appendAudit(prev, hash, "tool_call", "{\"n\":$i}").also { prev = hash }
                }
            assertEquals(rows.map { it.seq }.sorted(), rows.map { it.seq })
            assertEquals(5, rows.map { it.seq }.toSet().size)
            assertEquals("genesis", rows[0].prevHash)
            assertEquals("h0", rows[1].prevHash)

            assertEquals(rows.last(), s.lastAudit())
            assertEquals(rows, s.listAuditAfter(0, 10))
            assertEquals(rows.subList(2, 4), s.listAuditAfter(rows[1].seq, 2))
            assertEquals(emptyList(), s.listAuditAfter(rows[4].seq, 10))
            assertFailsWith<IllegalArgumentException> { s.listAuditAfter(0, -1) }
        }

    @Test
    fun aForkedAuditChainIsRefused() =
        withStore { s, _ ->
            s.appendAudit("genesis", "h0", "k", "{}")
            // A second writer that read the same chain head.
            assertFailsWith<StoreException> { s.appendAudit("genesis", "h0b", "k", "{}") }
            assertEquals(1, s.listAuditAfter(0, 10).size)
        }

    @Test
    fun auditRowsSurviveReopening() =
        TempDir.create(".aulo-store-test").use { dir ->
            val path = databasePath(dir.path).toString()
            val row = Store.open(path).use { it.appendAudit("genesis", "h0", "k", "{}") }
            Store.open(path).use { assertEquals(row, it.lastAudit()) }
        }

    private class FixedClock(
        private val ms: Long,
    ) : Clock {
        override fun now(): Instant = Instant.fromEpochMilliseconds(ms)
    }
}
