use iced::{
    Alignment, Color, Element, Length, Padding, Theme,
    widget::{Space, column, container, mouse_area, row, svg, text},
};

use crate::gui::theme::{
    COLOR_ACCENT, COLOR_BG, COLOR_CONTRAST, COLOR_TEXT_PRIMARY, COLOR_TEXT_SECONDARY,
    FONT_INTER_SANS_MEDIUM,
};
use crate::gui::widgets::{
    album_header_widget, artist_header_widget, smart_truncate_text, standard_scrollable,
    standard_scrollbar,
};
use crate::utils::{SortColumn, format_metadata, truncate_text};
use std::collections::HashMap;

/// Estilos de Placeholder para el widget de carátulas
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaceholderStyle {
    /// Versión grande (Grid, Lista Detallada): Icono 96px + Texto "AuDoxiDY"
    Large,
    /// Versión pequeña (Thumbnail List): Solo icono
    Small,
    /// Versión específica del reproductor (Solo texto)
    Player,
}

/// Widget global para mostrar carátulas de álbumes de forma eficiente y consistente.
/// Soporta: Caché AVIF (Prioridad 1) -> Datos binarios crudos (Prioridad 2) -> Placeholder Automático.
pub fn album_art_widget<'a, Message: 'a>(
    avif_path: Option<&str>,
    raw_data: Option<&Vec<u8>>,
    preloaded_handle: Option<iced::widget::image::Handle>,
    style: PlaceholderStyle,
    bounds: Length,
    radius: f32,
) -> Element<'a, Message> {
    // 1. Usar handle precargado si existe (Prioridad 0 - Máximo rendimiento)
    let mut handle_opt = preloaded_handle;

    // 2. Intentar cargar desde archivo AVIF en disco (Prioridad 1) — sin caché RAM
    if handle_opt.is_none() {
        handle_opt = avif_path.and_then(|p| crate::utils::covers::load_cover_handle(p));
    }

    // 3. Fallback: Cargar desde bytes crudos (Prioridad 2)
    if handle_opt.is_none() {
        if let Some(data) = raw_data {
            handle_opt = crate::utils::covers::load_raw_image_for_iced(data);
        }
    }

    if let Some(handle) = handle_opt {
        container(
            iced::widget::image(handle)
                .width(bounds)
                .height(bounds)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(radius),
        )
        .width(bounds)
        .height(bounds)
        .style(move |_t: &Theme| {
            container::Style::default().border(iced::Border {
                radius: radius.into(),
                ..Default::default()
            })
        })
        .clip(true)
        .into()
    } else {
        // 4. Fallback: Placeholder Automático basado en el estilo solicitado
        match style {
            PlaceholderStyle::Large => container(
                column![
                    svg(svg::Handle::from_path("assets/icons/album.svg"))
                        .width(Length::Fixed(96.0))
                        .height(Length::Fixed(96.0))
                        .style(|_t: &Theme, _s| svg::Style {
                            color: Some(Color::from(COLOR_TEXT_SECONDARY))
                        }),
                    text("AuDoxiDY")
                        .font(crate::gui::theme::FONT_STAGE_WANDER)
                        .size(11)
                        .color(COLOR_TEXT_SECONDARY)
                ]
                .align_x(Alignment::Center)
                .spacing(5),
            )
            .width(bounds)
            .height(bounds)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(move |_t: &Theme| {
                container::Style::default()
                    .background(COLOR_BG)
                    .border(iced::Border {
                        radius: radius.into(),
                        ..Default::default()
                    })
            })
            .into(),
            PlaceholderStyle::Small => container(
                svg(svg::Handle::from_path("assets/icons/album.svg"))
                    .width(Length::Fixed(20.0))
                    .height(Length::Fixed(20.0))
                    .style(|_t: &Theme, _s| svg::Style {
                        color: Some(Color::from(COLOR_TEXT_SECONDARY)),
                    }),
            )
            .width(bounds)
            .height(bounds)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(move |_t: &Theme| {
                container::Style::default()
                    .background(COLOR_BG)
                    .border(iced::Border {
                        radius: radius.into(),
                        ..Default::default()
                    })
            })
            .into(),
            PlaceholderStyle::Player => container(
                text("AuDoxiDY")
                    .font(crate::gui::theme::FONT_STAGE_WANDER)
                    .size(40)
                    .color(COLOR_TEXT_PRIMARY),
            )
            .width(bounds)
            .height(bounds)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .padding(20)
            .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
            .into(),
        }
    }
}

