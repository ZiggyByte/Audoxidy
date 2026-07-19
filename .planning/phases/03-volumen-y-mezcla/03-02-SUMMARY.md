---
phase: 03-volumen-y-mezcla
plan: 02
subsystem: gui
tags: [iced, widgets, checkbox, stepper, customslider, input-validation]

# Dependency graph
requires:
  - phase: 03-volumen-y-mezcla
    plan: 01
    provides: AudioState fields with correct factory defaults for volume/mix features
provides:
  - StandardCheckbox custom widget: 12×12 px with ON/OFF visual states, focus-gating callback
  - NumberStepper custom widget: 64×14 px with left/right chevrons, manual input editing, StepperUnit enum
  - CustomSlider Fix B2: suffix stripping before parse, character filter, Delete key support
  - strip_suffix helper function for numeric suffix removal ("dB", "ms", "Hz", "%")
affects:
  - 03-04 (gui integration — uses these widgets for Volumen y Mezcla tab)
  - 03-06 (any phase that needs custom input widgets)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "iced::advanced::Widget trait implementation for custom-drawn checkboxes and steppers"
    - "Unicode glyph rendering (◀ U+25C0, ▶ U+25B6) via fill_text for chevron icons"
    - "Character filter pattern: digits, '.', '-', whitespace only via is_ascii_digit() guard"
    - "Suffix stripping before f64 parse for unit-aware numeric input"
    - "GLOBAL_SLIDER_SELECTED atomic flag for exclusive edit focus across widgets"
    - "Focus gating via on_selected_state_change callback → AppFocus::AudioCenter (D-35)"

key-files:
  created: []
  modified:
    - src/gui/widgets.rs (StandardCheckbox + NumberStepper widgets, strip_suffix helper, CustomSlider Fix B2)

key-decisions:
  - "Unicode chevrons (◀/▶) chosen over SVG assets for NumberStepper — simpler rendering in custom widgets via fill_text"
  - "NumberStepper manual input NEVER rounds to step — raw parsed value clamped directly to range (D-34)"
  - "CustomSlider still snaps to configured step on Enter/click-outside — only suffix stripping was added, existing behavior preserved"
  - "strip_suffix added as module-level private function, shared by NumberStepper and CustomSlider"
  - "All three parse locations in CustomSlider (Enter + two click-outside paths) updated to use strip_suffix"

patterns-established:
  - "CustomCheckbox pattern: iced::advanced::Widget with fill_quad for bg/border, fill_text for check mark"
  - "NumberStepper pattern: three-zone layout (chevron/value/chevron) with hover detection per zone"
  - "Input filter pattern: char-level guard in keyboard handler before String::insert"

requirements-completed: []

# Metrics
duration: 8min
completed: 2026-07-19
---

# Phase 03 Plan 02: StandardCheckbox 12×12 px, NumberStepper 64×14 px widgets, and CustomSlider Fix B2

**StandardCheckbox and NumberStepper custom widgets with iced::advanced::Widget trait, plus B2 fix: suffix stripping, character filter, and Delete key support in CustomSlider**

## Performance

- **Duration:** 8 min
- **Started:** 2026-07-19T03:22:14Z
- **Completed:** 2026-07-19T03:30:35Z
- **Tasks:** 3
- **Files modified:** 1

## Accomplishments

- Implemented `StandardCheckbox` custom widget: 12×12 px with OFF (COLOR_BG + border) and ON (COLOR_ACCENT + "✓") visual states, mouse click toggling, and focus-gating via `on_selected_state_change` callback (D-31, D-35)
- Implemented `NumberStepper` custom widget: 64×14 px fixed layout with left/right Unicode chevrons (◀/▶), centered value display with suffix ("dB"/"ms"), step buttons, and manual input editing (D-32, D-33, D-34, D-35)
- Implemented `StepperUnit` enum: Decibels (step 0.25) and Milliseconds (step 50)
- NumberStepper keyboard input: character filter (digits, '.', '-', whitespace only, max 20 chars), Enter/click-outside to apply (NO rounding to step — D-34), Escape to cancel, ArrowUp/ArrowDown to step, Backspace/Delete for editing
- Added `strip_suffix` helper function stripping "dB", "ms", "Hz", "%" suffixes before numeric parse
- Fixed CustomSlider B2: suffix stripping applied to all three parse locations (Enter key, both click-outside paths), character filter added, Delete key support added (D-45)
- All 146 existing tests pass; cargo check clean; widgets.rs cargo fmt applied

