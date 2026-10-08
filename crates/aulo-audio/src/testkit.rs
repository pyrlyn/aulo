//! Scriptable backends: the test plays the part of the audio thread, by calling
//! [`FakeBackend::feed`] for capture and [`FakeOutputBackend::pull`] for
//! playback, so both run deterministically without a microphone, a speaker or
//! a sleep.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::AudioError;
use crate::capture::{DeviceSelector, InputBackend, InputDevice, StreamGuard};
use crate::playback::{OutputBackend, OutputDevice};
use crate::sink::SampleSink;
use crate::source::SampleSource;

/// A fake device: its name and the rate it delivers mono samples at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeDeviceSpec {
    pub name: String,
    pub sample_rate_hz: u32,
}

impl FakeDeviceSpec {
    pub fn new(name: &str, sample_rate_hz: u32) -> Self {
        Self {
            name: name.to_owned(),
            sample_rate_hz,
        }
    }
}

#[derive(Debug, Default)]
struct State {
    sink: Option<SampleSink>,
    started: Vec<String>,
}

/// Clones share one state, so a test keeps a handle after the backend moved
/// into the code under test.
#[derive(Debug, Clone, Default)]
pub struct FakeBackend {
    devices: Vec<FakeDeviceSpec>,
    state: Arc<Mutex<State>>,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    // The state is plain data, so a poisoned lock is still consistent.
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

impl FakeBackend {
    /// The first device is the default; an empty list has no default device.
    pub fn new(devices: Vec<FakeDeviceSpec>) -> Self {
        Self {
            devices,
            state: Arc::default(),
        }
    }

    /// Delivers mono samples as the audio callback would. Does nothing while
    /// no stream is running.
    pub fn feed(&self, mono: &[f32]) {
        if let Some(sink) = lock(&self.state).sink.as_mut() {
            sink.push(mono.iter().copied());
        }
    }

    pub fn is_streaming(&self) -> bool {
        lock(&self.state).sink.is_some()
    }

    /// Names of the devices streams were started on, oldest first.
    pub fn started_devices(&self) -> Vec<String> {
        lock(&self.state).started.clone()
    }
}

impl InputBackend for FakeBackend {
    type Device = FakeDevice;

    fn find(&self, selector: &DeviceSelector) -> Result<FakeDevice, AudioError> {
        let spec = pick(
            &self.devices,
            selector,
            AudioError::NoInputDevice,
            AudioError::DeviceNotFound,
        )?;
        Ok(FakeDevice {
            spec,
            state: Arc::clone(&self.state),
        })
    }
}

/// The default device is the first; a name picks the device called that.
fn pick(
    devices: &[FakeDeviceSpec],
    selector: &DeviceSelector,
    none: AudioError,
    not_found: fn(aulo_speech::ErrorDetail) -> AudioError,
) -> Result<FakeDeviceSpec, AudioError> {
    match selector {
        DeviceSelector::Default => devices.first().ok_or(none),
        DeviceSelector::Named(wanted) => devices
            .iter()
            .find(|d| d.name == *wanted)
            .ok_or_else(|| not_found(aulo_speech::ErrorDetail::new(wanted))),
    }
    .cloned()
}

#[derive(Debug)]
pub struct FakeDevice {
    spec: FakeDeviceSpec,
    state: Arc<Mutex<State>>,
}

impl InputDevice for FakeDevice {
    fn name(&self) -> &str {
        &self.spec.name
    }

    fn sample_rate_hz(&self) -> u32 {
        self.spec.sample_rate_hz
    }

    fn start(self, sink: SampleSink) -> Result<StreamGuard, AudioError> {
        {
            let mut state = lock(&self.state);
            state.sink = Some(sink);
            state.started.push(self.spec.name);
        }
        Ok(Box::new(Stopper(self.state)))
    }
}

struct Stopper(Arc<Mutex<State>>);

impl Drop for Stopper {
    fn drop(&mut self) {
        lock(&self.0).sink = None;
    }
}

#[derive(Debug, Default)]
struct OutputState {
    source: Option<SampleSource>,
    started: Vec<String>,
}

/// A fake speaker. The test calls [`pull`](Self::pull) where the platform's
/// audio thread would, and gets back exactly what the speaker would play.
#[derive(Debug, Clone, Default)]
pub struct FakeOutputBackend {
    devices: Vec<FakeDeviceSpec>,
    state: Arc<Mutex<OutputState>>,
}

fn lock_output(state: &Mutex<OutputState>) -> MutexGuard<'_, OutputState> {
    // The state is plain data, so a poisoned lock is still consistent.
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

impl FakeOutputBackend {
    /// The first device is the default; an empty list has no default device.
    pub fn new(devices: Vec<FakeDeviceSpec>) -> Self {
        Self {
            devices,
            state: Arc::default(),
        }
    }

    /// Fills `out` as one audio callback would; a stopped stream plays nothing.
    pub fn pull_into(&self, out: &mut [f32]) {
        match lock_output(&self.state).source.as_mut() {
            Some(source) => source.fill(out),
            None => out.fill(0.0),
        }
    }

    /// `frames` mono samples as one audio callback would deliver them.
    pub fn pull(&self, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames];
        self.pull_into(&mut out);
        out
    }

    pub fn is_streaming(&self) -> bool {
        lock_output(&self.state).source.is_some()
    }

    /// Names of the devices streams were started on, oldest first.
    pub fn started_devices(&self) -> Vec<String> {
        lock_output(&self.state).started.clone()
    }
}

impl OutputBackend for FakeOutputBackend {
    type Device = FakeOutputDevice;

    fn find(&self, selector: &DeviceSelector) -> Result<FakeOutputDevice, AudioError> {
        let spec = pick(
            &self.devices,
            selector,
            AudioError::NoOutputDevice,
            AudioError::OutputDeviceNotFound,
        )?;
        Ok(FakeOutputDevice {
            spec,
            state: Arc::clone(&self.state),
        })
    }
}

#[derive(Debug)]
pub struct FakeOutputDevice {
    spec: FakeDeviceSpec,
    state: Arc<Mutex<OutputState>>,
}

impl OutputDevice for FakeOutputDevice {
    fn name(&self) -> &str {
        &self.spec.name
    }

    fn sample_rate_hz(&self) -> u32 {
        self.spec.sample_rate_hz
    }

    fn start(self, source: SampleSource) -> Result<StreamGuard, AudioError> {
        {
            let mut state = lock_output(&self.state);
            state.source = Some(source);
            state.started.push(self.spec.name);
        }
        Ok(Box::new(OutputStopper(self.state)))
    }
}

struct OutputStopper(Arc<Mutex<OutputState>>);

impl Drop for OutputStopper {
    fn drop(&mut self) {
        lock_output(&self.0).source = None;
    }
}
