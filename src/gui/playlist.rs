use iced::{
    widget::{button, column, container, row, scrollable, text, text_input, svg, mouse_area, Space},
    Alignment, Color, Element, Length, Theme, Background, Padding,
};
use crate::audio::AudioManager;
use crate::db::database::{PlaylistFolderGroup, PlaylistSongRef, ShuffleSession};
use crate::gui::app::Message;
use crate::gui::theme::*;
use crate::utils::{format_duration, truncate_text};

// ============================================================
// Estado del PlaylistManager
// ============================================================

pub struct PlaylistManager {
    /// ID de la playlist activa (referencia a BD)
    pub active_playlist_id: i64,

    /// Canciones de la playlist activa, agrupadas por carpeta
    pub groups: Vec<PlaylistFolderGroup>,

    /// Índice de la canción que se está reproduciendo (dentro del grupo aplanado)
    pub playing_song_idx: Option<usize>,

    /// Índice del item seleccionado para navegación por teclado
    pub selected_song_idx: Option<usize>,

    /// Estado shuffle/repeat
    pub shuffle_active: bool,
    pub repeat_mode: u8, // 0=Off, 1=All, 2=One

    /// Búsqueda local dentro de la playlist
    pub search_query: String,
    pub filtered_groups: Option<Vec<PlaylistFolderGroup>>,

    /// Estado de la barra de pestañas
    pub show_tab_dropdown: bool,

    /// IDs de pestañas visibles (para overflow management)
    pub visible_tabs_count: Option<usize>,

    /// Sesión de shuffle activa
    pub shuffle_session: Option<ShuffleSession>,

    // ============================================================
    // Campos legacy para compatibilidad con app.rs existente
    // Se eliminarán cuando se migre app.rs completamente.
    // ============================================================
    pub lists: Vec<(String, Vec<PlaylistItem>)>,
    pub active_list_idx: usize,
}

impl Default for PlaylistManager {
    fn default() -> Self {
        Self {
            active_playlist_id: 1,
            groups: Vec::new(),
            playing_song_idx: None,
            selected_song_idx: None,
            shuffle_active: false,
            repeat_mode: 0,
            search_query: String::new(),
            filtered_groups: None,
            show_tab_dropdown: false,
            visible_tabs_count: None,
            shuffle_session: None,
            lists: vec![
                ("Default".into(), Vec::new()),
                ("Favoritos".into(), Vec::new()),
            ],
            active_list_idx: 0,
        }
    }
}

/// Aplanar todos los grupos en un solo índice lineal
pub fn flatten_groups(groups: &[PlaylistFolderGroup]) -> Vec<&PlaylistSongRef> {
    groups.iter()
        .flat_map(|g| g.songs.iter())
        .collect()
}

/// Contar items lineales (separadores + canciones)
pub fn count_linear_items(groups: &[PlaylistFolderGroup]) -> usize {
    groups.iter().map(|g| 1 + g.songs.len()).sum()
}

