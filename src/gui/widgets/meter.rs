use super::*;

use crate::gui::dimensions::METER_NUMERIC_WIDTH;

use iced::widget::column;

// ==============================
// VuMeterWidget — stereo L/R horizontal bars with dBFS scale
// Display-only: no event handling, no state tree.
// ==============================

pub const METER_MIN_DB: f32 = -60.0;
const METER_MAX_DB: f32 = 6.0;
const METER_TOTAL_RANGE: f32 = 66.0;
/// Base height for compact mode (bars + scale only).
const METER_HEIGHT: f32 = 24.0;
/// Extended height when numeric readout, phase correlation, or crest factor are active.
const METER_HEIGHT_EXTENDED: f32 = 48.0;
const METER_BAR_HEIGHT: f32 = 6.0;
const METER_BAR_GAP: f32 = 2.0;
const METER_LABEL_WIDTH: f32 = 12.0;
const METER_LED_SIZE: f32 = 8.0;
const METER_SCALE_HEIGHT: f32 = 8.0;

// Zone colors for dBFS level ranges
const ZONE_GREEN: Color = color!(0x4CAF50);
const ZONE_YELLOW: Color = color!(0xFFC107);
const ZONE_ORANGE: Color = color!(0xFF9800);
const ZONE_RED: Color = color!(0xF44336);

// Scale ticks: (db, _is_major) — all labeled for full dBFS visibility
const METER_TICKS: &[(f32, bool)] = &[
    (-60.0, true),
    (-40.0, true),
    (-20.0, true),
    (-15.0, true),
    (-12.0, true),
    (-9.0, true),
    (-6.0, true),
    (-3.0, true),
    (0.0, true),
    (3.0, true),
    (6.0, true),
];

/// Display-only stereo VU meter widget with horizontal L/R bars.
pub struct VuMeterWidget {
    peak_l: f32,
    rms_l: f32,
    hold_l: f32,
    peak_r: f32,
    rms_r: f32,
    hold_r: f32,
    clipping: bool,
    /// M/S mode: when true, labels show "M"/"S" instead of "L"/"R"
    ms_mode: bool,
    /// Phase correlation: -1.0 (mono) to +1.0 (wide)
    phase_corr: f32,
    /// Show numeric dBFS readout for peak/RMS values
    show_numeric: bool,
    /// Crest factor for L channel in dB (peak - RMS)
    crest_l: f32,
    /// Crest factor for R channel in dB (peak - RMS)
    crest_r: f32,
}

