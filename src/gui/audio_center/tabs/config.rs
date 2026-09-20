//! Audio device configuration tab.

use crate::audio::AudioManager;
use crate::audio::device_manager::BitDepth;
use crate::gui::dimensions::{
    CONFIG_DROPDOWN_WIDE, CONFIG_DROPDOWN_WIDTH, CONFIG_FIELD_LABEL_WIDTH, CONFIG_INFO_LABEL_WIDTH,
    CONFIG_LABEL_STACK_SPACING, CONFIG_LABEL_WIDTH, CONFIG_SEPARATOR_COLUMN,
    CONFIG_TABLE_DIVIDER_COLUMN, CONFIG_TABLE_DIVIDER_HEIGHT, CONFIG_TABLE_LABEL_WIDTH,
    DIVIDER_THIN, PANEL_SEPARATOR_HEIGHT, SPACE_LG, SPACE_MD, SPACE_SM, SPACE_XS, STROKE_HAIRLINE,
};
use crate::gui::theme::*;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Color, Element, Length, Theme};
use std::sync::Arc;

use super::super::messages::{AudioCenterMessage, OptionWrapper, SystemSelection};
use super::super::state::AudioCenterManager;

// ==========================================
// VISTA: PESTAÑA 1 - CONFIGURACIÓN DE AUDIO
// ==========================================

