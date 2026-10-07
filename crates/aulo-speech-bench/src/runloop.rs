//! AVSpeechSynthesizer delivers audio on the main dispatch queue, so on macOS
//! the process's main thread must run its run loop while the benchmark runs on
//! another thread. Elsewhere there is nothing to pump.

/// Runs `work` and returns its result. Call it from the main thread.
#[cfg(target_os = "macos")]
pub fn with_main_run_loop<T: Send>(work: impl FnOnce() -> T + Send) -> T {
    use objc2_foundation::{NSDate, NSRunLoop};

    const PUMP_SECONDS: f64 = 0.02;
    std::thread::scope(|scope| {
        let worker = scope.spawn(work);
        while !worker.is_finished() {
            let until = NSDate::dateWithTimeIntervalSinceNow(PUMP_SECONDS);
            NSRunLoop::currentRunLoop().runUntilDate(&until);
        }
        match worker.join() {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    })
}

#[cfg(not(target_os = "macos"))]
pub fn with_main_run_loop<T: Send>(work: impl FnOnce() -> T + Send) -> T {
    work()
}
