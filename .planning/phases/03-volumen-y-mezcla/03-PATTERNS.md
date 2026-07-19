# Phase 03: Volumen y Mezcla - Pattern Map

**Mapped:** 2026-07-18
**Files analyzed:** 10 (9 modified, 1 new)
**Analogs found:** 10 / 10

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/audio/decoder.rs` | audio processing hot path | streaming/transform | `src/audio/decoder.rs:790-897` (existing decode loop) | exact (same file) |
| `src/audio/engine.rs` | state management/model | CRUD (state r/w) | `src/audio/engine.rs:70-141` (existing AudioState + Default) | exact (same file) |
| `src/audio/dsp.rs` | DSP processing | transform | `src/audio/dsp.rs:48-106` (process_frame) + `1911-1938` (Limiter struct) | exact (same file) |
| `src/audio/manager.rs` | service/API facade | request-response | `src/audio/manager.rs:40-119,270-298` (existing public API) | exact (same file) |
| `src/gui/widgets.rs` | custom widget/component | event-driven (UI input) | `src/gui/widgets.rs:2307-3679` (CustomSlider Widget impl) | role-match |
| `src/gui/app.rs` | orchestrator/controller | event-driven/message-routing | `src/gui/app.rs:3879-3963` (AudioCenterMsg routing) + `2526-2646` (GlobalKeyDown) | exact (same file) |
| `src/gui/audio_center.rs` | manager/component | event-driven + persistence | `src/gui/audio_center.rs:278-315` (save_dsp_settings) + `2293-2325` (view_audio_config) | exact (same file) |
| `src/gui/theme.rs` | config/constants | static config | `src/gui/theme.rs:1-40` (existing constants + custom_theme) | exact (same file) |
| `src/db/database.rs` | persistence/model | CRUD (SQLite) | `src/db/database.rs:2033-2050` (get_setting/set_setting) | exact (same file) |
| `src/gui/widgets_tests.rs` | test | unit test | `src/audio/dsp_tests.rs:1-55` (test structure) + `src/audio/tests.rs:42-56` (state default tests) | role-match |

---

## Pattern Assignments

### 1. `src/audio/decoder.rs` (audio processing hot path, streaming/transform)

**Analog:** `src/audio/decoder.rs:790-897` (existing decode loop — gain→DSP→volume→ringbuf)

#### Imports pattern (lines 244, 1-2):
```rust
// Lines 1-2
use super::AudioError;
use crate::audio::decoder::AudioDecoder;
// Line 244
use crate::audio::engine::{AudioEngine, AudioCommand, ChannelMap};
```
**Divergence:** No new imports needed. All new state uses existing types (f64, Vec, crossbeam Receiver).

#### State variables pattern (lines 247-264):
```rust
// Lines 246-264: Loop-local state pre-allocated before the loop
pub(crate) fn audio_decode_loop(command_rx: Receiver<AudioCommand>, engine: AudioEngine) {
    let mut current_format: Option<Box<dyn FormatReader>> = None;
    let mut current_decoder: Option<Box<dyn Decoder>> = None;
    let mut track_id = 0;

    let mut resampler: Option<Async<f64>> = None;
    let state = engine.state.clone();
    let producer_mutex = engine.buffer_producer.clone();
    let mut channel_map = ChannelMap::default();

    // Zero-Allocation Pool Buffers
    let mut output_accumulator: Vec<f64> = Vec::with_capacity(131072);
    let mut output_accumulator_f32: Vec<f32> = Vec::with_capacity(131072);
```
**Divergence:** Add new state variables AFTER line 264: `fade_state: FadeState`, `silence_samples: usize`, `effective_end_sec: Option<f64>`, `rms_window: VecDeque<f64>`, `normalization_gain_db: f64`, `limiter_was_enabled: bool`, `previous_was_natural_eof: bool`. All zero-alloc (pre-sized VecDeque, no allocation in hot path).

#### Gain computation pattern (lines 800-813):
```rust
// Lines 800-813: Current RG-only gain computation — REPLACE/EXPAND
let s = state.read();
let mut gain_db: f64 = 0.0;
if s.replay_gain_track_enabled {
    if let Some(tg) = s.replay_gain_track {
        gain_db += tg as f64;
    }
}
if s.replay_gain_album_enabled {
    if let Some(ag) = s.replay_gain_album {
        gain_db += ag as f64;
    }
}
let gain_linear = 10.0f64.powf(gain_db.min(12.0) / 20.0);
```
**Divergence:** Expand to single loudness gain point (D-01): sum RG tags + RG offsets + normalization_gain_db (shared controller D-05) + RT fallback (only when no RG tags). Drop `s` lock before per-frame loop.

#### DSP+volume+ringbuf pattern (lines 816-887):
```rust
// Lines 816-887: DSP processing → volume → clamp f32 → push ringbuf
{
    let _span = tracing::debug_span!("dsp_process", ...).entered();
    let out_ch = out_channels as usize;
    if let Some(mut dsp_lock) = engine.dsp.try_write() {
        for frame in output_accumulator.chunks_mut(out_ch) {
            for s in frame.iter_mut() { *s *= gain_linear; }
            dsp_lock.process_frame(frame);
            for s in frame.iter_mut() { *s *= vol; }
        }
    } else {
        // bypass...
    }
}
// f64→f32 clamp conversion (lines 844-846)
for &sample in output_accumulator.iter() {
    output_accumulator_f32.push(sample.clamp(-1.0, 1.0) as f32);
}
// Ringbuf push with frame alignment (lines 848-887)
let mut pos = 0;
while pos < output_accumulator_f32.len() { ... }
```
**Divergence:**
1. **Fix B1 (D-44):** Replace `try_write()` at line 820 with spin-wait loop (up to 5ms, `std::hint::spin_loop()`) before falling back to bypass. Insert between lines 819-820.
2. **New order (D-03):** Move `gain_linear` multiplication BEFORE DSP chain (gain → DspChain → fades×volume). The fade×volume multiplication replaces `*s *= vol` at line 827.
3. **Silence detection (D-14-D-20):** Insert per-frame peak measurement BEFORE the DSP block (after line 815). If silent > threshold, skip the DSP+push block entirely (continue the decode loop). Measured PRE-fade (D-19).
4. **effective_end (D-18):** Track last non-silent frame position in seconds. Set as `effective_end_sec` field, consumed by fade-out trigger and EOF early-termination.

#### Load/seek state purge pattern (lines 288-418, 419-439):
```rust
// Line 288-306: AudioCommand::Load — state reset
let mut s = state.write();
s.title = title;
s.artist = artist;
s.replay_gain_track = track_gain.map(|g| g as f32);
s.replay_gain_album = album_gain.map(|g| g as f32);

// Lines 361-377: MASTER STATE PURGE on Load
resampler = None;
resampler_rates = None;
output_accumulator.clear();
output_accumulator_f32.clear();
{
    if let Some(mut dsp_lock) = engine.dsp.try_write() {
        dsp_lock.reset_state();
    }
}

// Lines 419-439: AudioCommand::Seek — state reset
{
    let mut s = state.write();
    s.current_pos_sec = time;
}
output_accumulator.clear();
output_accumulator_f32.clear();
```
**Divergence:** After each Load/Seek/Stop purge block, also reset: `fade_state = FadeState::Idle`, `silence_samples = 0`, `in_silence = false`, `effective_end_sec = None`, `normalization_gain_db = 0.0`. On Load, set `previous_was_natural_eof` from the previous track's EOF type (D-09).

#### EOF natural pattern (lines 541-578):
```rust
// Lines 541-578: Natural EOF → is_playing=false, eof_reached=true
state.write().eof_reached = true;
state.write().is_playing = false;
```
**Divergence:** Before setting `eof_reached`, check `effective_end_sec` — if set, the track ends earlier. Signal `previous_was_natural_eof = true` for the next track's fade-in trigger (D-09).

---

### 2. `src/audio/engine.rs` (state management/model, CRUD)

**Analog:** `src/audio/engine.rs:70-141` (AudioState struct + Default)

#### AudioState struct pattern (lines 71-106):
```rust
// Lines 71-106: AudioState with all fields
#[derive(Clone)]
pub struct AudioState {
    pub is_playing: bool,
    pub volume: f32,
    pub sample_rate: u32,
    pub channels: u16,
    pub current_pos_sec: f64,
    pub total_duration_sec: f64,
    pub title: String,
    pub artist: String,
    pub path: String,
    pub eof_reached: bool,

    pub device_sample_rate: u32,
    pub bit_depth_display: String,
    pub buffer_size: u32,
    pub config_channels: ChannelConfig,

    pub downmix_center: f32,
    pub downmix_lfe: f32,
    pub downmix_surround: f32,
    pub downmix_center_enabled: bool,
    pub downmix_lfe_enabled: bool,
    pub downmix_surround_enabled: bool,

    pub replay_gain_track: Option<f32>,
    pub replay_gain_album: Option<f32>,
    pub replay_gain_track_enabled: bool,
    pub replay_gain_album_enabled: bool,
}
```
**Divergence:** Add new fields after line 105:
```rust
// Volumen y Mezcla — Fades
pub fades_enabled: bool,              // D-07 master
pub fade_in_ms: f32,                  // D-09 default 1000.0
pub fade_out_ms: f32,                 // D-10 default 1000.0
// Silence removal
pub silence_enabled: bool,            // D-14 master
pub silence_duration_ms: f32,         // D-16 default 1000.0
pub silence_threshold_db: f32,        // D-16 default -50.0
// Normalization
pub normalize_enabled: bool,          // D-21 master
pub normalize_target_db: f32,         // D-22 default -14.0
pub normalize_cap_db: f32,            // D-23 default 6.0
// ReplayGain offsets
pub rg_master_enabled: bool,          // D-30 master
pub rg_offset_album_db: f32,          // D-29 default 0.0
pub rg_offset_track_db: f32,          // D-29 default 0.0
pub rg_offset_rt_db: f32,             // D-29 default 0.0
pub rg_analyze_rt_enabled: bool,      // D-28 RT fallback
```

#### Default impl pattern (lines 110-141):
```rust
// Lines 110-141: Default for AudioState
impl Default for AudioState {
    fn default() -> Self {
        Self {
            is_playing: false,
            volume: 0.3,
            // ... all fields with defaults ...
            replay_gain_track_enabled: true,
            replay_gain_album_enabled: true,
        }
    }
}
```
**Divergence:** Add defaults for all new fields as listed in the approved defaults table (CONTEXT.md lines 80-101). `volume` stays at `0.3` as fallback — actual value loaded from DB (Fix B3, D-43).

#### set_volume pattern (lines 803-805):
```rust
// Lines 803-805: Existing volume setter
pub fn set_volume(&self, volume: f32) {
    self.state.write().volume = volume.clamp(0.0, 1.0);
}
```
**Divergence:** No change to `set_volume` — it's used as-is. Persistence is handled at the app.rs layer (Fix B3).

---

### 3. `src/audio/dsp.rs` (DSP processing, transform)

**Analog:** `src/audio/dsp.rs:48-106` (process_frame order) + `1911-1938` (Limiter struct)

#### DspChain::process_frame pattern (lines 48-106):
```rust
// Lines 48-106: Full DSP chain order
pub fn process_frame(&mut self, frame: &mut [f64]) {
    if !self.enabled { return; }
    // Preamp EQ (coupled to EQ enabled — D-02, DON'T TOUCH)
    if self.equalizer.enabled {
        for s in frame.iter_mut() { *s *= self.preamp_gain as f64; }
        self.equalizer.process_frame(frame);
    }
    // Noise Gate
    if self.noise_gate.enabled { self.noise_gate.process(frame); }
    // Sub Bass
    if self.sub_bass.enabled { self.sub_bass.process(frame); }
    // ... MidBass, VoiceBoost, Compressor, Reverb, StereoExpander, StereoBalance ...
    // Limiter (Always last, line 103-105)
    if self.limiter.enabled { self.limiter.process(frame); }
}
```
**Divergence:** Add a helper method on DspChain for limiter auto-on/restore:
```rust
// NEW: Limiter auto-on/restore (D-25)
impl DspChain {
    /// Force limiter on with tracking (for normalization)
    pub fn force_limiter_on(&mut self) -> bool {
        let was_enabled = self.limiter.enabled;
        self.limiter.enabled = true;
        was_enabled  // return previous state for restore
    }

    /// Restore limiter to its previous user state
    pub fn restore_limiter(&mut self, was_enabled: bool) {
        self.limiter.enabled = was_enabled;
    }
}
```

#### Limiter struct pattern (lines 1911-1938):
```rust
// Lines 1911-1938: Limiter default (ceiling -1.0, enabled false)
pub struct Limiter {
    pub enabled: bool,
    pub ceiling: f32,
    // ... internal fields ...
}
impl Default for Limiter {
    fn default() -> Self {
        Self {
            enabled: false,
            ceiling: -1.0,
            // ...
        }
    }
}
```
**Divergence:** No changes to Limiter struct. The existing `enabled` flag is what gets toggled by the auto-on/restore mechanism. The `ceiling` stays at -1.0 dBTP.

---

### 4. `src/audio/manager.rs` (service/API facade, request-response)

**Analog:** `src/audio/manager.rs:40-119` (existing public API) + `270-298` (DSP access)

#### Public API pattern (lines 41-81):
```rust
// Lines 41-44: State access
pub fn state(&self) -> Arc<RwLock<AudioState>> {
    self.engine.state.clone()
}

// Lines 47-49: play()
pub fn play(&self) {
    self.engine.set_playing(true);
}

// Lines 62-64: stop()
pub fn stop(&self) {
    self.engine.stop();
}

// Lines 71-75: toggle_play_pause()
pub fn toggle_play_pause(&self) {
    let playing = self.is_playing();
    self.engine.set_playing(!playing);
}
```

#### DSP access patterns (lines 280-298):
```rust
// Lines 282-288: Read access
pub fn with_dsp<F, R>(&self, f: F) -> R
where
    F: FnOnce(&crate::audio::dsp::DspChain) -> R,
{
    let dsp = self.engine.dsp.read();
    f(&dsp)
}

// Lines 290-297: Write access
pub fn with_dsp_mut<F>(&self, f: F)
where
    F: FnOnce(&mut crate::audio::dsp::DspChain),
{
    let mut dsp = self.engine.dsp.write();
    f(&mut dsp);
}
```
**Divergence:** Add new public methods mirroring the existing pattern:
```rust
/// Force limiter on (for normalization auto-on).
/// Returns previous limiter enabled state for restore.
pub fn force_limiter_on(&self) -> bool {
    self.with_dsp_mut(|dsp| dsp.force_limiter_on())
}

/// Restore limiter to its previous state.
pub fn restore_limiter(&self, was_enabled: bool) {
    self.with_dsp_mut(|dsp| dsp.restore_limiter(was_enabled));
}
```

---

### 5. `src/gui/widgets.rs` (custom widget/component, event-driven)

**Analog:** `src/gui/widgets.rs:2307-3679` (CustomSlider — full Widget impl)

#### Imports pattern (lines 1-25):
```rust
// Lines 1-25: All imports for CustomSlider
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer,
    widget::{Operation, Tree},
};
use iced::{
    Alignment, Color, Element, Length, Padding, Theme,
    widget::{...},
};
use iced::{Event, Rectangle, Size, Vector};

