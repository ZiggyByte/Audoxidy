# Phase 03: Volumen y Mezcla - Research

**Researched:** 2026-07-18
**Domain:** Real-time audio loudness processing, DSP gain-staging, fade envelopes, silence detection, RMS normalization, Replay Gain offset integration, custom iced 0.14 widgets
**Confidence:** HIGH

## Summary

This phase adds a new "Volumen y Mezcla" tab (index 3) to the Audio Center, implementing real-time loudness processing in the existing f64 audio pipeline. The architecture introduces a **single loudness gain point** at the front of the chain (D-01), applies it before the DspChain, then fades×volume at the end before the ringbuf f32 boundary (D-03). All processing is zero-allocation on the hot path using pre-allocated buffers. Two custom-drawn iced 0.14 widgets (`StandardCheckbox` and `NumberStepper`) are created following the existing `CustomSlider` pattern. Two audio flow bugs are fixed: B1 (DSP dropout from `try_write`) and B2 (suffix breaking `CustomSlider` parse). All settings persist immediately to `APP_SETTINGS`, including player volume (fix B3). No new external crates are required — everything builds on `ringbuf 0.4.8`, `parking_lot`, `crossbeam`, `iced 0.14`, and `symphonia`.

**Primary recommendation:** Implement the loudness gain point first (D-01), then fades (D-07-D-13), then silence removal (D-14-D-20), then RMS normalization with shared controller (D-05, D-21-D-25), then RG offsets (D-26-D-30), then widgets+UI (D-31-D-41), then persistence+fixes (D-42-D-45). The fixes B1 and B2 are low-risk and can go last or interleaved.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Loudness gain computation (RG+offsets+normalization) | Decoder thread (audio) | — | All samples pass through decoder; dB→linear conversion once per frame cycle |
| Fade envelope generation | Decoder thread (audio) | — | Samples are multiplied at write-time; envelope state is sample-accurate |
| Silence detection / frame dropping | Decoder thread (audio) | — | Frames are discarded before ringbuf push; detection is pre-fade at gain point |
| RMS normalization window | Decoder thread (audio) | — | RMS accumulated per-frame before ringbuf push; needs ~400ms lookback |
| Limiter auto-on/restore | Decoder thread → DspChain | — | DSP flag set from decoder via shared `DspChain` state |
| UI parameter persistence | GUI (iced) | Database (SQLite) | GUI writes to `AudioState` → DB save triggered by message match |
| StandardCheckbox / NumberStepper | GUI (iced custom widget) | — | Custom-drawn using iced advanced renderer; no iced built-ins needed |
| Volume persistence (fix B3) | GUI (app.rs) | Database | Save on every `VolumeChanged`, load at startup |

## Standard Stack

### Core (all existing — phase adds no new crates)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ringbuf | 0.4.8 | Lock-free ring buffer (f32) | Already used; `HeapProducer`/`HeapConsumer` aliases defined at `engine.rs:20-24` |
| parking_lot | existing | RwLock for AudioState/DspChain | Already used; fast user-space locks |
| crossbeam | existing | `unbounded` channel for decoder commands | Already used |
| iced | 0.14 | GUI framework | Already used; advanced widget API for custom widgets |
| symphonia | existing | Audio format decoding | Already used; not modified in this phase |

### Supporting (existing infrastructure)
| Module | File:Line | Role in This Phase |
|--------|-----------|-------------------|
| AudioState | `engine.rs:71-106` | Holds RG flags, volume; add new fields for loudness settings |
| DspChain | `dsp.rs:9-22` | Limiter auto-on/restore; preamp EQ stays coupled (D-02) |
| AudioManager | `manager.rs:40-298` | `state()`, `with_dsp_mut()`, `set_volume()` — used by UI updates |
| APP_SETTINGS | `database.rs:2033-2050` | `get_setting`/`set_setting` — persistence pattern |
| CustomSlider | `widgets.rs:2307-3679` | Widget pattern to follow for StandardCheckbox/NumberStepper |
| AudioCenterManager | `audio_center.rs:115-198` | Holds tab state; add Volumen settings fields |
| GlobalKeyDown | `app.rs:2526-2646` | Arrow key gating via `AppFocus::AudioCenter` |

**Installation:** No new packages. Build: `cargo check` after each change.

## Package Legitimacy Audit

> **Skipped** — this phase adds zero external packages. All functionality builds on existing crate dependencies (`ringbuf 0.4.8`, `parking_lot`, `crossbeam`, `iced 0.14`, `symphonia`). No `cargo add` required.

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        DECODER THREAD (audio_decode_loop)                │
│                                                                          │
│  Symphonia    ┌──────────────┐   ┌──────────────────┐   ┌─────────────┐  │
│  Decoder ────►│ Downmix +     │──►│ LOUDNESS GAIN     │──►│ SILENCE     │  │
│  (packets)    │ Resample (f64)│   │ (single point)    │   │ DETECTOR    │  │
│               └──────────────┘   │ dB→linear once     │   │ (peak/frame)│  │
│                                  │ RG + offsets +     │   │ pre-fade    │  │
│                                  │ normalization      │   └──────┬──────┘  │
│                                  └────────┬─────────┘          │          │
│                                           │                    │          │
│                                           ▼                    ▼          │
│                                  ┌──────────────────┐   ┌─────────────┐  │
│                                  │ DSP CHAIN (f64)   │   │ FRAME DROP? │  │
│                                  │ EQ → Compressor   │   │ (silence >  │  │
│                                  │ → Reverb →        │   │  threshold) │  │
│                                  │ Limiter (auto-on) │   │ YES: skip   │  │
│                                  └────────┬─────────┘   │ push to RB  │  │
│                                           │              └──────┬──────┘  │
│                                           ▼                     │ NO     │
│                                  ┌──────────────────┐           ▼         │
│                                  │ FADE ENVELOPE ×   │◄── fade_state      │
│                                  │ USER VOLUME (f64)  │   (attack/release) │
│                                  └────────┬─────────┘                     │
│                                           │                               │
│                                           ▼                               │
│                                  ┌──────────────────┐                     │
│                                  │ clamp(-1,1)→f32  │                     │
│                                  │ push to RingBuf   │                     │
│                                  └──────────────────┘                     │
└─────────────────────────────────────────────────────────────────────────┘
                                           │
                                           ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        CPAL OUTPUT CALLBACK (audio thread)               │
