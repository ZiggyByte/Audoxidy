//! Library view composition: the grid and list modes plus the surrounding bars.

use crate::db::Database;
use crate::gui::app::Message;
use crate::gui::dimensions::{
    BAR_BOTTOM, BAR_TOP, IconSize, LIST_ROW, LIST_ROW_HEADER, MARGIN_MD, SPACE_LG, SPACE_MD,
    SPACE_XS, STROKE_HAIRLINE, WIDTH_SEARCH,
};
use crate::gui::theme::*;
use crate::gui::widgets::{
    IconState, IconVariant, VuMeterWidget, action_icon_button, icon_button, standard_scrollable,
    standard_scrollbar,
};
use crate::utils::{SortColumn, format_duration, format_size};
use iced::{
    Alignment, Color, Element, Length, Theme,
    widget::{Space, button, column, container, mouse_area, row, text},
};
use std::sync::{Arc, Mutex};

use super::state::{
    LIBRARY_SCROLL_ID, LIBRARY_SEARCH_ID, LibraryManager, library_redraw_count_log,
};
use super::types::{
    AlbumEntry, LibraryGridElement, LibraryListItem, LibrarySource, LibraryViewMode,
};

pub fn view<'a>(
    manager: &'a LibraryManager,
    _database: &'a Arc<Mutex<Database>>,
    playing_path: &'a str,
    meter_state: &'a crate::gui::player::MeterUiState,
    clipping: bool,
    meter_enabled: bool,
) -> Element<'a, Message> {
    library_redraw_count_log();

    // --- BARRA SUPERIOR (40px) ---
    let create_source_tab =
        |label: &'a str, source: LibrarySource, current: LibrarySource| -> Element<'a, Message> {
            let is_active = source == current;
            let text_el = text(label)
                .size(15)
                .font(FONT_INTER_SANS_MEDIUM)
                .color(if is_active {
                    Color::WHITE
                } else {
                    COLOR_TEXT_SECONDARY
                });

            let source_color = match source {
                LibrarySource::Spotify => Color::from_rgb8(29, 185, 84),
                LibrarySource::YouTube => Color::from_rgb8(221, 36, 37),
                LibrarySource::Qobuz => Color::from_rgb8(48, 48, 48),
                LibrarySource::Deezer => Color::from_rgb8(162, 56, 255),
                LibrarySource::Tidal => Color::from_rgb8(68, 68, 68),
                LibrarySource::Local => COLOR_ACCENT,
            };

            let btn_container =
                container(text_el)
                    .padding([SPACE_XS, SPACE_MD])
                    .style(move |_t: &Theme| {
                        if is_active {
                            container::Style::default().background(source_color).border(
                                iced::Border {
                                    radius: 14.0.into(),
                                    ..Default::default()
                                },
                            )
                        } else {
                            container::Style::default()
                        }
                    });

            iced::widget::mouse_area(btn_container)
                .on_press(Message::LibrarySourceSelected(source))
                .interaction(iced::mouse::Interaction::Pointer)
                .into()
        };

    let source_tabs = row![
        create_source_tab("Biblioteca", LibrarySource::Local, manager.source),
        create_source_tab("Spotify", LibrarySource::Spotify, manager.source),
        create_source_tab("YouTube", LibrarySource::YouTube, manager.source),
        create_source_tab("Qobuz", LibrarySource::Qobuz, manager.source),
        create_source_tab("Deezer", LibrarySource::Deezer, manager.source),
        create_source_tab("Tidal", LibrarySource::Tidal, manager.source),
    ]
    .spacing(SPACE_MD)
    .align_y(Alignment::Center)
    .height(Length::Fill);

    let top_bar = row![
        source_tabs,
        Space::new().width(Length::Fill),
        row![
            container(action_icon_button(
                "minimize.svg",
                IconSize::L,
                Message::PlayerWindowAction(crate::gui::player::WindowAction::Minimize),
            ))
            .padding(iced::Padding {
                top: 0.0,
                right: 0.0,
                bottom: 8.0,
                left: 0.0
            }),
            action_icon_button(
                "maximize.svg",
                IconSize::L,
                Message::PlayerWindowAction(crate::gui::player::WindowAction::Maximize),
            ),
            action_icon_button(
                "close.svg",
                IconSize::L,
                Message::PlayerWindowAction(crate::gui::player::WindowAction::Close),
            ),
        ]
        .spacing(SPACE_XS)
        .align_y(Alignment::Center)
    ]
    .align_y(Alignment::Center)
    .height(Length::Fill)
    .padding(iced::Padding {
        top: 0.0,
        right: SPACE_XS,
        bottom: 0.0,
        left: SPACE_LG,
    });

    let top_container = container(top_bar)
        .width(Length::Fill)
        .height(Length::Fixed(BAR_TOP))
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    // --- BARRA DE ORDENAMIENTO (30px) ---
    let columns = if manager.view_mode == LibraryViewMode::DetailedList {
        vec![
            SortColumn::AlbumCard,
            SortColumn::TrackNumber,
            SortColumn::Title,
            SortColumn::Artist,
            SortColumn::AlbumArtist,
            SortColumn::Duration,
            SortColumn::Format,
            SortColumn::SampleRate,
            SortColumn::BitDepth,
            SortColumn::Bitrate,
            SortColumn::Channels,
            SortColumn::Size,
        ]
    } else if manager.view_mode == LibraryViewMode::ThumbnailList {
        vec![
            SortColumn::AlbumThumbnail,
            SortColumn::TrackNumber,
            SortColumn::Title,
            SortColumn::Artist,
            SortColumn::AlbumArtist,
            SortColumn::Album,
            SortColumn::Genre,
            SortColumn::Year,
            SortColumn::Duration,
            SortColumn::Format,
            SortColumn::SampleRate,
            SortColumn::BitDepth,
            SortColumn::Bitrate,
            SortColumn::Channels,
            SortColumn::Size,
        ]
    } else {
        vec![
            SortColumn::TrackNumber,
            SortColumn::Title,
            SortColumn::Artist,
            SortColumn::AlbumArtist,
            SortColumn::Album,
            SortColumn::Genre,
            SortColumn::Year,
            SortColumn::Duration,
            SortColumn::Format,
            SortColumn::SampleRate,
            SortColumn::BitDepth,
            SortColumn::Bitrate,
            SortColumn::Channels,
            SortColumn::Size,
        ]
    };

    let sort_bar_pad = match manager.view_mode {
        LibraryViewMode::Grid => 19.0,
        LibraryViewMode::DetailedList => 29.0,
        LibraryViewMode::ThumbnailList => 31.0,
        LibraryViewMode::SimpleList => 29.0,
    };

    let sort_container = crate::gui::widgets::build_sort_bar(
        &columns,
        manager.sort_column,
        manager.sort_ascending,
        &manager.column_widths,
        manager.resizing_column,
        manager.hovered_column,
        sort_bar_pad,
        |col| Message::ColumnHover(col),
        |col| Message::StartColumnResize(col),
        |col| Message::LibrarySortChanged(col),
    );

    // --- CONTENIDO GRID / LISTA ---
    let content: Element<'a, Message> = match manager.view_mode {
        LibraryViewMode::Grid => {
            // Determinamos qué álbumes mostrar: filtrados o todos
            let albums_to_show = if let Some(filtered) = &manager.filtered_albums {
                filtered
            } else if let Some(cached) = &manager.cached_albums {
                cached
            } else {
                static EMPTY: Vec<AlbumEntry> = Vec::new();
                &EMPTY
            };

            if albums_to_show.is_empty() {
                container(
                    text(if manager.search_query.is_empty() {
                        "La biblioteca está vacía."
                    } else {
                        "No se encontraron álbumes."
                    })
                    .color(COLOR_TEXT_SECONDARY)
                    .font(FONT_INTER_SANS_MEDIUM),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
            } else {
                let res_grid = iced::widget::responsive(move |size| {
                    let card_w = 187.0;
                    let mut columns_count = (size.width / card_w).floor() as usize;
                    if columns_count < 2 {
                        columns_count = 2;
                    }
                    let max_cols = if crate::utils::is_low_resource() {
                        5
                    } else {
                        10
                    };
                    if columns_count > max_cols {
                        columns_count = max_cols;
                    }
                    manager.albums_per_row.set(columns_count);

                    let view_h = manager
                        .last_viewport
                        .as_ref()
                        .map(|v| v.height)
                        .unwrap_or(1000.0);
                    let (top_space, bottom_space, visible_elements) = manager
                        .get_visible_grid_elements(columns_count, manager.scroll_offset.y, view_h);

                    let mut grid_col: iced::widget::Column<'_, Message> = column![].spacing(0);

                    if top_space > 0.0 {
                        grid_col = grid_col.push(Space::new().height(Length::Fixed(top_space)));
                    }

                    for element in visible_elements {
                        match element {
                            LibraryGridElement::AlbumRow {
                                albums,
                                active_expansion_id,
                            } => {
                                let mut current_row = row![].spacing(5);

                                for album_entry in albums {
                                    let album_id = album_entry.id.clone();
                                    let album_title = album_entry.title.clone();
                                    let artist_name = album_entry.artist.clone();
                                    let genre_name = album_entry.genre.clone();
                                    let year_str = album_entry.year.clone();
                                    let cover_path = album_entry.cover_path.clone();

                                    let composite_id = format!("{}|{}", artist_name, album_id);
                                    let is_expanded = active_expansion_id.as_deref()
                                        == Some(composite_id.as_str());
                                    let is_selected = manager
                                        .selected_items
                                        .contains(&LibraryListItem::Album(composite_id.clone()));

                                    let album_art = crate::gui::library::lists::album_art_widget(
                                        cover_path.as_deref(),
                                        None,
                                        None,
                                        crate::gui::library::lists::PlaceholderStyle::Large,
                                        Length::Fixed(152.0),
                                        8.0,
                                    );

                                    let info_col = column![
                                        crate::gui::widgets::smart_truncate_text(
                                            artist_name,
                                            12.0,
                                            FONT_INTER_SANS_MEDIUM,
                                            COLOR_TEXT_PRIMARY
                                        ),
                                        crate::gui::widgets::smart_truncate_text(
                                            album_title,
                                            12.0,
                                            FONT_INTER_SANS_MEDIUM,
                                            COLOR_TEXT_PRIMARY
                                        ),
                                        crate::gui::widgets::smart_truncate_text(
                                            genre_name,
                                            12.0,
                                            FONT_INTER_SANS_MEDIUM,
                                            COLOR_TEXT_PRIMARY
                                        ),
                                        text(year_str)
                                            .size(12)
                                            .color(COLOR_TEXT_PRIMARY)
                                            .font(FONT_INTER_SANS_MEDIUM)
                                            .line_height(iced::widget::text::LineHeight::Absolute(
                                                iced::Pixels(14.0)
                                            )),
                                    ]
                                    .spacing(2)
                                    .width(Length::Fill);

                                    let chevron_svg = if is_expanded {
                                        "arrow-up-chevron.svg"
                                    } else {
                                        "arrow-down-chevron.svg"
                                    };
                                    let chevron_btn = button(
                                        iced::widget::svg(iced::widget::svg::Handle::from_path(
                                            format!("assets/icons/{}", chevron_svg),
                                        ))
                                        .width(IconSize::L.px())
                                        .height(IconSize::L.px())
                                        .style(
                                            move |_t: &Theme, _s| iced::widget::svg::Style {
                                                color: Some(COLOR_TEXT_PRIMARY),
                                            },
                                        ),
                                    )
                                    .padding(0)
                                    .on_press(Message::ToggleAlbumExpansion(composite_id.clone()))
                                    .style(|_t, _s| {
                                        button::Style::default().with_background(Color::TRANSPARENT)
                                    });

                                    let card_bottom = row![info_col, chevron_btn]
                                        .align_y(Alignment::Center)
                                        .width(Length::Fill);

                                    let item_col = column![album_art, card_bottom].spacing(5);
                                    let card_wrapper = mouse_area(item_col)
                                        .on_press(Message::SelectAlbum(composite_id.clone()))
                                        .interaction(iced::mouse::Interaction::Pointer);

                                    let card_container = container(card_wrapper)
                                        .width(Length::Fixed(182.0))
                                        .padding(iced::Padding {
                                            top: 18.0,
                                            bottom: 15.0,
                                            left: 15.0,
                                            right: 15.0,
                                        })
                                        .style(move |_t: &Theme| {
                                            if is_expanded || is_selected {
                                                container::Style::default()
                                                    .background(COLOR_CONTRAST)
                                                    .border(iced::Border {
                                                        radius: 10.0.into(),
                                                        ..Default::default()
                                                    })
                                            } else {
                                                container::Style::default()
                                            }
                                        });

                                    current_row = current_row.push(card_container);
                                }
                                grid_col = grid_col.push(current_row);
                            }
                            LibraryGridElement::ExpandedSongs {
                                album_id: _,
                                songs,
                                top_space: s_top,
                                bottom_space: s_bottom,
                            } => {
                                let mut album_songs_col = column![].spacing(0).padding([32, 0]);

                                if s_top > 0.0 {
                                    album_songs_col = album_songs_col
                                        .push(Space::new().height(Length::Fixed(s_top)));
                                }

                                for (song, _song_i) in songs {
                                    let is_song_selected = manager
                                        .selected_items
                                        .contains(&LibraryListItem::Song(song.id));
                                    let columns = [
                                        SortColumn::TrackNumber,
                                        SortColumn::Title,
                                        SortColumn::Artist,
                                        SortColumn::AlbumArtist,
                                        SortColumn::Album,
                                        SortColumn::Genre,
                                        SortColumn::Year,
                                        SortColumn::Duration,
                                        SortColumn::Format,
                                        SortColumn::SampleRate,
                                        SortColumn::BitDepth,
                                        SortColumn::Bitrate,
                                        SortColumn::Channels,
                                        SortColumn::Size,
                                    ];

                                    let song_row =
                                        crate::gui::library::lists::universal_song_row_widget(
                                            &song,
                                            0,
                                            is_song_selected,
                                            &columns,
                                            &manager.column_widths,
                                            Message::SelectSong(Some(song.id)),
                                            playing_path,
                                            LIST_ROW_HEADER,
                                            false,
                                        );
                                    album_songs_col = album_songs_col.push(song_row);
                                }

                                if s_bottom > 0.0 {
                                    album_songs_col = album_songs_col
                                        .push(Space::new().height(Length::Fixed(s_bottom)));
                                }

                                grid_col = grid_col.push(
                                    container(album_songs_col)
                                        .width(Length::Fill)
                                        .padding(iced::Padding {
                                            top: 0.0,
                                            right: 10.0,
                                            bottom: 5.0,
                                            left: 5.0,
                                        })
                                        .style(|_t| {
                                            container::Style::default().background(COLOR_BG)
                                        }),
                                );
                            }
                        }
                    }

                    if bottom_space > 0.0 {
                        grid_col = grid_col.push(Space::new().height(Length::Fixed(bottom_space)));
                    }

                    grid_col.into()
                });

                // IMPORTANTE: NO usamos IDs dinámicos (A/B) basados en search_nonce a menos que sea estrictamente necesario.
                // Mantener el scroll_id constante permite que Iced mantenga el caché de carátulas y estado del scroll.
                let scroll_id = LIBRARY_SCROLL_ID.clone();
                let scrollable_grid = standard_scrollable(
                    scroll_id,
                    res_grid,
                    iced::widget::scrollable::Direction::Vertical(standard_scrollbar()),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .on_scroll(Message::LibraryScroll);

                container(scrollable_grid)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
        }
        LibraryViewMode::SimpleList => {
            let cols = columns.clone();
            crate::gui::library::lists::universal_song_list(
                manager,
                move |song, _song_idx, is_selected, playing_path| {
                    crate::gui::library::lists::library_song_row_widget(
                        song,
                        0,
                        is_selected,
                        &cols,
                        &manager.column_widths,
                        Message::SelectSong(Some(song.id)),
                        playing_path,
                    )
                },
                LIST_ROW_HEADER,
                playing_path,
            )
        }
        LibraryViewMode::DetailedList => {
            let cols = columns;
            crate::gui::library::lists::detailed_song_list(
                manager,
                move |song, _song_idx, is_selected, playing_path| {
                    crate::gui::library::lists::library_song_row_widget(
                        song,
                        0,
                        is_selected,
                        &cols,
                        &manager.column_widths,
                        Message::SelectSong(Some(song.id)),
                        playing_path,
                    )
                },
                LIST_ROW_HEADER,
                playing_path,
            )
        }
        LibraryViewMode::ThumbnailList => {
            let cols = columns.clone();
            crate::gui::library::lists::universal_song_list(
                manager,
                move |song, _song_idx, is_selected, playing_path| {
                    crate::gui::library::lists::thumbnail_song_row_widget(
                        song,
                        0,
                        is_selected,
                        &cols,
                        &manager.column_widths,
                        Message::SelectSong(Some(song.id)),
                        playing_path,
                    )
                },
                LIST_ROW,
                playing_path,
            )
        }
    };

    // --- BARRA INFERIOR (40px) ---
    let search_input = crate::gui::widgets::standard_search_input(
        Some(LIBRARY_SEARCH_ID.clone()),
        "Buscar...",
        &manager.search_query,
        Message::LibrarySearchQueryChanged,
        Message::LibrarySearchQueryChanged(String::new()),
        Length::Fixed(WIDTH_SEARCH),
    );

    let (s_count, a_count, art_count, d_secs, s_bytes) = if let Some(sel) = &manager.selection_stats
    {
        (
            sel.songs,
            sel.albums,
            sel.artists,
            sel.duration_secs,
            sel.size_bytes,
        )
    } else if let Some(flt) = &manager.filter_stats {
        (
            flt.songs,
            flt.albums,
            flt.artists,
            flt.duration_secs,
            flt.size_bytes,
        )
    } else {
        (
            manager.total_songs as u64,
            manager.total_albums as u64,
            manager.total_artists as u64,
            manager.total_duration_secs,
            manager.total_size_bytes,
        )
    };

    let stats_text = if manager.view_mode == LibraryViewMode::Grid {
        format!(
            "{} Canciones | {} Álbumes | {} Artistas | {} | {}",
            s_count,
            a_count,
            art_count,
            format_duration(d_secs),
            format_size(s_bytes as i64)
        )
    } else {
        format!(
            "{} Canciones | {} Álbumes | {} Artistas | {} | {}",
            s_count,
            a_count,
            art_count,
            format_duration(d_secs),
            format_size(s_bytes as i64)
        )
    };

    let view_icon_str = match manager.view_mode {
        LibraryViewMode::Grid => "view-grid-outlined.svg",
        LibraryViewMode::ThumbnailList => "view-list-thumbnail-outlined.svg",
        LibraryViewMode::DetailedList => "view-list-thumbnail-fill.svg",
        LibraryViewMode::SimpleList => "view-list.svg",
    };

    let bottom_actions = row![
        icon_button(
            "play-straight-outlined.svg",
            IconSize::L,
            IconState::Default,
            Some(Message::PlayLibrarySelection),
            IconVariant::Default,
            Some("Reproducir selección"),
        ),
        Space::new().width(SPACE_XS),
        icon_button(
            "more-small.svg",
            IconSize::L,
            IconState::Default,
            Some(Message::OpenFolderPicker),
            IconVariant::Default,
            Some("Elegir carpeta"),
        ),
        Space::new().width(SPACE_XS),
        icon_button(
            "radio-button-on.svg",
            IconSize::L,
            IconState::Default,
            Some(Message::LibraryShowPlaying),
            IconVariant::Default,
            Some("Mostrar en reproducción"),
        ),
        Space::new().width(SPACE_XS),
        icon_button(
            view_icon_str,
            IconSize::L,
            IconState::Default,
            Some(Message::ToggleLibraryViewDropdown),
            IconVariant::Default,
            Some("Cambiar vista"),
        ),
    ]
    .align_y(Alignment::Center)
    .height(Length::Fill);

    let meter_widget = VuMeterWidget::new(
        meter_state.display_l,
        meter_state.smooth_peak_l,
        meter_state.peak_hold_l,
        meter_state.display_r,
        meter_state.smooth_peak_r,
        meter_state.peak_hold_r,
        clipping,
        meter_state.ms_mode,
        meter_state.phase_corr,
        meter_state.show_numeric,
        meter_state.crest_l,
        meter_state.crest_r,
    );

    // Envolver el medidor con mouse_area para capturar right-click
    let meter_with_rightclick: Element<'_, Message> = mouse_area(
        iced::widget::container(meter_widget)
            .padding(0)
            .width(Length::Shrink)
            .height(Length::Shrink),
    )
    .on_press(Message::NoOp)
    .on_right_press(Message::MeterRightClicked(iced::Point::ORIGIN))
    .into();

    let bottom_bar = if meter_enabled {
        row![
            search_input,
            Space::new().width(SPACE_LG),
            text(stats_text)
                .size(13)
                .color(COLOR_TEXT_SECONDARY)
                .font(FONT_INTER_SANS_MEDIUM),
            Space::new().width(SPACE_MD),
            meter_with_rightclick,
            Space::new().width(SPACE_MD),
            bottom_actions
        ]
        .padding(iced::Padding {
            top: 0.0,
            bottom: 0.0,
            left: MARGIN_MD,
            right: MARGIN_MD,
        })
        .height(Length::Fill)
        .align_y(Alignment::Center)
    } else {
        row![
            search_input,
            Space::new().width(SPACE_LG),
            text(stats_text)
                .size(13)
                .color(COLOR_TEXT_SECONDARY)
                .font(FONT_INTER_SANS_MEDIUM),
            Space::new().width(Length::Fill),
            bottom_actions
        ]
        .padding(iced::Padding {
            top: 0.0,
            bottom: 0.0,
            left: MARGIN_MD,
            right: MARGIN_MD,
        })
        .height(Length::Fill)
        .align_y(Alignment::Center)
    };

    let content_with_deselection = iced::widget::mouse_area(
        container(content)
            .padding(iced::Padding {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: SPACE_XS,
            })
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::LibraryDeselect);

    let mut content_stack = iced::widget::Stack::new().push(content_with_deselection);

    if manager.is_dragging {
        if let (Some(start), Some(end)) = (manager.marquee_start, manager.marquee_end) {
            let dx = start.x - end.x;
            let dy = start.y - end.y;
            let dist_sq = dx * dx + dy * dy;

            if dist_sq > 9.0 {
                let scroll_y = manager.scroll_offset.y;
                let visual_start_y = start.y - scroll_y;
                let visual_end_y = end.y - scroll_y;

                // Calculamos los límites visuales reales (clamped al viewport del área de contenido)
                let y_min = visual_start_y.min(visual_end_y);
                let y_max = visual_start_y.max(visual_end_y);

                let x = start.x.min(end.x);
                let y = y_min.max(0.0);
                let w = dx.abs().max(1.0);
                let h = (y_max - y).max(0.0);

                if h > 0.1 && w > 0.1 {
                    let marquee_rect = container(
                        iced::widget::Space::new()
                            .width(Length::Fill)
                            .height(Length::Fill),
                    )
                    .width(Length::Fixed(w))
                    .height(Length::Fixed(h))
                    .style(move |_t| container::Style {
                        background: Some(COLOR_ACCENT.scale_alpha(0.3).into()),
                        border: iced::Border {
                            color: COLOR_ACCENT,
                            width: STROKE_HAIRLINE,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    });

                    content_stack =
                        content_stack.push(container(marquee_rect).padding(iced::Padding {
                            top: y,
                            right: 0.0,
                            bottom: 0.0,
                            left: x,
                        }));
                }
            }
        }
    }

    let final_view = mouse_area(
        container(column![
            top_container,
            sort_container,
            content_stack,
            container(bottom_bar)
                .width(Length::Fill)
                .height(Length::Fixed(BAR_BOTTOM))
                .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST))
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .style(|_t: &Theme| container::Style::default().background(COLOR_BG)),
    )
    .on_press(Message::LibraryFocus);

    final_view.into()
}