use crate::gui::theme::{
    COLOR_ACCENT, COLOR_BG, COLOR_CONTRAST, COLOR_TEXT_PRIMARY, COLOR_TEXT_SECONDARY,
    FONT_INTER_SANS_MEDIUM, FONT_INTER_SANS_NORMAL,
};
use std::sync::atomic::{AtomicBool, Ordering};
```
**Divergence:** Same imports needed for both StandardCheckbox and NumberStepper. Add `use iced::keyboard;` for key handling in stepper.

#### CustomSlider struct + builder pattern (lines 2460-2676):
```rust
// Lines 2460-2473: Widget struct
pub struct CustomSlider<'a, Message> {
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    on_change: Box<dyn Fn(f32) -> Message + 'a>,
    on_right_click: Box<dyn Fn() -> Message + 'a>,
    width: Length,
    height: Length,
    // ... options, format_fn, callbacks ...
}

// Lines 2477-2497: new() constructor
pub fn new(value: f32, range: ..., on_change: impl Fn(f32) -> Message + 'a, ...) -> Self {
    Self {
        value: value.clamp(*range.start(), *range.end()),
        range,
        on_change: Box::new(on_change),
        // ... defaults ...
    }
}
```

#### NumberStepper struct pattern (NEW — divergence):
```rust
/// Enum for stepper unit type (D-33)
pub enum StepperUnit {
    Decibels,      // step 0.25, suffix "dB"
    Milliseconds,  // step 50, suffix "ms"
}