impl PlaylistManager {
    pub fn apply_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_groups = None;
            return;
        }

        let query = self.search_query.to_lowercase();
        let mut filtered: Vec<PlaylistFolderGroup> = Vec::new();

        for group in &self.groups {
            let matching_songs: Vec<PlaylistSongRef> = group.songs.iter()
                .filter(|s| {
                    s.title.to_lowercase().contains(&query)
                        || s.artist_name.to_lowercase().contains(&query)
                        || s.album_title.to_lowercase().contains(&query)
                })
                .cloned()
                .collect();

            if !matching_songs.is_empty() {
                let mut g = group.clone();
                g.songs = matching_songs;
                g.total_duration = g.songs.iter().map(|s| s.duration).sum();
                filtered.push(g);
            }
        }

        self.filtered_groups = if filtered.is_empty() { None } else { Some(filtered) };
    }

    /// Obtener los grupos activos (filtrados o no)
    pub fn active_groups(&self) -> &[PlaylistFolderGroup] {
        self.filtered_groups.as_ref().unwrap_or(&self.groups)
    }

    /// Obtener la canción en un índice lineal
    pub fn get_song_at_linear_index(&self, linear_idx: usize) -> Option<&PlaylistSongRef> {
        let groups = self.active_groups();
        let mut count = 0;
        for group in groups {
            if linear_idx == count {
                return None; // Separador
            }
            let song_local_idx = linear_idx - count - 1;
            if song_local_idx < group.songs.len() {
                return Some(&group.songs[song_local_idx]);
            }
            count += 1 + group.songs.len();
        }
        None
    }

    /// Toggle enabled de una canción por índice lineal
    pub fn toggle_song_enabled_at_linear_index(&mut self, linear_idx: usize) -> Option<bool> {
        let groups = if self.filtered_groups.is_some() {
            self.filtered_groups.as_mut().unwrap()
        } else {
            &mut self.groups
        };

        let mut count = 0;
        for group in groups {
            if linear_idx == count {
                return None; // Separador
            }
            let song_local_idx = linear_idx - count - 1;
            if song_local_idx < group.songs.len() {
                let s = &mut group.songs[song_local_idx];
                s.enabled = !s.enabled;
                return Some(s.enabled);
            }
            count += 1 + group.songs.len();
        }
        None
    }

    /// Obtener información del grupo en un índice lineal
    pub fn get_group_info_at_linear_index(&self, linear_idx: usize) -> Option<(usize, bool)> {
        let groups = self.active_groups();
        let mut count = 0;
        for (gi, group) in groups.iter().enumerate() {
            if linear_idx == count {
                return Some((gi, true)); // Es el divisor
            }
            let song_local_idx = linear_idx - count - 1;
            if song_local_idx < group.songs.len() {
                return Some((gi, false)); // Es una canción
            }
            count += 1 + group.songs.len();
        }
        None
    }

    /// Toggle habilitado de todo un grupo
    pub fn toggle_group_enabled_at_linear_index(&mut self, linear_idx: usize) {
        if let Some((gi, is_folder)) = self.get_group_info_at_linear_index(linear_idx) {
            if !is_folder { return; }

            let groups = if self.filtered_groups.is_some() {
                self.filtered_groups.as_mut().unwrap()
            } else {
                &mut self.groups
            };

            if let Some(group) = groups.get_mut(gi) {
                // Si alguna está habilitada, deshabilitamos todas. Si no, habilitamos todas.
                let any_enabled = group.songs.iter().any(|s| s.enabled);
                let new_state = !any_enabled;
                for s in &mut group.songs {
                    s.enabled = new_state;
                }
            }
        }
    }

    /// Obtiene el índice lineal de una canción por su song_id de base de datos.
    pub fn get_linear_index_by_song_id(&self, song_id: i64) -> Option<usize> {
        let groups = self.active_groups();
        let mut count = 0;
        for group in groups {
            count += 1; // Separador
            for (idx, song) in group.songs.iter().enumerate() {
                if song.song_id == song_id {
                    return Some(count + idx);
                }
            }
            count += group.songs.len();
        }
        None
    }

    /// Activa el modo shuffle generando una nueva sesión aleatoria.
    pub fn activate_shuffle(&mut self) {
        let groups = self.active_groups();
        let mut songs: Vec<i64> = groups.iter()
            .flat_map(|g| g.songs.iter())
            .filter(|s| s.enabled)
            .map(|s| s.song_id)
            .collect();
        
        if songs.is_empty() { 
            self.shuffle_active = false;
            return; 
        }

        // Fisher-Yates Shuffle
        for i in (1..songs.len()).rev() {
            let j = fastrand::usize(0..=i);
            songs.swap(i, j);
        }

        // Si hay una canción sonando, ponerla al principio del historial para no interrumpir el flujo
        let current_song_id = self.playing_song_idx
            .and_then(|idx| self.get_song_at_linear_index(idx))
            .map(|s| s.song_id);

        if let Some(sid) = current_song_id {
            // Eliminar de la lista aleatoria si ya está ahí (para que no se repita pronto)
            songs.retain(|&id| id != sid);
        }

        self.shuffle_session = Some(ShuffleSession {
            session_id: format!("sess-{}", fastrand::u64(..)),
            shuffle_order: songs,
            current_position: 0,
            history: current_song_id.into_iter().collect(),
        });
        self.shuffle_active = true;
    }

    /// Desactiva el modo shuffle.
    pub fn deactivate_shuffle(&mut self) {
        self.shuffle_active = false;
        self.shuffle_session = None;
    }
}

