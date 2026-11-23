package aulo.chat

import app.cash.sqldelight.driver.jdbc.sqlite.JdbcSqliteDriver
import aulo.chat.data.ChatCache
import aulo.chat.data.ChatRemote
import aulo.chat.data.SettingsStore
import aulo.chat.data.cache.ChatDatabase
import aulo.chat.data.ServerEndpoint
import aulo.chat.data.SwitchableChatRemote
import aulo.chat.data.address
import aulo.chat.data.excerpt
import aulo.chat.data.parseAddress
import aulo.chat.domain.Appearance
import aulo.chat.domain.OfflineException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import java.util.Properties
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue

class SettingsTest {
    @Test
    fun `addresses are host and port and nothing else`() {
        assertEquals(ServerEndpoint("127.0.0.1", 50051), parseAddress(" 127.0.0.1:50051 "))
        assertEquals(ServerEndpoint("::1", 6000), parseAddress("[::1]:6000"))
        assertEquals("[::1]:6000", ServerEndpoint("::1", 6000).address())
        for (bad in listOf("", "localhost", ":50051", "localhost:", "localhost:0", "localhost:65536", "a b:1", "http://x:1")) {
            assertNull(parseAddress(bad), bad)
        }
    }

    @Test
    fun `settings survive a round trip and unknown rows fall back to defaults`() = runTest {
        val store = memorySettings(Dispatchers.Unconfined)
        assertEquals(Appearance.System, store.current().appearance)
        store.setAppearance(Appearance.Light)
        store.setEndpoint(ServerEndpoint("192.0.2.5", 6000))
        store.setDefaultModel(SLOW)
        val saved = store.settings.first()
        assertEquals(Appearance.Light, saved.appearance)
        assertEquals(ServerEndpoint("192.0.2.5", 6000), saved.endpoint)
        assertEquals(SLOW, saved.defaultModel)
        store.setDefaultModel(null)
        assertNull(store.current().defaultModel)
    }

    @Test
    fun `a cache written by the first version gains the settings table`() = runTest {
        val file = kotlin.io.path.createTempFile("aulo-chat", ".db").toFile().apply { deleteOnExit() }
        val url = "jdbc:sqlite:" + file.absolutePath
        // The version-1 file as T12.22 wrote it. SQLDelight cannot produce an old schema, so the
        // test writes those statements itself.
        JdbcSqliteDriver(url).use { old ->
            for (statement in listOf(
                "CREATE TABLE chat (id TEXT NOT NULL PRIMARY KEY, title TEXT NOT NULL, model_provider TEXT NOT NULL, model_name TEXT NOT NULL, updated_at_ms INTEGER NOT NULL)",
                "INSERT INTO chat VALUES ('lisbon', 'Trip to Lisbon', 'stub', 'echo', 1)",
                "PRAGMA user_version = 1",
            )) old.execute(null, statement, 0)
        }
        val driver = JdbcSqliteDriver(url, Properties(), ChatDatabase.Schema)
        val store = SettingsStore(driver, Dispatchers.Unconfined)
        store.setAppearance(Appearance.Dark)
        assertEquals(Appearance.Dark, store.current().appearance)
        assertEquals("Trip to Lisbon", ChatCache(driver, Dispatchers.Unconfined).latestChat()?.title)
        driver.close()
    }

    @Test
    fun `a server that does not answer never replaces the one in use`() = runTest {
        val good = FakeRemote()
        val bad = FakeRemote().apply { reachable = false }
        val remote = SwitchableChatRemote(ServerEndpoint("192.0.2.1", 1)) { if (it.port == 1) good else bad }
        assertFailsWith<OfflineException> { remote.switchTo(ServerEndpoint("192.0.2.2", 2)) }
        assertEquals(1, remote.endpoint.port)
        good.seedChat("a", "Still here")
        assertEquals("Still here", remote.listChats(1).single().title)

        val other: ChatRemote = FakeRemote().apply { seedChat("b", "Other server") }
        val switched = SwitchableChatRemote(ServerEndpoint("192.0.2.1", 1)) { if (it.port == 1) good else other }
        switched.switchTo(ServerEndpoint("192.0.2.3", 3))
        assertEquals("Other server", switched.listChats(1).single().title)
        assertEquals(3, switched.endpoint.port)
    }

    @Test
    fun `an excerpt is one line around the match`() {
        val text = "word ".repeat(100) + "Lisbon\nis lovely"
        val cut = excerpt(text, "lisbon")
        assertTrue("Lisbon is lovely" in cut, cut)
        assertTrue(cut.startsWith("…"))
        assertTrue('\n' !in cut)
        assertEquals("short", excerpt("short", "nothing"))
    }
}
