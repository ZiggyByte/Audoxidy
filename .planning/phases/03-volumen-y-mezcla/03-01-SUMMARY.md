---
phase: 03-volumen-y-mezcla
plan: 01
subsystem: audio
tags: [audio, dsp, state, limiter, volume, mezcla, audiostate, dspchain]

# Dependency graph
requires: []
provides:
  - 14 new AudioState fields with correct factory defaults for all volume/mix features
  - DspChain::force_limiter_on() and restore_limiter() for normalization auto-on (D-25)
  - AudioManager::force_limiter_on() and restore_limiter() thread-safe public API
affects:
  - 03-02 (widgets — reads AudioState fields for UI)
  - 03-03 (decoder — calls force_limiter_on/restore_limiter, reads AudioState fields)
  - 03-04 (gui integration — wires AudioState fields to UI)
  - 03-05 (persistence — saves/loads AudioState fields to/from DB)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Volumen settings stored as public fields on existing AudioState struct with Clone derive"
    - "Limiter auto-on/restore via parking_lot::RwLock delegation (AudioManager → with_dsp_mut → DspChain)"
    - "TDD RED→GREEN cycle for AudioState field additions"

key-files:
  created: []
  modified:
    - src/audio/engine.rs (AudioState struct + Default — 14 new fields)
    - src/audio/dsp.rs (DspChain — force_limiter_on/restore_limiter)
    - src/audio/manager.rs (AudioManager — force_limiter_on/restore_limiter public API)
    - src/audio/tests.rs (test_audio_state_defaults — extended with new field assertions)

key-decisions:
  - "All 14 new AudioState fields are Clone (bool, f32) — AudioState still derives Clone"
  - "Existing replay_gain_track_enabled and replay_gain_album_enabled kept as-is (default true, not modified)"
  - "Existing volume: f32 = 0.3 kept as fallback; actual value loaded from DB in Plan 03-05"
  - "Limiter ceiling stays at -1.0 dBTP — no changes to Limiter struct"
  - "force_limiter_on closure captured via mutable external variable since with_dsp_mut returns ()"

patterns-established:
  - "AudioState struct fields: public bool/f32 with inline comments for range and step size"
  - "Default impl: one field per line with domain-grouping comments (Fades / Silence / Normalization / RG offsets)"
  - "DspChain helpers: simple O(1) boolean toggles, no allocation, no error paths"
  - "AudioManager API: delegates via with_dsp_mut() closure, external mutable capture for return values"

requirements-completed: []

# Metrics
duration: ~7min
completed: 2026-07-19
---

# Phase 03 Plan 01: Foundation Data Structures — AudioState fields, DspChain limiter helpers, and AudioManager public API

**14 new AudioState fields with approved defaults, DspChain limiter auto-on/restore for normalization (D-25), and thread-safe AudioManager public API delegation**

## Performance

- **Duration:** ~7 min
- **Started:** 2026-07-19T03:11:00Z
- **Completed:** 2026-07-19T03:18:56Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Added 14 new AudioState fields across 4 groups (Fades, Silence, Normalization, ReplayGain offsets) with correct factory defaults per the approved defaults table
- Extended test_audio_state_defaults with assertions for all 14 new fields
- Added DspChain::force_limiter_on() / restore_limiter() for normalization auto-on (D-25)
- Added AudioManager::force_limiter_on() / restore_limiter() public API delegating through parking_lot::RwLock
- All 146 existing tests pass with zero regressions; cargo check and cargo build --release clean

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Failing tests for new AudioState fields** — `642f5ad` (test)
2. **Task 1 GREEN: Implement 14 AudioState fields** — `d54a666` (feat)
3. **Task 2: DspChain limiter helpers + AudioManager API** — `f937897` (feat)

**Plan metadata:** (pending)

## Files Created/Modified

- `src/audio/engine.rs` — AudioState struct: added 14 new fields (Fades, Silence, Normalization, RG offsets); Default impl: added corresponding default values
- `src/audio/dsp.rs` — DspChain: added `force_limiter_on()` and `restore_limiter()` methods between process_frame() and set_sample_rate()
- `src/audio/manager.rs` — AudioManager: added `force_limiter_on()` and `restore_limiter()` public API delegating via with_dsp_mut()
- `src/audio/tests.rs` — test_audio_state_defaults: extended with 14 new field assertions

## Decisions Made

- **External capture for with_dsp_mut return values:** Since `with_dsp_mut` takes `FnOnce(&mut DspChain)` returning `()`, the `force_limiter_on()` return value needed a mutable `was_enabled` variable captured from the outer scope — the plan's compact syntax `self.with_dsp_mut(|dsp| dsp.force_limiter_on())` was incompatible with the closure signature. Fixed in implementation.
- **No changes to existing RG flags:** `replay_gain_track_enabled` and `replay_gain_album_enabled` were kept exactly as-is (default `true`), per the plan's explicit directive. The new `rg_master_enabled` is a separate master toggle.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed with_dsp_mut return type mismatch**
- **Found during:** Task 2 (AudioManager API)
- **Issue:** `with_dsp_mut` closure signature returns `()`, but `force_limiter_on()` returns `bool`. The plan's proposed one-liner `self.with_dsp_mut(|dsp| dsp.force_limiter_on())` caused type error E0308.
- **Fix:** Captured return value via external mutable variable: `let mut was_enabled = false; self.with_dsp_mut(|dsp| { was_enabled = dsp.force_limiter_on(); }); was_enabled`
- **Files modified:** src/audio/manager.rs
- **Verification:** `cargo check` passes, `cargo test` 146/146
- **Committed in:** f937897

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Minor implementation detail — the semantic behavior is identical. No scope creep.

## Issues Encountered
None.

## Known Stubs
None — all fields are fully defined with concrete defaults. No placeholder values or TODO comments.

## Next Phase Readiness
- Foundation data structures complete — all downstream plans (03-02 widgets, 03-03 decoder, 03-04 GUI, 03-05 persistence) can read from these AudioState fields and call the limiter API
- Ready for Plan 03-02 (StandardCheckbox + NumberStepper widgets)

---
*Phase: 03-volumen-y-mezcla*
*Completed: 2026-07-19*
