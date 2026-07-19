use iced::advanced::{
    layout, mouse, overlay, renderer,
    widget::{Operation, Tree},
    Clipboard, Layout, Shell, Widget,
};
use iced::{
    widget::{
        button, column, container, mouse_area, pick_list, radio, row, scrollable, svg, text,
        text_input, toggler, Responsive, Space,
    },
    Alignment, Color, Element, Length, Padding, Theme,
};
use iced::{Event, Rectangle, Size, Vector};

use crate::gui::theme::{
    COLOR_ACCENT, COLOR_BG, COLOR_CONTRAST, COLOR_TEXT_PRIMARY, COLOR_TEXT_SECONDARY,
    FONT_INTER_SANS_MEDIUM, FONT_INTER_SANS_NORMAL,
};
use crate::utils::{format_duration, format_metadata, truncate_text, SortColumn};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Flag global: true cuando algún slider tiene is_selected activo.
/// Los demás sliders revisan esto para ocultar su tooltip en hover.
pub static GLOBAL_SLIDER_SELECTED: AtomicBool = AtomicBool::new(false);

// ==============================
// Helper: strip unit suffixes from numeric strings before parsing (D-45)
// Used by CustomSlider and NumberStepper for keyboard input.
// ==============================
fn strip_suffix(s: &str) -> &str {
    s.trim()
        .trim_end_matches(" dB")
        .trim_end_matches(" ms")
        .trim_end_matches(" Hz")
        .trim_end_matches(" %")
        .trim()
}

// ==============================
// StandardCheckbox — 12×12 px custom checkbox (D-31)
// ==============================

#[derive(Debug, Clone, Default)]
struct StandardCheckboxState {
    is_hover: bool,
}

/// Custom checkbox widget: 12×12 px drawn via iced advanced renderer (D-31).
/// OFF: COLOR_BG background, 1px COLOR_TEXT_SECONDARY border.
/// ON: COLOR_ACCENT background, 1px COLOR_ACCENT border, "✓" in COLOR_TEXT_PRIMARY centered.
pub struct StandardCheckbox<'a, Message> {
    checked: bool,
    on_toggle: Box<dyn Fn(bool) -> Message + 'a>,
    on_selected_state_change: Option<Box<dyn Fn(bool) -> Message + 'a>>,
}

impl<'a, Message> StandardCheckbox<'a, Message> {
    /// Create a new StandardCheckbox with the given initial state and toggle callback.
    pub fn new(checked: bool, on_toggle: impl Fn(bool) -> Message + 'a) -> Self {
        Self {
            checked,
            on_toggle: Box::new(on_toggle),
            on_selected_state_change: None,
        }
    }

    /// Set callback for focus-gating state changes (D-35).
    /// Called with `true` when the checkbox receives a click,
    /// `false` when the mouse button is released outside its bounds.
    pub fn on_selected_state_change(mut self, callback: impl Fn(bool) -> Message + 'a) -> Self {
        self.on_selected_state_change = Some(Box::new(callback));
        self
    }
}

impl<'a, Message: 'a> iced::advanced::Widget<Message, Theme, iced::Renderer>
    for StandardCheckbox<'a, Message>
{
    fn size(&self) -> iced::Size<Length> {
        iced::Size {
            width: Length::Fixed(12.0),
            height: Length::Fixed(12.0),
        }
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(StandardCheckboxState::default())
    }

    fn layout(
        &mut self,
        _tree: &mut iced::advanced::widget::Tree,
        _renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let size = limits.resolve(Length::Fixed(12.0), Length::Fixed(12.0), iced::Size::ZERO);
        iced::advanced::layout::Node::new(size)
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        _viewport: &iced::Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<StandardCheckboxState>();

        let Some(cursor_pos) = cursor.position() else {
            state.is_hover = false;
            return;
        };

        state.is_hover = bounds.contains(cursor_pos);

        match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                if bounds.contains(cursor_pos) {
                    shell.capture_event();
                    let new_checked = !self.checked;
                    self.checked = new_checked;
                    shell.publish((self.on_toggle)(new_checked));
                    if let Some(ref cb) = self.on_selected_state_change {
                        shell.publish(cb(true));
                    }
                }
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                if !bounds.contains(cursor_pos) {
                    if let Some(ref cb) = self.on_selected_state_change {
                        shell.publish(cb(false));
                    }
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        _tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        _theme: &Theme,
        _style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        _viewport: &iced::Rectangle,
    ) {
        use iced::advanced::text::Renderer as _;
        use iced::advanced::Renderer as _;

        let bounds = layout.bounds();

        let bg = if self.checked { COLOR_ACCENT } else { COLOR_BG };
        let border_color = if self.checked {
            COLOR_ACCENT
        } else {
            COLOR_TEXT_SECONDARY
        };

        renderer.fill_quad(
            iced::advanced::graphics::core::renderer::Quad {
                bounds,
                border: iced::Border {
                    radius: 1.0.into(),
                    width: 1.0,
                    color: border_color,
                },
                ..Default::default()
            },
            bg,
        );

        if self.checked {
            let font_size = 10.0;
            let text_x = bounds.x + bounds.width / 2.0;
            let text_y = bounds.y + (bounds.height - font_size) / 2.0;

            renderer.fill_text(
                iced::advanced::text::Text {
                    content: "✓".to_string(),
                    bounds: iced::Size::new(bounds.width, font_size),
                    size: iced::Pixels(font_size),
                    line_height: iced::advanced::text::LineHeight::Relative(1.0),
                    font: FONT_INTER_SANS_MEDIUM,
                    align_x: iced::alignment::Horizontal::Center.into(),
                    align_y: iced::alignment::Vertical::Top,
                    shaping: iced::advanced::text::Shaping::Basic,
                    wrapping: iced::advanced::text::Wrapping::None,
                },
                iced::Point::new(text_x, text_y),
                COLOR_TEXT_PRIMARY,
                bounds,
            );
        }
    }
}

impl<'a, Message: 'a> From<StandardCheckbox<'a, Message>> for Element<'a, Message> {
    fn from(checkbox: StandardCheckbox<'a, Message>) -> Self {
        Element::new(checkbox)
    }
}

// ==============================
// NumberStepper — 64×14 px with ← → chevrons + manual input (D-32, D-33, D-34, D-35)
// ==============================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepperUnit {
    /// Step 0.25, suffix "dB"
    Decibels,
    /// Step 50, suffix "ms"
    Milliseconds,
}

impl StepperUnit {
    fn step(&self) -> f64 {
        match self {
            StepperUnit::Decibels => 0.25,
            StepperUnit::Milliseconds => 50.0,
        }
    }

    fn suffix(&self) -> &'static str {
        match self {
            StepperUnit::Decibels => "dB",
            StepperUnit::Milliseconds => "ms",
        }
    }
}

/// Format a numeric value with unit suffix, stripping trailing zeros but keeping
/// minimum 2 decimal places. Examples: -50.00 dB → "-50 dB", -14.25 dB stays, 1000.00 ms → "1000 ms".
fn format_stepper_value(value: f64, unit: &StepperUnit) -> String {
    let formatted = format!("{:.2}", value);
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    format!(
        "{} {}",
        if trimmed.is_empty() { "0" } else { trimmed },
        unit.suffix()
    )
}

#[derive(Debug, Clone, Default)]
struct NumberStepperState {
    is_hover_left: bool,
    is_hover_right: bool,
    is_input_editing: bool,
    input_value_text: String,
    input_cursor_pos: usize,
    input_has_focus: bool,
}

/// Custom stepper widget: 64×14 px with Unicode chevrons and manual numeric input (D-32).
/// Left/right chevrons step the value. Center zone shows formatted value + suffix;
/// click to enter edit mode. Enter/click-outside applies the raw parsed value
/// (never rounded to step — D-34). Escape cancels.
pub struct NumberStepper<'a, Message> {
    value: f64,
    range: std::ops::RangeInclusive<f64>,
    unit: StepperUnit,
    on_change: Box<dyn Fn(f64) -> Message + 'a>,
    on_selected_state_change: Option<Box<dyn Fn(bool) -> Message + 'a>>,
}

impl<'a, Message> NumberStepper<'a, Message> {
    /// Create a new NumberStepper. The value is clamped to range on construction.
    pub fn new(
        value: f64,
        range: std::ops::RangeInclusive<f64>,
        unit: StepperUnit,
        on_change: impl Fn(f64) -> Message + 'a,
    ) -> Self {
        Self {
            value: value.clamp(*range.start(), *range.end()),
            range,
            unit,
            on_change: Box::new(on_change),
            on_selected_state_change: None,
        }
    }

    /// Set callback for focus-gating state changes (D-35).
    pub fn on_selected_state_change(mut self, callback: impl Fn(bool) -> Message + 'a) -> Self {
        self.on_selected_state_change = Some(Box::new(callback));
        self
    }

    /// Apply a new value (clamped to range) and fire the on_change callback.
    fn commit_value(&self, v: f64, shell: &mut iced::advanced::Shell<'_, Message>) {
        let clamped = v.clamp(*self.range.start(), *self.range.end());
        shell.publish((self.on_change)(clamped));
    }
}

