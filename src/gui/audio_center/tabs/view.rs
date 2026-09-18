//! Audio center panel view composition and tab dispatch.

use crate::audio::AudioManager;
use crate::gui::theme::*;
use iced::{
    Alignment, Color, Element, Length, Theme,
    widget::{Space, button, column, container, mouse_area, opaque, row, svg, text},
};
use std::sync::Arc;

use super::super::messages::AudioCenterMessage;
use super::super::state::AudioCenterManager;
use super::config::view_audio_config;
use super::effects::view_audio_effects;
use super::equalizer::view_equalizer;
use super::volumen::view_volumen_mezcla;

pub fn view<'a>(
    manager: &'a AudioCenterManager,
    audio_manager: &'a Arc<AudioManager>,
) -> Element<'a, crate::gui::app::Message> {
    // 1. Logotipo de marca estilizado con la tipografía "Stage Wander" a 18px
    let logo = row![
        text("A")
            .color(COLOR_ACCENT)
            .font(FONT_STAGE_WANDER)
            .size(18),
        text("uD")
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_STAGE_WANDER)
            .size(18),
        text("o")
            .color(COLOR_ACCENT)
            .font(FONT_STAGE_WANDER)
            .size(18),
        text("xiD")
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_STAGE_WANDER)
            .size(18),
        text("Y")
            .color(COLOR_ACCENT)
            .font(FONT_STAGE_WANDER)
            .size(18),
    ]
    .spacing(0);

    // Logo con padding-top 9px
    let logo = container(logo).padding(iced::Padding {
        top: 9.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    });

    // 2. Botón de cerrar con icono SVG "close-big.svg" de 24px
    let close_icon = svg(iced::widget::svg::Handle::from_path(
        "assets/icons/close-big.svg",
    ))
    .width(Length::Fixed(24.0))
    .height(Length::Fixed(24.0))
    .style(|_t: &Theme, status| {
        if status == iced::widget::svg::Status::Hovered {
            iced::widget::svg::Style {
                color: Some(COLOR_ACCENT),
            }
        } else {
            iced::widget::svg::Style {
                color: Some(COLOR_TEXT_PRIMARY),
            }
        }
    });

    let close_btn = button(close_icon)
        .padding(4)
        .style(|_t: &Theme, _status| button::Style {
            background: Some(Color::TRANSPARENT.into()),
            border: iced::Border::default(),
            ..Default::default()
        })
        .on_press(crate::gui::app::Message::AudioCenterMsg(
            AudioCenterMessage::Close,
        ));

    // 3. Contenido de la cabecera con espacio y alineación vertical centrada
    let header_content = row![logo, Space::new().width(Length::Fill), close_btn,]
        .align_y(Alignment::Center)
        .padding(iced::Padding {
            top: 0.0,
            right: 6.0,
            bottom: 0.0,
            left: 15.0,
        });

    let centered_title = container(
        text("Centro de Audio Avanzado")
            .size(16)
            .font(FONT_INTER_SANS_MEDIUM)
            .color(COLOR_TEXT_PRIMARY),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(iced::Fill)
    .center_y(iced::Fill);

    // Cabecera con 40px de altura, con esquinas superiores redondeadas (8px) para acoplarse al contenedor principal
    let header = container(
        iced::widget::Stack::new()
            .push(centered_title)
            .push(header_content),
    )
    .height(Length::Fixed(40.0))
    .width(Length::Fill)
    .style(|_t: &Theme| {
        container::Style::default()
            .background(COLOR_BG)
            .border(iced::Border {
                radius: iced::border::Radius {
                    top_left: 8.0,
                    top_right: 8.0,
                    bottom_left: 0.0,
                    bottom_right: 0.0,
                },
                width: 0.0,
                color: Color::TRANSPARENT,
            })
    });

    // Barra de título arrastrable
    let header_block = mouse_area(header).on_press(crate::gui::app::Message::AudioCenterMsg(
        AudioCenterMessage::DragStart,
    ));

    // 4. Barra de pestañas (Tab Bar)
    let tab_names = [
        "Configuración de Audio",
        "Ecualizador",
        "Efectos de Audio",
        "Volumen y Mezcla",
    ];
    let mut tab_row = row![].spacing(10);

    for (i, name) in tab_names.iter().enumerate() {
        let is_selected = manager.selected_tab == i;

        let tab_btn = button(text(*name).size(14).font(FONT_INTER_SANS_MEDIUM))
            .padding([5, 15])
            .style(move |_t: &Theme, status| {
                let is_hovered = matches!(status, button::Status::Hovered);
                button::Style {
                    background: if is_selected {
                        Some(COLOR_ACCENT.into())
                    } else {
                        Some(Color::TRANSPARENT.into())
                    },
                    text_color: if is_selected {
                        COLOR_TEXT_PRIMARY
                    } else if is_hovered {
                        COLOR_TEXT_PRIMARY
                    } else {
                        COLOR_TEXT_SECONDARY
                    },
                    border: iced::Border {
                        radius: iced::border::Radius {
                            top_left: 2.0,
                            top_right: 2.0,
                            bottom_left: 0.0,
                            bottom_right: 0.0,
                        },
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                }
            })
            .on_press(crate::gui::app::Message::AudioCenterMsg(
                AudioCenterMessage::TabSelected(i),
            ));

        tab_row = tab_row.push(tab_btn);
    }

    // Divisor
    let divider = container(Space::new().width(Length::Fill).height(Length::Fixed(2.0)))
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    // 5. Contenido de pestaña activa
    let content: Element<'a, crate::gui::app::Message> = match manager.selected_tab {
        0 => view_audio_config(manager, audio_manager),
        1 => view_equalizer(manager, audio_manager),
        2 => view_audio_effects(manager, audio_manager),
        3 => view_volumen_mezcla(manager, audio_manager),
        _ => Space::new().into(),
    };

    // Contenido interno con padding de 15px en laterales y fondo
    let inner_content = column![tab_row, divider, content,]
        .padding(iced::Padding {
            top: 0.0,
            right: 15.0,
            bottom: 15.0,
            left: 15.0,
        })
        .width(Length::Fill)
        .height(Length::Fill);

    // Contenedor principal de la ventana
    let window_layout = column![header_block, inner_content,]
        .width(Length::Fill)
        .height(Length::Fill);

    // Evitamos el traspaso de clics capturando todo en un mouse_area con NoOp
    let non_pass_through_window = mouse_area(
        container(window_layout)
            .width(Length::Fixed(940.0))
            .height(Length::Fixed(480.0))
            .padding(2.0) // Inset de 2px para que el borde del contenedor principal no sea tapado por los hijos
            .style(|_t: &Theme| {
                container::Style::default()
                    .background(COLOR_BG)
                    .border(iced::Border {
                        color: COLOR_ACCENT,
                        width: 2.0,
                        radius: 8.0.into(),
                    })
            }),
    )
    .on_press(crate::gui::app::Message::NoOp);

    // Envolvemos con opaque para que no se traspase el "focus" / "hover" del puntero a la interfaz de abajo en esta área
    opaque(non_pass_through_window).into()
}