## Task Commits

Each task was committed atomically:

1. **Task 1: StandardCheckbox custom widget** — `34beb11` (feat)
2. **Task 2: NumberStepper custom widget** — `617c641` (feat)
3. **Task 3: Fix B2 — CustomSlider suffix stripping + char filter + Delete key** — `f1b40ee` (fix)

**Plan metadata:** (pending)

## Files Created/Modified

- `src/gui/widgets.rs` — Added: `strip_suffix` helper (line 33), `StandardCheckbox` widget (~180 lines), `StepperUnit` enum + `NumberStepper` widget (~460 lines), CustomSlider B2 fixes (~60 lines changed). Total: ~640 new lines in single file.

## Decisions Made

- **Unicode chevrons for NumberStepper:** Used `◀` (U+25C0) and `▶` (U+25B6) Unicode glyphs rendered via `fill_text` instead of SVG assets. This avoids async texture loading in custom widgets and matches the iced 0.14 advanced renderer API cleanly.
- **strip_suffix placed early in file:** Added as module-level private function after `GLOBAL_SLIDER_SELECTED`, before both StandardCheckbox and CustomSlider. Both widgets now share it (NumberStepper uses it for f64 parse, CustomSlider for f32 parse).
- **CustomSlider snapping preserved:** The existing step-snapping logic (lines 3782-3784 in new numbering) was not modified — only the suffix strip was added before parse. Manual input still snaps to configured step in CustomSlider.

## Deviations from Plan

### Pre-existing fmt/clippy issues

**cargo fmt --check** reports trailing whitespace errors in `src/db/database.rs` and `src/gui/app.rs`, and **cargo clippy -- -D warnings** reports ~352 errors — all pre-existing in files not modified by this plan. Widgets.rs itself has zero clippy errors and zero fmt issues after this plan's changes.

New code was formatted with `rustfmt src/gui/widgets.rs` directly and passes. Clippy issues in the new code (unnecessary map_or, unused variable, collapsible if) were fixed proactively during implementation.

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed clippy::unnecessary-map-or in StandardCheckbox and NumberStepper**
- **Found during:** Task 3 (cargo clippy verification)
- **Issue:** Four `map_or(false, |p| bounds.contains(p))` calls and three nested `if let Some` blocks triggered clippy warnings
- **Fix:** Replaced with `is_some_and()` pattern and `let Some(cursor_pos) = cursor.position() else { return; }` early-return pattern (matching CustomSlider's established style)
- **Files modified:** src/gui/widgets.rs (StandardCheckbox update/draw, NumberStepper update)
- **Verification:** `cargo clippy -- -D warnings` shows zero errors in widgets.rs
- **Committed in:** f1b40ee

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Style improvement — no behavior change. Aligns with existing CustomSlider code patterns.

## Issues Encountered

None.

## Known Stubs

None — both widgets are fully functional with concrete implementations. No placeholder values, TODO comments, or unimplemented callbacks.

## Threat Flags

None — no new network endpoints, auth paths, or file access patterns introduced. All input goes through character filter + parse + clamp chain as documented in the threat model (T-03-04, T-03-05).

## Next Phase Readiness

- Both custom widgets compile and are ready for use by Plan 03-04 (GUI integration for Volumen y Mezcla tab)
- `strip_suffix` helper is available for any future widget needing suffix-stripped numeric parsing
- CustomSlider B2 fix enables keyboard input on sliders with unit display values
- Ready for Plan 03-03 (decoder — audio processing engine changes)

---
*Phase: 03-volumen-y-mezcla*
*Completed: 2026-07-19*