impl<'a, Message: 'a> iced::advanced::Widget<Message, Theme, iced::Renderer>
    for NumberStepper<'a, Message>
{
    fn size(&self) -> iced::Size<Length> {
        iced::Size {
            width: Length::Fixed(64.0),
            height: Length::Fixed(14.0),
        }
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(NumberStepperState::default())
    }

    fn layout(
        &mut self,
        _tree: &mut iced::advanced::widget::Tree,
        _renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let size = limits.resolve(Length::Fixed(64.0), Length::Fixed(14.0), iced::Size::ZERO);
        iced::advanced::layout::Node::new(size)
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        _viewport: &iced::Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<NumberStepperState>();
        let cursor_pos = cursor.position();
        let is_focus = cursor_pos.is_some_and(|p| bounds.contains(p));

        // Update hover state for chevron zones
        if let Some(pos) = cursor_pos {
            let left_zone = Rectangle {
                x: bounds.x,
                y: bounds.y,
                width: 14.0,
                height: 14.0,
            };
            let right_zone = Rectangle {
                x: bounds.x + 50.0,
                y: bounds.y,
                width: 14.0,
                height: 14.0,
            };
            state.is_hover_left = left_zone.contains(pos);
            state.is_hover_right = right_zone.contains(pos);
        } else {
            state.is_hover_left = false;
            state.is_hover_right = false;
        }

        // Determine click zones
        let center_zone = Rectangle {
            x: bounds.x + 14.0,
            y: bounds.y,
            width: 36.0,
            height: 14.0,
        };
        let left_zone = Rectangle {
            x: bounds.x,
            y: bounds.y,
            width: 14.0,
            height: 14.0,
        };
        let right_zone = Rectangle {
            x: bounds.x + 50.0,
            y: bounds.y,
            width: 14.0,
            height: 14.0,
        };
        let in_center = cursor_pos.is_some_and(|p| center_zone.contains(p));
        let in_left = cursor_pos.is_some_and(|p| left_zone.contains(p));
        let in_right = cursor_pos.is_some_and(|p| right_zone.contains(p));

        match event {
            // Click outside the stepper while editing → apply and exit
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
                if !is_focus && state.input_has_focus =>
            {
                // Apply current input value
                let stripped = strip_suffix(&state.input_value_text);
                if let Ok(v) = stripped.parse::<f64>() {
                    self.commit_value(v, shell);
                }
                state.is_input_editing = false;
                state.input_has_focus = false;
                GLOBAL_SLIDER_SELECTED.store(false, Ordering::Relaxed);
                if let Some(ref cb) = self.on_selected_state_change {
                    shell.publish(cb(false));
                }
            }

            // Mouse button press
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                if in_left {
                    // Step down
                    let new_val = self.value - self.unit.step();
                    self.value = new_val.clamp(*self.range.start(), *self.range.end());
                    shell.publish((self.on_change)(self.value));
                    state.is_input_editing = false;
                    state.input_has_focus = false;
                    GLOBAL_SLIDER_SELECTED.store(false, Ordering::Relaxed);
                } else if in_right {
                    // Step up
                    let new_val = self.value + self.unit.step();
                    self.value = new_val.clamp(*self.range.start(), *self.range.end());
                    shell.publish((self.on_change)(self.value));
                    state.is_input_editing = false;
                    state.input_has_focus = false;
                    GLOBAL_SLIDER_SELECTED.store(false, Ordering::Relaxed);
                } else if in_center {
                    // Enter edit mode
                    shell.capture_event();
                    state.input_value_text = format_stepper_value(self.value, &self.unit);
                    state.input_cursor_pos = state.input_value_text.len();
                    state.is_input_editing = true;
                    state.input_has_focus = true;
                    GLOBAL_SLIDER_SELECTED.store(true, Ordering::Relaxed);
                    if let Some(ref cb) = self.on_selected_state_change {
                        shell.publish(cb(true));
                    }
                }
            }

            // Keyboard input (when editing)
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. })
                if state.input_has_focus && state.is_input_editing =>
            {
                match key {
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => {
                        shell.capture_event();
                        let stripped = strip_suffix(&state.input_value_text);
                        if let Ok(v) = stripped.parse::<f64>() {
                            let clamped = v.clamp(*self.range.start(), *self.range.end());
                            self.value = clamped;
                            shell.publish((self.on_change)(clamped));
                        }
                        state.is_input_editing = false;
                        state.input_has_focus = false;
                        GLOBAL_SLIDER_SELECTED.store(false, Ordering::Relaxed);
                        if let Some(ref cb) = self.on_selected_state_change {
                            shell.publish(cb(false));
                        }
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                        shell.capture_event();
                        state.is_input_editing = false;
                        state.input_has_focus = false;
                        GLOBAL_SLIDER_SELECTED.store(false, Ordering::Relaxed);
                        if let Some(ref cb) = self.on_selected_state_change {
                            shell.publish(cb(false));
                        }
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp) => {
                        shell.capture_event();
                        let new_val = self.value + self.unit.step();
                        self.value = new_val.clamp(*self.range.start(), *self.range.end());
                        shell.publish((self.on_change)(self.value));
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown) => {
                        shell.capture_event();
                        let new_val = self.value - self.unit.step();
                        self.value = new_val.clamp(*self.range.start(), *self.range.end());
                        shell.publish((self.on_change)(self.value));
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft) => {
                        if state.input_cursor_pos > 0 {
                            state.input_cursor_pos -= 1;
                        }
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight) => {
                        if state.input_cursor_pos < state.input_value_text.len() {
                            state.input_cursor_pos += 1;
                        }
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
                        if state.input_cursor_pos > 0 && !state.input_value_text.is_empty() {
                            state.input_value_text.remove(state.input_cursor_pos - 1);
                            state.input_cursor_pos -= 1;
                        }
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete) => {
                        if state.input_cursor_pos < state.input_value_text.len() {
                            state.input_value_text.remove(state.input_cursor_pos);
                        }
                    }
                    iced::keyboard::Key::Character(c) => {
                        if state.input_value_text.len() < 20 {
                            let ch = c.chars().next().unwrap_or(' ');
                            if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch.is_whitespace() {
                                state.input_value_text.insert(state.input_cursor_pos, ch);
                                state.input_cursor_pos += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }

            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        _theme: &Theme,
        _style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        _viewport: &iced::Rectangle,
    ) {
        use iced::advanced::text::Renderer as _;
        use iced::advanced::Renderer as _;

        let bounds = layout.bounds();
        let state = tree.state.downcast_ref::<NumberStepperState>();

        let font_size: f32 = 12.0;
        let chevron_size: f32 = 12.0;
        let chevron_glyph_size: f32 = 10.0;

        // --- Left chevron zone ---
        let left_zone = Rectangle {
            x: bounds.x,
            y: bounds.y,
            width: 14.0,
            height: 14.0,
        };
        // Background (hover highlight)
        renderer.fill_quad(
            iced::advanced::graphics::core::renderer::Quad {
                bounds: left_zone,
                ..Default::default()
            },
            if state.is_hover_left {
                COLOR_CONTRAST
            } else {
                COLOR_BG
            },
        );
        // Chevron glyph: ◀ (U+25C0)
        let chevron_left_x = left_zone.x + (left_zone.width - chevron_glyph_size) / 2.0;
        let chevron_left_y = left_zone.y + (left_zone.height - chevron_glyph_size) / 2.0;
        renderer.fill_text(
            iced::advanced::text::Text {
                content: "\u{25C0}".to_string(),
                bounds: iced::Size::new(chevron_glyph_size, chevron_glyph_size),
                size: iced::Pixels(chevron_size),
                line_height: iced::advanced::text::LineHeight::Relative(1.0),
                font: FONT_INTER_SANS_MEDIUM,
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Top,
                shaping: iced::advanced::text::Shaping::Basic,
                wrapping: iced::advanced::text::Wrapping::None,
            },
            iced::Point::new(chevron_left_x, chevron_left_y),
            COLOR_TEXT_PRIMARY,
            left_zone,
        );

        // --- Right chevron zone ---
        let right_zone = Rectangle {
            x: bounds.x + 50.0,
            y: bounds.y,
            width: 14.0,
            height: 14.0,
        };
        renderer.fill_quad(
            iced::advanced::graphics::core::renderer::Quad {
                bounds: right_zone,
                ..Default::default()
            },
            if state.is_hover_right {
                COLOR_CONTRAST
            } else {
                COLOR_BG
            },
        );
        let chevron_right_x = right_zone.x + (right_zone.width - chevron_glyph_size) / 2.0;
        let chevron_right_y = right_zone.y + (right_zone.height - chevron_glyph_size) / 2.0;
        renderer.fill_text(
            iced::advanced::text::Text {
                content: "\u{25B6}".to_string(),
                bounds: iced::Size::new(chevron_glyph_size, chevron_glyph_size),
                size: iced::Pixels(chevron_size),
                line_height: iced::advanced::text::LineHeight::Relative(1.0),
                font: FONT_INTER_SANS_MEDIUM,
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Top,
                shaping: iced::advanced::text::Shaping::Basic,
                wrapping: iced::advanced::text::Wrapping::None,
            },
            iced::Point::new(chevron_right_x, chevron_right_y),
            COLOR_TEXT_PRIMARY,
            right_zone,
        );

        // --- Center value zone ---
        let center_zone = Rectangle {
            x: bounds.x + 14.0,
            y: bounds.y,
            width: 36.0,
            height: 14.0,
        };
        // Background for center zone
        renderer.fill_quad(
            iced::advanced::graphics::core::renderer::Quad {
                bounds: center_zone,
                ..Default::default()
            },
            COLOR_BG,
        );

        let display_text = if state.is_input_editing {
            // Show edit buffer with cursor
            let pos = state.input_cursor_pos.min(state.input_value_text.len());
            let (before, after) = state.input_value_text.split_at(pos);
            format!("{}|{}", before, after)
        } else {
            format_stepper_value(self.value, &self.unit)
        };

        // Center the text in the center zone
        let text_x = center_zone.x + center_zone.width / 2.0;
        let text_y = center_zone.y + (center_zone.height - font_size) / 2.0;

        renderer.fill_text(
            iced::advanced::text::Text {
                content: display_text,
                bounds: iced::Size::new(center_zone.width - 2.0, font_size),
                size: iced::Pixels(font_size),
                line_height: iced::advanced::text::LineHeight::Relative(1.0),
                font: FONT_INTER_SANS_MEDIUM,
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Top,
                shaping: iced::advanced::text::Shaping::Basic,
                wrapping: iced::advanced::text::Wrapping::None,
            },
            iced::Point::new(text_x, text_y),
            COLOR_TEXT_PRIMARY,
            center_zone,
        );
    }
}

impl<'a, Message: 'a> From<NumberStepper<'a, Message>> for Element<'a, Message> {
    fn from(stepper: NumberStepper<'a, Message>) -> Self {
        Element::new(stepper)
    }
}

// ==========================================
// 2. Elementos de Interfaz Auxiliares
// ==========================================

/// ======================================================================================
/// SISTEMA DE TRUNCAMIENTO DE TEXTO INTELIGENTE (RESPONSIVE)
/// ======================================================================================
///
/// Estas funciones proporcionan un widget de texto que se adapta dinámicamente al ancho real
/// de su contenedor, truncando con elipsis (...) cuando el espacio es insuficiente.
///
/// El sistema utiliza el componente `Responsive` de Iced para detectar el tamaño en tiempo real,
/// eliminando la necesidad de conteos de caracteres fijos que fallan al cambiar de fuente o ventana.

/// [NIVEL 1] Versión simplificada para el 90% de los casos.
/// - Sin saltos de línea (Wrapping::None)
/// - Sin sufijo adicional
/// - Alineación: Izquierda (Horizontal) y Centro (Vertical) por defecto.
///
/// Ejemplo: smart_truncate_text(song_title, 14.0, FONT, COLOR)
pub fn smart_truncate_text<'a, Message: Clone + 'a>(
    content: impl Into<String>,
    font_size: f32,
    font: iced::Font,
    color: Color,
) -> Element<'a, Message> {
    smart_truncate_text_advanced(
        content.into(),
        font_size,
        font,
        color,
        iced::widget::text::Wrapping::None,
        None,
        Alignment::Start,
        Alignment::Center,
    )
}

/// [NIVEL 2] Versión con soporte para sufijo pegado al texto.
/// - Ideal para cabeceras que llevan un punto indicador o icono pegado al nombre.
/// - El sufijo siempre permanecerá junto al texto, cortando el texto si es necesario.
/// - Sin saltos de línea (Wrapping::None)
/// - Alineación: Izquierda (Horizontal) y Centro (Vertical) por defecto.
///
/// Ejemplo: smart_truncate_text_with_suffix(name, 15.0, FONT, COLOR, (" •".to_string(), DOT_COLOR))
pub fn smart_truncate_text_with_suffix<'a, Message: Clone + 'a>(
    content: String,
    size: f32,
    font: iced::Font,
    color: Color,
    suffix: (String, Color),
) -> Element<'a, Message> {
    smart_truncate_text_advanced(
        content,
        size,
        font,
        color,
        iced::widget::text::Wrapping::None,
        Some(suffix),
        Alignment::Start,
        Alignment::Center,
    )
}

/// [NIVEL 3] Versión avanzada para control total.
/// Permite definir manualmente el comportamiento de wrapping, sufijos y alineación.
///
/// Alineaciones (Usando el enum `Alignment` de Iced):
/// - `align_x`: Alineación HORIZONTAL (Start = Izquierda, Center = Centro, End = Derecha).
/// - `align_y`: Alineación VERTICAL (Start = Arriba, Center = Centro, End = Abajo).
pub fn smart_truncate_text_advanced<'a, Message: Clone + 'a>(
    content: String,
    size: f32,
    font: iced::Font,
    color: Color,
    wrapping: iced::widget::text::Wrapping,
    suffix: Option<(String, Color)>,
    align_x: Alignment,
    align_y: Alignment,
) -> Element<'a, Message> {
    Responsive::new(move |size_info| {
        // Estimación: ~0.55 el tamaño de la fuente por carácter.
        let char_w = size * 0.55;
        let suffix_s = suffix.as_ref().map(|(s, _)| s.clone()).unwrap_or_default();
        let suffix_c = suffix.as_ref().map(|(_, c)| *c).unwrap_or(color);

        // Calculamos ancho de sufijo (incluyendo un margen de 2px)
        let suffix_w = if suffix_s.is_empty() {
            0.0
        } else {
            (suffix_s.chars().count() as f32) * char_w + 2.0
        };

        let available_w = size_info.width;
        let text_w_needed = (content.chars().count() as f32) * char_w;

        // Decidimos si truncar basándonos en el espacio real asignado por Iced
        let display_text = if (text_w_needed + suffix_w) <= available_w {
            content.clone()
        } else {
            let limit_w = (available_w - suffix_w).max(0.0);
            let max_chars = (limit_w / char_w).floor() as usize;
            truncate_text(&content, max_chars)
        };

        let mut r = row![text(display_text)
            .size(size)
            .font(font)
            .color(color)
            .wrapping(wrapping)
            .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(
                size + 2.0
            )))]
        .align_y(align_y)
        .spacing(0);

        if !suffix_s.is_empty() {
            r = r.push(
                text(suffix_s)
                    .size(size)
                    .font(font)
                    .color(suffix_c)
                    .wrapping(wrapping)
                    .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(
                        size + 2.0,
                    ))),
            );
        }

        container(r)
            .width(Length::Fill)
            .height(Length::Shrink)
            .align_x(align_x)
            .align_y(align_y)
            .into()
    })
    .width(Length::Fill)
    .height(Length::Shrink)
    .into()
}

// ==========================================
// 3. Widgets de Cabecera y Filas
// ==========================================

/// Envuelve un contenido y captura específicamente la rueda del scroll del mouse
/// devolviendo un valor de tipo delta que puede transformarse al Mensaje deseado.
pub struct VolumeScrollArea<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    on_scroll: Box<dyn Fn(f32) -> Message + 'a>,
}

impl<'a, Message, Theme, Renderer> VolumeScrollArea<'a, Message, Theme, Renderer> {
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        on_scroll: impl Fn(f32) -> Message + 'a,
    ) -> Self {
        Self {
            content: content.into(),
            on_scroll: Box::new(on_scroll),
        }
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for VolumeScrollArea<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        )
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content))
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
            if cursor.is_over(layout.bounds()) {
                let d = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 10.0,
                };
                shell.publish((self.on_scroll)(d));
                return;
            }
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        )
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<VolumeScrollArea<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(area: VolumeScrollArea<'a, Message, Theme, Renderer>) -> Self {
        Element::new(area)
    }
}

// ==========================================
// 2. Funciones Libres y Componentes UI Globales
// ==========================================

/// Crea un botón transparente que muestra un ícono SVG y emite un mensaje al presionarse.
/// Útil para la barra superior, cabeceras, o cualquier elemento "sin bordes".
pub fn action_icon_button<'a, Message: Clone + 'a>(
    icon_filename: &str,
    size: u32,
    action: Message,
) -> Element<'a, Message> {
    let content = svg(svg::Handle::from_path(format!(
        "assets/icons/{}",
        icon_filename
    )))
    .width(size)
    .height(size);

    mouse_area(content)
        .on_press(action)
        .interaction(iced::mouse::Interaction::Pointer)
        .into()
}

/// Botón icono SVG reutilizable con hover y estado deshabilitado.
/// - action: Some(msg) = botón activo con on_press. None = deshabilitado, sin interacción.
pub fn icon_button<'a, Message: Clone + 'a>(
    icon_filename: &str,
    size: u32,
    action: Option<Message>,
) -> Element<'a, Message> {
    let has_action = action.is_some();
    let btn_size = (size + 4).max(26);
    let icon = svg(svg::Handle::from_path(format!(
        "assets/icons/{}",
        icon_filename
    )))
    .width(iced::Length::Fill)
    .height(iced::Length::Fill)
    .style(|_t: &Theme, status| svg::Style {
        color: Some(match status {
            svg::Status::Hovered => COLOR_TEXT_PRIMARY,
            _ => COLOR_TEXT_SECONDARY,
        }),
    });

    let btn = button(icon)
        .width(btn_size)
        .height(btn_size)
        .padding(0)
        .style(move |_t: &Theme, status| {
            let is_hovered = matches!(status, button::Status::Hovered);
            button::Style {
                background: if has_action && is_hovered {
                    Some(COLOR_ACCENT.into())
                } else {
                    Some(Color::TRANSPARENT.into())
                },
                text_color: COLOR_TEXT_PRIMARY,
                border: iced::Border {
                    radius: 6.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..Default::default()
            }
        });

    if let Some(msg) = action {
        btn.on_press(msg).into()
    } else {
        btn.into()
    }
}

/// Interruptor (Toggler) global de diseño premium personalizado.
/// - Permite personalizar el tamaño, colores de fondo y círculo para estados activo e inactivo.
pub fn standard_toggler<'a, Message>(
    is_active: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
    size: f32,
    active_color: Color,
    inactive_color: Color,
    thumb_active_color: Color,
    thumb_inactive_color: Color,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    standard_toggler_full(
        is_active,
        on_toggle,
        size,
        active_color,
        inactive_color,
        thumb_active_color,
        thumb_inactive_color,
        Color::TRANSPARENT,
    )
}

/// Toggler con control total de colores, incluyendo fondo del estado inactivo.
/// - `inactive_bg_color`: Color de fondo cuando está desactivado (por defecto TRANSPARENT).
pub fn standard_toggler_full<'a, Message>(
    is_active: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
    size: f32,
    active_color: Color,
    inactive_color: Color,
    thumb_active_color: Color,
    thumb_inactive_color: Color,
    inactive_bg_color: Color,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    toggler(is_active)
        .size(size)
        .on_toggle(on_toggle)
        .style(move |_theme: &Theme, _status| {
            let (bg, border_color, border_width, fg) = if is_active {
                (
                    active_color.into(),
                    active_color,
                    1.0,
                    thumb_active_color.into(),
                )
            } else {
                (
                    inactive_bg_color.into(),
                    inactive_color,
                    1.0,
                    thumb_inactive_color.into(),
                )
            };

            iced::widget::toggler::Style {
                background: bg,
                background_border_width: border_width,
                background_border_color: border_color,
                foreground: fg,
                foreground_border_width: 0.0,
                foreground_border_color: Color::TRANSPARENT,
                text_color: None,
                border_radius: None,
                padding_ratio: 0.2,
            }
        })
        .into()
}

/// Botón de radio (Radio Button) global de diseño premium personalizado.
/// - Desactivados: Borde en COLOR_TEXT_PRIMARY y fondo transparente.
/// - Activados: Borde en COLOR_ACCENT y el círculo (dot) en COLOR_TEXT_SECONDARY.
pub fn standard_radio<'a, Message, V>(
    label: impl Into<String>,
    value: V,
    selected: Option<V>,
    on_click: impl Fn(V) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
    V: Copy + Eq + 'a,
{
    let is_selected = selected == Some(value);

    radio(label, value, selected, on_click)
        .size(16)
        .text_size(14)
        .font(FONT_INTER_SANS_MEDIUM)
        .style(move |_theme: &Theme, _status| {
            let (border_color, dot_color) = if is_selected {
                (COLOR_ACCENT, COLOR_ACCENT)
            } else {
                (COLOR_TEXT_PRIMARY, Color::TRANSPARENT)
            };

            iced::widget::radio::Style {
                background: Color::TRANSPARENT.into(),
                dot_color,
                border_width: 1.3,
                border_color,
                text_color: Some(COLOR_TEXT_PRIMARY),
            }
        })
        .into()
}