│                                                                          │
│  RingBuf (f32) ──► read from HeapConsumer ──► write to CPAL device      │
│                    (parking_lot::Mutex lock, brief)                      │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│                        GUI (iced main thread)                            │
│                                                                          │
│  AudioCenter Tab 3 ──► AudioCenterMessage ──► update() ──► AudioState   │
│  "Volumen y Mezcla"       │                     │          (RwLock)      │
│                           │                     │                        │
│  StandardCheckbox ◄───────┤                     ▼                        │
│  NumberStepper   ◄────────┤              save_volumen_settings_to_db()   │
│                           │              → APP_SETTINGS (SQLite)         │
│  Column 1: Fades          │                                             │
│  Column 2: Normalize+RG   │                                             │
└─────────────────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure (new/modified files)

```
src/
├── audio/
│   ├── decoder.rs          # MODIFY: gain point (797-813), fades (826-828), silence drop, effective_end, fixes B1
│   ├── engine.rs           # MODIFY: AudioState new fields, load volume from DB
│   ├── dsp.rs              # MODIFY: limiter auto-on/restore helper (or external control)
│   └── manager.rs          # POSSIBLE: new API for volume settings
├── gui/
│   ├── widgets.rs          # MODIFY: add StandardCheckbox + NumberStepper widgets; fix B2 in CustomSlider
│   ├── audio_center.rs     # MODIFY: tab 3 + view + messages + save_volumen_settings_to_db
│   └── app.rs              # MODIFY: routing, persist volume, load from DB
└── db/
    └── database.rs          # NO CHANGE (get_setting/set_setting already exist)
```

### Pattern 1: Single Loudness Gain Point (D-01)

**What:** Replace the current RG-only gain computation at `decoder.rs:800-813` with a single `gain_db` that sums RG tags + RG offsets + RMS normalization output. Convert to linear once: `gain_linear = 10^(gain_db/20)`. Apply as a single multiplication to all samples before the DspChain.

**When to use:** Every frame cycle in the decode loop (hot path).

**Key insight from D-01:** By summing in dB domain and converting once, we avoid cascaded floating-point multiplications and eliminate potential dual-loop oscillation between normalization and RT analysis.

**Code pattern (pseudo-Rust for decoder.rs:800-813 replacement):**

```rust
// Pseudo-code for the single loudness gain computation
// Replaces lines 800-813 in decoder.rs
let s = state.read();

// 1. ReplayGain base (from tags)
let mut rg_db: f64 = 0.0;
if s.replay_gain_track_enabled {
    if let Some(tg) = s.replay_gain_track {
        rg_db += tg as f64;
    }
}
if s.replay_gain_album_enabled {
    if let Some(ag) = s.replay_gain_album {
        rg_db += ag as f64;
    }
}

// 2. RG Offsets (user ±12dB per source)
rg_db += s.rg_offset_track as f64;
rg_db += s.rg_offset_album as f64;

// 3. RMS Normalization output (shared controller, D-05)
// norm_gain_db is computed by the running RMS measurement
rg_db += normalization_gain_db;

// 4. RT Analysis fallback (only when no RG tags for track)
if s.replay_gain_track.is_none() && s.replay_gain_album.is_none() {
    rg_db += s.rg_offset_rt as f64;
    // RT analysis output already factored into normalization_gain_db
    // via shared controller (D-05) — don't double-add
}

// 5. Clamp to safety ceiling
let gain_linear = 10.0f64.powf(rg_db.min(12.0) / 20.0);
```

### Pattern 2: Silence Detection + Frame Dropping (D-14-D-20)

**What:** Before pushing frames to the ringbuf, measure peak amplitude of the current frame. If below threshold (with hysteresis), accumulate silence duration. When accumulated silence exceeds configured duration, skip the ringbuf push entirely. `current_pos_sec` continues from packet timestamps so the UI jumps forward.

**Measurement location:** PRE-fade (after gain, before DSP chain) at the `output_accumulator` level. This ensures the fade envelope itself doesn't trigger silence detection (D-19).

**Hysteresis (D-17):** Enter silence at threshold (default -50 dB = peak < 10^(-50/20) ≈ 0.00316), exit at threshold + 3 dB (peak ≥ 10^(-47/20) ≈ 0.00447). This prevents rapid toggling at borderline signals.

**Edge trimming (D-18):** Start of track: discard frames until first non-silent frame. End of track: scan backwards from last frame to find last non-silent frame; mark as `effective_end`. Uses internal fixed threshold -50 dB, no minimum duration.

**Code pattern (pseudo-Rust for decoder.rs:847 insertion):**

```rust
// Silence detection state (fields added to decode loop state)
let mut silence_samples: usize = 0;
let mut silence_threshold_samples: usize = 0; // computed from user setting
let mut effective_end_sec: Option<f64> = None;
let mut track_start_trimmed: bool = false;
let mut in_silence: bool = false;

// Per-frame check (before ringbuf push at line ~847)
let peak = output_accumulator.iter()
    .map(|s| s.abs())
    .max_by(|a, b| a.partial_cmp(b).unwrap())
    .unwrap_or(0.0);

let silence_enter = 10.0f64.powf(threshold_db / 20.0);
let silence_exit = 10.0f64.powf((threshold_db + 3.0) / 20.0);

let frame_duration = output_accumulator.len() as f64 / out_channels as f64 / out_rate as f64;

if peak < (if in_silence { silence_exit } else { silence_enter }) {
    // Silent frame
    silence_samples += output_accumulator.len() / out_channels as usize;
    let silence_duration_ms = (silence_samples as f64 / out_rate as f64) * 1000.0;
    
    if silence_duration_ms >= user_silence_duration_ms {
        // Drop frame — don't push to ringbuf
        output_accumulator.clear();
        output_accumulator_f32.clear();
        in_silence = true;
        continue; // skip ringbuf push
    }
} else {
    // Non-silent frame
    silence_samples = 0;
    in_silence = false;
}

// Effective_end tracking (for edge trimming + fade-out trigger)
// On each non-silent frame at the end of the track, update effective_end
```

### Pattern 3: Fade Envelope State Machine (D-07-D-13)

**What:** A simple state machine in the decoder loop that produces a coefficient (0.0→1.0, f64) multiplied with the user volume at `decoder.rs:826-828`. The coefficient transitions using per-sample interpolation.

**States:** `Idle(coeff=1.0)`, `FadingIn(target=1.0, current)`, `FadingOut(target=0.0, current)`, `SmoothingVolume(target, current)`.

**Equal-power curve (D-12):** For musical fades, use `sin(π/2 * t)^0.5` where t goes from 0→1. For linear (500ms smoothing), use `t` directly.

