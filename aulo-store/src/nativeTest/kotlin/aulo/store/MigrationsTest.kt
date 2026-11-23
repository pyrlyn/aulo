package aulo.store

import aulo.store.db.AuloDatabase
import db.Db
import db.DbException
import db.sqldelight.KitSqlDriver
import fs.TempDir
import kotlinx.io.files.Path
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

// Raw SQL here only inspects a database (sqlite_master, the table-valued PRAGMAs) or stands in
// for a migration file: SQLDelight has no way to say either.
class MigrationsTest {
    private val core = MIGRATIONS.single().key

    @Test
    fun migrationsRevertAndReapply() =
        withStore { s, _ ->
            assertEquals(listOf(core), s.migrator.applied())
            assertEquals(0, s.listChats(10).size)

            assertEquals(listOf(core), s.migrator.down())
            assertEquals(emptyList(), s.migrator.applied())
            // The typed queries cannot list tables; a failing query proves the table is gone.
            assertFailsWith<StoreException> { s.listChats(10) }
            assertFailsWith<StoreException> { s.listUsage("c") }

            assertEquals(listOf(core), s.migrator.up())
            assertEquals(0, s.listBots().size)
        }

    @Test
    fun reopeningDoesNotMigrateAgain() =
        TempDir.create(".aulo-store-test").use { dir ->
            val path = databasePath(dir.path).toString()
            Store.open(path).close()
            Store.open(path).use { assertEquals(emptyList(), it.migrator.up()) }
        }

    @Test
    fun aRevertedDatabaseKeepsOnlyTheLedger() =
        scratch { db, path ->
            Store.open(path).use { it.migrator.down() }
            assertEquals(listOf("__aulo_migrations"), tables(db))
        }

    @Test
    fun pendingMigrationsRunInKeyOrder() =
        scratch { db, _ ->
            val a = Migration("2026-01-01-000000_a", "CREATE TABLE t1 (x INTEGER);", "DROP TABLE t1;")
            val b = Migration("2026-01-02-000000_b", "CREATE TABLE t2 AS SELECT x FROM t1;", "DROP TABLE t2;")
            assertEquals(listOf(a.key, b.key), migrator(db, listOf(b, a)).up())
            assertEquals(listOf("__aulo_migrations", "t1", "t2"), tables(db))
        }

    @Test
    fun aMigrationFromABranchMergedLaterStillRuns() =
        scratch { db, _ ->
            val a = Migration("2026-01-01-000000_a", "CREATE TABLE t1 (x INTEGER);", "DROP TABLE t1;")
            val b = Migration("2026-01-02-000000_b", "CREATE TABLE t2 (x INTEGER);", "DROP TABLE t2;")
            migrator(db, listOf(b)).up()
            assertEquals(listOf(a.key), migrator(db, listOf(a, b)).up())
            assertEquals(listOf(a.key, b.key), migrator(db, listOf(a, b)).applied())
        }

    @Test
    fun aFailingMigrationLeavesNothingBehind() =
        scratch { db, _ ->
            val good = Migration("2026-01-01-000000_good", "CREATE TABLE t1 (x INTEGER);", "DROP TABLE t1;")
            val bad = Migration("2026-01-02-000000_bad", "CREATE TABLE t2 (x INTEGER); CREATE TABLE broken (", "")
            val m = migrator(db, listOf(good, bad))
            assertFailsWith<DbException> { m.up() }
            assertEquals(listOf(good.key), m.applied())
            assertEquals(listOf("__aulo_migrations", "t1"), tables(db))
        }

    @Test
    fun aDatabaseFromANewerAuloIsLeftAlone() =
        scratch { db, _ ->
            val a = Migration("2026-01-01-000000_a", "CREATE TABLE t1 (x INTEGER);", "DROP TABLE t1;")
            val b = Migration("2026-01-02-000000_b", "CREATE TABLE t2 (x INTEGER);", "DROP TABLE t2;")
            migrator(db, listOf(a, b)).up()
            val e = assertFailsWith<StoreException> { migrator(db, listOf(a)).up() }
            assertTrue(b.key in e.message.orEmpty(), e.message)
            assertFailsWith<StoreException> { migrator(db, listOf(a)).down() }
        }