/// Desplegable (PickList) global de diseño premium personalizado.
/// - 24px de alto.
/// - Fondo en COLOR_CONTRAST.
/// - Tipografía de 14px y fuente Inter Medium.
/// - Sin bordes.
/// - Hover/Focus en COLOR_ACCENT con texto COLOR_TEXT_PRIMARY.
pub fn standard_pick_list<'a, T, Message>(
    options: Vec<T>,
    selected: Option<T>,
    on_selected: impl Fn(T) -> Message + 'a,
    width: Length,
) -> Element<'a, Message>
where
    T: Clone + PartialEq + std::fmt::Display + 'a,
    Message: Clone + 'a,
{
    let pick = pick_list(options, selected, on_selected)
        .width(width)
        .padding([4, 10]) // Padding vertical mínimo para que tenga 24px de alto con fuente de 14px
        .font(FONT_INTER_SANS_MEDIUM)
        .text_size(14)
        .style(|_theme: &Theme, status| {
            let is_active_hover = match status {
                iced::widget::pick_list::Status::Hovered
                | iced::widget::pick_list::Status::Opened { .. } => true,
                _ => false,
            };

            let bg_color = if is_active_hover {
                COLOR_ACCENT
            } else {
                COLOR_CONTRAST
            };

            let txt_color = if is_active_hover {
                COLOR_TEXT_PRIMARY
            } else {
                COLOR_TEXT_PRIMARY
            };

            iced::widget::pick_list::Style {
                text_color: txt_color,
                placeholder_color: COLOR_TEXT_PRIMARY,
                handle_color: txt_color,
                background: bg_color.into(),
                border: iced::Border {
                    width: 0.0,
                    color: Color::TRANSPARENT,
                    radius: 0.0.into(),
                },
            }
        })
        .menu_style(|_theme: &Theme| iced::overlay::menu::Style {
            background: COLOR_CONTRAST.into(),
            border: iced::Border {
                width: 0.0,
                color: Color::TRANSPARENT,
                radius: 0.0.into(),
            },
            text_color: COLOR_TEXT_PRIMARY,
            selected_text_color: COLOR_TEXT_PRIMARY,
            selected_background: COLOR_ACCENT.into(),
            shadow: iced::Shadow::default(),
        });

    pick.into()
}

/// Estilos de Placeholder para el widget de carátulas
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaceholderStyle {
    /// Versión grande (Grid, Lista Detallada): Icono 96px + Texto "AuDoxiDY"
    Large,
    /// Versión pequeña (Thumbnail List): Solo icono
    Small,
    /// Versión específica del reproductor (Solo texto)
    Player,
}

/// Widget global para mostrar carátulas de álbumes de forma eficiente y consistente.
/// Soporta: Caché AVIF (Prioridad 1) -> Datos binarios crudos (Prioridad 2) -> Placeholder Automático.
pub fn album_art_widget<'a, Message: 'a>(
    avif_path: Option<&str>,
    raw_data: Option<&Vec<u8>>,
    preloaded_handle: Option<iced::widget::image::Handle>,
    style: PlaceholderStyle,
    bounds: Length,
    radius: f32,
) -> Element<'a, Message> {
    // 1. Usar handle precargado si existe (Prioridad 0 - Máximo rendimiento)
    let mut handle_opt = preloaded_handle;

    // 2. Intentar cargar desde archivo AVIF en disco (Prioridad 1) — sin caché RAM
    if handle_opt.is_none() {
        handle_opt = avif_path.and_then(|p| crate::utils::covers::load_cover_handle(p));
    }

    // 3. Fallback: Cargar desde bytes crudos (Prioridad 2)
    if handle_opt.is_none() {
        if let Some(data) = raw_data {
            handle_opt = crate::utils::covers::load_raw_image_for_iced(data);
        }
    }

    if let Some(handle) = handle_opt {
        container(
            iced::widget::image(handle)
                .width(bounds)
                .height(bounds)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(radius),
        )
        .width(bounds)
        .height(bounds)
        .style(move |_t: &Theme| {
            container::Style::default().border(iced::Border {
                radius: radius.into(),
                ..Default::default()
            })
        })
        .clip(true)
        .into()
    } else {
        // 4. Fallback: Placeholder Automático basado en el estilo solicitado
        match style {
            PlaceholderStyle::Large => container(
                column![
                    svg(svg::Handle::from_path("assets/icons/album.svg"))
                        .width(Length::Fixed(96.0))
                        .height(Length::Fixed(96.0))
                        .style(|_t: &Theme, _s| svg::Style {
                            color: Some(Color::from(COLOR_TEXT_SECONDARY))
                        }),
                    text("AuDoxiDY")
                        .font(crate::gui::theme::FONT_STAGE_WANDER)
                        .size(11)
                        .color(COLOR_TEXT_SECONDARY)
                ]
                .align_x(Alignment::Center)
                .spacing(5),
            )
            .width(bounds)
            .height(bounds)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(move |_t: &Theme| {
                container::Style::default()
                    .background(COLOR_BG)
                    .border(iced::Border {
                        radius: radius.into(),
                        ..Default::default()
                    })
            })
            .into(),
            PlaceholderStyle::Small => container(
                svg(svg::Handle::from_path("assets/icons/album.svg"))
                    .width(Length::Fixed(20.0))
                    .height(Length::Fixed(20.0))
                    .style(|_t: &Theme, _s| svg::Style {
                        color: Some(Color::from(COLOR_TEXT_SECONDARY)),
                    }),
            )
            .width(bounds)
            .height(bounds)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(move |_t: &Theme| {
                container::Style::default()
                    .background(COLOR_BG)
                    .border(iced::Border {
                        radius: radius.into(),
                        ..Default::default()
                    })
            })
            .into(),
            PlaceholderStyle::Player => container(
                text("AuDoxiDY")
                    .font(crate::gui::theme::FONT_STAGE_WANDER)
                    .size(40)
                    .color(COLOR_TEXT_PRIMARY),
            )
            .width(bounds)
            .height(bounds)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .padding(20)
            .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
            .into(),
        }
    }
}

pub fn custom_scrollbar_style(
    _theme: &Theme,
    status: iced::widget::scrollable::Status,
) -> iced::widget::scrollable::Style {
    let color = match status {
        iced::widget::scrollable::Status::Hovered {
            is_vertical_scrollbar_hovered,
            is_horizontal_scrollbar_hovered,
            ..
        } => {
            if is_vertical_scrollbar_hovered || is_horizontal_scrollbar_hovered {
                Color::from(COLOR_TEXT_PRIMARY)
            } else {
                Color::from(COLOR_TEXT_SECONDARY)
            }
        }
        iced::widget::scrollable::Status::Dragged { .. } => Color::from(COLOR_ACCENT),
        _ => Color::TRANSPARENT,
    };

    iced::widget::scrollable::Style {
        container: iced::widget::container::Style::default(),
        vertical_rail: iced::widget::scrollable::Rail {
            background: None,
            border: iced::Border::default(),
            scroller: iced::widget::scrollable::Scroller {
                background: if color != Color::TRANSPARENT {
                    iced::Background::Gradient(iced::Gradient::Linear(iced::gradient::Linear {
                        angle: 1.5707964.into(), // 90 grados (Horizontal)
                        stops: [
                            Some(iced::gradient::ColorStop {
                                offset: 0.49,
                                color: Color::TRANSPARENT,
                            }),
                            Some(iced::gradient::ColorStop {
                                offset: 0.5,
                                color: color,
                            }),
                            Some(iced::gradient::ColorStop {
                                offset: 1.0,
                                color: color,
                            }),
                            None,
                            None,
                            None,
                            None,
                            None,
                        ],
                    }))
                } else {
                    Color::TRANSPARENT.into()
                },
                border: iced::Border {
                    radius: 0.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
            },
        },
        horizontal_rail: iced::widget::scrollable::Rail {
            background: None,
            border: iced::Border::default(),
            scroller: iced::widget::scrollable::Scroller {
                background: if color != Color::TRANSPARENT {
                    iced::Background::Gradient(iced::Gradient::Linear(iced::gradient::Linear {
                        angle: 3.1415927.into(), // 180 grados (Vertical)
                        stops: [
                            Some(iced::gradient::ColorStop {
                                offset: 0.49,
                                color: Color::TRANSPARENT,
                            }),
                            Some(iced::gradient::ColorStop {
                                offset: 0.5,
                                color: color,
                            }),
                            Some(iced::gradient::ColorStop {
                                offset: 1.0,
                                color: color,
                            }),
                            None,
                            None,
                            None,
                            None,
                            None,
                        ],
                    }))
                } else {
                    Color::TRANSPARENT.into()
                },
                border: iced::Border {
                    radius: 0.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
            },
        },
        gap: None,
        auto_scroll: iced::widget::scrollable::AutoScroll {
            background: Color::TRANSPARENT.into(),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            icon: Color::TRANSPARENT,
        },
    }
}

/// Widget scrollable estándar con barra de desplazamiento de 12px (4px visibles + 4px de margen interactivo transparente a cada lado).
pub fn standard_scrollable<'a, Message: 'a>(
    id: iced::widget::Id,
    content: impl Into<Element<'a, Message>>,
    direction: iced::widget::scrollable::Direction,
) -> iced::widget::scrollable::Scrollable<'a, Message> {
    scrollable(content)
        .id(id)
        .direction(direction)
        .style(custom_scrollbar_style)
}

/// Configuración estándar de la barra de desplazamiento para uso global.
pub fn standard_scrollbar() -> iced::widget::scrollable::Scrollbar {
    iced::widget::scrollable::Scrollbar::new()
        .width(8)
        .scroller_width(8)
        .margin(0)
}

/// Aplica un efecto de rotación tipo "Marquesina" sobre una cadena de texto dada una longitud límite.
/// Devuelve el string recortado o rotado en función de un "tick" temporal global.
pub fn apply_marquee(text_str: &str, limit: usize, tick: u64) -> String {
    // Optimización rápida: si es corto, no procesar nada
    if text_str.len() <= limit && text_str.is_ascii() {
        return text_str.to_string();
    }

    let chars: Vec<char> = text_str.chars().collect();
    if chars.len() <= limit {
        return text_str.to_string();
    }

    // Suavizamos el movimiento dividiendo el tick
    let offset = (tick / 3) as usize % (chars.len() + 10);
    if offset < chars.len() {
        let end = (offset + limit).min(chars.len());
        let mut s: String = chars[offset..end].iter().collect();
        // Si llegamos al final, mostramos la separación invisible y empalmamos el inicio
        if offset + limit > chars.len() {
            s.push_str("   ");
            let needed = (offset + limit) - chars.len();
            if needed > 3 {
                let rem = needed - 3;
                s.push_str(&chars[0..rem.min(chars.len())].iter().collect::<String>());
            }
        }
        s
    } else {
        chars[0..limit.min(chars.len())].iter().collect()
    }
}

/// Construye una barra de ordenamiento interactiva, reciclable para cualquier tabla.
pub fn build_sort_bar<'a, Message: Clone + 'a>(
    columns: &[SortColumn],
    current_sort: Option<SortColumn>,
    sort_ascending: Option<bool>,
    column_widths: &HashMap<SortColumn, u16>,
    resizing_column: Option<SortColumn>,
    hovered_column: Option<SortColumn>,
    left_padding: f32,
    on_hover: impl Fn(Option<SortColumn>) -> Message + 'a,
    on_resize: impl Fn(SortColumn) -> Message + 'a,
    on_sort: impl Fn(SortColumn) -> Message + 'a,
) -> Element<'a, Message> {
    let mut sort_bar_content = row![]
        .align_y(Alignment::Center)
        .height(Length::Fill)
        .padding(iced::Padding {
            top: 0.0,
            right: 5.0,
            bottom: 0.0,
            left: left_padding,
        });

    for &sort in columns {
        let is_active = current_sort == Some(sort);
        let width = *column_widths.get(&sort).unwrap_or(&100) as f32;

        let available_w = width - 15.0; // Espacio reservado para icono/separador
        let max_chars = (available_w / 7.0).max(1.0) as usize;
        let t_str = truncate_text(sort.as_str(), max_chars);

        let t = text(t_str)
            .size(12)
            .font(FONT_INTER_SANS_MEDIUM)
            .color(COLOR_TEXT_SECONDARY)
            .wrapping(iced::widget::text::Wrapping::None);

        let icon_el = if is_active {
            let handle = match sort_ascending {
                Some(true) => Some(svg::Handle::from_path("assets/icons/arrow-up-chevron.svg")),
                Some(false) => Some(svg::Handle::from_path(
                    "assets/icons/arrow-down-chevron.svg",
                )),
                _ => None,
            };

            if let Some(h) = handle {
                Some(
                    svg(h)
                        .width(20)
                        .height(20)
                        .style(move |_t: &Theme, _s: svg::Status| svg::Style {
                            color: Some(COLOR_TEXT_SECONDARY),
                        }),
                )
            } else {
                None
            }
        } else {
            None
        };
        let is_hovered = resizing_column == Some(sort) || hovered_column == Some(sort);

        let separator_visual = container(Space::new())
            .width(Length::Fixed(9.0))
            .height(Length::Fixed(16.0))
            .style(move |_t: &Theme| {
                let bg_color = if is_hovered {
                    COLOR_ACCENT
                } else {
                    COLOR_TEXT_SECONDARY
                };
                container::Style::default()
                    .background(bg_color)
                    .border(iced::Border {
                        radius: 4.0.into(),
                        width: 4.0,
                        color: Color::TRANSPARENT,
                    })
            });

        let on_h = on_hover(Some(sort));
        let on_h_exit = on_hover(None);
        let separator_area: Element<Message> =
            if sort == SortColumn::AlbumCard || sort == SortColumn::AlbumThumbnail {
                Space::new().width(4.0).into()
            } else {
                mouse_area(separator_visual)
                    .on_enter(on_h)
                    .on_exit(on_h_exit)
                    .on_press(on_resize(sort))
                    .interaction(iced::mouse::Interaction::ResizingHorizontally)
                    .into()
            };

        let sort_btn_content = if sort == SortColumn::TrackNumber {
            if let Some(ic) = icon_el {
                row![
                    Space::new().width(Length::Fill),
                    t,
                    ic,
                    Space::new().width(3.0)
                ]
                .align_y(Alignment::Center)
            } else {
                row![Space::new().width(Length::Fill), t, Space::new().width(3.0)]
                    .align_y(Alignment::Center)
            }
        } else {
            if let Some(ic) = icon_el {
                row![t, Space::new().width(Length::Fill), ic].align_y(Alignment::Center)
            } else {
                row![t, Space::new().width(Length::Fill)].align_y(Alignment::Center)
            }
        };

        let sort_btn = if sort == SortColumn::AlbumCard || sort == SortColumn::AlbumThumbnail {
            button(sort_btn_content)
                .width(Length::Fill)
                .padding(iced::Padding {
                    left: 5.0,
                    right: 0.0,
                    top: 0.0,
                    bottom: 0.0,
                })
                .style(|_t: &Theme, _s| {
                    button::Style::default().with_background(Color::TRANSPARENT)
                })
        } else {
            button(sort_btn_content)
                .width(Length::Fill)
                .padding(iced::Padding {
                    left: 5.0,
                    right: 0.0,
                    top: 0.0,
                    bottom: 0.0,
                })
                .style(|_t: &Theme, _s| {
                    button::Style::default().with_background(Color::TRANSPARENT)
                })
                .on_press(on_sort(sort))
        };

        let content = row![
            sort_btn,
            Space::new().width(3.0),
            separator_area,
            Space::new().width(3.0)
        ]
        .align_y(Alignment::Center)
        .width(Length::Fixed(width));

        let col_container = container(content).width(Length::Fixed(width)).clip(true);

        sort_bar_content = sort_bar_content.push(col_container);
    }

    column![
        container(
            scrollable(sort_bar_content)
                .direction(iced::widget::scrollable::Direction::Horizontal(
                    iced::widget::scrollable::Scrollbar::new().width(0).scroller_width(0)
                ))
        )
            .width(Length::Fill)
            .height(Length::Fixed(28.0))
            .style(|_t: &Theme| container::Style::default().background(COLOR_BG)),
        container(Space::new().width(Length::Fill).height(2.0))
            .style(|_t: &Theme| container::Style::default().background(Color::from(COLOR_CONTRAST)))
    ]
    .into()
}