/// NumberStepper widget: 64×14px with ← → chevrons + manual input (D-32, D-33, D-34)
pub struct NumberStepper<'a, Message> {
    value: f64,
    range: std::ops::RangeInclusive<f64>,
    unit: StepperUnit,
    on_change: Box<dyn Fn(f64) -> Message + 'a>,
    on_selected_state_change: Option<Box<dyn Fn(bool) -> Message + 'a>>,
}
```

#### StandardCheckbox struct pattern (NEW — divergence):
```rust
/// StandardCheckbox: 12×12 px custom checkbox (D-31)
pub struct StandardCheckbox<'a, Message> {
    checked: bool,
    on_toggle: Box<dyn Fn(bool) -> Message + 'a>,
}
```

#### impl Widget pattern (lines 2853-2893):
```rust
// Lines 2853-2893: Widget trait implementation
impl<'a, Message: 'a> iced::advanced::Widget<Message, Theme, iced::Renderer>
    for CustomSlider<'a, Message>
{
    fn size(&self) -> iced::Size<Length> {
        iced::Size { width: self.width, height: self.height }
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(CustomSliderState::default())
    }

    fn layout(&mut self, ...) -> iced::advanced::layout::Node {
        let mut size = limits.resolve(self.width, self.height, iced::Size::ZERO);
        // ... input expansion logic ...
        iced::advanced::layout::Node::new(size)
    }
```
**Divergence:** Both StandardCheckbox and NumberStepper implement the same Widget trait. StandardCheckbox uses fixed 12×12 size. NumberStepper uses fixed 64×14 size. NumberStepper needs `on_event` mirroring the CustomSlider pattern (lines 2896-3196) with the following key event handlers:

#### Keyboard input pattern (lines 3135-3187 — to copy for NumberStepper + Fix B2):
```rust
// Lines 3135-3187: Keyboard input for CustomSlider
iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. })
    if self.options.enable_keyboard_input && state.input_has_focus =>
{
    match key {
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => {
            shell.capture_event();
            let parsed = state.input_value_text.parse::<f32>();
            if let Ok(v) = parsed {
                let clamped = snapped.clamp(*self.range.start(), *self.range.end());
                self.value = clamped;
                shell.publish((self.on_change)(clamped));
            }
            state.is_input_editing = false;
            state.input_has_focus = false;
        }
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => { ... }
        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft) => { ... }
        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight) => { ... }
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
            if state.input_cursor_pos > 0 && !state.input_value_text.is_empty() {
                state.input_value_text.remove(state.input_cursor_pos - 1);
                state.input_cursor_pos -= 1;
            }
        }
        iced::keyboard::Key::Character(c) => {
            if state.input_value_text.len() < 20 {
                state.input_value_text.insert(state.input_cursor_pos, c.chars().next().unwrap_or(' '));
                state.input_cursor_pos += 1;
            }
        }
        _ => {}
    }
}
```
**Divergence for NumverStepper:** 
1. Copy the pattern but add character filter (digits, `.`, `-` only) at Character arm (D-34)
2. Add Delete key support (missing in CustomSlider): `state.input_value_text.remove(state.input_cursor_pos)`
3. Strip suffix before parse on Enter/click-outside (Fix B2 pattern applied here too)
4. **No rounding to step** on manual input apply (D-34) — unlike CustomSlider line 3143-3144 which snaps

#### Fix B2 in existing CustomSlider (lines 3141, 2963):
```rust
// Lines 3141-3148: Enter key — ADD suffix strip BEFORE parse
// OLD: let parsed = state.input_value_text.parse::<f32>();
// NEW:
let stripped = state.input_value_text.trim()
    .trim_end_matches(" dB").trim_end_matches(" ms")
    .trim_end_matches(" Hz").trim_end_matches(" %")
    .trim();
