use crate::audio::AudioManager;
use crate::db::database::PlaylistSongRef;
use crate::gui::app::Message;
use crate::gui::dimensions::{
    BAR_BOTTOM, BAR_TAB, MARGIN_MD, PLAYLIST_PANEL_WIDTH, PLAYLIST_ROW, SPACE_MD, SPACE_SM,
    SPACE_XS, STROKE_HAIRLINE, WIDTH_SEARCH,
};
use crate::gui::theme::*;
use crate::gui::widgets::{standard_scrollable, standard_scrollbar};
use crate::utils::format_duration;
use iced::{
    Alignment, Background, Color, Element, Length, Padding, Theme,
    widget::{Space, button, column, container, mouse_area, row, svg, text},
};
use std::sync::atomic::{AtomicU64, Ordering};

use super::context_menu::get_item_context_menu_entries;
use super::state::{PlaylistItemType, PlaylistManager};

// Contadores de redraw para diagnóstico
static PLAYLIST_VIEW_REDRAWS: AtomicU64 = AtomicU64::new(0);

// ============================================================
// Constantes y IDs
// ============================================================

pub static PLAYLIST_SCROLL_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(iced::widget::Id::unique);
pub static PLAYLIST_TABS_SCROLL_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(iced::widget::Id::unique);
pub static PLAYLIST_SEARCH_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::new("playlist_search_input"));

// ============================================================
// Vista principal del módulo de Playlist
// ============================================================

pub fn view<'a>(
    manager: &'a PlaylistManager,
    _audio_manager: &AudioManager,
) -> Element<'a, Message> {
    let count = PLAYLIST_VIEW_REDRAWS.fetch_add(1, Ordering::Relaxed) + 1;
    if count % 100 == 0 {
        tracing::info!("Playlist view redraws: {}", count);
    }
    let tabs_container = build_tabs_bar(manager);
    let playlist_scroll = build_song_list(manager);
    let bottom_container = build_bottom_bar(manager);

    mouse_area(
        container(column![tabs_container, playlist_scroll, bottom_container])
            .width(Length::Fixed(PLAYLIST_PANEL_WIDTH))
            .height(Length::Fill)
            .style(|_t: &Theme| container::Style::default().background(COLOR_BG)),
    )
    .on_press(Message::PlaylistDeselect)
    .into()
}

// ============================================================
// Barra superior de pestañas
// ============================================================