// ==========================================
// 4. Widgets de Entrada y Búsqueda
// ==========================================

/// Widget de búsqueda estandarizado para toda la aplicación.
pub fn standard_search_input<'a, Message: Clone + 'a>(
    id: Option<iced::widget::Id>,
    placeholder: &'a str,
    value: &'a str,
    on_change: impl Fn(String) -> Message + 'a,
    on_clear: Message,
    width: Length,
) -> Element<'a, Message> {
    let has_content = !value.is_empty();

    let mut input = text_input(placeholder, value)
        .on_input(on_change)
        .padding(iced::Padding {
            right: 25.0,
            ..Default::default()
        }) // Espacio a la derecha para la 'x'
        .size(14)
        .font(FONT_INTER_SANS_MEDIUM)
        .width(Length::Fill)
        .style(
            move |_t: &Theme, status: iced::widget::text_input::Status| {
                let is_focused = matches!(status, iced::widget::text_input::Status::Focused { .. });

                let (bg, txt) = if is_focused {
                    (COLOR_CONTRAST, COLOR_TEXT_PRIMARY)
                } else {
                    (COLOR_CONTRAST, COLOR_TEXT_SECONDARY)
                };

                iced::widget::text_input::Style {
                    background: bg.into(),
                    border: iced::Border {
                        radius: 0.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    icon: COLOR_TEXT_SECONDARY,
                    placeholder: COLOR_TEXT_SECONDARY,
                    value: txt,
                    selection: COLOR_ACCENT,
                }
            },
        );

    if let Some(actual_id) = id {
        input = input.id(actual_id);
    }

    let mut content = iced::widget::stack![input];

    if has_content {
        let clear_btn = button(
            container(
                text("x")
                    .size(14)
                    .color(COLOR_TEXT_PRIMARY)
                    .font(FONT_INTER_SANS_MEDIUM),
            )
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(20.0))
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
        )
        .padding(0)
        .style(|_t, _s| button::Style::default().with_background(Color::TRANSPARENT))
        .on_press(on_clear);

        content = content.push(
            container(clear_btn)
                .width(Length::Fill)
                .height(Length::Fixed(24.0))
                .align_x(iced::alignment::Horizontal::Right)
                .padding(iced::Padding {
                    right: 5.0,
                    ..Default::default()
                }),
        );
    }

    container(content)
        .width(width)
        .height(Length::Fixed(24.0))
        .center_y(Length::Fill)
        .into()
}

// ==========================================
// 5. Menús Contextuales
// ==========================================

/// Representa una entrada en un menú contextual.
#[derive(Debug, Clone)]
pub struct ContextMenuEntry<Message> {
    pub label: String,
    pub icon: Option<String>,    // Nombre del icono en assets/icons/
    pub action: Option<Message>, // None representa un divisor
}

/// Construye el contenido visual de un menú contextual basado en una lista de entradas.
pub fn build_context_menu_content<'a, Message: Clone + 'a>(
    entries: Vec<ContextMenuEntry<Message>>,
) -> Element<'a, Message> {
    let max_chars = entries
        .iter()
        .map(|e| e.label.chars().count())
        .max()
        .unwrap_or(0);
    let calculated_width = (max_chars as f32 * 7.5) + 52.0;
    let final_width = calculated_width.max(80.0);

    let mut content = column![].spacing(0).width(Length::Fixed(final_width));

    for entry in entries {
        if let Some(action) = entry.action {
            let label = entry.label.clone();
            let icon_name = entry.icon.clone();

            let is_rename = label == "Renombrar lista";
            let left_padding = if is_rename { 9.0 } else { 10.0 };
            let row_spacing = if is_rename { 16.0 } else { 15.0 };

            let btn = button(
                container(
                    row![
                        // Icono
                        if let Some(icon) = icon_name {
                            Element::from(
                                container(
                                    svg(svg::Handle::from_path(format!("assets/icons/{}", icon)))
                                        .width(Length::Fixed(24.0))
                                        .height(Length::Fixed(24.0))
                                        .style(|_t, _s| iced::widget::svg::Style {
                                            color: Some(COLOR_TEXT_PRIMARY),
                                        }),
                                )
                                .width(Length::Fixed(24.0)),
                            )
                        } else {
                            Element::from(Space::new().width(Length::Fixed(24.0)))
                        },
                        Space::new().width(Length::Fixed(row_spacing)),
                        // Texto
                        text(label)
                            .size(13)
                            .font(FONT_INTER_SANS_NORMAL)
                            .wrapping(iced::widget::text::Wrapping::None)
                    ]
                    .align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .height(Length::Fixed(32.0))
                .padding(iced::Padding {
                    left: left_padding,
                    right: 0.0,
                    ..Default::default()
                })
                .align_y(iced::alignment::Vertical::Center),
            )
            .on_press(action)
            .padding(0)
            .style(move |_t: &Theme, status: iced::widget::button::Status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);

                button::Style {
                    background: if is_hovered {
                        Some(COLOR_CONTRAST.into())
                    } else {
                        None
                    },
                    text_color: COLOR_TEXT_PRIMARY,
                    border: iced::Border {
                        radius: 0.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                }
            });

            content = content.push(btn);
        } else {
            // Divisor
            content = content.push(
                container(Space::new().height(Length::Fixed(1.0)))
                    .width(Length::Fill)
                    .padding(iced::Padding {
                        top: 0.0,
                        bottom: 0.0,
                        ..Default::default()
                    })
                    .style(|_t: &Theme| {
                        container::Style::default().background(COLOR_TEXT_SECONDARY)
                    }),
            );
        }
    }

    container(column![content].spacing(0))
        .width(Length::Fixed(final_width))
        .style(|_t: &Theme| {
            container::Style::default()
                .background(COLOR_BG)
                .border(iced::Border {
                    color: COLOR_TEXT_SECONDARY,
                    width: 1.0,
                    radius: 8.0.into(),
                })
                .shadow(iced::Shadow {
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 10.0,
                    color: Color::from_rgba8(0, 0, 0, 0.5),
                })
        })
        .padding(iced::Padding {
            top: 5.0,
            bottom: 5.0,
            left: 1.0,
            right: 1.0,
        })
        .into()
}

/// Widget para diálogos modales (sub-ventanas).
pub fn standard_modal<'a, Message: Clone + 'a>(
    title: String,
    content: Element<'a, Message>,
    cancel_msg: Option<Message>,
    confirm_msg: Option<Message>,
    confirm_label: String,
) -> Element<'a, Message> {
    let mut footer = row![]
        .spacing(10)
        .padding(iced::Padding {
            top: 5.0,
            ..Default::default()
        })
        .align_y(Alignment::Center);

    if let Some(cancel) = cancel_msg {
        footer = footer.push(
            button(text("Cancelar").size(14).font(FONT_INTER_SANS_MEDIUM))
                .padding([5, 10])
                .on_press(cancel)
                .style(|_t: &Theme, status: iced::widget::button::Status| {
                    let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                    button::Style {
                        background: if is_hovered {
                            Some(COLOR_ACCENT.into())
                        } else {
                            Some(COLOR_CONTRAST.into())
                        },
                        text_color: if is_hovered {
                            COLOR_TEXT_PRIMARY
                        } else {
                            COLOR_TEXT_SECONDARY
                        },
                        border: iced::Border {
                            radius: 6.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        ..Default::default()
                    }
                }),
        );
    }

    if let Some(confirm) = confirm_msg {
        footer = footer.push(
            button(text(confirm_label).size(14).font(FONT_INTER_SANS_MEDIUM))
                .padding([5, 10])
                .on_press(confirm)
                .style(|_t: &Theme, status: iced::widget::button::Status| {
                    let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                    button::Style {
                        background: if is_hovered {
                            Some(COLOR_ACCENT.into())
                        } else {
                            Some(COLOR_CONTRAST.into())
                        },
                        text_color: if is_hovered {
                            COLOR_TEXT_PRIMARY
                        } else {
                            COLOR_TEXT_SECONDARY
                        },
                        border: iced::Border {
                            radius: 6.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        ..Default::default()
                    }
                }),
        );
    }

    container(
        column![
            Space::new()
                .width(Length::Fixed(200.0))
                .height(Length::Fixed(0.0)),
            text(title)
                .size(16)
                .font(FONT_INTER_SANS_MEDIUM)
                .color(COLOR_TEXT_PRIMARY)
                .align_x(iced::alignment::Horizontal::Center),
            container(content).padding([10, 0]).width(Length::Shrink),
            footer,
        ]
        .spacing(0)
        .align_x(Alignment::Center)
        .width(Length::Shrink),
    )
    .padding(15)
    .width(Length::Shrink)
    .style(|_t: &Theme| container::Style {
        background: Some(COLOR_BG.into()),
        border: iced::Border {
            color: COLOR_ACCENT,
            width: 2.0,
            radius: 8.0.into(),
        },
        shadow: iced::Shadow {
            offset: iced::Vector::new(0.0, 10.0),
            blur_radius: 30.0,
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.8),
        },
        ..container::Style::default()
    })
    .into()
}

/// Helper para texto dentro de modales con estilo global.
pub fn modal_text<'a, Message: Clone + 'a>(content: String) -> Element<'a, Message> {
    text(content)
        .size(14)
        .color(COLOR_TEXT_PRIMARY)
        .font(FONT_INTER_SANS_NORMAL)
        .align_x(iced::alignment::Horizontal::Center)
        .into()
}

// ==========================================
// 6. Lógica de Virtualización y Scroll
// ==========================================

/// Botón circular pequeño para cheurones (flechas) de expansión.
pub fn chevron_btn<'a, Message: Clone + 'a>(
    icon_filename: &str,
    action: Message,
    btn_size: f32,
    icon_size: f32,
) -> Element<'a, Message> {
    button(
        container(
            svg(svg::Handle::from_path(format!(
                "assets/icons/{}",
                icon_filename
            )))
            .width(icon_size)
            .height(icon_size)
            .style(move |_t: &Theme, _s: svg::Status| svg::Style {
                color: Some(COLOR_TEXT_SECONDARY),
            }),
        )
        .width(btn_size)
        .height(btn_size)
        .center_x(btn_size)
        .center_y(btn_size),
    )
    .on_press(action)
    .padding(0)
    .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
    .into()
}

/// Renderiza la cabecera de un grupo de artistas (Gris oscuro, 32px).
pub fn artist_header_widget<'a, Message: Clone + 'a>(
    name: String,
    is_collapsed: bool,
    is_selected: bool,
    is_playing: bool,
    albums_count: usize,
    songs_count: usize,
    duration_secs: f64,
    row_h: f32, // Altura de la fila: 32px para SimpleList/DetailedList, 42px para ThumbnailList
    on_select: Message,
    on_toggle: Message,
) -> Element<'a, Message> {
    let chevron = if is_collapsed {
        "arrow-down-chevron.svg"
    } else {
        "arrow-up-chevron.svg"
    };
    let time_str = format_duration(duration_secs);

    let (name_color, dot_color, show_dot) = if is_selected {
        (COLOR_TEXT_PRIMARY, COLOR_TEXT_PRIMARY, true)
    } else if is_playing {
        (COLOR_ACCENT, COLOR_ACCENT, true)
    } else {
        (COLOR_TEXT_PRIMARY, COLOR_TEXT_PRIMARY, false)
    };

    let on_select_clone = on_select.clone();
    let on_toggle_clone = on_toggle.clone();

    let font_size = if row_h > 40.0 { 16.0 } else { 15.0 };

    let name_widget = if show_dot {
        smart_truncate_text_with_suffix(
            name,
            font_size,
            FONT_INTER_SANS_MEDIUM,
            name_color,
            (" •".to_string(), dot_color),
        )
    } else {
        smart_truncate_text(name, font_size, FONT_INTER_SANS_MEDIUM, name_color)
    };

    let header_content = row![
        name_widget,
        container(
            text(format!(
                "{} Canciones | {} Álbumes | {}",
                songs_count, albums_count, time_str
            ))
            .size(14)
            .color(COLOR_TEXT_SECONDARY)
            .font(FONT_INTER_SANS_MEDIUM)
            .wrapping(iced::widget::text::Wrapping::None)
        )
        .padding(Padding {
            left: 10.0,
            right: 0.0,
            top: 0.0,
            bottom: 0.0
        })
        .center_y(Length::Fill),
        Space::new().width(15),
        chevron_btn(chevron, on_toggle_clone, row_h, row_h - 4.0),
    ]
    .align_y(Alignment::Center)
    .padding(Padding {
        left: 15.0,
        right: 6.0,
        top: 0.0,
        bottom: 0.0,
    })
    .height(Length::Fill);

    container(mouse_area(header_content).on_press(on_select_clone))
        .width(Length::Fill)
        .height(Length::Fixed(row_h))
        .center_y(Length::Fill)
        .style(|_t| container::Style::default().background(COLOR_CONTRAST))
        .into()
}

/// Renderiza la cabecera de un grupo de álbumes dentro de un artista.
pub fn album_header_widget<'a, Message: Clone + 'a>(
    album_name: String,
    songs_count: usize,
    duration_secs: f64,
    is_expanded: bool,
    is_selected: bool,
    is_playing: bool,
    row_h: f32,
    on_select: Message,
    on_toggle: Message,
) -> Element<'a, Message> {
    let chevron = if is_expanded {
        "arrow-up-chevron.svg"
    } else {
        "arrow-down-chevron.svg"
    };
    let time_str = format_duration(duration_secs);

    let (txt_color, dot_color, show_dot) = if is_selected {
        (COLOR_TEXT_PRIMARY, COLOR_TEXT_PRIMARY, true)
    } else if is_playing {
        (COLOR_ACCENT, COLOR_ACCENT, true)
    } else {
        (COLOR_TEXT_PRIMARY, COLOR_TEXT_PRIMARY, false)
    };

    let on_select_clone = on_select.clone();
    let on_toggle_clone = on_toggle.clone();

    let name_widget = if show_dot {
        smart_truncate_text_with_suffix(
            album_name,
            15.0,
            FONT_INTER_SANS_MEDIUM,
            txt_color,
            (" •".to_string(), dot_color),
        )
    } else {
        smart_truncate_text(album_name, 15.0, FONT_INTER_SANS_MEDIUM, txt_color)
    };

    let header_content = row![
        name_widget,
        container(
            text(format!("{} Canciones | {}", songs_count, time_str))
                .size(14)
                .color(COLOR_TEXT_SECONDARY)
                .font(FONT_INTER_SANS_MEDIUM)
                .wrapping(iced::widget::text::Wrapping::None)
        )
        .padding(Padding {
            left: 10.0,
            right: 0.0,
            top: 0.0,
            bottom: 0.0
        })
        .center_y(Length::Fill),
        Space::new().width(15),
        chevron_btn(chevron, on_toggle_clone, row_h, row_h - 4.0),
    ]
    .align_y(Alignment::Center)
    .padding(Padding {
        left: 15.0,
        right: 6.0,
        top: 0.0,
        bottom: 0.0,
    })
    .height(Length::Fill);

    container(mouse_area(header_content).on_press(on_select_clone))
        .width(Length::Fill)
        .height(Length::Fixed(row_h))
        .center_y(Length::Fill)
        .style(|_t| container::Style::default().background(COLOR_CONTRAST))
        .into()
}

