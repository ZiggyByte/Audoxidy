use iced::{
    widget::{button, column, container, row, scrollable, stack, text, text_input, Space, image},
    Alignment, Color, Element, Length, Theme, Task,
};
use std::sync::{Arc, Mutex};
use crate::db::Database;
use crate::gui::app::Message;
use crate::gui::theme::*;
use crate::gui::widgets::{artist_header_widget, library_song_row_widget};
use crate::utils::{format_duration, format_size, format_metadata, truncate_text, SortColumn};

/// ID estático para el scrollable de la biblioteca — garantiza que view y update usan EXACTAMENTE el mismo ID
pub static LIBRARY_SCROLL_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::unique());

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LibraryViewMode {
    Grid,
    DetailedList,
    ThumbnailList,
    SimpleList,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LibraryNavDir {
    Up,
    Down,
    Left,
    Right,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LibrarySource {
    Local,
    Spotify,
    YouTube,
    Qobuz,
    Deezer,
    Tidal,
}

#[derive(Debug, Clone, Default)]
pub struct LibraryStats {
    pub songs: u64,
    pub albums: u64,
    pub artists: u64,
    pub duration_secs: f64,
    pub size_bytes: f64,
}

#[derive(Debug, Clone)]
pub struct ArtistGroup {
    pub name: String,
    pub albums: std::collections::HashSet<String>,
    pub songs: Vec<crate::db::database::SongData>,
    pub duration_secs: f64,
}

pub struct LibraryManager {
    pub view_mode: LibraryViewMode,
    pub source: LibrarySource,
    pub sort_column: Option<SortColumn>,
    pub sort_ascending: Option<bool>,
    pub search_query: String,
    pub expanded_album: Option<String>,
    pub expanded_album_songs: Option<Vec<crate::db::database::SongData>>,
    pub cached_albums: Option<Vec<(String, String, String, String, String, Option<String>)>>,
    pub cached_all_songs: Option<Vec<crate::db::database::SongData>>,
    pub filtered_songs: Option<Vec<crate::db::database::SongData>>,
    pub artist_groups: Vec<ArtistGroup>,
    pub collapsed_artists: std::collections::HashSet<String>,
    pub view_menu_open: bool,
    pub add_menu_open: bool,
    pub total_songs: usize,
    pub total_albums: usize,
    pub total_artists: usize,
    pub total_duration_secs: f64,
    pub total_size_bytes: f64,
    
    pub column_widths: std::collections::HashMap<SortColumn, u16>,
    pub resizing_column: Option<SortColumn>,
    pub resizing_start_x: f32,
    pub resizing_start_w: u16,
    pub hovered_column: Option<SortColumn>,
    
    // Selection & keyboard navigation
    pub selected_header: Option<String>,
    pub selected_album: Option<String>,
    pub selected_song_idx: Option<usize>,
    pub albums_per_row: std::cell::Cell<usize>,
    pub library_area_width: f32,
    pub last_selected_album: Option<String>,
    pub last_selected_song_idx: Option<usize>,
    pub last_viewport: Option<iced::widget::scrollable::Viewport>,
    pub selection_stats: Option<LibraryStats>,
    pub artist_last_selection: std::collections::HashMap<String, usize>, // artist -> global_song_idx
}

impl Default for LibraryManager {
    fn default() -> Self {
        let mut column_widths = std::collections::HashMap::new();
        column_widths.insert(SortColumn::TrackNumber, 30);
        column_widths.insert(SortColumn::Title, 285);
        column_widths.insert(SortColumn::Artist, 155);
        column_widths.insert(SortColumn::AlbumArtist, 155);
        column_widths.insert(SortColumn::Album, 165);
        column_widths.insert(SortColumn::Genre, 135);
        column_widths.insert(SortColumn::Year, 50);
        column_widths.insert(SortColumn::Duration, 53);
        column_widths.insert(SortColumn::Format, 53);
        column_widths.insert(SortColumn::SampleRate, 80);
        column_widths.insert(SortColumn::Channels, 55);
        column_widths.insert(SortColumn::BitDepth, 60);
        column_widths.insert(SortColumn::Bitrate, 90);
        column_widths.insert(SortColumn::Size, 85);
        
        Self {
            view_mode: LibraryViewMode::Grid,
            source: LibrarySource::Local,
            sort_column: None,
            sort_ascending: None,
            search_query: String::new(),
            expanded_album: None,
            expanded_album_songs: None,
            cached_albums: None,
            cached_all_songs: None,
            filtered_songs: None,
            artist_groups: Vec::new(),
            collapsed_artists: std::collections::HashSet::new(),
            view_menu_open: false,
            add_menu_open: false,
            total_songs: 0,
            total_albums: 0,
            total_artists: 0,
            total_duration_secs: 0.0,
            total_size_bytes: 0.0,
            
            column_widths,
            resizing_column: None,
            resizing_start_x: 0.0,
            resizing_start_w: 0,
            hovered_column: None,
            selected_album: None,
            selected_song_idx: None,
            selected_header: None,
            albums_per_row: std::cell::Cell::new(5),
            library_area_width: 800.0,
            last_selected_album: None,
            last_selected_song_idx: None,
            last_viewport: None,
            selection_stats: None,
            artist_last_selection: std::collections::HashMap::new(),
        }
    }
}

impl LibraryManager {
    pub fn sort_songs(&self, songs: &mut [crate::db::database::SongData]) {
        if let Some(col_ref) = self.sort_column {
            let is_asc = self.sort_ascending.unwrap_or(true);
            songs.sort_by(|a, b| {
                let res = match col_ref {
                    SortColumn::TrackNumber => {
                        let tn_a = a.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let tn_b = b.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        tn_a.cmp(&tn_b)
                    },
                    SortColumn::Title => {
                        let tn_a = a.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let tn_b = b.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        a.title.cmp(&b.title).then(tn_a.cmp(&tn_b))
                    },
                    SortColumn::Artist => {
                        let tn_a = a.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let tn_b = b.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let art_a = a.artist.as_ref().unwrap_or(&String::from("Desconocido")).clone();
                        let art_b = b.artist.as_ref().unwrap_or(&String::from("Desconocido")).clone();
                        art_a.cmp(&art_b).then(a.release_year.cmp(&b.release_year)).then(a.album.cmp(&b.album)).then(tn_a.cmp(&tn_b))
                    },
                    SortColumn::AlbumArtist => {
                        let tn_a = a.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let tn_b = b.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let alb_art_a = a.album_artist.as_ref().unwrap_or(a.artist.as_ref().unwrap_or(&String::from("Desconocido"))).clone();
                        let alb_art_b = b.album_artist.as_ref().unwrap_or(b.artist.as_ref().unwrap_or(&String::from("Desconocido"))).clone();
                        alb_art_a.cmp(&alb_art_b).then(a.release_year.cmp(&b.release_year)).then(a.album.cmp(&b.album)).then(tn_a.cmp(&tn_b))
                    },
                    SortColumn::Album => {
                        let tn_a = a.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let tn_b = b.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let alb_art_a = a.album_artist.as_ref().unwrap_or(a.artist.as_ref().unwrap_or(&String::from("Desconocido"))).clone();
                        let alb_art_b = b.album_artist.as_ref().unwrap_or(b.artist.as_ref().unwrap_or(&String::from("Desconocido"))).clone();
                        a.album.cmp(&b.album).then(alb_art_a.cmp(&alb_art_b)).then(tn_a.cmp(&tn_b))
                    },
                    SortColumn::Format => {
                        a.format.cmp(&b.format)
                    },
                    SortColumn::Size => {
                        a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0))
                    },
                    SortColumn::SampleRate => {
                        a.sample_rate.unwrap_or(0).cmp(&b.sample_rate.unwrap_or(0))
                    },
                    SortColumn::Channels => {
                        a.channels.unwrap_or(0).cmp(&b.channels.unwrap_or(0))
                    },
                    SortColumn::Bitrate => {
                        let bit_a = if a.duration_secs.unwrap_or(0.0) > 0.0 { (a.size.unwrap_or(0) as f64 * 8.0) / (a.duration_secs.unwrap() * 1000.0) } else { 0.0 };
                        let bit_b = if b.duration_secs.unwrap_or(0.0) > 0.0 { (b.size.unwrap_or(0) as f64 * 8.0) / (b.duration_secs.unwrap() * 1000.0) } else { 0.0 };
                        bit_a.partial_cmp(&bit_b).unwrap_or(std::cmp::Ordering::Equal)
                    },
                    SortColumn::Genre => {
                        let tn_a = a.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let tn_b = b.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let alb_art_a = a.album_artist.as_ref().unwrap_or(a.artist.as_ref().unwrap_or(&String::from("Desconocido"))).clone();
                        let alb_art_b = b.album_artist.as_ref().unwrap_or(b.artist.as_ref().unwrap_or(&String::from("Desconocido"))).clone();
                        a.genre.cmp(&b.genre).then(alb_art_a.cmp(&alb_art_b)).then(a.album.cmp(&b.album)).then(tn_a.cmp(&tn_b))
                    },
                    SortColumn::Year => {
                        let tn_a = a.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let tn_b = b.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let alb_art_a = a.album_artist.as_ref().unwrap_or(a.artist.as_ref().unwrap_or(&String::from("Desconocido"))).clone();
                        let alb_art_b = b.album_artist.as_ref().unwrap_or(b.artist.as_ref().unwrap_or(&String::from("Desconocido"))).clone();
                        a.release_year.cmp(&b.release_year).then(alb_art_a.cmp(&alb_art_b)).then(a.album.cmp(&b.album)).then(tn_a.cmp(&tn_b))
                    },
                    SortColumn::Duration => {
                        let dur_a = a.duration_secs.unwrap_or(0.0);
                        let dur_b = b.duration_secs.unwrap_or(0.0);
                        dur_a.partial_cmp(&dur_b).unwrap_or(std::cmp::Ordering::Equal)
                    },
                    SortColumn::BitDepth => {
                        a.bit_depth.unwrap_or(0).cmp(&b.bit_depth.unwrap_or(0))
                    },
                };
                if is_asc { res } else { res.reverse() }
            });
        }
    }


    /// Maneja la navegación por teclado unificada entre cabeceras y canciones.
    /// Devuelve (NuevaCabecera, NuevoIndiceCanción, FocoEnCanción).
    pub fn handle_key_nav(&self, dir: LibraryNavDir) -> (Option<String>, Option<usize>) {
        let mut new_header = self.selected_header.clone();
        let mut new_song_idx = self.selected_song_idx;

        match dir {
            LibraryNavDir::Down => {
                if let Some(current_artist) = &self.selected_header {
                    let is_collapsed = self.collapsed_artists.contains(current_artist);
                    for (i, group) in self.artist_groups.iter().enumerate() {
                        if &group.name == current_artist {
                            if !is_collapsed && !group.songs.is_empty() {
                                let mut song_acc = 0;
                                for prev_g in &self.artist_groups[..i] {
                                    song_acc += prev_g.songs.len();
                                }
                                new_header = None;
                                new_song_idx = Some(song_acc);
                            } else if i + 1 < self.artist_groups.len() {
                                new_header = Some(self.artist_groups[i+1].name.clone());
                                new_song_idx = None;
                            }
                            break;
                        }
                    }
                } else if let Some(current_idx) = self.selected_song_idx {
                    let mut song_acc = 0;
                    for (i, group) in self.artist_groups.iter().enumerate() {
                        let group_len = group.songs.len();
                        if current_idx >= song_acc && current_idx < song_acc + group_len {
                            if current_idx + 1 < song_acc + group_len {
                                new_song_idx = Some(current_idx + 1);
                                new_header = None;
                            } else if i + 1 < self.artist_groups.len() {
                                new_header = Some(self.artist_groups[i+1].name.clone());
                                new_song_idx = None;
                            }
                            break;
                        }
                        song_acc += group_len;
                    }
                } else {
                    new_header = self.artist_groups.first().map(|g| g.name.clone());
                }
            },
            LibraryNavDir::Up => {
                if let Some(current_artist) = &self.selected_header {
                    for (i, group) in self.artist_groups.iter().enumerate() {
                        if &group.name == current_artist {
                            if i > 0 {
                                let prev_group = &self.artist_groups[i-1];
                                if !self.collapsed_artists.contains(&prev_group.name) && !prev_group.songs.is_empty() {
                                    let mut song_acc = 0;
                                    for prev_g in &self.artist_groups[..i] {
                                        song_acc += prev_g.songs.len();
                                    }
                                    new_header = None;
                                    new_song_idx = Some(song_acc - 1);
                                } else {
                                    new_header = Some(prev_group.name.clone());
                                    new_song_idx = None;
                                }
                            }
                            break;
                        }
                    }
                } else if let Some(current_idx) = self.selected_song_idx {
                    let mut song_acc = 0;
                    for group in &self.artist_groups {
                        let group_len = group.songs.len();
                        if current_idx >= song_acc && current_idx < song_acc + group_len {
                            if current_idx > song_acc {
                                new_song_idx = Some(current_idx - 1);
                                new_header = None;
                            } else {
                                new_header = Some(group.name.clone());
                                new_song_idx = None;
                            }
                            break;
                        }
                        song_acc += group_len;
                    }
                }
            },
            _ => {}
        }
        (new_header, new_song_idx)
    }

    /// Calcula la tarea de scroll para asegurar que el elemento seleccionado sea visible.
    /// Si 'force_top' es true, el elemento se moverá directamente a la parte superior.
    pub fn get_scroll_task<Message: 'static>(&self, force_top: bool) -> Task<Message> {
        if self.view_mode != LibraryViewMode::SimpleList {
            return Task::none();
        }

        if let Some(viewport) = &self.last_viewport {
            let view_min = viewport.absolute_offset().y;
            let view_max = view_min + viewport.bounds().height;
            let scroll_id = LIBRARY_SCROLL_ID.clone();

            let header_h = 32.0;
            let song_h = 32.0;

            let mut target_y = 0.0;
            let mut current_song_global = 0;
            let mut found_y = false;

            for group in &self.artist_groups {
                let is_collapsed = self.collapsed_artists.contains(&group.name);
                let group_songs = group.songs.len();

                if let Some(sel_h) = &self.selected_header {
                    if sel_h == &group.name {
                        found_y = true;
                        break;
                    }
                }

                if let Some(sel_idx) = self.selected_song_idx {
                    if sel_idx >= current_song_global && sel_idx < current_song_global + group_songs {
                        target_y += header_h;
                        target_y += (sel_idx - current_song_global) as f32 * song_h;
                        found_y = true;
                        break;
                    }
                }

                target_y += header_h;
                if !is_collapsed {
                    target_y += group_songs as f32 * song_h;
                }
                current_song_global += group_songs;
            }

            if found_y {
                let item_top = target_y;
                let item_height = if self.selected_header.is_some() { header_h } else { song_h };
                let item_bottom = item_top + item_height;
                let margin = 5.0; 

                if force_top {
                    return iced::widget::operation::scroll_to(scroll_id, iced::widget::operation::AbsoluteOffset { x: 0.0, y: (item_top - margin).max(0.0) });
                }

                if item_top < view_min + margin {
                    return iced::widget::operation::scroll_to(scroll_id, iced::widget::operation::AbsoluteOffset { x: 0.0, y: (item_top - margin).max(0.0) });
                } else if item_bottom > view_max - margin {
                    let offset = (item_bottom - viewport.bounds().height + margin).max(0.0);
                    return iced::widget::operation::scroll_to(scroll_id, iced::widget::operation::AbsoluteOffset { x: 0.0, y: offset });
                }
            }
        }
        Task::none()
    }

    /// Obtiene el nombre del artista que se debe contraer/expandir según la dirección de la tecla flecha.
    pub fn get_artist_to_toggle(&self, dir: LibraryNavDir) -> Option<String> {
        let artist_name = if let Some(h) = &self.selected_header {
            Some(h.clone())
        } else if let Some(song_idx) = self.selected_song_idx {
            self.filtered_songs.as_ref()
                .and_then(|songs| songs.get(song_idx))
                .and_then(|s| s.artist.clone().or(s.album_artist.clone()))
        } else {
            None
        };

        if let Some(artist) = artist_name {
            let is_collapsed = self.collapsed_artists.contains(&artist);
            if (dir == LibraryNavDir::Left && !is_collapsed) || (dir == LibraryNavDir::Right && is_collapsed) {
                return Some(artist);
            }
        }
        None
    }

    /// Actualiza la memoria de la última canción seleccionada para un artista.
    pub fn update_selection_memory(&mut self) {
        if let Some(song_idx) = self.selected_song_idx {
            if let Some(songs) = &self.filtered_songs {
                if let Some(song) = songs.get(song_idx) {
                    let artist = song.artist.clone().or(song.album_artist.clone()).unwrap_or_else(|| "Artista Desconocido".to_string());
                    self.artist_last_selection.insert(artist, song_idx);
                }
            }
        }
    }

    pub fn select_song(&mut self, idx: Option<usize>) {
        self.selected_song_idx = idx;
    }

    pub fn apply_filter(&mut self) {
        self.artist_last_selection.clear();
        if let Some(songs) = &self.cached_all_songs {
            let filtered: Vec<_> = if self.search_query.is_empty() {
                songs.clone()
            } else {
                let query = self.search_query.to_lowercase();
                songs.iter()
                    .filter(|s| {
                        s.title.as_ref().map(|t| t.to_lowercase().contains(&query)).unwrap_or(false)
                        || s.artist.as_ref().map(|t| t.to_lowercase().contains(&query)).unwrap_or(false)
                        || s.album.as_ref().map(|t| t.to_lowercase().contains(&query)).unwrap_or(false)
                    })
                    .cloned()
                    .collect()
            };
            self.filtered_songs = Some(filtered.clone());
            
            // Si antes no había selección y ahora sí hay canciones, no autoseleccionar si queremos navegar por artistas
            // o dejarlo como está. Por ahora mantenemos la lógica de selección de app.rs.
            
            // Agrupar por artista
            let mut groups_map: std::collections::BTreeMap<String, ArtistGroup> = std::collections::BTreeMap::new();
            
            for song in filtered {
                let artist_name = song.artist.clone()
                    .or_else(|| song.album_artist.clone())
                    .unwrap_or_else(|| "Artista Desconocido".to_string());
                
                let group = groups_map.entry(artist_name.clone()).or_insert(ArtistGroup {
                    name: artist_name,
                    albums: std::collections::HashSet::new(),
                    songs: Vec::new(),
                    duration_secs: 0.0,
                });
                
                if let Some(album) = &song.album {
                    group.albums.insert(album.clone());
                }
                group.duration_secs += song.duration_secs.unwrap_or(0.0);
                group.songs.push(song);
            }
            
            self.artist_groups = groups_map.into_values().collect();

            // Reconstruir filtered_songs a partir de los grupos para asegurar que los índices 
            // globales coincidan EXACTAMENTE con el orden visual de la lista agrupada.
            let mut flattened = Vec::new();
            for group in &self.artist_groups {
                for song in &group.songs {
                    flattened.push(song.clone());
                }
            }
            self.filtered_songs = Some(flattened);
        } else {
            self.filtered_songs = None;
            self.artist_groups = Vec::new();
        }
    }
}