// ============================================================
// Vista principal del módulo de Playlist
// ============================================================

const TABS_BAR_HEIGHT: f32 = 40.0;
const BOTTOM_BAR_HEIGHT: f32 = 40.0;
const TABS_PADDING_H: u16 = 10;

pub fn view<'a>(manager: &'a PlaylistManager, _audio_manager: &AudioManager) -> Element<'a, Message> {
    let tabs_container = build_tabs_bar(manager);
    let playlist_scroll = build_song_list(manager);
    let bottom_container = build_bottom_bar(manager);

    container(
        column![
            tabs_container,
            playlist_scroll,
            bottom_container
        ]
    )
    .width(Length::Fixed(400.0))
    .height(Length::Fill)
    .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
    .into()
}

// ============================================================
// Barra superior de pestañas
// ============================================================

fn build_tabs_bar<'a>(manager: &'a PlaylistManager) -> Element<'a, Message> {
    let playlists: Vec<(&'static str, i64, bool)> = vec![
        ("Archivos locales", 1, true),
        ("Default", 2, true),
    ];

    let mut tabs_row = row![]
        .spacing(10)
        .padding([0, TABS_PADDING_H as u16])
        .align_y(Alignment::Center);

    let total_width: f32 = 400.0;
    let mut used_width: f32 = TABS_PADDING_H as f32 * 2.0;
    let mut visible_count: usize = 0;
    let chevron_width: f32 = 38.0; // 28 + 10 spacing

    for (i, (name, id, is_system)) in playlists.iter().enumerate() {
        let is_active = *id == manager.active_playlist_id;
        let estimated_tab_width = estimate_tab_width(name);
        let force_visible = *is_system && i < 2;

        if force_visible || used_width + estimated_tab_width + chevron_width <= total_width {
            let tab_color = if is_active { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };
            let tab = text(*name)
                .size(14)
                .color(tab_color)
                .font(FONT_INTER_SANS_MEDIUM);

            let tab_btn = button(tab)
                .padding([2, 0])
                .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
                .on_press(Message::SwitchPlaylist(*id));

            tabs_row = tabs_row.push(tab_btn);
            used_width += estimated_tab_width + 10.0;
            visible_count += 1;
        } else {
            break;
        }
    }

    let has_overflow = visible_count < playlists.len();
    if has_overflow {
        tabs_row = tabs_row.push(build_tab_dropdown_button(manager));
    }

    let mut content = column![tabs_row];

    if manager.show_tab_dropdown && has_overflow {
        content = content.push(build_dropdown_menu(&playlists, visible_count));
    }

    container(content)
        .width(Length::Fill)
        .height(Length::Fixed(TABS_BAR_HEIGHT))
        .align_y(iced::alignment::Vertical::Center)
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST))
        .into()
}

fn estimate_tab_width(name: &str) -> f32 {
    name.chars().count() as f32 * 7.5 + 20.0
}

fn build_tab_dropdown_button<'a>(_manager: &'a PlaylistManager) -> Element<'a, Message> {
    let chevron = svg(svg::Handle::from_path("assets/icons/arrow-down-chevron.svg"))
        .width(28)
        .height(28)
        .style(|_t: &Theme, _s| svg::Style { color: Some(COLOR_TEXT_SECONDARY) });

    button(chevron)
        .padding(4)
        .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
        .on_press(Message::ToggleTabDropdown)
        .into()
}