/// Widget universal para renderizar una fila de canción en cualquier vista de lista.
/// Soporta: Diferentes alturas, miniaturas opcionales, resaltado de reproducción y selección.
pub fn universal_song_row_widget<'a, Message: Clone + 'a>(
    song: &crate::db::database::SongData,
    _song_idx: usize,
    is_selected: bool,
    columns: &[SortColumn],
    column_widths: &HashMap<SortColumn, u16>,
    on_select: Message,
    playing_path: &str,
    row_height: f32,
    show_thumbnail: bool,
) -> Element<'a, Message> {
    let txt_color = if is_selected {
        COLOR_TEXT_PRIMARY
    } else {
        COLOR_TEXT_SECONDARY
    };
    let is_playing = song.full_file_path.as_ref() == playing_path;

    let get_col = |col: SortColumn| -> Element<'a, Message> {
        let w = *column_widths.get(&col).unwrap_or(&100) as f32;
        let max_chars = ((w - 10.0) / 7.0).max(1.0) as usize;
        let val = format_metadata(song, &col);
        let truncated = truncate_text(&val, max_chars);

        let content: Element<'a, Message> = if col == SortColumn::TrackNumber {
            row![
                container(if is_playing {
                    text("•")
                        .size(13)
                        .color(COLOR_ACCENT)
                        .font(FONT_INTER_SANS_MEDIUM)
                        .wrapping(iced::widget::text::Wrapping::None)
                } else {
                    text("").size(13)
                })
                .width(Length::Fixed(26.0))
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center),
                container(
                    text(truncated)
                        .size(13)
                        .color(Color::from(txt_color))
                        .font(FONT_INTER_SANS_MEDIUM)
                        .wrapping(iced::widget::text::Wrapping::None)
                )
                .width(Length::Fixed(26.0))
                .align_x(iced::alignment::Horizontal::Right)
                .align_y(iced::alignment::Vertical::Center)
            ]
            .spacing(0)
            .align_y(Alignment::Center)
            .into()
        } else {
            text(truncated)
                .size(13)
                .color(Color::from(txt_color))
                .font(FONT_INTER_SANS_MEDIUM)
                .wrapping(iced::widget::text::Wrapping::None)
                .into()
        };

        let pad_left = if col == SortColumn::TrackNumber {
            0.0
        } else {
            15.0
        };
        container(content)
            .width(Length::Fixed(w))
            .height(Length::Fill)
            .center_y(Length::Fill)
            .padding(Padding {
                left: pad_left,
                right: 5.0,
                top: 0.0,
                bottom: 0.0,
            })
            .clip(true)
            .into()
    };

    let mut elements: Vec<Element<'a, Message>> = Vec::new();

    // 1. Agregar Miniatura si se solicita
    if show_thumbnail {
        let thumb_img = album_art_widget(
            song.compressed_cached_cover_root.as_deref(),
            None,
            None,
            PlaceholderStyle::Small,
            Length::Fixed(32.0),
            4.0,
        );

        elements.push(
            container(thumb_img)
                .width(Length::Fixed(42.0))
                .height(Length::Fixed(row_height))
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .into(),
        );
    }

    // 2. Agregar Columnas de metadatos (evitando duplicar el espacio de Thumbnail)
    for col in columns {
        if *col != SortColumn::AlbumCard && *col != SortColumn::AlbumThumbnail {
            elements.push(get_col(*col));
        }
    }

    let song_row_inner = iced::widget::Row::with_children(elements)
        .align_y(Alignment::Center)
        .padding([0, 5])
        .height(Length::Fill);

    mouse_area(
        container(song_row_inner)
            .width(Length::Fill)
            .height(Length::Fixed(row_height))
            .align_y(Alignment::Center)
            .style(move |_t: &Theme| {
                if is_selected {
                    container::Style::default().background(Color::from(COLOR_CONTRAST))
                } else {
                    container::Style::default()
                }
            }),
    )
    .on_press(on_select)
    .interaction(iced::mouse::Interaction::Pointer)
    .into()
}

pub fn library_song_row_widget<'a, Message: Clone + 'a>(
    song: &crate::db::database::SongData,
    song_idx: usize,
    is_selected: bool,
    columns: &[SortColumn],
    column_widths: &HashMap<SortColumn, u16>,
    on_select: Message,
    playing_path: &str,
) -> Element<'a, Message> {
    universal_song_row_widget(
        song,
        song_idx,
        is_selected,
        columns,
        column_widths,
        on_select,
        playing_path,
        32.0,
        false,
    )
}

/// Renderiza una fila de canción en modo ThumbnailList (42px) con thumbnail 32x32 redondeado al inicio.
/// Renderiza una fila de canción en modo ThumbnailList (42px) con thumbnail 32x32 redondeado al inicio.
pub fn thumbnail_song_row_widget<'a, Message: Clone + 'a>(
    song: &crate::db::database::SongData,
    song_idx: usize,
    is_selected: bool,
    columns: &[SortColumn],
    column_widths: &HashMap<SortColumn, u16>,
    on_select: Message,
    playing_path: &str,
) -> Element<'a, Message> {
    universal_song_row_widget(
        song,
        song_idx,
        is_selected,
        columns,
        column_widths,
        on_select,
        playing_path,
        42.0,
        true,
    )
}

