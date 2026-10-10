//! The real backend: `speech-capture` opens the microphone (it owns `cpal`) and
//! this adapter only maps its device and errors onto the aulo traits.

use aulo_speech::ErrorDetail;

use crate::AudioError;
use crate::capture::{DeviceSelector, InputBackend, InputDevice, StreamGuard};
use crate::sink::SampleSink;

fn map_error(e: speech_capture::Error) -> AudioError {
    match e {
        speech_capture::Error::NoInputDevice => AudioError::NoInputDevice,
        speech_capture::Error::DeviceNotFound(name) => {
            AudioError::DeviceNotFound(ErrorDetail::new(&name))
        }
        other => AudioError::stream(&other.to_string()),
    }
}

/// The platform's audio host (CoreAudio, WASAPI, ALSA).
#[derive(Debug, Default, Clone, Copy)]
pub struct CpalBackend;

impl InputBackend for CpalBackend {
    type Device = CpalDevice;

    fn find(&self, selector: &DeviceSelector) -> Result<CpalDevice, AudioError> {
        let name = match selector {
            DeviceSelector::Default => None,
            DeviceSelector::Named(name) => Some(name.as_str()),
        };
        speech_capture::InputDevice::find(name)
            .map(CpalDevice)
            .map_err(map_error)
    }
}

#[derive(Debug)]
pub struct CpalDevice(speech_capture::InputDevice);

impl InputDevice for CpalDevice {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn sample_rate_hz(&self) -> u32 {
        self.0.sample_rate_hz()
    }

    fn start(self, mut sink: SampleSink) -> Result<StreamGuard, AudioError> {
        let stream = self
            .0
            .start(move |mono| sink.push(mono.iter().copied()))
            .map_err(map_error)?;
        Ok(Box::new(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_keep_their_kind() {
        assert_eq!(
            map_error(speech_capture::Error::NoInputDevice),
            AudioError::NoInputDevice
        );
        assert!(matches!(
            map_error(speech_capture::Error::DeviceNotFound("x".into())),
            AudioError::DeviceNotFound(_)
        ));
        assert!(matches!(
            map_error(speech_capture::Error::Stream("denied".into())),
            AudioError::Stream(_)
        ));
    }
}
