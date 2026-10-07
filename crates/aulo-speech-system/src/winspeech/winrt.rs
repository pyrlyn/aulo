//! The WinRT side: `SpeechSynthesizer` and the stream it returns. Every
//! Windows call is here, and all of them run on the worker thread.

use std::io::{self, Read};
use std::thread;
use std::time::Duration;

use aulo_speech::{SpeechError, SpeechRate, Voice};
use windows::Media::SpeechSynthesis::{SpeechSynthesisStream, SpeechSynthesizer, VoiceInformation};
use windows::Storage::Streams::DataReader;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use windows::core::HSTRING;

use super::convert::speaking_rate;
use super::worker::{Backend, Catalog};

/// How often a worker waiting for a sentence looks for a cancel. A cancel
/// reaches `IAsyncOperation::Cancel` within this, well inside barge-in's budget.
const CANCEL_CHECK: Duration = Duration::from_millis(5);
/// Bytes loaded from the stream per call, so one read never holds more.
const LOAD_BYTES: u64 = 8 * 1024;

/// `AsyncStatus` values from the WinRT ABI. The enum itself lives in
/// `windows-future`, which the `windows` crate does not re-export, and the
/// values are part of a stable ABI, so the raw field is compared instead of
/// taking another dependency.
const STATUS_STARTED: i32 = 0;
const STATUS_CANCELED: i32 = 2;

/// Joins the multithreaded apartment for the worker's lifetime. WinRT async
/// results are awaited by blocking, which an STA thread must never do.
struct Apartment(bool);

impl Apartment {
    #[allow(unsafe_code)]
    fn enter() -> Self {
        // SAFETY: takes no pointers. `RPC_E_CHANGED_MODE` (the thread already
        // chose another apartment) is an Err, so it is never paired with an
        // uninitialize below, and WinRT works in either apartment.
        Self(unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok())
    }
}

impl Drop for Apartment {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful `RoInitialize` of this thread.
            unsafe { RoUninitialize() };
        }
    }
}

pub(super) struct WinRtBackend {
    synthesizer: SpeechSynthesizer,
    infos: Vec<VoiceInformation>,
    // Last, so the apartment outlives every WinRT object above.
    _apartment: Apartment,
}

impl WinRtBackend {
    pub(super) fn open() -> Result<Self, SpeechError> {
        let apartment = Apartment::enter();
        let unavailable =
            |what: &str, e: windows::core::Error| SpeechError::unavailable(&format!("{what}: {e}"));
        let synthesizer =
            SpeechSynthesizer::new().map_err(|e| unavailable("SpeechSynthesizer", e))?;
        let infos = SpeechSynthesizer::AllVoices()
            .map_err(|e| unavailable("AllVoices", e))?
            .into_iter()
            .collect();
        Ok(Self {
            synthesizer,
            infos,
            _apartment: apartment,
        })
    }
}

fn failed(e: windows::core::Error) -> SpeechError {
    SpeechError::failed(&format!("Windows speech: {e}"))
}

fn describe(info: &VoiceInformation) -> windows::core::Result<Voice> {
    let language = info.Language()?.to_string();
    Ok(Voice {
        id: info.Id()?.to_string(),
        name: info.DisplayName()?.to_string(),
        language: (!language.is_empty()).then_some(language),
    })
}

impl Backend for WinRtBackend {
    fn catalog(&self) -> Result<Catalog, SpeechError> {
        let voices = self
            .infos
            .iter()
            .map(describe)
            .collect::<Result<Vec<_>, _>>()
            .map_err(failed)?;
        let default_id = SpeechSynthesizer::DefaultVoice()
            .and_then(|voice| voice.Id())
            .map(|id| id.to_string());
        let default = default_id
            .ok()
            .and_then(|id| voices.iter().position(|v| v.id == id));
        Ok(Catalog { voices, default })
    }

    fn synthesize(
        &mut self,
        text: &str,
        voice: &str,
        rate: SpeechRate,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<Box<dyn Read>>, SpeechError> {
        let info = self
            .infos
            .iter()
            .find(|info| info.Id().is_ok_and(|id| id == voice))
            .ok_or_else(|| SpeechError::unsupported(voice))?;
        self.synthesizer.SetVoice(info).map_err(failed)?;
        let options = self.synthesizer.Options().map_err(failed)?;
        options
            .SetSpeakingRate(speaking_rate(rate))
            .map_err(failed)?;
        let operation = self
            .synthesizer
            .SynthesizeTextToStreamAsync(&HSTRING::from(text))
            .map_err(failed)?;
        // Polled rather than joined, so a cancel can drop the operation while
        // the voice is still synthesizing.
        while operation.Status().map_err(failed)?.0 == STATUS_STARTED {
            if cancelled() {
                // Already finished or closed is as good as cancelled.
                let _ = operation.Cancel();
                return Ok(None);
            }
            thread::sleep(CANCEL_CHECK);
        }
        if operation.Status().map_err(failed)?.0 == STATUS_CANCELED {
            return Ok(None);
        }
        let stream = operation.GetResults().map_err(failed)?;
        Ok(Some(Box::new(StreamReader::new(&stream).map_err(failed)?)))
    }
}

/// A `std::io::Read` over the synthesized stream, loading a bounded piece per
/// call. The stream is already in memory, so each load returns at once.
struct StreamReader {
    reader: DataReader,
    remaining: u64,
}

impl StreamReader {
    fn new(stream: &SpeechSynthesisStream) -> windows::core::Result<Self> {
        let remaining = stream.Size()?;
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
        Ok(Self { reader, remaining })
    }
}

impl Read for StreamReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let wanted = (buf.len() as u64).min(self.remaining).min(LOAD_BYTES);
        if wanted == 0 {
            return Ok(0);
        }
        // `wanted` is at most `LOAD_BYTES`, so the conversion cannot fail.
        let wanted = u32::try_from(wanted).unwrap_or(u32::MAX);
        let to_io = |e: windows::core::Error| io::Error::other(e.to_string());
        let loaded = self.reader.LoadAsync(wanted).and_then(|op| op.join());
        let loaded = loaded.map_err(to_io)?.min(wanted) as usize;
        self.reader.ReadBytes(&mut buf[..loaded]).map_err(to_io)?;
        self.remaining -= loaded as u64;
        Ok(loaded)
    }
}