fn build_dropdown_menu<'a>(
    playlists: &[(&'static str, i64, bool)],
    visible_count: usize,
) -> Element<'a, Message> {
    let mut items = column![]
        .padding([5, TABS_PADDING_H as u16])
        .spacing(4);

    for (i, (name, id, _is_system)) in playlists.iter().enumerate().skip(visible_count) {
        let _ = i;
        let item = text(*name)
            .size(14)
            .color(COLOR_TEXT_SECONDARY)
            .font(FONT_INTER_SANS_MEDIUM);

        let item_btn = button(item)
            .width(Length::Fill)
            .padding([6, 10])
            .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
            .on_press(Message::SwitchPlaylist(*id));

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

const SONG_ROW_HEIGHT: f32 = 46.0;
const FOLDER_SEPARATOR_HEIGHT: f32 = 46.0;

fn build_song_list<'a>(manager: &'a PlaylistManager) -> Element<'a, Message> {
    let groups = manager.active_groups();

    let mut songs_col = column![].spacing(0).padding(Padding { top: 0.0, right: 9.0, bottom: 0.0, left: 0.0 });

    let mut linear_idx = 0;

    // Detectar si hay un grupo seleccionado actualmente
    let selected_group_idx = manager.selected_song_idx.and_then(|idx| {
        manager.get_group_info_at_linear_index(idx).and_then(|(gi, is_folder)| {
            if is_folder { Some(gi) } else { None }
        })
    });

    for (gi, group) in groups.iter().enumerate() {
        // Separador de carpeta
        let is_group_selected = selected_group_idx == Some(gi);
        let is_folder_selected = manager.selected_song_idx == Some(linear_idx);
        let any_song_enabled = group.songs.iter().any(|s| s.enabled);
        
        let song_count = group.songs.len();
        let total_dur = format_duration(group.total_duration);
        let folder_sep = build_folder_separator(&group.folder_name, song_count, total_dur, linear_idx, is_folder_selected, any_song_enabled);
        songs_col = songs_col.push(folder_sep);
        linear_idx += 1;

        // Canciones del grupo
        for song in group.songs.iter() {
            let is_playing = manager.playing_song_idx == Some(linear_idx);
            let is_selected = manager.selected_song_idx == Some(linear_idx);
            let song_row = build_song_row(song, linear_idx, is_playing, is_selected, is_group_selected);
            songs_col = songs_col.push(song_row);
            linear_idx += 1;
        }
    }

    if groups.is_empty() {
        songs_col = songs_col.push(
            container(
                text("No hay canciones en esta lista")
                    .size(16)
                    .color(COLOR_TEXT_SECONDARY)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fill)
            .padding(15)
        );
    }

    scrollable(songs_col)
        .height(Length::Fill)
        .direction(iced::widget::scrollable::Direction::Vertical(
            iced::widget::scrollable::Scrollbar::new()
                .width(4)
                .margin(0)
                .scroller_width(4)
        ))
        .style(crate::gui::widgets::custom_scrollbar_style)
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
    let indicator_color = if is_enabled { COLOR_TEXT_SECONDARY } else { Color::TRANSPARENT };
    
    let indicator = text("•")
        .size(19)
        .color(indicator_color);

    let name_text = text(truncate_text(folder_name, 40))
        .size(14)
        .color(COLOR_TEXT_SECONDARY)
        .font(FONT_INTER_SANS_MEDIUM)
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::None);

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
        .on_press(Message::ToggleGroupEnabled(linear_idx)),
        Space::new().width(Length::Fixed(5.0)),
        name_text,
        Space::new().width(Length::Shrink),
        stats_text
    ]
    .spacing(0)
    .align_y(Alignment::Center);

    let bg_color = if is_selected { COLOR_CONTRAST } else { Color::TRANSPARENT };

    mouse_area(
        container(
            column![
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fixed(FOLDER_SEPARATOR_HEIGHT - 11.0))
                    .align_y(iced::alignment::Vertical::Center),
                container(Space::new().width(Length::Fill).height(1.0))
                    .style(|_t: &Theme| container::Style::default().background(COLOR_TEXT_SECONDARY)),
                Space::new().height(10.0)
            ]
        )
        .width(Length::Fill)
        .padding(Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 10.0 })
        .style(move |_t: &Theme| container::Style {
            background: Some(Background::Color(bg_color)),
            ..container::Style::default()
        })
    )
    .on_press(Message::PlaySongIndex(linear_idx))
    .into()
}