/// Permite construir una lista universalizada que agrupa canciones por artistas,
/// gestiona el scroll, la virtualización, y las cabeceras pegajosas de forma global.
pub fn universal_song_list<'a, F>(
    manager: &'a crate::gui::library::LibraryManager,
    row_builder: F,
    _row_height: f32,
    playing_path: &'a str,
) -> Element<'a, crate::gui::app::Message>
where
    F: Fn(
            &std::sync::Arc<crate::db::database::SongData>,
            usize,
            bool,
            &str,
        ) -> Element<'a, crate::gui::app::Message>
        + 'a,
{
    let groups = &manager.artist_groups;
    if groups.is_empty() {
        return container(
            text("La biblioteca está vacía o cargando...")
                .color(COLOR_TEXT_SECONDARY)
                .font(FONT_INTER_SANS_MEDIUM),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    }

    let header_h = if manager.view_mode == crate::gui::library::LibraryViewMode::ThumbnailList {
        42.0
    } else {
        32.0
    };
    let album_header_h = if manager.view_mode == crate::gui::library::LibraryViewMode::ThumbnailList
    {
        42.0
    } else {
        32.0
    };

    let (top_space, bottom_space, _total_h, visible_elements, sticky_artist_info) =
        manager.get_visible_elements();

    let mut list_col = column![].spacing(0);
    if top_space > 0.0 {
        list_col = list_col.push(
            mouse_area(Space::new().height(Length::Fixed(top_space)))
                .on_press(crate::gui::app::Message::LibraryDeselect),
        );
    }

    for element in visible_elements {
        match element {
            crate::gui::library::LibraryVirtualRow::ArtistHeader {
                name,
                is_collapsed,
                albums_count,
                songs_count,
                duration_secs,
            } => {
                let is_header_selected = manager
                    .selected_items
                    .contains(&crate::gui::library::LibraryListItem::Artist(name.clone()));

                let is_artist_playing = manager
                    .artist_groups
                    .iter()
                    .find(|g| g.name == *name)
                    .map(|g| {
                        g.songs
                            .iter()
                            .any(|s| s.full_file_path.as_ref() == playing_path)
                    })
                    .unwrap_or(false);

                list_col = list_col.push(artist_header_widget(
                    name.clone(),
                    is_collapsed,
                    is_header_selected,
                    is_artist_playing,
                    albums_count,
                    songs_count,
                    duration_secs,
                    header_h,
                    crate::gui::app::Message::SelectArtistHeader(name.clone()),
                    crate::gui::app::Message::ToggleArtistExpansion(name.clone()),
                ));
            }
            crate::gui::library::LibraryVirtualRow::AlbumBlock {
                album_name,
                album_hash,
                artist_name,
                is_expanded,
                songs_count,
                duration_secs,
                ..
            } => {
                let composite_id = format!("{}|{}", artist_name, album_hash);
                let is_album_selected =
                    manager
                        .selected_items
                        .contains(&crate::gui::library::LibraryListItem::Album(
                            composite_id.clone(),
                        ));

                // En este modo, el header del álbum se renderiza como una fila
                list_col = list_col.push(album_header_widget(
                    album_name.clone(),
                    songs_count,
                    duration_secs,
                    is_expanded,
                    is_album_selected,
                    false,
                    album_header_h,
                    crate::gui::app::Message::SelectAlbum(composite_id.clone()),
                    crate::gui::app::Message::ToggleAlbumExpansion(composite_id),
                ));
            }
            crate::gui::library::LibraryVirtualRow::SimpleSong { song, global_idx } => {
                let is_song_selected = manager
                    .selected_items
                    .contains(&crate::gui::library::LibraryListItem::Song(song.id));
                list_col = list_col.push(row_builder(
                    &song,
                    global_idx,
                    is_song_selected,
                    playing_path,
                ));
            }
        }
    }

    if bottom_space > 0.0 {
        list_col = list_col.push(
            mouse_area(Space::new().height(Length::Fixed(bottom_space)))
                .on_press(crate::gui::app::Message::LibraryDeselect),
        );
    }

    let list_container = mouse_area(container(list_col).width(Length::Fill).padding(Padding {
        left: 10.0,
        right: 15.0,
        top: 0.0,
        bottom: 0.0,
    }))
    .on_press(crate::gui::app::Message::LibraryDeselect);

    let main_scroll = standard_scrollable(
        crate::gui::library::LIBRARY_SCROLL_ID.clone(),
        list_container,
        iced::widget::scrollable::Direction::Vertical(standard_scrollbar()),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .on_scroll(crate::gui::app::Message::LibraryScroll);

    let content: Element<'a, crate::gui::app::Message> =
        if let Some((ref st_name, st_collapsed, st_albums, st_songs, st_duration)) =
            sticky_artist_info
        {
            let is_header_selected =
                manager
                    .selected_items
                    .contains(&crate::gui::library::LibraryListItem::Artist(
                        st_name.clone(),
                    ));

            let is_artist_playing = manager
                .artist_groups
                .iter()
                .find(|g| g.name == *st_name)
                .map(|g| {
                    g.songs
                        .iter()
                        .any(|s| s.full_file_path.as_ref() == playing_path)
                })
                .unwrap_or(false);

            let sticky_overlay = container(artist_header_widget(
                st_name.clone(),
                st_collapsed,
                is_header_selected,
                is_artist_playing,
                st_albums,
                st_songs,
                st_duration,
                header_h,
                crate::gui::app::Message::SelectArtistHeader(st_name.clone()),
                crate::gui::app::Message::ToggleArtistExpansion(st_name.clone()),
            ))
            .width(Length::Fill)
            .height(Length::Fixed(header_h))
            .padding(Padding {
                left: 10.0,
                right: 15.0,
                top: 0.0,
                bottom: 0.0,
            })
            .align_y(iced::alignment::Vertical::Top);

            iced::widget::stack![main_scroll, sticky_overlay].into()
        } else {
            main_scroll.into()
        };

    content
}

/// Vista Detallada: Combina agrupación por Artista, y dentro por Álbum.
/// Muestra a la izquierda la tarjeta del Álbum y a la derecha el Header de Álbum + la lista de canciones.
pub fn detailed_song_list<'a, F>(
    manager: &'a crate::gui::library::LibraryManager,
    row_builder: F,
    _row_height: f32,
    playing_path: &'a str,
) -> Element<'a, crate::gui::app::Message>
where
    F: Fn(
            &std::sync::Arc<crate::db::database::SongData>,
            usize,
            bool,
            &str,
        ) -> Element<'a, crate::gui::app::Message>
        + 'a,
{
    let groups = &manager.artist_groups;
    if groups.is_empty() {
        return container(
            text("La biblioteca está vacía o cargando...")
                .color(COLOR_TEXT_SECONDARY)
                .font(FONT_INTER_SANS_MEDIUM),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    }

    let _header_h = if manager.view_mode == crate::gui::library::LibraryViewMode::ThumbnailList {
        42.0
    } else {
        32.0
    };
    let album_header_h = 32.0;
    let card_w = 250.0;

    let (top_space, bottom_space, _total_h, visible_elements, sticky_artist_info) =
        manager.get_visible_elements();

    let mut list_col = column![].spacing(0);
    if top_space > 0.0 {
        list_col = list_col.push(
            mouse_area(Space::new().height(Length::Fixed(top_space)))
                .on_press(crate::gui::app::Message::LibraryDeselect),
        );
    }

    for element in visible_elements {
        match element {
            crate::gui::library::LibraryVirtualRow::ArtistHeader {
                name,
                is_collapsed,
                albums_count,
                songs_count,
                duration_secs,
            } => {
                let is_artist_explicitly_selected = manager
                    .selected_items
                    .contains(&crate::gui::library::LibraryListItem::Artist(name.clone()));

                let is_artist_playing = manager
                    .artist_groups
                    .iter()
                    .find(|g| g.name == *name)
                    .map(|g| {
                        g.songs
                            .iter()
                            .any(|s| s.full_file_path.as_ref() == playing_path)
                    })
                    .unwrap_or(false);

                let header = artist_header_widget(
                    name.clone(),
                    is_collapsed,
                    is_artist_explicitly_selected,
                    is_artist_playing,
                    albums_count,
                    songs_count,
                    duration_secs,
                    32.0,
                    crate::gui::app::Message::SelectArtistHeader(name.clone()),
                    crate::gui::app::Message::ToggleArtistExpansion(name.clone()),
                );
                list_col = list_col.push(header);
            }
            crate::gui::library::LibraryVirtualRow::AlbumBlock {
                album_name,
                album_hash,
                artist_name,
                genre,
                year,
                is_expanded,
                songs,
                songs_count,
                duration_secs,
            } => {
                let cover_path = if let Some(albums) = &manager.cached_albums {
                    albums
                        .iter()
                        .find(|a| a.title == *album_name && a.artist == *artist_name)
                        .and_then(|a| a.cover_path.clone())
                } else {
                    None
                };

                let card_wrapper = album_art_widget(
                    cover_path.as_deref(),
                    None,
                    None,
                    PlaceholderStyle::Large,
                    Length::Fixed(card_w - 30.0),
                    8.0,
                );

                let info_col = column![
                    smart_truncate_text(
                        artist_name.clone(),
                        13.0,
                        FONT_INTER_SANS_MEDIUM,
                        COLOR_TEXT_PRIMARY
                    ),
                    smart_truncate_text(
                        album_name.clone(),
                        13.0,
                        FONT_INTER_SANS_MEDIUM,
                        COLOR_TEXT_PRIMARY
                    ),
                    smart_truncate_text(
                        genre.clone(),
                        13.0,
                        FONT_INTER_SANS_MEDIUM,
                        COLOR_TEXT_PRIMARY
                    ),
                    text(year.clone())
                        .size(13)
                        .color(COLOR_TEXT_PRIMARY)
                        .font(FONT_INTER_SANS_MEDIUM),
                ]
                .spacing(2)
                .width(Length::Fill);

                let composite_id = format!("{}|{}", artist_name, album_hash);
                let is_album_explicitly_selected =
                    manager
                        .selected_items
                        .contains(&crate::gui::library::LibraryListItem::Album(
                            composite_id.clone(),
                        ));
                let is_song_selected_in_album = songs.iter().any(|(s, _)| {
                    manager
                        .selected_items
                        .contains(&crate::gui::library::LibraryListItem::Song(s.id))
                });
                let is_album_card_highlighted =
                    is_album_explicitly_selected || is_song_selected_in_album;

                let card_col = column![card_wrapper, info_col]
                    .spacing(5)
                    .width(Length::Fixed(card_w - 30.0));

                let card_container = mouse_area(
                    container(card_col)
                        .width(Length::Fixed(card_w))
                        .height(Length::Fixed(323.0))
                        .padding(iced::Padding {
                            top: 15.0,
                            bottom: 15.0,
                            left: 15.0,
                            right: 15.0,
                        })
                        .style(move |_t: &Theme| {
                            if is_album_card_highlighted {
                                let rad = iced::border::Radius {
                                    top_left: 0.0,
                                    top_right: 0.0,
                                    bottom_right: 10.0,
                                    bottom_left: 10.0,
                                };
                                container::Style::default()
                                    .background(COLOR_CONTRAST)
                                    .border(iced::Border {
                                        radius: rad,
                                        ..Default::default()
                                    })
                            } else {
                                container::Style::default()
                            }
                        }),
                )
                .on_press(crate::gui::app::Message::SelectAlbum(composite_id.clone()))
                .interaction(iced::mouse::Interaction::Pointer);

                let is_album_playing = songs
                    .iter()
                    .any(|(s, _)| s.full_file_path.as_ref() == playing_path);

                let alb_header = album_header_widget(
                    album_name.clone(),
                    songs_count,
                    duration_secs,
                    is_expanded,
                    is_album_explicitly_selected,
                    is_album_playing,
                    album_header_h,
                    crate::gui::app::Message::SelectAlbum(composite_id.clone()),
                    crate::gui::app::Message::ToggleAlbumExpansion(composite_id),
                );

                let mut right_col = column![alb_header].spacing(0).width(Length::Fill);

                if is_expanded {
                    for (song, song_i) in &songs {
                        let is_song_selected = manager
                            .selected_items
                            .contains(&crate::gui::library::LibraryListItem::Song(song.id));
                        let song_row = row_builder(song, *song_i, is_song_selected, playing_path);
                        right_col = right_col.push(song_row);
                    }
                }

                let content_row = if is_expanded {
                    row![
                        container(card_container)
                            .height(Length::Shrink)
                            .align_y(iced::alignment::Vertical::Top),
                        container(right_col)
                            .height(Length::Shrink)
                            .align_y(iced::alignment::Vertical::Top)
                    ]
                    .spacing(0)
                    .width(Length::Fill)
                    .align_y(Alignment::Start)
                } else {
                    row![
                        // If collapsed, we omit the card (it's hidden) and just show the header block spanning
                        container(Space::new().width(card_w)).height(Length::Shrink),
                        container(right_col)
                            .height(Length::Shrink)
                            .align_y(iced::alignment::Vertical::Top)
                    ]
                    .spacing(0)
                    .width(Length::Fill)
                    .align_y(Alignment::Start)
                };

                list_col = list_col.push(column![
                    content_row,
                    Space::new().height(Length::Fixed(10.0))
                ]);
            }
            _ => {}
        }
    }

    if bottom_space > 0.0 {
        list_col = list_col.push(
            mouse_area(Space::new().height(Length::Fixed(bottom_space)))
                .on_press(crate::gui::app::Message::LibraryDeselect),
        );
    }

    let list_container = mouse_area(container(list_col).width(Length::Fill).padding(Padding {
        left: 10.0,
        right: 15.0,
        top: 0.0,
        bottom: 0.0,
    }))
    .on_press(crate::gui::app::Message::LibraryDeselect);

    let main_scroll = standard_scrollable(
        crate::gui::library::LIBRARY_SCROLL_ID.clone(),
        list_container,
        iced::widget::scrollable::Direction::Vertical(standard_scrollbar()),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .on_scroll(crate::gui::app::Message::LibraryScroll);

    let content: Element<'a, crate::gui::app::Message> =
        if let Some((ref st_name, st_collapsed, st_albums, st_songs, st_duration)) =
            sticky_artist_info
        {
            let is_header_selected =
                manager
                    .selected_items
                    .contains(&crate::gui::library::LibraryListItem::Artist(
                        st_name.clone(),
                    ));

            let is_artist_playing = manager
                .artist_groups
                .iter()
                .find(|g| g.name == *st_name)
                .map(|g| {
                    g.songs
                        .iter()
                        .any(|s| s.full_file_path.as_ref() == playing_path)
                })
                .unwrap_or(false);

            let sticky_overlay = container(artist_header_widget(
                st_name.clone(),
                st_collapsed,
                is_header_selected,
                is_artist_playing,
                st_albums,
                st_songs,
                st_duration,
                32.0,
                crate::gui::app::Message::SelectArtistHeader(st_name.clone()),
                crate::gui::app::Message::ToggleArtistExpansion(st_name.clone()),
            ))
            .width(Length::Fill)
            .height(Length::Fixed(32.0))
            .padding(Padding {
                left: 10.0,
                right: 15.0,
                top: 0.0,
                bottom: 0.0,
            })
            .align_y(iced::alignment::Vertical::Top);

            iced::widget::stack![main_scroll, sticky_overlay].into()
        } else {
            main_scroll.into()
        };

    content
}

// ==========================================
// WIDGET GLOBAL: CUSTOM SLIDER PERSONALIZADO
// ==========================================
//
// Widget de slider universal reutilizable con soporte para:
// - Orientaciones vertical y horizontal
// - Reset por clic secundario
// - Tooltip dinámico junto al cursor
// - Navegación con flechas de teclado (solo en estado selected)
// - Input de texto editable con cursor navegable
// - Track coloreado hasta la posición del handle
// - Snap a grid automático al soltar el arrastre
// - AppFocus routing para bloquear flechas a otros módulos
//
// Estados del slider:
// - Normal:     Sin interacción. Color: handle_color (defecto COLOR_ACCENT)
// - Hover:      Mouse sobre el slider. Color: handle_hover_color (defecto COLOR_TEXT_PRIMARY)
// - Selected:   Clic en el slider. Color: handle_selected_color (defecto COLOR_TEXT_PRIMARY)
//               + borde inset: border_selected (defecto 2px COLOR_ACCENT). Activa teclado.

/// Enumeración para la orientación del slider
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderOrientation {
    Vertical,
    Horizontal,
}

/// Posición del input de texto relativa al slider
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputPosition {
    Auto,
    Top,
    Bottom,
    Left,
    Right,
}

/// Modo de ancho del input de texto
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputWidth {
    Auto,
    Fixed(f32),
}

/// Alineación del texto dentro del input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAlign {
    Left,
    Center,
    Right,
}

/// Configuración de opciones opcionales del slider
#[derive(Debug, Clone)]
pub struct CustomSliderOptions {
    pub enable_colored_track: bool,
    pub enable_keyboard_input: bool,
    pub enable_arrow_keys: bool,
    pub show_tooltip: bool,
    pub step_size: f32,
    pub tooltip_font_size: f32,
    pub track_color: Option<Color>,
    pub active_track_color: Option<Color>,
    pub handle_color: Option<Color>,
    /// Color del handle en estado hover (mouse sobre slider, sin clic, solo visual)
    pub handle_hover_color: Option<Color>,
    /// Color del handle en estado selected (clic activo, visual + teclado)
    pub handle_selected_color: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: f32,
    /// Color del borde en estado hover (mouse sobre slider, sin clic)
    pub border_hover_color: Option<Color>,
    /// Ancho del borde en estado hover
    pub border_hover_width: f32,
    /// Color del borde inset en estado selected (clic activo)
    pub border_selected_color: Option<Color>,
    /// Ancho del borde inset en estado selected
    pub border_selected_width: f32,
    /// Radio del borde del handle (defecto: 2.0, usar handle_size / 2.0 para redondo)
    pub handle_border_radius: f32,
    // Opciones de input de teclado
    pub input_position: InputPosition,
    pub input_gap: f32,
    pub input_width: InputWidth,
    pub input_bg_color: Option<Color>,
    pub input_border_color: Option<Color>,
    pub input_border_hover_color: Option<Color>,
    pub input_border_width: f32,
    pub input_border_radius: f32,
    pub input_font_size: f32,
    pub input_font_color: Option<Color>,
    pub input_align: InputAlign,
    pub input_padding_h: f32,
    pub input_padding_v: f32,
    /// Alto fijo del input (None = automático desde font_size + padding)
    pub input_fixed_height: Option<f32>,
}

impl Default for CustomSliderOptions {
    fn default() -> Self {
        Self {
            enable_colored_track: false,
            enable_keyboard_input: false,
            enable_arrow_keys: false,
            show_tooltip: false,
            tooltip_font_size: 13.0,
            step_size: 0.1,
            track_color: None,
            active_track_color: None,
            handle_color: None,
            handle_hover_color: None,
            handle_selected_color: None,
            border_color: None,
            border_width: 0.0,
            border_hover_color: None,
            border_hover_width: 0.0,
            border_selected_color: None,
            border_selected_width: 0.0,
            handle_border_radius: 2.0,
            input_position: InputPosition::Auto,
            input_gap: 10.0,
            input_width: InputWidth::Auto,
            input_bg_color: Some(COLOR_BG),
            input_border_color: Some(COLOR_TEXT_SECONDARY),
            input_border_hover_color: Some(COLOR_ACCENT),
            input_border_width: 1.0,
            input_border_radius: 4.0,
            input_font_size: 11.0,
            input_font_color: Some(COLOR_TEXT_PRIMARY),
            input_align: InputAlign::Left,
            input_padding_h: 0.0,
            input_padding_v: 2.0,
            input_fixed_height: None,
        }
    }
}

/// Estado interno del slider (arrastre, selección, foco teclado, tooltip, input)
/// - is_selected: true cuando el slider recibió clic (teclado activado)
/// - keyboard_focused: true cuando el teclado está activo para este slider
#[derive(Debug, Clone, Default)]
struct CustomSliderState {
    is_dragging: bool,
    is_selected: bool,
    keyboard_focused: bool,
    tooltip_pos: Option<iced::Point>,
    is_input_editing: bool,
    input_value_text: String,
    input_has_focus: bool,
    input_cursor_pos: usize,
}

/// Widget de slider personalizado global
pub struct CustomSlider<'a, Message> {
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    on_change: Box<dyn Fn(f32) -> Message + 'a>,
    on_right_click: Box<dyn Fn() -> Message + 'a>,
    width: Length,
    height: Length,
    orientation: SliderOrientation,
    options: CustomSliderOptions,
    track_width: f32,
    handle_size: f32,
    format_fn: Option<Box<dyn Fn(f32) -> String + 'a>>,
    on_selected_state_change: Option<Box<dyn Fn(bool) -> Message + 'a>>,
}

impl<'a, Message> CustomSlider<'a, Message> {
    /// Crear un nuevo CustomSlider con valores por defecto
    pub fn new(
        value: f32,
        range: std::ops::RangeInclusive<f32>,
        on_change: impl Fn(f32) -> Message + 'a,
        on_right_click: impl Fn() -> Message + 'a,
    ) -> Self {
        Self {
            value: value.clamp(*range.start(), *range.end()),
            range,
            on_change: Box::new(on_change),
            on_right_click: Box::new(on_right_click),
            width: Length::Fixed(20.0),
            height: Length::Fill,
            orientation: SliderOrientation::Vertical,
            options: CustomSliderOptions::default(),
            track_width: 8.0,
            handle_size: 16.0,
            format_fn: None,
            on_selected_state_change: None,
        }
    }

    /// Establecer anchura del widget
    pub fn width(mut self, width: Length) -> Self {
        self.width = width;
        self
    }

    /// Establecer altura del widget
    pub fn height(mut self, height: Length) -> Self {
        self.height = height;
        self
    }

    /// Establecer orientación (Vertical | Horizontal)
    pub fn orientation(mut self, orientation: SliderOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Establecer opciones configurables
    pub fn options(mut self, options: CustomSliderOptions) -> Self {
        self.options = options;
        self
    }

    /// Habilitar o deshabilitar track coloreado
    pub fn with_colored_track(mut self, enabled: bool) -> Self {
        self.options.enable_colored_track = enabled;
        self
    }

    /// Color para la parte activa del track (hasta el handle)
    pub fn active_track_color(mut self, color: Color) -> Self {
        self.options.active_track_color = Some(color);
        self
    }

    /// Colores personalizados para el slider track
    pub fn track_color(mut self, color: Color) -> Self {
        self.options.track_color = Some(color);
        self
    }

    /// Colores personalizados para el handle
    pub fn handle_color(mut self, color: Color) -> Self {
        self.options.handle_color = Some(color);
        self
    }

    /// Color del handle en estado hover (mouse sobre slider, sin clic)
    pub fn handle_hover_color(mut self, color: Color) -> Self {
        self.options.handle_hover_color = Some(color);
        self
    }

    /// Color del handle en estado selected (clic activo)
    pub fn handle_selected_color(mut self, color: Color) -> Self {
        self.options.handle_selected_color = Some(color);
        self
    }

    /// Establecer borde del slider con ancho y color personalizados
    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.options.border_width = width;
        self.options.border_color = Some(color);
        self
    }

    /// Establecer borde en estado hover (mouse sobre slider, sin clic)
    pub fn border_hover(mut self, width: f32, color: Color) -> Self {
        self.options.border_hover_width = width;
        self.options.border_hover_color = Some(color);
        self
    }

    /// Establecer borde inset en estado selected (clic activo)
    pub fn border_selected(mut self, width: f32, color: Color) -> Self {
        self.options.border_selected_width = width;
        self.options.border_selected_color = Some(color);
        self
    }

    /// Radio del borde del handle (defecto: 2.0). Usar handle_size / 2.0 para forma circular.
    pub fn handle_border_radius(mut self, radius: f32) -> Self {
        self.options.handle_border_radius = radius;
        self
    }

    /// Callback opcional cuando el slider se selecciona/deselecciona (true = selected, false = deselected)
    pub fn on_selected_state_change(mut self, f: impl Fn(bool) -> Message + 'a) -> Self {
        self.on_selected_state_change = Some(Box::new(f));
        self
    }

    /// Espacio entre slider e input de teclado
    pub fn input_gap(mut self, gap: f32) -> Self {
        self.options.input_gap = gap;
        self
    }

    /// Ancho fijo del input en píxeles (None/Auto = automático desde el contenido)
    pub fn input_width_fixed(mut self, width: f32) -> Self {
        self.options.input_width = InputWidth::Fixed(width);
        self
    }

    /// Alto fijo del input en píxeles (None = automático desde font_size + padding)
    pub fn input_height_fixed(mut self, height: f32) -> Self {
        self.options.input_fixed_height = Some(height);
        self
    }

    /// Alineación del texto dentro del input
    pub fn input_align(mut self, align: InputAlign) -> Self {
        self.options.input_align = align;
        self
    }

    /// Estilo completo del input de teclado
    pub fn input_style(
        mut self,
        bg: Option<Color>,
        border: Option<Color>,
        border_focus: Option<Color>,
        border_width: f32,
        border_radius: f32,
        font_size: f32,
        font_color: Option<Color>,
    ) -> Self {
        self.options.input_bg_color = bg;
        self.options.input_border_color = border;
        self.options.input_border_hover_color = border_focus;
        self.options.input_border_width = border_width;
        self.options.input_border_radius = border_radius;
        self.options.input_font_size = font_size;
        self.options.input_font_color = font_color;
        self
    }

    /// Habilitar o deshabilitar navegación con teclado (flechas)
    pub fn with_arrow_keys(mut self, enabled: bool) -> Self {
        self.options.enable_arrow_keys = enabled;
        self
    }

    /// Habilitar o deshabilitar input de texto
    pub fn with_keyboard_input(mut self, enabled: bool) -> Self {
        self.options.enable_keyboard_input = enabled;
        self
    }

    /// Establecer función de formato personalizado para el tooltip
    pub fn format_value(mut self, f: impl Fn(f32) -> String + 'a) -> Self {
        self.format_fn = Some(Box::new(f));
        self
    }

    /// Habilitar o deshabilitar tooltip
    pub fn show_tooltip(mut self, enabled: bool) -> Self {
        self.options.show_tooltip = enabled;
        self
    }

    /// Establecer tamaño de fuente del tooltip
    pub fn tooltip_font_size(mut self, size: f32) -> Self {
        self.options.tooltip_font_size = size;
        self
    }

    /// Establecer tamaño del ancho de la pista
    pub fn track_width(mut self, width: f32) -> Self {
        self.track_width = width;
        self
    }

    /// Establecer tamaño del handle
    pub fn handle_size(mut self, size: f32) -> Self {
        self.handle_size = size;
        self
    }

    /// Calcular el porcentaje (0.0 - 1.0) basado en el valor actual
    fn calculate_percent(&self) -> f32 {
        let range_size = self.range.end() - self.range.start();
        if range_size == 0.0 {
            return 0.0;
        }
        ((self.value - self.range.start()) / range_size).clamp(0.0, 1.0)
    }

    /// Convertir posición en porcentaje a valor en el rango
    fn percent_to_value(&self, percent: f32) -> f32 {
        let range_size = self.range.end() - self.range.start();
        let new_value = self.range.start() + percent * range_size;
        new_value.clamp(*self.range.start(), *self.range.end())
    }

    /// Formatear valor para mostrar en tooltip (método privado)
    fn format_display_value(&self, val: f32) -> String {
        if let Some(ref fmt) = self.format_fn {
            fmt(val)
        } else {
            format!("{:.1}", val)
        }
    }

    /// Calcular dimensiones del input (ancho, alto)
    fn compute_input_size(&self) -> (f32, f32) {
        let font_size = self.options.input_font_size;
        let pad_h = self.options.input_padding_h;
        let pad_v = self.options.input_padding_v;
        let input_height = self
            .options
            .input_fixed_height
            .unwrap_or(font_size + pad_v * 2.0);
        let display_text = self.format_display_value(self.value);
        let input_width = match self.options.input_width {
            InputWidth::Fixed(w) => w,
            InputWidth::Auto => {
                let char_w = font_size * 0.6;
                (display_text.len() as f32 * char_w + pad_h * 2.0)
                    .ceil()
                    .max(40.0)
            }
        };
        (input_width, input_height)
    }

    /// Calcular rectángulo del input para hit testing
    fn input_bounds(&self, slider_bounds: Rectangle) -> Option<Rectangle> {
        if !self.options.enable_keyboard_input {
            return None;
        }
        let (input_width, input_height) = self.compute_input_size();
        let gap = self.options.input_gap;

        let pos = self.options.input_position;
        let resolved_pos = if pos == InputPosition::Auto {
            match self.orientation {
                SliderOrientation::Horizontal => InputPosition::Right,
                SliderOrientation::Vertical => InputPosition::Bottom,
            }
        } else {
            pos
        };

        let (input_x, input_y) = match resolved_pos {
            InputPosition::Right => {
                let x = slider_bounds.x + slider_bounds.width + gap;
                let y = slider_bounds.y + (slider_bounds.height - input_height) / 2.0;
                (x, y)
            }
            InputPosition::Bottom => {
                let x = slider_bounds.x + (slider_bounds.width - input_width) / 2.0;
                let y = slider_bounds.y + slider_bounds.height + gap;
                (x, y)
            }
            InputPosition::Left => {
                let x = slider_bounds.x - input_width - gap;
                let y = slider_bounds.y + (slider_bounds.height - input_height) / 2.0;
                (x, y)
            }
            InputPosition::Top => {
                let x = slider_bounds.x + (slider_bounds.width - input_width) / 2.0;
                let y = slider_bounds.y - input_height - gap;
                (x, y)
            }
            InputPosition::Auto => unreachable!(),
        };

        Some(Rectangle {
            x: input_x,
            y: input_y,
            width: input_width,
            height: input_height,
        })
    }
}

// ── Tooltip overlay para renderizar SIEMPRE por encima de otros widgets ──

struct TooltipOverlay {
    value_text: String,
    cursor_pos: iced::Point,
    font_size: f32,
}

impl<Message, Theme> iced::advanced::overlay::Overlay<Message, Theme, iced::Renderer>
    for TooltipOverlay
{
    fn layout(
        &mut self,
        _renderer: &iced::Renderer,
        _bounds: iced::Size,
    ) -> iced::advanced::layout::Node {
        iced::advanced::layout::Node::new(iced::Size::ZERO)
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        _theme: &Theme,
        _style: &iced::advanced::renderer::Style,
        _layout: iced::advanced::Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
    ) {
        use iced::advanced::text::Renderer as _;
        use iced::advanced::Renderer as _;

        let val_display = &self.value_text;
        let font_size = self.font_size;
        let padding: f32 = 4.0;
        let border_radius = 4.0;

        let char_width = font_size * 0.6;
        let text_width = (val_display.len() as f32 * char_width).ceil().max(20.0);
        let tooltip_width = (text_width + padding * 2.0 + 2.0).max(56.0) - 4.0;
        let tooltip_height = (font_size + padding * 2.0 + 2.0).max(25.0);

        let cursor_pos = self.cursor_pos;

        // No clamp contra viewport porque es overlay (va sobre todo)
        let tooltip_x = cursor_pos.x - tooltip_width / 2.0;
        let tooltip_y = cursor_pos.y - tooltip_height - 10.0;

        let tooltip_rect = iced::Rectangle {
            x: tooltip_x,
            y: tooltip_y,
            width: tooltip_width,
            height: tooltip_height,
        };

        renderer.fill_quad(
            iced::advanced::graphics::core::renderer::Quad {
                bounds: tooltip_rect,
                border: iced::Border {
                    radius: border_radius.into(),
                    width: 1.0,
                    color: COLOR_TEXT_SECONDARY,
                },
                ..Default::default()
            },
            COLOR_CONTRAST,
        );

        renderer.fill_text(
            iced::advanced::text::Text {
                content: val_display.clone(),
                bounds: iced::Size::new(
                    tooltip_width - padding * 2.0,
                    tooltip_height - padding * 2.0,
                ),
                size: iced::Pixels(font_size),
                line_height: iced::advanced::text::LineHeight::Relative(1.0),
                font: FONT_INTER_SANS_MEDIUM,
                align_x: iced::alignment::Horizontal::Left.into(),
                align_y: iced::alignment::Vertical::Center,
                shaping: iced::advanced::text::Shaping::Basic,
                wrapping: iced::advanced::text::Wrapping::None,
            },
            iced::Point::new(tooltip_x + padding + 4.0, tooltip_y + padding + 9.0),
            COLOR_TEXT_PRIMARY,
            tooltip_rect,
        );
    }
}

impl<'a, Message: 'a> iced::advanced::Widget<Message, Theme, iced::Renderer>
    for CustomSlider<'a, Message>
{
    fn size(&self) -> iced::Size<Length> {
        iced::Size {
            width: self.width,
            height: self.height,
        }
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(CustomSliderState::default())
    }

    fn layout(
        &mut self,
        _tree: &mut iced::advanced::widget::Tree,
        _renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let mut size = limits.resolve(self.width, self.height, iced::Size::ZERO);

        if self.options.enable_keyboard_input {
            let pos = self.options.input_position;
            let resolved_pos = if pos == InputPosition::Auto {
                match self.orientation {
                    SliderOrientation::Horizontal => InputPosition::Right,
                    SliderOrientation::Vertical => InputPosition::Bottom,
                }
            } else {
                pos
            };

            if matches!(resolved_pos, InputPosition::Right | InputPosition::Left) {
                let (input_w, _input_h) = self.compute_input_size();
                size.width = (size.width - input_w - self.options.input_gap).max(20.0);
            }
        }

        iced::advanced::layout::Node::new(size)
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        _viewport: &iced::Rectangle,
    ) {
        let bounds = layout.bounds();
        let Some(cursor_pos) = cursor.position() else {
            tree.state.downcast_mut::<CustomSliderState>().is_dragging = false;
            return;
        };

        let is_focus = bounds.contains(cursor_pos);
        let state = tree.state.downcast_mut::<CustomSliderState>();

        // Almacenar posición del cursor para tooltip overlay
        let show_tooltip_hover = is_focus
            && !state.is_selected
            && (!GLOBAL_SLIDER_SELECTED.load(Ordering::Relaxed) || state.is_dragging);
        state.tooltip_pos = if (show_tooltip_hover || state.is_dragging || state.is_selected)
            && self.options.show_tooltip
        {
            Some(cursor_pos)
        } else {
            None
        };

        let input_bounds_opt = self.input_bounds(bounds);
        let in_input = input_bounds_opt
            .map(|b| b.contains(cursor_pos))
            .unwrap_or(false);

        match event {
            // Cualquier clic del mouse fuera del slider desactiva hover (DEBE ir primero)
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
                if !is_focus && state.is_selected =>
            {
                state.is_selected = false;
                state.keyboard_focused = false;
                GLOBAL_SLIDER_SELECTED.store(false, Ordering::Relaxed);
                if let Some(ref cb) = self.on_selected_state_change {
                    shell.publish(cb(false));
                }
            }

            // Clic izquierdo: input o slider
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                if self.options.enable_keyboard_input && in_input {
                    state.is_selected = false;
                    state.keyboard_focused = false;
                    GLOBAL_SLIDER_SELECTED.store(false, Ordering::Relaxed);
                    if let Some(ref cb) = self.on_selected_state_change {
                        shell.publish(cb(true));
                    }
                    state.is_input_editing = true;
                    state.input_value_text = self.format_display_value(self.value);
                    state.input_has_focus = true;
                    if let Some(input_b) = input_bounds_opt {
                        let rel_x = cursor_pos.x - input_b.x;
                        let char_w = self.options.input_font_size * 0.6;
                        let pos = (rel_x / char_w).round() as usize;
                        state.input_cursor_pos = pos.min(state.input_value_text.len());
                    } else {
                        state.input_cursor_pos = state.input_value_text.len();
                    }
                } else if is_focus {
                    if state.input_has_focus && state.is_input_editing {
                        let parsed = strip_suffix(&state.input_value_text).parse::<f32>();
                        if let Ok(v) = parsed {
                            let clamped = v.clamp(*self.range.start(), *self.range.end());
                            self.value = clamped;
                            shell.publish((self.on_change)(clamped));
                        }
                        state.is_input_editing = false;
                        state.input_has_focus = false;
                    }
                    if !state.is_selected {
                        state.is_selected = true;
                        state.keyboard_focused = true;
                        GLOBAL_SLIDER_SELECTED.store(true, Ordering::Relaxed);
                        if let Some(ref cb) = self.on_selected_state_change {
                            shell.publish(cb(true));
                        }
                    }
                    state.is_dragging = true;
                    let percent = match self.orientation {
                        SliderOrientation::Vertical => {
                            1.0 - ((cursor_pos.y - bounds.y) / bounds.height).clamp(0.0, 1.0)
                        }
                        SliderOrientation::Horizontal => {
                            ((cursor_pos.x - bounds.x) / bounds.width).clamp(0.0, 1.0)
                        }
                    };
                    let raw_value = self.percent_to_value(percent);
                    let step = self.options.step_size;
                    let new_value = if step > 0.0 {
                        (raw_value / step).round() * step
                    } else {
                        raw_value
                    };
                    let clamped = new_value.clamp(*self.range.start(), *self.range.end());
                    self.value = clamped;
                    shell.publish((self.on_change)(clamped));
                } else if state.input_has_focus {
                    let parsed = strip_suffix(&state.input_value_text).parse::<f32>();
                    if let Ok(v) = parsed {
                        let clamped = v.clamp(*self.range.start(), *self.range.end());
                        self.value = clamped;
                        shell.publish((self.on_change)(clamped));
                    }
                    state.is_input_editing = false;
                    state.input_has_focus = false;
                    if let Some(ref cb) = self.on_selected_state_change {
                        shell.publish(cb(false));
                    }
                } else if state.is_selected {
                    // Clic izquierdo fuera del slider → deseleccionar
                    state.is_selected = false;
                    state.keyboard_focused = false;
                    GLOBAL_SLIDER_SELECTED.store(false, Ordering::Relaxed);
                    if let Some(ref cb) = self.on_selected_state_change {
                        shell.publish(cb(false));
                    }
                }
            }

            // Clic derecho: reset en slider o en input
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Right)) => {
                if is_focus {
                    shell.publish((self.on_right_click)());
                } else if self.options.enable_keyboard_input && in_input {
                    shell.publish((self.on_right_click)());
                }
            }

            // Liberación de botón izquierdo: snap a grid (NO redundante — se ejecuta al soltar)
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                state.is_dragging = false;
                if state.is_selected {
                    let step = self.options.step_size;
                    if step > 0.0 {
                        let snapped = (self.value / step).round() * step;
                        let clamped = snapped.clamp(*self.range.start(), *self.range.end());
                        if (clamped - self.value).abs() > f32::EPSILON {
                            self.value = clamped;
                            shell.publish((self.on_change)(clamped));
                        }
                    }
                }
            }

            // Movimiento del mouse (arrastre continuo)
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. }) => {
                if state.is_dragging {
                    let percent = match self.orientation {
                        SliderOrientation::Vertical => {
                            1.0 - ((cursor_pos.y - bounds.y) / bounds.height).clamp(0.0, 1.0)
                        }
                        SliderOrientation::Horizontal => {
                            ((cursor_pos.x - bounds.x) / bounds.width).clamp(0.0, 1.0)
                        }
                    };
                    let raw_value = self.percent_to_value(percent);
                    let step = self.options.step_size;
                    let new_value = if step > 0.0 {
                        (raw_value / step).round() * step
                    } else {
                        raw_value
                    };
                    let clamped = new_value.clamp(*self.range.start(), *self.range.end());
                    self.value = clamped;
                    shell.publish((self.on_change)(clamped));
                }
            }

            // Navegación con teclado (flechas) - solo si keyboard_focused está activo
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. })
                if self.options.enable_arrow_keys && state.keyboard_focused =>
            {
                let step = self.options.step_size;

                let should_change = match (self.orientation, key) {
                    (
                        SliderOrientation::Vertical,
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp),
                    ) => {
                        let raw = self.value + step;
                        self.value = (raw / step).round() * step;
                        self.value = self.value.clamp(*self.range.start(), *self.range.end());
                        true
                    }
                    (
                        SliderOrientation::Vertical,
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown),
                    ) => {
                        let raw = self.value - step;
                        self.value = (raw / step).round() * step;
                        self.value = self.value.clamp(*self.range.start(), *self.range.end());
                        true
                    }
                    (
                        SliderOrientation::Horizontal,
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight),
                    ) => {
                        let raw = self.value + step;
                        self.value = (raw / step).round() * step;
                        self.value = self.value.clamp(*self.range.start(), *self.range.end());
                        true
                    }
                    (
                        SliderOrientation::Horizontal,
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft),
                    ) => {
                        let raw = self.value - step;
                        self.value = (raw / step).round() * step;
                        self.value = self.value.clamp(*self.range.start(), *self.range.end());
                        true
                    }
                    (
                        _,
                        iced::keyboard::Key::Named(
                            iced::keyboard::key::Named::ArrowUp
                            | iced::keyboard::key::Named::ArrowDown
                            | iced::keyboard::key::Named::ArrowLeft
                            | iced::keyboard::key::Named::ArrowRight,
                        ),
                    ) => {
                        // Flecha que no corresponde a la orientación — no limpiar foco
                        false
                    }
                    _ => {
                        state.keyboard_focused = false;
                        false
                    }
                };

                if should_change {
                    shell.publish((self.on_change)(self.value));
                }
            }

            // Input de teclado (cuando el input tiene foco)
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. })
                if self.options.enable_keyboard_input && state.input_has_focus =>
            {
                match key {
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => {
                        shell.capture_event();
                        let parsed = strip_suffix(&state.input_value_text).parse::<f32>();
                        if let Ok(v) = parsed {
                            let step = self.options.step_size;
                            let snapped = if step > 0.0 {
                                (v / step).round() * step
                            } else {
                                v
                            };
                            let clamped = snapped.clamp(*self.range.start(), *self.range.end());
                            self.value = clamped;
                            shell.publish((self.on_change)(clamped));
                        }
                        state.is_input_editing = false;
                        state.input_has_focus = false;
                        if let Some(ref cb) = self.on_selected_state_change {
                            shell.publish(cb(false));
                        }
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                        shell.capture_event();
                        state.is_input_editing = false;
                        state.input_has_focus = false;
                        if let Some(ref cb) = self.on_selected_state_change {
                            shell.publish(cb(false));
                        }
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft) => {
                        if state.input_cursor_pos > 0 {
                            state.input_cursor_pos -= 1;
                        }
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight) => {
                        if state.input_cursor_pos < state.input_value_text.len() {
                            state.input_cursor_pos += 1;
                        }
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
                        if state.input_cursor_pos > 0 && !state.input_value_text.is_empty() {
                            state.input_value_text.remove(state.input_cursor_pos - 1);
                            state.input_cursor_pos -= 1;
                        }
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete) => {
                        if state.input_cursor_pos < state.input_value_text.len() {
                            state.input_value_text.remove(state.input_cursor_pos);
                        }
                    }
                    iced::keyboard::Key::Character(c) => {
                        if state.input_value_text.len() < 20 {
                            let ch = c.chars().next().unwrap_or(' ');
                            if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch.is_whitespace() {
                                state.input_value_text.insert(state.input_cursor_pos, ch);
                                state.input_cursor_pos += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }

            // Limpiar foco de teclado en otras teclas (solo KeyPressed, no KeyReleased)
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { .. })
                if state.keyboard_focused =>
            {
                state.keyboard_focused = false;
            }

            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        _theme: &Theme,
        _style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        _viewport: &iced::Rectangle,
    ) {
        let bounds = layout.bounds();
        let is_hover = cursor
            .position()
            .map(|p| bounds.contains(p))
            .unwrap_or(false);
        let state = tree.state.downcast_ref::<CustomSliderState>();
        let is_selected = state.is_selected;

        let percent = self.calculate_percent();

        match self.orientation {
            SliderOrientation::Vertical => {
                self.draw_vertical_slider(renderer, bounds, is_hover, is_selected, percent);
            }
            SliderOrientation::Horizontal => {
                self.draw_horizontal_slider(renderer, bounds, is_hover, is_selected, percent);
            }
        }

        if self.options.enable_keyboard_input {
            self.draw_input(renderer, bounds, state);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut iced::advanced::widget::Tree,
        _layout: iced::advanced::Layout<'_>,
        _renderer: &iced::Renderer,
        _viewport: &iced::Rectangle,
        _translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, iced::Renderer>> {
        let state = tree.state.downcast_ref::<CustomSliderState>();
        let cursor_pos = state.tooltip_pos?;

        let val_display = self.format_display_value(self.value);
        if val_display.is_empty() {
            return None;
        }

        let overlay = TooltipOverlay {
            value_text: val_display,
            cursor_pos,
            font_size: self.options.tooltip_font_size,
        };

        Some(iced::advanced::overlay::Element::new(Box::new(overlay)))
    }
}

impl<'a, Message> CustomSlider<'a, Message> {
    /// Renderizar slider en orientación vertical
    fn draw_vertical_slider(
        &self,
        renderer: &mut iced::Renderer,
        bounds: Rectangle,
        is_hover: bool,
        is_selected: bool,
        percent: f32,
    ) {
        use iced::advanced::Renderer as _;

        let track_color = self.options.track_color.unwrap_or(COLOR_CONTRAST);
        let active_color = self.options.active_track_color.unwrap_or(COLOR_ACCENT);

        // Dibujar track completo (fondo) — SIN borde (borde solo para handle)
        let track_x = bounds.x + (bounds.width - self.track_width) / 2.0;
        let track_rect = Rectangle {
            x: track_x,
            y: bounds.y,
            width: self.track_width,
            height: bounds.height,
        };

        renderer.fill_quad(
            iced::advanced::graphics::core::renderer::Quad {
                bounds: track_rect,
                border: iced::Border {
                    radius: (self.track_width / 2.0).into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..Default::default()
            },
            track_color,
        );

        // Dibujar track coloreado si está habilitado
        if self.options.enable_colored_track {
            let colored_height = bounds.height * percent;
            let colored_y = bounds.y + bounds.height - colored_height;

            let colored_rect = Rectangle {
                x: track_x,
                y: colored_y,
                width: self.track_width,
                height: colored_height,
            };

            renderer.fill_quad(
                iced::advanced::graphics::core::renderer::Quad {
                    bounds: colored_rect,
                    border: iced::Border {
                        radius: (self.track_width / 2.0).into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                active_color,
            );
        }

        // Dibujar handle
        let handle_height = self.handle_size;
        let handle_width = self.handle_size;
        let handle_y = bounds.y + bounds.height - (percent * bounds.height) - handle_height / 2.0;
        let handle_x = bounds.x + (bounds.width - handle_width) / 2.0;

        // Seleccionar color y borde según estado
        let handle_color = if is_selected {
            self.options
                .handle_selected_color
                .unwrap_or(COLOR_TEXT_PRIMARY)
        } else if is_hover {
            self.options
                .handle_hover_color
                .unwrap_or(COLOR_TEXT_PRIMARY)
        } else {
            self.options.handle_color.unwrap_or(COLOR_ACCENT)
        };

        let (brd_color, brd_width) = if is_selected {
            let bw = self.options.border_selected_width;
            let bc = self.options.border_selected_color.unwrap_or(COLOR_ACCENT);
            (bc, bw)
        } else if is_hover {
            let bw = self.options.border_hover_width;
            let bc = self
                .options
                .border_hover_color
                .unwrap_or(Color::TRANSPARENT);
            (bc, bw)
        } else {
            let bw = self.options.border_width;
            let bc = self.options.border_color.unwrap_or(Color::TRANSPARENT);
            (bc, bw)
        };

        let handle_rect = Rectangle {
            x: handle_x,
            y: handle_y.clamp(bounds.y, bounds.y + bounds.height - handle_height),
            width: handle_width,
            height: handle_height,
        };

        if brd_width > 0.0 {
            // Borde inset: dibujar borde exterior primero
            renderer.fill_quad(
                iced::advanced::graphics::core::renderer::Quad {
                    bounds: handle_rect,
                    border: iced::Border {
                        radius: self.options.handle_border_radius.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                },
                brd_color,
            );
            // Luego el interior reducido
            let inset = brd_width;
            renderer.fill_quad(
                iced::advanced::graphics::core::renderer::Quad {
                    bounds: Rectangle {
                        x: handle_rect.x + inset,
                        y: handle_rect.y + inset,
                        width: handle_rect.width - 2.0 * inset,
                        height: handle_rect.height - 2.0 * inset,
                    },
                    border: iced::Border {
                        radius: (2.0 - inset * 0.5).max(0.0).into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                },
                handle_color,
            );
        } else {
            renderer.fill_quad(
                iced::advanced::graphics::core::renderer::Quad {
                    bounds: handle_rect,
                    border: iced::Border {
                        radius: self.options.handle_border_radius.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                },
                handle_color,
            );
        }
    }

    /// Renderizar slider en orientación horizontal
    fn draw_horizontal_slider(
        &self,
        renderer: &mut iced::Renderer,
        bounds: Rectangle,
        is_hover: bool,
        is_selected: bool,
        percent: f32,
    ) {
        use iced::advanced::Renderer as _;

        let track_color = self.options.track_color.unwrap_or(COLOR_CONTRAST);
        let active_color = self.options.active_track_color.unwrap_or(COLOR_ACCENT);

        // Dibujar track fondo
        let track_y = bounds.y + (bounds.height - self.track_width) / 2.0;
        renderer.fill_quad(
            iced::advanced::graphics::core::renderer::Quad {
                bounds: Rectangle {
                    x: bounds.x,
                    y: track_y,
                    width: bounds.width,
                    height: self.track_width,
                },
                border: iced::Border {
                    radius: (self.track_width / 2.0).into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..Default::default()
            },
            track_color,
        );

        // Dibujar track activo (si habilitado)
        if self.options.enable_colored_track {
            renderer.fill_quad(
                iced::advanced::graphics::core::renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x,
                        y: track_y,
                        width: bounds.width * percent,
                        height: self.track_width,
                    },
                    border: iced::Border {
                        radius: (self.track_width / 2.0).into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                active_color,
            );
        }

        // Dibujar handle
        let handle_size = self.handle_size;
        let handle_x = bounds.x + (percent * bounds.width) - handle_size / 2.0;
        let handle_y = bounds.y + (bounds.height - handle_size) / 2.0;
        let handle_clamped_x = handle_x.clamp(bounds.x, bounds.x + bounds.width - handle_size);

        // Seleccionar color y borde según estado
        let handle_color = if is_selected {
            self.options
                .handle_selected_color
                .unwrap_or(COLOR_TEXT_PRIMARY)
        } else if is_hover {
            self.options
                .handle_hover_color
                .unwrap_or(COLOR_TEXT_PRIMARY)
        } else {
            self.options.handle_color.unwrap_or(COLOR_ACCENT)
        };

        let (brd_color, brd_width) = if is_selected {
            let bw = self.options.border_selected_width;
            let bc = self.options.border_selected_color.unwrap_or(COLOR_ACCENT);
            (bc, bw)
        } else if is_hover {
            let bw = self.options.border_hover_width;
            let bc = self
                .options
                .border_hover_color
                .unwrap_or(Color::TRANSPARENT);
            (bc, bw)
        } else {
            let bw = self.options.border_width;
            let bc = self.options.border_color.unwrap_or(Color::TRANSPARENT);
            (bc, bw)
        };

        let handle_rect = Rectangle {
            x: handle_clamped_x,
            y: handle_y,
            width: handle_size,
            height: handle_size,
        };

        if brd_width > 0.0 {
            // Borde inset: borde exterior
            renderer.fill_quad(
                iced::advanced::graphics::core::renderer::Quad {
                    bounds: handle_rect,
                    border: iced::Border {
                        radius: self.options.handle_border_radius.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                },
                brd_color,
            );
            // Interior reducido
            let inset = brd_width;
            renderer.fill_quad(
                iced::advanced::graphics::core::renderer::Quad {
                    bounds: Rectangle {
                        x: handle_rect.x + inset,
                        y: handle_rect.y + inset,
                        width: handle_rect.width - 2.0 * inset,
                        height: handle_rect.height - 2.0 * inset,
                    },
                    border: iced::Border {
                        radius: (2.0 - inset * 0.5).max(0.0).into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                },
                handle_color,
            );
        } else {
            renderer.fill_quad(
                iced::advanced::graphics::core::renderer::Quad {
                    bounds: handle_rect,
                    border: iced::Border {
                        radius: self.options.handle_border_radius.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                },
                handle_color,
            );
        }
    }

    /// Renderizar input de texto junto al slider
    fn draw_input(
        &self,
        renderer: &mut iced::Renderer,
        slider_bounds: Rectangle,
        state: &CustomSliderState,
    ) {
        use iced::advanced::text::Renderer as _;
        use iced::advanced::Renderer as _;

        let font_size = self.options.input_font_size;
        let pad_h = self.options.input_padding_h;
        let pad_v = self.options.input_padding_v;
        let (input_width, input_height) = self.compute_input_size();
        let gap = self.options.input_gap;

        let display_text = if state.is_input_editing {
            state.input_value_text.clone()
        } else {
            self.format_display_value(self.value)
        };

        let pos = self.options.input_position;
        let resolved_pos = if pos == InputPosition::Auto {
            match self.orientation {
                SliderOrientation::Horizontal => InputPosition::Right,
                SliderOrientation::Vertical => InputPosition::Bottom,
            }
        } else {
            pos
        };

        let (input_x, input_y) = match resolved_pos {
            InputPosition::Right => {
                let x = slider_bounds.x + slider_bounds.width + gap;
                let y = slider_bounds.y + (slider_bounds.height - input_height) / 2.0;
                (x, y)
            }
            InputPosition::Bottom => {
                let x = slider_bounds.x + (slider_bounds.width - input_width) / 2.0;
                let y = slider_bounds.y + slider_bounds.height + gap;
                (x, y)
            }
            InputPosition::Left => {
                let x = slider_bounds.x - input_width - gap;
                let y = slider_bounds.y + (slider_bounds.height - input_height) / 2.0;
                (x, y)
            }
            InputPosition::Top => {
                let x = slider_bounds.x + (slider_bounds.width - input_width) / 2.0;
                let y = slider_bounds.y - input_height - gap;
                (x, y)
            }
            InputPosition::Auto => unreachable!(),
        };

        let input_rect = Rectangle {
            x: input_x,
            y: input_y,
            width: input_width,
            height: input_height,
        };

        let bg = self.options.input_bg_color.unwrap_or(COLOR_BG);
        let border_color = if state.input_has_focus {
            self.options
                .input_border_hover_color
                .unwrap_or(COLOR_ACCENT)
        } else {
            self.options
                .input_border_color
                .unwrap_or(COLOR_TEXT_SECONDARY)
        };
        let border_width = self.options.input_border_width;
        let border_radius = self.options.input_border_radius;
        let font_color = self.options.input_font_color.unwrap_or(COLOR_TEXT_PRIMARY);

        renderer.fill_quad(
            iced::advanced::graphics::core::renderer::Quad {
                bounds: input_rect,
                border: iced::Border {
                    radius: border_radius.into(),
                    width: border_width,
                    color: border_color,
                },
                ..Default::default()
            },
            bg,
        );

        let align_x = match self.options.input_align {
            InputAlign::Left => iced::alignment::Horizontal::Left,
            InputAlign::Center => iced::alignment::Horizontal::Center,
            InputAlign::Right => iced::alignment::Horizontal::Right,
        };

        let text_x = match align_x {
            iced::alignment::Horizontal::Left => input_x + pad_h,
            iced::alignment::Horizontal::Center => input_x + input_width / 2.0,
            iced::alignment::Horizontal::Right => input_x + input_width - pad_h,
            _ => input_x + pad_h,
        };

        // Centrado vertical manual: posición Y en el centro del input menos mitad del alto de fuente
        let text_y = input_y + (input_height - font_size) / 2.0;
        let text_bounds_h = font_size;

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
                line_height: iced::advanced::text::LineHeight::Relative(1.0),
                font: FONT_INTER_SANS_MEDIUM,
                align_x: align_x.into(),
                align_y: iced::alignment::Vertical::Top,
                shaping: iced::advanced::text::Shaping::Basic,
                wrapping: iced::advanced::text::Wrapping::None,
            },
            iced::Point::new(text_x, text_y),
            font_color,
            input_rect,
        );
    }
}

impl<'a, Message: 'a> From<CustomSlider<'a, Message>> for Element<'a, Message> {
    fn from(slider: CustomSlider<'a, Message>) -> Self {
        Element::new(slider)
    }
}
