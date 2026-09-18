use super::*;

use iced::widget::column;

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