pub fn view<'a>(
    manager: &'a LibraryManager,
    _database: &'a Arc<Mutex<Database>>,
) -> Element<'a, Message> {
    
    // Función auxiliar para iconos sin fondo (top y bottom bar) interactivos
    let icon_btn_size = |icon: &str, action: Message, size: f32| -> Element<'a, Message> {
        let content = iced::widget::svg(iced::widget::svg::Handle::from_path(format!("assets/icons/{}", icon)))
            .width(Length::Fixed(size))
            .height(Length::Fixed(size));
        iced::widget::mouse_area(content)
            .on_press(action)
            .interaction(iced::mouse::Interaction::Pointer)
            .into()
    };
    

    let _icon_btn = |icon: &str, action: Message| -> Element<'a, Message> {
        icon_btn_size(icon, action, 20.0)
    };
    
    // Controles de ventana
    let win_action_btn = |icon: &str, action: Message, size: f32| -> Element<'a, Message> {
        let content = iced::widget::svg(iced::widget::svg::Handle::from_path(format!("assets/icons/{}", icon)))
            .width(Length::Fixed(size))
            .height(Length::Fixed(size));
        
        iced::widget::mouse_area(content)
            .on_press(action)
            .interaction(iced::mouse::Interaction::Pointer)
            .into()
    };

    // --- BARRA SUPERIOR (40px) ---
    let create_source_tab = |label: &'a str, source: LibrarySource, current: LibrarySource| -> Element<'a, Message> {
        let is_active = source == current;
        let text_el = text(label)
            .size(15)
            .font(FONT_INTER_SANS_MEDIUM)
            .color(if is_active { Color::WHITE } else { COLOR_TEXT_SECONDARY });
        
        let source_color = match source {
            LibrarySource::Spotify => Color::from_rgb8(29, 185, 84),
            LibrarySource::YouTube => Color::from_rgb8(221, 36, 37),
            LibrarySource::Qobuz => Color::from_rgb8(48, 48, 48),
            LibrarySource::Deezer => Color::from_rgb8(162, 56, 255),
            LibrarySource::Tidal => Color::from_rgb8(68, 68, 68),
            LibrarySource::Local => COLOR_ACCENT,
        };

        let btn_container = container(text_el)
            .padding([2, 10]) // 3px arriba/abajo, 10px lados
            .style(move |_t: &Theme| {
                if is_active {
                    container::Style::default()
                        .background(source_color)
                        .border(iced::Border {
                            radius: 14.0.into(),
                            ..Default::default()
                        })
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
    ].spacing(10).align_y(Alignment::Center).height(Length::Fill);

    let top_bar = row![
        source_tabs,
        Space::new().width(Length::Fill),
        row![
            container(win_action_btn("minimize.svg", Message::PlayerWindowAction(crate::gui::player::WindowAction::Minimize), 30.0))
                .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 8.0, left: 0.0 }),
            win_action_btn("maximize.svg", Message::PlayerWindowAction(crate::gui::player::WindowAction::Maximize), 30.0),
            win_action_btn("close.svg", Message::PlayerWindowAction(crate::gui::player::WindowAction::Close), 34.0),
        ].spacing(5).align_y(Alignment::Center)
    ]
    .align_y(Alignment::Center)
    .height(Length::Fill)
    .padding(iced::Padding { top: 0.0, right: 5.0, bottom: 0.0, left: 15.0 });

    let top_container = container(top_bar)
        .width(Length::Fill)
        .height(Length::Fixed(40.0))
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    // --- BARRA DE ORDENAMIENTO (30px) ---
    let sort_container = crate::gui::widgets::build_sort_bar(
        &[
            SortColumn::TrackNumber, SortColumn::Title, SortColumn::Artist, SortColumn::AlbumArtist,
            SortColumn::Album, SortColumn::Genre, SortColumn::Year, SortColumn::Duration,
            SortColumn::Format, SortColumn::SampleRate, SortColumn::Channels, SortColumn::BitDepth,
            SortColumn::Bitrate, SortColumn::Size
        ],
        manager.sort_column,
        manager.sort_ascending,
        &manager.column_widths,
        manager.resizing_column,
        manager.hovered_column,
        |col| Message::ColumnHover(col),
        |col| Message::StartColumnResize(col),
        |col| Message::LibrarySortChanged(col),
    );

    // --- CONTENIDO GRID / LISTA ---
    let content: Element<'a, Message> = match manager.view_mode {
        LibraryViewMode::Grid => {
            let is_empty = manager.cached_albums.as_ref().map(|v| v.is_empty()).unwrap_or(true);

            if is_empty {
                container(text("La biblioteca está vacía o cargando...").color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into()
            } else {
                let res_grid = iced::widget::responsive(|size| {
                    let db_albums = manager.cached_albums.as_ref().unwrap();
                    let mut grid_col = column![].spacing(0);
                    
                    // Cálculo de columnas
                    let card_w = 180.0;
                    let mut columns_count = (size.width / card_w).floor() as usize;
                    if columns_count < 2 { columns_count = 2; }
                    if columns_count > 7 { columns_count = 7; }
                    // Actualiza el valor real de columnas para que LibraryKeyNav lo use
                    manager.albums_per_row.set(columns_count);

                    for row_chunk in db_albums.chunks(columns_count) {
                        let mut current_row = row![].spacing(5);
                        let mut active_expansion: Option<String> = None;

                        for (album_id, album, artist, genre, year, cover_path) in row_chunk {
                            let is_expanded = manager.expanded_album.as_deref() == Some(album_id.as_str());
                            if is_expanded {
                                active_expansion = Some(album_id.clone());
                            }

                            let album_art: Element<'a, Message> = if let Some(path) = cover_path {
                                container(
                                    image(iced::widget::image::Handle::from_path(path.clone()))
                                        .width(Length::Fixed(158.0))
                                        .height(Length::Fixed(158.0))
                                        .content_fit(iced::ContentFit::Cover)
                                        .border_radius(8.0)
                                )
                                .width(Length::Fixed(158.0))
                                .height(Length::Fixed(158.0))
                                .style(|_t| container::Style::default().border(iced::Border { radius: 8.0.into(), ..Default::default() }))
                                .clip(true)
                                .into()
                            } else {
                                let icon = iced::widget::svg(iced::widget::svg::Handle::from_path("assets/icons/album.svg"))
                                    .width(Length::Fixed(96.0))
                                    .height(Length::Fixed(96.0))
                                    .style(|_t: &Theme, _s| iced::widget::svg::Style { color: Some(COLOR_TEXT_SECONDARY) });
                                let title_text = text("AuDoxiDY").font(crate::gui::theme::FONT_STAGE_WANDER).size(11).color(COLOR_TEXT_SECONDARY);
                                
                                container(column![icon, title_text].align_x(Alignment::Center).spacing(5))
                                    .width(Length::Fixed(158.0))
                                    .height(Length::Fixed(158.0))
                                    .center_x(Length::Fill)
                                    .center_y(Length::Fill)
                                    .style(|_t: &Theme| container::Style::default().background(Color::from(COLOR_BG)).border(iced::Border { radius: 8.0.into(), ..Default::default() }))
                                    .clip(true)
                                    .into()
                            };
                            
                            let info_col = column![
                                text(truncate_text(artist, 20)).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM).line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(14.0))),
                                text(truncate_text(album, 19)).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM).line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(14.0))),
                                text(truncate_text(genre, 20)).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM).line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(14.0))),
                                text(year.as_str()).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM).line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(14.0))),
                            ].spacing(2).width(Length::Fill);

                            let chevron_svg = if is_expanded { "arrow-up-chevron.svg" } else { "arrow-down-chevron.svg" };
                            let chevron_btn = button(
                                iced::widget::svg(iced::widget::svg::Handle::from_path(format!("assets/icons/{}", chevron_svg)))
                                    .width(30)
                                    .height(30)
                                    .style(move |_t: &Theme, _s: iced::widget::svg::Status| iced::widget::svg::Style { color: Some(COLOR_TEXT_PRIMARY) })
                            )
                                .padding(0)
                                .on_press(Message::ToggleAlbumExpansion(album_id.clone()))
                                .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT));

                            let card_bottom = row![
                                info_col,
                                chevron_btn
                            ].align_y(Alignment::Center).width(Length::Fill);

                            let is_selected = manager.selected_album.as_deref() == Some(album_id.as_str());
                            
                            let item_col = column![
                                album_art,
                                card_bottom
                            ].spacing(5);

                            let card_wrapper = iced::widget::mouse_area(item_col)
                                .on_press(Message::SelectAlbum(album_id.clone()))
                                .interaction(iced::mouse::Interaction::Pointer);
                            
                            let card_container = container(card_wrapper)
                                .width(Length::Fixed(188.0))
                                .padding(iced::Padding { top: 18.0, bottom: 15.0, left: 15.0, right: 15.0 })
                                .style(move |_t: &Theme| {
                                    if is_expanded {
                                        container::Style::default().background(COLOR_CONTRAST).border(iced::Border { radius: 10.0.into(), ..Default::default() })
                                    } else if is_selected {
                                        container::Style::default().background(COLOR_CONTRAST).border(iced::Border { radius: 10.0.into(), ..Default::default() })
                                    } else {
                                        container::Style::default()
                                    }
                                });
                                
                            current_row = current_row.push(card_container);
                        }

                        grid_col = grid_col.push(current_row);

                        // --- Expansión Inline debajo de la fila ---
                        if let Some(_exp_album) = active_expansion {
                            let mut album_songs_col = column![].spacing(0).padding([25, 0]);
                            
                            if let Some(songs) = manager.expanded_album_songs.as_ref() {
                                if !songs.is_empty() {
                                    for (song_i, song) in songs.iter().enumerate() {
                                        let s_clone = song.clone();
                                        let is_song_selected = manager.selected_song_idx == Some(song_i);
                                        
                                        let txt_color = if is_song_selected { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };
                                        
                                        let get_col = |col: SortColumn| -> Element<'a, Message> {
                                            let w = *manager.column_widths.get(&col).unwrap_or(&100) as f32;
                                            let max_chars = ((w - 10.0) / 7.0).max(1.0) as usize;
                                            
                                            // Metadato Puro abstraído de DB
                                            let val = format_metadata(song, &col);
                                            let truncated = truncate_text(&val, max_chars);
                                            
                                            container(text(truncated).size(13).color(Color::from(txt_color)).font(FONT_INTER_SANS_MEDIUM))
                                                .width(Length::Fixed(w))
                                                .height(Length::Fixed(15.0))
                                                .center_y(Length::Fill)
                                                .padding(iced::Padding { left: 5.0, right: 5.0, top: 0.0, bottom: 0.0 })
                                                .clip(true)
                                                .into()
                                        };

                                        let song_row_inner = row![
                                            get_col(SortColumn::TrackNumber),
                                            get_col(SortColumn::Title),
                                            get_col(SortColumn::Artist),
                                            get_col(SortColumn::AlbumArtist),
                                            get_col(SortColumn::Album),
                                            get_col(SortColumn::Genre),
                                            get_col(SortColumn::Year),
                                            get_col(SortColumn::Duration),
                                            get_col(SortColumn::Format),
                                            get_col(SortColumn::SampleRate),
                                            get_col(SortColumn::Channels),
                                            get_col(SortColumn::BitDepth),
                                            get_col(SortColumn::Bitrate),
                                            get_col(SortColumn::Size),
                                            button(text("►").size(11).color(Color::from(txt_color))).on_press(Message::AddSongToPlaylist(s_clone)).style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT)),
                                        ].align_y(Alignment::Center).padding([0, 15]).height(Length::Fixed(15.0));
                                        
                                        let song_row = iced::widget::mouse_area(
                                            container(song_row_inner)
                                                .width(Length::Fill)
                                                .height(Length::Fixed(32.0))
                                                .align_y(Alignment::Center)
                                                .style(move |_t: &Theme| {
                                                    if is_song_selected {
                                                        container::Style::default().background(Color::from(COLOR_CONTRAST))
                                                    } else {
                                                        container::Style::default()
                                                    }
                                                })
                                        ).on_press(Message::SelectSong(Some(song_i)))
                                         .interaction(iced::mouse::Interaction::Pointer);
                                        
                                        album_songs_col = album_songs_col.push(song_row);
                                    }
                                } else {
                                    album_songs_col = album_songs_col.push(text("No se encontraron canciones.").color(COLOR_TEXT_SECONDARY));
                                }
                            } else {
                                album_songs_col = album_songs_col.push(text("Cargando...").color(COLOR_TEXT_SECONDARY));
                            }

                            let exp_container = container(album_songs_col)
                                .width(Length::Fill)
                                .padding([5, 15])
                                .style(|_t: &Theme| container::Style::default().background(COLOR_BG));
                            grid_col = grid_col.push(exp_container);
                        }
                    }

                    // El closure ahora solo devuelve el grid (SIN scrollable)
                    grid_col.into()
                });
                
                // El scrollable envuelve el responsive directamente - así snap_to puede encontrarlo
                scrollable(res_grid)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .direction(iced::widget::scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::new()
                            .width(4)
                            .margin(0)
                            .scroller_width(4)
                    ))
                    .id(LIBRARY_SCROLL_ID.clone())
                    .on_scroll(Message::LibraryScroll)
                    .style(crate::gui::widgets::custom_scrollbar_style)
                    .into()
            }
        }
        LibraryViewMode::DetailedList => {
            container(text("Vista Detallada - En desarrollo").color(COLOR_TEXT_PRIMARY)).into()
        }
        LibraryViewMode::ThumbnailList => {
            container(text("Vista con Thumbnail - En desarrollo").color(COLOR_TEXT_PRIMARY)).into()
        }
        LibraryViewMode::SimpleList => {
            let groups = &manager.artist_groups;
            if groups.is_empty() {
                container(text("La biblioteca está vacía o cargando...").color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM))
                    .width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill).into()
            } else {
                let header_h = 32.0;
                let song_h = 32.0;
                
                // 1. Calcular alturas acumuladas para virtualización eficiente
                let view_min_raw = manager.last_viewport.as_ref().map(|v| v.absolute_offset().y).unwrap_or(0.0);
                let viewport_h = manager.last_viewport.as_ref().map(|v| v.bounds().height).unwrap_or(800.0);
                
                let mut total_content_h = 0.0;
                let mut artist_tops = Vec::new(); // (top, group, is_collapsed)

                for group in groups {
                    let is_collapsed = manager.collapsed_artists.contains(&group.name);
                    let group_h = header_h + if is_collapsed { 0.0 } else { group.songs.len() as f32 * song_h };
                    
                    artist_tops.push((total_content_h, group, is_collapsed));
                    total_content_h += group_h;
                }

                // Clampear view_min manualmente para evitar desajustes durante el colapso (que reduce total_h)
                let max_scroll = (total_content_h - viewport_h).max(0.0);
                let view_min = view_min_raw.min(max_scroll);
                let view_max = view_min + viewport_h;
                
                // Margen de seguridad para scroll suave
                let render_min = view_min - 200.0;
                let render_max = view_max + 200.0;

                let mut top_space = 0.0;
                let mut bottom_space = 0.0;
                let mut visible_elements = Vec::new();
                let mut global_song_idx = 0;
                let mut sticky_artist_info = None;

                for (h_start, group, is_collapsed) in artist_tops {
                    let group_songs = group.songs.len();
                    let group_h = header_h + if is_collapsed { 0.0 } else { group_songs as f32 * song_h };
                    let h_end = h_start + group_h;

                    // Lógica de Sticky Header: El artista actual es el último cuyo INICIO está por encima o igual a view_min
                    // Ajustamos con +31.0 para que el cambio ocurra en cuanto la siguiente cabecera toque la zona pegajosa
                    if h_start <= view_min + 31.0 {
                        sticky_artist_info = Some((group, is_collapsed));
                    }

                    // Virtualización
                    if h_end < render_min {
                        top_space += group_h;
                        global_song_idx += group_songs;
                    } else if h_start > render_max {
                        bottom_space += group_h;
                        global_song_idx += group_songs;
                    } else {
                        // El header es visible?
                        let header_end = h_start + header_h;
                        if header_end >= render_min && h_start <= render_max {
                            visible_elements.push(ArtistRow::Header(group, is_collapsed));
                        }
                        
                        // Canciones visibles?
                        if !is_collapsed {
                            let mut song_y = h_start + header_h;
                            for song in &group.songs {
                                let s_end = song_y + song_h;
                                if s_end >= render_min && song_y <= render_max {
                                    visible_elements.push(ArtistRow::Song(song, global_song_idx));
                                } else if s_end < render_min {
                                    top_space += song_h;
                                } else {
                                    bottom_space += song_h;
                                }
                                song_y += song_h;
                                global_song_idx += 1;
                            }
                        } else {
                            global_song_idx += group_songs;
                        }
                    }
                }

            let mut list_col = column![].spacing(0);
            if top_space > 0.0 {
                list_col = list_col.push(Space::new().height(Length::Fixed(top_space)));
            }

            for element in visible_elements {
                match element {
                    ArtistRow::Header(group, is_collapsed) => {
                        let is_header_selected = manager.selected_header.as_ref() == Some(&group.name);
                        
                        let header = artist_header_widget(
                            group.name.clone(),
                            is_collapsed,
                            is_header_selected,
                            group.albums.len(),
                            group.songs.len(),
                            group.duration_secs,
                            Message::SelectArtistHeader(group.name.clone()),
                            Message::ToggleArtistExpansion(group.name.clone()),
                        );
                        
                        list_col = list_col.push(header);
                    }
                    ArtistRow::Song(song, song_i) => {
                        let is_song_selected = manager.selected_song_idx == Some(song_i);
                        let s_clone = song.clone();

                        let song_row = library_song_row_widget(
                            &song,
                            song_i,
                            is_song_selected,
                            &manager.column_widths,
                            Message::SelectSong(Some(song_i)),
                            Message::AddSongToPlaylist(s_clone),
                        );

                        list_col = list_col.push(song_row);
                    }
                }
            }

            if bottom_space > 0.0 {
                list_col = list_col.push(Space::new().height(Length::Fixed(bottom_space)));
            }

            let main_scroll = scrollable(container(list_col).width(Length::Fill).padding([0, 15]))
                    .width(Length::Fill).height(Length::Fill)
                    .direction(iced::widget::scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::new().width(4).margin(0).scroller_width(4)
                    ))
                    .id(LIBRARY_SCROLL_ID.clone())
                    .on_scroll(Message::LibraryScroll)
                    .style(crate::gui::widgets::custom_scrollbar_style);

                let content: Element<Message> = if let Some((st_group, is_collapsed)) = sticky_artist_info {
                    let is_header_selected = manager.selected_header.as_ref() == Some(&st_group.name);
                    
                    let sticky_header = container(
                        artist_header_widget(
                            st_group.name.clone(),
                            is_collapsed,
                            is_header_selected,
                            st_group.albums.len(),
                            st_group.songs.len(),
                            st_group.duration_secs,
                            Message::SelectArtistHeader(st_group.name.clone()),
                            Message::ToggleArtistExpansion(st_group.name.clone()),
                        )
                    )
                    .width(Length::Fill)
                    .height(Length::Fixed(32.0)) // Altura fija de 32px para que no cubra toda la pantalla
                    .padding([0, 0]) 
                    .style(|_theme| container::Style {
                        background: Some(iced::Background::Color(COLOR_CONTRAST)), 
                        ..Default::default()
                    });

                    // Wrap sticky_header in a top-aligned container with right padding for the scrollbar
                    let sticky_overlay = container(sticky_header)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .padding(iced::Padding { top: 0.0, bottom: 0.0, left: 15.0, right: 15.0 }) // Alinear sticky con demas filas de artistas (15px)
                        .align_y(iced::alignment::Vertical::Top);

                    stack![
                        main_scroll,
                        sticky_overlay
                    ].into()
                } else {
                    main_scroll.into()
                };
                
                content
            }
        }
    };

    // --- BARRA INFERIOR (40px) ---
    // Buscar sin bordes
    let search_input = container(
        text_input("Buscar...", &manager.search_query)
            .on_input(Message::LibrarySearchQueryChanged)
            .font(FONT_INTER_SANS_MEDIUM)
            .width(Length::Fixed(200.0))
    ).padding([0, 0]).center_y(Length::Fill);

      // Estadísticas: DD:HH:MM:SS
    let (s_count, a_count, art_count, d_secs, s_bytes) = if let Some(sel) = &manager.selection_stats {
        (sel.songs, sel.albums, sel.artists, sel.duration_secs, sel.size_bytes)
    } else {
        (manager.total_songs as u64, manager.total_albums as u64, manager.total_artists as u64, manager.total_duration_secs, manager.total_size_bytes)
    };

    let time_str = format_duration(d_secs);
    let size_str = format_size(s_bytes as i64);

    let stats_text = format!("{} Canciones | {} Álbumes | {} Artistas | {} | {}", s_count, a_count, art_count, time_str, size_str);
    
    // Icono vista actual
    let view_icon_str = match manager.view_mode {
        LibraryViewMode::Grid => "view-grid-outlined.svg",
        LibraryViewMode::ThumbnailList => "view-list-thumbnail-outlined.svg",
        LibraryViewMode::DetailedList => "view-list-thumbnail-fill.svg",
        LibraryViewMode::SimpleList => "view-list.svg",
    };

    let bottom_actions = row![
        icon_btn_size("play-rounded.outlined.svg", Message::PlayLibrarySelection, 36.0),
        Space::new().width(5.0),
        icon_btn_size("more-small.svg", Message::OpenFolderPicker, 31.0),
        Space::new().width(5.0),
        icon_btn_size(view_icon_str, Message::ToggleLibraryViewDropdown, 30.0),
    ].align_y(Alignment::Center).height(Length::Fill);

    let info_text_el = text(stats_text).size(13).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM);

    let bottom_bar = row![
        search_input,
        Space::new().width(15.0),
        info_text_el,
        Space::new().width(Length::Fill),
        bottom_actions,
    ]
    .padding([0, 15])
    .height(Length::Fill)
    .align_y(Alignment::Center);

    let bottom_container = container(bottom_bar)
        .width(Length::Fill)
        .height(Length::Fixed(40.0))
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));


    // Todo junto con padding a los lados donde corresponda
    // Deselección: Envolvemos el contenido en un mouse_area que capture clics en vacío
    let content_with_deselection = iced::widget::mouse_area(
        container(content)
            .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 5.0 })
            .width(Length::Fill)
            .height(Length::Fill)
    ).on_press(Message::LibraryDeselect);

    container(
        column![
            top_container,
            sort_container,
            content_with_deselection,
            bottom_container
        ]
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
    .into()
}

enum ArtistRow<'a> {
    Header(&'a ArtistGroup, bool), // group, is_collapsed
    Song(&'a crate::db::database::SongData, usize), // song, global_idx
}
