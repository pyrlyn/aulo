//! The cpal backend. The callback downmixes straight into the [`SampleSink`];
//! everything that can allocate or fail (device lookup, config, stream
//! building) happens before the stream plays.

use std::sync::mpsc;
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};

use crate::AudioError;
use crate::capture::{DeviceSelector, InputBackend, InputDevice, StreamGuard};
use crate::sink::SampleSink;

fn stream_error(e: cpal::Error) -> AudioError {
    AudioError::stream(&e.to_string())
}

/// The platform's audio host (CoreAudio, WASAPI, ALSA).
#[derive(Debug, Default, Clone, Copy)]
pub struct CpalBackend;

impl InputBackend for CpalBackend {
    type Device = CpalDevice;

    fn find(&self, selector: &DeviceSelector) -> Result<CpalDevice, AudioError> {
        let host = cpal::default_host();
        let device = match selector {
            DeviceSelector::Default => host
                .default_input_device()
                .ok_or(AudioError::NoInputDevice)?,
            DeviceSelector::Named(wanted) => host
                .input_devices()
                .map_err(stream_error)?
                .find(|d| d.description().is_ok_and(|d| d.name() == wanted))
                .ok_or_else(|| AudioError::DeviceNotFound(aulo_speech::ErrorDetail::new(wanted)))?,
        };
        let name = device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_default();
        let supported = device.default_input_config().map_err(stream_error)?;
        Ok(CpalDevice {
            device,
            name,
            rate: supported.sample_rate(),
            channels: usize::from(supported.channels()).max(1),
            format: supported.sample_format(),
            config: supported.into(),
        })
    }
}

pub struct CpalDevice {
    device: cpal::Device,
    name: String,
    rate: u32,
    channels: usize,
    format: SampleFormat,
    config: cpal::StreamConfig,
}

impl std::fmt::Debug for CpalDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CpalDevice")
            .field("name", &self.name)
            .field("rate", &self.rate)
            .field("channels", &self.channels)
            .finish_non_exhaustive()
    }
}

impl InputDevice for CpalDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn sample_rate_hz(&self) -> u32 {
        self.rate
    }

    fn start(self, sink: SampleSink) -> Result<StreamGuard, AudioError> {
        // A cpal stream is not `Send` on every platform, so it lives and dies
        // on its own thread and the guard is just the thread's stop switch.
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let thread = std::thread::Builder::new()
            .name("aulo-audio-capture".into())
            .spawn(move || match self.open(sink) {
                Ok(stream) => {
                    let _ = ready_tx.send(Ok(()));
                    // Returns when the guard drops the sender.
                    let _ = stop_rx.recv();
                    drop(stream);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            })
            .map_err(|e| AudioError::stream(&e.to_string()))?;
        let guard = Guard {
            stop: Some(stop_tx),
            thread: Some(thread),
        };
        ready_rx
            .recv()
            .map_err(|_| AudioError::stream("the capture thread ended"))??;
        Ok(Box::new(guard))
    }
}

impl CpalDevice {
    fn open(self, sink: SampleSink) -> Result<cpal::Stream, AudioError> {
        let Self {
            device,
            channels,
            format,
            config,
            ..
        } = self;
        let stream = match format {
            SampleFormat::I8 => build::<i8>(&device, config, channels, sink),
            SampleFormat::I16 => build::<i16>(&device, config, channels, sink),
            SampleFormat::I32 => build::<i32>(&device, config, channels, sink),
            SampleFormat::I64 => build::<i64>(&device, config, channels, sink),
            SampleFormat::U8 => build::<u8>(&device, config, channels, sink),
            SampleFormat::U16 => build::<u16>(&device, config, channels, sink),
            SampleFormat::U32 => build::<u32>(&device, config, channels, sink),
            SampleFormat::U64 => build::<u64>(&device, config, channels, sink),
            SampleFormat::F32 => build::<f32>(&device, config, channels, sink),
            SampleFormat::F64 => build::<f64>(&device, config, channels, sink),
            other => Err(AudioError::stream(&format!(
                "unsupported sample format {other}"
            ))),
        }?;
        stream.play().map_err(stream_error)?;
        Ok(stream)
    }
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    mut sink: SampleSink,
) -> Result<cpal::Stream, AudioError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                sink.push(data.chunks(channels).map(downmix));
            },
            |e| tracing::warn!(error = %e, "microphone stream error"),
            None,
        )
        .map_err(stream_error)
}

/// One interleaved frame as a single `f32` in `-1.0..=1.0`.
fn downmix<T>(frame: &[T]) -> f32
where
    T: Sample,
    f32: FromSample<T>,
{
    let sum: f32 = frame.iter().map(|s| s.to_sample::<f32>()).sum();
    sum / frame.len().max(1) as f32
}

struct Guard {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_are_averaged() {
        assert_eq!(downmix(&[1.0f32, 0.0]), 0.5);
        assert_eq!(downmix(&[0.25f32]), 0.25);
        assert_eq!(downmix(&[0.5f32, 0.5, -1.0]), 0.0);
    }

    #[test]
    fn integer_samples_convert_to_f32_before_averaging() {
        assert_eq!(downmix(&[i16::MIN, i16::MIN]), -1.0);
        assert_eq!(downmix(&[0u16, 32_768]), -0.5);
    }

    #[test]
    fn an_empty_frame_is_silence_not_nan() {
        assert_eq!(downmix::<f32>(&[]), 0.0);
    }
}