fn build_tabs_bar<'a>(manager: &'a PlaylistManager) -> Element<'a, Message> {
    let playlists = &manager.playlists;

    // Use zero padding internally so the scrollable bounds determine visibility
    let mut tabs_row = row![]
        .spacing(SPACE_XS)
        .padding([0.0, 0.0])
        .align_y(Alignment::Center);

    let mut total_tabs_width: f32 = 0.0;

    for p_data in playlists.iter() {
        let name = &p_data.name;
        let id = p_data.id;

        let is_active = id == manager.active_playlist_id;
        let tab_color = if is_active {
            COLOR_TEXT_PRIMARY
        } else {
            COLOR_TEXT_SECONDARY
        };
        let tab = text(name.clone())
            .size(14)
            .color(tab_color)
            .font(FONT_INTER_SANS_MEDIUM);

        let tab_btn = button(tab)
            .padding([SPACE_XS, SPACE_XS])
            .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
            .on_press(Message::SwitchPlaylist(id, false));

        let tab_entries = vec![
            crate::gui::widgets::ContextMenuEntry {
                label: "Reproducir".to_string(),
                icon: Some("playlist-play-straight.svg".to_string()),
                action: Some(Message::SwitchPlaylist(id, true)),
            },
            crate::gui::widgets::ContextMenuEntry {
                label: "Nueva lista".to_string(),
                icon: Some("playlist-add-straight.svg".to_string()),
                action: Some(Message::OpenDialog(
                    crate::gui::app::ActiveDialog::CreatePlaylist {
                        name: "".into(),
                        pending_add_songs: Vec::new(),
                    },
                )),
            },
            crate::gui::widgets::ContextMenuEntry {
                label: "Renombrar lista".to_string(),
                icon: Some("playlist-edit-straight.svg".to_string()),
                action: Some(Message::OpenDialog(
                    crate::gui::app::ActiveDialog::RenamePlaylist {
                        id,
                        current_name: name.clone(),
                        new_name: name.clone(),
                    },
                )),
            },
            crate::gui::widgets::ContextMenuEntry {
                label: "Eliminar lista".to_string(),
                icon: Some("playlist-remove-straight.svg".to_string()),
                action: Some(Message::OpenDialog(
                    crate::gui::app::ActiveDialog::DeleteConfirm {
                        id,
                        name: name.clone(),
                    },
                )),
            },
            crate::gui::widgets::ContextMenuEntry {
                label: "".to_string(), // Divisor
                icon: None,
                action: None,
            },
            crate::gui::widgets::ContextMenuEntry {
                label: "Guardar lista".to_string(),
                icon: Some("playlist-add-check-straight.svg".to_string()),
                action: Some(Message::OpenDialog(
                    crate::gui::app::ActiveDialog::ExportConfirm {
                        id,
                        name: name.clone(),
                        format: crate::gui::app::ExportFormat::M3U8,
                        mode: crate::gui::app::ExportMode::SingleFile,
                    },
                )),
            },
            crate::gui::widgets::ContextMenuEntry {
                label: "Importar lista".to_string(),
                icon: Some("import-straight.svg".to_string()),
                action: Some(Message::OpenPlaylistFilePicker),
            },
            crate::gui::widgets::ContextMenuEntry {
                label: "Exportar lista".to_string(),
                icon: Some("export-straight.svg".to_string()),
                action: Some(Message::OpenDialog(
                    crate::gui::app::ActiveDialog::ExportConfirm {
                        id,
                        name: name.clone(),
                        format: crate::gui::app::ExportFormat::M3U8,
                        mode: crate::gui::app::ExportMode::PortableFolder,
                    },
                )),
            },
        ];

        tabs_row = tabs_row.push(
            mouse_area(tab_btn).on_right_press(Message::RequestContextMenu(tab_entries.clone())),
        );
        total_tabs_width += estimate_tab_width(name) + 5.0;
    }

    // Usamos padding 12 a la izquierda para el corte visual, y el limite real a la derecha
    let bar_width = 380.0;
    let has_overflow = total_tabs_width > bar_width;

    let scrollable_tabs = iced::widget::scrollable(tabs_row)
        .id(PLAYLIST_TABS_SCROLL_ID.clone())
        .direction(iced::widget::scrollable::Direction::Horizontal(
            iced::widget::scrollable::Scrollbar::new()
                .width(0)
                .scroller_width(0)
                .margin(0),
        ))
        .on_scroll(Message::PlaylistTabsScrolled);

    let mut bar_content =
        row![
            container(scrollable_tabs)
                .width(Length::Fill)
                .padding(iced::Padding {
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                    left: 12.0
                }),
        ]
        .align_y(Alignment::Center);

    if has_overflow {
        bar_content = bar_content.push(build_tab_dropdown_button(manager));
    }

    let content = column![bar_content];

    let bar_entries = vec![
        crate::gui::widgets::ContextMenuEntry {
            label: "Nueva lista".to_string(),
            icon: Some("playlist-add-straight.svg".to_string()),
            action: Some(Message::OpenDialog(
                crate::gui::app::ActiveDialog::CreatePlaylist {
                    name: "".into(),
                    pending_add_songs: Vec::new(),
                },
            )),
        },
        crate::gui::widgets::ContextMenuEntry {
            label: "Importar lista".to_string(),
            icon: Some("import-straight.svg".to_string()),
            action: Some(Message::OpenPlaylistFilePicker),
        },
    ];

    mouse_area(
        container(content)
            .width(Length::Fill)
            .height(Length::Fixed(BAR_TAB))
            .align_y(iced::alignment::Vertical::Center)
            .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST)),
    )
    .on_right_press(Message::RequestContextMenu(bar_entries))
    .into()
}

pub fn estimate_tab_width(name: &str) -> f32 {
    name.chars().count() as f32 * 7.5 + 10.0
}

