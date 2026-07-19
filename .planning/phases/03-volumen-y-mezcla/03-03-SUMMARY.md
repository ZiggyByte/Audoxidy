---
phase: 03-volumen-y-mezcla
plan: 03
subsystem: audio
tags: [audio, dsp, loudness, gain, fade, silence, rms, normalization, limiter, rg, replaygain, spin-wait, decoder]

# Dependency graph
requires:
  - phase: 03-01
    provides: AudioState fields (fades, silence, normalization, RG offsets), DspChain force_limiter_on/restore_limiter, AudioManager API
  - phase: 03-02
    provides: StandardCheckbox + NumberStepper widgets, Fix B2 (suffix stripping in CustomSlider)
provides:
  - Single loudness gain point (RG tags + offsets + normalization in dB, converted to linear once)
  - Fade state machine with equal-power curve and 500ms linear volume smoothing
  - Silence detection with +3dB hysteresis and edge trimming
  - RMS normalization with ~400ms sliding window, attack/release smoothing
  - Limiter auto-on/restore for normalization (D-25)
  - Fix B1: spin-wait DSP lock acquisition up to 5ms (D-44)
affects:
  - 03-04 (gui integration — reads AudioState fields written by decoder features)
  - 03-05 (persistence — normalization/limiter state may need DB save)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Single loudness gain point: all sources summed in dB, converted to linear once, applied before DspChain"
    - "Chain order: gain_linear → DspChain → fades×volume → ringbuf (D-03)"
    - "FadeState enum: Idle/FadingIn/FadingOut/Smoothing with equal-power curve (D-12)"
    - "Silence detection: peak-per-frame, +3dB hysteresis, pre-fade measurement (D-17, D-19)"
    - "RMS normalization: ~400ms VecDeque sliding window, 3000ms attack / 1000ms release (D-24)"
    - "RG offsets: ~100ms EMA anti-click ramp (D-29)"
    - "Fix B1: spin-wait up to 5ms with std::hint::spin_loop() before DSP bypass fallback"

key-files:
  created: []
  modified:
    - src/audio/decoder.rs (main file — 604 insertions: gain point, fades, silence, RMS, spin-wait)
    - src/audio/engine.rs (AudioEngine wrappers: force_limiter_on / restore_limiter)

key-decisions:
  - "FadeState derives Copy — match on value (not &mut) to reassign safely within match arms"
  - "step_per_frame stores per-sample step; per-batch advance computed as step * frames_in_batch at runtime"
  - "RMS window capacity computed as 0.4 * device_sample_rate at decode loop startup"
  - "Silence detection and RMS measurement use brief state.read() locks to minimize contention"
  - "Limiter auto-on/restore via AudioEngine::force_limiter_on/restore_limiter thin wrappers over DspChain"

requirements-completed: []

# Metrics
duration: ~10min
completed: 2026-07-19
---

# Phase 03 Plan 03: Decoder Pipeline — Loudness Gain, Fades, Silence, RMS Normalization, Fix B1

**Single loudness gain point with RG offsets + EMA smoothing, fade state machine with equal-power curves, silence detection with hysteresis, RMS normalization with attack/release smoothing, and Fix B1 spin-wait DSP lock acquisition**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-07-19T03:34:58Z
- **Completed:** 2026-07-19T03:44:27Z
- **Tasks:** 3
- **Files modified:** 2 (decoder.rs: +604/-112, engine.rs: +84/-N/A)

## Accomplishments

- **Single loudness gain point (D-01-D-06, D-26-D-30):** Sums RG album+track tags, EMA-smoothed RG offsets (album/track/RT), and RMS normalization output in dB domain, converts to linear once with +12dB safety cap. RG master enabled gate (D-30). RT analysis fallback when no tags exist (D-28). ~100ms EMA anti-click ramp for RG offset UI changes (D-29) with NaN guards.
- **Fade state machine + silence detection (D-07-D-20):** FadeState enum (Idle/FadingIn/FadingOut/Smoothing) with equal-power curve for musical fades. Fade-in triggered only on natural track start after previous EOF (D-09). Fade-out triggered at effective_end − fade_out_ms (D-10). Volume smoothing: 500ms linear ramp (D-08). Silence detection: peak-per-frame, +3dB hysteresis, pre-fade measurement (D-14-D-19). Edge trimming: discard leading silence, track effective_end for early fade-out (D-18). Chain reorder per D-03: gain → DSP → fades×volume.
- **RMS normalization + Fix B1 (D-21-D-25, D-44):** ~400ms sliding RMS window with running sum. Attack 3000ms, release 1000ms exponential smoothing. feeds normalization_gain_db into the single loudness gain point. Limiter auto-on when normalize_enabled=true, restore to previous user state on disable. RMS state reset when normalization disabled. Fix B1: spin-wait up to 5ms (std::hint::spin_loop()) for DSP lock acquisition; only falls back to bypass after timeout. Timeout message changed to indicate bypass is rare. engine.rs: AudioEngine force_limiter_on()/restore_limiter() wrappers.

## Task Commits

Each task was committed atomically:

1. **Task 1: Single loudness gain point + RG offsets** — `3e49ac0` (feat)
2. **Task 2: Fade envelope state machine + silence detection + effective_end** — `92934d0` (feat)
3. **Task 3: RMS normalization + limiter auto-on + Fix B1** — `f422cef` (feat)

## Files Created/Modified

- `src/audio/decoder.rs` — Full audio loudness pipeline: single gain point, FadeState enum, silence detection, RMS normalization, spin-wait DSP lock (604 insertions, 112 deletions)
- `src/audio/engine.rs` — AudioEngine::force_limiter_on() and restore_limiter() thin wrappers for D-25

## Decisions Made

- **FadeState Copy derive — match on value:** Since FadeState derives Copy and Clone, the fade coefficient match works on the value (not &mut reference) and reconstructs the enum for state transitions. This avoids borrow-checker issues with simultaneous mutable references to inner fields.
- **step_per_frame semantics:** Despite the name, stores per-sample step size. Per-batch advance computed as `step * frames_in_batch` at runtime to handle variable batch sizes from the resampler.
- **Decoupled state reads:** Gain computation, EMA smoothing, silence detection, and RMS measurement each use brief `state.read()` calls to minimize RwLock contention. The AudioState lock is released before DSP processing.
- **RMS window pre-allocation:** Window capacity computed at decode loop startup from device sample rate, avoiding dynamic reallocation in the hot path.

## Deviations from Plan

None — plan executed exactly as written. All acceptance criteria met, cargo check passes, 146 tests green.

## Issues Encountered

- **Pre-existing cargo fmt failures:** `cargo fmt --check` reports trailing whitespace errors in src/db/database.rs and src/gui/app.rs — these are pre-existing issues in files not modified by this plan. decoder.rs and engine.rs are properly formatted.

## Known Stubs

None — all features are fully wired. normalization_gain_db is a real value computed by the RMS loop, not a placeholder. All state variables have concrete initial values with proper reset logic.

## Next Phase Readiness

- All audio processing decisions from CONTEXT.md that touch the decode loop are implemented
- Single loudness gain point ready for UI integration (Plan 03-04)
- Fades, silence detection, and RMS normalization all functional — widgets can toggle them via AudioState fields
- Fix B1 eliminates DSP dropout during GUI interaction
- Ready for Plan 03-04 (GUI integration: tab, layout, widgets + AudioState wiring)

---
*Phase: 03-volumen-y-mezcla*
*Completed: 2026-07-19*
