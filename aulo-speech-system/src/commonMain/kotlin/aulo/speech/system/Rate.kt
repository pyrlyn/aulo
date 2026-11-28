package aulo.speech.system

import aulo.speech.SpeechRate

/*
 * AVSpeechUtterance.rate runs from 0 to 1 with 0.5 as the voice's normal pace, and the scale is
 * not linear: above 0.5 speech speeds up far faster than below it slows down. Apple documents
 * only the bounds, so the pace of each rate was measured on macOS 27 (Samantha and Milena, one
 * sentence each, research.md §4.18) and a pace in between is interpolated.
 */
private val PACES = floatArrayOf(0.56f, 0.67f, 0.77f, 0.91f, 1.0f, 1.36f, 1.72f, 2.49f)
private val RATES = floatArrayOf(0.0f, 0.1f, 0.25f, 0.375f, 0.5f, 0.5625f, 0.625f, 0.75f)

/** The AVSpeechUtterance rate that speaks at [rate] times the voice's normal pace. */
fun appleRate(rate: SpeechRate): Float {
    val pace = rate.value
    if (pace <= PACES.first()) return RATES.first()
    for (i in 1 until PACES.size) {
        if (pace <= PACES[i]) {
            val t = (pace - PACES[i - 1]) / (PACES[i] - PACES[i - 1])
            return RATES[i - 1] + t * (RATES[i] - RATES[i - 1])
        }
    }
    return RATES.last()
}
