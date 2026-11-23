package aulo.speech

/**
 * Sample rate of the capture pipeline (spec §4.3). aulo-audio resamples every input to it, so
 * engines that take only this rate need no resampler.
 */
const val PIPELINE_SAMPLE_RATE_HZ = 16_000

/** Highest channel count a format may claim; formats can come from plugins. */
const val MAX_CHANNELS = 255

private const val MS_PER_SECOND = 1_000L

/** Sample rate and channel count of interleaved `Float` samples. */
data class AudioFormat(
    val sampleRateHz: Int,
    val channels: Int,
) {
    // Rejected here so the time conversions below can never divide by zero.
    init {
        if (sampleRateHz <= 0) throw SpeechException.Invalid("audio format", "sample rate must be positive")
        if (channels !in 1..MAX_CHANNELS) throw SpeechException.Invalid("audio format", "channel count must be 1 to $MAX_CHANNELS")
    }

    /** A stream position (per-channel samples since the stream started) in milliseconds, the unit events carry. */
    fun positionToMs(position: Long): Long {
        // Split so `position * 1000` cannot overflow for any position.
        val seconds = position / sampleRateHz
        val rest = position % sampleRateHz
        return seconds * MS_PER_SECOND + rest * MS_PER_SECOND / sampleRateHz
    }

    companion object {
        /** Every capture frame: 16 kHz mono. */
        val PIPELINE = AudioFormat(PIPELINE_SAMPLE_RATE_HZ, 1)
    }
}

/**
 * A run of interleaved samples inside the caller's array and where it sits in its stream. It is
 * a window, not a copy, so handing audio to an engine copies nothing; an engine that keeps audio
 * copies it into its own fixed-size storage, because the caller reuses the array.
 */
class AudioFrame(
    private val samples: FloatArray,
    val format: AudioFormat,
    /** Per-channel index of the first sample since the stream started; detectors stamp events with it. */
    val position: Long,
    private val offset: Int = 0,
    /** Interleaved samples in the frame. */
    val size: Int = samples.size - offset,
) {
    init {
        if (position < 0) throw SpeechException.Invalid("audio frame", "position is negative")
        if (offset !in 0..samples.size || size !in 0..samples.size - offset) {
            throw SpeechException.Invalid("audio frame", "window is outside the sample array")
        }
        if (size % format.channels != 0) {
            throw SpeechException.Invalid("audio frame", "sample count is not a multiple of the channel count")
        }
    }

    /** Per-channel samples in the frame. */
    val length: Int get() = size / format.channels

    /** Where the next contiguous frame starts. */
    val endPosition: Long get() = position + length

    val atMs: Long get() = format.positionToMs(position)

    /** Interleaved sample [index] of this frame. */
    operator fun get(index: Int): Float {
        if (index !in 0 until size) throw IndexOutOfBoundsException("sample $index of $size")
        return samples[offset + index]
    }

    /** Copies the frame's samples into [destination] at [destinationOffset]. */
    fun copyInto(
        destination: FloatArray,
        destinationOffset: Int = 0,
    ) {
        samples.copyInto(destination, destinationOffset, offset, offset + size)
    }
}
