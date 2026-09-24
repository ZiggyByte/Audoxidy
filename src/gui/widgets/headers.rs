use super::*;

use crate::gui::dimensions::{
    BAR_SUB, DIVIDER_THIN, IconSize, RADIUS_S, SORT_COLUMN_MIN, SPACE_LG, SPACE_XS,
};

use iced::widget::column;

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
        let width = *column_widths
            .get(&sort)
            .unwrap_or(&(SORT_COLUMN_MIN as u16)) as f32;

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
                Some(true) => Some(svg::Handle::from_path(icon_asset_path(
                    "arrow-up-chevron.svg",
                ))),
                Some(false) => Some(svg::Handle::from_path(icon_asset_path(
                    "arrow-down-chevron.svg",
                ))),
                _ => None,
            };

            if let Some(h) = handle {
                Some(
                    svg(h)
                        .width(IconSize::M.px())
                        .height(IconSize::M.px())
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
                        radius: RADIUS_S.into(),
                        width: RADIUS_S,
                        color: Color::TRANSPARENT,
                    })
            });

        let on_h = on_hover(Some(sort));
        let on_h_exit = on_hover(None);
        let separator_area: Element<Message> =
            if sort == SortColumn::AlbumCard || sort == SortColumn::AlbumThumbnail {
                Space::new().width(SPACE_XS).into()
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
                    Space::new().width(SPACE_XS)
                ]
                .align_y(Alignment::Center)
            } else {
                row![
                    Space::new().width(Length::Fill),
                    t,
                    Space::new().width(SPACE_XS)
                ]
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
            Space::new().width(SPACE_XS),
            separator_area,
            Space::new().width(SPACE_XS)
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
            .height(Length::Fixed(BAR_SUB))
            .style(|_t: &Theme| container::Style::default().background(COLOR_BG)),
        container(Space::new().width(Length::Fill).height(DIVIDER_THIN))
            .style(|_t: &Theme| container::Style::default().background(Color::from(COLOR_CONTRAST)))
    ]
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
        Space::new().width(SPACE_LG),
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
        Space::new().width(SPACE_LG),
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