fn build_tab_dropdown_button<'a>(manager: &'a PlaylistManager) -> Element<'a, Message> {
    let icon_path = if manager.show_tab_dropdown {
        "assets/icons/arrow-up-chevron.svg"
    } else {
        "assets/icons/arrow-down-chevron.svg"
    };

    let chevron = svg(svg::Handle::from_path(icon_path))
        .width(28)
        .height(28)
        .style(|_t: &Theme, _s| svg::Style {
            color: Some(COLOR_TEXT_SECONDARY),
        });

    button(chevron)
        .padding(SPACE_XS)
        .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
        .on_press(Message::ToggleTabDropdown)
        .into()
}

fn build_dropdown_menu<'a>(
    playlists: &[crate::db::database::PlaylistData],
    visible_count: usize,
) -> Element<'a, Message> {
    let mut items = column![].padding([SPACE_XS, SPACE_SM]).spacing(SPACE_XS);

    for p_data in playlists.iter().skip(visible_count) {
        let name = &p_data.name;
        let id = p_data.id;

        let item = text(name.clone())
            .size(14)
            .color(COLOR_TEXT_SECONDARY)
            .font(FONT_INTER_SANS_MEDIUM);

        let item_btn = button(item)
            .width(Length::Fill)
            .padding([SPACE_XS, SPACE_XS])
            .style(|_t: &Theme, _s| button::Style::default().with_background(COLOR_BG))
            .on_press(Message::SwitchPlaylist(id, false));

        items = items.push(item_btn);
    }

    container(items)
        .width(Length::Fill)
        .style(|_t: &Theme| container::Style {
            background: Some(Background::Color(COLOR_CONTRAST)),
            ..container::Style::default()
        })
        .into()
}

// ============================================================
// Lista de canciones con separadores de carpeta
// ============================================================

pub(in crate::gui::playlist) const SONG_ROW_HEIGHT: f32 = PLAYLIST_ROW;
pub(in crate::gui::playlist) const FOLDER_SEPARATOR_HEIGHT: f32 = PLAYLIST_ROW;