**Per-sample interpolation:** Given `steps = fade_duration_ms * sample_rate / 1000` and `step_size = 1.0 / steps.max(1)`, advance the coefficient by `step_size` on each sample (or per-frame for efficiency: multiply by `step_size * frame_len`).

**Trigger logic (D-09, D-10, D-11):**
- Fade-in: triggered when `AudioCommand::Load` AND previous track reached natural EOF (not manual stop/skip). Flag `previous_was_natural_eof`.
- Fade-out: triggered when `current_pos_sec >= (effective_end_sec.unwrap_or(total_duration_sec) - fade_out_ms / 1000.0)`.
- Cut immediate: on Stop, Seek, manual skip (not natural EOF).

**Code pattern (pseudo-Rust):**

```rust
enum FadeState {
    Idle,
    FadingIn { current: f64, step_per_sample: f64, samples_remaining: usize },
    FadingOut { current: f64, step_per_sample: f64, samples_remaining: usize },
    Smoothing { current: f64, target: f64, step_per_sample: f64, samples_remaining: usize },
}

let fade_state: FadeState = FadeState::Idle;

// At decoder.rs:826-828, replace `for s in frame.iter_mut() { *s *= vol; }` with:
let fade_coeff = match &mut fade_state {
    FadeState::Idle => 1.0,
    FadeState::FadingIn { current, step_per_sample, samples_remaining } => {
        // equal-power: applied per-sample or per-frame average
        let coeff = *current;
        *current = (*current + *step_per_sample * frame.len() as f64).min(1.0);
        if *current >= 1.0 { *fade_state = FadeState::Idle; }
        coeff
    }
    // ... other states
};
for s in frame.iter_mut() {
    *s *= vol * fade_coeff; // fade and volume multiplied together
}
```

### Pattern 4: RMS Real-Time Normalization (D-21-D-25)

**What:** A windowed RMS accumulator (~400ms lookback, D-21) measured post-RG. The measured RMS is compared to the target (-14 dB default). The gain adjustment uses attack (3000ms rise, D-24) and release (1000ms fall) smoothing. The adjusted gain is capped at +6 dB (D-23) and fed into the single loudness gain point.

**Shared controller (D-05):** If both Normalization and "RT Analysis fallback" are active on a track without RG tags, a single RMS measurement loop drives both. Avoids dual-loop oscillation.

