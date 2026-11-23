package aulo.speech.sherpa

import aulo.speech.AudioFormat
import aulo.speech.AudioFrame
import aulo.speech.EngineSpec
import aulo.speech.SpeechException
import aulo.speech.SttEngine
import aulo.speech.SttPoll
import aulo.speech.Transcript
import aulo.speech.helper.SherpaStt
import aulo.speech.helper.registerSherpa
import aulo.types.TranscriptKind
import aulo.types.TurnId
import aulo.voice.SttRegistry
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.io.files.Path
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TimeSource

/**
 * The engine aulod builds, over the helper process this module links: AULO_SHERPA_HELPER is set by Gradle for the
 * host test task. A real utterance runs only with AULO_SHERPA_MODELS and AULO_FLEURS set as for [WerTest].
 */
class HelperProcessTest {
    private val helper = env("AULO_SHERPA_HELPER")

    private fun registry(
        helper: String,
        modelDir: (String) -> String?,
    ) = SttRegistry().apply { registerSherpa(helper, modelDir) }

    private fun SttEngine.failure(timeout: Duration = 30.seconds): SpeechException {
        val start = TimeSource.Monotonic.markNow()
        while (start.elapsedNow() < timeout) {
            try {
                poll(Transcript(TurnId.new()))
            } catch (e: SpeechException) {
                return e
            }
            runBlocking { delay(10) }
        }
        error("no failure in $timeout")
    }

    @Test
    fun aHelperWhoseModelFilesAreMissingIsUnavailable() {
        val helper = helper ?: return
        registry(helper) { "/nonexistent/$it" }.build(EngineSpec(SherpaStt.PARAKEET.engineId)).use { stt ->
            val e = stt.failure()
            assertTrue(e is SpeechException.Unavailable && "model files missing" in e.message!!, e.message)
        }
    }

    @Test
    fun aModelThatIsNotInstalledOrAHelperThatIsNotThereIsUnavailable() {
        val helper = helper ?: return
        val notInstalled = registry(helper) { null }
        assertFailsWith<SpeechException.Unavailable> { notInstalled.build(EngineSpec(SherpaStt.MOONSHINE.engineId)) }
        val noHelper = registry("$helper-not-there") { "/tmp" }
        assertFailsWith<SpeechException.Unavailable> { noHelper.build(EngineSpec(SherpaStt.MOONSHINE.engineId)) }
        assertFailsWith<SpeechException.Unsupported> {
            notInstalled.build(EngineSpec(SherpaStt.MOONSHINE.engineId, model = SherpaStt.PARAKEET.catalogId))
        }
    }

    @Test
    fun anUtteranceThroughTheHelperEndsInItsTranscript() {
        val helper = helper ?: return
        val models = env("AULO_SHERPA_MODELS") ?: return
        val fleurs = env("AULO_FLEURS") ?: return
        val (file, ref) = readRefs(Path(fleurs, "ru_ru", "dev.tsv")).first()
        val samples = readWave(Path(fleurs, "ru_ru", "pcm", file))
        registry(helper) { Path(models, it).toString() }.build(EngineSpec(SherpaStt.PARAKEET.engineId)).use { stt ->
            val turn = TurnId.new()
            val start = TimeSource.Monotonic.markNow()
            stt.begin(turn, "ru")
            for (at in samples.indices step FRAME) {
                val frame = AudioFrame(samples, AudioFormat.PIPELINE, at.toLong(), at, minOf(FRAME, samples.size - at))
                // Capture pushes in real time; this pushes faster, so a full queue means waiting, not losing audio.
                // A dropped frame was not taken at all, so pushing it again sends it once.
                while (stt.push(frame) != 0) {
                    assertTrue(start.elapsedNow() < 60.seconds, "the helper stopped taking audio")
                    runBlocking { delay(1) }
                }
            }
            stt.finish()
            val out = Transcript(turn)
            while (stt.poll(out) != SttPoll.UPDATED) {
                assertTrue(start.elapsedNow() < 60.seconds, "no transcript")
                runBlocking { delay(10) }
            }
            assertEquals(TranscriptKind.FINAL, out.kind)
            val r = normalize(ref)
            val errors = editDistance(r, normalize(out.text.toString()))
            assertTrue(errors * 5 <= r.size, "${out.text} against $ref")
            assertEquals(SttPoll.DONE, stt.poll(out))
        }
    }

    private companion object {
        /** 20 ms frames, as the capture pipeline pushes them. */
        const val FRAME = 320
    }
}
