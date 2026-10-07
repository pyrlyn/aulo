//! The producer half of the capture ring: what runs on the platform's audio
//! thread. It never blocks and never allocates: the resampler is plain state,
//! the frame is a fixed array and the ring was sized before the stream started.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use aulo_speech::PIPELINE_SAMPLE_RATE_HZ;
use aulo_speech_system::Resampler;
use ringbuf::HeapProd;
use ringbuf::traits::{Observer, Producer};

use crate::AudioError;

/// Frame length of the voice pipeline (spec §4.3).
pub const FRAME_MS: usize = 20;
/// Samples in one frame: 20 ms at 16 kHz.
pub const FRAME_SAMPLES: usize = PIPELINE_SAMPLE_RATE_HZ as usize * FRAME_MS / 1_000;

/// Takes mono samples at the device rate and turns them into whole 16 kHz
/// frames in the ring. A device calls [`SampleSink::push`] from its audio
/// callback.
pub struct SampleSink {
    resampler: Resampler,
    frame: [f32; FRAME_SAMPLES],
    filled: usize,
    producer: HeapProd<f32>,
    overflow: Arc<AtomicU64>,
}

impl std::fmt::Debug for SampleSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleSink")
            .field("filled", &self.filled)
            .finish_non_exhaustive()
    }
}

impl SampleSink {
    pub(crate) fn new(
        device_rate_hz: u32,
        producer: HeapProd<f32>,
        overflow: Arc<AtomicU64>,
    ) -> Result<Self, AudioError> {
        // A zero rate makes the resampler step zero, which never terminates.
        if device_rate_hz == 0 {
            return Err(AudioError::Invalid {
                what: "input device",
                reason: "sample rate is zero",
            });
        }
        Ok(Self {
            resampler: Resampler::new(f64::from(device_rate_hz), PIPELINE_SAMPLE_RATE_HZ),
            frame: [0.0; FRAME_SAMPLES],
            filled: 0,
            producer,
            overflow,
        })
    }

    /// Feeds mono samples at the device rate. A frame the ring has no room for
    /// is dropped whole and counted, so frames in the ring stay aligned and the
    /// audio thread never waits for the consumer.
    pub fn push(&mut self, mono: impl IntoIterator<Item = f32>) {
        for sample in mono {
            self.resampler.push(sample, |out| {
                self.frame[self.filled] = out;
                self.filled += 1;
                if self.filled < FRAME_SAMPLES {
                    return;
                }
                self.filled = 0;
                if self.producer.vacant_len() >= FRAME_SAMPLES {
                    self.producer.push_slice(&self.frame);
                } else {
                    self.overflow
                        .fetch_add(FRAME_SAMPLES as u64, Ordering::Relaxed);
                }
            });
        }
    }
}
