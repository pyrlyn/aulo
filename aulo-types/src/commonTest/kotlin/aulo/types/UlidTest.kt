package aulo.types

import kotlinx.serialization.json.Json
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue
import kotlin.time.Clock
import kotlin.time.Instant

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

    private class FixedClock(
        var ms: Long,
    ) : Clock {
        override fun now(): Instant = Instant.fromEpochMilliseconds(ms)
    }

    @Test
    fun monotonicIdsInOneMillisecondCountUp() {
        val clock = FixedClock(1_700_000_000_000)
        val gen = MonotonicUlid(clock) { ByteArray(10) }
        val ids = List(3) { gen.next() }
        assertEquals(ids.sorted(), ids)
        assertEquals(3, ids.toSet().size)
        assertEquals(listOf("0000000000000000", "0000000000000001", "0000000000000002"), ids.map { it.drop(10) })
        assertTrue(ids.all { Ulid.timestampMs(it) == clock.ms })
    }

    @Test
    fun aNewMillisecondTakesFreshRandomness() {
        val clock = FixedClock(1_700_000_000_000)
        var draws = 0
        val gen =
            MonotonicUlid(clock) {
                draws++
                ByteArray(10) { 7 }
            }
        val a = gen.next()
        clock.ms += 1
        val b = gen.next()
        assertEquals(2, draws)
        assertEquals(a.drop(10), b.drop(10))
        assertTrue(a < b)
    }

    @Test
    fun aClockThatStepsBackStillSortsLater() {
        val clock = FixedClock(1_700_000_000_000)
        val gen = MonotonicUlid(clock) { ByteArray(10) }
        val a = gen.next()
        clock.ms -= 5_000
        val b = gen.next()
        assertTrue(a < b)
        assertEquals(Ulid.timestampMs(a), Ulid.timestampMs(b))
    }

    @Test
    fun exhaustedRandomnessCarriesIntoTheNextMillisecond() {
        val clock = FixedClock(1_700_000_000_000)
        val gen = MonotonicUlid(clock) { ByteArray(10) { -1 } }
        val a = gen.next()
        val b = gen.next()
        assertTrue(a < b)
        assertEquals(clock.ms + 1, Ulid.timestampMs(b))
    }
}
