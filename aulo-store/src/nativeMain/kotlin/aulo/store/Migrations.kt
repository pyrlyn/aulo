package aulo.store

import app.cash.sqldelight.db.SqlDriver
import aulo.store.ledger.LedgerDatabase
import db.Db

/** One schema change: [up] applies it, [down] reverts it. [key] orders migrations and names them in the ledger. */
class Migration(
    val key: String,
    val up: String,
    val down: String,
)

/** Every embedded migration, in key order. */
internal val MIGRATIONS: List<Migration> = migrationsFrom(MIGRATION_FILES)

/** Groups `<key>/up.sql` and `<key>/down.sql` into migrations; a key without exactly those two is a build mistake. */
internal fun migrationsFrom(files: Map<String, String>): List<Migration> {
    val byKey = files.keys.groupBy({ it.substringBefore('/') }, { it.substringAfter('/', "") })
    return byKey.keys.sorted().map { key ->
        val names = byKey.getValue(key).sorted()
        check(names == listOf("down.sql", "up.sql")) { "migration $key needs up.sql and down.sql only, has $names" }
        Migration(key, files.getValue("$key/up.sql"), files.getValue("$key/down.sql"))
    }
}

/**
 * Applies and reverts [migrations] on [db]. Each migration and its ledger row land in one
 * transaction, so a failing migration leaves neither half behind.
 */
internal class Migrator(
    private val db: Db,
    driver: SqlDriver,
    private val migrations: List<Migration>,
    private val now: () -> Long,
) {
    private val ledger = LedgerDatabase(driver)
    private val queries = ledger.ledgerQueries

    init {
        LedgerDatabase.Schema.create(driver)
    }

    fun applied(): List<String> = queries.applied().executeAsList()

    /**
     * Applies every pending migration in key order and returns their keys. A pending key older
     * than an applied one still runs: it came from a branch merged later.
     */
    fun up(): List<String> {
        val done = applied().toSet()
        val unknown = done - migrations.map { it.key }.toSet()
        // Running against a schema this binary does not know could corrupt what a newer aulo wrote.
        if (unknown.isNotEmpty()) {
            throw StoreException(
                "the database has migrations this aulo does not know (${unknown.sorted().joinToString()}); a newer aulo wrote it",
            )
        }
        return migrations.sortedBy { it.key }.filter { it.key !in done }.map { m ->
            ledger.transaction {
                db.exec(m.up)
                queries.record(m.key, now())
            }
            m.key
        }
    }

    /** Reverts the [steps] newest applied migrations, newest first, and returns their keys. */
    fun down(steps: Int = Int.MAX_VALUE): List<String> {
        val byKey = migrations.associateBy { it.key }
        return applied().sortedDescending().take(steps).map { key ->
            val m = byKey[key] ?: throw StoreException("cannot revert $key: this aulo does not know it")
            ledger.transaction {
                db.exec(m.down)
                queries.forget(key)
            }
            key
        }
    }
}