impl VuMeterWidget {
    /// Create a new VuMeterWidget.
    ///
    /// - `peak_l/r`: current peak dBFS per channel
    /// - `rms_l/r`: current RMS dBFS per channel (bar fill)
    /// - `hold_l/r`: held peak dBFS per channel (peak hold marker)
    /// - `clipping`: whether clipping LED should light up
    /// - `ms_mode`: when true, labels show "M"/"S" instead of "L"/"R"
    /// - `phase_corr`: phase correlation -1.0..+1.0
    /// - `show_numeric`: show numeric dBFS readout
    /// - `crest_l/r`: crest factor peak-RMS in dB
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        peak_l: f32,
        rms_l: f32,
        hold_l: f32,
        peak_r: f32,
        rms_r: f32,
        hold_r: f32,
        clipping: bool,
        ms_mode: bool,
        phase_corr: f32,
        show_numeric: bool,
        crest_l: f32,
        crest_r: f32,
    ) -> Self {
        Self {
            peak_l,
            rms_l,
            hold_l,
            peak_r,
            rms_r,
            hold_r,
            clipping,
            ms_mode,
            phase_corr: phase_corr.clamp(-1.0, 1.0),
            show_numeric,
            crest_l,
            crest_r,
        }
    }

    /// Convert a dB value to a pixel x-coordinate within the bar area.
    fn db_to_x(db: f32, bar_left: f32, bar_width: f32) -> f32 {
        let clamped = db.clamp(METER_MIN_DB, METER_MAX_DB);
        let ratio = (clamped - METER_MIN_DB) / METER_TOTAL_RANGE;
        (bar_left + ratio * bar_width).round()
    }

    /// Compute the effective widget height: extended when advanced features are active.
    fn effective_height(&self) -> f32 {
        if self.show_numeric
            || self.phase_corr.abs() > 0.001
            || self.crest_l.abs() > 0.001
            || self.crest_r.abs() > 0.001
        {
            METER_HEIGHT_EXTENDED
        } else {
            METER_HEIGHT
        }
    }

    /// Format a dBFS value for numeric readout: one decimal place with sign.
    fn format_db(db: f32) -> String {
        if db <= METER_MIN_DB {
            "-inf".to_string()
        } else {
            format!("{:+.1}", db)
        }
    }

    /// Map a correlation value (-1..+1) to a pixel x in the gauge area.
    fn corr_to_x(corr: f32, gauge_left: f32, gauge_width: f32) -> f32 {
        let ratio = (corr + 1.0) / 2.0; // 0..1
        (gauge_left + ratio * gauge_width).round()
    }

    /// Return the zone color for a given dB value.
    fn zone_color(db: f32) -> Color {
        if db <= -9.0 {
            ZONE_GREEN
        } else if db <= -3.0 {
            ZONE_YELLOW
        } else if db <= 0.0 {
            ZONE_ORANGE
        } else {
            ZONE_RED
        }
    }

    /// Return the label string for a tick dB value.
    fn tick_label(db: f32) -> &'static str {
        match db as i32 {
            -60 => "-60",
            -40 => "-40",
            -20 => "-20",
            -15 => "-15",
            -12 => "-12",
            -9 => "-9",
            -6 => "-6",
            -3 => "-3",
            0 => "0",
            3 => "+3",
            6 => "+6",
            _ => "",
        }
    }
}

impl<'a, Message: 'a> iced::advanced::Widget<Message, Theme, iced::Renderer> for VuMeterWidget {
    fn size(&self) -> iced::Size<Length> {
        iced::Size {
            width: Length::Fixed(400.0),
            height: Length::Fixed(self.effective_height()),
        }
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(())
    }