    @Test
    fun downRevertsNewestFirstAndCanStopEarly() =
        scratch { db, _ ->
            val a = Migration("2026-01-01-000000_a", "CREATE TABLE t1 (x INTEGER);", "DROP TABLE t1;")
            val b = Migration("2026-01-02-000000_b", "CREATE TABLE t2 (x INTEGER);", "DROP TABLE t2;")
            val m = migrator(db, listOf(a, b))
            m.up()
            assertEquals(listOf(b.key), m.down(1))
            assertEquals(listOf(a.key), m.applied())
            assertEquals(listOf("__aulo_migrations", "t1"), tables(db))
        }

    @Test
    fun aMigrationNeedsBothHalves() {
        val e = assertFailsWith<IllegalStateException> { migrationsFrom(mapOf("k/up.sql" to "", "k/notes.txt" to "")) }
        assertTrue("k" in e.message.orEmpty(), e.message)
        assertFailsWith<IllegalStateException> { migrationsFrom(mapOf("k/up.sql" to "")) }
        assertEquals(
            listOf(
                "a",
                "b",
            ),
            migrationsFrom(mapOf("b/up.sql" to "", "b/down.sql" to "", "a/down.sql" to "", "a/up.sql" to "")).map {
                it.key
            },
        )
    }

    /** The typed queries are checked against the .sq schema; the database gets the migrations' one. They must agree. */
    @Test
    fun theMigrationsBuildTheSchemaTheQueriesWereCheckedAgainst() =
        TempDir.create(".aulo-store-test").use { dir ->
            val migrated = Path(dir.path, "migrated.db").toString()
            Store.open(migrated).close()
            val declared = Path(dir.path, "declared.db").toString()
            val driver = KitSqlDriver(sqlite.open(declared))
            try {
                AuloDatabase.Schema.create(driver)
            } finally {
                driver.close()
            }

            val want = sqlite.open(declared).use(::schema)
            assertTrue(want.any { it.startsWith("messages.column wire ") }, want.joinToString("\n"))
            assertEquals(want, sqlite.open(migrated).use(::schema))
        }

    private fun scratch(block: (Db, String) -> Unit) =
        TempDir.create(".aulo-store-test").use { dir ->
            val path = Path(dir.path, "scratch.db").toString()
            sqlite.open(path).use { block(it, path) }
        }

    private fun migrator(
        db: Db,
        migrations: List<Migration>,
    ) = Migrator(db, KitSqlDriver(db), migrations) { 0L }

    private fun tables(db: Db): List<String> =
        db.query("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name") { it.string(0) }

    /** Every table but the ledger: columns, foreign keys and indexes, one line each. */
    private fun schema(db: Db): List<String> =
        tables(db)
            .filter { it != "__aulo_migrations" }
            .flatMap { t ->
                db.query("SELECT name, type, \"notnull\", dflt_value, pk FROM pragma_table_info(?)", t) {
                    "$t.column ${it.string(0)} ${it.string(1)} notnull=${it.long(2)} default=${it.stringOrNull(3)} pk=${it.long(4)}"
                } +
                    db.query("SELECT \"table\", \"from\", \"to\", on_update, on_delete FROM pragma_foreign_key_list(?)", t) {
                        "$t.fk ${it.string(1)} -> ${it.string(0)}(${it.stringOrNull(2)}) update=${it.string(3)} delete=${it.string(4)}"
                    } +
                    db.query("SELECT name, \"unique\" FROM pragma_index_list(?)", t) { it.string(0) to it.long(1) }.map { (index, unique) ->
                        val columns = db.query("SELECT name FROM pragma_index_info(?) ORDER BY seqno", index) { it.string(0) }
                        "$t.index $index unique=$unique $columns"
                    }
            }.sorted()
}
