//! The real output backend: `speech-capture` opens the speaker (it owns `cpal`)
//! and this adapter only maps its device and errors onto the aulo traits.

use aulo_speech::ErrorDetail;

use crate::AudioError;
use crate::capture::{DeviceSelector, StreamGuard};
use crate::cpal_input::CpalBackend;
use crate::playback::{OutputBackend, OutputDevice};
use crate::source::SampleSource;

fn map_error(e: speech_capture::Error) -> AudioError {
    match e {
        speech_capture::Error::NoOutputDevice => AudioError::NoOutputDevice,
        speech_capture::Error::OutputDeviceNotFound(name) => {
            AudioError::OutputDeviceNotFound(ErrorDetail::new(&name))
        }
        other => AudioError::output_stream(&other.to_string()),
    }
}

impl OutputBackend for CpalBackend {
    type Device = CpalOutputDevice;

    fn find(&self, selector: &DeviceSelector) -> Result<CpalOutputDevice, AudioError> {
        let name = match selector {
            DeviceSelector::Default => None,
            DeviceSelector::Named(name) => Some(name.as_str()),
        };
        speech_capture::OutputDevice::find(name)
            .map(CpalOutputDevice)
            .map_err(map_error)
    }
}

#[derive(Debug)]
pub struct CpalOutputDevice(speech_capture::OutputDevice);

impl OutputDevice for CpalOutputDevice {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn sample_rate_hz(&self) -> u32 {
        self.0.sample_rate_hz()
    }

    fn start(self, mut source: SampleSource) -> Result<StreamGuard, AudioError> {
        let stream = self
            .0
            .start(move |mono| source.fill(mono))
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
            map_error(speech_capture::Error::NoOutputDevice),
            AudioError::NoOutputDevice
        );
        assert!(matches!(
            map_error(speech_capture::Error::OutputDeviceNotFound("x".into())),
            AudioError::OutputDeviceNotFound(_)
        ));
        assert!(matches!(
            map_error(speech_capture::Error::Output("busy".into())),
            AudioError::OutputStream(_)
        ));
    }
}