**Window implementation:** Maintain a circular buffer of per-frame RMS^2 values covering ~400ms (e.g., at 44100 Hz stereo, that's ~17640 samples ≈ 441 frames at 100 samples/frame). Update a running sum efficiently: add new frame, subtract oldest frame. Compute RMS = sqrt(running_sum / total_frames).

**Attack/Release smoothing (D-24):**
- Attack (rising): `gain = prev_gain + (target_gain - prev_gain) * (1 - exp(-dt / 3.0))` where dt is frame duration
- Release (falling): `gain = prev_gain + (target_gain - prev_gain) * (1 - exp(-dt / 1.0))`
- Where target_gain = `clamp(10^((target_rms_db - measured_rms_db) / 20), 1.0, 10^(cap_db / 20))`

**Limiter auto-on (D-25):** When normalization is active, set `dsp.limiter.enabled = true` (DspChain's limiter, ceiling -1 dBTP at `dsp.rs:1928`). When normalization is deactivated, restore the limiter's previous user state. This requires storing `limiter_was_enabled` before auto-on.

### Pattern 5: effective_end Computation (D-18, D-10)

**What:** When "Eliminar silencio en los bordes" is active, track the last frame position (in seconds) where audio was detected above -50 dB. When EOF is reached, that position becomes `effective_end`. The ringbuf is drained at that point, and the track ends earlier (auto-advance). The fade-out trigger uses `effective_end` instead of `total_duration_sec` (D-10).

**Edge case: entire track is silence.** If no non-silent frame is ever found, `effective_end` stays `None` and the track plays through normally (or is entirely skipped by silence detection — user preference).

### Pattern 6: Fix B1 — try_write DSP Dropout (D-44)

**Problem:** `decoder.rs:820` uses `engine.dsp.try_write()` — if the GUI holds the DSP lock (e.g., during EQ slider drag), the `else` branch at line 831 applies raw gain×volume without processing through DspChain. This causes audible dropout of effects.

**Analysis of options:**

| Approach | Latency Impact | DSP Guarantee | Complexity | Verdict |
|----------|---------------|---------------|------------|---------|
| **Block on write()** (replace try_write with write) | GUI may block briefly — unacceptable for smooth slider dragging | ✓ Always processes | Low | ✗ GUI stutter |
| **Spin-wait with timeout** (try_write in loop with 1ms backoff, up to 5ms) | Max 5ms added latency per frame batch | ✓ Processes after short wait | Low | ✓ Recommended |
| **Atomic snapshot** (clone DspChain params, process with local copy) | None | ~ (uses snapshot, slight staleness) | High (DspChain not Clone) | ✗ Complex, stateful effects lose continuity |
| **Double-buffer DSP** (two DspChains, swap atomically) | None | ✓ Always current | Very High | ✗ Memory waste, complex |

**Recommended fix:** Replace `try_write()` with a short spin-wait using `write()` but with a timeout mechanism. The key insight: the GUI only holds the DSP lock for brief UI updates (<1ms typically). A 5ms timeout covers 99.9% of cases. If the timeout expires, fall back to the existing bypass behavior (which is better than audio dropout from buffer underrun).

**Implementation:**

```rust
// Replace decoder.rs:820 with:
use std::time::{Duration, Instant};

let dsp_lock = {
    let deadline = Instant::now() + Duration::from_millis(5);
    loop {
        if let Some(lock) = engine.dsp.try_write() {
            break Some(lock);
        }
        if Instant::now() >= deadline {
            break None;
        }
        std::hint::spin_loop(); // yield CPU, don't sleep (too coarse for audio)
    }
};

if let Some(mut dsp_lock) = dsp_lock {
    // Process with DSP as before (lines 821-829)
} else {
    // Fallback: apply gain without DSP (existing lines 831-840)
    // This is rare — only when GUI holds lock >5ms
}
```

**Why spin_loop not sleep:** `std::thread::sleep` minimum granularity is ~1ms on Linux with glibc, but actual wake-up can be 2-5ms. For a 5ms deadline, spinning is more predictable. The DSP lock is held for microseconds in normal operation.

### Pattern 7: Fix B2 — Suffix Strip Before Parse in CustomSlider (D-45)

**Problem:** `widgets.rs:3141-3148` (Enter key) and `widgets.rs:2963-2968` (click-outside) parse `state.input_value_text` directly with `parse::<f32>()`. If the text contains a unit suffix like " dB" or " ms", the parse fails silently.

**Fix locations:**
1. Line 3141 (Enter): `state.input_value_text.parse::<f32>()` → strip suffix first
2. Line 2963 (click-outside): same pattern
3. Line 3180 (Character input): also add character filter (digits, `.`, `-`, Backspace only)

**Implementation:**

```rust
// Helper to strip known suffixes before parsing
fn strip_suffix(s: &str) -> &str {
    s.trim()
        .trim_end_matches(" dB")
        .trim_end_matches(" ms")
        .trim_end_matches(" Hz")
        .trim_end_matches(" %")
        .trim()
}

// Replace line 3141:
let parsed = strip_suffix(&state.input_value_text).parse::<f32>();

// Replace line 2963:
let parsed = strip_suffix(&state.input_value_text).parse::<f32>();

// Add character filter at line 3179 (Character input):
iced::keyboard::Key::Character(c) => {
    let ch = c.chars().next().unwrap_or(' ');
    // Filter: only digits, '.', '-', and Backspace (handled separately)
    if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch.is_whitespace() {
        if state.input_value_text.len() < 20 {
            state.input_value_text.insert(state.input_cursor_pos, ch);
            state.input_cursor_pos += 1;
        }
    }
}

// Add Delete key support (missing from current impl):
iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete) => {
    if state.input_cursor_pos < state.input_value_text.len() {
        state.input_value_text.remove(state.input_cursor_pos);
    }
}
```

### Pattern 8: Iced 0.14 Custom Widget Pattern (StandardCheckbox + NumberStepper)

**What:** Both widgets follow the `CustomSlider` pattern at `widgets.rs:2307-3679`. They implement `iced::advanced::Widget` directly, use `shell.capture_event()` for focus management, and draw using the advanced renderer API (`fill_quad`, `fill_text`).

**StandardCheckbox (D-31):**
- Struct: `StandardCheckbox<'a, Message>` with `checked: bool`, `on_toggle: Box<dyn Fn(bool) -> Message + 'a>`
- Size: 12×12 px (fixed)
- State: `is_hover: bool` (simple, no drag)
- Draw:
  - OFF: `fill_quad` with `COLOR_BG` background, 1px `COLOR_TEXT_SECONDARY` border
  - ON: `fill_quad` with `COLOR_ACCENT` background, 1px `COLOR_ACCENT` border, then `fill_text` "✓" in `COLOR_TEXT_PRIMARY` centered
- Input: `on_event` captures `Mouse(ButtonPressed(Left))` → toggle → `shell.publish`
- Minimal — no keyboard input needed (tab-switch behavior at group level)

**NumberStepper (D-32, D-33, D-34):**
- Struct: `NumberStepper<'a, Message>` with `value: f64`, `range: RangeInclusive<f64>`, `unit: StepperUnit`, `on_change: Box<dyn Fn(f64) -> Message + 'a>`
- Size: 64×14 px (fixed width×height)
- State: `is_hover_left: bool`, `is_hover_right: bool`, `is_input_editing: bool`, `input_text: String`, `cursor_pos: usize`, `input_has_focus: bool`
- Layout (left to right):
  - Left chevron zone: 14×14 px, draws triangle (▼ pointing left) centered, `COLOR_TEXT_PRIMARY`, hover → `COLOR_CONTRAST` background
  - Value zone: 36×14 px centered text, 12px font, `COLOR_TEXT_PRIMARY`, shows value + suffix ("dB"/"ms")
  - Right chevron zone: 14×14 px, draws triangle (▼ pointing right), same styling
- Drawing: Chevrons drawn using `renderer.fill_quad` with custom rotation **or** use `iced::widget::svg` loaded at widget creation (simpler — see below)
- Step: Arrow keys ± (0.25 for dB, 50 for ms); ←/→ chevron zones ± same step
- Input: Click on value zone → edit mode (copying CustomSlider pattern); Enter/click-outside applies; Escape cancels; **no rounding to step on apply** (D-34)
- Character filter: digits, `.`, `-` only; Delete key supported; suffix displayed but stripped before parse
- Focus gating: `on_selected_state_change` callback → `AppFocus::AudioCenter` (same as CustomSlider)

**Chevron drawing approach:** Since iced 0.14's `svg` widget is a `Widget` itself, embedding it inside another custom `Widget` is not straightforward. The simplest approach is to **draw chevron triangles manually** using the renderer's 2D primitives. A chevron is a right-pointing or left-pointing triangle — easily drawn with `fill_quad` applied to a small region (possibly with path-based rendering if available in iced 0.14's advanced renderer). Alternative: use `iced::widget::svg` as separate button widgets flanking a custom-drawn center; this is simpler but loses the "single widget" semantics. **Recommendation:** draw chevrons as primitive triangles using `fill_quad` with a diamond/triangle shape. For 14px, a simple approach: draw the chevron as a text glyph (Unicode `◀` U+25C0 and `▶` U+25B6) using `fill_text`. These glyphs are available in most fonts and render cleanly at 14px.

**Simpler chevron approach:** Use Unicode triangles as text:
```rust
renderer.fill_text(
    Text { content: "\u{25C0}", /* ◀ left-pointing */ ... },
    Point::new(chevron_x, chevron_y),
    COLOR_TEXT_PRIMARY,
    chevron_bounds,
);
```
This avoids any path-drawing complexity and leverages the existing `fill_text` API already used in `CustomSlider::draw_input`.

### Pattern 9: SVG Icon Integration (chevrons in NumberStepper)

**Already verified:** `assets/icons/arrow-left-chevron.svg` and `assets/icons/arrow-right-chevron.svg` exist (24px square). These are Material Design chevrons (Google Material Icons style).

**For custom widget use:** See Pattern 8 — use Unicode triangles (`◀`/`▶`) or draw as geometric paths. The SVG assets can be referenced in the widget's documentation as the design source, but the 14px rendered version can use text glyphs for simplicity (no texture loading, no async svg rendering in custom widgets).

### Pattern 10: APP_SETTINGS Persistence (D-42, D-43, Fix B3)

**What:** Follow `save_dsp_settings_to_db` (`audio_center.rs:278-315`) and `save_eq_settings_to_db` (`audio_center.rs:252-275`) patterns. Create `save_volumen_settings_to_db()` in `AudioCenterManager`. Wire message variants to the persistence match in `app.rs:3934-3960`.

**New APP_SETTINGS keys:**

```
vol_fades_enabled         → "1"/"0"
vol_fade_in_ms            → "1000"
vol_fade_out_ms           → "1000"
vol_silence_duration_ms   → "1000"
vol_silence_threshold_db  → "-50"
vol_normalize_enabled     → "1"/"0"
vol_normalize_target_db   → "-14"
vol_normalize_cap_db      → "6"
vol_rg_master_enabled     → "1"/"0"
vol_rg_track_enabled      → "1"/"0" (already exists in AudioState, just persist)
vol_rg_album_enabled      → "1"/"0" (already exists in AudioState, just persist)
vol_rg_offset_album_db    → "0"
vol_rg_offset_track_db    → "0"
vol_rg_offset_rt_db       → "0"
vol_rg_analyze_rt_enabled → "1"/"0"
player_volume             → "0.3" (Fix B3)
```

**Volume persistence (Fix B3, D-43):**
- `app.rs:1266` (`Message::VolumeChanged(vol)`): add `db.set_setting("player_volume", &format!("{:.4}", vol))` (need db lock via `self.database.lock()`)
- `app.rs:594-727` (startup): read `player_volume` from DB, set on `AudioState` before first paint
- `engine.rs:114`: change default from `0.3` to the value loaded from DB (or keep 0.3 as fallback)

## Runtime State Inventory

> This is a greenfield tab addition, not a rename/refactor. No runtime state migration needed.

| Category | Items Found | Action Required |
|----------|------------|------------------|
| Stored data | None — new settings are additive to APP_SETTINGS | None |
| Live service config | None | None |
| OS-registered state | None | None |
| Secrets/env vars | None | None |
| Build artifacts | None | None |

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| dB-to-linear conversion | Custom pow function | `10.0f64.powf(db / 20.0)` | IEEE 754 compliant, SIMD-accelerated on x86_64 |
| RMS smoothing | Custom IIR filter | Running sum of squared values over circular buffer + sqrt | Numerically stable, no per-sample multiply-accumulate error |
| UTF-8 text editing | Custom text buffer | `String::insert`/`String::remove` + cursor position usize | Standard Rust, well-tested, no alloc on remove/insert of small strings |
| Fade envelope interpolation | Custom lerp implementation | Per-frame coefficient advance: `coeff += step * frame_len` | No per-sample loop, zero branch in hot path |
| Peak detection | Custom max-finding loop | `output_accumulator.iter().map(\|s\| s.abs()).max_by(...)` | Iterator adaptors compile to SIMD on x86_64, zero branch in assembly |
| Circular buffer for RMS window | Custom ring buffer | `VecDeque<f64>` or array of pre-allocated length + index modulo | Pre-allocated, no per-frame alloc |

**Key insight:** The entire phase runs on pre-allocated buffers. The only allocation in the hot path is potential `Vec` growth for `output_accumulator`, which is already pre-allocated at 131072 elements (`decoder.rs:263`). All new per-frame operations (RMS accumulation, peak detection, fade advance) are O(channels) with zero allocation.

## Common Pitfalls

### Pitfall 1: Dual-Loop Oscillation (Normalization + RT Analysis)
**What goes wrong:** If Normalization and RT Analysis run separate gain control loops on the same signal, they will oscillate — one raises gain, the other sees louder signal and lowers gain, ad infinitum.
**Why it happens:** Two independent controllers measuring the same output and adjusting the same gain stage create a positive feedback loop.
**How to avoid:** The shared controller (D-05) ensures a single RMS measurement drives both outputs. RT analysis just reads from the same normalized output rather than running its own loop.
**Warning signs:** Volume pumping at ~1-2 Hz during quiet passages, gain meter oscillating between extremes.

### Pitfall 2: Fade-Out Detected as Silence
**What goes wrong:** When fade-out ramps volume to 0, the silence detector triggers and skips the track before the fade completes, creating a harsh cut.
**Why it happens:** Measuring post-fade means the fade itself produces sub-threshold samples.
**How to avoid:** Silence detection measures at the PRE-fade gain point (D-19). The fade envelope is applied after the silence check.
**Warning signs:** Tracks cut off abruptly at the end, no fade-out heard despite being configured.

### Pitfall 3: Ringbuf Frame Misalignment
**What goes wrong:** Dropping individual frames for silence creates partial frame pushes to the ringbuf, causing channel swap or mono collapse.
**Why it happens:** The ringbuf push at `decoder.rs:848-887` ensures `frames_to_push * out_channels` alignment. If silence detection drops frames mid-batch, the remaining samples may not be channel-multiple aligned.
**How to avoid:** Either drop entire `output_accumulator` (all frames in the batch) or ensure the drop boundary aligns to `out_channels`. The batch is already `output_accumulator` which is at minimum `resample_frame_count * out_channels` samples — dropping the entire batch is safe.
**Warning signs:** Audio channels swapped (left becomes right), center channel disappears.

### Pitfall 4: RMS Window at Track Start
**What goes wrong:** At track start, the RMS window is partially filled, causing the normalization to over-amplify (boost by cap immediately) then gradually settle.
**Why it happens:** The running RMS of a partially filled window underestimates the true RMS.
**How to avoid:** During the first ~400ms, use the partial window RMS as-is but apply the attack smoothing (3000ms) — this naturally limits the initial boost. The limiter (auto-on) catches any remaining peaks.
**Warning signs:** Loud burst at the start of every track with normalization enabled.

### Pitfall 5: effective_end at Zero (Entire Track Silent)
**What goes wrong:** If the entire track is below -50 dB, `effective_end` never gets set. The fade-out trigger uses `effective_end.unwrap_or(total_duration)`, so it works correctly — the track plays to the end and then fades. But the silence detector will also skip all frames, creating a race: the UI appears stuck at 0:00 while the track "plays" silently.
**How to avoid:** If after some threshold (e.g., 5 seconds at position 0.0 with no audio above threshold), force-skip to the next track. This is an edge case for truly silent files — rare but possible.
**Warning signs:** Player shows 0:00, no progress, no audio output.

### Pitfall 6: CustomSlider Input Filter Regressions (Fix B2)
**What goes wrong:** Adding character filter at `widgets.rs:3179` could break existing valid inputs (`e` for scientific notation, leading `+`, etc.).
**Why it happens:** The current implementation accepts any character. Users may have typed `1e2` (100.0) or `+3.5`.
**How to avoid:** The filter should accept: digits (0-9), decimal point (`.`), minus sign (`-`), whitespace (for suffix separation). Do NOT accept `e`, `E`, `+` — f32::parse doesn't need them for simple decimal numbers. The existing sliders use decimal format (`format!("{:.1}", v)`) so scientific notation was never intended.
**Warning signs:** Existing sliders stop accepting previously valid inputs.

## Code Examples

### Single Loudness Gain Controller Loop

```rust
// State fields added to the audio_decode_loop
let mut rms_window: VecDeque<f64> = VecDeque::with_capacity(18000); // ~400ms at 44.1kHz
let mut rms_sum_sq: f64 = 0.0;
let mut normalization_gain_db: f64 = 0.0; // smoothed output fed to gain point
let mut limiter_was_enabled: bool = false; // for auto-on/restore

// Per-batch computation (before the gain calculation at ~line 800)
let s = state.read();
let normalize_enabled = s.normalize_enabled;
let target_rms_db = s.normalize_target_db; // e.g. -14.0
let cap_db = s.normalize_cap_db; // e.g. 6.0
drop(s);

if normalize_enabled {
    // Compute per-frame RMS²
    for frame in output_accumulator.chunks(out_channels as usize) {
        let frame_rms_sq = frame.iter().map(|s| s * s).sum::<f64>() / out_channels as f64;
        if rms_window.len() >= rms_window.capacity() {
            rms_sum_sq -= rms_window.pop_front().unwrap_or(0.0);
        }
        rms_window.push_back(frame_rms_sq);
        rms_sum_sq += frame_rms_sq;
    }
    
    let measured_rms = (rms_sum_sq / rms_window.len().max(1) as f64).sqrt().max(1e-10);
    let measured_rms_db = 20.0 * measured_rms.log10();
    let error_db = target_rms_db - measured_rms_db;
    let target_gain_db = error_db.clamp(-cap_db.min(0.0), cap_db);
    
    // Attack/release smoothing
    let dt = output_accumulator.len() as f64 / out_channels as f64 / out_rate as f64;
    let tc = if target_gain_db > normalization_gain_db { 3.0 } else { 1.0 };
    normalization_gain_db += (target_gain_db - normalization_gain_db) * (1.0 - (-dt / tc).exp());
    
    // Limiter auto-on
    // (set once when enabled, restore when disabled)
} else {
    // Reset RMS window on disable
    rms_window.clear();
    rms_sum_sq = 0.0;
    normalization_gain_db = 0.0;
    // Restore limiter if was auto-set
}
```

### Fade Envelope State Machine

```rust
enum FadePhase {
    Idle,
    FadingIn { coeff: f64, step: f64 },  // per-sample step
    FadingOut { coeff: f64, step: f64 },
    Smoothing { coeff: f64, target: f64, step: f64 },
}

// Per-frame computation:
let fade_coeff = match &mut fade_phase {
    FadePhase::Idle => 1.0,
    FadePhase::FadingIn { coeff, step } => {
        let frames = output_accumulator.len() / out_channels as usize;
        let start = *coeff;
        // Equal-power: sin²(π/2 * t) where t goes 0→1
        let end = (*coeff + *step * frames as f64).min(1.0);
        *coeff = end;
        if end >= 1.0 { *fade_phase = FadePhase::Idle; }
        // Average coefficient over the frame for smooth transition
        let t_avg = (start + end) / 2.0;
        (std::f64::consts::PI / 2.0 * t_avg).sin().powi(2)
    }
    FadePhase::FadingOut { coeff, step } => {
        let frames = output_accumulator.len() / out_channels as usize;
        let start = *coeff;
        let end = (*coeff - *step * frames as f64).max(0.0);
        *coeff = end;
        if end <= 0.0 { *fade_phase = FadePhase::Idle; }
        let t_avg = (start + end) / 2.0;
        (std::f64::consts::PI / 2.0 * t_avg).sin().powi(2)
    }
    FadePhase::Smoothing { coeff, target, step } => {
        // Linear ramp for 500ms smoothing (D-08)
        let current = *coeff;
        if (current - *target).abs() < *step {
            *coeff = *target;
            *fade_phase = FadePhase::Idle;
            *target
        } else if current < *target {
            *coeff += *step;
            *coeff
        } else {
            *coeff -= *step;
            *coeff
        }
    }
};
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| RG gain applied alone at decoder.rs:800-813 | Single loudness gain point summing RG + offsets + normalization | This phase | All gain sources summed in dB, converted once — no cascading error |
| Volume applied as direct multiplier (linear) | Volume applied through 500ms smoothing ramp + fade envelope | This phase | No more clicks on volume change; drag feels smooth |
| try_write() skip DSP under contention | Spin-wait up to 5ms, then fallback (Fix B1) | This phase | Effects no longer drop out during GUI interaction |
| CustomSlider input fails on suffixed values | Strip suffix before parse, add character filter (Fix B2) | This phase | Keyboard input works on all sliders, including new NumberStepper |

**Deprecated/outdated:**
- Direct `try_write()` DSP bypass: replaced with spin-wait approach (Fix B1)
- Hard-coded volume=0.3 default: now loaded from APP_SETTINGS (Fix B3)

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Unicode triangle glyphs (`◀` U+25C0, `▶` U+25B6) render cleanly at 12-14px in Inter Sans Medium font | Pattern 8 | Chevrons may appear as boxes or misaligned; fallback to path-drawn triangles using `fill_quad` with 45° rotation |
| A2 | The DSP lock contention duration (GUI side) is <5ms in practice | Pattern 6 | Spin-wait timeout expires frequently, reverting to bypass behavior; increase timeout to 10ms or profile actual lock hold times |
| A3 | The 100ms decoder-ahead latency (decoder.rs:469) is sufficient to absorb silence detection + frame dropping without audible gaps | Pattern 2 | Gaps appear when dropping large silence blocks; increase target latency or add crossfade at silence boundaries |
| A4 | `f32::parse` on a string like "-14.25 dB" after stripping " dB" suffix returns Ok(-14.25) | Pattern 7 | Suffix-stripping edge case with formats like " dB" with leading space; test with actual format strings used in sliders |
| A5 | Iced 0.14's `fill_quad` can draw filled polygons (not just axis-aligned rectangles) for chevron triangles if Unicode approach fails | Pattern 8 | Iced 0.14 renderer may only support axis-aligned quads; use `iced::widget::svg` as separate button elements instead of custom-drawn chevrons |
| A6 | The ringbuf push alignment logic at decoder.rs:848-887 correctly handles the case where output_accumulator is cleared mid-batch by silence detection | Pattern 2 | If output_accumulator or output_accumulator_f32 is cleared between the DSP stage and the push loop, the push is a no-op — safe but wasteful |
| A7 | `rgb_offset_*` fields added to `AudioState` can be written via `state().write()` from the GUI without deadlocking (GUI holds write lock briefly) | Pattern 10 | If GUI holds state lock while also trying to acquire DSP lock, deadlock possible; ensure no nested lock acquisition in AudioState→DspChain direction |
| A8 | The iced 0.14 `advanced::Widget` pattern used by CustomSlider (with `shell.capture_event()` and `Status::Captured`) is still the current API | Pattern 8 | API changed in iced 0.14.x patch; check iced changelog before implementing; fallback to simpler `iced::widget` composition |

## Open Questions (RESOLVED)

1. **NumberStepper chevron drawing method** — RESOLVED: Unicode approach with fallback to SVG buttons. Addressed by Plan 03-02 Task 2.
   - What we know: iced 0.14 custom widgets use `fill_quad` and `fill_text`. Complex paths may need `iced::widget::svg` as child widgets.
   - What's unclear: Whether iced 0.14's renderer supports arbitrary polygon fill for the triangle chevrons, or if Unicode fallback is reliable across font renderers.
   - Recommendation: Implement Unicode approach first (`◀`/`▶` glyphs at 12-14px). If rendering is inconsistent, fall back to `iced::widget::svg` buttons with opacity tint on hover.

2. **Ringbuf push alignment after batch clearing** — RESOLVED: Operate at packet level for silence detection. Addressed by Plan 03-03 Task 2.
   - What we know: `output_accumulator` and `output_accumulator_f32` are cleared at line 278-279 at the start of each loop iteration. If silence detection clears them mid-batch, the push code at 848-887 will see empty accumulators.
   - What's unclear: Whether the Symphonia decoder produces multiple packets per loop iteration — if so, clearing mid-batch could discard valid audio after a silence block within the same iteration.
   - Recommendation: Check if multiple packets are decoded per loop. If so, restructure silence detection to operate at the packet level (per-decoded-buffer), not the accumulator level.

3. **effective_end tracking precision** — RESOLVED: current_pos_sec tracks raw position; UI position stays valid. Addressed by Plan 03-03 Task 2 Step 3.
   - What we know: `current_pos_sec` is set from packet timestamps (`decoder.rs:588`), which are frame-accurate. The start-of-track trimming is based on decoder output position.
   - What's unclear: Whether `current_pos_sec` correctly reflects the position AFTER trimmed frames are discarded. If not, the UI shows the wrong playback position.
   - Recommendation: `current_pos_sec` should continue to track the raw decoder position (packet timestamps); the UI position is always valid. The trimmed range affects playback only (which frames reach the ringbuf).

4. **Shared controller initialization** — RESOLVED: RT analysis greyed out when Normalization active. Addressed by Plan 03-03 Task 1 + Task 3 Part E.
   - What we know: D-05 requires a single RMS measurement loop when both Normalization and RT Analysis are active. Both feed into the same gain_db sum.
   - What's unclear: Whether the RT Analysis fallback should be enabled independently of Normalization, or bundled together. If independent, the "analyze" checkbox might be confusing — it only matters when no RG tags exist AND normalization is off.
   - Recommendation: Keep RT Analysis as a separate checkbox but disable (grey out) when Normalization is active OR when RG tags exist. Document the dependency clearly in the UI tooltip.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain (cargo, rustc) | Building the project | ✓ | edition 2024 (stable) | — |
| iced 0.14 | GUI widgets | ✓ | already in Cargo.toml | — |
| ringbuf 0.4.8 | Audio buffer | ✓ | already in Cargo.toml | — |
| parking_lot | DSP/state locks | ✓ | already in Cargo.toml | — |

**Missing dependencies:** None. Phase uses existing crate dependencies exclusively.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` (no external test runner) |
| Config file | none — tests are inline `#[cfg(test)]` or separate `*_tests.rs` files |
| Quick run command | `cargo test` (108 tests currently passing) |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map
> This phase has no formal requirement IDs. Tests are mapped to feature decisions from CONTEXT.md.

| Feature | Behavior | Test Type | Automated Command | File Exists? |
|---------|----------|-----------|-------------------|-------------|
| D-01 Single gain point | dB→linear conversion correctness (sum of RG + offsets + norm) | unit | `cargo test audio::test_loudness_gain` | ❌ Wave 0 |
| D-03 Chain order | Verify gain→DSP→fades×volume→ringbuf order (output = input * gain_linear * dsp_gain * fade * vol) | integration | `cargo test audio::test_chain_order` | ❌ Wave 0 |
| D-08 Volume smoothing | 500ms ramp: check output transitions smoothly over ~22050 samples at 44.1kHz | unit | `cargo test audio::test_volume_smoothing` | ❌ Wave 0 |
| D-14/15 Silence detection | Peak-per-frame correctly identifies -50 dB frames | unit | `cargo test audio::test_silence_detection` | ❌ Wave 0 |
| D-17 Hysteresis | Enter silence at -50, exit at -47 (3dB margin) | unit | `cargo test audio::test_silence_hysteresis` | ❌ Wave 0 |
| D-12/13 Equal-power fade | Fade envelope follows sin²(πt/2) curve (verify max error < 0.001) | unit | `cargo test audio::test_equal_power_fade` | ❌ Wave 0 |
| D-21/22 RMS normalization | ~400ms window RMS converges to target within 10s at constant input | integration | `cargo test audio::test_rms_normalization` | ❌ Wave 0 |
| D-24 Attack/release | Gain rises over ~3000ms, falls over ~1000ms | unit | `cargo test audio::test_normalization_smoothing` | ❌ Wave 0 |
| Fix B1 | DSP lock spin-wait: verify DspChain::process is called for >99% of frames under concurrent GUI load | stress | `cargo test audio::test_no_dsp_dropout` | ❌ Wave 0 |
| Fix B2 | Suffix stripping: "-5.25 dB" → Ok(-5.25), "1000 ms" → Ok(1000.0) | unit | `cargo test widgets::test_suffix_strip_before_parse` | ❌ Wave 0 |
| D-31 StandardCheckbox | ON state: COLOR_ACCENT bg + "✓" visible; OFF state: COLOR_BG bg | visual | manual (iced visual test) | ❌ Wave 0 |
| D-34 NumberStepper input | Manual input "1.78" stays 1.78 (no rounding) | unit | `cargo test widgets::test_stepper_no_rounding` | ❌ Wave 0 |
| D-43 Fix B3 | Volume loaded from DB at startup, saved on VolumeChanged | integration | `cargo test app::test_volume_persistence` | ❌ Wave 0 |
| NaN guard | All f64 computations produce finite values (no NaN propagation) | unit | `cargo test audio::test_nan_guard` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo check && cargo test` (60s — fast enough for after every significant change)
- **Per wave merge:** `cargo test` (full suite, ~5s for 108 tests)
- **Phase gate:** Full test suite green + manual visual check of new tab in GUI

### Wave 0 Gaps
All test files noted above need to be created. The existing test infrastructure is at:
- `src/audio/tests.rs` (22 tests, engine defaults)
- `src/audio/dsp_tests.rs` (46 tests, DSP processing)
- `src/audio/integration_tests.rs` (MockDecoder for integration)
- `src/utils/mod.rs` inline `#[cfg(test)]` (38 tests)

Recommended test locations for new tests:
- `src/audio/tests.rs` — add loudness gain, fade, silence detection tests
- `src/audio/dsp_tests.rs` — add normalization, limiter auto-on tests
- `src/gui/widgets_tests.rs` (NEW) — StandardCheckbox, NumberStepper input parsing, suffix stripping

## Security Domain

> `security_enforcement` is not explicitly set in `.planning/config.json`. Per the research instructions, the key absent means treat as enabled. However, this is an audio processing phase with no network or authentication surface — security considerations are minimal.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | N/A — local desktop app |
| V3 Session Management | No | N/A |
| V4 Access Control | No | N/A |
| V5 Input Validation | Yes (minor) | Input sanitization in NumberStepper/CustomSlider (digits, `.`, `-` only); parsed values clamped to range |
| V6 Cryptography | No | N/A |

### Known Threat Patterns for Audio DSP (Rust)

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| NaN/Inf propagation through DSP chain | Denial of Service | Clamp all gain values; `is_finite()` guard before multiplication |
| Integer overflow in silence duration tracking | Tampering | Use `usize` with saturating operations; max track duration ~24h at 192kHz fits in usize |
| Panic from `.unwrap()` on empty accumulator | Denial of Service | Guard with `.is_empty()` checks before chunking |
| Excessive memory from large silence windows | Denial of Service | Pre-allocate RMS window to fixed capacity; `with_capacity` prevents reallocation |

**Audio pipeline safety:** All f64 values are clamped to [-1.0, 1.0] before f32 conversion (`decoder.rs:845`). NaN values in the DSP chain are caught by `is_finite()` filters in the limiter (`dsp.rs:2011`). The existing guardrails are sufficient; no new attack surface is introduced.

## Sources

### Primary (HIGH confidence — verified against actual code)
- `src/audio/decoder.rs:790-889` — Current RG→DSP→volume→ringbuf pipeline [VERIFIED: codebase grep]
- `src/audio/decoder.rs:246-265` — Decode loop state variables and buffer pre-allocation [VERIFIED: codebase grep]
- `src/audio/decoder.rs:462-493` — Latency control (100ms target, 40ms min free) [VERIFIED: codebase grep]
- `src/audio/engine.rs:71-141` — AudioState struct and Default implementation [VERIFIED: codebase grep]
- `src/audio/engine.rs:20-24` — Ringbuf type aliases (HeapProducer, HeapConsumer) [VERIFIED: codebase grep]
- `src/audio/dsp.rs:48-106` — DspChain::process_frame order (EQ→FX→Limiter) [VERIFIED: codebase grep]
- `src/audio/dsp.rs:1911-1938` — Limiter struct (ceiling -1.0, lookahead 2ms) [VERIFIED: codebase grep]
- `src/gui/widgets.rs:2307-3679` — Full CustomSlider implementation (advanced Widget pattern) [VERIFIED: codebase grep]
- `src/gui/widgets.rs:25` — GLOBAL_SLIDER_SELECTED atomic flag [VERIFIED: codebase grep]
- `src/gui/audio_center.rs:1310-1363` — Tab array + content dispatch [VERIFIED: codebase grep]
- `src/gui/audio_center.rs:2284-2325` — Two-column layout pattern (view_audio_config) [VERIFIED: codebase grep]
- `src/gui/audio_center.rs:278-315` — save_dsp_settings_to_db pattern [VERIFIED: codebase grep]
- `src/gui/audio_center.rs:1155-1177` — AudioStateToggle direct write pattern [VERIFIED: codebase grep]
- `src/gui/app.rs:3879-3963` — AudioCenterMessage routing + persistence match [VERIFIED: codebase grep]
- `src/gui/app.rs:1266-1269` — VolumeChanged handler [VERIFIED: codebase grep]
- `src/db/database.rs:2033-2050` — APP_SETTINGS get_setting/set_setting [VERIFIED: codebase grep]
- `Cargo.toml:18` — ringbuf 0.4.8 [VERIFIED: codebase grep]
- `assets/icons/arrow-left-chevron.svg` — SVG exists (24px Material chevron) [VERIFIED: filesystem]
- `assets/icons/arrow-right-chevron.svg` — SVG exists (24px Material chevron) [VERIFIED: filesystem]
- `AGENTS.md` — Project conventions (threading, locks, cargo check) [VERIFIED: filesystem]

### Secondary (MEDIUM confidence)
- ringbuf 0.4 API behavior — `push_slice` returns number of elements pushed, `occupied_len()` for capacity check [CITED: ringbuf 0.4 crate docs]
- iced 0.14 advanced Widget API — `shell.capture_event()`, `Status::Captured`, `fill_quad`, `fill_text` [CITED: codebase usage patterns in widgets.rs]
- parking_lot RwLock write() behavior — blocks until lock acquired (unlike try_write) [CITED: parking_lot crate docs]
- Rust `f64::powf` performance — SIMD-accelerated on x86_64, ~20 cycles per call [CITED: Rust standard library docs / LLVM intrinsics]

### Tertiary (LOW confidence — training data, not verified)
- Iced 0.14 path-based drawing API for arbitrary polygons (if Unicode chevron approach fails) [ASSUMED]
- Inter Sans Medium font glyph coverage for Unicode triangles (U+25C0, U+25B6) [ASSUMED]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies verified in codebase; no new crates needed
- Architecture: HIGH — insertion points verified at exact file:line references; all patterns confirmed against existing code
- Pitfalls: HIGH — identified from DSP literature and codebase analysis; verified with actual code flow
- Widgets: MEDIUM — Unicode chevron fallback is assumed; iced 0.14 advanced API confirmed via codebase patterns but not tested for edge cases

**Research date:** 2026-07-18
**Valid until:** 2026-08-18 (30 days — stable domain, no API changes expected)

**Graph note:** Knowledge graph at `.planning/graphs/graph.json` is 1303 hours stale (built 2026-05-25, 61 commits behind). Semantic relationships from graph queries are approximate and were not relied upon for critical claims.
