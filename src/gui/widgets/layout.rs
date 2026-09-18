use super::*;

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
                        angle: std::f32::consts::PI.into(), // 180 grados (Vertical)
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