let parsed = stripped.parse::<f32>();

// Lines 2963: Click-outside — SAME fix
// Lines 3179-3184: Character input — ADD character filter (digits, '.', '-', whitespace only)
```

#### draw_input pattern for NumberStepper (lines 3545-3671):
```rust
// Lines 3545-3671: Custom input drawing with cursor
fn draw_input(&self, renderer: &mut iced::Renderer, slider_bounds: Rectangle, state: &CustomSliderState) {
    use iced::advanced::Renderer as _;
    use iced::advanced::text::Renderer as _;

    // Background quad
    renderer.fill_quad(
        iced::advanced::graphics::core::renderer::Quad {
            bounds: input_rect,
            border: iced::Border { radius: border_radius.into(), width: border_width, color: border_color },
            ..Default::default()
        },
        bg,
    );

    // Text with cursor
    let display = if state.is_input_editing {
        let pos = state.input_cursor_pos.min(display_text.len());
        let (before, after) = display_text.split_at(pos);
        format!("{}|{}", before, after)
    } else {
        display_text
    };

    renderer.fill_text(
        iced::advanced::text::Text {
            content: display,
            bounds: iced::Size::new(input_width - pad_h * 2.0, text_bounds_h),
            size: iced::Pixels(font_size),
            font: FONT_INTER_SANS_MEDIUM,
            // ...
        },
        iced::Point::new(text_x, text_y),
        font_color,
        input_rect,
    );
}
```
**Divergence:** NumberStepper's draw method uses the same `fill_quad` + `fill_text` pattern but:
- Chevrons drawn as Unicode text glyphs (`◀` U+25C0 and `▶` U+25B6) via `fill_text` at 12-14px (Pattern 8 recommendation from RESEARCH.md)
- Fixed layout: left chevron (14px) | value text (36px) | right chevron (14px) = 64px total
- Suffix ("dB"/"ms") appended to value display, excluded from edit buffer

#### From<Widget> pattern (lines 3675-3679):
```rust
// Lines 3675-3679: Conversion to Element
impl<'a, Message: 'a> From<CustomSlider<'a, Message>> for Element<'a, Message> {
    fn from(slider: CustomSlider<'a, Message>) -> Self {
        Element::new(slider)
    }
}
```
**Divergence:** Both StandardCheckbox and NumberStepper need identical `From` impls.

---

### 6. `src/gui/app.rs` (orchestrator/controller, event-driven/message-routing)

**Analog:** `src/gui/app.rs:3879-3963` (AudioCenterMsg routing + persistence) + `2526-2646` (GlobalKeyDown)

#### AudioCenterMsg routing pattern (lines 3879-3963):
```rust
// Lines 3879-3891: Focus gating via SliderHoverActive
Message::AudioCenterMsg(ac_msg) => {
    match &ac_msg {
        crate::gui::audio_center::AudioCenterMessage::SliderHoverActive(true) => {
            if self.focus != AppFocus::AudioCenter {
                self.previous_focus = self.focus;
            }
            self.focus = AppFocus::AudioCenter;
        }
        crate::gui::audio_center::AudioCenterMessage::SliderHoverActive(false) => {
            self.focus = self.previous_focus;
        }
        _ => {}
    }

    // Forward to audio_center_manager.update()
    self.audio_center_manager
        .update(ac_msg.clone(), &self.audio_manager, &self.database);

    // Persistence — immediate save for EQ changes (lines 3934-3946)
    match &ac_msg {
        crate::gui::audio_center::AudioCenterMessage::EqToggleSelected(_)
        | crate::gui::audio_center::AudioCenterMessage::EqPreampChanged(_)
        | ... => {
            self.audio_center_manager.save_eq_settings_to_db(&*self.database);
        }
        _ => {}
    }
    // Persistence — DSP changes (lines 3949-3960)
    match &ac_msg {
        crate::gui::audio_center::AudioCenterMessage::DspToggle(_, _)
        | crate::gui::audio_center::AudioCenterMessage::DspValueChanged(_, _)
        | ... => {
            self.audio_center_manager.save_dsp_settings_to_db(&*self.audio_manager, &*self.database);
        }
        _ => {}
    }
```
**Divergence:** Add a THIRD persistence match block for Volumen settings (D-42):
```rust
match &ac_msg {
    // Volumen y Mezcla: cualquier cambio se guarda de inmediato
    crate::gui::audio_center::AudioCenterMessage::VolumenFadesToggle(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenFadeInChanged(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenFadeOutChanged(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenSilenceToggle(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenSilenceDurationChanged(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenSilenceThresholdChanged(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenNormalizeToggle(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenNormalizeTargetChanged(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenNormalizeCapChanged(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenRgMasterToggle(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenRgTrackToggle(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenRgAlbumToggle(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenRgAnalyzeRtToggle(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenRgOffsetAlbumChanged(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenRgOffsetTrackChanged(_)
    | crate::gui::audio_center::AudioCenterMessage::VolumenRgOffsetRtChanged(_) => {
        self.audio_center_manager.save_volumen_settings_to_db(&*self.audio_manager, &*self.database);
    }
    _ => {}
}
```

#### VolumeChanged pattern (lines 1266-1269) — Fix B3 (D-43):
```rust
// Lines 1266-1269: Current handler — ADD persistence
Message::VolumeChanged(vol) => {
    self.audio_manager.set_volume(vol);
    Task::none()
}
```
**Divergence:** Add DB save:
```rust
Message::VolumeChanged(vol) => {
    self.audio_manager.set_volume(vol);
    if let Ok(db) = self.database.lock() {
        let _ = db.set_setting("player_volume", &format!("{:.4}", vol));
    }
    Task::none()
}
```

#### GlobalKeyDown pattern (lines 2526-2646):
```rust
// Lines 2526-2544: Arrow keys gated by AppFocus
Message::GlobalKeyDown(key, modifiers) => {
    match key {
        Key::Named(Named::ArrowUp) => {
            if self.focus == AppFocus::Playlist { ... }
            else if self.focus == AppFocus::AudioCenter { Task::none() }
            else { self.update(Message::LibraryKeyNav(...)) }
        }
        Key::Named(Named::ArrowDown) => { ... }
        Key::Named(Named::ArrowLeft) => { ... }
        Key::Named(Named::ArrowRight) => { ... }
        ...
    }
}
```
**Divergence:** When `AppFocus::AudioCenter`, arrow keys should now be forwarded to the active NumberStepper widget in the AudioCenter tab (D-35). The existing pattern already returns `Task::none()` — the individual widget captures keys via `shell.capture_event()` in its `on_event`. No change needed to GlobalKeyDown; the gating ensures arrows don't escape to Playlist/Library.

#### AppFocus enum (lines 354-359):
```rust
// Lines 354-359: Existing AppFocus
pub enum AppFocus {
    Library,
    Playlist,
    AudioCenter,
}
```
**Divergence:** No changes needed. `AudioCenter` already exists and is the correct focus for the new tab's widgets.

---

### 7. `src/gui/audio_center.rs` (manager/component, event-driven + persistence)

**Analog:** Multiple sections — tab structure, view layout, persistence, state toggles

#### AudioCenterMessage enum pattern (lines 13-55):
```rust
// Lines 13-55: Existing message variants
#[derive(Debug, Clone)]
pub enum AudioCenterMessage {
    TabSelected(usize),
    DragStart,
    // Tab 1: Config
    HostSelected(String),
    DeviceSelected(String),
    // Tab 2: EQ
    EqToggleSelected(bool),
    EqBandsSelected(bool),
    EqPreampChanged(f32),
    // Tab 3: Efectos
    DspToggle(DspEffect, bool),
    DspValueChanged(DspEffect, f32),
    AudioStateToggle(AudioStateToggle, bool),
    AudioStateValueChanged(AudioStateToggle, f32),
    SliderHoverActive(bool),
}
```
**Divergence:** Add ~16 new message variants for Volumen y Mezcla (after line 53):
```rust
// Tab 3 → becomes Tab 4 (or re-indexed)
// Tab 4: Volumen y Mezcla (NEW)
VolumenFadesToggle(bool),
VolumenFadeInChanged(f64),
VolumenFadeOutChanged(f64),
VolumenSilenceToggle(bool),
VolumenSilenceDurationChanged(f64),
VolumenSilenceThresholdChanged(f64),
VolumenNormalizeToggle(bool),
VolumenNormalizeTargetChanged(f64),
VolumenNormalizeCapChanged(f64),
VolumenRgMasterToggle(bool),
VolumenRgTrackToggle(bool),
VolumenRgAlbumToggle(bool),
VolumenRgAnalyzeRtToggle(bool),
VolumenRgOffsetAlbumChanged(f64),
VolumenRgOffsetTrackChanged(f64),
VolumenRgOffsetRtChanged(f64),
```

#### Tab bar + content dispatch pattern (lines 1310-1363):
```rust
// Lines 1310: tab_names array
let tab_names = ["Configuración de Audio", "Ecualizador", "Efectos de Audio"];

// Lines 1357-1363: Content dispatch
let content: Element<'a, crate::gui::app::Message> = match manager.selected_tab {
    0 => view_audio_config(manager, audio_manager),
    1 => view_equalizer(manager, audio_manager),
    2 => view_audio_effects(manager, audio_manager),
    _ => Space::new().into(),
};
```
**Divergence:** 
- Add `"Volumen y Mezcla"` to `tab_names` array (index 3, D-36)
- Add `3 => view_volumen_mezcla(manager, audio_manager),` to the match (D-36)
- Move existing Tab 3 ("Efectos de Audio") from index 2 → index 2 stays (no re-index needed; Volumen is 'índice 3' meaning the fourth tab, 0-indexed as 3)

#### Two-column layout pattern (lines 2284-2325 — view_audio_config):
```rust
// Lines 2284-2289: Vertical divider
let main_divider = container(
    Space::new().width(Length::Fixed(2.0)).height(Length::Fixed(240.0)),
).style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

// Lines 2293-2325: Two-column row
column![
    row![
        container(left_col).width(Length::FillPortion(5))
            .padding(iced::Padding { top: -33.0, ... }),
        container(main_divider)
            .width(Length::Fixed(90.0))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
        container(right_col)
            .width(Length::FillPortion(4))
            .padding(iced::Padding { ... })
    ]
    .height(Length::Fill)
    .align_y(Alignment::Center),
    bottom_actions  // NO bottom_actions for Volumen tab (D-37)
]
.into()
```
**Divergence for view_volumen_mezcla (D-37):**
- Change `FillPortion(5)` → `FillPortion(1)` and `FillPortion(4)` → `FillPortion(1)` (equal width columns)
- Remove `bottom_actions` row entirely (no apply button)
- Column 1: Fades group + Silence group
- Column 2: Normalize group + Replay Gain group

#### Group separator pattern (lines 2889-2898):
```rust
// Lines 2889-2898: Horizontal group separator (2px COLOR_TEXT_SECONDARY)
container(
    container(Space::new().width(Length::Fill).height(Length::Fixed(2.0)))
        .style(|_t: &Theme| container::Style::default().background(COLOR_TEXT_SECONDARY))
)
.padding(iced::Padding {
    top: 7.0, right: 15.0, bottom: 7.0, left: 15.0,
}),
```
**Divergence:** Use identically between each Volumen group (D-38).

#### Disabled pattern (lines 2240-2282):
```rust
// Lines 2267-2282: Disabled button with scale_alpha(0.5) + no on_press
button(text("Aplicar").size(15).font(FONT_INTER_SANS_MEDIUM))
    .padding([8, 12])
    .style(|_t: &Theme, _status| button::Style {
        background: Some(COLOR_CONTRAST.into()),
        text_color: COLOR_TEXT_SECONDARY.scale_alpha(0.5),
        // ...
    })
```
**Divergence:** Apply the same pattern to group sub-functions when the group master is OFF (D-40). Use `text_color: COLOR_TEXT_SECONDARY.scale_alpha(0.5)` and omit `on_press` for checkboxes/steppers in disabled groups.

#### AudioStateToggle write pattern (lines 1160-1177):
```rust
// Lines 1160-1177: Direct AudioState write pattern
AudioCenterMessage::AudioStateToggle(toggle, enabled) => {
    let state_arc = audio_manager.state();
    let mut state = state_arc.write();
    match toggle {
        AudioStateToggle::DownmixCenter => state.downmix_center_enabled = enabled,
        AudioStateToggle::DownmixLfe => state.downmix_lfe_enabled = enabled,
        AudioStateToggle::DownmixSurround => state.downmix_surround_enabled = enabled,
    }
}
```
**Divergence:** Add similar match arms for Volumen message variants, following the exact same pattern:
```rust
AudioCenterMessage::VolumenFadesToggle(enabled) => {
    audio_manager.state().write().fades_enabled = enabled;
}
AudioCenterMessage::VolumenFadeInChanged(val) => {
    audio_manager.state().write().fade_in_ms = val as f32;
}
// ... etc for all new state fields ...
```

#### save_dsp_settings_to_db pattern (lines 278-315):
```rust
// Lines 278-315: Persistence function
pub fn save_dsp_settings_to_db(&self, audio_manager: &AudioManager, db: &std::sync::Mutex<crate::db::Database>) {
    if let Ok(db_lock) = db.lock() {
        audio_manager.with_dsp(|dsp| {
            let _ = db_lock.set_setting("dsp_sub_bass_enabled", if dsp.sub_bass.enabled { "1" } else { "0" });
            let _ = db_lock.set_setting("dsp_sub_bass_gain", &format!("{:.1}", dsp.sub_bass.gain));
            // ...
        });

        let state = audio_manager.state();
        let state_read = state.read();
        let _ = db_lock.set_setting("audio_downmix_center_enabled", if state_read.downmix_center_enabled { "1" } else { "0" });
        // ...
    }
}
```
**Divergence:** Create `save_volumen_settings_to_db()` following the exact same pattern:
```rust
pub fn save_volumen_settings_to_db(&self, audio_manager: &AudioManager, db: &std::sync::Mutex<crate::db::Database>) {
    if let Ok(db_lock) = db.lock() {
        let state = audio_manager.state();
        let s = state.read();
        let _ = db_lock.set_setting("vol_fades_enabled", if s.fades_enabled { "1" } else { "0" });
        let _ = db_lock.set_setting("vol_fade_in_ms", &format!("{:.0}", s.fade_in_ms));
        let _ = db_lock.set_setting("vol_fade_out_ms", &format!("{:.0}", s.fade_out_ms));
        let _ = db_lock.set_setting("vol_silence_enabled", if s.silence_enabled { "1" } else { "0" });
        let _ = db_lock.set_setting("vol_silence_duration_ms", &format!("{:.0}", s.silence_duration_ms));
        let _ = db_lock.set_setting("vol_silence_threshold_db", &format!("{:.2}", s.silence_threshold_db));
        let _ = db_lock.set_setting("vol_normalize_enabled", if s.normalize_enabled { "1" } else { "0" });
        let _ = db_lock.set_setting("vol_normalize_target_db", &format!("{:.2}", s.normalize_target_db));
        let _ = db_lock.set_setting("vol_normalize_cap_db", &format!("{:.2}", s.normalize_cap_db));
        let _ = db_lock.set_setting("vol_rg_master_enabled", if s.rg_master_enabled { "1" } else { "0" });
        let _ = db_lock.set_setting("vol_rg_track_enabled", if s.replay_gain_track_enabled { "1" } else { "0" });
        let _ = db_lock.set_setting("vol_rg_album_enabled", if s.replay_gain_album_enabled { "1" } else { "0" });
        let _ = db_lock.set_setting("vol_rg_analyze_rt_enabled", if s.rg_analyze_rt_enabled { "1" } else { "0" });
        let _ = db_lock.set_setting("vol_rg_offset_album_db", &format!("{:.2}", s.rg_offset_album_db));
        let _ = db_lock.set_setting("vol_rg_offset_track_db", &format!("{:.2}", s.rg_offset_track_db));
        let _ = db_lock.set_setting("vol_rg_offset_rt_db", &format!("{:.2}", s.rg_offset_rt_db));
    }
}
```

#### sync_from_engine pattern (lines 317+):
```rust
// Lines 317-379: Load settings from DB at startup/sync
pub fn sync_from_engine(&mut self, audio_manager: &AudioManager) {
    if let Some(db_arc) = audio_manager.get_database() {
        if let Ok(db) = db_arc.try_lock() {
            // Load each setting with .get_setting()
            if let Some(val) = db.get_setting("eq_enabled") {
                self.equalizer_enabled = val == "1";
            }
            // ...
        }
    }
}
```
**Divergence:** Add Volumen settings loading to `sync_from_engine`, using `format!("{:.2}", val).parse::<f32>()` for f64 values stored as strings. Also load these settings in `App::new` startup at `app.rs:594-727`.

---

### 8. `src/gui/theme.rs` (config/constants, static config)

**Analog:** `src/gui/theme.rs:1-40` (existing colors + fonts)

#### Constants pattern (lines 5-27):
```rust
// Lines 5-27: All color and font constants
pub const COLOR_ACCENT: Color = color!(0xFF003D);
pub const COLOR_BG: Color = color!(0x000000);
pub const COLOR_CONTRAST: Color = color!(0x111111);
pub const COLOR_TEXT_PRIMARY: Color = color!(0xAFAFAF);
pub const COLOR_TEXT_SECONDARY: Color = color!(0x5B5B5B);
pub const COLOR_SUCCESS: Color = color!(0x10B981);
pub const COLOR_WARNING: Color = color!(0xFBBF24);

pub const FONT_INTER_SANS_MEDIUM: Font = Font { ... };
pub const FONT_INTER_SANS_NORMAL: Font = Font { ... };
```
**Divergence:** May need minor additions (e.g., `COLOR_SILENCE_INDICATOR` or `COLOR_CHEVRON_HOVER`), but most colors are already reusable. No changes expected unless the planner decides specific new colors are warranted.

---

### 9. `src/db/database.rs` (persistence/model, CRUD SQLite)

**Analog:** `src/db/database.rs:2033-2050` (get_setting/set_setting)

#### APP_SETTINGS pattern (lines 2033-2050):
```rust
// Lines 2033-2041: get_setting
pub fn get_setting(&self, key: &str) -> Option<String> {
    self.conn
        .query_row("SELECT value FROM APP_SETTINGS WHERE key = ?1", [key], |r| r.get(0))
        .ok()
}

// Lines 2044-2050: set_setting
pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
    self.conn.execute(
        "INSERT OR REPLACE INTO APP_SETTINGS (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}
```
**Divergence:** No code changes to database.rs. The `get_setting`/`set_setting` API is fully generic and already handles arbitrary key-value pairs. New keys are used from the caller side (audio_center.rs → save_volumen_settings_to_db()). The APP_SETTINGS table is schemaless; no migrations needed.

---

### 10. `src/gui/widgets_tests.rs` (test, unit test) — NEW FILE

**Analog:** `src/audio/dsp_tests.rs:1-55` (test structure) + `src/audio/tests.rs:42-56` (state defaults)

#### Test module pattern (dsp_tests.rs lines 1-55):
```rust
// Lines 1-6: Module declaration + imports
#[cfg(test)]
mod tests {
    use crate::audio::dsp::{
        Compressor, DspChain, Equalizer, ...
    };

    // Lines 12-17: Basic assertion test
    #[test]
    fn test_dsp_chain_default_enabled() {
        let chain = DspChain::default();
        assert!(chain.enabled);
        assert!((chain.preamp_gain - 1.0).abs() < 1e-6);
    }

    // Lines 20-27: Bypass test (no modification)
    #[test]
    fn test_dsp_chain_process_frame_bypass() {
        let mut chain = DspChain::default();
        chain.enabled = false;
        let mut frame = [1.0_f64, -0.5_f64];
        chain.process_frame(&mut frame);
        assert!((frame[0] - 1.0).abs() < 1e-10);
        assert!((frame[1] + 0.5).abs() < 1e-10);
    }
}
```

#### State defaults test pattern (tests.rs lines 42-56):
```rust
// Lines 42-56: Testing default state values
#[test]
fn test_audio_state_defaults() {
    let state = AudioState::default();
    assert!(!state.is_playing);
    assert!((state.volume - 0.3).abs() < f64::EPSILON as f32);
    assert_eq!(state.sample_rate, 44100);
    assert_eq!(state.replay_gain_track_enabled, true);
    assert_eq!(state.replay_gain_album_enabled, true);
}
```

**Divergence — new file structure:**
```rust
// src/gui/widgets_tests.rs
#[cfg(test)]
mod tests {
    use crate::gui::widgets::{StandardCheckbox, NumberStepper, StepperUnit};

    // --- StandardCheckbox tests ---

    #[test]
    fn test_standard_checkbox_default_off() {
        // Verify initial state
    }

    #[test]
    fn test_standard_checkbox_toggle() {
        // Verify callback fires with correct value
    }

    // --- NumberStepper tests ---

    #[test]
    fn test_stepper_parse_db_no_suffix() {
        // "-14.25" → Ok(-14.25), no rounding
    }

    #[test]
    fn test_stepper_parse_db_with_suffix() {
        // "-14.25 dB" → strip → Ok(-14.25)
    }

    #[test]
    fn test_stepper_parse_ms() {
        // "1000 ms" → Ok(1000.0)
    }

    #[test]
    fn test_stepper_no_rounding_on_manual_input() {
        // "1.78" stays 1.78 (not rounded to step 0.25)
    }

    #[test]
    fn test_stepper_range_clamp() {
        // Values outside range clamped to bounds
    }

    #[test]
    fn test_stepper_character_filter() {
        // Non-digit chars rejected (only digits, '.', '-', whitespace)
    }

    // --- Suffix stripping (Fix B2) ---

    #[test]
    fn test_suffix_strip_db() {
        // "-50.25 dB" → parse Ok(-50.25)
    }

    #[test]
    fn test_suffix_strip_ms() {
        // "1000 ms" → parse Ok(1000.0)
    }

    #[test]
    fn test_suffix_strip_hz() {
        // "44100 Hz" → parse Ok(44100.0)
    }

    #[test]
    fn test_suffix_strip_no_suffix() {
        // "3.14" → parse Ok(3.14)
    }
}
```

---

## Shared Patterns

### Authentication/Authorization
**Source:** N/A — Desktop app, no auth required.

### Error Handling
**Source:** `src/audio/error.rs` (AudioError enum)
**Apply to:** All audio module files (decoder.rs, engine.rs, manager.rs)
```rust
// AudioError enum pattern — all audio functions return Result<_, AudioError>
// New code in decoder.rs returns () from hot loop (no Result, panics are caught by thread boundary)
// manager.rs new methods follow existing pattern: no Result for state writes, just RwLock
```

### Locking Conventions
**Source:** `src/audio/engine.rs:1-14` (`parking_lot::RwLock` for AudioState/DspChain, `std::sync::Mutex` for Database)
**Apply to:** All files touching AudioState or DspChain
```rust
// AudioState: parking_lot::RwLock
let s = state.read();     // read lock
let mut s = state.write(); // write lock

// DspChain: parking_lot::RwLock
engine.dsp.read()   // read access (with_dsp)
engine.dsp.write()  // write access (with_dsp_mut)

// Database: std::sync::Mutex
db.lock()         // blocking lock
db_arc.try_lock()  // non-blocking (used in decoder hot path)
```
**Divergence:** Fix B1 changes the locking strategy at `decoder.rs:820` from `try_write()` to spin-wait with timeout. This keeps the same lock type but changes acquisition semantics.

### Thread Boundary
**Source:** `src/audio/decoder.rs:246` (decode loop is a background thread, GUI is iced main thread)
**Apply to:** decoder.rs new code
```rust
// Decoder thread: writes to ringbuf, reads AudioState (parking_lot::RwLock)
// GUI thread: writes AudioState, reads ringbuf via CPAL callback
// Database: shared via std::sync::Mutex<Database>, accessed from both threads
```

### Zero-Allocation Hot Path
**Source:** `src/audio/decoder.rs:260-264` (pre-allocated buffers: `Vec::with_capacity(131072)`)
**Apply to:** All new per-frame audio processing in decoder.rs
```rust
// All new per-frame operations must be O(channels) with zero allocation:
let mut rms_window: VecDeque<f64> = VecDeque::with_capacity(18000); // ~400ms at 44.1kHz
let mut normalization_gain_db: f64 = 0.0;
// Per-frame: iterator methods (SIMD compilable), no Vec::push in hot path
```

### Persistence
**Source:** `src/gui/audio_center.rs:278-315` (save_dsp_settings_to_db) + `src/gui/app.rs:3934-3960` (message match → save)
**Apply to:** All Volumen settings persistence
```rust
// Pattern: Db lock → state.read() → set_setting for each field
// Trigger: immediate on every message (no "apply" button), matched in app.rs
```

### Custom Widget: Iced Advanced API
**Source:** `src/gui/widgets.rs:2853-3679` (impl Widget for CustomSlider)
**Apply to:** StandardCheckbox, NumberStepper, and Fix B2 in CustomSlider
```rust
// Key trait bounds:
impl<'a, Message: 'a> iced::advanced::Widget<Message, Theme, iced::Renderer>
    for YourWidget<'a, Message> { ... }

// Key methods: size(), state(), layout(), on_event(), draw()
// Focus management: on_selected_state_change callback → AudioCenterMessage::SliderHoverActive
// Drawing: renderer.fill_quad() for backgrounds/borders, renderer.fill_text() for labels
```

### DB Read at Startup
**Source:** `src/gui/audio_center.rs:317-379` (sync_from_engine) + `src/gui/app.rs:594-727` (App::new loading)
**Apply to:** Loading Volumen settings at startup
```rust
// Pattern: db.get_setting("key") → parse → set on manager field or AudioState.write()
if let Some(val) = db.get_setting("vol_fades_enabled") {
    audio_manager.state().write().fades_enabled = val == "1";
}
```

---

## No Analog Found

All 10 files have close analogs in the existing codebase. No files need to fall back to RESEARCH.md patterns exclusively.

| File | Role | Data Flow | Closest Match Quality |
|------|------|-----------|----------------------|
| — | — | — | None needed — all matched |

---

## Metadata

**Analog search scope:**
- `src/audio/decoder.rs` (full file, 897 lines)
- `src/audio/engine.rs` (full file, 806 lines)
- `src/audio/dsp.rs` (sections at 48-106, 1911-1938)
- `src/audio/manager.rs` (full file, 298 lines)
- `src/gui/widgets.rs` (sections at 1-25, 2307-3679)
- `src/gui/audio_center.rs` (sections at 13-80, 278-379, 1140-1189, 1300-1379, 2230-2325, 2880-2909)
- `src/gui/app.rs` (sections at 354-359, 1260-1269, 2520-2646, 3870-3963)
- `src/gui/theme.rs` (full file, 40 lines)
- `src/db/database.rs` (section at 2030-2059)
- `src/audio/tests.rs` (section at 1-60)
- `src/audio/dsp_tests.rs` (section at 1-55)

**Files scanned:** ~3,600 total lines across 11 files
**Pattern extraction date:** 2026-07-18
