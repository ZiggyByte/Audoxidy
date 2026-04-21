use iced::{
    widget::{button, column, container, row, scrollable, text, Space},
    Alignment, Color, Element, Length, Theme, Task,
};
use std::sync::{Arc, Mutex};
use crate::db::Database;
use crate::gui::app::Message;
use crate::gui::theme::*;
// Import deleted since song row is injected and artist header is in universal_song_list
use crate::utils::{format_duration, format_size, format_metadata, truncate_text, SortColumn};

/// ID estático para el scrollable de la biblioteca — garantiza que view y update usan EXACTAMENTE el mismo ID
pub static LIBRARY_SCROLL_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::unique());
pub static GRID_ID_A: std::sync::LazyLock<iced::widget::Id> = std::sync::LazyLock::new(|| iced::widget::Id::new("grid_view_a"));
pub static GRID_ID_B: std::sync::LazyLock<iced::widget::Id> = std::sync::LazyLock::new(|| iced::widget::Id::new("grid_view_b"));

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LibraryViewMode {
    Grid,
    DetailedList,
    ThumbnailList,
    SimpleList,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LibraryListItem {
    Artist(String),
    Album(String),
    Song(usize),
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
    pub albums_count: usize,
    pub songs_count: usize,
    pub songs: Vec<std::sync::Arc<crate::db::database::SongData>>,
    pub duration_secs: f64,
}

pub struct LibraryManager {
    pub view_mode: LibraryViewMode,
    pub source: LibrarySource,
    pub sort_column: Option<SortColumn>,
    pub sort_ascending: Option<bool>,
    pub search_query: String,
    pub search_nonce: u32,
    pub expanded_album: Option<String>,
    pub expanded_album_songs: Option<Vec<std::sync::Arc<crate::db::database::SongData>>>,
    pub cached_albums: Option<Vec<(String, String, String, String, String, Option<String>)>>,
    pub filtered_albums: Option<Vec<(String, String, String, String, String, Option<String>)>>,
    pub cached_all_songs: Option<Vec<std::sync::Arc<crate::db::database::SongData>>>,
    pub filtered_songs: Option<Vec<std::sync::Arc<crate::db::database::SongData>>>,
    pub cached_folders: Option<Vec<(i64, String, String)>>,
    pub mount_points: Vec<(String, String)>,
    pub filter_artist: Option<String>,
    pub filter_album: Option<String>,
    pub filter_genre: Option<String>,
    pub filter_year: Option<String>,
    pub filter_folder_id: Option<i64>,
    pub artist_groups: Vec<ArtistGroup>,
    pub collapsed_artists: std::collections::HashSet<String>,
    pub collapsed_albums: std::collections::HashSet<String>,
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
    pub filter_stats: Option<LibraryStats>, // Estadísticas calculadas de las canciones filtradas
    pub artist_last_selection: std::collections::HashMap<String, usize>, // artist -> global_song_idx
    /// Stores the exact (y, h) of the last navigated item to avoid name-based lookup jumping
    pub selected_item_hint: Option<(f32, f32)>,
}

impl Default for LibraryManager {
    fn default() -> Self {
        let mut column_widths = std::collections::HashMap::new();
        column_widths.insert(SortColumn::TrackNumber, 47);
        column_widths.insert(SortColumn::Title, 275);
        column_widths.insert(SortColumn::Artist, 155);
        column_widths.insert(SortColumn::AlbumArtist, 135);
        column_widths.insert(SortColumn::Album, 165);
        column_widths.insert(SortColumn::Genre, 135);
        column_widths.insert(SortColumn::Year, 50);
        column_widths.insert(SortColumn::Duration, 53);
        column_widths.insert(SortColumn::Format, 53);
        column_widths.insert(SortColumn::SampleRate, 80);
        column_widths.insert(SortColumn::Channels, 43);
        column_widths.insert(SortColumn::BitDepth, 60);
        column_widths.insert(SortColumn::Bitrate, 90);
        column_widths.insert(SortColumn::Size, 85);
        column_widths.insert(SortColumn::AlbumCard, 250);
        column_widths.insert(SortColumn::AlbumThumbnail, 40);

        Self {
            view_mode: LibraryViewMode::Grid,
            source: LibrarySource::Local,
            sort_column: None,
            sort_ascending: Some(true),
            search_query: String::new(),
            search_nonce: 0,
            expanded_album: None,
            expanded_album_songs: None,
            cached_albums: None,
            filtered_albums: None,
            cached_all_songs: None,
            filtered_songs: None,
            cached_folders: None,
            mount_points: Vec::new(),
            filter_artist: None,
            filter_album: None,
            filter_genre: None,
            filter_year: None,
            filter_folder_id: None,
            artist_groups: Vec::new(),
            collapsed_artists: std::collections::HashSet::new(),
            collapsed_albums: std::collections::HashSet::new(),
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
            selected_header: None,
            selected_album: None,
            selected_song_idx: None,
            albums_per_row: std::cell::Cell::new(5),
            library_area_width: 800.0,
            last_selected_album: None,
            last_selected_song_idx: None,
            last_viewport: None,
            selection_stats: None,
            filter_stats: None,
            artist_last_selection: std::collections::HashMap::new(),
            selected_item_hint: None,
        }
    }
}




impl LibraryManager {
    pub fn sort_songs(&self, songs: &mut [std::sync::Arc<crate::db::database::SongData>]) {
        if let Some(col_ref) = self.sort_column {
            let is_asc = self.sort_ascending.unwrap_or(true);
            songs.sort_by(|a, b| {
                let res = match col_ref {
                    SortColumn::TrackNumber => {
                        let tn_a = a.track_number.as_ref().and_then(|t| t.parse::<u32>().ok());
                        let tn_b = b.track_number.as_ref().and_then(|t| t.parse::<u32>().ok());
                        match (tn_a, tn_b) {
                            (Some(na), Some(nb)) => na.cmp(&nb),
                            _ => a.track_number.cmp(&b.track_number),
                        }
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
                        let tn_a = a.track_number.as_ref().and_then(|t| t.parse::<u32>().ok());
                        let tn_b = b.track_number.as_ref().and_then(|t| t.parse::<u32>().ok());
                        let alb_art_a = a.album_artist.as_ref().unwrap_or(a.artist.as_ref().unwrap_or(&String::from("Desconocido"))).clone();
                        let alb_art_b = b.album_artist.as_ref().unwrap_or(b.artist.as_ref().unwrap_or(&String::from("Desconocido"))).clone();
                        a.album.cmp(&b.album).then(alb_art_a.cmp(&alb_art_b)).then(match (tn_a, tn_b) {
                            (Some(na), Some(nb)) => na.cmp(&nb),
                            _ => a.track_number.cmp(&b.track_number),
                        })
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
                    SortColumn::AlbumCard => std::cmp::Ordering::Equal,
                    SortColumn::AlbumThumbnail => std::cmp::Ordering::Equal,
                };
                if is_asc { res } else { res.reverse() }
            });
        }
    }


    /// Maneja la navegación por teclado unificada entre cabeceras y canciones.
    /// Devuelve (NuevaCabecera, NuevoIndiceCanción, FocoEnCanción).
    pub fn is_list_mode(&self) -> bool {
        self.view_mode == LibraryViewMode::SimpleList ||
        self.view_mode == LibraryViewMode::DetailedList ||
        self.view_mode == LibraryViewMode::ThumbnailList
    }

    pub fn get_song_by_global_idx(&self, target_idx: usize) -> Option<std::sync::Arc<crate::db::database::SongData>> {
        let mut current_idx = 0;
        for group in &self.artist_groups {
            if current_idx + group.songs.len() > target_idx {
                return group.songs.get(target_idx - current_idx).cloned();
            }
            current_idx += group.songs.len();
        }
        None
    }

    pub fn get_visible_items(&self) -> Vec<(LibraryListItem, f32, f32)> {
        let mut items = Vec::new();
        let mut current_y = 0.0;
        // Header height must match the widget's artist_header_widget render height
        let header_h: f32 = match self.view_mode {
            LibraryViewMode::ThumbnailList => 42.0,
            _ => 32.0,
        };
        // Each view mode uses its own row height — must match the widget render height
        let row_height: f32 = match self.view_mode {
            LibraryViewMode::ThumbnailList => 42.0,
            _ => 32.0,
        };
        let mut global_song_idx = 0;

        for group in &self.artist_groups {
            items.push((LibraryListItem::Artist(group.name.clone()), current_y, header_h));
            current_y += header_h;

            let is_collapsed = self.collapsed_artists.contains(&group.name);
            
            // SimpleList and ThumbnailList: flat song rows per artist group (no album blocks)
            if self.view_mode == LibraryViewMode::SimpleList || self.view_mode == LibraryViewMode::ThumbnailList {
                if !is_collapsed {
                    for _ in 0..group.songs.len() {
                        items.push((LibraryListItem::Song(global_song_idx), current_y, row_height));
                        current_y += row_height;
                        global_song_idx += 1;
                    }
                } else {
                    global_song_idx += group.songs.len();
                }
            } else if self.is_list_mode() { // DetailedList or ThumbnailList
                let mut albums_info: Vec<(String, usize)> = Vec::new(); // name, count
                for song in &group.songs {
                    let alb_name = song.album.clone().unwrap_or_else(|| "Desconocido".to_string());
                    if let Some(last) = albums_info.last_mut() {
                        if last.0 == alb_name {
                            last.1 += 1;
                            continue;
                        }
                    }
                    albums_info.push((alb_name, 1));
                }

                if !is_collapsed {
                    for (alb_name, count) in albums_info {
                        let is_album_expanded = !self.collapsed_albums.contains(&alb_name);
                        
                        let album_header_h = 32.0;
                        let card_h: f32 = if self.view_mode == LibraryViewMode::DetailedList && is_album_expanded { 323.0 } else { 0.0 };
                        let right_h = if is_album_expanded {
                            album_header_h + count as f32 * row_height
                        } else {
                            album_header_h
                        };
                        let block_h = card_h.max(right_h) + if self.view_mode == LibraryViewMode::DetailedList { 10.0 } else { 0.0 };

                        items.push((LibraryListItem::Album(alb_name.clone()), current_y, album_header_h));
                        
                        if is_album_expanded {
                            let mut song_y = current_y + album_header_h;
                            for _ in 0..count {
                                items.push((LibraryListItem::Song(global_song_idx), song_y, row_height));
                                song_y += row_height;
                                global_song_idx += 1;
                            }
                        } else {
                            global_song_idx += count;
                        }
                        
                        current_y += block_h;
                    }
                } else {
                    global_song_idx += group.songs.len();
                }
            } else {
                global_song_idx += group.songs.len();
            }
        }
        items
    }

    /// Returns the new navigation item and its exact Y position from the items list.
    pub fn handle_key_nav(&self, dir: LibraryNavDir) -> Option<(LibraryListItem, f32, f32)> {
        // Permitir navegación linear en todos los modos, app.rs manejará la lógica 2D del Grid si es necesario

        let items = self.get_visible_items();
        if items.is_empty() { return None; }

        let mut current_idx = 0;
        let mut found = false;
        for (i, (item, _, _)) in items.iter().enumerate() {
            match item {
                LibraryListItem::Song(idx) => {
                    if Some(*idx) == self.selected_song_idx {
                        current_idx = i; found = true; break;
                    }
                }
                LibraryListItem::Album(name) => {
                    // Use hint to distinguish multiple occurrences of the same album name
                    if Some(name) == self.selected_album.as_ref() && self.selected_song_idx.is_none() {
                        // Verify position matches hint if available
                        if let Some((hint_y, _)) = self.selected_item_hint {
                            let (_, item_y, _) = &items[i];
                            // Permitir un margen de error pequeño (2.0px) tras cambios de estructura
                            if (item_y - hint_y).abs() < 2.0 {
                                current_idx = i; found = true; break;
                            }
                            // Keep searching for the right occurrence
                        } else {
                            current_idx = i; found = true; break;
                        }
                    }
                }
                LibraryListItem::Artist(name) => {
                    if Some(name) == self.selected_header.as_ref() && self.selected_song_idx.is_none() && self.selected_album.is_none() {
                        current_idx = i; found = true; break;
                    }
                }
            }
        }

        if !found { current_idx = 0; } else {
            match dir {
                LibraryNavDir::Up => { current_idx = current_idx.saturating_sub(1); }
                LibraryNavDir::Down => { current_idx = (current_idx + 1).min(items.len() - 1); }
                _ => {}
            }
        }

        let (item, y, h) = &items[current_idx];
        Some((item.clone(), *y, *h))
    }

    /// Calcula la tarea de scroll para asegurar que el elemento seleccionado sea visible.
    /// Si 'force_top' es true, el elemento se moverá directamente a la parte superior.
    pub fn get_scroll_task<Message: 'static>(&self, force_top: bool) -> Task<Message> {
        if !self.is_list_mode() {
            return Task::none();
        }

        let scroll_id = LIBRARY_SCROLL_ID.clone();
        let margin_top = match self.view_mode {
            LibraryViewMode::DetailedList => 32.0_f32,
            LibraryViewMode::ThumbnailList => 84.0_f32,
            LibraryViewMode::Grid => 0.0, // Grid usa navegación propia pero permitimos el cálculo
            _ => 64.0_f32,
        };

        // FAST PATH: use the exact hint coordinates stored during keyboard navigation.
        // This avoids any name-based search that would jump to the first occurrence
        // of an album name when the same album has multiple artist groups.
        if let Some((hint_y, hint_h)) = self.selected_item_hint {
            let is_header = self.selected_header.is_some() && self.selected_song_idx.is_none() && self.selected_album.is_none();
            let effective_margin_top = if is_header { 0.0 } else { margin_top };
            // SimpleList: 0px bottom margin so selector reaches the last visible row.
            // DetailedList: one row height (hint_h) so it doesn't clip behind the sort bar.
            let margin_bottom: f32 = if self.view_mode == LibraryViewMode::SimpleList { 0.0 } else { hint_h };

            if force_top {
                return iced::widget::operation::scroll_to(
                    scroll_id,
                    iced::widget::operation::AbsoluteOffset { x: 0.0, y: (hint_y - effective_margin_top).max(0.0) },
                );
            }

            if let Some(viewport) = &self.last_viewport {
                let view_min = viewport.absolute_offset().y;
                let view_max = view_min + viewport.bounds().height;
                let bottom = hint_y + hint_h;

                if hint_y < view_min + effective_margin_top {
                    return iced::widget::operation::scroll_to(
                        scroll_id,
                        iced::widget::operation::AbsoluteOffset { x: 0.0, y: (hint_y - effective_margin_top).max(0.0) },
                    );
                } else if bottom > view_max - margin_bottom {
                    let offset = (bottom - viewport.bounds().height + margin_bottom).max(0.0);
                    return iced::widget::operation::scroll_to(
                        scroll_id,
                        iced::widget::operation::AbsoluteOffset { x: 0.0, y: offset },
                    );
                }
            } else {
                let offset = (hint_y - effective_margin_top).max(0.0);
                return iced::widget::operation::scroll_to(
                    scroll_id,
                    iced::widget::operation::AbsoluteOffset { x: 0.0, y: offset },
                );
            }
            return Task::none();
        }

        // FALLBACK: name-based search (used when clicking with mouse, not keyboarding)
        // Priority: Song > Album > Artist
        let items = self.get_visible_items();
        for (item, y, h) in &items {
            let matches = match item {
                LibraryListItem::Song(idx) => Some(*idx) == self.selected_song_idx,
                LibraryListItem::Album(name) => {
                    self.selected_song_idx.is_none()
                        && Some(name) == self.selected_album.as_ref()
                }
                LibraryListItem::Artist(name) => {
                    self.selected_song_idx.is_none()
                        && self.selected_album.is_none()
                        && Some(name) == self.selected_header.as_ref()
                }
            };
            let is_header = matches!(item, LibraryListItem::Artist(_));

            if matches {
                let effective_margin_top = if is_header { 0.0 } else { margin_top };
                // Same logic: SimpleList gets 0px bottom margin, DetailedList gets one row.
                let margin_bottom: f32 = if self.view_mode == LibraryViewMode::SimpleList { 0.0 } else { *h };

                if let Some(viewport) = &self.last_viewport {
                    let view_min = viewport.absolute_offset().y;
                    let view_max = view_min + viewport.bounds().height;
                    let bottom = y + h;

                    if force_top {
                        return iced::widget::operation::scroll_to(
                            scroll_id,
                            iced::widget::operation::AbsoluteOffset { x: 0.0, y: (*y - effective_margin_top).max(0.0) },
                        );
                    }
                    if *y < view_min + effective_margin_top {
                        return iced::widget::operation::scroll_to(
                            scroll_id,
                            iced::widget::operation::AbsoluteOffset { x: 0.0, y: (*y - effective_margin_top).max(0.0) },
                        );
                    } else if bottom > view_max - margin_bottom {
                        let offset = (bottom - viewport.bounds().height + margin_bottom).max(0.0);
                        return iced::widget::operation::scroll_to(
                            scroll_id,
                            iced::widget::operation::AbsoluteOffset { x: 0.0, y: offset },
                        );
                    }
                } else {
                    let offset = if is_header { *y } else { (*y - effective_margin_top).max(0.0) };
                    return iced::widget::operation::scroll_to(
                        scroll_id,
                        iced::widget::operation::AbsoluteOffset { x: 0.0, y: offset },
                    );
                }
                break;
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
        self.search_nonce = self.search_nonce.wrapping_add(1);
        
        let query_lower = self.search_query.to_lowercase();
        let is_empty = query_lower.is_empty();
        let has_any_filter = !is_empty || self.filter_artist.is_some() || self.filter_album.is_some() || 
                           self.filter_genre.is_some() || self.filter_year.is_some() || self.filter_folder_id.is_some();

        // 1. Filtrado de Canciones y Grupos (Base de la Biblioteca)
        let mut albums_matching_songs = std::collections::HashSet::new();
        if let Some(songs) = &self.cached_all_songs {
            let mut filtered_songs: Vec<_> = songs.iter()
                .filter(|s| {
                    // A. Filtro de búsqueda rápida
                    let matches_search = is_empty || crate::utils::song_matches_search(s, &query_lower);
                    if !matches_search { return false; }

                    // B. Filtros específicos (Carpeta, Artista, Álbum, etc.)
                    if let Some(f_art) = &self.filter_artist {
                        if s.artist.as_deref() != Some(f_art) && s.album_artist.as_deref() != Some(f_art) { return false; }
                    }
                    if let Some(f_alb) = &self.filter_album {
                        if s.album.as_deref() != Some(f_alb) { return false; }
                    }
                    if let Some(f_gen) = &self.filter_genre {
                        if s.genre.as_deref() != Some(f_gen) { return false; }
                    }
                    if let Some(f_year) = &self.filter_year {
                        if s.release_year.as_deref() != Some(f_year) { return false; }
                    }
                    if let Some(f_id) = self.filter_folder_id {
                        if s.folder_id != f_id { return false; }
                    }

                    true
                })
                .cloned()
                .collect();
            
            // Coleccionar pares únicos Artista + Álbum para sincronizar el Grid
            for s in &filtered_songs {
                if let Some(alb) = &s.album {
                    let art = s.artist.as_deref().unwrap_or("Desconocido");
                    albums_matching_songs.insert((art.to_lowercase(), alb.trim().to_lowercase()));
                }
            }
            
            // Ordenamiento canónico: Artista -> Año -> Álbum -> Pista
            filtered_songs.sort_by(|a, b| crate::utils::compare_songs_for_listing(a, b));
            
            let mut groups_map: std::collections::BTreeMap<String, ArtistGroup> = std::collections::BTreeMap::new();
            for song in &filtered_songs {
                let artist_name = song.artist.clone()
                    .or_else(|| song.album_artist.clone())
                    .unwrap_or_else(|| "Artista Desconocido".to_string());
                
                let group = groups_map.entry(artist_name.clone()).or_insert(ArtistGroup {
                    name: artist_name,
                    albums_count: 0,
                    songs_count: 0,
                    songs: Vec::new(),
                    duration_secs: 0.0,
                });
                
                group.duration_secs += song.duration_secs.unwrap_or(0.0);
                group.songs.push(song.clone());
            }
            
            for g in groups_map.values_mut() {
                let mut unique_albums = std::collections::HashSet::new();
                for s in &g.songs {
                    if let Some(a) = &s.album { unique_albums.insert(a.clone()); }
                }
                g.albums_count = unique_albums.len();
                g.songs_count = g.songs.len();
            }
            
            let groups: Vec<ArtistGroup> = groups_map.into_values().collect();
            
            let mut flattened_filtered = Vec::with_capacity(filtered_songs.len());
            for group in &groups {
                for song in &group.songs {
                    flattened_filtered.push(song.clone());
                }
            }
            
            self.artist_groups = groups;
            self.filtered_songs = Some(flattened_filtered);
        } else {
            self.filtered_songs = None;
            self.artist_groups = Vec::new();
        }

        // 2. Filtrado de Álbumes (Grid) - Derivado de la base común
        if let Some(albums) = &self.cached_albums {
            if !has_any_filter {
                self.filtered_albums = None;
            } else {
                let mut grid_filtered: Vec<_> = albums.iter()
                    .filter(|a| {
                        let alb_name = a.1.trim().to_lowercase();
                        let art_name = a.2.trim().to_lowercase();
                        
                        // Un álbum es visible en el Grid si:
                        // 1. Contiene canciones que pasaron todos los filtros activos (Sincronización Total)
                        let has_matching_songs = albums_matching_songs.contains(&(art_name.clone(), alb_name.clone()));
                        
                        // 2. O si el texto de búsqueda coincide directamente con el artista o álbum (Búsqueda de Discografía)
                        let matches_search_directly = !is_empty && (alb_name.contains(&query_lower) || art_name.contains(&query_lower));
                        
                        has_matching_songs || matches_search_directly
                    })
                    .cloned()
                    .collect();
                
                // Ordenamiento canónico Grid
                grid_filtered.sort_by(|a, b| {
                    a.2.cmp(&b.2) // Artista
                        .then(a.4.cmp(&b.4)) // Año
                        .then(a.1.cmp(&b.1)) // Álbum
                });
                
                self.filtered_albums = Some(grid_filtered);
            }
        }

        // 3. Calcular estadísticas del filtro activo
        if has_any_filter {
            if let Some(songs) = &self.filtered_songs {
                let total_dur: f64 = songs.iter().map(|s| s.duration_secs.unwrap_or(0.0)).sum();
                let total_size: f64 = songs.iter().map(|s| s.size.unwrap_or(0) as f64).sum();
                let mut unique_albums: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
                let mut unique_artists = std::collections::HashSet::new();
                for s in songs {
                    let art = s.artist.as_deref().or(s.album_artist.as_deref()).unwrap_or("Desconocido");
                    let alb = s.album.as_deref().unwrap_or("Desconocido");
                    unique_albums.insert((art.to_string(), alb.to_string()));
                    unique_artists.insert(art.to_string());
                }
                self.filter_stats = Some(LibraryStats {
                    songs: songs.len() as u64,
                    albums: unique_albums.len() as u64,
                    artists: unique_artists.len() as u64,
                    duration_secs: total_dur,
                    size_bytes: total_size,
                });
            } else {
                self.filter_stats = None;
            }
        } else {
            self.filter_stats = None;
        }
    }
}

pub fn view<'a>(
    manager: &'a LibraryManager,
    _database: &'a Arc<Mutex<Database>>,
    playing_path: &'a str,
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
    let columns = if manager.view_mode == LibraryViewMode::DetailedList {
        vec![
            SortColumn::AlbumCard, SortColumn::TrackNumber, SortColumn::Title,
            SortColumn::AlbumArtist, SortColumn::Duration, SortColumn::Format, SortColumn::SampleRate,
            SortColumn::Channels, SortColumn::BitDepth, SortColumn::Bitrate,
            SortColumn::Size
        ]
    } else if manager.view_mode == LibraryViewMode::ThumbnailList {
        vec![
            SortColumn::AlbumThumbnail,
            SortColumn::TrackNumber, SortColumn::Title, SortColumn::Artist, SortColumn::AlbumArtist,
            SortColumn::Album, SortColumn::Genre, SortColumn::Year, SortColumn::Duration,
            SortColumn::Format, SortColumn::SampleRate, SortColumn::Channels, SortColumn::BitDepth,
            SortColumn::Bitrate, SortColumn::Size
        ]
    } else {
        vec![
            SortColumn::TrackNumber, SortColumn::Title, SortColumn::Artist, SortColumn::AlbumArtist,
            SortColumn::Album, SortColumn::Genre, SortColumn::Year, SortColumn::Duration,
            SortColumn::Format, SortColumn::SampleRate, SortColumn::Channels, SortColumn::BitDepth,
            SortColumn::Bitrate, SortColumn::Size
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
                static EMPTY: Vec<(String, String, String, String, String, Option<String>)> = Vec::new();
                &EMPTY
            };

            if albums_to_show.is_empty() {
                container(text(if manager.search_query.is_empty() { "La biblioteca está vacía o cargando..." } else { "No se encontraron álbumes." })
                    .color(COLOR_TEXT_SECONDARY)
                    .font(FONT_INTER_SANS_MEDIUM))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into()
            } else {
                let res_grid = iced::widget::responsive(move |size| {
                    let mut grid_col = column![].spacing(0);
                    
                    let card_w = 180.0;
                    let max_cols = if crate::utils::is_low_resource() { 5 } else { 10 };
                    let mut columns_count = (size.width / card_w).floor() as usize;
                    if columns_count < 2 { columns_count = 2; }
                    if columns_count > max_cols { columns_count = max_cols; }
                    manager.albums_per_row.set(columns_count);

                    // 1. Calcular alturas y visibilidad (Virtualización similar a las listas)
                    let mut total_height = 0.0;
                    let mut rows_data = Vec::new(); // (top_y, height, chunk_data, expanded_id)

                    for chunk in albums_to_show.chunks(columns_count) {
                        let mut row_h = 252.0;
                        let mut expanded_id = None;
                        
                        for (album_id, _, artist, _, _, _) in chunk {
                            let composite_id = format!("{}|{}", artist, album_id);
                            if manager.expanded_album.as_deref() == Some(composite_id.as_str()) {
                                expanded_id = Some(composite_id);
                                let songs_count = if let Some(songs) = manager.expanded_album_songs.as_ref() {
                                    let query = manager.search_query.trim();
                                    if query.is_empty() {
                                        songs.len()
                                    } else {
                                        songs.iter().filter(|s| crate::utils::song_matches_search(s, query)).count()
                                    }
                                } else {
                                    1 // Texto "Cargando..."
                                };
                                // 60px de padding total (50 del column + 10 del container) + filas de canciones
                                row_h += 60.0 + (songs_count.max(1) as f32 * 32.0);
                                break;
                            }
                        }
                        rows_data.push((total_height, row_h, chunk, expanded_id));
                        total_height += row_h;
                    }

                    let view_top = manager.last_viewport.as_ref().map(|v| v.absolute_offset().y).unwrap_or(0.0);
                    let view_h = manager.last_viewport.as_ref().map(|v| v.bounds().height).unwrap_or(800.0);
                    let lazy_margin = if crate::utils::is_low_resource() { 300.0 } else { 600.0 };
                    
                    let render_min = view_top - lazy_margin;
                    let render_max = view_top + view_h + lazy_margin;

                    let mut top_space = 0.0;
                    let mut bottom_space = 0.0;
                    let mut visible_rows = Vec::new();

                    for (h_start, h_row, chunk, exp_id) in rows_data {
                        let h_end = h_start + h_row;
                        if h_end < render_min {
                            top_space += h_row;
                        } else if h_start > render_max {
                            bottom_space += h_row;
                        } else {
                            visible_rows.push((chunk, exp_id));
                        }
                    }

                    // 2. Construir el Widget Tree
                    if top_space > 0.0 {
                        grid_col = grid_col.push(iced::widget::Space::new().height(Length::Fixed(top_space)));
                    }

                    for (row_chunk_data, active_expansion) in visible_rows {
                        let mut current_row = row![].spacing(5);
                        
                        for (album_id, album, artist, genre, year, cover_path) in row_chunk_data {
                            let composite_id = format!("{}|{}", artist, album_id);
                            let is_expanded = active_expansion.as_deref() == Some(composite_id.as_str());

                            let effective_cover = cover_path.as_ref();
                            let album_art = crate::gui::widgets::album_art_widget(
                                effective_cover,
                                None,
                                None,
                                crate::gui::widgets::PlaceholderStyle::Large,
                                Length::Fixed(152.0),
                                8.0,
                            );
                            
                            let info_col = column![
                                text(truncate_text(artist, 22)).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM)
                                    .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(14.0)))
                                    .wrapping(iced::widget::text::Wrapping::None),
                                text(truncate_text(album, 18)).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM)
                                    .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(14.0)))
                                    .wrapping(iced::widget::text::Wrapping::None),
                                text(truncate_text(genre, 18)).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM)
                                    .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(14.0)))
                                    .wrapping(iced::widget::text::Wrapping::None),
                                text(year.as_str()).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM)
                                    .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(14.0)))
                                    .wrapping(iced::widget::text::Wrapping::None),
                            ].spacing(2).width(Length::Fill);

                            let chevron_svg = if is_expanded { "arrow-up-chevron.svg" } else { "arrow-down-chevron.svg" };
                            let chevron_btn = button(
                                iced::widget::svg(iced::widget::svg::Handle::from_path(format!("assets/icons/{}", chevron_svg)))
                                    .width(30).height(30)
                                    .style(move |_t: &Theme, _s| iced::widget::svg::Style { color: Some(COLOR_TEXT_PRIMARY) })
                            )
                            .padding(0)
                            .on_press(Message::ToggleAlbumExpansion(composite_id.clone()))
                            .style(|_t, _s| button::Style::default().with_background(Color::TRANSPARENT));

                            let card_bottom = row![info_col, chevron_btn].align_y(Alignment::Center).width(Length::Fill);
                            let is_selected = manager.selected_album.as_deref() == Some(composite_id.as_str());
                            
                            let item_col = column![album_art, card_bottom].spacing(5);
                            let card_wrapper = iced::widget::mouse_area(item_col)
                                .on_press(Message::SelectAlbum(composite_id.clone()))
                                .interaction(iced::mouse::Interaction::Pointer);
                            
                            let card_container = container(card_wrapper)
                                .width(Length::Fixed(182.0))
                                .padding(iced::Padding { top: 18.0, bottom: 15.0, left: 15.0, right: 15.0 })
                                .style(move |_t: &Theme| {
                                    if is_expanded || is_selected {
                                        container::Style::default().background(COLOR_CONTRAST).border(iced::Border { radius: 10.0.into(), ..Default::default() })
                                    } else {
                                        container::Style::default()
                                    }
                                });
                                
                            current_row = current_row.push(card_container);
                        }
                        grid_col = grid_col.push(current_row);

                        if let Some(_exp_album) = active_expansion {
                            let mut album_songs_col = column![].spacing(0).padding([25, 0]);
                            if let Some(songs) = manager.expanded_album_songs.as_ref() {
                                let query = manager.search_query.trim();
                                let filtered_songs: Vec<_> = if query.is_empty() {
                                    songs.iter().enumerate().collect()
                                } else {
                                    songs.iter().enumerate()
                                        .filter(|(_, s)| crate::utils::song_matches_search(s, query))
                                        .collect()
                                };

                                if !filtered_songs.is_empty() {
                                    for (song_i, song) in filtered_songs {
                                        let s_clone = song.clone();
                                        let is_song_selected = manager.selected_song_idx == Some(song_i);
                                        let is_playing = song.full_file_path == playing_path;
                                        let txt_color = if is_song_selected { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };
                                        
                                        let get_col = |col: SortColumn| -> Element<'a, Message> {
                                            let w = *manager.column_widths.get(&col).unwrap_or(&100) as f32;
                                            let val = format_metadata(song, &col);
                                            let truncated = truncate_text(&val, ((w - 10.0) / 7.0).max(1.0) as usize);

                                            let content: Element<'a, Message> = if col == SortColumn::TrackNumber {
                                                row![
                                                    container(if is_playing { text("•").size(13).color(COLOR_ACCENT).font(FONT_INTER_SANS_MEDIUM).wrapping(iced::widget::text::Wrapping::None) } else { text("").size(13) })
                                                        .width(Length::Fixed(26.0)).align_x(iced::alignment::Horizontal::Center).align_y(iced::alignment::Vertical::Center),
                                                    container(text(truncated).size(13).color(Color::from(txt_color)).font(FONT_INTER_SANS_MEDIUM).wrapping(iced::widget::text::Wrapping::None))
                                                        .width(Length::Fixed(26.0)).align_x(iced::alignment::Horizontal::Right).align_y(iced::alignment::Vertical::Center)
                                                ].spacing(0).align_y(Alignment::Center).into()
                                            } else {
                                                text(truncated).size(13).color(Color::from(txt_color)).font(FONT_INTER_SANS_MEDIUM).wrapping(iced::widget::text::Wrapping::None).into()
                                            };

                                            let pad_left = if col == SortColumn::TrackNumber { 0.0 } else { 15.0 };
                                            container(content)
                                                .width(Length::Fixed(w)).height(Length::Fill).center_y(Length::Fill)
                                                .padding(iced::Padding { left: pad_left, right: 5.0, top: 0.0, bottom: 0.0 }).clip(true).into()
                                        };

                                        let song_row = iced::widget::mouse_area(
                                            container(row![
                                                get_col(SortColumn::TrackNumber), get_col(SortColumn::Title), get_col(SortColumn::Artist),
                                                get_col(SortColumn::AlbumArtist), get_col(SortColumn::Album), get_col(SortColumn::Genre),
                                                get_col(SortColumn::Year), get_col(SortColumn::Duration), get_col(SortColumn::Format),
                                                get_col(SortColumn::SampleRate), get_col(SortColumn::Channels), get_col(SortColumn::BitDepth),
                                                get_col(SortColumn::Bitrate), get_col(SortColumn::Size),
                                                button(text("►").size(11).color(Color::from(txt_color))).on_press(Message::AddSongToPlaylist(s_clone)).style(|_t, _s| button::Style::default().with_background(Color::TRANSPARENT)),
                                            ].align_y(Alignment::Center).padding([0, 0]).height(Length::Fill))
                                            .width(Length::Fill).height(Length::Fixed(32.0)).align_y(Alignment::Center)
                                            .style(move |_t| if is_song_selected { container::Style::default().background(Color::from(COLOR_CONTRAST)) } else { container::Style::default() })
                                        ).on_press(Message::SelectSong(Some(song_i))).interaction(iced::mouse::Interaction::Pointer);
                                        album_songs_col = album_songs_col.push(song_row);
                                    }
                                } else {
                                    album_songs_col = album_songs_col.push(text("No se encontraron canciones.").color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM));
                                }
                            } else {
                                album_songs_col = album_songs_col.push(text("Cargando...").color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM));
                            }
                            grid_col = grid_col.push(container(album_songs_col).width(Length::Fill).padding([5, 5]).style(|_t| container::Style::default().background(COLOR_BG)));
                        }
                    }

                    if bottom_space > 0.0 {
                        grid_col = grid_col.push(iced::widget::Space::new().height(Length::Fixed(bottom_space)));
                    }
                    grid_col.into()
                });
                
                // IMPORTANTE: NO usamos IDs dinámicos (A/B) basados en search_nonce a menos que sea estrictamente necesario.
                // Mantener el scroll_id constante permite que Iced mantenga el caché de carátulas y estado del scroll.
                let scroll_id = LIBRARY_SCROLL_ID.clone();
                let scrollable_grid = scrollable(res_grid)
                    .width(Length::Fill).height(Length::Fill).id(scroll_id).on_scroll(Message::LibraryScroll)
                    .style(crate::gui::widgets::custom_scrollbar_style)
                    .direction(iced::widget::scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::new()
                            .width(4.0)
                            .margin(4.0)
                            .scroller_width(4.0)
                    ));

                container(scrollable_grid).width(Length::Fill).height(Length::Fill).into()
            }
        },
        LibraryViewMode::SimpleList => {
            let cols = columns.clone();
            crate::gui::widgets::universal_song_list(
                manager,
                move |song, song_idx, is_selected, playing_path| {
                    crate::gui::widgets::library_song_row_widget(
                        song, song_idx, is_selected, &cols, &manager.column_widths,
                        Message::SelectSong(Some(song_idx)), Message::AddSongToPlaylist(song.clone()),
                        playing_path,
                    )
                },
                32.0,
                playing_path,
            )
        },
        LibraryViewMode::DetailedList => {
            let cols = columns;
            crate::gui::widgets::detailed_song_list(
                manager,
                move |song, song_idx, is_selected, playing_path| {
                    crate::gui::widgets::library_song_row_widget(
                        song, song_idx, is_selected, &cols, &manager.column_widths,
                        Message::SelectSong(Some(song_idx)), Message::AddSongToPlaylist(song.clone()),
                        playing_path,
                    )
                },
                32.0,
                playing_path,
            )
        },
        LibraryViewMode::ThumbnailList => {
            let cols = columns.clone();
            crate::gui::widgets::universal_song_list(
                manager,
                move |song, song_idx, is_selected, playing_path| {
                    crate::gui::widgets::thumbnail_song_row_widget(
                        song, song_idx, is_selected, &cols, &manager.column_widths,
                        Message::SelectSong(Some(song_idx)), Message::AddSongToPlaylist(song.clone()),
                        playing_path,
                    )
                },
                42.0,
                playing_path,
            )
        }
    };

    // --- BARRA INFERIOR (40px) ---
    let search_input = crate::gui::widgets::standard_search_input(
        "Buscar...",
        &manager.search_query,
        Message::LibrarySearchQueryChanged,
        Message::LibrarySearchQueryChanged(String::new()),
        Length::Fixed(180.0),
    );

    let (s_count, a_count, art_count, d_secs, s_bytes) = if let Some(sel) = &manager.selection_stats {
        (sel.songs, sel.albums, sel.artists, sel.duration_secs, sel.size_bytes)
    } else if let Some(flt) = &manager.filter_stats {
        (flt.songs, flt.albums, flt.artists, flt.duration_secs, flt.size_bytes)
    } else {
        (manager.total_songs as u64, manager.total_albums as u64, manager.total_artists as u64, manager.total_duration_secs, manager.total_size_bytes)
    };

    let stats_text = if manager.view_mode == LibraryViewMode::Grid {
        format!("{} Canciones | {} Álbumes | {} Artistas | {} | {}", s_count, a_count, art_count, format_duration(d_secs), format_size(s_bytes as i64))
    } else {
        format!("{} Canciones | {} Álbumes | {} Artistas | {} | {}", s_count, a_count, art_count, format_duration(d_secs), format_size(s_bytes as i64))
    };
    
    let view_icon_str = match manager.view_mode {
        LibraryViewMode::Grid => "view-grid-outlined.svg",
        LibraryViewMode::ThumbnailList => "view-list-thumbnail-outlined.svg",
        LibraryViewMode::DetailedList => "view-list-thumbnail-fill.svg",
        LibraryViewMode::SimpleList => "view-list.svg",
    };

    let bottom_actions = row![
        icon_btn_size("play-straight-outlined.svg", Message::PlayLibrarySelection, 36.0),
        Space::new().width(5.0),
        icon_btn_size("more-small.svg", Message::OpenFolderPicker, 31.0),
        Space::new().width(5.0),
        icon_btn_size(view_icon_str, Message::ToggleLibraryViewDropdown, 30.0),
    ].align_y(Alignment::Center).height(Length::Fill);

    let bottom_bar = row![search_input, Space::new().width(15.0), text(stats_text).size(13).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM), Space::new().width(Length::Fill), bottom_actions]
        .padding([0, 15]).height(Length::Fill).align_y(Alignment::Center);

    let content_with_deselection = iced::widget::mouse_area(
        container(content).padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 5.0 }).width(Length::Fill).height(Length::Fill)
    ).on_press(Message::LibraryDeselect);

    container(column![top_container, sort_container, content_with_deselection, container(bottom_bar).width(Length::Fill).height(Length::Fixed(40.0)).style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST))])
        .width(Length::Fill).height(Length::Fill).style(|_t: &Theme| container::Style::default().background(COLOR_BG)).into()
}