fn build_song_list<'a>(manager: &'a PlaylistManager) -> Element<'a, Message> {
    let groups = manager.active_groups();
    if groups.is_empty() {
        return mouse_area(
            container(
                text("No hay canciones en esta lista")
                    .size(16)
                    .color(COLOR_TEXT_SECONDARY)
                    .font(FONT_INTER_SANS_MEDIUM),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(MARGIN_MD),
        )
        .on_enter(Message::PlaylistMouseOver(true))
        .on_exit(Message::PlaylistMouseOver(false))
        .on_press(Message::PlaylistDeselect)
        .on_right_press(Message::RequestContextMenu(
            get_empty_playlist_context_menu_entries(),
        ))
        .into();
    }

    // 1. Obtener ítems expandidos (visibles en la estructura lógica)
    let visible_items = manager.get_visible_items();
    let total_items = visible_items.len();

    // 2. Parámetros de virtualización
    let viewport = manager.last_viewport.unwrap_or(iced::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 800.0,
    });
    let view_min = viewport.y;
    let view_max = view_min + viewport.height;

    // Margen de renderizado reducido para menos redraws
    let margin = if crate::utils::is_low_resource() {
        50.0
    } else {
        150.0
    };
    let render_min = (view_min - margin).max(0.0);
    let render_max = view_max + margin;

    let item_h = SONG_ROW_HEIGHT; // Ambas son 46.0

    // 3. Identificar rango de índices a renderizar
    let start_idx = (render_min / item_h).floor() as usize;
    let end_idx = (render_max / item_h).ceil() as usize;

    let start_idx = start_idx.min(total_items);
    let end_idx = end_idx.min(total_items);

    let top_space = start_idx as f32 * item_h;
    let bottom_space = (total_items.saturating_sub(end_idx)) as f32 * item_h;

    let mut songs_col = column![].spacing(0).padding(Padding {
        top: 0.0,
        right: 9.0,
        bottom: 0.0,
        left: 0.0,
    });

    // Espaciador superior
    if top_space > 0.0 {
        songs_col = songs_col.push(Space::new().height(Length::Fixed(top_space)));
    }

    // Renderizar solo los ítems en el rango visible
    for (item_type, linear_idx) in &visible_items[start_idx..end_idx] {
        match item_type {
            PlaylistItemType::Separator(g_idx, _) => {
                if let Some(group) = groups.get(*g_idx) {
                    let is_selected = manager.selected_idxs.contains(linear_idx);
                    let any_song_enabled = group.songs.iter().any(|s| s.enabled);
                    let total_dur = format_duration(group.total_duration);

                    songs_col = songs_col.push(build_folder_separator(
                        &group.folder_name,
                        group.songs.len(),
                        total_dur,
                        *linear_idx,
                        is_selected,
                        any_song_enabled,
                    ));
                }
            }
            PlaylistItemType::Song(g_idx, s_idx, _) => {
                if let Some(group) = groups.get(*g_idx) {
                    if let Some(song) = group.songs.get(*s_idx) {
                        let is_playing = manager.playing_song_idx == Some(*linear_idx);
                        let is_selected = manager.selected_idxs.contains(linear_idx);

                        // Un ítem de canción se considera "dentro de grupo seleccionado" si el separador del grupo lo está
                        let sep_idx = manager
                            .find_separator_index_for_anchor(group.first_item_id)
                            .unwrap_or(0);
                        let is_group_selected = manager.selected_idxs.contains(&sep_idx);

                        songs_col = songs_col.push(build_song_row(
                            song,
                            *linear_idx,
                            is_playing,
                            is_selected,
                            is_group_selected,
                        ));
                    }
                }
            }
        }
    }

    // Espaciador inferior
    if bottom_space > 0.0 {
        songs_col = songs_col.push(Space::new().height(Length::Fixed(bottom_space)));
    }

    let scroll = standard_scrollable(
        PLAYLIST_SCROLL_ID.clone(),
        songs_col,
        iced::widget::scrollable::Direction::Vertical(standard_scrollbar()),
    )
    .height(Length::Fill)
    .on_scroll(Message::PlaylistScrolled);

    mouse_area(scroll)
        .on_enter(Message::PlaylistMouseOver(true))
        .on_exit(Message::PlaylistMouseOver(false))
        .on_press(Message::PlaylistDeselect)
        .on_right_press(Message::RequestContextMenu(
            get_empty_playlist_context_menu_entries(),
        ))
        .into()
}

fn build_folder_separator<'a>(
    folder_name: &'a str,
    song_count: usize,
    total_duration: String,
    linear_idx: usize,
    is_selected: bool,
    is_enabled: bool,
) -> Element<'a, Message> {
    let indicator_color = if is_enabled {
        COLOR_TEXT_SECONDARY
    } else {
        Color::TRANSPARENT
    };

    let indicator = text("•").size(19).color(indicator_color);

    let name_text = crate::gui::widgets::smart_truncate_text(
        folder_name.to_string(),
        14.0,
        FONT_INTER_SANS_MEDIUM,
        COLOR_TEXT_SECONDARY,
    );

    let stats_text = text(format!("{} | {}", song_count, total_duration))
        .size(13)
        .color(COLOR_TEXT_SECONDARY)
        .font(FONT_INTER_SANS_MEDIUM)
        .wrapping(iced::widget::text::Wrapping::None);

    let content = row![
        mouse_area(
            container(indicator)
                .width(Length::Fixed(12.0))
                .height(Length::Fixed(FOLDER_SEPARATOR_HEIGHT))
                .align_y(Alignment::Center)
        )
        .on_press(Message::ToggleGroupEnabled(linear_idx, None)),
        Space::new().width(Length::Fixed(5.0)),
        name_text,
        Space::new().width(Length::Shrink),
        stats_text
    ]
    .spacing(0)
    .align_y(Alignment::Center);

    let bg_color = if is_selected {
        COLOR_CONTRAST
    } else {
        Color::TRANSPARENT
    };

    mouse_area(
        container(column![
            container(content)
                .width(Length::Fill)
                .height(Length::Fixed(FOLDER_SEPARATOR_HEIGHT - 11.0))
                .align_y(iced::alignment::Vertical::Center),
            container(Space::new().width(Length::Fill).height(STROKE_HAIRLINE)).style(
                move |_t: &Theme| {
                    let col = if is_selected {
                        COLOR_TEXT_PRIMARY
                    } else {
                        COLOR_TEXT_SECONDARY
                    };
                    container::Style::default().background(col)
                }
            ),
            Space::new().height(10.0)
        ])
        .width(Length::Fill)
        .padding(Padding {
            top: 0.0,
            right: 4.0,
            bottom: 0.0,
            left: 10.0,
        })
        .style(move |_t: &Theme| container::Style {
            background: Some(Background::Color(bg_color)),
            ..container::Style::default()
        }),
    )
    .on_press(Message::TogglePlaylistFolder(linear_idx))
    .on_right_press(Message::RequestContextMenu(get_item_context_menu_entries(
        linear_idx,
    )))
    .interaction(iced::mouse::Interaction::Pointer)
    .into()
}