/// Widget universal para renderizar una fila de canción en cualquier vista de lista.
/// Soporta: Diferentes alturas, miniaturas opcionales, resaltado de reproducción y selección.
pub fn universal_song_row_widget<'a, Message: Clone + 'a>(
    song: &crate::db::database::SongData,
    _song_idx: usize,
    is_selected: bool,
    columns: &[SortColumn],
    column_widths: &HashMap<SortColumn, u16>,
    on_select: Message,
    playing_path: &str,
    row_height: f32,
    show_thumbnail: bool,
) -> Element<'a, Message> {
    let txt_color = if is_selected {
        COLOR_TEXT_PRIMARY
    } else {
        COLOR_TEXT_SECONDARY
    };
    let is_playing = song.full_file_path.as_ref() == playing_path;

    let get_col = |col: SortColumn| -> Element<'a, Message> {
        let w = *column_widths.get(&col).unwrap_or(&100) as f32;
        let max_chars = ((w - 10.0) / 7.0).max(1.0) as usize;
        let val = format_metadata(song, &col);
        let truncated = truncate_text(&val, max_chars);

        let content: Element<'a, Message> = if col == SortColumn::TrackNumber {
            row![
                container(if is_playing {
                    text("•")
                        .size(13)
                        .color(COLOR_ACCENT)
                        .font(FONT_INTER_SANS_MEDIUM)
                        .wrapping(iced::widget::text::Wrapping::None)
                } else {
                    text("").size(13)
                })
                .width(Length::Fixed(26.0))
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center),
                container(
                    text(truncated)
                        .size(13)
                        .color(Color::from(txt_color))
                        .font(FONT_INTER_SANS_MEDIUM)
                        .wrapping(iced::widget::text::Wrapping::None)
                )
                .width(Length::Fixed(26.0))
                .align_x(iced::alignment::Horizontal::Right)
                .align_y(iced::alignment::Vertical::Center)
            ]
            .spacing(0)
            .align_y(Alignment::Center)
            .into()
        } else {
            text(truncated)
                .size(13)
                .color(Color::from(txt_color))
                .font(FONT_INTER_SANS_MEDIUM)
                .wrapping(iced::widget::text::Wrapping::None)
                .into()
        };

        let pad_left = if col == SortColumn::TrackNumber {
            0.0
        } else {
            15.0
        };
        container(content)
            .width(Length::Fixed(w))
            .height(Length::Fill)
            .center_y(Length::Fill)
            .padding(Padding {
                left: pad_left,
                right: 5.0,
                top: 0.0,
                bottom: 0.0,
            })
            .clip(true)
            .into()
    };

    let mut elements: Vec<Element<'a, Message>> = Vec::new();

    // 1. Agregar Miniatura si se solicita
    if show_thumbnail {
        let thumb_img = album_art_widget(
            song.compressed_cached_cover_root.as_deref(),
            None,
            None,
            PlaceholderStyle::Small,
            Length::Fixed(32.0),
            4.0,
        );

        elements.push(
            container(thumb_img)
                .width(Length::Fixed(42.0))
                .height(Length::Fixed(row_height))
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .into(),
        );
    }

    // 2. Agregar Columnas de metadatos (evitando duplicar el espacio de Thumbnail)
    for col in columns {
        if *col != SortColumn::AlbumCard && *col != SortColumn::AlbumThumbnail {
            elements.push(get_col(*col));
        }
    }

    let song_row_inner = iced::widget::Row::with_children(elements)
        .align_y(Alignment::Center)
        .padding([0, 5])
        .height(Length::Fill);

    mouse_area(
        container(song_row_inner)
            .width(Length::Fill)
            .height(Length::Fixed(row_height))
            .align_y(Alignment::Center)
            .style(move |_t: &Theme| {
                if is_selected {
                    container::Style::default().background(Color::from(COLOR_CONTRAST))
                } else {
                    container::Style::default()
                }
            }),
    )
    .on_press(on_select)
    .interaction(iced::mouse::Interaction::Pointer)
    .into()
}

