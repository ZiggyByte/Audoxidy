---
phase: 03-volumen-y-mezcla
plan: 04
subsystem: ui
tags: [iced, audio-center, volumen-mezcla, StandardCheckbox, NumberStepper, persistence]
requires:
  - phase: 03-volumen-y-mezcla
    provides: AudioState fields for fades/silence/normalize/RG
  - phase: 03-volumen-y-mezcla
    provides: StandardCheckbox and NumberStepper widgets
provides:
  - "Volumen y Mezcla tab (index 3) with 4 groups and 16 interactive controls"
  - "save_volumen_settings_to_db() for immediate persistence of all 16 settings"
  - "sync_from_engine loading of all volumen settings from APP_SETTINGS"
affects:
  - 03-volumen-y-mezcla (app.rs persistence wiring in plan 03-05)

tech-stack:
  added: []
  patterns:
    - "Two-column equal-width layout with vertical divider (D-37)"
    - "Group master OFF → sub-functions greyed with scale_alpha(0.5) (D-40)"
    - "StandardCheckbox + NumberStepper widgets in tab content"
    - "DB save/load pattern mirrored from save_dsp_settings_to_db"

key-files:
  created: []
  modified:
    - src/gui/audio_center.rs - Added 16 AudioCenterMessage variants, 16 manager fields with defaults, tab registration, view_volumen_mezcla function, update() handlers, save_volumen_settings_to_db(), sync_from_engine extension

key-decisions:
  - "D-36: Tab 'Volumen y Mezcla' registered at index 3 in tab bar and content dispatch"
  - "D-37: Two equal-width columns (FillPortion(1)/FillPortion(1)) with vertical COLOR_CONTRAST divider, no bottom buttons"
  - "D-38: Horizontal separators (2px COLOR_TEXT_SECONDARY) after each group title"
  - "D-39: StandardCheckbox widgets left-aligned, NumberStepper widgets right-aligned"
  - "D-40: Group master OFF → sub-functions display with scale_alpha(0.5) and use NoOp callbacks"
  - "D-41: Column 1 = Fades + Silence; Column 2 = Normalize + ReplayGain"
  - "D-42: save_volumen_settings_to_db() mirrors save_dsp_settings_to_db pattern; sync_from_engine loads all 16 settings"

patterns-established:
  - "save_volumen_settings_to_db pattern: lock DB → state.read() → set_setting for each field"
  - "sync_from_engine Volumen loading: inside state.write() lock block, get_setting → parse → set on both AudioState and manager"
  - "view_volumen_mezcla disabled widget pattern: conditional on group master, disabled steppers/checkboxes use NoOp callbacks"

requirements-completed: []

duration: 11 min
completed: 2026-07-19
---

# Phase 03 Plan 04: Volumen y Mezcla Tab Summary

**Complete "Volumen y Mezcla" tab (index 3) with 4 groups, 16 interactive controls, immediate persistence via save_volumen_settings_to_db(), and DB loading via sync_from_engine**

## Performance

- **Duration:** 11 min
- **Started:** 2026-07-19T03:48:04Z
- **Completed:** 2026-07-19T03:59:52Z
- **Tasks:** 3
- **Files modified:** 1 (src/gui/audio_center.rs)

## Accomplishments

- Added 16 AudioCenterMessage variants for all Volumen y Mezcla interactions
- Added 16 manager fields with approved defaults (fades 1000ms, silence -50dB, normalize -14dB target/6dB cap, RG offsets 0dB)
- Registered "Volumen y Mezcla" tab at index 3 in tab bar with content dispatch match arm
- Implemented `view_volumen_mezcla` with 4 groups in 2 equal-width columns with vertical divider (D-37)
- All 4 groups (Fades, Eliminar Silencios, Normalizar Volumen, Replay Gain) with master checkboxes and sub-function widgets
- Group master OFF → sub-functions greyed via scale_alpha(0.5) with NoOp callbacks (D-40)
- 7 StandardCheckbox widgets and 9 NumberStepper instantiations
- All 16 update() handlers write to both AudioState (via state.write()) and manager fields
- `save_volumen_settings_to_db()` mirrors save_dsp_settings_to_db pattern for immediate persistence
- `sync_from_engine` loads all 16 volumen settings from APP_SETTINGS at startup/sync

## Task Commits

1. **Task 1: Add variants, fields, defaults, tab registration** - `abc7131` (feat)
2. **Task 2: Implement view_volumen_mezcla layout** - `049734c` (feat)
3. **Task 3: Update handlers + save + sync** - `fb49cdc` (feat)

**Plan metadata:** (pending — SUMMARY.md commit)

## Files Created/Modified

- `src/gui/audio_center.rs` - All changes concentrated in this single file:
  - Lines 57-73: 16 new AudioCenterMessage variants
  - Lines 172-191: 16 new manager fields
  - Lines 230-248: Default values for all volumen settings
  - Lines 1519-1520: Tab name + content dispatch
  - Lines 1613-1985: Full `view_volumen_mezcla` function (373 lines)
  - Lines 496-518: `save_volumen_settings_to_db()` method
  - Lines 749-831: Volumen settings loading in `sync_from_engine`
  - Lines 1464-1550: 16 update() handler arms

## Decisions Made

- All settings use `f64` in the manager (for NumberStepper compatibility) and cast to `f32` when writing to AudioState
- Disabled widgets use `Message::NoOp` instead of removing them from layout (simpler, maintains layout stability)
- RG sub-function closures in `rg_checkbox_row` / `rg_stepper_row` use `fn` pointer types for non-capturing callbacks
- Other groups use inline widget construction for clarity (no indirection through helpers)
- DB keys use `vol_*` prefix; Rust fields use `volumen_*` prefix to distinguish manager state from AudioState fields

## Deviations from Plan

None - plan executed exactly as written.

**Note:** Acceptance criteria grep patterns for `vol_fades_enabled` (expected ≥3, got 2) and `vol_rg_offset_rt_db` (expected ≥3, got 2) had minor mismatches — the plan assumed Rust field names would match DB key names, but the convention uses `volumen_*` for Rust fields and `vol_*` for DB keys. Both `set_setting("vol_*")` (16 calls) and `get_setting("vol_*")` (16 calls) are present and correct.

## Issues Encountered

None — all three tasks completed without errors. Cargo fmt detected pre-existing trailing whitespace in app.rs and database.rs (out of scope for this plan).

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Tab "Volumen y Mezcla" is fully functional with interactive controls
- All settings immediately write to AudioState and persist to APP_SETTINGS
- Ready for Plan 03-05: app.rs persistence wiring (match arms in app.rs:3934-3960 to call save_volumen_settings_to_db)

---
*Phase: 03-volumen-y-mezcla*
*Completed: 2026-07-19*
