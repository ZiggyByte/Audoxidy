//! Volume and mix tab.

use crate::audio::AudioManager;
use crate::gui::dimensions::{
    DIVIDER_THIN, PANEL_SEPARATOR_COLUMN, PANEL_SEPARATOR_HEIGHT, SPACE_LG, SPACE_SM, SPACE_XL,
    SPACE_XS, STROKE_HAIRLINE,
};
use crate::gui::theme::*;
use iced::widget::{Space, column, container, mouse_area, row, text};
use iced::{Alignment, Element, Length, Theme};
use std::sync::Arc;

use super::super::messages::AudioCenterMessage;
use super::super::state::AudioCenterManager;

// ==========================================
// VISTA: PESTAÑA 4 - VOLUMEN Y MEZCLA
// ==========================================

pub(super) fn view_volumen_mezcla<'a>(
    manager: &'a AudioCenterManager,
    _audio_manager: &'a Arc<AudioManager>,
) -> Element<'a, crate::gui::app::Message> {
    use crate::gui::widgets::{NumberStepper, StandardCheckbox, StepperUnit};

    // Helper to build a horizontal separator line that fills remaining row space.
    let h_sep = || -> iced::Element<'a, crate::gui::app::Message> {
        container(Space::new().width(Length::Fill).height(STROKE_HAIRLINE))
            .style(|_t: &Theme| container::Style::default().background(COLOR_TEXT_SECONDARY))
            .into()
    };

    // --- Typography helpers ---
    // Group titles: 14px, primary text color.
    // Sub-function labels: 13px, primary when the function is enabled, secondary when not.
    // Centralizes the repetitive font/size/color styling of every label in this tab.
    let group_title = |name: &'a str| -> iced::Element<'a, crate::gui::app::Message> {
        text(name)
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM)
            .into()
    };
    let subfunc_label =
        |name: &'a str, enabled: bool| -> iced::Element<'a, crate::gui::app::Message> {
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
    // Etiqueta de sub-grupo (ayuda visual sin checkbox): 14px, mismo esquema de color.
    let subgroup_label =
        |name: &'a str, enabled: bool| -> iced::Element<'a, crate::gui::app::Message> {
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

    // Wraps a label in a clickable area that toggles a function on click
    // (clicking the group/sub-function name toggles its checkbox).
    let clickable_toggle = |label: iced::Element<'a, crate::gui::app::Message>,
                            msg: crate::gui::app::Message|
     -> iced::Element<'a, crate::gui::app::Message> {
        mouse_area(label).on_press(msg).into()
    };

    // --- Group 1: Fades ---
    let fades_master_on = manager.volumen_fades_enabled;

    let fades_title_row = row![
        {
            let chk: iced::Element<'_, _> = StandardCheckbox::new(fades_master_on, |b| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenFadesToggle(b))
            })
            .into();
            chk
        },
        Space::new().width(SPACE_XS),
        clickable_toggle(
            group_title("Desvanecimiento de Volumen"),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenFadesToggle(
                !fades_master_on,
            )),
        ),
        Space::new().width(SPACE_SM),
        h_sep(),
    ]
    .align_y(Alignment::Center);

    // Sub-row: Suavizar el cambio de volumen (individual checkbox, gated by group master)
    let smooth_row = {
        let chk: iced::Element<'_, _> = if fades_master_on {
            StandardCheckbox::new(manager.volumen_smooth_volume_enabled, |b| {
                crate::gui::app::Message::AudioCenterMsg(
                    AudioCenterMessage::VolumenSmoothVolumeToggle(b),
                )
            })
            .into()
        } else {
            StandardCheckbox::new(false, |_| crate::gui::app::Message::NoOp).into()
        };
        row![
            chk,
            Space::new().width(SPACE_XS),
            clickable_toggle(
                subfunc_label(
                    "Suavizar el cambio de volumen",
                    fades_master_on && manager.volumen_smooth_volume_enabled,
                ),
                if fades_master_on {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenSmoothVolumeToggle(
                            !manager.volumen_smooth_volume_enabled,
                        ),
                    )
                } else {
                    crate::gui::app::Message::NoOp
                },
            ),
            Space::new().width(Length::Fill),
        ]
        .align_y(Alignment::Center)
    };

    let fade_in_row = {
        let disabled = !fades_master_on || !manager.volumen_fade_in_enabled;
        let chk: iced::Element<'_, _> = if fades_master_on {
            StandardCheckbox::new(manager.volumen_fade_in_enabled, |b| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenFadeInToggle(b))
            })
            .into()
        } else {
            StandardCheckbox::new(false, |_| crate::gui::app::Message::NoOp).into()
        };
        let stepper: iced::Element<'_, _> = if disabled {
            NumberStepper::new(
                manager.volumen_fade_in_ms,
                0.0..=10000.0,
                StepperUnit::Milliseconds,
                |_| crate::gui::app::Message::NoOp,
            )
            .disabled()
            .into()
        } else {
            NumberStepper::new(
                manager.volumen_fade_in_ms,
                0.0..=10000.0,
                StepperUnit::Milliseconds,
                |v| {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenFadeInChanged(v),
                    )
                },
            )
            .on_selected_state_change(|active| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(
                    active,
                ))
            })
            .on_right_click(|| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenFadeInChanged(
                    1000.0,
                ))
            })
            .into()
        };
        row![
            chk,
            Space::new().width(SPACE_XS),
            clickable_toggle(
                subfunc_label(
                    "Desvanecimiento de volumen al iniciar la cancion",
                    fades_master_on && manager.volumen_fade_in_enabled,
                ),
                if fades_master_on {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenFadeInToggle(!manager.volumen_fade_in_enabled),
                    )
                } else {
                    crate::gui::app::Message::NoOp
                },
            ),
            Space::new().width(Length::Fill),
            stepper,
        ]
        .align_y(Alignment::Center)
    };

    let fade_out_row = {
        let disabled = !fades_master_on || !manager.volumen_fade_out_enabled;
        let chk: iced::Element<'_, _> = if fades_master_on {
            StandardCheckbox::new(manager.volumen_fade_out_enabled, |b| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenFadeOutToggle(
                    b,
                ))
            })
            .into()
        } else {
            StandardCheckbox::new(false, |_| crate::gui::app::Message::NoOp).into()
        };
        let stepper: iced::Element<'_, _> = if disabled {
            NumberStepper::new(
                manager.volumen_fade_out_ms,
                0.0..=10000.0,
                StepperUnit::Milliseconds,
                |_| crate::gui::app::Message::NoOp,
            )
            .disabled()
            .into()
        } else {
            NumberStepper::new(
                manager.volumen_fade_out_ms,
                0.0..=10000.0,
                StepperUnit::Milliseconds,
                |v| {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenFadeOutChanged(v),
                    )
                },
            )
            .on_selected_state_change(|active| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(
                    active,
                ))
            })
            .on_right_click(|| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenFadeOutChanged(
                    2000.0,
                ))
            })
            .into()
        };
        row![
            chk,
            Space::new().width(SPACE_XS),
            clickable_toggle(
                subfunc_label(
                    "Desvanecimiento de volumen al terminar la cancion",
                    fades_master_on && manager.volumen_fade_out_enabled,
                ),
                if fades_master_on {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenFadeOutToggle(!manager.volumen_fade_out_enabled),
                    )
                } else {
                    crate::gui::app::Message::NoOp
                },
            ),
            Space::new().width(Length::Fill),
            stepper,
        ]
        .align_y(Alignment::Center)
    };

    let fades_group =
        column![fades_title_row, smooth_row, fade_in_row, fade_out_row,].spacing(SPACE_XS);

    // --- Group 2: Silencio ---
    let silence_master_on = manager.volumen_silence_enabled;

    let silence_title_row = row![
        {
            let chk: iced::Element<'_, _> = StandardCheckbox::new(silence_master_on, |b| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenSilenceToggle(
                    b,
                ))
            })
            .into();
            chk
        },
        Space::new().width(SPACE_XS),
        clickable_toggle(
            group_title("Eliminar silencio"),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenSilenceToggle(
                !silence_master_on,
            )),
        ),
        Space::new().width(SPACE_SM),
        h_sep(),
    ]
    .align_y(Alignment::Center);

    let silence_duration_row = {
        let stepper: iced::Element<'_, _> = if !silence_master_on {
            NumberStepper::new(
                manager.volumen_silence_duration_ms,
                100.0..=10000.0,
                StepperUnit::Milliseconds,
                |_| crate::gui::app::Message::NoOp,
            )
            .disabled()
            .into()
        } else {
            NumberStepper::new(
                manager.volumen_silence_duration_ms,
                100.0..=10000.0,
                StepperUnit::Milliseconds,
                |v| {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenSilenceDurationChanged(v),
                    )
                },
            )
            .on_selected_state_change(|active| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(
                    active,
                ))
            })
            .on_right_click(|| {
                crate::gui::app::Message::AudioCenterMsg(
                    AudioCenterMessage::VolumenSilenceDurationChanged(1000.0),
                )
            })
            .into()
        };
        row![
            Space::new().width(SPACE_SM),
            subfunc_label("Eliminar los silencios mayores a:", silence_master_on,),
            Space::new().width(Length::Fill),
            stepper,
        ]
        .align_y(Alignment::Center)
    };

    let silence_threshold_row = {
        let stepper: iced::Element<'_, _> = if !silence_master_on {
            NumberStepper::new(
                manager.volumen_silence_threshold_db,
                (-80.0)..=0.0,
                StepperUnit::Decibels,
                |_| crate::gui::app::Message::NoOp,
            )
            .disabled()
            .into()
        } else {
            NumberStepper::new(
                manager.volumen_silence_threshold_db,
                (-80.0)..=0.0,
                StepperUnit::Decibels,
                |v| {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenSilenceThresholdChanged(v),
                    )
                },
            )
            .on_selected_state_change(|active| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(
                    active,
                ))
            })
            .on_right_click(|| {
                crate::gui::app::Message::AudioCenterMsg(
                    AudioCenterMessage::VolumenSilenceThresholdChanged(-47.0),
                )
            })
            .into()
        };
        row![
            Space::new().width(SPACE_SM),
            subfunc_label(
                "Umbral de eliminación de silencios menores a:",
                silence_master_on
            ),
            Space::new().width(Length::Fill),
            stepper,
        ]
        .align_y(Alignment::Center)
    };

    let silence_border_row = {
        let chk: iced::Element<'_, _> = if silence_master_on {
            StandardCheckbox::new(manager.volumen_silence_edge_trim_enabled, |b| {
                crate::gui::app::Message::AudioCenterMsg(
                    AudioCenterMessage::VolumenSilenceEdgeTrimToggle(b),
                )
            })
            .into()
        } else {
            StandardCheckbox::new(false, |_| crate::gui::app::Message::NoOp).into()
        };
        row![
            chk,
            Space::new().width(SPACE_XS),
            clickable_toggle(
                subfunc_label(
                    "Eliminar silencios de inicio y fin de las canciones",
                    silence_master_on && manager.volumen_silence_edge_trim_enabled,
                ),
                if silence_master_on {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenSilenceEdgeTrimToggle(
                            !manager.volumen_silence_edge_trim_enabled,
                        ),
                    )
                } else {
                    crate::gui::app::Message::NoOp
                },
            ),
            Space::new().width(Length::Fill),
        ]
        .align_y(Alignment::Center)
    };

    let silence_group = column![
        silence_title_row,
        silence_duration_row,
        silence_threshold_row,
        silence_border_row,
    ]
    .spacing(SPACE_XS);

    // --- Group 3: Replay gain fijo ---
    // Igual que "gain para canciones sin etiqueta" pero se aplica SIEMPRE
    // (sin la condición de ausencia de etiquetas).
    let rg_fixed_on = manager.volumen_rg_fixed_enabled;

    let rg_fixed_title_row = row![
        {
            let chk: iced::Element<'_, _> = StandardCheckbox::new(rg_fixed_on, |b| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenRgFixedToggle(
                    b,
                ))
            })
            .into();
            chk
        },
        Space::new().width(SPACE_XS),
        clickable_toggle(
            group_title("Pre-Amplificador"),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenRgFixedToggle(
                !rg_fixed_on,
            )),
        ),
        Space::new().width(SPACE_SM),
        h_sep(),
    ]
    .align_y(Alignment::Center);

    let rg_fixed_row = {
        let stepper: iced::Element<'_, _> = if !rg_fixed_on {
            NumberStepper::new(
                manager.volumen_rg_fixed_db,
                (-30.0)..=12.0,
                StepperUnit::Decibels,
                |_| crate::gui::app::Message::NoOp,
            )
            .disabled()
            .into()
        } else {
            NumberStepper::new(
                manager.volumen_rg_fixed_db,
                (-30.0)..=12.0,
                StepperUnit::Decibels,
                |v| {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenRgFixedChanged(v),
                    )
                },
            )
            .on_selected_state_change(|active| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(
                    active,
                ))
            })
            .on_right_click(|| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenRgFixedChanged(
                    0.0,
                ))
            })
            .into()
        };
        row![
            Space::new().width(SPACE_SM),
            subfunc_label("Nivel del Pre-Amplificador", rg_fixed_on),
            Space::new().width(Length::Fill),
            stepper,
        ]
        .align_y(Alignment::Center)
    };

    let rg_fixed_group = column![rg_fixed_title_row, rg_fixed_row,].spacing(SPACE_XS);

    // --- Group 4: Replay Gain ---
    let rg_master_on = manager.volumen_rg_master_enabled;

    let rg_title_row = row![
        {
            let chk: iced::Element<'_, _> = StandardCheckbox::new(rg_master_on, |b| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenRgMasterToggle(
                    b,
                ))
            })
            .into();
            chk
        },
        Space::new().width(SPACE_XS),
        clickable_toggle(
            group_title("Replay Gain"),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenRgMasterToggle(
                !rg_master_on,
            )),
        ),
        Space::new().width(SPACE_SM),
        h_sep(),
    ]
    .align_y(Alignment::Center);

    let rg_album_row = {
        // The checkbox is clickable whenever the group master is ON (regardless of
        // its own individual state). The stepper is disabled when the master is OFF
        // OR this specific function is OFF.
        let checkbox_locked = !rg_master_on;
        let stepper_locked = !rg_master_on || !manager.volumen_rg_album_enabled;
        let chk: iced::Element<'_, _> = if !checkbox_locked {
            StandardCheckbox::new(manager.volumen_rg_album_enabled, |b| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenRgAlbumToggle(
                    b,
                ))
            })
            .into()
        } else {
            StandardCheckbox::new(false, |_| crate::gui::app::Message::NoOp).into()
        };
        let stepper: iced::Element<'_, _> = if stepper_locked {
            NumberStepper::new(
                manager.volumen_rg_offset_album_db,
                (-12.0)..=12.0,
                StepperUnit::Decibels,
                |_| crate::gui::app::Message::NoOp,
            )
            .disabled()
            .into()
        } else {
            NumberStepper::new(
                manager.volumen_rg_offset_album_db,
                (-12.0)..=12.0,
                StepperUnit::Decibels,
                |v| {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenRgOffsetAlbumChanged(v),
                    )
                },
            )
            .on_selected_state_change(|active| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(
                    active,
                ))
            })
            .on_right_click(|| {
                crate::gui::app::Message::AudioCenterMsg(
                    AudioCenterMessage::VolumenRgOffsetAlbumChanged(0.0),
                )
            })
            .into()
        };
        row![
            chk,
            Space::new().width(SPACE_XS),
            clickable_toggle(
                subfunc_label(
                    "Usar valor de la etiqueta incrustada 'Album'",
                    !stepper_locked
                ),
                if !checkbox_locked {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenRgAlbumToggle(!manager.volumen_rg_album_enabled),
                    )
                } else {
                    crate::gui::app::Message::NoOp
                },
            ),
            Space::new().width(Length::Fill),
            stepper,
        ]
        .align_y(Alignment::Center)
    };

    let rg_track_row = {
        let checkbox_locked = !rg_master_on;
        let stepper_locked = !rg_master_on || !manager.volumen_rg_track_enabled;
        let chk: iced::Element<'_, _> = if !checkbox_locked {
            StandardCheckbox::new(manager.volumen_rg_track_enabled, |b| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::VolumenRgTrackToggle(
                    b,
                ))
            })
            .into()
        } else {
            StandardCheckbox::new(false, |_| crate::gui::app::Message::NoOp).into()
        };
        let stepper: iced::Element<'_, _> = if stepper_locked {
            NumberStepper::new(
                manager.volumen_rg_offset_track_db,
                (-12.0)..=12.0,
                StepperUnit::Decibels,
                |_| crate::gui::app::Message::NoOp,
            )
            .disabled()
            .into()
        } else {
            NumberStepper::new(
                manager.volumen_rg_offset_track_db,
                (-12.0)..=12.0,
                StepperUnit::Decibels,
                |v| {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenRgOffsetTrackChanged(v),
                    )
                },
            )
            .on_selected_state_change(|active| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(
                    active,
                ))
            })
            .on_right_click(|| {
                crate::gui::app::Message::AudioCenterMsg(
                    AudioCenterMessage::VolumenRgOffsetTrackChanged(0.0),
                )
            })
            .into()
        };
        row![
            chk,
            Space::new().width(SPACE_XS),
            clickable_toggle(
                subfunc_label(
                    "Usar valor de la etiqueta incrustada 'Canción'",
                    !stepper_locked
                ),
                if !checkbox_locked {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenRgTrackToggle(!manager.volumen_rg_track_enabled),
                    )
                } else {
                    crate::gui::app::Message::NoOp
                },
            ),
            Space::new().width(Length::Fill),
            stepper,
        ]
        .align_y(Alignment::Center)
    };

    let rg_untagged_row = {
        let checkbox_locked = !rg_master_on;
        let stepper_locked = !rg_master_on || !manager.volumen_rg_analyze_rt_enabled;
        let chk: iced::Element<'_, _> = if !checkbox_locked {
            StandardCheckbox::new(manager.volumen_rg_analyze_rt_enabled, |b| {
                crate::gui::app::Message::AudioCenterMsg(
                    AudioCenterMessage::VolumenRgAnalyzeRtToggle(b),
                )
            })
            .into()
        } else {
            StandardCheckbox::new(false, |_| crate::gui::app::Message::NoOp).into()
        };
        let stepper: iced::Element<'_, _> = if stepper_locked {
            NumberStepper::new(
                manager.volumen_rg_offset_rt_db,
                (-12.0)..=12.0,
                StepperUnit::Decibels,
                |_| crate::gui::app::Message::NoOp,
            )
            .disabled()
            .into()
        } else {
            NumberStepper::new(
                manager.volumen_rg_offset_rt_db,
                (-12.0)..=12.0,
                StepperUnit::Decibels,
                |v| {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenRgOffsetRtChanged(v),
                    )
                },
            )
            .on_selected_state_change(|active| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(
                    active,
                ))
            })
            .on_right_click(|| {
                crate::gui::app::Message::AudioCenterMsg(
                    AudioCenterMessage::VolumenRgOffsetRtChanged(0.0),
                )
            })
            .into()
        };
        row![
            chk,
            Space::new().width(SPACE_XS),
            clickable_toggle(
                subfunc_label(
                    "Replay Gain fijo para canciones sin etiquetas",
                    !stepper_locked
                ),
                if !checkbox_locked {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::VolumenRgAnalyzeRtToggle(
                            !manager.volumen_rg_analyze_rt_enabled,
                        ),
                    )
                } else {
                    crate::gui::app::Message::NoOp
                },
            ),
            Space::new().width(Length::Fill),
            stepper,
        ]
        .align_y(Alignment::Center)
    };

    let rg_group =
        column![rg_title_row, rg_album_row, rg_track_row, rg_untagged_row,].spacing(SPACE_XS);

    // Left column: all groups
    let left_col =
        container(column![fades_group, silence_group, rg_fixed_group, rg_group,].spacing(SPACE_LG))
            .width(Length::FillPortion(1));

    // --- Right column: Crossfade (Mezcla Cruzada) ---
    // Replica el patrón de la columna izquierda: checkbox master + título clicable
    // + separador horizontal; sub-grupos como etiquetas visuales; sub-funciones con
    // checkbox + nombre clicable + NumberStepper alineado a la derecha.
    let xfade_master_on = manager.crossfade_enabled;

    let xfade_title_row = row![
        {
            let chk: iced::Element<'_, _> = StandardCheckbox::new(xfade_master_on, |b| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::CrossfadeToggle(b))
            })
            .into();
            chk
        },
        Space::new().width(SPACE_XS),
        clickable_toggle(
            group_title("Mezcla Cruzada"),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::CrossfadeToggle(
                !xfade_master_on,
            )),
        ),
        Space::new().width(SPACE_SM),
        h_sep(),
    ]
    .align_y(Alignment::Center);

    // Sub-grupo: "Cambio Manual" — checkbox + texto clicable (patrón Pre-Amplificador).
    let xfade_manual_label = {
        let chk: iced::Element<'_, _> = if xfade_master_on {
            StandardCheckbox::new(manager.crossfade_manual_enabled, |b| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::CrossfadeManualToggle(
                    b,
                ))
            })
            .into()
        } else {
            StandardCheckbox::new(false, |_| crate::gui::app::Message::NoOp).into()
        };
        row![
            chk,
            Space::new().width(SPACE_XS),
            clickable_toggle(
                subgroup_label(
                    "Cambio manual:",
                    xfade_master_on && manager.crossfade_manual_enabled
                ),
                if xfade_master_on {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::CrossfadeManualToggle(
                            !manager.crossfade_manual_enabled,
                        ),
                    )
                } else {
                    crate::gui::app::Message::NoOp
                },
            ),
            Space::new().width(Length::Fill),
        ]
        .align_y(Alignment::Center)
    };

    let xfade_manual_row = {
        let stepper_locked = !xfade_master_on || !manager.crossfade_manual_enabled;
        let stepper: iced::Element<'_, _> = if stepper_locked {
            NumberStepper::new(
                manager.crossfade_manual_ms,
                0.0..=10000.0,
                StepperUnit::Milliseconds,
                |_| crate::gui::app::Message::NoOp,
            )
            .disabled()
            .into()
        } else {
            NumberStepper::new(
                manager.crossfade_manual_ms,
                0.0..=10000.0,
                StepperUnit::Milliseconds,
                |v| {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::CrossfadeManualChanged(v),
                    )
                },
            )
            .on_selected_state_change(|active| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(
                    active,
                ))
            })
            .on_right_click(|| {
                crate::gui::app::Message::AudioCenterMsg(
                    AudioCenterMessage::CrossfadeManualChanged(1000.0),
                )
            })
            .into()
        };
        row![
            Space::new().width(SPACE_SM),
            subfunc_label(
                "Mezclar canción actual con la siguiente",
                xfade_master_on && manager.crossfade_manual_enabled,
            ),
            Space::new().width(Length::Fill),
            stepper,
        ]
        .align_y(Alignment::Center)
    };

    // Sub-grupo: "Cambio Automático" — checkbox + texto clicable (patrón Pre-Amplificador).
    let xfade_auto_label = {
        let chk: iced::Element<'_, _> = if xfade_master_on {
            StandardCheckbox::new(manager.crossfade_auto_enabled, |b| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::CrossfadeAutoToggle(b))
            })
            .into()
        } else {
            StandardCheckbox::new(false, |_| crate::gui::app::Message::NoOp).into()
        };
        row![
            chk,
            Space::new().width(SPACE_XS),
            clickable_toggle(
                subgroup_label(
                    "Cambio automático:",
                    xfade_master_on && manager.crossfade_auto_enabled
                ),
                if xfade_master_on {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::CrossfadeAutoToggle(!manager.crossfade_auto_enabled),
                    )
                } else {
                    crate::gui::app::Message::NoOp
                },
            ),
            Space::new().width(Length::Fill),
        ]
        .align_y(Alignment::Center)
    };

    let xfade_auto_row = {
        let stepper_locked = !xfade_master_on || !manager.crossfade_auto_enabled;
        let stepper: iced::Element<'_, _> = if stepper_locked {
            NumberStepper::new(
                manager.crossfade_auto_ms,
                0.0..=10000.0,
                StepperUnit::Milliseconds,
                |_| crate::gui::app::Message::NoOp,
            )
            .disabled()
            .into()
        } else {
            NumberStepper::new(
                manager.crossfade_auto_ms,
                0.0..=10000.0,
                StepperUnit::Milliseconds,
                |v| {
                    crate::gui::app::Message::AudioCenterMsg(
                        AudioCenterMessage::CrossfadeAutoChanged(v),
                    )
                },
            )
            .on_selected_state_change(|active| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(
                    active,
                ))
            })
            .on_right_click(|| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::CrossfadeAutoChanged(
                    250.0,
                ))
            })
            .into()
        };
        row![
            Space::new().width(SPACE_SM),
            subfunc_label(
                "Mezclar canción actual con la siguiente",
                xfade_master_on && manager.crossfade_auto_enabled,
            ),
            Space::new().width(Length::Fill),
            stepper,
        ]
        .align_y(Alignment::Center)
    };

    let xfade_group = column![
        xfade_title_row,
        xfade_manual_label,
        xfade_manual_row,
        xfade_auto_label,
        xfade_auto_row,
    ]
    .spacing(SPACE_XS);

    // Vertical separator
    let vertical_separator = container(Space::new())
        .width(DIVIDER_THIN)
        .height(PANEL_SEPARATOR_HEIGHT)
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    let right_col = container(column![xfade_group].spacing(SPACE_LG)).width(Length::FillPortion(1));

    column![
        row![
            container(left_col)
                .width(Length::FillPortion(5))
                .padding(iced::Padding {
                    top: 0.0,
                    bottom: 0.0,
                    left: 0.0,
                    right: SPACE_XS
                }),
            container(
                container(vertical_separator)
                    .height(Length::Fill)
                    .align_y(Alignment::Center)
            )
            .width(PANEL_SEPARATOR_COLUMN)
            .align_x(Alignment::Center)
            .padding(iced::Padding {
                top: -52.0,
                bottom: 0.0,
                left: -SPACE_XS,
                right: 0.0
            }),
            container(right_col)
                .width(Length::FillPortion(4))
                .padding(iced::Padding {
                    top: 0.0,
                    bottom: 0.0,
                    left: -STROKE_HAIRLINE,
                    right: 0.0
                })
        ]
        .height(Length::Fill)
        .align_y(Alignment::Start),
    ]
    .padding(iced::Padding {
        top: SPACE_XL,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    })
    .into()
}
