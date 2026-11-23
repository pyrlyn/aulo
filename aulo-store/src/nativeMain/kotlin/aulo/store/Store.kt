package aulo.store

import aulo.store.db.AuloDatabase
import aulo.types.MonotonicUlid
import db.Db
import db.DbException
import db.sqldelight.KitSqlDriver
import kotlinx.io.files.Path
import kotlin.time.Clock

/** The SQLite file of one aulo home (spec §14). */
fun databasePath(home: Path): Path = Path(home, "aulo.db")

/** A database operation failed. */
class StoreException(
    message: String,
    cause: Throwable? = null,
) : Exception(message, cause) {
    /** A write named a row that does not exist (a foreign-key violation): callers answer "not found". */
    val isMissingReference: Boolean =
        (cause as? DbException)?.let { it.code == SQLITE_CONSTRAINT_FOREIGNKEY || "FOREIGN KEY constraint failed" in (it.message ?: "") }
            ?: false
}

// SQLite's extended result code; a driver that reports only the primary code still names the constraint.
private const val SQLITE_CONSTRAINT_FOREIGNKEY = 787

/**
 * One connection to an aulo database. Not thread-safe: the daemon shares a store behind its own
 * lock rather than this class hiding one. Callers see typed operations only; the SQL lives in
 * the module's `.sq` files and migrations.
 */
