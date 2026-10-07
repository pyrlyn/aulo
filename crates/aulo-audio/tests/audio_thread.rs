//! Proves the audio-callback path allocates nothing, with and without a full
//! ring, by counting the allocations the feeding thread makes.

// A counting allocator is the only way to observe allocations, and needs `unsafe`.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use aulo_audio::testkit::{FakeBackend, FakeDeviceSpec};
use aulo_audio::{Capture, DeviceSelector, FRAME_SAMPLES};

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
}

struct Counting;

// SAFETY: every call is forwarded unchanged to the system allocator.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.get() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.get() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn feeding_the_ring_never_allocates_even_when_it_overflows() {
    let fake = FakeBackend::new(vec![FakeDeviceSpec::new("mic", 48_000)]);
    // Two frames of room, then 2 s of audio: the ring fills and overflows.
    let mut capture = Capture::start(&fake, &DeviceSelector::Default, 2).unwrap();
    let callback = vec![0.1f32; 480];

    COUNTING.set(true);
    for _ in 0..200 {
        fake.feed(&callback);
    }
    COUNTING.set(false);

    assert_eq!(ALLOCATIONS.load(Ordering::Relaxed), 0);
    assert!(capture.overflowed_samples() > 0);
    assert_eq!(capture.next_frame().unwrap().len(), FRAME_SAMPLES);
}
