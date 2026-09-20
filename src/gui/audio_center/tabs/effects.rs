//! Audio effects tab.

use crate::audio::AudioManager;
use crate::gui::dimensions::{
    EFFECT_INPUT_WIDTH, EFFECT_SLIDER_HEIGHT, RADIUS_M, SPACE_LG, SPACE_MD, SPACE_SM, SPACE_XS,
    STROKE_HAIRLINE,
};
use crate::gui::theme::*;
use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};

use super::super::messages::{AudioCenterMessage, AudioStateToggle, DspEffect};
use super::super::state::AudioCenterManager;

// ==========================================
// VISTA: PESTAÑA 3 - EFECTOS DE AUDIO
// ==========================================

pub(super) fn view_audio_effects<'a>(
    _manager: &'a AudioCenterManager,
    audio_manager: &'a AudioManager,
) -> Element<'a, crate::gui::app::Message> {
    // Helper manual inline effect param constructor
    fn view_effect<'a>(
        title: &'a str,
        param_label: &'a str,
        val: f32,
        range: std::ops::RangeInclusive<f32>,
        enabled: bool,
        _default_val: f32,
        on_toggle: impl Fn(bool) -> crate::gui::app::Message + 'a,
        on_change: impl Fn(f32) -> crate::gui::app::Message + 'a,
        on_reset: crate::gui::app::Message,
        extra_widget: Option<Element<'a, crate::gui::app::Message>>,
    ) -> Element<'a, crate::gui::app::Message> {
        view_effect_with_secondary(
            title,
            param_label,
            val,
            range,
            enabled,
            _default_val,
            on_toggle,
            on_change,
            on_reset,
            extra_widget,
            None,
            None,
            0.1,
            "{:.1}",
            0.0,
        )
    }

    fn view_effect_with_secondary<'a>(
        title: &'a str,
        param_label: &'a str,
        val: f32,
        range: std::ops::RangeInclusive<f32>,
        enabled: bool,
        _default_val: f32,
        on_toggle: impl Fn(bool) -> crate::gui::app::Message + 'a,
        on_change: impl Fn(f32) -> crate::gui::app::Message + 'a,
        on_reset: crate::gui::app::Message,
        extra_widget: Option<Element<'a, crate::gui::app::Message>>,
        fixed_height: Option<f32>,
        secondary: Option<(
            f32,
            std::ops::RangeInclusive<f32>,
            Box<dyn Fn(f32) -> crate::gui::app::Message + 'a>,
            crate::gui::app::Message,
            &'a str,
            f32,
            &'a str,
        )>,
        primary_step_size: f32,
        primary_fmt: &'a str,
        extra_padding_top: f32,
    ) -> Element<'a, crate::gui::app::Message> {
        let stroke_color = if enabled {
            COLOR_ACCENT
        } else {
            COLOR_CONTRAST
        };

        // Cabecera con Toggle Switch a la derecha
        let top_row = row![
            text(title)
                .size(13)
                .color(COLOR_TEXT_PRIMARY)
                .font(FONT_INTER_SANS_MEDIUM),
            Space::new().width(SPACE_SM),
            if let Some(w) = extra_widget {
                w
            } else {
                Space::new().into()
            },
            Space::new().width(Length::Fill),
            crate::gui::widgets::standard_toggler(
                enabled,
                on_toggle,
                17.0,
                COLOR_ACCENT,
                COLOR_TEXT_SECONDARY,
                COLOR_BG,
                COLOR_TEXT_SECONDARY,
            )
        ]
        .align_y(Alignment::Center);

        // --- NUEVO CUSTOM SLIDER ---
        let on_reset_msg = on_reset.clone();
        let mut primary_opts = crate::gui::widgets::CustomSliderOptions::default();
        primary_opts.step_size = primary_step_size;
        primary_opts.enable_colored_track = true;
        primary_opts.enable_arrow_keys = true;
        primary_opts.track_color = Some(COLOR_BG);
        let param_slider = crate::gui::widgets::CustomSlider::new(
            val,
            range.clone(),
            on_change,
            move || on_reset_msg.clone(), // Reset con clic derecho
        )
        .orientation(crate::gui::widgets::SliderOrientation::Horizontal)
        .width(Length::Fill)
        .height(Length::Fixed(EFFECT_SLIDER_HEIGHT))
        .options(primary_opts)
        .with_keyboard_input(true)
        .input_width_fixed(EFFECT_INPUT_WIDTH)
        .input_height_fixed(EFFECT_SLIDER_HEIGHT)
        .input_align(crate::gui::widgets::InputAlign::Center)
        .input_style(
            Some(COLOR_BG),
            Some(COLOR_BG),
            Some(COLOR_ACCENT),
            1.0,
            4.0,
            11.0,
            Some(COLOR_TEXT_PRIMARY),
        )
        .on_selected_state_change(|active| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(active))
        });
        let param_slider = if primary_fmt == "{:.2}" {
            param_slider.format_value(|v| format!("{:.2}", v))
        } else {
            param_slider
        };

        // --- FILA INFERIOR REESTRUCTURADA ---
        let bottom_row = row![
            text(param_label)
                .size(11)
                .color(COLOR_TEXT_SECONDARY)
                .font(FONT_INTER_SANS_MEDIUM),
            Space::new().width(SPACE_MD),
            param_slider,
        ]
        .align_y(Alignment::Center);

        // Secondary slider row (optional)
        let secondary_row: Option<Element<_>> = secondary.map(
            |(s_val, s_range, s_change, s_reset, s_label, s_step, s_fmt)| {
                let mut sec_opts = crate::gui::widgets::CustomSliderOptions::default();
                sec_opts.step_size = s_step;
                sec_opts.enable_colored_track = true;
                sec_opts.enable_arrow_keys = true;
                sec_opts.track_color = Some(COLOR_BG);
                let sec_slider =
                    crate::gui::widgets::CustomSlider::new(s_val, s_range, s_change, move || {
                        s_reset.clone()
                    })
                    .orientation(crate::gui::widgets::SliderOrientation::Horizontal)
                    .width(Length::Fill)
                    .height(Length::Fixed(EFFECT_SLIDER_HEIGHT))
                    .options(sec_opts)
                    .with_keyboard_input(true)
                    .input_width_fixed(EFFECT_INPUT_WIDTH)
                    .input_height_fixed(EFFECT_SLIDER_HEIGHT)
                    .input_align(crate::gui::widgets::InputAlign::Center)
                    .input_style(
                        Some(COLOR_BG),
                        Some(COLOR_BG),
                        Some(COLOR_ACCENT),
                        1.0,
                        4.0,
                        11.0,
                        Some(COLOR_TEXT_PRIMARY),
                    )
                    .on_selected_state_change(|active| {
                        crate::gui::app::Message::AudioCenterMsg(
                            AudioCenterMessage::SliderHoverActive(active),
                        )
                    });
                let sec_slider = if s_fmt == "{:.2}" {
                    sec_slider.format_value(|v| format!("{:.2}", v))
                } else {
                    sec_slider
                };
                row![
                    text(s_label)
                        .size(11)
                        .color(COLOR_TEXT_SECONDARY)
                        .font(FONT_INTER_SANS_MEDIUM),
                    Space::new().width(SPACE_MD),
                    sec_slider,
                ]
                .align_y(Alignment::Center)
                .into()
            },
        );

        // Build column - top row then spacer then slider rows
        let mut col = column![
            top_row,
            Space::new().height(SPACE_MD + extra_padding_top),
            bottom_row
        ];
        if let Some(sr) = secondary_row {
            col = col.push(iced::widget::Space::new().height(SPACE_SM));
            col = col.push(sr);
        }

        let c = container(col)
            .padding([SPACE_MD, SPACE_MD])
            .style(move |_t: &Theme| {
                container::Style::default()
                    .background(COLOR_CONTRAST)
                    .border(iced::Border {
                        color: stroke_color,
                        width: STROKE_HAIRLINE,
                        radius: RADIUS_M.into(),
                    })
            });
        let c: Element<_> = if let Some(h) = fixed_height {
            c.height(Length::Fixed(h)).into()
        } else {
            c.into()
        };
        c
    }

    let (
        sub_bass_enabled,
        sub_bass_gain,
        mid_bass_enabled,
        mid_bass_gain,
        voice_boost_enabled,
        voice_boost_gain,
        noise_gate_enabled,
        noise_gate_threshold,
        stereo_expander_enabled,
        stereo_expander_width,
        stereo_balance_enabled,
        stereo_balance_balance,
        compressor_enabled,
        compressor_threshold,
        compressor_intensity,
        limiter_enabled,
        limiter_ceiling,
        reverb_enabled,
        reverb_wet,
        reverb_room_size,
        stereo_expander_mode,
    ) = audio_manager.with_dsp(|dsp| {
        (
            dsp.sub_bass.enabled,
            dsp.sub_bass.gain,
            dsp.mid_bass.enabled,
            dsp.mid_bass.gain,
            dsp.voice_boost.enabled,
            dsp.voice_boost.gain,
            dsp.noise_gate.enabled,
            dsp.noise_gate.threshold,
            dsp.stereo_expander.enabled,
            dsp.stereo_expander.width,
            dsp.stereo_balance.enabled,
            dsp.stereo_balance.balance,
            dsp.compressor.enabled,
            dsp.compressor.threshold,
            dsp.compressor.intensity,
            dsp.limiter.enabled,
            dsp.limiter.ceiling,
            dsp.reverb.enabled,
            dsp.reverb.wet,
            dsp.reverb.room_size,
            dsp.stereo_expander.mode,
        )
    });

    let state_arc = audio_manager.state();
    let audio_s = state_arc.read();

    // Construir Sub-Paneles manuales a través del helper.
    let col1 = column![
        view_effect(
            "Refuerzo de Sub-Graves",
            "Nivel dB",
            sub_bass_gain,
            -4.0..=24.0,
            sub_bass_enabled,
            0.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::SubBass,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::SubBass,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::SubBass,
                0.0
            )),
            None
        ),
        view_effect(
            "Reducción de Ruido",
            "Umbral dB",
            noise_gate_threshold,
            -85.0..=-10.0,
            noise_gate_enabled,
            -60.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::NoiseGate,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::NoiseGate,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::NoiseGate,
                -60.0
            )),
            None
        ),
        view_effect_with_secondary(
            "Compresor",
            "Umbral dB",
            compressor_threshold,
            -24.0..=0.0,
            compressor_enabled,
            -3.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::Compressor,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::Compressor,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::Compressor,
                -3.0
            )),
            None,
            Some(92.0),
            Some((
                compressor_intensity * 100.0,
                0.0..=100.0,
                Box::new(|v| crate::gui::app::Message::AudioCenterMsg(
                    AudioCenterMessage::DspValueChanged(DspEffect::CompressorIntensity, v)
                )),
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                    DspEffect::CompressorIntensity,
                    50.0
                )),
                "Intensidad %",
                1.0,
                "{:.1}",
            )),
            0.1,
            "{:.1}",
            0.0,
        ),
        container(
            container(
                Space::new()
                    .width(Length::Fill)
                    .height(Length::Fixed(STROKE_HAIRLINE))
            )
            .style(|_t: &Theme| container::Style::default().background(COLOR_TEXT_SECONDARY))
        )
        .padding(iced::Padding {
            top: SPACE_SM,
            right: SPACE_LG,
            bottom: SPACE_XS,
            left: SPACE_LG,
        }),
        view_effect_with_secondary(
            "Canal de Subwoofer",
            "Mix",
            audio_s.downmix_lfe,
            0.0..=2.0,
            audio_s.downmix_lfe_enabled,
            0.66,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateToggle(
                AudioStateToggle::DownmixLfe,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(
                AudioCenterMessage::AudioStateValueChanged(AudioStateToggle::DownmixLfe, v)
            ),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateValueChanged(
                AudioStateToggle::DownmixLfe,
                0.66
            )),
            None,
            None,
            None,
            0.01,
            "{:.2}",
            0.0,
        )
    ]
    .spacing(SPACE_LG)
    .width(Length::FillPortion(1));

    let col2 = column![
        view_effect(
            "Refuerzo de Graves",
            "Nivel dB",
            mid_bass_gain,
            -4.0..=15.0,
            mid_bass_enabled,
            0.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::MidBass,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::MidBass,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::MidBass,
                0.0
            )),
            None
        ),
        view_effect(
            "Expansor Estéreo",
            "Ancho %",
            stereo_expander_width * 100.0,
            0.0..=260.0,
            stereo_expander_enabled,
            100.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::StereoExpander,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::StereoExpander,
                v / 100.0
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::StereoExpander,
                1.0
            )),
            Some(
                row![
                    text("Natural").size(10).color(COLOR_TEXT_SECONDARY),
                    crate::gui::widgets::standard_toggler_full(
                        stereo_expander_mode == crate::audio::dsp::ExpanderMode::Surround,
                        |b| crate::gui::app::Message::AudioCenterMsg(
                            AudioCenterMessage::StereoExpanderModeToggled(b)
                        ),
                        13.0,
                        COLOR_TEXT_SECONDARY,
                        COLOR_TEXT_SECONDARY,
                        COLOR_TEXT_PRIMARY,
                        COLOR_TEXT_PRIMARY,
                        COLOR_TEXT_SECONDARY,
                    ),
                    text("Surround").size(10).color(COLOR_TEXT_SECONDARY),
                ]
                .spacing(SPACE_XS)
                .align_y(Alignment::Center)
                .into()
            )
        ),
        view_effect_with_secondary(
            "Limitador",
            "Techo dB",
            limiter_ceiling,
            -12.0..=0.0,
            limiter_enabled,
            -1.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::Limiter,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::Limiter,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::Limiter,
                -1.0
            )),
            None,
            Some(92.0),
            None,
            0.1,
            "{:.1}",
            12.0,
        ),
        container(
            text("Volumen de canales en mezcla menor a 5.1")
                .size(12)
                .color(COLOR_TEXT_PRIMARY)
                .font(FONT_INTER_SANS_MEDIUM)
        )
        .width(Length::Fill)
        .center_x(Length::Fill)
        .padding(iced::Padding {
            top: -2.0,
            right: 0.0,
            bottom: -2.0,
            left: 0.0,
        }),
        view_effect_with_secondary(
            "Canal Central",
            "Mix",
            audio_s.downmix_center,
            0.0..=2.0,
            audio_s.downmix_center_enabled,
            0.76,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateToggle(
                AudioStateToggle::DownmixCenter,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(
                AudioCenterMessage::AudioStateValueChanged(AudioStateToggle::DownmixCenter, v)
            ),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateValueChanged(
                AudioStateToggle::DownmixCenter,
                0.76
            )),
            None,
            None,
            None,
            0.01,
            "{:.2}",
            0.0,
        )
    ]
    .spacing(SPACE_LG)
    .width(Length::FillPortion(1));

    let col3 = column![
        view_effect(
            "Refuerzo de Voces",
            "Nivel dB",
            voice_boost_gain,
            -4.0..=13.0,
            voice_boost_enabled,
            0.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::VoiceBoost,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::VoiceBoost,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::VoiceBoost,
                0.0
            )),
            None
        ),
        view_effect(
            "Balance Estéreo",
            "Left | Right",
            stereo_balance_balance,
            -1.0..=1.0,
            stereo_balance_enabled,
            0.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::StereoBalance,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::StereoBalance,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::StereoBalance,
                0.0
            )),
            None
        ),
        view_effect_with_secondary(
            "Reverberación",
            "Mix",
            reverb_wet,
            0.0..=1.0,
            reverb_enabled,
            0.5,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::Reverb,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::Reverb,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::Reverb,
                0.5
            )),
            None,
            Some(92.0),
            Some((
                reverb_room_size,
                0.0..=1.0,
                Box::new(|v| crate::gui::app::Message::AudioCenterMsg(
                    AudioCenterMessage::DspValueChanged(DspEffect::ReverbRoomSize, v)
                )),
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                    DspEffect::ReverbRoomSize,
                    0.5
                )),
                "Tamaño",
                0.01,
                "{:.2}",
            )),
            0.01,
            "{:.2}",
            0.0,
        ),
        container(
            container(
                Space::new()
                    .width(Length::Fill)
                    .height(Length::Fixed(STROKE_HAIRLINE))
            )
            .style(|_t: &Theme| container::Style::default().background(COLOR_TEXT_SECONDARY))
        )
        .padding(iced::Padding {
            top: SPACE_SM,
            right: SPACE_LG,
            bottom: SPACE_XS,
            left: SPACE_LG,
        }),
        view_effect_with_secondary(
            "Canales Surround  SL/SR | SBL/SBR",
            "Mix",
            audio_s.downmix_surround,
            0.0..=2.0,
            audio_s.downmix_surround_enabled,
            0.74,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateToggle(
                AudioStateToggle::DownmixSurround,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(
                AudioCenterMessage::AudioStateValueChanged(AudioStateToggle::DownmixSurround, v)
            ),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateValueChanged(
                AudioStateToggle::DownmixSurround,
                0.74
            )),
            None,
            None,
            None,
            0.01,
            "{:.2}",
            0.0,
        )
    ]
    .spacing(SPACE_LG)
    .width(Length::FillPortion(1));

    column![
        Space::new().height(SPACE_LG),
        row![
            col1,
            Space::new().width(SPACE_LG),
            col2,
            Space::new().width(SPACE_LG),
            col3
        ]
        .width(Length::Fill),
    ]
    .padding(iced::Padding {
        top: SPACE_XS,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    })
    .into()
}