fn build_song_row<'a>(
    song: &'a PlaylistSongRef,
    linear_idx: usize,
    is_playing: bool,
    is_selected: bool,
    is_group_selected: bool,
) -> Element<'a, Message> {
    let text_color = if is_playing { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };
    let duration_color = if is_playing { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };

    // Indicador •
    let indicator_color = if !song.enabled {
        Color::TRANSPARENT
    } else if is_playing {
        COLOR_TEXT_PRIMARY
    } else {
        COLOR_TEXT_SECONDARY
    };
    let indicator = text("•")
        .size(17)
        .color(indicator_color);

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
            .font(FONT_INTER_SANS_MEDIUM)
    )
    .width(Length::Fixed(prefix_width))
    .align_x(Alignment::End);

    let title_text = text(truncate_text(&song.title, 40))
        .size(13)
        .color(text_color)
        .font(FONT_INTER_SANS_MEDIUM)
        .width(Length::Fill)
        .wrapping(iced::widget::text::Wrapping::None);

    let duration_text = text(format_duration(song.duration))
        .size(13)
        .color(duration_color)
        .font(FONT_INTER_SANS_MEDIUM)
        .wrapping(iced::widget::text::Wrapping::None);

    let row1 = row![
        prefix_container,
        title_text,
        duration_text
    ]
    .spacing(prefix_spacing)
    .align_y(Alignment::Center);

    // Fila 2: "Artista - Álbum :: Año" (Alineada con el Título de la Fila 1)
    let year_str = song.year.as_deref().unwrap_or("-");
    let row2_text = format!("{} - {} :: {}", song.artist_name, song.album_title, year_str);
    
    let row2 = row![
        Space::new().width(Length::Fixed(prefix_width + prefix_spacing)),
        text(truncate_text(&row2_text, 45))
            .size(13)
            .color(text_color) // Cambia a primario si está reproduciendo
            .font(FONT_INTER_SANS_MEDIUM)
            .width(Length::Fill)
            .wrapping(iced::widget::text::Wrapping::None)
    ]
    .spacing(0)
    .align_y(Alignment::Center);

    let content = column![row1, row2]
        .spacing(0)
        .padding([0, 0]);

    let bg_color = if is_selected {
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
        .on_press(Message::ToggleSongEnabled(linear_idx)),
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
            .padding(Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 15.0 })
            .style(move |_t: &Theme| container::Style {
                background: Some(Background::Color(bg_color)),
                ..container::Style::default()
            })
    )
    .on_press(Message::PlaySongIndex(linear_idx))
    .interaction(iced::mouse::Interaction::Pointer)
    .into()
}

// ============================================================
// Barra inferior
// ============================================================

fn build_bottom_bar<'a>(manager: &'a PlaylistManager) -> Element<'a, Message> {
    let search_box = text_input("Buscar...", &manager.search_query)
        .on_input(Message::PlaylistSearchChanged)
        .width(Length::Fixed(170.0))
        .size(13)
        .padding([6, 10])
        .font(FONT_INTER_SANS_MEDIUM);

    let eq_btn = icon_button("equalizer-rounded.svg", false, Message::ToggleAudioCenter);
    let (repeat_icon, repeat_active) = if manager.repeat_mode == 2 {
        ("repeat-one-rounded-outlined.svg", true)
    } else {
        ("repeat-rounded-outlined.svg", manager.repeat_mode > 0)
    };
    let shuffle_btn = icon_button("shuffle-rounded-outlined.svg", manager.shuffle_active, Message::ToggleShuffle);
    let repeat_btn = icon_button(repeat_icon, repeat_active, Message::ToggleRepeat);
    let lyrics_icon = icon_button("lyrics-outlined.svg", false, Message::ToggleLyrics);

    let icons_group = row![
        container(lyrics_icon).padding(Padding { top: 3.0, right: 0.0, bottom: 0.0, left: 0.0 }),
        shuffle_btn,
        repeat_btn,
        eq_btn
    ]
        .spacing(10)
        .align_y(Alignment::Center);

    let bar_content = row![
        container(search_box).center_y(Length::Fill),
        Space::new().width(Length::Fill),
        container(icons_group).center_y(Length::Fill)
    ]
    .padding([0, 15])
    .spacing(10)
    .align_y(Alignment::Center);

    container(bar_content)
        .width(Length::Fill)
        .height(Length::Fixed(BOTTOM_BAR_HEIGHT))
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST))
        .into()
}

