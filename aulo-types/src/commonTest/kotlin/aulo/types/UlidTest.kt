package aulo.types

import kotlinx.serialization.json.Json
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

class UlidTest {
    @Test
    fun encodesTheSpecExampleLayout() {
        // All-zero randomness leaves only the timestamp in the first ten characters.
        val id = Ulid.encode(1_469_918_176_385, ByteArray(10))
        assertEquals("01ARYZ6S410000000000000000", id)
        assertEquals(1_469_918_176_385, Ulid.timestampMs(id))
        assertEquals("7ZZZZZZZZZZZZZZZZZZZZZZZZZ", Ulid.encode((1L shl 48) - 1, ByteArray(10) { -1 }))
    }

    @Test
    fun generatedIdsAreValidUniqueAndCurrent() {
        val a = Ulid.next()
        val b = Ulid.next()
        assertTrue(Ulid.isValid(a))
        assertNotEquals(a, b)
        assertTrue(Ulid.timestampMs(a) > 1_700_000_000_000)
    }

    @Test
    fun laterTimestampsSortLater() {
        val ids = listOf(5L, 1_000L, 1_700_000_000_000L).map { Ulid.encode(it, ByteArray(10) { -1 }) }
        val earlierWithMaxRandom = Ulid.encode(999, ByteArray(10) { -1 })
        val laterWithZeroRandom = Ulid.encode(1_000, ByteArray(10))
        assertEquals(ids, ids.sorted())
        assertTrue(earlierWithMaxRandom < laterWithZeroRandom)
    }

    @Test
    fun rejectsNonCanonicalIds() {
        val good = Ulid.encode(1_000, ByteArray(10))
        for (bad in listOf("", good.lowercase(), good.drop(1), good + "0", "8" + good.drop(1), good.dropLast(1) + "U")) {
            assertFalse(Ulid.isValid(bad), bad)
            assertFailsWith<IllegalArgumentException> { ChatId(bad) }
        }
        assertFailsWith<IllegalArgumentException> { Ulid.encode(-1, ByteArray(10)) }
        assertFailsWith<IllegalArgumentException> { Ulid.encode(1L shl 48, ByteArray(10)) }
    }

    @Test
    fun idsSerializeAsBareStrings() {
        val id = TurnId.new()
        val json = Json.encodeToString(TurnId.serializer(), id)
        assertEquals("\"$id\"", json)
        assertEquals(id, Json.decodeFromString(TurnId.serializer(), json))
        assertEquals(BotId.new().value.length, Ulid.LENGTH)
        assertTrue(Ulid.isValid(ClientId.new().value) && Ulid.isValid(CallId.new().value))
        assertFailsWith<IllegalArgumentException> { Json.decodeFromString(BotId.serializer(), "\"not-a-ulid\"") }
    }
}
