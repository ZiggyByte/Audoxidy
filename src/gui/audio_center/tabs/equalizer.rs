//! Equalizer tab.

use crate::audio::AudioManager;
use crate::gui::dimensions::{IconSize, SPACE_MD, SPACE_SM, SPACE_XS};
use crate::gui::theme::*;
use crate::gui::widgets::{IconState, IconVariant, icon_button};
use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Element, Length};

use super::super::messages::AudioCenterMessage;
use super::super::state::AudioCenterManager;

// ==========================================
// VISTA: PESTAÑA 2 - ECUALIZADOR (CUSTOM VERTICAL SLIDER)
// ==========================================

pub(super) fn view_equalizer<'a>(
    manager: &'a AudioCenterManager,
    audio_manager: &'a AudioManager,
) -> Element<'a, crate::gui::app::Message> {
    let toggle_eq = row![
        text("Activar Ecualizador")
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        crate::gui::widgets::standard_toggler(
            manager.equalizer_enabled,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqToggleSelected(b)),
            18.0,
            COLOR_ACCENT,
            COLOR_TEXT_SECONDARY,
            COLOR_TEXT_PRIMARY,
            COLOR_TEXT_SECONDARY,
        )
    ]
    .spacing(SPACE_MD)
    .align_y(Alignment::Center);

    let bands_mode = row![
        text("Bandas:")
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        crate::gui::widgets::standard_radio("20", false, Some(manager.equalizer_bands_31), |b| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqBandsSelected(b))
        }),
        crate::gui::widgets::standard_radio("31", true, Some(manager.equalizer_bands_31), |b| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqBandsSelected(b))
        }),
    ]
    .spacing(SPACE_MD)
    .align_y(Alignment::Center);

    // Preset name label — visible only when a non-Default preset is selected
    let preset_label_text = match &manager.selected_preset {
        Some(p) if p.name != "Default" => format!("Preset: {}", p.name),
        _ => String::new(),
    };
    let preset_label: Element<'a, crate::gui::app::Message> = if !preset_label_text.is_empty() {
        iced::widget::container(
            iced::widget::text(preset_label_text)
                .size(14)
                .color(COLOR_TEXT_PRIMARY)
                .font(FONT_INTER_SANS_MEDIUM),
        )
        .padding([0.0, SPACE_XS])
        .into()
    } else {
        iced::widget::Space::new().width(Length::Fixed(0.0)).into()
    };

    let preset_actions = row![
        preset_label,
        icon_button(
            "equalizer-straight.svg",
            IconSize::S,
            IconState::Default,
            Some(crate::gui::app::Message::AudioCenterMsg(
                AudioCenterMessage::EqPresetIconLoad
            )),
            IconVariant::Default,
            Some("Cargar preset"),
        ),
        icon_button(
            "save-outlined-straight.svg",
            IconSize::S,
            IconState::Default,
            Some(crate::gui::app::Message::AudioCenterMsg(
                AudioCenterMessage::EqPresetIconSave
            )),
            IconVariant::Default,
            Some("Guardar preset"),
        ),
        icon_button(
            "restore-straight.svg",
            IconSize::S,
            IconState::Default,
            Some(crate::gui::app::Message::AudioCenterMsg(
                AudioCenterMessage::EqPresetIconReset
            )),
            IconVariant::Default,
            Some("Restablecer preset"),
        ),
    ]
    .spacing(SPACE_SM)
    .align_y(Alignment::Center);

    let top_row = row![
        container(toggle_eq)
            .width(Length::FillPortion(1))
            .align_x(Alignment::Start),
        container(bands_mode)
            .width(Length::FillPortion(1))
            .align_x(Alignment::Center),
        container(preset_actions)
            .width(Length::FillPortion(1))
            .align_x(Alignment::End),
    ]
    .align_y(Alignment::Center);

    // Contenedor Principal de Sliders
    // Preamp (1) + Eq Bands (20 / 31)

    // Custom Canvas Vertical Slider Helper (usando nuevo widget global)
    fn vertical_slider<'a>(
        label_top: String,
        label_bot: String,
        value: f32,
        on_change: impl Fn(f32) -> crate::gui::app::Message + 'a,
        on_right_click: impl Fn() -> crate::gui::app::Message + 'a,
        on_hover: impl Fn(bool) -> crate::gui::app::Message + 'a,
        eq_disabled: bool,
    ) -> Element<'a, crate::gui::app::Message> {
        let slider_base =
            crate::gui::widgets::CustomSlider::new(value, -9.0..=9.0, on_change, on_right_click)
                .orientation(crate::gui::widgets::SliderOrientation::Vertical)
                .with_arrow_keys(true)
                .format_value(|v| format!("{:.1} dB", v))
                .show_tooltip(true)
                .tooltip_font_size(12.0)
                .width(Length::Fixed(24.0))
                .height(Length::Fixed(240.0))
                .on_selected_state_change(on_hover);

        let slider = if eq_disabled {
            slider_base
                .handle_color(COLOR_CONTRAST)
                .border(0.0, COLOR_ACCENT)
                .handle_hover_color(COLOR_ACCENT)
                .handle_selected_color(COLOR_CONTRAST)
                .border_selected(1.0, COLOR_TEXT_SECONDARY)
        } else {
            slider_base
        };

        column![
            text(label_top)
                .size(10.4)
                .color(COLOR_TEXT_PRIMARY)
                .font(FONT_INTER_SANS_MEDIUM),
            Space::new().height(Length::Fixed(10.0)),
            slider,
            Space::new().height(Length::Fixed(10.0)),
            text(label_bot)
                .size(10.4)
                .color(COLOR_TEXT_PRIMARY)
                .font(FONT_INTER_SANS_MEDIUM),
        ]
        .align_x(Alignment::Center)
        .into()
    }

    let preamp_col = vertical_slider(
        "Pre".to_string(),
        format!("{:.1}", manager.preamp_gain),
        manager.preamp_gain,
        |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqPreampChanged(v)),
        || crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqPreampChanged(0.0)),
        |active| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(active))
        },
        !manager.equalizer_enabled,
    );

    // Bandas
    let bands_count = if manager.equalizer_bands_31 { 31 } else { 20 };
    let mut bands_row = row![].spacing(if manager.equalizer_bands_31 {
        3.98
    } else {
        20.18
    });

    for i in 0..bands_count {
        let info = audio_manager.get_eq_band_info(i).unwrap_or((0.0, 0.0));
        let freq = info.0;
        let gain = manager.eq_band_gains.get(i).copied().unwrap_or(0.0);

        let freq_label = if freq >= 1000.0 {
            format!("{:.1}k", freq / 1000.0).replace(".0k", "k")
        } else {
            format!("{:.0}", freq)
        };
        let show_top = i % 2 == 0;

        let label_top = if show_top {
            freq_label.clone()
        } else {
            " ".to_string()
        };
        let label_bot = if !show_top {
            freq_label
        } else {
            " ".to_string()
        };

        bands_row = bands_row.push(vertical_slider(
            label_top,
            label_bot,
            gain,
            move |v| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqBandChanged(i, v))
            },
            move || {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqBandChanged(i, 0.0))
            },
            |active| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(
                    active,
                ))
            },
            !manager.equalizer_enabled,
        ));
    }

    let legend = text(
        "*Puede restablecer al valor por defecto haciendo clic secundario sobre un deslizador.",
    )
    .size(12)
    .color(COLOR_TEXT_SECONDARY)
    .font(FONT_INTER_SANS_MEDIUM);

    // Guia vertical de nivel de ganancia: -9 dB (mínimo) a +9 dB (máximo), con 0 dB como referencia de "sin cambio".
    let guide_gain = column![
        Space::new().height(Length::Fixed(25.0)),
        text("+9")
            .size(11)
            .color(COLOR_TEXT_SECONDARY)
            .font(FONT_INTER_SANS_MEDIUM),
        Space::new().height(Length::Fixed(97.0)),
        text("0")
            .size(12)
            .color(COLOR_TEXT_SECONDARY)
            .font(FONT_INTER_SANS_MEDIUM),
        Space::new().height(Length::Fixed(97.0)),
        text("-9")
            .size(11)
            .color(COLOR_TEXT_SECONDARY)
            .font(FONT_INTER_SANS_MEDIUM),
    ]
    .align_x(Alignment::Center);

    column![
        Space::new().height(Length::Fixed(19.0)),
        top_row,
        Space::new().height(Length::Fixed(25.0)),
        row![
            preamp_col,
            Space::new().width(Length::Fixed(2.0)),
            guide_gain,
            Space::new().width(Length::Fixed(2.0)),
            bands_row
        ],
        Space::new().height(Length::Fill),
        legend
    ]
    .into()
}