fn icon_button<'a>(icon_name: &str, active: bool, msg: Message) -> Element<'a, Message> {
    let icon_color = if active {
        COLOR_ACCENT
    } else {
        COLOR_TEXT_SECONDARY
    };

    let icon = svg(svg::Handle::from_path(format!("assets/icons/{}", icon_name)))
        .width(24)
        .height(24)
        .style(move |_t: &Theme, _s| svg::Style { color: Some(icon_color) });

    button(icon)
        .padding(2)
        .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
        .on_press(msg)
        .into()
}

// ============================================================
// Compatibilidad temporal con app.rs existente
// ============================================================

/// Estructura temporal para compatibilidad con app.rs que espera PlaylistItem.
/// Se eliminará cuando se migre app.rs completamente a la nueva estructura.
#[derive(Clone, PartialEq, Debug)]
pub struct PlaylistItem {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_sec: f32,
    pub year: String,
    pub path: String,
    pub cover_cache_path: Option<String>,
}

impl PlaylistManager {
    /// Obtiene la siguiente canción para precarga de cover (compatibilidad temporal).
    pub fn get_next_song(&self) -> Option<PlaylistItem> {
        let groups = self.active_groups();
        let current = self.playing_song_idx?;
        let total = count_linear_items(groups);

        let next_idx = if self.repeat_mode == 2 {
            current // Repeat one
        } else if self.shuffle_active {
            return None; // No se puede predecir en shuffle
        } else {
            let n = current + 1;
            if n < total { n } else if self.repeat_mode == 1 { 0 } else { return None; }
        };

        self.get_song_at_linear_index(next_idx).map(|s| PlaylistItem {
            title: s.title.clone(),
            artist: s.artist_name.clone(),
            album: s.album_title.clone(),
            duration_sec: s.duration as f32,
            year: s.year.clone().unwrap_or_default(),
            path: s.file_path.clone(),
            cover_cache_path: s.cover_path.clone(),
        })
    }

    /// Avanza a la siguiente canción (auto-advance EOF).
    pub fn play_next(&mut self, audio_manager: &AudioManager) {
        if self.repeat_mode == 2 {
            // Repeat one
            if let Some(idx) = self.playing_song_idx {
                if let Some(song) = self.get_song_at_linear_index(idx) {
                    let _ = audio_manager.load_file(&song.file_path);
                    audio_manager.play();
                }
            }
            return;
        }

        if self.shuffle_active {
            let next_song_id = if let Some(session) = &mut self.shuffle_session {
                if session.current_position < session.shuffle_order.len() {
                    let song_id = session.shuffle_order[session.current_position];
                    session.history.push(song_id);
                    session.current_position += 1;
                    Some(song_id)
                } else {
                    None
                }
            } else { None };

            if let Some(song_id) = next_song_id {
                if let Some(l_idx) = self.get_linear_index_by_song_id(song_id) {
                    if let Some(song) = self.get_song_at_linear_index(l_idx) {
                        let path = song.file_path.clone();
                        self.playing_song_idx = Some(l_idx);
                        let _ = audio_manager.load_file(&path);
                        audio_manager.play();
                        return;
                    }
                }
            } else if self.shuffle_active && self.repeat_mode == 1 {
                // Re-barajar si Repeat All está activo en shuffle y llegamos al final
                self.activate_shuffle();
                if !self.shuffle_session.as_ref().map(|s| s.shuffle_order.is_empty()).unwrap_or(true) {
                    self.play_next(audio_manager);
                }
                return;
            }
            if self.shuffle_active { return; } 
        }

        // Sequential logic
        let next_info = {
            let groups = self.active_groups();
            let total = count_linear_items(groups);
            if total == 0 { None } else {
                let current = self.playing_song_idx.unwrap_or(usize::MAX);
                self.find_next_enabled_song_internal(groups, current, total).and_then(|next| {
                    self.get_song_at_linear_index_from(groups, next).map(|s| (next, s.file_path.clone()))
                })
            }
        };

        if let Some((next_idx, path)) = next_info {
            self.playing_song_idx = Some(next_idx);
            let _ = audio_manager.load_file(&path);
            audio_manager.play();
        }
    }

