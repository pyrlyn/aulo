# Spike T0.6: echo cancellation from Rust

Question: which acoustic echo canceller (AEC) should `aulo-audio` use so that barge-in does not fire on aulo's own voice? The candidates were:

- (a) macOS Voice Processing I/O (VP-IO) through `AVAudioEngine` and `objc2-avf-audio`;
- (b) `webrtc-audio-processing` (WebRTC AEC3) with `cpal` capture and the playback stream as the far-end reference.

Spec context: §5.3 (Audio), §4 data flow step 1 and step 8 (barge-in to silence under 150 ms).

## Summary

| | VP-IO (macOS) | WebRTC AEC3 |
| --- | --- | --- |
| Steady-state echo residual (built-in speakers, built-in mic) | −78 dBFS (≈38 dB below raw; cross-run raw reference) | −67 dBFS (27 dB below raw); −70 dBFS with noise suppression (30 dB) |
| Median ERLE per 100 ms block | 61 dB (cross-run; this includes VP's own noise gating) | 37 dB (AEC only), 49 dB (AEC + NS) |
| False barge-ins in one 7 s echo-only playback (earshot, ≥ 200 ms rule) | 0 (5 speech frames, all at playback onset) | 0 (0 speech frames) |
| Raw mic, same playback | 1 event that stays on (327 of 391 frames are speech) | same |
| Barge-in during playback (near-end talker) | not measurable with a loudspeaker stand-in, see Double talk | detected 1.07 s after onset; first word mostly removed |
| Build | Apple frameworks only, no C/C++ | bundled C++ build with meson + ninja; abseil fetched over the network |
| `unsafe` in our code | almost all of it (objc2 calls, CoreAudio property calls) | none (the wrapper's API is safe) |
| Licence | part of macOS | BSD-3-Clause (WebRTC, with a patent grant) + Apache-2.0 (abseil) |

Recommendation:

- **macOS:** use VP-IO as the default. Keep AEC3 as a config fallback.
- **Windows 11:** use the OS echo canceller on a communications-category WASAPI stream. Treat AEC3 as the fallback once its MSVC build works. This needs its own spike.
- **Linux:** use AEC3, built with the `bundled` feature.

Details and caveats follow.

## Setup

- Machine: Apple M3 Max, 16 cores, macOS 27.0.1 (26A434), macOS SDK 27.0, `rustc 1.99.0 (b940084d7 2026-09-28)` via `mise exec rust@1.99.0`.
- Devices: the system default input and output were AirPods Pro over Bluetooth (HFP input at 24 kHz). A headset has almost no acoustic echo, so it cannot answer this question. All measured runs therefore choose the devices inside the program: **MacBook Pro Speakers** (44.1 kHz, 2 ch) and **MacBook Pro Microphone** (48 kHz, 1 ch). The system defaults were not changed. No run used the AirPods for playback.
- Level: system output volume 66 (read only, not changed), with playback gain 0.3 in the program (sample gain for AEC3, `mainMixerNode.outputVolume` for VP-IO). The raw mic peaked at about −22 dBFS and the echo averaged −40 dBFS RMS.
- Far end: `say -o` text-to-speech (default voice, 7.16 s) converted with `afconvert -f WAVE -d LEI16@48000 -c 1`. Each run records 1 s of silence, then the speech, then about 1 s of tail.
- Near-end stand-in (double talk): a second `say` voice ("Daniel", 3.14 s, "Stop. Wait a second, what about tomorrow?") starts 3.5 s into the recording.
- Total audible playback: 7 runs, about 40 s. There were also several silent runs (gain 0) for the permission check and baselines.
- Microphone permission: `AVCaptureDevice.authorizationStatus(for: .audio)` was `notDetermined` at first. The first capture blocked inside `AudioUnitSetProperty` until the creator answered the system prompt. After that the status was `authorized`.
- Scratch code (not in the repo): `aec3/` (live AEC3 tool plus the `analyze` metrics tool) and `vpio/` (live VP-IO tool), about 600 lines in total. They live under this session's scratchpad (`…/scratchpad/t0.6/`).

### AEC3 tool

- `cpal` 0.18.2 opens one input stream and one output stream at each device's default config, so no device sample rate is changed.
- The output callback writes the far-end samples and advances an atomic frame counter.
- The input callback pushes mono samples into a fixed-size `ringbuf` 0.5.2 SPSC ring. Overflow is counted (it was 0 in every run).
- A worker thread maps the output position to the capture rate and does two things:
  - it feeds the reference with `Processor::analyze_render_frame` in 10 ms frames, as soon as the output callback has consumed them;
  - it runs `process_capture_frame` on every 10 ms capture frame.
- Two processors run on the same capture: AEC3 with the high-pass filter, and AEC3 with the high-pass filter and noise suppression. AGC is off in both.
- The processors run at the capture rate (48 kHz). The reference is resampled linearly from the 48 kHz WAV.
- AEC3 finds the delay by itself (`delay_ms` stat): 52 ms, locked about 1.5 s after playback starts.

### VP-IO tool

- An `AVAudioEngine` with `inputNode.setVoiceProcessingEnabled(true)`. The header says this also enables voice processing on the output node.
- An `AVAudioPlayerNode` → `mainMixerNode` plays the file.
- A tap on input bus 0 records the processed mic. The tap format with voice processing on was 48 kHz with **3 channels**; channel 0 is used.
- The devices are chosen with `AudioUnitSetProperty(kAudioOutputUnitProperty_CurrentDevice)` on the VP unit: element 1 for the mic, element 0 for the speaker. After start, the setting was read back to confirm it.
- An optional `cpal` raw capture of the same mic runs in parallel, trimmed to the engine start.

### Metrics (`analyze`)

- **Echo energy:** RMS in dBFS over the playback window. The noise floor comes from the 0.1–0.9 s pre-roll.
- **ERLE:** raw echo energy minus processed echo energy over the window. A segmental version takes the median over 100 ms blocks in which the raw mic is at least 10 dB above its floor.
- **VAD:** `earshot` 1.2.2 (score ≥ 0.5) on 256-sample frames at 16 kHz. The 16 kHz signal comes from a windowed-sinc resampler.
  - A *false barge-in* is a run of at least 13 speech frames (≈ 200 ms). Events are separated by at least 300 ms of non-speech.
  - An energy VAD (more than 12 dB above the file's own floor) gives a second count.
- **Steady-state numbers:** the 1.6–7.3 s window, which skips the first 0.6 s of playback.
- **Masked interval:** 7.3–8.7 s is removed from every metric because of an unexplained sound, described under Measurement caveats.

## Results

### Echo only (no near-end talker)

Window 1.05–7.3 s. Raw = the mic before any processing, recorded in the same run as AEC3.

| Signal | Echo dBFS | ERLE (whole window) | Median ERLE per block | earshot speech frames | False barge-ins |
| --- | --- | --- | --- | --- | --- |
| Raw mic | −40.0 | — | — | 327 / 391 | 1 (stays on) |
| AEC3 | −67.5 | 27.5 dB | 37.7 dB | 0 | 0 |
| AEC3 + NS | −70.1 | 30.1 dB | 49.5 dB | 0 | 0 |
| VP-IO (same window) | −51.1 | — | — | 5 (onset only) | 0 |
| VP-IO, steady state 1.6–7.3 s, against the AEC3 run's raw | −78.3 | 38.4 dB | 61.0 dB | 0 | 0 |

Notes:

- **VP-IO onset leak.** In the first ≈ 0.5 s of playback, VP-IO lets echo through (−42 and −39 dBFS in the 1.0–1.5 s blocks; three runs showed the same thing). After that its output sits at −90 to −115 dBFS during playback. The leak triggered 5 earshot frames, below the 200 ms rule. A shorter barge-in rule would fire on it.
- **AEC3 convergence.** AEC3 has a smaller convergence blip (−55 dBFS at 1.75–2.0 s, before its delay estimate locks). Otherwise it holds at about −77 dBFS, or about −89 dBFS with noise suppression.
- **Why VP-IO's ERLE uses another run's raw.** While VP-IO runs, a second raw HAL client of the same built-in mic records about **25 dB lower**: −65 dBFS echo and a −94 dBFS floor, against −40 and −60 to −77 dBFS without VP-IO. So VP-IO's ERLE cannot be measured against a simultaneous raw capture. The VP-IO rows use the raw recording from the AEC3 run instead: same devices, same level, and the same file, but not sample-aligned, so the per-block number is approximate.
- **VP-IO AGC.** VP-IO's AGC was on, which is the default. Its pre-roll floor (−74.5 dBFS) is higher than raw because of it.

### Double talk (near-end stand-in during playback)

- **AEC3 (valid test).** The stand-in comes out of the same speaker but is left out of the AEC reference, so AEC3 has to treat it as near-end speech.
  - The talker starts at 3.5 s. earshot marked 108 of 196 frames in the near-end window as speech, and the 200 ms rule fired **1.07 s after onset**.
  - The first word, "Stop.", was mostly removed: −80 dBFS in the 3.5–3.75 s block and −54 dBFS in the next, against about −35 dBFS raw. Later words passed about 10 dB down (window energy −48.7 dBFS, against −73 dBFS of residual echo in the echo-only run).
  - There were no false events outside the window. Noise suppression did not change detection (108 frames, 1.06 s).
- **VP-IO (not valid).** The stand-in played through a separate `cpal` client on the same speaker.
  - With the default other-audio ducking, it did not show up in the VP-IO output: 13 frames, no trigger.
  - With ducking set to `Min` (`setVoiceProcessingOtherAudioDuckingConfiguration`), it partly showed up in a stand-in-only run: about −45 to −56 dBFS, 21 frames.
  - VP-IO either ducks this audio or cancels it as device output. Its header says it takes out "audio that is played from the device", so either is plausible. A loudspeaker stand-in therefore cannot measure how well VP-IO keeps a real user's voice. That needs a person (see Open questions).

## Measurement caveats

- **Unexplained sound at 7.5 s.** In 5 of 7 runs with a `cpal` output stream on MacBook Pro Speakers, a voice-like burst of about −32 dBFS appeared 7.5–8.3 s after the streams started. It appeared even at gain 0 and with a 10 s file. It did not appear with `cpal` output to BlackHole (virtual), or with AVAudioEngine-only output. Its source is unknown. It is masked (7.3–8.7 s) from every metric above, and the last ≈ 0.85 s of speech is masked with it.
- **Room noise.** Pre-roll floors ranged from −56 to −77 dBFS between runs, so there was some activity in the room.
- **One take per condition.** Every condition was measured once, on one machine, at one level, with synthetic speech.

## Build complexity

### VP-IO

- Crates: `objc2` 0.6.5, `block2` 0.6.2, `objc2-avf-audio` 0.3.2, `objc2-foundation` 0.3.2, `objc2-audio-toolbox` 0.3.2, `objc2-core-audio` 0.3.2, `objc2-core-foundation` 0.3.2.
- No system dependencies. A clean release build takes seconds. The binary is 0.88 MB.
- Every generated AVFAudio/CoreAudio method is `unsafe`. About 210 of the tool's 235 lines are inside `unsafe` blocks: engine, tap block, raw pointer reads of `floatChannelData`, `AudioObjectGetPropertyData`, `AudioUnitSetProperty`. In aulo this belongs in one small module inside `aulo-audio` that exposes a safe API.
- Pitfalls found:
  - **Start fails without a format fix.** With voice processing on, `engine.start()` failed with `-10875`, and the failed call was `PerformCommand(*outputNode, kAUInitialize, NULL, 0)`. This happened with the default devices too. Start works after an explicit `connect(mainMixerNode → outputNode, format: inputNode.outputFormat(0))`. The header says the input node's output format and the output node's input format must be the same.
  - **Device selection.** `AUAudioUnit.setDeviceID` sets both directions of the single VP unit, and start then fails. Use `kAudioOutputUnitProperty_CurrentDevice` per element (1 = mic, 0 = speaker).
  - **Playback path.** VP-IO plays audio through the engine, while §5.3 plans `rodio` for playback. On macOS, TTS would go through `AVAudioPlayerNode` or `AVAudioSourceNode` instead.
  - **Effect on other audio and other mic clients.** Other audio is ducked by default; set the ducking level to `Min` (macOS 14+). The raw mic seen by other clients changes while VP-IO runs (the 25 dB drop above), which matters for a wake-word engine on a raw stream.
- Muted speech detection: `setMutedSpeechActivityEventListener` (macOS 14+) reports speech only while `voiceProcessingInputMuted` is on, so it is not a barge-in VAD during playback.

### WebRTC AEC3

- Crates: `webrtc-audio-processing` 2.1.0 (`bundled` feature), `webrtc-audio-processing-config` 2.1.0, `cpal` 0.18.2, `ringbuf` 0.5.2, `hound` 3.5.1, `earshot` 1.2.2.
- The `bundled` build needs a C++ compiler, `pkg-config`, `meson` and `ninja`.
- Clean build without system abseil (`PKG_CONFIG_LIBDIR` set to an empty directory): 45 s wall time on this machine, of which 30 s was user time. Meson downloaded abseil-cpp 20240722.0 from wrapdb, so the **build needs the network**. The build output is 106 MB; the binary is 1.6 MB with abseil linked statically.
- **Distribution trap.** When Homebrew `abseil` (20260817) is installed, the same build silently links `/opt/homebrew/opt/abseil/lib/libabsl_*.dylib` dynamically. That binary will not run on other machines. Release builds must hide system abseil, or vendor the wrap.
- No `unsafe` in our code. `Processor` takes `&self`, which makes a separate render/capture thread split easy.
- Limits:
  - Capture and render must use one sample rate (`Processor::new(sample_rate_hz)`), in 10 ms frames. Different device rates (44.1 kHz out, 48 kHz in here) need resampling. `rubato` from §5.3 would do this; the spike used linear interpolation.
  - The wrapper follows the upstream major version, and its minor versions may break the API. The README advises a `~2.1` requirement.

## Licences

| Component | Licence | aulo (GPL-3.0-or-later OR royalty-free) |
| --- | --- | --- |
| VP-IO / AVFAudio | part of macOS | system framework |
| `webrtc-audio-processing`, `-sys`, `-config` 2.1.0 | BSD-3-Clause (crates.io shows "non-standard" because it uses `license-file`) | compatible; ship the notice |
| Bundled WebRTC sources | BSD-3-Clause + PATENTS (additional IP rights grant); third-party `pffft`, `ooura`, `fft`, `spl_sqrt_floor` carry their own permissive LICENSE files | compatible; ship the notices |
| abseil-cpp (static, via meson wrap) | Apache-2.0 | compatible with GPL-3.0; ship the notice |
| `cpal` 0.18.2, `hound` 3.5.1 | Apache-2.0 | compatible |
| `earshot` 1.2.2, `ringbuf` 0.5.2 | MIT OR Apache-2.0 | compatible |
| `objc2` 0.6.5, `block2` 0.6.2, `objc2-foundation` 0.3.2 | MIT | compatible |
| `objc2-avf-audio`, `objc2-audio-toolbox`, `objc2-core-audio`, `objc2-core-foundation` 0.3.2 | Zlib OR Apache-2.0 OR MIT | compatible |

## Recommendation per OS

### macOS (arm64, 15+)

VP-IO by default:

- It gave the lowest steady-state residual, needs no C++ toolchain or network at build time, and is maintained by Apple.
- Conditions:
  - Keep the AVFAudio code in one `unsafe` module.
  - Route TTS playback through the engine.
  - Apply the format and device fixes above.
  - Set other-audio ducking to `Min`.
  - Guard barge-in against the onset leak: require at least 200 ms of speech, or raise the threshold for about 500 ms after each playback start.
- Keep AEC3 behind a config flag as the fallback. It is measured to work on macOS, and it is the only option here with a verified near-end result.
- Headphones mode (no AEC, or AEC off) stays as in the risk table.

### Windows 11 (x86_64)

No measurement was possible on this machine. From the sources:

- Windows has an OS echo canceller for apps that open communications streams. `IAcousticEchoCancellationControl::SetEchoCancellationRenderEndpoint` (build 22540+) picks the reference endpoint, and the default is the default render device. Windows 11 also ships "Voice Clarity" echo control (Microsoft's hardware test page confirms it exists for 22631, x64 and Arm64). That it is on by default for every PC with the Communications signal processing mode is **unverified** (secondary sources only).
- `cpal` 0.18.2 has no stream-category setting: its source has no `AudioCategory` or `SetClientProperties`. Using the OS AEC therefore means direct WASAPI code with the `windows` crate.
- The `webrtc-audio-processing` CI covers only `ubuntu-latest` and `macos-latest`. An open PR (#102, opened 2026-08-08) says `bundled` builds currently fail on `x86_64-pc-windows-msvc` (**unverified**, contributor report).
- Recommendation: try the OS communications-stream AEC first, with AEC3 as the fallback once the MSVC build works. Plan a Windows spike for both. §5.3 currently says AEC3 for Windows; the build risk above should be added there.

### Linux (x86_64, aarch64)

AEC3 in process, built with `bundled`. This keeps the build hermetic and avoids depending on the distro's library version. Hide system abseil in CI, and vendor or cache the abseil wrap for offline builds.

PipeWire's `module-echo-cancel` (same WebRTC library by default) is a user-level alternative that needs no code. It belongs in the user guide, not as aulo's default.

## Open questions (for the creator)

1. **VP-IO with a real user.** How well does VP-IO keep a real user's voice during playback, and how fast is barge-in detected? A 2-minute test with a person saying "stop" during TTS, on both VP-IO and AEC3, would settle the macOS default. A loudspeaker stand-in cannot measure this.
2. **AEC3 near-end loss.** AEC3 delayed near-end detection by about 1 s in double talk and cut the first word. Is tuning AEC3 (`experimental-aec3-config` feature, not semver-stable) in scope, or do we accept it as the fallback?
3. **Sound at 7.5 s.** Should a follow-up find the source of the burst that appears with `cpal` output on the built-in speakers? It may matter for `rodio`/`cpal` playback on macOS generally.
4. **Windows spike.** Should a Windows 11 spike (OS AEC via WASAPI communications streams vs AEC3 on MSVC) be added to `roadmap.md`?

## Sources

All checked 2026-10-07.

- crates.io registry API, `https://crates.io/api/v1/crates/<name>`: versions, dates and licences.
  - objc2 0.6.5 (2026-10-05); block2 0.6.2 (2025-10-04).
  - objc2-avf-audio, objc2-foundation, objc2-audio-toolbox, objc2-core-audio, objc2-core-foundation 0.3.2 (2025-10-04).
  - webrtc-audio-processing, -sys, -config 2.1.0 (2026-05-13).
  - cpal 0.18.2 (2026-08-16); earshot 1.2.2 (2026-08-19); ringbuf 0.5.2 (2026-09-13); hound 3.5.1 (2023-09-25).
- Apple macOS SDK 27.0 header `AVFAudio.framework/Headers/AVAudioIONode.h`:
  - `setVoiceProcessingEnabled:error:` discussion (both nodes, format equality, stopped engine);
  - `setMutedSpeechActivityEventListener` (macOS 14);
  - `voiceProcessingOtherAudioDuckingConfiguration` (macOS 14).
  - Online page: https://developer.apple.com/documentation/avfaudio/avaudioionode
- `webrtc-audio-processing` 2.1.0 crate sources (https://github.com/tonarino/webrtc-audio-processing):
  - README: features, `bundled` tool list, versioning advice;
  - `src/lib.rs`: `Processor` API;
  - `webrtc-audio-processing-sys` `build.rs`: abseil detection;
  - bundled `webrtc-audio-processing/meson.build` (version 2.1), `NEWS` (WebRTC M131, abseil 20240722);
  - `subprojects/abseil-cpp.wrap` (wrapdb 20240722.0-3);
  - `COPYING`, `webrtc/LICENSE`, `webrtc/PATENTS`, `webrtc/third_party/pffft/LICENSE`.
- GitHub API for tonarino/webrtc-audio-processing: licence BSD-3-Clause, `.github/workflows/rust.yml` matrix (ubuntu-latest, macos-latest), PR #102 (open, 2026-08-08), issue #34 (open).
- GitHub API for abseil/abseil-cpp: licence Apache-2.0.
- cpal 0.18.2 sources (https://github.com/RustAudio/cpal): `src/traits.rs` API; no `AudioCategory` or `SetClientProperties` in `src/`.
- earshot 1.2.2 README and `src/lib.rs` (https://github.com/pykeio/earshot): 256-sample frames at 16 kHz, 0.5 threshold.
- Microsoft, Acoustic echo cancellation sample (page dated 2022-05-12): https://learn.microsoft.com/en-us/samples/microsoft/windows-classic-samples/acousticechocancellation
- Microsoft, Voice Clarity Loopback Bandwidth Test (updated 2024-09-13): https://learn.microsoft.com/en-us/windows-hardware/test/hlk/testref/voice-clarity-system-verification-test-loopback-bandwidth
- Secondary only (**unverified**): Voice Clarity is on by default and used by apps in the Communications signal processing mode. Leads: https://pureinfotech.com/whats-voice-clarity-windows-11-enable/ and https://www.techradar.com/computing/windows/windows-11s-ai-powered-voice-clarity-feature-improves-your-video-chats-plus-setup-has-a-new-look-finally
- PipeWire, Echo Cancel module docs (PipeWire 1.6.9): https://docs.pipewire.org/page_module_echo_cancel.html (default method `aec/libspa-aec-webrtc`).
