# Roadmap: Audoxidy VU Meter

## Overview

This roadmap adds a stereo VU meter to Audoxidy across 4 phases: a lock-free data layer that taps the post-DSP signal, a 30ms GUI subscription with animation state management, a custom iced widget rendering L/R bars with color zones, and polish features (configurable settings, M/S mode, phase correlation, numeric readout). Each phase builds on the previous, delivering a complete, testable capability. All 21 requirements are mapped with zero orphans.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Meter Data Layer** - Lock-free atomic data transfer from decoder thread to GUI
- [ ] **Phase 2: GUI Subscription & State** - 30ms meter tick, peak hold decay, pause/stop fade
- [ ] **Phase 3: Meter Widget Rendering** - Custom iced widget with L/R bars, dBFS scale, color zones, clipping
- [ ] **Phase 4: Polish & Configuration** - Settings, numeric readout, M/S mode, phase correlation

## Phase Details

### Phase 1: Meter Data Layer
**Goal**: Decoder thread computes peak/RMS values and writes them to lock-free atomics with zero overhead on the hot path.
**Depends on**: Nothing (first phase)
**Requirements**: METER-09, METER-10, METER-21
**Success Criteria** (what must be TRUE):
  1. When audio is playing, AtomicU32 variables contain updated peak and RMS values for both L/R channels
  2. The decoder thread computes peak/RMS after DSP processing and after crossfade mix, before ringbuf push
  3. No allocations occur on the decoder thread hot path during meter computation
**Plans**: 1 plan
Plans:
- [ ] 05-01-PLAN.md — MeterData struct + decoder loop integration (peak/RMS atomics)

### Phase 2: GUI Subscription & State
**Goal**: The GUI reads meter data at 30ms intervals and manages animation state (peak hold decay, smooth ballistics) with proper pause/stop behavior.
**Depends on**: Phase 1
**Requirements**: METER-04, METER-05, METER-11
**Success Criteria** (what must be TRUE):
  1. Meter values update smoothly at ~33Hz during playback (not jumping in discrete steps)
  2. Peak hold marker stays at the maximum level for the configured hold time, then decays
  3. Bar animation uses smooth attack (~5ms) and release (~200-600ms) ballistics
  4. Pausing playback causes meter bars to smoothly fade to silence
  5. Changing tracks resets meter state (no stale values from previous track)
**Plans**: 1 plan
Plans:
- [ ] 06-01-PLAN.md — MeterTick subscription + MeterUiState (peak hold, ballistics, pause fade, track reset)

**UI hint**: yes

### Phase 3: Meter Widget Rendering
**Goal**: A custom iced widget renders stereo L/R bars with dBFS scale, color zones, and clipping indicator in the bottom library bar.
**Depends on**: Phase 2
**Requirements**: METER-01, METER-02, METER-03, METER-06, METER-07, METER-08, METER-19, METER-20
**Success Criteria** (what must be TRUE):
  1. Two horizontal bars (L/R) are visible in the bottom bar between audio stats and icon buttons
  2. dBFS scale shows ticks from -60 to +6 with labeled major divisions
  3. Bar color changes through green → yellow → orange → red zones as level increases
  4. Clipping LED lights up when peak exceeds 0 dBFS
  5. Peak hold marker is visible as a thin line at the held peak position
**Plans**: 1 plan
Plans:
- [ ] 07-01-PLAN.md — VuMeterWidget (iced::advanced::Widget) + bottom bar integration
**UI hint**: yes

### Phase 4: Polish & Configuration
**Goal**: User can configure meter behavior and access advanced modes (M/S, phase correlation, numeric readout).
**Depends on**: Phase 3
**Requirements**: METER-12, METER-13, METER-14, METER-15, METER-16, METER-17, METER-18
**Success Criteria** (what must be TRUE):
  1. User can adjust peak hold time and RMS integration time in settings, with changes taking effect immediately
  2. Numeric dBFS readout shows precise peak and RMS values for both channels
  3. M/S mode toggle switches display from L/R to Mid/Side metering
  4. Phase correlation indicator shows stereo width from -1 (mono) to +1 (wide)
  5. Infinite hold mode keeps peak at maximum until manually reset
**Plans**: 2 plans
Plans:
- [ ] 08-01-PLAN.md — Settings infrastructure (popup widget, APP_SETTINGS persistence, hold time, RMS window, infinite hold)
- [ ] 08-02-PLAN.md — Advanced display (numeric readout, M/S mode, phase correlation, crest factor)
**UI hint**: yes

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Meter Data Layer | 0/1 | Not started | - |
| 2. GUI Subscription & State | 0/1 | Not started | - |
| 3. Meter Widget Rendering | 0/1 | Not started | - |
| 4. Polish & Configuration | 0/2 | Not started | - |
