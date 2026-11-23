package aulo.speech.system

import aulo.speech.SpeechRate
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class RateTest {
    @Test
    fun theNormalPaceIsApplesDefault() {
        assertEquals(0.5f, appleRate(SpeechRate.NORMAL))
    }

    @Test
    fun theRangeMapsInsideApplesBounds() {
        assertEquals(0.0f, appleRate(SpeechRate(SpeechRate.MIN)))
        val fastest = appleRate(SpeechRate(SpeechRate.MAX))
        // Twice the pace is between the measured 0.625 (1.72x) and 0.75 (2.49x).
        assertTrue(fastest in 0.625f..0.75f, "$fastest")
    }

    @Test
    fun aFasterPaceIsNeverASlowerRate() {
        var last = -1f
        var pace = SpeechRate.MIN
        while (pace <= SpeechRate.MAX) {
            val rate = appleRate(SpeechRate(pace))
            assertTrue(rate >= last, "pace $pace gives $rate after $last")
            last = rate
            pace += 0.01f
        }
    }
}
