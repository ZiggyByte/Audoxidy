---
phase: 03
slug: volumen-y-mezcla
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-18
---

# Phase 03 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` |
| **Config file** | none — tests inline `#[cfg(test)]` or `*_tests.rs` files |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~5 s (108 tests currently) |

---

## Sampling Rate

- **After every task commit:** `cargo check && cargo test`
- **After every plan wave:** `cargo test` (full suite)
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** ~60 s

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 03-W0-01 | 00 | 0 | D-01 | — | N/A | unit | `cargo test audio::test_loudness_gain` | ❌ W0 | ⬜ pending |
| 03-W0-02 | 00 | 0 | D-03 | — | N/A | integration | `cargo test audio::test_chain_order` | ❌ W0 | ⬜ pending |
| 03-W0-03 | 00 | 0 | D-08 | — | N/A | unit | `cargo test audio::test_volume_smoothing` | ❌ W0 | ⬜ pending |
| 03-W0-04 | 00 | 0 | D-14/15 | — | N/A | unit | `cargo test audio::test_silence_detection` | ❌ W0 | ⬜ pending |
| 03-W0-05 | 00 | 0 | D-17 | — | N/A | unit | `cargo test audio::test_silence_hysteresis` | ❌ W0 | ⬜ pending |
| 03-W0-06 | 00 | 0 | D-12/13 | — | N/A | unit | `cargo test audio::test_equal_power_fade` | ❌ W0 | ⬜ pending |
| 03-W0-07 | 00 | 0 | D-21/22 | — | N/A | integration | `cargo test audio::test_rms_normalization` | ❌ W0 | ⬜ pending |
| 03-W0-08 | 00 | 0 | D-24 | — | N/A | unit | `cargo test audio::test_normalization_smoothing` | ❌ W0 | ⬜ pending |
| 03-W0-09 | 00 | 0 | B1 fix | — | N/A | stress | `cargo test audio::test_no_dsp_dropout` | ❌ W0 | ⬜ pending |
| 03-W0-10 | 00 | 0 | B2 fix | — | Input filter: only digits, `.`, `-`; suffix stripped before f32::parse | unit | `cargo test widgets::test_suffix_strip_before_parse` | ❌ W0 | ⬜ pending |
| 03-W0-11 | 00 | 0 | D-34 | — | Input clamped to range | unit | `cargo test widgets::test_stepper_no_rounding` | ❌ W0 | ⬜ pending |
| 03-W0-12 | 00 | 0 | B3 fix | — | N/A | integration | `cargo test app::test_volume_persistence` | ❌ W0 | ⬜ pending |
| 03-W0-13 | 00 | 0 | global | — | All f64 ops produce finite values | unit | `cargo test audio::test_nan_guard` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/audio/tests.rs` — add `test_loudness_gain`, `test_chain_order`, `test_volume_smoothing`, `test_silence_detection`, `test_silence_hysteresis`, `test_equal_power_fade`, `test_rms_normalization`, `test_normalization_smoothing`, `test_no_dsp_dropout`, `test_nan_guard`
- [ ] `src/gui/widgets_tests.rs` (**NEW**) — `test_suffix_strip_before_parse`, `test_stepper_no_rounding`
- [ ] `src/audio/integration_tests.rs` — add `test_volume_persistence` (or in `src/gui/app.rs` inline `#[cfg(test)]`)

Existing test infrastructure:
- `src/audio/tests.rs` (22 tests)
- `src/audio/dsp_tests.rs` (46 tests)
- `src/audio/integration_tests.rs` (MockDecoder)
- `src/utils/mod.rs` inline `#[cfg(test)]` (38 tests)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| StandardCheckbox rendering (ON/OFF states) | D-31 | Custom-drawn iced widget — no pixel-level snapshot testing | Open Audio Center tab 3, toggle any checkbox, verify ✓ visible on ON, invisible on OFF, colors match specs |
| NumberStepper rendering + interaction | D-32/D-34 | Custom-drawn widget with click-to-edit + key handling | Open tab 3, click stepper input, type value, press Enter — confirm value applies and widget reflects it |
| Tab navigation + disabled states | D-40 | Visual state machine (greyed sub-functions when master off) | Toggle master checkboxes off, verify sub-rows grayed out and non-interactive |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60 s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
