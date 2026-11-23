@file:OptIn(ExperimentalForeignApi::class)

package aulo.speech.sherpa

import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.get
import kotlinx.cinterop.pointed
import kotlinx.cinterop.toKString
import kotlinx.io.buffered
import kotlinx.io.files.Path
import kotlinx.io.files.SystemFileSystem
import kotlinx.io.readString
import platform.posix.getenv
import sherpa.SherpaOnnxFreeWave
import sherpa.SherpaOnnxReadWave
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlin.test.fail

/**
 * Word error rate on FLEURS dev, the acceptance check of T7.4 (thresholds agreed with the creator).
 * It needs gigabytes of models and audio, so it runs only when both variables are set:
 * `AULO_SHERPA_MODELS` is a models directory as `aulo models pull` fills it (one directory per
 * catalog id), `AULO_FLEURS` holds `<lang>/dev.tsv` and `<lang>/pcm/<file>` as 16 kHz WAV
 * (`docs/spikes/sherpa-onnx.md` says how the spike made them). A model whose directory is absent
 * is skipped with a note, so the check can run with only one of them pulled.
 */
class WerTest {
    private class Case(
        val catalogId: String,
        val model: SttModel,
        val fleurs: String,
        val language: String,
        val maxWer: Double,
    )

    private val cases =
        listOf(
            Case("parakeet-tdt-0.6b-v3-int8", SttModel.PARAKEET, "en_us", "en", 10.0),
            Case("parakeet-tdt-0.6b-v3-int8", SttModel.PARAKEET, "ru_ru", "ru", 10.0),
            // Moonshine gives an empty transcript for about 3% of clips (spike R5), counted as errors.
            Case("moonshine-base-en-quantized", SttModel.MOONSHINE, "en_us", "en", 15.0),
        )

    @Test
    fun werOnFleursDevIsWithinItsThreshold() {
        val models = env("AULO_SHERPA_MODELS") ?: return
        val fleurs = env("AULO_FLEURS") ?: return
        for (case in cases) {
            val dir = Path(models, case.catalogId)
            if (SystemFileSystem.metadataOrNull(dir)?.isDirectory != true) {
                println("WER ${case.catalogId}: skipped, no $dir")
                continue
            }
            val refs = readRefs(Path(fleurs, case.fleurs, "dev.tsv"))
            assertTrue(refs.isNotEmpty(), "no clips in ${case.fleurs}")
            var errors = 0
            var words = 0
            OfflineRecognizer(case.model, dir.toString(), threads = 4).use { recognizer ->
                for ((file, ref) in refs) {
                    val samples = readWave(Path(fleurs, case.fleurs, "pcm", file))
                    val hyp = recognizer.decode(samples, samples.size, case.language)
                    val r = normalize(ref)
                    errors += editDistance(r, normalize(hyp))
                    words += r.size
                }
            }
            val wer = 100.0 * errors / words
            println("WER ${case.catalogId} ${case.fleurs}: ${refs.size} clips, $words words, $errors errors, ${wer.format()}%")
            assertTrue(wer <= case.maxWer, "WER ${wer.format()}% of ${case.catalogId} on ${case.fleurs} is above ${case.maxWer}%")
        }
    }

    @Test
    fun theNormalizerMatchesTheSpikeScript() {
        assertEquals(listOf("the", "main", "beer", "is", "number", "one"), normalize("The main beer is 'Number One',"))
        assertEquals(listOf("еще", "раз"), normalize("Ещё — раз!"))
        assertEquals(listOf("dont", "a", "b"), normalize("don't a_b"))
        assertEquals(1, editDistance(listOf("a", "b", "c"), listOf("a", "c")))
        assertEquals(3, editDistance(listOf("a", "b", "c"), emptyList()))
    }

    private fun Double.format() = (kotlin.math.round(this * 100) / 100).toString()
}

/**
 * The normalizer of `docs/spikes/sherpa-onnx/wer.py`, so the numbers compare with the spike's: lower
 * case, ё as е, anything but letters, digits and apostrophes as a space, then apostrophes dropped.
 * The script's NFKC step is left out because Kotlin/Native has no Unicode normalizer; FLEURS en and
 * ru text has no compatibility characters, which the matching numbers confirm.
 */
internal fun normalize(s: String): List<String> =
    buildString {
        for (c in s.lowercase().replace('ё', 'е')) {
            when {
                c == '\'' -> Unit
                c.isLetterOrDigit() -> append(c)
                else -> append(' ')
            }
        }
    }.split(' ').filter { it.isNotEmpty() }

internal fun editDistance(
    r: List<String>,
    h: List<String>,
): Int {
    var prev = IntArray(h.size + 1) { it }
    for (i in 1..r.size) {
        val cur = IntArray(h.size + 1)
        cur[0] = i
        for (j in 1..h.size) {
            cur[j] = minOf(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + if (r[i - 1] == h[j - 1]) 0 else 1)
        }
        prev = cur
    }
    return prev[h.size]
}

/** File name to raw transcription; each FLEURS row is one recording of a sentence. */
internal fun readRefs(tsv: Path): List<Pair<String, String>> =
    SystemFileSystem
        .source(tsv)
        .buffered()
        .use { it.readString() }
        .lineSequence()
        .filter { it.isNotBlank() }
        .map { it.split('\t') }
        .map { it[1] to it[2] }
        .toList()

internal fun readWave(path: Path): FloatArray {
    val wave = SherpaOnnxReadWave(path.toString()) ?: fail("sherpa-onnx could not read $path")
    try {
        val w = wave.pointed
        assertEquals(16_000, w.sample_rate, "$path")
        val samples = w.samples!!
        return FloatArray(w.num_samples) { samples[it] }
    } finally {
        SherpaOnnxFreeWave(wave)
    }
}

/** An environment variable, null when unset or empty. */
internal fun env(name: String) = getenv(name)?.toKString()?.takeIf { it.isNotEmpty() }