fn build_song_row<'a>(
    song: &'a PlaylistSongRef,
    linear_idx: usize,
    is_playing: bool,
    is_selected: bool,
    is_group_selected: bool,
) -> Element<'a, Message> {
    let text_color = if is_playing {
        COLOR_TEXT_PRIMARY
    } else {
        COLOR_TEXT_SECONDARY
    };
    let duration_color = if is_playing {
        COLOR_TEXT_PRIMARY
    } else {
        COLOR_TEXT_SECONDARY
    };

    // Indicador •
    let indicator_color = if !song.enabled {
        Color::TRANSPARENT
    } else if is_playing {
        COLOR_TEXT_PRIMARY
    } else {
        COLOR_TEXT_SECONDARY
    };
    let indicator = text("•").size(17).color(indicator_color);

    // Fila 1: "Nº. Nombre de canción .... Duración"
    let track_prefix = if let Some(ref tn) = song.track_number {
        format!("{}.", tn)
    } else {
        format!("{:02}.", linear_idx)
    };

    let prefix_width = 20.0;
    let prefix_spacing = 5.0;

    let prefix_container = container(
        text(track_prefix)
            .size(13)
            .color(text_color)
            .font(FONT_INTER_SANS_MEDIUM),
    )
    .width(Length::Fixed(prefix_width))
    .align_x(Alignment::End);

    let title_text = crate::gui::widgets::smart_truncate_text(
        song.title.to_string(),
        13.0,
        FONT_INTER_SANS_MEDIUM,
        text_color,
    );

    let duration_text = text(format_duration(song.duration))
        .size(13)
        .color(duration_color)
        .font(FONT_INTER_SANS_MEDIUM)
        .wrapping(iced::widget::text::Wrapping::None);

    let row1 = row![prefix_container, title_text, duration_text]
        .spacing(prefix_spacing)
        .align_y(Alignment::Center);

    // Fila 2: "Artista - Álbum :: Año" (Alineada con el Título de la Fila 1)
    let year_str = song.year.as_deref().unwrap_or("-");
    let row2_text = format!(
        "{} - {} :: {}",
        song.artist_name, song.album_title, year_str
    );

    let row2 = row![
        Space::new().width(Length::Fixed(prefix_width + prefix_spacing)),
        crate::gui::widgets::smart_truncate_text(
            row2_text,
            13.0,
            FONT_INTER_SANS_MEDIUM,
            text_color
        )
    ]
    .spacing(0)
    .align_y(Alignment::Center);

    let content = column![row1, row2].spacing(2).padding([0, 0]);

    let bg_color = if is_selected || is_playing {
        COLOR_CONTRAST
    } else if is_group_selected {
        COLOR_CONTRAST
    } else {
        Color::TRANSPARENT
    };

    let song_row_inner = row![
        mouse_area(
            container(indicator)
                .width(Length::Fixed(12.0))
                .height(Length::Fixed(SONG_ROW_HEIGHT))
                .align_y(Alignment::Center)
        )
        .on_press(Message::ToggleSongEnabled(linear_idx, None)),
        Space::new().width(Length::Fixed(0.0)),
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Center)
    ]
    .spacing(0)
    .align_y(Alignment::Center);

    mouse_area(
        container(song_row_inner)
            .width(Length::Fill)
            .height(Length::Fixed(SONG_ROW_HEIGHT))
            .padding(Padding {
                top: 0.0,
                right: 4.0,
                bottom: 0.0,
                left: 15.0,
            })
            .style(move |_t: &Theme| container::Style {
                background: Some(Background::Color(bg_color)),
                ..container::Style::default()
            }),
    )
    .on_press(Message::PlaySongIndex(linear_idx))
    .on_right_press(Message::RequestContextMenu(get_item_context_menu_entries(
        linear_idx,
    )))
    .interaction(iced::mouse::Interaction::Pointer)
    .into()
}

