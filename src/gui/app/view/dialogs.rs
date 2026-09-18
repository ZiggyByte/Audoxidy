//! Modal dialog overlay rendering.

use crate::gui::theme::{
    COLOR_ACCENT, COLOR_BG, COLOR_CONTRAST, COLOR_TEXT_PRIMARY, COLOR_TEXT_SECONDARY,
};
use iced::{Color, Element};

use super::super::{ActiveDialog, AudoxidyApp, ExportFormat, ExportMode, Message};

pub(in crate::gui::app) static DIALOG_TEXT_INPUT_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(iced::widget::Id::unique);

impl AudoxidyApp {
    pub(super) fn dialog_overlay(&self) -> Option<Element<'_, Message>> {
        fn export_choice_button<'a>(
            label: &'a str,
            active: bool,
            msg: Message,
        ) -> iced::Element<'a, Message> {
            iced::widget::button(
                iced::widget::text(label)
                    .size(14)
                    .font(crate::gui::theme::FONT_INTER_SANS_MEDIUM),
            )
            .padding([5, 10])
            .on_press(msg)
            .style(move |_t, status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                iced::widget::button::Style {
                    background: if active {
                        Some(COLOR_ACCENT.into())
                    } else if is_hovered {
                        Some(COLOR_ACCENT.into())
                    } else {
                        Some(COLOR_CONTRAST.into())
                    },
                    text_color: if active {
                        COLOR_TEXT_PRIMARY
                    } else if is_hovered {
                        COLOR_TEXT_PRIMARY
                    } else {
                        COLOR_TEXT_SECONDARY
                    },
                    border: iced::Border {
                        radius: 6.0.into(),
                        width: 0.0,
                        color: if active {
                            iced::Color::TRANSPARENT
                        } else {
                            COLOR_TEXT_SECONDARY
                        },
                    },
                    ..Default::default()
                }
            })
            .into()
        }

        if self.active_dialog != ActiveDialog::None {
            let dialog_content = match &self.active_dialog {
                ActiveDialog::CreatePlaylist { name, .. } => {
                    crate::gui::widgets::standard_modal(
                        "Nueva lista de reproducción".to_string(),
                        iced::widget::column![
                            crate::gui::widgets::modal_text("Nombre para la nueva lista:".to_string()),
                            iced::widget::text_input("Nombre de la lista...", name)
                                .id(DIALOG_TEXT_INPUT_ID.clone())
                                .on_input(Message::UpdateDialogInput)
                                .on_submit(Message::ConfirmDialogAction)
                                .padding([4, 5])
                                .size(14)
                                .style(|_t: &iced::Theme, _status: iced::widget::text_input::Status| {
                                    iced::widget::text_input::Style {
                                        background: COLOR_CONTRAST.into(),
                                        border: iced::Border { radius: 4.0.into(), width: 1.0, color: COLOR_ACCENT },
                                        icon: Color::TRANSPARENT,
                                        placeholder: COLOR_TEXT_SECONDARY,
                                        value: COLOR_TEXT_PRIMARY,
                                        selection: COLOR_ACCENT,
                                    }
                                })
                        ].spacing(12).into(),
                        Some(Message::CloseDialog),
                        Some(Message::ConfirmDialogAction),
                        "Crear".to_string(),
                    )
                },
                ActiveDialog::RenamePlaylist { new_name, .. } => {
                    crate::gui::widgets::standard_modal(
                        "Renombrar lista".to_string(),
                        iced::widget::column![
                            crate::gui::widgets::modal_text("Nuevo nombre de la lista:".to_string()),
                            iced::widget::text_input("Nuevo nombre...", new_name)
                                .id(DIALOG_TEXT_INPUT_ID.clone())
                                .on_input(Message::UpdateDialogInput)
                                .on_submit(Message::ConfirmDialogAction)
                                .padding([4, 5])
                                .size(14)
                                .style(|_t: &iced::Theme, _status: iced::widget::text_input::Status| {
                                    iced::widget::text_input::Style {
                                        background: COLOR_CONTRAST.into(),
                                        border: iced::Border { radius: 4.0.into(), width: 1.0, color: COLOR_ACCENT },
                                        icon: Color::TRANSPARENT,
                                        placeholder: COLOR_TEXT_SECONDARY,
                                        value: COLOR_TEXT_PRIMARY,
                                        selection: COLOR_ACCENT,
                                    }
                                })
                        ].spacing(12).into(),
                        Some(Message::CloseDialog),
                        Some(Message::ConfirmDialogAction),
                        "Renombrar".to_string(),
                    )
                },
                ActiveDialog::DeleteConfirm { name, .. } => {
                    crate::gui::widgets::standard_modal(
                        "¿Eliminar lista?".to_string(),
                        crate::gui::widgets::modal_text(format!("¿Estás seguro de querer eliminar\nla lista \"{}\"?\nEsta acción no se puede deshacer.", name)),
                        Some(Message::CloseDialog),
                        Some(Message::ConfirmDialogAction),
                        "Eliminar".to_string(),
                    )
                },
                ActiveDialog::ExportConfirm { name, id: _, format, mode } => {
                    let format_choices = iced::widget::row![
                        export_choice_button("M3U", *format == ExportFormat::M3U, Message::SelectExportFormat(ExportFormat::M3U)),
                        export_choice_button("M3U8", *format == ExportFormat::M3U8, Message::SelectExportFormat(ExportFormat::M3U8)),
                    ].spacing(10).align_y(iced::Alignment::Center);

                    let mode_choices = iced::widget::row![
                        export_choice_button("Solo Lista", *mode == ExportMode::SingleFile, Message::SelectExportMode(ExportMode::SingleFile)),
                        export_choice_button("Carpeta Portable", *mode == ExportMode::PortableFolder, Message::SelectExportMode(ExportMode::PortableFolder)),
                    ].spacing(10).align_y(iced::Alignment::Center);

                    crate::gui::widgets::standard_modal(
                        "Guardar | Exportar lista".to_string(),
                        iced::widget::column![
                            crate::gui::widgets::modal_text(format!("Puedes guardar solo la lista o también\nexportar todas las canciones de la lista\n\"{}\".", name)),
                            iced::widget::Space::new().height(iced::Length::Fixed(0.0)),
                            crate::gui::widgets::modal_text("Formato de la lista:".to_string()),
                            format_choices,
                            iced::widget::Space::new().height(iced::Length::Fixed(0.0)),
                            crate::gui::widgets::modal_text("Modo:".to_string()),
                            mode_choices,
                        ].spacing(10).align_x(iced::Alignment::Center).into(),
                        Some(Message::CloseDialog),
                        Some(Message::ConfirmDialogAction),
                        "Continuar".to_string(),
                    )
                },
                ActiveDialog::EqPresetLoad { selected_preset_idx, .. } => {
                    let presets = self.audio_center_manager.get_unified_presets();
                    let custom_count = self.audio_center_manager.custom_presets.len();
                    let has_default = presets.first()
                        .map(|p| p.name == "Default")
                        .unwrap_or(false);
                    let selected_name = selected_preset_idx.and_then(|idx| {
                        presets.get(idx).map(|p| p.name.clone())
                    });

                    // --- LEFT COLUMN: Preset List ---
                    let mut preset_rows = iced::widget::column![].spacing(0);

                    for (idx, preset) in presets.iter().enumerate() {
                        if has_default && idx == 1 + custom_count {
                            preset_rows = preset_rows.push(
                                iced::widget::container(
                                    iced::widget::Space::new()
                                        .width(iced::Length::Fill)
                                        .height(iced::Length::Fixed(1.0)),
                                )
                                .width(iced::Length::Fill)
                                .style(|_t: &iced::Theme| iced::widget::container::Style::default()
                                    .background(COLOR_TEXT_SECONDARY)),
                            );
                        }

                        let is_selected = *selected_preset_idx == Some(idx);
                        let btn = iced::widget::button(
                            iced::widget::container(
                                iced::widget::text(&preset.name)
                                    .size(14)
                                    .color(COLOR_TEXT_PRIMARY)
                                    .font(crate::gui::theme::FONT_INTER_SANS_MEDIUM)
                                    .width(iced::Length::Fill)
                                    .wrapping(iced::widget::text::Wrapping::None),
                            )
                            .width(iced::Length::Fill)
                            .center_y(iced::Length::Fill),
                        )
                        .width(iced::Length::Fill)
                        .height(iced::Length::Fixed(26.0))
                        .padding([0, 8])
                        .style(move |_t: &iced::Theme, status: iced::widget::button::Status| {
                            let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                            iced::widget::button::Style {
                                background: if is_selected || is_hovered {
                                    Some(iced::Background::Color(COLOR_ACCENT))
                                } else {
                                    Some(iced::Background::Color(iced::Color::TRANSPARENT))
                                },
                                text_color: COLOR_TEXT_PRIMARY,
                                border: iced::Border {
                                    radius: 0.0.into(),
                                    width: 0.0,
                                    color: iced::Color::TRANSPARENT,
                                },
                                ..Default::default()
                            }
                        })
                        .on_press(crate::gui::app::Message::AudioCenterMsg(
                            crate::gui::audio_center::AudioCenterMessage::EqPresetLoadSelected(
                                Some(idx),
                            ),
                        ));
                        preset_rows = preset_rows.push(btn);
                    }

                    let preset_list = iced::widget::container(
                        crate::gui::widgets::standard_scrollable(
                            iced::widget::Id::new("eq-preset-load-list"),
                            preset_rows,
                            iced::widget::scrollable::Direction::Vertical(
                                iced::widget::scrollable::Scrollbar::default(),
                            ),
                        )
                        .height(iced::Length::Fixed(287.0)),
                    )
                    .width(iced::Length::Fixed(140.0))
                    .height(iced::Length::Fixed(289.0))
                    .padding(1)
                    .style(|_t: &iced::Theme| iced::widget::container::Style::default()
                        .background(COLOR_CONTRAST)
                        .border(iced::Border {
                            width: 1.0,
                            color: COLOR_TEXT_SECONDARY,
                            radius: 2.0.into(),
                        }));

                    // --- RIGHT COLUMN: Vertical Buttons ---
                    fn dialog_btn<'a>(
                        label: &'a str,
                        enabled: bool,
                        msg: crate::gui::app::Message,
                    ) -> iced::Element<'a, crate::gui::app::Message> {
                        let mut btn = iced::widget::button(
                            iced::widget::container(
                                iced::widget::text(label)
                                    .size(14)
                                    .font(crate::gui::theme::FONT_INTER_SANS_MEDIUM),
                            )
                            .width(iced::Length::Fill)
                            .center_x(iced::Length::Fill)
                            .center_y(iced::Length::Fill),
                        )
                        .width(iced::Length::Fixed(150.0))
                        .height(iced::Length::Fixed(30.0))
                        .padding(0)
                        .style(move |_t: &iced::Theme, status: iced::widget::button::Status| {
                            let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                            iced::widget::button::Style {
                                background: if is_hovered && enabled {
                                    Some(iced::Background::Color(COLOR_ACCENT))
                                } else if enabled {
                                    Some(iced::Background::Color(COLOR_CONTRAST))
                                } else {
                                    Some(iced::Background::Color(COLOR_CONTRAST))
                                },
                                text_color: if enabled {
                                    if is_hovered { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY }
                                } else {
                                    COLOR_TEXT_SECONDARY.scale_alpha(0.5)
                                },
                                border: iced::Border {
                                    radius: 6.0.into(),
                                    width: 0.0,
                                    color: iced::Color::TRANSPARENT,
                                },
                                ..Default::default()
                            }
                        });
                        if enabled {
                            btn = btn.on_press(msg);
                        }
                        btn.into()
                    }

                    let accept_msg = Message::ConfirmDialogAction;
                    let cancel_msg = Message::CloseDialog;
                    let delete_msg = selected_name.as_ref().map(|name| {
                        Message::AudioCenterMsg(
                            crate::gui::audio_center::AudioCenterMessage::EqPresetDelete(name.clone())
                        )
                    }).unwrap_or(Message::NoOp);
                    let import_msg = Message::EqPresetImportFile;
                    let export_msg = selected_name.as_ref().map(|name| {
                        Message::EqPresetExportFile(Some(name.clone()))
                    }).unwrap_or(Message::NoOp);
                    let restore_msg = Message::AudioCenterMsg(
                        crate::gui::audio_center::AudioCenterMessage::EqPresetRestoreDefaults
                    );

                    let right_col = iced::widget::column![
                        dialog_btn("Aceptar", true, accept_msg),
                        dialog_btn("Cancelar", true, cancel_msg),
                        dialog_btn("Eliminar", true, delete_msg),
                        dialog_btn("Importar", true, import_msg),
                        dialog_btn("Exportar", true, export_msg),
                        iced::widget::Space::new().height(iced::Length::Fixed(0.0)),
                        dialog_btn("Predeterminado", true, restore_msg),
                        iced::widget::Space::new().height(iced::Length::Fixed(0.0)),
                        iced::widget::text("Advertencia: Se elimarán los presets personalizados y se restablecerán los presets predeterminados.")
                            .size(11)
                            .color(COLOR_TEXT_SECONDARY)
                            .font(crate::gui::theme::FONT_INTER_SANS_MEDIUM),
                    ]
                    .spacing(8)
                    .align_x(iced::Alignment::Center)
                    .width(iced::Length::Fixed(150.0));

                    let content = iced::widget::row![
                        preset_list,
                        iced::widget::Space::new().width(iced::Length::Fixed(10.0)),
                        right_col,
                    ]
                    .spacing(0)
                    .align_y(iced::Alignment::Start);

                    // Build complete dialog with title
                    let dialog: iced::Element<'_, Message> = iced::widget::container(
                        iced::widget::column![
                            iced::widget::container(
                                iced::widget::text("Cargar Preset")
                                    .size(16)
                                    .font(crate::gui::theme::FONT_INTER_SANS_MEDIUM)
                                    .color(COLOR_TEXT_PRIMARY)
                                    .align_x(iced::alignment::Horizontal::Center),
                            )
                            .padding(iced::Padding {
                                top: -2.0,
                                bottom: 2.0,
                                left: 0.0,
                                right: 0.0,
                            }),
                            iced::widget::Space::new().height(iced::Length::Fixed(8.0)),
                            content,
                        ]
                        .align_x(iced::Alignment::Center),
                    )
                    .width(iced::Length::Fixed(330.0))
                    .height(iced::Length::Fixed(352.0))
                    .padding(17)
                    .style(|_t: &iced::Theme| iced::widget::container::Style::default()
                        .background(iced::Background::Color(COLOR_BG))
                        .border(iced::Border {
                            color: COLOR_ACCENT,
                            width: 2.0,
                            radius: 8.0.into(),
                        })
                        .shadow(iced::Shadow {
                            offset: iced::Vector::new(0.0, 10.0),
                            blur_radius: 30.0,
                            color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.8),
                        }))
                    .into();

                    // Wrap in mouse_area to prevent clicks inside from closing
                    iced::widget::mouse_area(dialog)
                        .interaction(iced::mouse::Interaction::Idle)
                        .into()
                },
                ActiveDialog::EqPresetSave { name_input } => {
                    let save_enabled = !name_input.is_empty();

                    let input = iced::widget::text_input("Nombre del preset...", name_input)
                        .id(DIALOG_TEXT_INPUT_ID.clone())
                        .on_input(Message::UpdateDialogInput)
                        .on_submit(Message::ConfirmDialogAction)
                        .padding([4, 5])
                        .size(14)
                        .style(|_t: &iced::Theme, _status: iced::widget::text_input::Status| {
                            iced::widget::text_input::Style {
                                background: COLOR_CONTRAST.into(),
                                border: iced::Border {
                                    radius: 4.0.into(),
                                    width: 1.0,
                                    color: COLOR_ACCENT,
                                },
                                icon: iced::Color::TRANSPARENT,
                                placeholder: COLOR_TEXT_SECONDARY,
                                value: COLOR_TEXT_PRIMARY,
                                selection: COLOR_ACCENT,
                            }
                        });

                    fn save_dialog_btn<'a>(
                        label: &'a str,
                        enabled: bool,
                        msg: Option<crate::gui::app::Message>,
                    ) -> iced::Element<'a, crate::gui::app::Message> {
                        let mut btn = iced::widget::button(
                            iced::widget::container(
                                iced::widget::text(label)
                                    .size(14)
                                    .font(crate::gui::theme::FONT_INTER_SANS_MEDIUM),
                            )
                            .width(iced::Length::Fill)
                            .center_x(iced::Length::Fill)
                            .center_y(iced::Length::Fill),
                        )
                        .width(iced::Length::Fixed(74.0))
                        .height(iced::Length::Fixed(30.0))
                        .padding(0)
                        .style(move |_t: &iced::Theme, status: iced::widget::button::Status| {
                            let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                            iced::widget::button::Style {
                                background: if is_hovered && enabled {
                                    Some(iced::Background::Color(COLOR_ACCENT))
                                } else {
                                    Some(iced::Background::Color(COLOR_CONTRAST))
                                },
                                text_color: if enabled {
                                    if is_hovered { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY }
                                } else {
                                    COLOR_TEXT_SECONDARY.scale_alpha(0.5)
                                },
                                border: iced::Border {
                                    radius: 6.0.into(),
                                    width: 0.0,
                                    color: iced::Color::TRANSPARENT,
                                },
                                ..Default::default()
                            }
                        });
                        if enabled {
                            if let Some(m) = msg {
                                btn = btn.on_press(m);
                            }
                        }
                        btn.into()
                    }

                    let mut content = iced::widget::column![
                        iced::widget::text("Nombre del preset:")
                            .size(13)
                            .color(COLOR_TEXT_PRIMARY)
                            .font(crate::gui::theme::FONT_INTER_SANS_MEDIUM)
                            .align_x(iced::alignment::Horizontal::Center),
                        iced::widget::Space::new().height(iced::Length::Fixed(12.0)),
                        input,
                        iced::widget::Space::new().height(iced::Length::Fixed(15.0)),
                        iced::widget::row![
                            save_dialog_btn("Cancelar", true, Some(Message::CloseDialog)),
                            iced::widget::Space::new().width(iced::Length::Fixed(8.0)),
                            save_dialog_btn(
                                "Guardar",
                                save_enabled,
                                if save_enabled { Some(Message::ConfirmDialogAction) } else { None },
                            ),
                        ]
                        .spacing(0)
                        .align_y(iced::Alignment::Center),
                    ]
                    .spacing(0)
                    .align_x(iced::Alignment::Center);

                    let dialog: iced::Element<'_, Message> = iced::widget::container(
                        iced::widget::column![
                            iced::widget::container(
                                iced::widget::text("Guardar Preset")
                                    .size(16)
                                    .font(crate::gui::theme::FONT_INTER_SANS_MEDIUM)
                                    .color(COLOR_TEXT_PRIMARY)
                                    .align_x(iced::alignment::Horizontal::Center),
                            )
                            .padding(iced::Padding {
                                top: 0.0,
                                bottom: 2.0,
                                left: 0.0,
                                right: 0.0,
                            }),
                            iced::widget::Space::new().height(iced::Length::Fixed(8.0)),
                            content,
                        ]
                        .align_x(iced::Alignment::Center),
                    )
                    .width(iced::Length::Fixed(190.0))
                    .height(iced::Length::Fixed(162.0))
                    .padding(15)
                    .style(|_t: &iced::Theme| iced::widget::container::Style::default()
                        .background(iced::Background::Color(COLOR_BG))
                        .border(iced::Border {
                            color: COLOR_ACCENT,
                            width: 2.0,
                            radius: 8.0.into(),
                        })
                        .shadow(iced::Shadow {
                            offset: iced::Vector::new(0.0, 10.0),
                            blur_radius: 30.0,
                            color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.8),
                        }))
                    .into();

                    iced::widget::mouse_area(dialog)
                        .interaction(iced::mouse::Interaction::Idle)
                        .into()
                },
                _ => iced::widget::Space::new().into(),
            };

            // Cálculo de posición dinámica según el tipo de diálogo
            let ac_width = 940.0;
            let ac_height = 480.0;
            let ac_pos = self.audio_center_manager.window_pos.unwrap_or_else(|| {
                let win_w = self.window_size.0 as f32;
                let win_h = self.window_size.1 as f32;
                iced::Point::new((win_w - ac_width) / 2.0, (win_h - ac_height) / 2.0)
            });
            let (target_x, target_y) = match &self.active_dialog {
                ActiveDialog::EqPresetLoad { .. } => {
                    let dialog_w = 330.0;
                    let dialog_h = 352.0;
                    (
                        ac_pos.x + (ac_width - dialog_w) / 2.0,
                        ac_pos.y + (ac_height - dialog_h) / 2.0 + 15.0,
                    )
                }
                ActiveDialog::EqPresetSave { .. } => {
                    let dialog_w = 194.0;
                    let x = ac_pos.x + ac_width - dialog_w - 15.0;
                    let y = ac_pos.y + 120.0;
                    (x, y)
                }
                _ => {
                    if let Some(pos) = self.dialog_pos {
                        let mut x = pos.x - 20.0;
                        let mut y = pos.y - 20.0;
                        if x + 350.0 + 40.0 > self.window_size.0 as f32 {
                            x = (self.window_size.0 as f32 - 350.0 - 40.0).max(10.0);
                        }
                        if y + 240.0 + 40.0 > self.window_size.1 as f32 {
                            y = (self.window_size.1 as f32 - 240.0 - 40.0).max(10.0);
                        }
                        (x, y)
                    } else {
                        (200.0, 200.0)
                    }
                }
            };

            // Área de bloqueo invisible - Fill para asegurar cobertura total
            let click_blocker = iced::widget::mouse_area(
                iced::widget::Space::new()
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill),
            )
            .on_press(Message::CloseDialog);
            let mut final_stack = iced::widget::Stack::new();
            final_stack = final_stack.push(click_blocker).push(
                iced::widget::container(dialog_content)
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill)
                    .padding(iced::Padding {
                        top: target_y,
                        left: target_x,
                        ..Default::default()
                    })
                    .align_x(iced::Alignment::Start)
                    .align_y(iced::Alignment::Start),
            );
            Some(final_stack.into())
        } else {
            None
        }
    }
}