    fn layout(
        &mut self,
        _tree: &mut iced::advanced::widget::Tree,
        _renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let height = self.effective_height();
        let size = limits.resolve(
            Length::Fixed(400.0),
            Length::Fixed(height),
            iced::Size::ZERO,
        );
        iced::advanced::layout::Node::new(size)
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
        use iced::advanced::Renderer as _;
        use iced::advanced::text::Renderer as _;

        let bounds = layout.bounds();
        let bx = bounds.x;
        let by = bounds.y;
        let bw = bounds.width;
        let bh = bounds.height;
        let _ = bh; // used indirectly via effective_height

        // ── Background ──
        renderer.fill_quad(
            iced::advanced::graphics::core::renderer::Quad {
                bounds,
                border: iced::Border::default(),
                ..Default::default()
            },
            COLOR_BG,
        );

        // ── Layout constants ──
        let left_margin = METER_LED_SIZE + 3.0 + 10.0;
        let numeric_available = self.show_numeric || self.ms_mode;
        let right_margin = if numeric_available {
            METER_NUMERIC_WIDTH + 5.0
        } else {
            5.0
        };
        let bar_left = (bx + left_margin).round();
        let bar_width = (bw - left_margin - right_margin).max(1.0);
        let _bar_right = (bar_left + bar_width).round();

        // Vertical layout: top padding 1px, L bar 6px, gap 1px, R bar 6px
        let l_bar_y = (by + 1.0).round();
        let r_bar_y = (l_bar_y + METER_BAR_HEIGHT + METER_BAR_GAP).round();

        // Phase correlation gauge row (below R bar): 6px tall
        let gauge_y = (r_bar_y + METER_BAR_HEIGHT + 2.0).round();
        let gauge_h: f32 = 3.0;
        let gauge_area_h: f32 = 6.0;

        // Crest factor text row (below gauge): 8px
        let crest_y = (gauge_y + gauge_area_h + 1.0).round();
        let crest_h: f32 = 8.0;

        // Scale ticks row (below crest): 8px
        let scale_y = (crest_y + crest_h + 1.0).round();

        // ── Draw L bar ──
        if self.rms_l > METER_MIN_DB {
            let fill_x = Self::db_to_x(self.rms_l, bar_left, bar_width);
            let fill_w = (fill_x - bar_left).max(0.0);
            if fill_w > 0.0 {
                renderer.fill_quad(
                    iced::advanced::graphics::core::renderer::Quad {
                        bounds: iced::Rectangle::new(
                            iced::Point::new(bar_left, l_bar_y),
                            iced::Size::new(fill_w, METER_BAR_HEIGHT),
                        ),
                        border: iced::Border::default(),
                        ..Default::default()
                    },
                    Self::zone_color(self.rms_l),
                );
            }
        }

        // ── Draw R bar ──
        if self.rms_r > METER_MIN_DB {
            let fill_x = Self::db_to_x(self.rms_r, bar_left, bar_width);
            let fill_w = (fill_x - bar_left).max(0.0);
            if fill_w > 0.0 {
                renderer.fill_quad(
                    iced::advanced::graphics::core::renderer::Quad {
                        bounds: iced::Rectangle::new(
                            iced::Point::new(bar_left, r_bar_y),
                            iced::Size::new(fill_w, METER_BAR_HEIGHT),
                        ),
                        border: iced::Border::default(),
                        ..Default::default()
                    },
                    Self::zone_color(self.rms_r),
                );
            }
        }

        // ── Draw peak hold markers: thin vertical lines ──
        if self.hold_l > METER_MIN_DB {
            let hx = Self::db_to_x(self.hold_l, bar_left, bar_width);
            renderer.fill_quad(
                iced::advanced::graphics::core::renderer::Quad {
                    bounds: iced::Rectangle::new(
                        iced::Point::new(hx, l_bar_y),
                        iced::Size::new(2.0, METER_BAR_HEIGHT),
                    ),
                    border: iced::Border::default(),
                    ..Default::default()
                },
                Self::zone_color(self.hold_l),
            );
        }
        if self.hold_r > METER_MIN_DB {
            let hx = Self::db_to_x(self.hold_r, bar_left, bar_width);
            renderer.fill_quad(
                iced::advanced::graphics::core::renderer::Quad {
                    bounds: iced::Rectangle::new(
                        iced::Point::new(hx, r_bar_y),
                        iced::Size::new(2.0, METER_BAR_HEIGHT),
                    ),
                    border: iced::Border::default(),
                    ..Default::default()
                },
                Self::zone_color(self.hold_r),
            );
        }

        // ── Draw clipping LED: small square indicator ──
        let led_x = (bx + 2.0).round();
        let led_y = (by + 1.0).round();
        let led_color = if self.clipping {
            ZONE_RED
        } else {
            COLOR_TEXT_SECONDARY.scale_alpha(0.3)
        };
        let led_border = if self.clipping {
            ZONE_RED
        } else {
            COLOR_TEXT_SECONDARY.scale_alpha(0.5)
        };
        renderer.fill_quad(
            iced::advanced::graphics::core::renderer::Quad {
                bounds: iced::Rectangle::new(
                    iced::Point::new(led_x, led_y),
                    iced::Size::new(METER_LED_SIZE, METER_LED_SIZE),
                ),
                border: iced::Border {
                    radius: 1.0.into(),
                    width: 1.0,
                    color: led_border,
                },
                ..Default::default()
            },
            led_color,
        );

        // ── Draw "L"/"R" or "M"/"S" labels ──
        let label_x = (bx + METER_LED_SIZE + 3.0).round();
        let label_font_size = 8.0;
        let (label_top, label_bot) = if self.ms_mode { ("M", "S") } else { ("L", "R") };

        renderer.fill_text(
            iced::advanced::text::Text {
                content: label_top.to_string(),
                bounds: iced::Size::new(10.0, METER_BAR_HEIGHT),
                size: iced::Pixels(label_font_size),
                line_height: iced::advanced::text::LineHeight::Relative(1.0),
                font: FONT_INTER_SANS_MEDIUM,
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                shaping: iced::advanced::text::Shaping::Basic,
                wrapping: iced::advanced::text::Wrapping::None,
            },
            iced::Point::new(label_x, l_bar_y),
            COLOR_TEXT_SECONDARY,
            iced::Rectangle::new(
                iced::Point::new(label_x, l_bar_y),
                iced::Size::new(10.0, METER_BAR_HEIGHT),
            ),
        );
        renderer.fill_text(
            iced::advanced::text::Text {
                content: label_bot.to_string(),
                bounds: iced::Size::new(10.0, METER_BAR_HEIGHT),
                size: iced::Pixels(label_font_size),
                line_height: iced::advanced::text::LineHeight::Relative(1.0),
                font: FONT_INTER_SANS_MEDIUM,
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                shaping: iced::advanced::text::Shaping::Basic,
                wrapping: iced::advanced::text::Wrapping::None,
            },
            iced::Point::new(label_x, r_bar_y),
            COLOR_TEXT_SECONDARY,
            iced::Rectangle::new(
                iced::Point::new(label_x, r_bar_y),
                iced::Size::new(10.0, METER_BAR_HEIGHT),
            ),
        );

        // ── Numeric dBFS readout: right side of widget ──
        if self.show_numeric {
            let num_x = (bar_left + bar_width + 4.0).round();
            let num_font = 7.0;
            let prefix_top = if self.ms_mode { "M: " } else { "" };
            let prefix_bot = if self.ms_mode { "S: " } else { "" };

            // Top row (L/M): Pk and Rms
            let top_text = format!(
                "{}Pk:{} Rms:{}",
                prefix_top,
                Self::format_db(self.peak_l),
                Self::format_db(self.rms_l),
            );
            renderer.fill_text(
                iced::advanced::text::Text {
                    content: top_text,
                    bounds: iced::Size::new(METER_NUMERIC_WIDTH, METER_BAR_HEIGHT),
                    size: iced::Pixels(num_font),
                    line_height: iced::advanced::text::LineHeight::Relative(1.0),
                    font: FONT_INTER_SANS_MEDIUM,
                    align_x: iced::alignment::Horizontal::Left.into(),
                    align_y: iced::alignment::Vertical::Center,
                    shaping: iced::advanced::text::Shaping::Basic,
                    wrapping: iced::advanced::text::Wrapping::None,
                },
                iced::Point::new(num_x, l_bar_y),
                COLOR_TEXT_SECONDARY,
                iced::Rectangle::new(
                    iced::Point::new(num_x, l_bar_y),
                    iced::Size::new(METER_NUMERIC_WIDTH, METER_BAR_HEIGHT),
                ),
            );

            // Bottom row (R/S): Pk and Rms
            let bot_text = format!(
                "{}Pk:{} Rms:{}",
                prefix_bot,
                Self::format_db(self.peak_r),
                Self::format_db(self.rms_r),
            );
            renderer.fill_text(
                iced::advanced::text::Text {
                    content: bot_text,
                    bounds: iced::Size::new(METER_NUMERIC_WIDTH, METER_BAR_HEIGHT),
                    size: iced::Pixels(num_font),
                    line_height: iced::advanced::text::LineHeight::Relative(1.0),
                    font: FONT_INTER_SANS_MEDIUM,
                    align_x: iced::alignment::Horizontal::Left.into(),
                    align_y: iced::alignment::Vertical::Center,
                    shaping: iced::advanced::text::Shaping::Basic,
                    wrapping: iced::advanced::text::Wrapping::None,
                },
                iced::Point::new(num_x, r_bar_y),
                COLOR_TEXT_SECONDARY,
                iced::Rectangle::new(
                    iced::Point::new(num_x, r_bar_y),
                    iced::Size::new(METER_NUMERIC_WIDTH, METER_BAR_HEIGHT),
                ),
            );
        }

        // ── Phase correlation gauge: horizontal bar -1 to +1 ──
        let gauge_left = bar_left;
        let gauge_width = bar_width;

        // Background bar
        renderer.fill_quad(
            iced::advanced::graphics::core::renderer::Quad {
                bounds: iced::Rectangle::new(
                    iced::Point::new(gauge_left, gauge_y),
                    iced::Size::new(gauge_width, gauge_h),
                ),
                border: iced::Border::default(),
                ..Default::default()
            },
            COLOR_CONTRAST,
        );

        // Fill from center (0.0) to phase_corr value
        let center_x = Self::corr_to_x(0.0, gauge_left, gauge_width);
        let corr_x = Self::corr_to_x(self.phase_corr, gauge_left, gauge_width);
        let (fill_left, fill_width) = if self.phase_corr >= 0.0 {
            (center_x, (corr_x - center_x).max(0.0))
        } else {
            (corr_x, (center_x - corr_x).max(0.0))
        };
        if fill_width > 0.0 {
            let corr_color = if self.phase_corr > 0.0 {
                ZONE_GREEN
            } else {
                ZONE_RED
            };
            renderer.fill_quad(
                iced::advanced::graphics::core::renderer::Quad {
                    bounds: iced::Rectangle::new(
                        iced::Point::new(fill_left, gauge_y),
                        iced::Size::new(fill_width, gauge_h),
                    ),
                    border: iced::Border::default(),
                    ..Default::default()
                },
                corr_color,
            );
        }

        // Center marker (0.0)
        renderer.fill_quad(
            iced::advanced::graphics::core::renderer::Quad {
                bounds: iced::Rectangle::new(
                    iced::Point::new(center_x - 0.5, gauge_y),
                    iced::Size::new(1.0, gauge_h),
                ),
                border: iced::Border::default(),
                ..Default::default()
            },
            COLOR_TEXT_SECONDARY,
        );

        // Correlation numeric label to the right of gauge
        let corr_label = format!("{:.2}", self.phase_corr);
        let corr_label_color = if self.phase_corr > 0.0 {
            ZONE_GREEN
        } else if self.phase_corr < 0.0 {
            ZONE_RED
        } else {
            COLOR_TEXT_SECONDARY
        };
        renderer.fill_text(
            iced::advanced::text::Text {
                content: corr_label,
                bounds: iced::Size::new(40.0, gauge_area_h),
                size: iced::Pixels(7.0),
                line_height: iced::advanced::text::LineHeight::Relative(1.0),
                font: FONT_INTER_SANS_MEDIUM,
                align_x: iced::alignment::Horizontal::Left.into(),
                align_y: iced::alignment::Vertical::Center,
                shaping: iced::advanced::text::Shaping::Basic,
                wrapping: iced::advanced::text::Wrapping::None,
            },
            iced::Point::new(bar_left, gauge_y - 1.0),
            corr_label_color,
            iced::Rectangle::new(
                iced::Point::new(bar_left, gauge_y - 1.0),
                iced::Size::new(40.0, gauge_area_h),
            ),
        );

        // ── Crest factor: numeric text + colored bar ──
        // Show the dominant channel (whichever has higher crest)
        let crest_val = self.crest_l.max(self.crest_r);
        let crest_text = format!("Crest: {:.1} dB", crest_val);
        let crest_color = if crest_val > 10.0 {
            ZONE_GREEN // dynamic
        } else if crest_val > 3.0 {
            ZONE_YELLOW // medium
        } else {
            ZONE_RED // compressed
        };
        renderer.fill_text(
            iced::advanced::text::Text {
                content: crest_text,
                bounds: iced::Size::new(METER_NUMERIC_WIDTH, crest_h),
                size: iced::Pixels(7.0),
                line_height: iced::advanced::text::LineHeight::Relative(1.0),
                font: FONT_INTER_SANS_MEDIUM,
                align_x: iced::alignment::Horizontal::Left.into(),
                align_y: iced::alignment::Vertical::Top,
                shaping: iced::advanced::text::Shaping::Basic,
                wrapping: iced::advanced::text::Wrapping::None,
            },
            iced::Point::new(bar_left, crest_y),
            COLOR_TEXT_SECONDARY,
            iced::Rectangle::new(
                iced::Point::new(bar_left, crest_y),
                iced::Size::new(METER_NUMERIC_WIDTH, crest_h),
            ),
        );

        // Small colored bar for crest factor visual (3px height, max 60px wide)
        let crest_bar_w = ((crest_val / 20.0) * 60.0).clamp(0.0, 60.0);
        if crest_bar_w > 0.0 {
            renderer.fill_quad(
                iced::advanced::graphics::core::renderer::Quad {
                    bounds: iced::Rectangle::new(
                        iced::Point::new(bar_left, crest_y + 7.0),
                        iced::Size::new(crest_bar_w, 2.0),
                    ),
                    border: iced::Border::default(),
                    ..Default::default()
                },
                crest_color,
            );
        }

        // ── Draw scale ticks and labels ──
        let tick_font_size = 7.0;
        for &(db, _is_major) in METER_TICKS {
            let tx = Self::db_to_x(db, bar_left, bar_width);

            // Tick line: 4px tall
            renderer.fill_quad(
                iced::advanced::graphics::core::renderer::Quad {
                    bounds: iced::Rectangle::new(
                        iced::Point::new(tx, scale_y),
                        iced::Size::new(1.0, 4.0),
                    ),
                    border: iced::Border::default(),
                    ..Default::default()
                },
                COLOR_TEXT_SECONDARY,
            );

            // Tick label
            let label = Self::tick_label(db);
            renderer.fill_text(
                iced::advanced::text::Text {
                    content: label.to_string(),
                    bounds: iced::Size::new(20.0, 7.0),
                    size: iced::Pixels(tick_font_size),
                    line_height: iced::advanced::text::LineHeight::Relative(1.0),
                    font: FONT_INTER_SANS_MEDIUM,
                    align_x: iced::alignment::Horizontal::Center.into(),
                    align_y: iced::alignment::Vertical::Top,
                    shaping: iced::advanced::text::Shaping::Basic,
                    wrapping: iced::advanced::text::Wrapping::None,
                },
                iced::Point::new(tx - 10.0, scale_y + 4.0),
                COLOR_TEXT_SECONDARY,
                iced::Rectangle::new(
                    iced::Point::new(tx - 10.0, scale_y + 4.0),
                    iced::Size::new(20.0, 7.0),
                ),
            );
        }
    }
}

