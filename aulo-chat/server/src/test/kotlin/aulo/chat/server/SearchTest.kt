package aulo.chat.server

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class SearchTest {
    private val model = ModelId("stub", "echo")

    @Test
    fun `every word must match, case does not matter, newest first`() {
        var now = 1_000L
        val store = ChatStore(clock = { now++ })
        val trip = store.createChat("", model)
        val notes = store.createChat("", model)
        store.append(trip.id, Role.USER, "Plan a trip to Lisbon in May")
        store.append(notes.id, Role.USER, "lisbon release notes")
        store.append(trip.id, Role.ASSISTANT, "Day 1: Alfama")

        assertEquals(
            listOf("lisbon release notes", "Plan a trip to Lisbon in May"),
            store.search("LISBON", null).map { it.content },
        )
        assertEquals(listOf("Plan a trip to Lisbon in May"), store.search("may  lisbon", null).map { it.content })
        assertEquals(listOf("Plan a trip to Lisbon in May"), store.search("lisbon", trip.id).map { it.content })
    }

    @Test
    fun `query syntax is plain text`() {
        val store = ChatStore()
        val chat = store.createChat("", model)
        store.append(chat.id, Role.USER, "a.b*c")
        assertEquals(1, store.search("a.b*c", null).size)
        assertTrue(store.search("a?b", null).isEmpty())
        assertTrue(store.search("   ", null).isEmpty())
    }
}
