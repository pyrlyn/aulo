@file:OptIn(ExperimentalForeignApi::class, ExperimentalAtomicApi::class)

package aulo.speech.system

import aulo.speech.SpeechException
import aulo.speech.SpeechRate
import aulo.speech.Voice
import aulo.speech.helper.HelperVoice
import aulo.speech.helper.MAX_HELPER_VOICES
import aulo.speech.helper.SpeechSynth
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.get
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull
import platform.AVFAudio.AVAudioPCMBuffer
import platform.AVFAudio.AVAudioPCMFormatFloat32
import platform.AVFAudio.AVAudioPCMFormatInt16
import platform.AVFAudio.AVSampleRateKey
import platform.AVFAudio.AVSpeechSynthesisVoice
import platform.AVFAudio.AVSpeechSynthesizer
import platform.AVFAudio.AVSpeechUtterance
import platform.Foundation.NSNumber
import kotlin.concurrent.atomics.AtomicBoolean
import kotlin.concurrent.atomics.AtomicReference
import kotlin.concurrent.atomics.ExperimentalAtomicApi
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TimeMark
import kotlin.time.TimeSource

private const val PCM16_SCALE = 32768f

/** How long the synthesizer may go without a buffer while nothing holds it back: past it, it is hung. */
private val HUNG_AFTER = 30.seconds
private val WAIT_SLICE = 1.seconds

/** A voice the synthesizer produced nothing for in [HUNG_AFTER]; the helper exits on it, as aulod would kill it. */
class SynthHungException(
    message: String,
) : Exception(message)

/**
 * The macOS voices through AVSpeechSynthesizer's `write(_:toBufferCallback:)`, which hands over
 * audio instead of playing it: aulod plays it, so barge-in and echo cancellation see every sample.
 * The buffers arrive on the main thread's run loop, which the helper's main thread runs.
 */
class AppleSynth : SpeechSynth {
    private val synth = AVSpeechSynthesizer()

    override val voices: List<HelperVoice> = installedVoices()

    private val rates = voices.associate { it.voice.id to it.sampleRateHz }

    override fun speak(
        text: String,
        voice: String,
        rate: SpeechRate,
        audio: (samples: FloatArray, count: Int) -> Unit,
    ) {
        val declared = rates[voice] ?: throw IllegalArgumentException("no voice $voice")
        val apple = AVSpeechSynthesisVoice.voiceWithIdentifier(voice) ?: throw IllegalArgumentException("voice $voice is gone")
        val utterance = AVSpeechUtterance.speechUtteranceWithString(text)
        utterance.voice = apple
        utterance.rate = appleRate(rate)

        val done = Channel<Unit>(Channel.CONFLATED)
        val finished = AtomicBoolean(false)
        val failure = AtomicReference<Throwable?>(null)
        val inCallback = AtomicBoolean(false)
        val lastBuffer = AtomicReference<TimeMark>(TimeSource.Monotonic.markNow())
        var scratch = FloatArray(0)

        synth.writeUtterance(utterance) { buffer ->
            if (finished.load()) return@writeUtterance
            inCallback.store(true)
            try {
                val pcm = buffer as? AVAudioPCMBuffer ?: return@writeUtterance
                val count = pcm.frameLength.toInt()
                if (count == 0) {
                    // An empty buffer is the end of the utterance.
                    finished.store(true)
                    done.trySend(Unit)
                    return@writeUtterance
                }
                val format = pcm.format
                if (format.sampleRate.toInt() != declared) {
                    error("voice $voice writes ${format.sampleRate.toInt()} Hz, it listed $declared Hz")
                }
                if (scratch.size < count) scratch = FloatArray(count)
                copyFirstChannel(pcm, count, scratch)
                audio(scratch, count)
            } catch (e: Throwable) {
                // Nothing may be thrown into the Objective-C caller; speak rethrows it.
                failure.store(e)
                finished.store(true)
                done.trySend(Unit)
            } finally {
                lastBuffer.store(TimeSource.Monotonic.markNow())
                inCallback.store(false)
            }
        }

        waitForEnd(done, inCallback, lastBuffer, voice)
        failure.load()?.let { throw it }
    }

    private fun waitForEnd(
        done: Channel<Unit>,
        inCallback: AtomicBoolean,
        lastBuffer: AtomicReference<TimeMark>,
        voice: String,
    ) {
        while (runBlocking { withTimeoutOrNull(WAIT_SLICE) { done.receive() } } == null) {
            // A callback waiting on a full pipe is aulod holding the helper back, not a hang.
            if (!inCallback.load() && lastBuffer.load().elapsedNow() > HUNG_AFTER) {
                throw SynthHungException("voice $voice gave no audio for $HUNG_AFTER")
            }
        }
    }
}

private fun copyFirstChannel(
    pcm: AVAudioPCMBuffer,
    count: Int,
    out: FloatArray,
) {
    val stride = pcm.stride.toInt()
    when (pcm.format.commonFormat) {
        AVAudioPCMFormatFloat32 -> {
            val data = pcm.floatChannelData?.get(0) ?: error("no float channel data")
            for (i in 0 until count) out[i] = data[i * stride]
        }

        AVAudioPCMFormatInt16 -> {
            val data = pcm.int16ChannelData?.get(0) ?: error("no int16 channel data")
            for (i in 0 until count) out[i] = data[i * stride] / PCM16_SCALE
        }

        else -> {
            error("unsupported sample format ${pcm.format.commonFormat}")
        }
    }
}

/**
 * Every installed voice whose rate is known, with the system's default per language and for
 * its own language marked. A voice the speech contract refuses (an overlong name, an odd language
 * tag) is left out rather than failing the helper.
 */
private fun installedVoices(): List<HelperVoice> {
    val systemDefault = AVSpeechSynthesisVoice.voiceWithLanguage(null)?.identifier
    val languageDefaults = mutableMapOf<String, String?>()
    val out = mutableListOf<HelperVoice>()
    for (entry in AVSpeechSynthesisVoice.speechVoices()) {
        if (out.size == MAX_HELPER_VOICES) break
        val voice = entry as? AVSpeechSynthesisVoice ?: continue
        val rate = (voice.audioFileSettings[AVSampleRateKey] as? NSNumber)?.intValue ?: continue
        val default = languageDefaults.getOrPut(voice.language) { AVSpeechSynthesisVoice.voiceWithLanguage(voice.language)?.identifier }
        try {
            out +=
                HelperVoice(
                    Voice(voice.identifier, voice.name, voice.language),
                    rate,
                    languageDefault = voice.identifier == default,
                    systemDefault = voice.identifier == systemDefault,
                )
        } catch (e: SpeechException) {
            continue
        } catch (e: IllegalArgumentException) {
            continue
        }
    }
    return out
}