class Store private constructor(
    private val db: Db,
    private val clock: Clock,
) : AutoCloseable {
    private val driver = KitSqlDriver(db)
    private val database = AuloDatabase(driver)
    private val bots = database.botsQueries
    private val chats = database.chatsQueries
    private val messages = database.messagesQueries
    private val toolCalls = database.toolCallsQueries
    private val usage = database.usageQueries
    private val grants = database.grantsQueries
    private val audit = database.auditQueries
    private val messageSearch = database.messageSearchQueries

    internal lateinit var migrator: Migrator
        private set

    companion object {
        /** Opens (creating if needed) the database at [path] and applies pending migrations. */
        fun open(
            path: String,
            clock: Clock = Clock.System,
        ): Store {
            val store = Store(io("cannot open $path") { sqlite.open(path) }, clock)
            try {
                io("cannot prepare $path") {
                    with(store.database.connectionQueries) {
                        setBusyTimeout()
                        enableForeignKeys()
                        enableWal()
                    }
                }
                io("cannot migrate $path") {
                    store.migrator = Migrator(store.db, store.driver, MIGRATIONS, store::now)
                    store.migrator.up()
                }
            } catch (e: Throwable) {
                store.close()
                throw e
            }
            return store
        }
    }

    override fun close() = driver.close()

    private fun now(): Long = clock.now().toEpochMilliseconds()

    // Monotonic, so ids sort in insertion order even within one millisecond; message pagination
    // relies on that.
    private val ids = MonotonicUlid(clock)

    private fun nextId(): String = ids.next()

    fun createBot(name: String): Bot =
        io("cannot create bot") {
            Bot(nextId(), name, now()).also { bots.insert(it.id, it.name, it.createdAt) }
        }

    fun listBots(): List<Bot> = io("cannot list bots") { bots.all(::Bot).executeAsList() }

    /** Returns false when no bot has that id. Its chats stay, without a bot. */
    fun deleteBot(id: String): Boolean = io("cannot delete bot") { bots.delete(id).value > 0 }

    fun createChat(
        botId: String?,
        title: String,
        model: String,
    ): Chat =
        io("cannot create chat") {
            val now = now()
            Chat(nextId(), botId, title, model, now, now).also { chats.insert(it.id, it.botId, it.title, it.model, now, now) }
        }

    fun getChat(id: String): Chat? = io("cannot read chat") { chats.byId(id, ::Chat).executeAsOneOrNull() }

    /** Most recently active chats first. */
    fun listChats(
        limit: Int,
        offset: Int = 0,
    ): List<Chat> {
        require(limit >= 0 && offset >= 0) { "limit and offset must not be negative" }
        return io("cannot list chats") { chats.recent(limit.toLong(), offset.toLong(), ::Chat).executeAsList() }
    }

    /**
     * Keyset page in the order of [listChats]: chats after [after], optionally of one bot.
     * Offsets would skip or repeat chats as their `updatedAt` changes between pages.
     */
    fun listChatsPage(
        botId: String?,
        after: ChatCursor?,
        limit: Int,
    ): List<Chat> {
        require(limit >= 0) { "limit must not be negative" }
        return io("cannot list chats") {
            // Without a cursor the query never reads afterId: a null afterUpdatedAt short-circuits it.
            chats.page(botId, after?.updatedAt, after?.id.orEmpty(), limit.toLong(), ::Chat).executeAsList()
        }
    }

    /** Returns false when no chat has that id. */
    fun setChatModel(
        id: String,
        model: String,
    ): Boolean = io("cannot update chat") { chats.setModel(model, now(), id).value > 0 }

    /** Returns false when no chat has that id. */
    fun renameChat(
        id: String,
        title: String,
    ): Boolean = io("cannot update chat") { chats.rename(title, now(), id).value > 0 }

    /** Its messages, tool calls and usage go with it (foreign-key cascade). Returns false when no chat has that id. */
    fun deleteChat(id: String): Boolean = io("cannot delete chat") { chats.delete(id).value > 0 }

    /** Appends a message and bumps the chat's `updatedAt` atomically. Sessions write whole turns with [appendTurn]. */
    fun appendMessage(
        chatId: String,
        role: String,
        content: String,
    ): Message =
        io("cannot append message") {
            val msg = Message(nextId(), chatId, role, content, now())
            database.transaction {
                messages.insert(msg.id, chatId, role, content, null, msg.createdAt)
                chats.touch(msg.createdAt, chatId)
            }
            msg
        }

    /**
     * Appends a finished turn's messages and their tool calls and bumps the chat's `updatedAt`,
     * in one transaction: a crash never leaves a tool use without its result, which would make
     * the resumed history malformed.
     */
    fun appendTurn(
        chatId: String,
        turn: List<NewMessage>,
    ): Unit =
        io("cannot append turn") {
            val now = now()
            database.transaction {
                for (m in turn) {
                    val id = nextId()
                    messages.insert(id, chatId, m.role, m.content, m.wire, now)
                    for (call in m.toolCalls) {
                        toolCalls.insert(nextId(), id, call.name, call.arguments, call.output, call.status, now)
                    }
                }
                chats.touch(now, chatId)
            }
        }

    /** Every message of a chat, oldest first, as a resumed session needs it. */
    fun chatHistory(chatId: String): List<HistoryMessage> =
        io("cannot read chat history") { messages.history(chatId, ::HistoryMessage).executeAsList() }

    /** Keyset page, oldest first: messages after [after], the last id of the previous page. */
    fun listMessages(
        chatId: String,
        after: String?,
        limit: Int,
    ): List<Message> {
        require(limit >= 0) { "limit must not be negative" }
        return io("cannot list messages") { messages.page(chatId, after, limit.toLong(), ::Message).executeAsList() }
    }

    fun recordToolCall(
        messageId: String,
        name: String,
        arguments: String,
        output: String?,
        status: String,
    ): ToolCall =
        io("cannot record tool call") {
            ToolCall(nextId(), messageId, name, arguments, output, status, now()).also {
                toolCalls.insert(it.id, messageId, name, arguments, output, status, it.createdAt)
            }
        }

    fun listToolCalls(messageId: String): List<ToolCall> =
        io("cannot list tool calls") { toolCalls.ofMessage(messageId, ::ToolCall).executeAsList() }

    /** The tool calls of several messages in one query, in id order. */
    fun listToolCallsFor(messageIds: Collection<String>): List<ToolCall> {
        if (messageIds.isEmpty()) return emptyList()
        return io("cannot list tool calls") { toolCalls.ofMessages(messageIds, ::ToolCall).executeAsList() }
    }

    fun recordUsage(
        chatId: String,
        provider: String,
        model: String,
        inputTokens: Long,
        outputTokens: Long,
    ): Usage =
        io("cannot record usage") {
            Usage(nextId(), chatId, provider, model, inputTokens, outputTokens, now()).also {
                usage.insert(it.id, chatId, provider, model, inputTokens, outputTokens, it.createdAt)
            }
        }

    fun listUsage(chatId: String): List<Usage> = io("cannot list usage") { usage.ofChat(chatId, ::Usage).executeAsList() }

    /**
     * Messages containing every word of [query], best matches first, optionally in one chat.
     * Keyset pages in `(rank, id)` order: pass the last hit as [after]. Ranks depend on corpus
     * statistics, so a message written between two pages can shift a hit across the boundary;
     * acceptable for search, unlike for history. A query with no searchable word matches nothing.
     */
    fun searchMessages(
        query: String,
        chatId: String?,
        after: SearchCursor?,
        limit: Int,
    ): List<SearchHit> {
        require(limit >= 0) { "limit must not be negative" }
        val expr = matchExpression(query) ?: return emptyList()
        return io("cannot search messages") {
            // Without a cursor the query never reads afterId: a null afterRank short-circuits it.
            messageSearch
                .search(expr, chatId, after?.rank, after?.id.orEmpty(), limit.toLong()) { id, chat, role, content, createdAt, rank ->
                    SearchHit(Message(id, chat, role, content, createdAt), rank)
                }.executeAsList()
        }
    }

    // Grants and the audit log only persist here: aulo-policy writes them, computes the audit
    // hashes and verifies the chain (T5.3, T5.9).

    /** Records a grant. A repeated subject and scope adds a row rather than replacing one, so the history stays listable. */
    fun putGrant(
        subject: String,
        scope: String,
        decision: String,
        expiresAt: Long?,
    ): Grant =
        io("cannot record grant") {
            Grant(nextId(), subject, scope, decision, expiresAt, now()).also {
                grants.insert(it.id, subject, scope, decision, expiresAt, it.createdAt)
            }
        }

    /** Every grant, expired ones included, oldest first. */
    fun listGrants(): List<Grant> = io("cannot list grants") { grants.all(::Grant).executeAsList() }

    /** Keyset page of [listGrants]: grants after [after], the last id of the previous page. */
    fun listGrantsPage(
        after: String?,
        limit: Int,
    ): List<Grant> {
        require(limit >= 0) { "limit must not be negative" }
        return io("cannot list grants") { grants.page(after, limit.toLong(), ::Grant).executeAsList() }
    }

    /** Grants of [subject] that have not expired by the store's clock, oldest first. */
    fun listActiveGrants(subject: String): List<Grant> = io("cannot list grants") { grants.active(subject, now(), ::Grant).executeAsList() }

    /** Returns false when no grant has that id. */
    fun revokeGrant(id: String): Boolean = io("cannot revoke grant") { grants.delete(id).value > 0 }

    /**
     * Appends one audit row. A [prevHash] already used by another row fails: two writers that
     * read the same chain head cannot both extend it.
     */
    fun appendAudit(
        prevHash: String,
        hash: String,
        kind: String,
        payload: String,
    ): AuditRow = io("cannot append audit row") { audit.append(prevHash, hash, kind, payload, now(), ::AuditRow).executeAsOne() }

    /** The chain head, which the caller needs to compute the next `prevHash`. */
    fun lastAudit(): AuditRow? = io("cannot read audit log") { audit.head(::AuditRow).executeAsOneOrNull() }

    /** Rows with `seq` greater than [after], in order; `0` starts from the beginning. */
    fun listAuditAfter(
        after: Long,
        limit: Int,
    ): List<AuditRow> {
        require(limit >= 0) { "limit must not be negative" }
        return io("cannot read audit log") { audit.after(after, limit.toLong(), ::AuditRow).executeAsList() }
    }
}

private inline fun <T> io(
    what: String,
    body: () -> T,
): T =
    try {
        body()
    } catch (e: DbException) {
        throw StoreException("$what: ${e.message}", e)
    }