/// Genera las opciones del menú contextual para el área vacía de la lista de reproducción.
fn get_empty_playlist_context_menu_entries() -> Vec<crate::gui::widgets::ContextMenuEntry<Message>>
{
    use crate::gui::widgets::ContextMenuEntry;
    vec![
        ContextMenuEntry {
            label: "Agregar archivo".to_string(),
            icon: Some("audio-file-outlined-straight.svg".to_string()),
            action: Some(Message::PlaylistAddFiles),
        },
        ContextMenuEntry {
            label: "Agregar carpeta".to_string(),
            icon: Some("folder-add-outlined.svg".to_string()),
            action: Some(Message::PlaylistAddFolder),
        },
    ]
}

// ============================================================
// Barra inferior
// ============================================================

fn build_bottom_bar<'a>(manager: &'a PlaylistManager) -> Element<'a, Message> {
    let search_box = crate::gui::widgets::standard_search_input(
        Some(PLAYLIST_SEARCH_ID.clone()),
        "Buscar...",
        &manager.search_query,
        Message::PlaylistSearchChanged,
        Message::PlaylistSearchChanged(String::new()),
        Length::Fixed(WIDTH_SEARCH),
    );

    let eq_btn = icon_button(
        "equalizer-straight.svg",
        false,
        Message::ToggleAudioCenter(Some(1)),
    );
    let shuffle_btn = icon_button(
        "shuffle-straight.svg",
        manager.shuffle_active,
        Message::ToggleShuffle,
    );
    let (repeat_icon, repeat_active) = if manager.repeat_mode == 2 {
        ("repeat-one-straight-outlined.svg", true)
    } else {
        ("repeat-straight-outlined.svg", manager.repeat_mode > 0)
    };
    let repeat_btn = icon_button(repeat_icon, repeat_active, Message::ToggleRepeat);
    let lyrics_icon = icon_button("lyrics-straight-outlined.svg", false, Message::ToggleLyrics);

    let icons_group = row![
        container(lyrics_icon).padding(Padding {
            top: 3.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0
        }),
        repeat_btn,
        shuffle_btn,
        eq_btn
    ]
    .spacing(SPACE_MD)
    .align_y(Alignment::Center);

    let bar_content = row![
        container(search_box).center_y(Length::Fill),
        Space::new().width(Length::Fill),
        container(icons_group).center_y(Length::Fill)
    ]
    .padding([0.0, MARGIN_MD])
    .spacing(SPACE_MD)
    .align_y(Alignment::Center);

    container(bar_content)
        .width(Length::Fill)
        .height(Length::Fixed(BAR_BOTTOM))
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST))
        .into()
}

fn icon_button<'a>(icon_name: &str, active: bool, msg: Message) -> Element<'a, Message> {
    let icon_color = if active {
        COLOR_ACCENT
    } else {
        COLOR_TEXT_SECONDARY
    };

    let icon = svg(svg::Handle::from_path(format!(
        "assets/icons/{}",
        icon_name
    )))
    .width(24)
    .height(24)
    .style(move |_t: &Theme, _s| svg::Style {
        color: Some(icon_color),
    });

    button(icon)
        .padding(2)
        .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
        .on_press(msg)
        .into()
}
