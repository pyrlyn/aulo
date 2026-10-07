//! A scriptable input backend: the test plays the part of the audio thread by
//! calling [`FakeBackend::feed`], so capture runs deterministically without a
//! microphone or a sleep.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::capture::{DeviceSelector, InputBackend, InputDevice, StreamGuard};
use crate::sink::SampleSink;
use crate::{AudioError, MicPermission, MicPermissionProbe};

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
        let spec = match selector {
            DeviceSelector::Default => self.devices.first().ok_or(AudioError::NoInputDevice)?,
            DeviceSelector::Named(wanted) => self
                .devices
                .iter()
                .find(|d| d.name == *wanted)
                .ok_or_else(|| AudioError::DeviceNotFound(aulo_speech::ErrorDetail::new(wanted)))?,
        };
        Ok(FakeDevice {
            spec: spec.clone(),
            state: Arc::clone(&self.state),
        })
    }
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

/// A probe that answers with a fixed permission and counts the questions.
#[derive(Debug, Clone)]
pub struct FakeProbe {
    answer: MicPermission,
    asked: Arc<AtomicUsize>,
}

impl FakeProbe {
    pub fn new(answer: MicPermission) -> Self {
        Self {
            answer,
            asked: Arc::default(),
        }
    }

    pub fn asked(&self) -> usize {
        self.asked.load(Ordering::Relaxed)
    }
}

impl MicPermissionProbe for FakeProbe {
    fn status(&self) -> MicPermission {
        self.asked.fetch_add(1, Ordering::Relaxed);
        self.answer
    }
}