pub(super) fn view_audio_config<'a>(
    manager: &'a AudioCenterManager,
    audio_manager: &'a Arc<AudioManager>,
) -> Element<'a, crate::gui::app::Message> {
    // Preparar opciones de Dropdowns
    let mut host_options = Vec::new();
    for host in &manager.cached_hosts {
        host_options.push(OptionWrapper::new(host.clone(), host.clone()));
    }
    let selected_host_opt = manager
        .selected_host
        .as_ref()
        .map(|h| OptionWrapper::new(h.clone(), h.clone()));

    let mut device_options = Vec::new();
    for dev in &manager.cached_devices {
        device_options.push(OptionWrapper::new(&dev.name, dev.name.clone()));
    }
    let selected_dev_opt = manager
        .selected_device
        .as_ref()
        .map(|d| OptionWrapper::new(d, d.clone()));

    let rates = [44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000];
    let rate_options: Vec<_> = rates
        .iter()
        .map(|&r| OptionWrapper::new(format!("{} Hz", r), Some(r)))
        .collect();
    let selected_rate_opt = manager
        .selected_sample_rate
        .map(|r| OptionWrapper::new(format!("{} Hz", r), Some(r)));

    let bits = [BitDepth::Bits16, BitDepth::Bits24, BitDepth::Bits32Float];
    let bit_options: Vec<_> = bits
        .iter()
        .map(|b| {
            let label = match b {
                BitDepth::Bits16 => "16 bits Int",
                BitDepth::Bits24 => "24 bits Int",
                BitDepth::Bits32Float => "32 bits Float",
            };
            OptionWrapper::new(label, b.clone())
        })
        .collect();
    let selected_bit_opt = {
        let label = match manager.selected_bit_depth {
            BitDepth::Bits16 => "16 bits Int",
            BitDepth::Bits24 => "24 bits Int",
            BitDepth::Bits32Float => "32 bits Float",
        };
        Some(OptionWrapper::new(
            label,
            manager.selected_bit_depth.clone(),
        ))
    };

    let channels_options = vec![
        OptionWrapper::new("1.0 Mono", 1),
        OptionWrapper::new("2.0 Stereo", 2),
        OptionWrapper::new("2.1 Stereo + Sub", 3),
        OptionWrapper::new("4.0 Quad", 4),
        OptionWrapper::new("5.1 Surround", 6),
        OptionWrapper::new("7.1 Surround", 8),
    ];
    let selected_ch_opt = channels_options
        .iter()
        .find(|o| o.value == manager.selected_channels_manual)
        .cloned();

    let mut buffer_options = vec![OptionWrapper::new("Automático", None)];
    let sizes = [256, 512, 1024, 2048, 4096, 8192];
    for &s in &sizes {
        buffer_options.push(OptionWrapper::new(format!("{}", s), Some(s)));
    }
    let selected_buf_opt = if let Some(s) = manager.selected_buffer_size {
        Some(OptionWrapper::new(format!("{}", s), Some(s)))
    } else {
        Some(OptionWrapper::new("Automático", None))
    };

    let host_dropdown = crate::gui::widgets::standard_pick_list(
        host_options,
        selected_host_opt,
        |o: OptionWrapper<String>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::HostSelected(o.value))
        },
        Length::Fixed(CONFIG_DROPDOWN_WIDTH),
    );
    let device_dropdown = crate::gui::widgets::standard_pick_list(
        device_options,
        selected_dev_opt,
        |o: OptionWrapper<String>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DeviceSelected(o.value))
        },
        Length::Fixed(CONFIG_DROPDOWN_WIDE),
    );
    let rate_dropdown = crate::gui::widgets::standard_pick_list(
        rate_options,
        selected_rate_opt,
        |o: OptionWrapper<Option<u32>>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SampleRateSelected(
                o.value,
            ))
        },
        Length::Fixed(CONFIG_DROPDOWN_WIDTH),
    );
    let bit_dropdown = crate::gui::widgets::standard_pick_list(
        bit_options,
        selected_bit_opt,
        |o: OptionWrapper<BitDepth>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::BitDepthSelected(o.value))
        },
        Length::Fixed(CONFIG_DROPDOWN_WIDTH),
    );
    let ch_dropdown = crate::gui::widgets::standard_pick_list(
        channels_options,
        selected_ch_opt,
        |o: OptionWrapper<u16>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::ChannelsManualSelected(
                o.value,
            ))
        },
        Length::Fixed(CONFIG_DROPDOWN_WIDTH),
    );
    let buffer_dropdown = crate::gui::widgets::standard_pick_list(
        buffer_options,
        selected_buf_opt,
        |o: OptionWrapper<Option<u32>>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::BufferSizeSelected(
                o.value,
            ))
        },
        Length::Fixed(CONFIG_DROPDOWN_WIDTH),
    );

    // --- PICKLISTS DE SISTEMA OPERATIVO (Pipewire | PulseAudio) ---
    let mut system_rate_options = vec![
        OptionWrapper::new("Default", SystemSelection::Default),
        OptionWrapper::new("Automatico", SystemSelection::Automatic),
    ];
    let sys_rates = [44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000];
    for &r in &sys_rates {
        system_rate_options.push(OptionWrapper::new(
            format!("{} Hz", r),
            SystemSelection::Fixed(r),
        ));
    }
    let selected_sys_rate_opt = Some(OptionWrapper::new(
        match manager.system_rate {
            SystemSelection::Default => "Default".to_string(),
            SystemSelection::Automatic => "Automatico".to_string(),
            SystemSelection::Fixed(r) => format!("{} Hz", r),
        },
        manager.system_rate,
    ));

    let mut system_quantum_options = vec![
        OptionWrapper::new("Default", SystemSelection::Default),
        OptionWrapper::new("Automatico", SystemSelection::Automatic),
    ];
    let sys_quantums = [256, 512, 1024, 2048, 4096, 8192];
    for &q in &sys_quantums {
        system_quantum_options.push(OptionWrapper::new(
            format!("{}", q),
            SystemSelection::Fixed(q),
        ));
    }
    let selected_sys_quantum_opt = Some(OptionWrapper::new(
        match manager.system_quantum {
            SystemSelection::Default => "Default".to_string(),
            SystemSelection::Automatic => "Automatico".to_string(),
            SystemSelection::Fixed(q) => format!("{}", q),
        },
        manager.system_quantum,
    ));

    let system_rate_dropdown = crate::gui::widgets::standard_pick_list(
        system_rate_options,
        selected_sys_rate_opt,
        |o: OptionWrapper<SystemSelection<u32>>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SystemRateSelected(
                o.value,
            ))
        },
        Length::Fixed(CONFIG_DROPDOWN_WIDTH),
    );

    let system_quantum_dropdown = crate::gui::widgets::standard_pick_list(
        system_quantum_options,
        selected_sys_quantum_opt,
        |o: OptionWrapper<SystemSelection<u32>>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SystemQuantumSelected(
                o.value,
            ))
        },
        Length::Fixed(CONFIG_DROPDOWN_WIDTH),
    );

    let system_config_label = column![
        text("Pipewire | PulseAudio")
            .color(COLOR_TEXT_PRIMARY)
            .size(14)
            .font(FONT_INTER_SANS_MEDIUM),
        text("Frecuencia | Quantum")
            .color(COLOR_TEXT_SECONDARY)
            .size(13)
            .font(FONT_INTER_SANS_MEDIUM),
    ]
    .spacing(CONFIG_LABEL_STACK_SPACING);

    let left_col = column![
        row![
            container(
                text("Núcleo de Audio:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(CONFIG_LABEL_WIDTH)),
            host_dropdown
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_MD),
        row![
            container(
                text("Dispositivo de Salida:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(CONFIG_LABEL_WIDTH)),
            device_dropdown
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_MD),
        row![
            container(
                text("Frecuencia de Muestreo:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(CONFIG_LABEL_WIDTH)),
            rate_dropdown
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_MD),
        row![
            container(
                text("Profundidad de Bits:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(CONFIG_LABEL_WIDTH)),
            bit_dropdown
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_MD),
        row![
            container(
                text("Canales de Salida:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(CONFIG_LABEL_WIDTH)),
            ch_dropdown
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_MD),
        row![
            container(
                text("Quantum (Buffer):")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(CONFIG_LABEL_WIDTH)),
            buffer_dropdown
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_MD),
        row![
            container(system_config_label).width(Length::Fixed(CONFIG_LABEL_WIDTH)),
            row![system_rate_dropdown, system_quantum_dropdown].spacing(SPACE_MD)
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_MD)
        .padding(iced::Padding {
            top: SPACE_XS,
            bottom: 0.0,
            left: 0.0,
            right: 0.0,
        }),
    ]
    .spacing(SPACE_LG);

    // Estado del Audio Derecho - Sacamos las variables del Guard inmediatamente
    let (
        sample_rate,
        channels,
        bit_depth_display,
        buffer_size,
        current_song_title,
        current_path,
        device_sample_rate,
    ) = {
        let state_arc = audio_manager.state();
        let state_read = state_arc.read();
        (
            state_read.sample_rate,
            state_read.channels,
            state_read.bit_depth_display.clone(),
            audio_manager.buffer_size(),
            state_read.title.clone(),
            state_read.path.clone(),
            state_read.device_sample_rate,
        )
    };

    // Estado del servidor de sonido del sistema (PipeWire / PulseAudio).
    // Cachear para no ejecutar pw-metadata cada frame (causaba corte de audio).
    // Nota: manager es immutable aquí, así que usamos el valor cacheado del último tick.
    let (system_sound_status, latency_str) = manager.cached_system_status.clone();

    let db_opt = audio_manager.get_database();

    let mut file_sample_rate = 0u32;
    let mut file_bit_depth = 0;
    let mut file_channels = 0;
    let mut track_gain = None;
    let mut album_gain = None;
    let mut file_format = String::new();

    if !current_path.is_empty() {
        if let Some(ref db_arc) = db_opt {
            // db-lock: solo lectura — refresco best-effort, se omite si el lock falla
            if let Ok(db) = db_arc.try_lock() {
                if let Ok(Some((sr, bd, ch))) = db.get_song_technical_meta_by_path(&current_path) {
                    if sr > 0 {
                        file_sample_rate = sr;
                    }
                    file_bit_depth = bd;
                    file_channels = ch;
                }
                if let Ok(Some(fmt)) = db.get_song_format_by_path(&current_path) {
                    file_format = fmt;
                }
                if let Ok((tg, ag)) = db.get_replay_gain_by_path(&current_path) {
                    track_gain = tg;
                    album_gain = ag;
                }
            }
        }
    }

    let file_sr_str = if file_sample_rate > 0 {
        format!("{} Hz", file_sample_rate)
    } else {
        "--".to_string()
    };

    let file_bd_str = if file_bit_depth > 0 {
        format!("{} bits", file_bit_depth)
    } else {
        "--".to_string()
    };

    let format_channels = |ch: u16| -> String {
        match ch {
            1 => "Mono".to_string(),
            2 => "Stereo".to_string(),
            3 => "Stereo + Sub".to_string(),
            4 => "4.0 Quad".to_string(),
            6 => "5.1 Surround".to_string(),
            8 => "7.1 Surround".to_string(),
            other => format!("{} canales", other),
        }
    };

    let file_ch_str = if file_channels > 0 {
        format_channels(file_channels as u16)
    } else {
        "--".to_string()
    };

    let output_ch_str = if channels > 0 {
        format_channels(channels)
    } else {
        "--".to_string()
    };

    let bit_depth_display_clean = match bit_depth_display.as_str() {
        "24/32-bit Int" | "24/32-bits Int" | "24-bit Int" | "24 bits Int" => {
            "24 bits Int".to_string()
        }
        other => other.replace("-bit", " bits"),
    };

    let gain_str = match (album_gain, track_gain) {
        (Some(ag), Some(tg)) => format!("{:.2} dB | {:.2} dB", ag, tg),
        (Some(ag), None) => format!("{:.2} dB | --", ag),
        (None, Some(tg)) => format!("-- | {:.2} dB", tg),
        (None, None) => "-- | --".to_string(),
    };

    let title_el = if current_song_title.is_empty() {
        text("Sin reproducción")
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM)
            .into()
    } else {
        crate::gui::widgets::smart_truncate_text(
            current_song_title,
            14.0,
            FONT_INTER_SANS_MEDIUM,
            COLOR_TEXT_PRIMARY,
        )
    };

    let quantum_repr = if buffer_size > 0 {
        format!("{}", buffer_size)
    } else {
        "Auto".to_string()
    };

    let labels_col = column![
        text("").size(12),
        text("Frecuencia:")
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text("Profundidad:")
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text("Canales:")
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
    ]
    .spacing(SPACE_SM);

    let entrada_col = column![
        text("Entrada")
            .size(12)
            .color(COLOR_TEXT_SECONDARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text(file_sr_str)
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text(file_bd_str)
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text(file_ch_str)
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
    ]
    .spacing(SPACE_SM)
    .align_x(Alignment::Center);

    let vertical_divider = container(
        Space::new()
            .width(Length::Fixed(DIVIDER_THIN))
            .height(Length::Fixed(CONFIG_TABLE_DIVIDER_HEIGHT)),
    )
    .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    let salida_col = column![
        text("Salida")
            .size(12)
            .color(COLOR_TEXT_SECONDARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text(format!("{} Hz", device_sample_rate))
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text(bit_depth_display_clean)
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text(output_ch_str)
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
    ]
    .spacing(SPACE_SM)
    .align_x(Alignment::Center);

    let table_row = row![
        container(labels_col).width(Length::Fixed(CONFIG_TABLE_LABEL_WIDTH)),
        container(entrada_col)
            .width(Length::FillPortion(1))
            .align_x(Alignment::Center),
        container(vertical_divider)
            .align_x(Alignment::Center)
            .width(Length::Fixed(CONFIG_TABLE_DIVIDER_COLUMN)),
        container(salida_col)
            .width(Length::FillPortion(1))
            .align_x(Alignment::Center),
    ]
    .align_y(Alignment::Center);

    let title_container = container(
        text("Información de Audio")
            .color(COLOR_TEXT_PRIMARY)
            .size(16)
            .font(FONT_INTER_SANS_MEDIUM),
    )
    .width(Length::Fill)
    .align_x(Alignment::Center);

    let right_col = column![
        title_container,
        row![
            container(
                text("Canción:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(CONFIG_FIELD_LABEL_WIDTH)),
            container(title_el).width(Length::Fill)
        ]
        .align_y(Alignment::Center),
        row![
            container(
                text("Formato:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(CONFIG_FIELD_LABEL_WIDTH)),
            text(if file_format.is_empty() {
                "--".to_string()
            } else {
                file_format
            })
            .color(COLOR_TEXT_PRIMARY)
            .size(14)
            .font(FONT_INTER_SANS_MEDIUM)
        ]
        .align_y(Alignment::Center),
        table_row,
        row![
            container(
                column![
                    text("Ganancia:")
                        .color(COLOR_TEXT_PRIMARY)
                        .size(14)
                        .font(FONT_INTER_SANS_MEDIUM),
                    text("Album | Cancion")
                        .color(COLOR_TEXT_SECONDARY)
                        .size(13)
                        .font(FONT_INTER_SANS_MEDIUM),
                ]
                .spacing(CONFIG_LABEL_STACK_SPACING)
            )
            .width(Length::Fixed(CONFIG_INFO_LABEL_WIDTH)),
            text(gain_str)
                .color(COLOR_TEXT_PRIMARY)
                .size(14)
                .font(FONT_INTER_SANS_MEDIUM)
        ]
        .align_y(Alignment::Center),
        row![
            container(
                text("Quantum de Audoxidy:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(CONFIG_INFO_LABEL_WIDTH)),
            text(quantum_repr)
                .color(COLOR_TEXT_PRIMARY)
                .size(14)
                .font(FONT_INTER_SANS_MEDIUM)
        ]
        .align_y(Alignment::Center),
        row![
            container(
                column![
                    text("Pipewire | PulseAudio:")
                        .color(COLOR_TEXT_PRIMARY)
                        .size(14)
                        .font(FONT_INTER_SANS_MEDIUM),
                    text("Frecuencia | Quantum")
                        .color(COLOR_TEXT_SECONDARY)
                        .size(13)
                        .font(FONT_INTER_SANS_MEDIUM),
                ]
                .spacing(CONFIG_LABEL_STACK_SPACING)
            )
            .width(Length::Fixed(CONFIG_INFO_LABEL_WIDTH)),
            column![
                text(system_sound_status)
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM),
                text(latency_str)
                    .color(COLOR_TEXT_SECONDARY)
                    .size(13)
                    .font(FONT_INTER_SANS_MEDIUM),
            ]
            .spacing(CONFIG_LABEL_STACK_SPACING)
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(SPACE_SM);

    let restart_btn = button(
        text("Reiniciar Servicio de Audio")
            .size(15)
            .font(FONT_INTER_SANS_MEDIUM),
    )
    .padding([SPACE_SM, SPACE_MD])
    .on_press(crate::gui::app::Message::AudioCenterMsg(
        AudioCenterMessage::RestartService,
    ))
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
    });

    let reset_btn = button(text("Predeterminado").size(15).font(FONT_INTER_SANS_MEDIUM))
        .padding([SPACE_SM, SPACE_MD])
        .on_press(crate::gui::app::Message::AudioCenterMsg(
            AudioCenterMessage::ResetToDefaults,
        ))
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
        });

    let apply_btn = if manager.apply_enabled {
        button(text("Aplicar").size(15).font(FONT_INTER_SANS_MEDIUM))
            .padding([SPACE_SM, SPACE_MD])
            .on_press(crate::gui::app::Message::AudioCenterMsg(
                AudioCenterMessage::ApplySettings,
            ))
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
            })
    } else {
        button(text("Aplicar").size(15).font(FONT_INTER_SANS_MEDIUM))
            .padding([SPACE_SM, SPACE_MD])
            .style(
                |_t: &Theme, _status: iced::widget::button::Status| button::Style {
                    background: Some(COLOR_CONTRAST.into()),
                    text_color: COLOR_TEXT_SECONDARY.scale_alpha(0.5),
                    border: iced::Border {
                        radius: 6.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                },
            )
    };

    let main_divider = container(
        Space::new()
            .width(Length::Fixed(DIVIDER_THIN))
            .height(Length::Fixed(PANEL_SEPARATOR_HEIGHT)),
    )
    .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    let bottom_actions = row![restart_btn, reset_btn, apply_btn].spacing(SPACE_MD);

    // Aviso no modal: la salida guardada ya no está y se usa la predeterminada.
    let notice: Element<'a, crate::gui::app::Message> =
        if let Some(msg) = audio_manager.get_state().audio_notice.clone() {
            text(msg).size(13).color(COLOR_ACCENT).into()
        } else {
            Space::new().height(Length::Fixed(0.0)).into()
        };

    column![
        row![
            container(left_col)
                .width(Length::FillPortion(5))
                .padding(iced::Padding {
                    top: SPACE_LG,
                    bottom: 0.0,
                    left: 0.0,
                    right: 0.0
                }),
            container(
                container(main_divider)
                    .height(Length::Fill)
                    .align_y(Alignment::Center)
            )
            .width(Length::Fixed(CONFIG_SEPARATOR_COLUMN))
            .align_x(Alignment::Center)
            .padding(iced::Padding {
                top: SPACE_SM,
                bottom: 0.0,
                left: 0.0,
                right: 0.0
            }),
            container(right_col)
                .width(Length::FillPortion(4))
                .padding(iced::Padding {
                    top: SPACE_LG,
                    bottom: 0.0,
                    left: SPACE_XS,
                    right: STROKE_HAIRLINE
                })
                .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
        ]
        .height(Length::Fill)
        .align_y(Alignment::Start),
        notice,
        bottom_actions
    ]
    .into()
}