pub fn library_song_row_widget<'a, Message: Clone + 'a>(
    song: &crate::db::database::SongData,
    song_idx: usize,
    is_selected: bool,
    columns: &[SortColumn],
    column_widths: &HashMap<SortColumn, u16>,
    on_select: Message,
    playing_path: &str,
) -> Element<'a, Message> {
    universal_song_row_widget(
        song,
        song_idx,
        is_selected,
        columns,
        column_widths,
        on_select,
        playing_path,
        32.0,
        false,
    )
}

/// Renderiza una fila de canción en modo ThumbnailList (42px) con thumbnail 32x32 redondeado al inicio.
/// Renderiza una fila de canción en modo ThumbnailList (42px) con thumbnail 32x32 redondeado al inicio.
pub fn thumbnail_song_row_widget<'a, Message: Clone + 'a>(
    song: &crate::db::database::SongData,
    song_idx: usize,
    is_selected: bool,
    columns: &[SortColumn],
    column_widths: &HashMap<SortColumn, u16>,
    on_select: Message,
    playing_path: &str,
) -> Element<'a, Message> {
    universal_song_row_widget(
        song,
        song_idx,
        is_selected,
        columns,
        column_widths,
        on_select,
        playing_path,
        42.0,
        true,
    )
}

/// Permite construir una lista universalizada que agrupa canciones por artistas,
/// gestiona el scroll, la virtualización, y las cabeceras pegajosas de forma global.
pub fn universal_song_list<'a, F>(
    manager: &'a crate::gui::library::LibraryManager,
    row_builder: F,
    _row_height: f32,
    playing_path: &'a str,
) -> Element<'a, crate::gui::app::Message>
where
    F: Fn(
            &std::sync::Arc<crate::db::database::SongData>,
            usize,
            bool,
            &str,
        ) -> Element<'a, crate::gui::app::Message>
        + 'a,
{
    let groups = &manager.artist_groups;
    if groups.is_empty() {
        return container(
            text("La biblioteca está vacía o cargando...")
                .color(COLOR_TEXT_SECONDARY)
                .font(FONT_INTER_SANS_MEDIUM),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    }

    let header_h = if manager.view_mode == crate::gui::library::LibraryViewMode::ThumbnailList {
        42.0
    } else {
        32.0
    };
    let album_header_h = if manager.view_mode == crate::gui::library::LibraryViewMode::ThumbnailList
    {
        42.0
    } else {
        32.0
    };

    let (top_space, bottom_space, _total_h, visible_elements, sticky_artist_info) =
        manager.get_visible_elements();

    let mut list_col = column![].spacing(0);
    if top_space > 0.0 {
        list_col = list_col.push(
            mouse_area(Space::new().height(Length::Fixed(top_space)))
                .on_press(crate::gui::app::Message::LibraryDeselect),
        );
    }

    for element in visible_elements {
        match element {
            crate::gui::library::LibraryVirtualRow::ArtistHeader {
                name,
                is_collapsed,
                albums_count,
                songs_count,
                duration_secs,
            } => {
                let is_header_selected = manager
                    .selected_items
                    .contains(&crate::gui::library::LibraryListItem::Artist(name.clone()));

                let is_artist_playing = manager
                    .artist_groups
                    .iter()
                    .find(|g| g.name == *name)
                    .map(|g| {
                        g.songs
                            .iter()
                            .any(|s| s.full_file_path.as_ref() == playing_path)
                    })
                    .unwrap_or(false);

                list_col = list_col.push(artist_header_widget(
                    name.clone(),
                    is_collapsed,
                    is_header_selected,
                    is_artist_playing,
                    albums_count,
                    songs_count,
                    duration_secs,
                    header_h,
                    crate::gui::app::Message::SelectArtistHeader(name.clone()),
                    crate::gui::app::Message::ToggleArtistExpansion(name.clone()),
                ));
            }
            crate::gui::library::LibraryVirtualRow::AlbumBlock {
                album_name,
                album_hash,
                artist_name,
                is_expanded,
                songs_count,
                duration_secs,
                ..
            } => {
                let composite_id = format!("{}|{}", artist_name, album_hash);
                let is_album_selected =
                    manager
                        .selected_items
                        .contains(&crate::gui::library::LibraryListItem::Album(
                            composite_id.clone(),
                        ));

                // En este modo, el header del álbum se renderiza como una fila
                list_col = list_col.push(album_header_widget(
                    album_name.clone(),
                    songs_count,
                    duration_secs,
                    is_expanded,
                    is_album_selected,
                    false,
                    album_header_h,
                    crate::gui::app::Message::SelectAlbum(composite_id.clone()),
                    crate::gui::app::Message::ToggleAlbumExpansion(composite_id),
                ));
            }
            crate::gui::library::LibraryVirtualRow::SimpleSong { song, global_idx } => {
                let is_song_selected = manager
                    .selected_items
                    .contains(&crate::gui::library::LibraryListItem::Song(song.id));
                list_col = list_col.push(row_builder(
                    &song,
                    global_idx,
                    is_song_selected,
                    playing_path,
                ));
            }
        }
    }

    if bottom_space > 0.0 {
        list_col = list_col.push(
            mouse_area(Space::new().height(Length::Fixed(bottom_space)))
                .on_press(crate::gui::app::Message::LibraryDeselect),
        );
    }

    let list_container = mouse_area(container(list_col).width(Length::Fill).padding(Padding {
        left: 10.0,
        right: 15.0,
        top: 0.0,
        bottom: 0.0,
    }))
    .on_press(crate::gui::app::Message::LibraryDeselect);

    let main_scroll = standard_scrollable(
        crate::gui::library::LIBRARY_SCROLL_ID.clone(),
        list_container,
        iced::widget::scrollable::Direction::Vertical(standard_scrollbar()),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .on_scroll(crate::gui::app::Message::LibraryScroll);

    let content: Element<'a, crate::gui::app::Message> =
        if let Some((ref st_name, st_collapsed, st_albums, st_songs, st_duration)) =
            sticky_artist_info
        {
            let is_header_selected =
                manager
                    .selected_items
                    .contains(&crate::gui::library::LibraryListItem::Artist(
                        st_name.clone(),
                    ));

            let is_artist_playing = manager
                .artist_groups
                .iter()
                .find(|g| g.name == *st_name)
                .map(|g| {
                    g.songs
                        .iter()
                        .any(|s| s.full_file_path.as_ref() == playing_path)
                })
                .unwrap_or(false);

            let sticky_overlay = container(artist_header_widget(
                st_name.clone(),
                st_collapsed,
                is_header_selected,
                is_artist_playing,
                st_albums,
                st_songs,
                st_duration,
                header_h,
                crate::gui::app::Message::SelectArtistHeader(st_name.clone()),
                crate::gui::app::Message::ToggleArtistExpansion(st_name.clone()),
            ))
            .width(Length::Fill)
            .height(Length::Fixed(header_h))
            .padding(Padding {
                left: 10.0,
                right: 15.0,
                top: 0.0,
                bottom: 0.0,
            })
            .align_y(iced::alignment::Vertical::Top);

            iced::widget::stack![main_scroll, sticky_overlay].into()
        } else {
            main_scroll.into()
        };

    content
}

/// Vista Detallada: Combina agrupación por Artista, y dentro por Álbum.
/// Muestra a la izquierda la tarjeta del Álbum y a la derecha el Header de Álbum + la lista de canciones.
pub fn detailed_song_list<'a, F>(
    manager: &'a crate::gui::library::LibraryManager,
    row_builder: F,
    _row_height: f32,
    playing_path: &'a str,
) -> Element<'a, crate::gui::app::Message>
where
    F: Fn(
            &std::sync::Arc<crate::db::database::SongData>,
            usize,
            bool,
            &str,
        ) -> Element<'a, crate::gui::app::Message>
        + 'a,
{
    let groups = &manager.artist_groups;
    if groups.is_empty() {
        return container(
            text("La biblioteca está vacía o cargando...")
                .color(COLOR_TEXT_SECONDARY)
                .font(FONT_INTER_SANS_MEDIUM),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    }

    let _header_h = if manager.view_mode == crate::gui::library::LibraryViewMode::ThumbnailList {
        42.0
    } else {
        32.0
    };
    let album_header_h = 32.0;
    let card_w = 250.0;

    let (top_space, bottom_space, _total_h, visible_elements, sticky_artist_info) =
        manager.get_visible_elements();

    let mut list_col = column![].spacing(0);
    if top_space > 0.0 {
        list_col = list_col.push(
            mouse_area(Space::new().height(Length::Fixed(top_space)))
                .on_press(crate::gui::app::Message::LibraryDeselect),
        );
    }

    for element in visible_elements {
        match element {
            crate::gui::library::LibraryVirtualRow::ArtistHeader {
                name,
                is_collapsed,
                albums_count,
                songs_count,
                duration_secs,
            } => {
                let is_artist_explicitly_selected = manager
                    .selected_items
                    .contains(&crate::gui::library::LibraryListItem::Artist(name.clone()));

                let is_artist_playing = manager
                    .artist_groups
                    .iter()
                    .find(|g| g.name == *name)
                    .map(|g| {
                        g.songs
                            .iter()
                            .any(|s| s.full_file_path.as_ref() == playing_path)
                    })
                    .unwrap_or(false);

                let header = artist_header_widget(
                    name.clone(),
                    is_collapsed,
                    is_artist_explicitly_selected,
                    is_artist_playing,
                    albums_count,
                    songs_count,
                    duration_secs,
                    32.0,
                    crate::gui::app::Message::SelectArtistHeader(name.clone()),
                    crate::gui::app::Message::ToggleArtistExpansion(name.clone()),
                );
                list_col = list_col.push(header);
            }
            crate::gui::library::LibraryVirtualRow::AlbumBlock {
                album_name,
                album_hash,
                artist_name,
                genre,
                year,
                is_expanded,
                songs,
                songs_count,
                duration_secs,
            } => {
                let cover_path = if let Some(albums) = &manager.cached_albums {
                    albums
                        .iter()
                        .find(|a| a.title == *album_name && a.artist == *artist_name)
                        .and_then(|a| a.cover_path.clone())
                } else {
                    None
                };

                let card_wrapper = album_art_widget(
                    cover_path.as_deref(),
                    None,
                    None,
                    PlaceholderStyle::Large,
                    Length::Fixed(card_w - 30.0),
                    8.0,
                );

                let info_col = column![
                    smart_truncate_text(
                        artist_name.clone(),
                        13.0,
                        FONT_INTER_SANS_MEDIUM,
                        COLOR_TEXT_PRIMARY
                    ),
                    smart_truncate_text(
                        album_name.clone(),
                        13.0,
                        FONT_INTER_SANS_MEDIUM,
                        COLOR_TEXT_PRIMARY
                    ),
                    smart_truncate_text(
                        genre.clone(),
                        13.0,
                        FONT_INTER_SANS_MEDIUM,
                        COLOR_TEXT_PRIMARY
                    ),
                    text(year.clone())
                        .size(13)
                        .color(COLOR_TEXT_PRIMARY)
                        .font(FONT_INTER_SANS_MEDIUM),
                ]
                .spacing(2)
                .width(Length::Fill);

                let composite_id = format!("{}|{}", artist_name, album_hash);
                let is_album_explicitly_selected =
                    manager
                        .selected_items
                        .contains(&crate::gui::library::LibraryListItem::Album(
                            composite_id.clone(),
                        ));
                let is_song_selected_in_album = songs.iter().any(|(s, _)| {
                    manager
                        .selected_items
                        .contains(&crate::gui::library::LibraryListItem::Song(s.id))
                });
                let is_album_card_highlighted =
                    is_album_explicitly_selected || is_song_selected_in_album;

                let card_col = column![card_wrapper, info_col]
                    .spacing(5)
                    .width(Length::Fixed(card_w - 30.0));

                let card_container = mouse_area(
                    container(card_col)
                        .width(Length::Fixed(card_w))
                        .height(Length::Fixed(323.0))
                        .padding(iced::Padding {
                            top: 15.0,
                            bottom: 15.0,
                            left: 15.0,
                            right: 15.0,
                        })
                        .style(move |_t: &Theme| {
                            if is_album_card_highlighted {
                                let rad = iced::border::Radius {
                                    top_left: 0.0,
                                    top_right: 0.0,
                                    bottom_right: 10.0,
                                    bottom_left: 10.0,
                                };
                                container::Style::default()
                                    .background(COLOR_CONTRAST)
                                    .border(iced::Border {
                                        radius: rad,
                                        ..Default::default()
                                    })
                            } else {
                                container::Style::default()
                            }
                        }),
                )
                .on_press(crate::gui::app::Message::SelectAlbum(composite_id.clone()))
                .interaction(iced::mouse::Interaction::Pointer);

                let is_album_playing = songs
                    .iter()
                    .any(|(s, _)| s.full_file_path.as_ref() == playing_path);

                let alb_header = album_header_widget(
                    album_name.clone(),
                    songs_count,
                    duration_secs,
                    is_expanded,
                    is_album_explicitly_selected,
                    is_album_playing,
                    album_header_h,
                    crate::gui::app::Message::SelectAlbum(composite_id.clone()),
                    crate::gui::app::Message::ToggleAlbumExpansion(composite_id),
                );

                let mut right_col = column![alb_header].spacing(0).width(Length::Fill);

                if is_expanded {
                    for (song, song_i) in &songs {
                        let is_song_selected = manager
                            .selected_items
                            .contains(&crate::gui::library::LibraryListItem::Song(song.id));
                        let song_row = row_builder(song, *song_i, is_song_selected, playing_path);
                        right_col = right_col.push(song_row);
                    }
                }

                let content_row = if is_expanded {
                    row![
                        container(card_container)
                            .height(Length::Shrink)
                            .align_y(iced::alignment::Vertical::Top),
                        container(right_col)
                            .height(Length::Shrink)
                            .align_y(iced::alignment::Vertical::Top)
                    ]
                    .spacing(0)
                    .width(Length::Fill)
                    .align_y(Alignment::Start)
                } else {
                    row![
                        // If collapsed, we omit the card (it's hidden) and just show the header block spanning
                        container(Space::new().width(card_w)).height(Length::Shrink),
                        container(right_col)
                            .height(Length::Shrink)
                            .align_y(iced::alignment::Vertical::Top)
                    ]
                    .spacing(0)
                    .width(Length::Fill)
                    .align_y(Alignment::Start)
                };

                list_col = list_col.push(column![
                    content_row,
                    Space::new().height(Length::Fixed(10.0))
                ]);
            }
            _ => {}
        }
    }

    if bottom_space > 0.0 {
        list_col = list_col.push(
            mouse_area(Space::new().height(Length::Fixed(bottom_space)))
                .on_press(crate::gui::app::Message::LibraryDeselect),
        );
    }

    let list_container = mouse_area(container(list_col).width(Length::Fill).padding(Padding {
        left: 10.0,
        right: 15.0,
        top: 0.0,
        bottom: 0.0,
    }))
    .on_press(crate::gui::app::Message::LibraryDeselect);

    let main_scroll = standard_scrollable(
        crate::gui::library::LIBRARY_SCROLL_ID.clone(),
        list_container,
        iced::widget::scrollable::Direction::Vertical(standard_scrollbar()),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .on_scroll(crate::gui::app::Message::LibraryScroll);

    let content: Element<'a, crate::gui::app::Message> =
        if let Some((ref st_name, st_collapsed, st_albums, st_songs, st_duration)) =
            sticky_artist_info
        {
            let is_header_selected =
                manager
                    .selected_items
                    .contains(&crate::gui::library::LibraryListItem::Artist(
                        st_name.clone(),
                    ));

            let is_artist_playing = manager
                .artist_groups
                .iter()
                .find(|g| g.name == *st_name)
                .map(|g| {
                    g.songs
                        .iter()
                        .any(|s| s.full_file_path.as_ref() == playing_path)
                })
                .unwrap_or(false);

            let sticky_overlay = container(artist_header_widget(
                st_name.clone(),
                st_collapsed,
                is_header_selected,
                is_artist_playing,
                st_albums,
                st_songs,
                st_duration,
                32.0,
                crate::gui::app::Message::SelectArtistHeader(st_name.clone()),
                crate::gui::app::Message::ToggleArtistExpansion(st_name.clone()),
            ))
            .width(Length::Fill)
            .height(Length::Fixed(32.0))
            .padding(Padding {
                left: 10.0,
                right: 15.0,
                top: 0.0,
                bottom: 0.0,
            })
            .align_y(iced::alignment::Vertical::Top);

            iced::widget::stack![main_scroll, sticky_overlay].into()
        } else {
            main_scroll.into()
        };

    content
}