    /// Retrocede a la canción anterior.
    pub fn play_prev(&mut self, audio_manager: &AudioManager) {
        if self.shuffle_active {
            let prev_song_id = if let Some(session) = &mut self.shuffle_session {
                if session.history.len() > 1 {
                    session.history.pop(); // Quitar la actual
                    if session.current_position > 0 {
                        session.current_position -= 1;
                    }
                    session.history.last().cloned()
                } else { None }
            } else { None };

            if let Some(prev_id) = prev_song_id {
                if let Some(l_idx) = self.get_linear_index_by_song_id(prev_id) {
                    if let Some(song) = self.get_song_at_linear_index(l_idx) {
                        let path = song.file_path.clone();
                        self.playing_song_idx = Some(l_idx);
                        let _ = audio_manager.load_file(&path);
                        audio_manager.play();
                        return;
                    }
                }
            }
        }

        let current = self.playing_song_idx.unwrap_or(0);
        let prev_info = {
            let groups = self.active_groups();
            let total = count_linear_items(groups);
            if total == 0 { None } else if current == 0 {
                if self.repeat_mode == 1 {
                    self.find_last_enabled(groups, total).and_then(|last| {
                        self.get_song_at_linear_index_from(groups, last).map(|s| (last, s.file_path.clone()))
                    })
                } else { None }
            } else {
                // Buscar hacia atrás
                let mut idx = current - 1;
                let mut found = None;
                loop {
                    if let Some(song) = self.get_song_at_linear_index_from(groups, idx) {
                        if song.enabled {
                            found = Some((idx, song.file_path.clone()));
                            break;
                        }
                    }
                    if idx == 0 { break; }
                    idx -= 1;
                }
                found
            }
        };

        if let Some((next_idx, path)) = prev_info {
            self.playing_song_idx = Some(next_idx);
            let _ = audio_manager.load_file(&path);
            audio_manager.play();
        }
    }

    fn find_next_enabled_song_internal(
        &self,
        groups: &[PlaylistFolderGroup],
        current: usize,
        total: usize,
    ) -> Option<usize> {
        if groups.is_empty() { return None; }

        let mut idx = if current == usize::MAX { 0 } else { current + 1 };
        let mut wrapped = false;

        loop {
            if idx >= total {
                if wrapped || self.repeat_mode != 1 { return None; }
                idx = 0;
                wrapped = true;
            }

            // Saltar separadores (índices que son inicio de grupo)
            if let Some(song) = self.get_song_at_linear_index_from(groups, idx) {
                if song.enabled {
                    return Some(idx);
                }
            }

            idx += 1;
        }
    }

    fn find_last_enabled(&self, groups: &[PlaylistFolderGroup], total: usize) -> Option<usize> {
        let mut idx = total;
        loop {
            if idx == 0 { return None; }
            idx -= 1;
            if let Some(song) = self.get_song_at_linear_index_from(groups, idx) {
                if song.enabled {
                    return Some(idx);
                }
            }
        }
    }

    fn get_song_at_linear_index_from<'b>(
        &self,
        groups: &'b [PlaylistFolderGroup],
        linear_idx: usize,
    ) -> Option<&'b PlaylistSongRef> {
        let mut count = 0;
        for group in groups {
            if linear_idx == count {
                return None; // Separador
            }
            let song_local_idx = linear_idx - count - 1;
            if song_local_idx < group.songs.len() {
                return Some(&group.songs[song_local_idx]);
            }
            count += 1 + group.songs.len();
        }
        None
    }

    fn find_next_enabled_song(&self, current_linear_idx: usize) -> Option<usize> {
        let groups = self.active_groups();
        let total = count_linear_items(groups);
        if total == 0 { return None; }

        let mut idx = current_linear_idx + 1;
        let mut wrapped = false;

        loop {
            if idx >= total {
                if wrapped || self.repeat_mode != 1 { return None; }
                idx = 0;
                wrapped = true;
            }

            if let Some(song) = self.get_song_at_linear_index(idx) {
                if song.enabled {
                    return Some(idx);
                }
            }

            idx += 1;
        }
    }
}
