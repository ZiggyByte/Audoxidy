# Requirements: Audoxidy VU Meter

**Defined:** 2026-09-08
**Core Value:** Deliver studio-quality audio playback with advanced DSP in a responsive, memory-efficient native desktop application.

## v1 Requirements

### Meter Core

- [ ] **METER-01**: Stereo L/R bar display showing both channels simultaneously
- [ ] **METER-02**: Peak level bar tracking instantaneous signal maximum per frame
- [ ] **METER-03**: RMS level bar with 300ms integration time (IEC 60268-17 VU standard)
- [ ] **METER-04**: Peak hold marker with decay (~1.5-2 dB/sec) after 1-3 second hold
- [ ] **METER-05**: Smooth bar animation at 60fps (5ms attack, 200-600ms release)

### Scale & Zones

- [ ] **METER-06**: dBFS scale from -60 to +6 dB with ticks at -60, -40, -20, -15, -12, -9, -6, -3, 0, +3, +6
- [ ] **METER-07**: Green zone (-∞ to -9 dBFS), Yellow zone (-9 to -3 dBFS), Orange zone (-3 to 0 dBFS), Red zone (0 to +6 dBFS clipping)

### Indicators

- [ ] **METER-08**: Clipping indicator (LED-style) when peak exceeds 0 dBFS

### Data Layer

- [ ] **METER-09**: Lock-free atomic data transfer from decoder thread to GUI (no RwLock on hot path)
- [ ] **METER-10**: Peak/RMS computation in decoder thread (post-DSP, pre-volume)
- [ ] **METER-11**: 30ms GUI subscription for smooth meter updates

### Configuration

- [ ] **METER-12**: Configurable peak hold time (0.5s-5s, default 1.5s)
- [ ] **METER-13**: Configurable RMS integration time (50ms-1000ms, default 300ms)

### Advanced Modes

- [ ] **METER-14**: Numeric dBFS readout (peak + RMS values as text)
- [ ] **METER-15**: Mid/Side metering mode (toggle between L/R and M/S display)
- [ ] **METER-16**: Infinite hold mode (peak stays at max until manually reset)
- [ ] **METER-17**: Phase correlation indicator (-1 to +1)
- [ ] **METER-18**: Crest factor display (peak - RMS in dB)

### Integration

- [ ] **METER-19**: Meter positioned in bottom library bar between audio stats and SVG icon buttons
- [ ] **METER-20**: Theme-consistent rendering (dark theme colors from theme.rs)
- [ ] **METER-21**: Zero allocations on decode thread hot path

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Spectrum Analyzer (Future Milestone)

- **SPEC-01**: Full spectrum analyzer panel with configurable FFT size
- **SPEC-02**: Multiple visualization modes (bars, waterfall, spectrogram)
- **SPEC-03**: Configurable frequency range and smoothing
- **SPEC-04**: Detachable/floating window option

## Out of Scope

| Feature | Reason |
|---------|--------|
| Full spectrum analyzer | Explicitly deferred to future milestone per user plan |
| LUFS/K-weighted measurement | Broadcast standard, not needed for music playback monitoring |
| True peak (inter-sample) | Requires 4x oversampling; sample peak adequate; limiter already handles ISP |
| Surround sound metering (5.1/7.1) | Audoxidy is stereo-only |
| Animated needle/VU display | Aesthetic, not functional; bar graph is the spec |
| DAW sync features | Player, not DAW |
| Plugin architecture | Hardcoded widget; future extensibility via configuration |
| Multiple meter themes/skins | One clean design matching dark theme; skins deferred |
| Numeric peak-to-peak voltage | Analog-only; dBFS is the correct digital unit |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| METER-01 | Phase 3 | Pending |
| METER-02 | Phase 3 | Pending |
| METER-03 | Phase 3 | Pending |
| METER-04 | Phase 2 | Pending |
| METER-05 | Phase 2 | Pending |
| METER-06 | Phase 3 | Pending |
| METER-07 | Phase 3 | Pending |
| METER-08 | Phase 3 | Pending |
| METER-09 | Phase 1 | Pending |
| METER-10 | Phase 1 | Pending |
| METER-11 | Phase 2 | Pending |
| METER-12 | Phase 4 | Pending |
| METER-13 | Phase 4 | Pending |
| METER-14 | Phase 4 | Pending |
| METER-15 | Phase 4 | Pending |
| METER-16 | Phase 4 | Pending |
| METER-17 | Phase 4 | Pending |
| METER-18 | Phase 4 | Pending |
| METER-19 | Phase 3 | Pending |
| METER-20 | Phase 3 | Pending |
| METER-21 | Phase 1 | Pending |

**Coverage:**
- v1 requirements: 21 total
- Mapped to phases: 21 ✓
- Unmapped: 0 ✓

---
*Requirements defined: 2026-09-08*
*Last updated: 2026-09-08 after roadmap creation*