impl<'a, Message: 'a> From<VuMeterWidget> for Element<'a, Message> {
    fn from(widget: VuMeterWidget) -> Self {
        Element::new(widget)
    }
}

// ==============================
// MeterPopup — vista flotante de configuración del VU Meter
// ==============================

/// Construye el contenido del popup de configuración del medidor VU.
///
/// Devuelve un `Element<Message>` que contiene:
/// 1. Título: "Configuración del Meter"
/// 2. Activar/desactivar medidor: toggle
/// 3. Tiempo de hold del pico: NumberStepper (500–5000 ms)
/// 4. Ventana RMS: NumberStepper (50–1000 ms)
/// 5. Modo M/S: toggle (solo el toggle, la computación M/S es en update_meter)
/// 6. Hold infinito: toggle
pub fn meter_popup_view<'a>(
    hold_time_ms: f32,
    rms_window_ms: f32,
    ms_mode: bool,
    infinite_hold: bool,
    meter_enabled: bool,
) -> Element<'a, crate::gui::app::Message> {
    use crate::gui::app::Message;

    let group_title = |name: &'a str| -> Element<'a, Message> {
        text(name)
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM)
            .into()
    };

    let subfunc_label = |name: &'a str, enabled: bool| -> Element<'a, Message> {
        let color = if enabled {
            COLOR_TEXT_PRIMARY
        } else {
            COLOR_TEXT_SECONDARY
        };
        text(name)
            .size(13)
            .color(color)
            .font(FONT_INTER_SANS_MEDIUM)
            .into()
    };

    let clickable_toggle = |label: Element<'a, Message>, msg: Message| -> Element<'a, Message> {
        mouse_area(label).on_press(msg).into()
    };

    // Título
    let title_row = row![
        group_title("Configuración del Meter"),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    // Activar/desactivar medidor: toggle
    let enabled_checkbox: Element<'a, Message> =
        StandardCheckbox::new(meter_enabled, |b| Message::MeterToggleEnabled(b)).into();
    let enabled_row = row![
        enabled_checkbox,
        Space::new().width(Length::Fixed(5.0)),
        clickable_toggle(
            subfunc_label("Mostrar medidor", meter_enabled),
            Message::MeterToggleEnabled(!meter_enabled),
        ),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    // Tiempo de hold del pico: NumberStepper (500–5000 ms)
    let hold_label = text("Tiempo de hold del pico:")
        .size(13)
        .color(COLOR_TEXT_PRIMARY)
        .font(FONT_INTER_SANS_MEDIUM);
    let hold_stepper: Element<'a, Message> = NumberStepper::new(
        hold_time_ms as f64,
        500.0..=5000.0,
        StepperUnit::Milliseconds,
        |v| Message::MeterHoldTimeChanged(v as f32),
    )
    .on_selected_state_change(|_| Message::NoOp)
    .into();
    let hold_row = row![hold_label, Space::new().width(Length::Fill), hold_stepper,]
        .align_y(Alignment::Center);

    // Ventana RMS: NumberStepper (50–1000 ms)
    let rms_label = text("Ventana RMS:")
        .size(13)
        .color(COLOR_TEXT_PRIMARY)
        .font(FONT_INTER_SANS_MEDIUM);
    let rms_stepper: Element<'a, Message> = NumberStepper::new(
        rms_window_ms as f64,
        50.0..=1000.0,
        StepperUnit::Milliseconds,
        |v| Message::MeterRmsWindowChanged(v as f32),
    )
    .on_selected_state_change(|_| Message::NoOp)
    .into();
    let rms_row =
        row![rms_label, Space::new().width(Length::Fill), rms_stepper,].align_y(Alignment::Center);

    // Modo M/S: toggle (solo visual, computación en update_meter)
    let ms_checkbox: Element<'a, Message> =
        StandardCheckbox::new(ms_mode, |b| Message::MeterMsModeToggle(b)).into();
    let ms_row = row![
        ms_checkbox,
        Space::new().width(Length::Fixed(5.0)),
        clickable_toggle(
            subfunc_label("Modo M/S", ms_mode),
            Message::MeterMsModeToggle(!ms_mode),
        ),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    // Hold infinito: toggle
    let inf_checkbox: Element<'a, Message> =
        StandardCheckbox::new(infinite_hold, |b| Message::MeterInfiniteHoldToggle(b)).into();
    let inf_row = row![
        inf_checkbox,
        Space::new().width(Length::Fixed(5.0)),
        clickable_toggle(
            subfunc_label("Hold infinito", infinite_hold),
            Message::MeterInfiniteHoldToggle(!infinite_hold),
        ),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    // Contenedor del popup con fondo COLOR_CONTRAST y bordes redondeados
    let content = column![
        title_row,
        Space::new().height(Length::Fixed(8.0)),
        enabled_row,
        Space::new().height(Length::Fixed(8.0)),
        hold_row,
        Space::new().height(Length::Fixed(6.0)),
        rms_row,
        Space::new().height(Length::Fixed(8.0)),
        ms_row,
        Space::new().height(Length::Fixed(6.0)),
        inf_row,
    ]
    .spacing(0)
    .padding(iced::Padding {
        top: 10.0,
        bottom: 10.0,
        left: 12.0,
        right: 12.0,
    })
    .width(Length::Fixed(280.0));

    container(content)
        .style(|_t: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(COLOR_CONTRAST)),
            border: iced::Border {
                radius: 6.0.into(),
                width: 1.0,
                color: COLOR_TEXT_SECONDARY,
            },
            ..Default::default()
        })
        .into()
}
