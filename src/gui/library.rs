use iced::{
    widget::{button, column, container, row, text, Space},
    Alignment, Color, Element, Length, Theme, Task,
};
use std::sync::{Arc, Mutex};
use crate::db::Database;
use crate::gui::app::Message;
use crate::gui::theme::*;
// Import deleted since song row is injected and artist header is in universal_song_list
use crate::gui::widgets::{standard_scrollable, standard_scrollbar};
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LibraryListItem {
    Artist(String),
    Album(String),
    Song(i64),
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

#[derive(Debug, Clone)]
pub struct AlbumEntry {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub genre: String,
    pub year: String,
    pub cover_path: Option<String>,
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
    pub cached_albums: Option<Vec<AlbumEntry>>,
    pub filtered_albums: Option<Vec<AlbumEntry>>,
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
    pub selected_items: std::collections::HashSet<LibraryListItem>,
    pub focused_item: Option<LibraryListItem>,
    pub selection_pivot: Option<LibraryListItem>,
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
            selected_items: std::collections::HashSet::new(),
            focused_item: None,
            selection_pivot: None,
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
    pub fn sort_songs_static(songs: &mut [std::sync::Arc<crate::db::database::SongData>], sort_column: Option<SortColumn>, sort_ascending: Option<bool>) {
        if let Some(col_ref) = sort_column {
            let is_asc = sort_ascending.unwrap_or(true);
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
                        crate::utils::compare_strings_ignore_case(a.title.as_deref().unwrap_or(""), b.title.as_deref().unwrap_or("")).then(tn_a.cmp(&tn_b))
                    },
                    SortColumn::Artist | SortColumn::AlbumArtist => {
                        let tn_a = a.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let tn_b = b.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let art_a = crate::utils::get_effective_artist(a);
                        let art_b = crate::utils::get_effective_artist(b);
                        crate::utils::compare_strings_ignore_case(art_a, art_b)
                            .then(crate::utils::compare_strings_ignore_case(a.release_year.as_deref().unwrap_or(""), b.release_year.as_deref().unwrap_or("")))
                            .then(crate::utils::compare_strings_ignore_case(a.album.as_deref().unwrap_or(""), b.album.as_deref().unwrap_or("")))
                            .then(tn_a.cmp(&tn_b))
                    },
                    SortColumn::Album => {
                        let tn_a = a.track_number.as_ref().and_then(|t| t.parse::<u32>().ok());
                        let tn_b = b.track_number.as_ref().and_then(|t| t.parse::<u32>().ok());
                        let art_a = crate::utils::get_effective_artist(a);
                        let art_b = crate::utils::get_effective_artist(b);
                        crate::utils::compare_strings_ignore_case(a.album.as_deref().unwrap_or(""), b.album.as_deref().unwrap_or(""))
                            .then(crate::utils::compare_strings_ignore_case(art_a, art_b))
                            .then(match (tn_a, tn_b) {
                                (Some(na), Some(nb)) => na.cmp(&nb),
                                _ => a.track_number.cmp(&b.track_number),
                            })
                    },
                    SortColumn::Format => {
                        crate::utils::compare_strings_ignore_case(a.format.as_deref().unwrap_or(""), b.format.as_deref().unwrap_or(""))
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
                        let art_a = crate::utils::get_effective_artist(a);
                        let art_b = crate::utils::get_effective_artist(b);
                        crate::utils::compare_strings_ignore_case(a.genre.as_deref().unwrap_or(""), b.genre.as_deref().unwrap_or(""))
                            .then(crate::utils::compare_strings_ignore_case(art_a, art_b))
                            .then(crate::utils::compare_strings_ignore_case(a.album.as_deref().unwrap_or(""), b.album.as_deref().unwrap_or("")))
                            .then(tn_a.cmp(&tn_b))
                    },
                    SortColumn::Year => {
                        let tn_a = a.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let tn_b = b.track_number.as_ref().and_then(|t| t.parse::<u32>().ok()).unwrap_or(0);
                        let art_a = crate::utils::get_effective_artist(a);
                        let art_b = crate::utils::get_effective_artist(b);
                        crate::utils::compare_strings_ignore_case(a.release_year.as_deref().unwrap_or(""), b.release_year.as_deref().unwrap_or(""))
                            .then(crate::utils::compare_strings_ignore_case(art_a, art_b))
                            .then(crate::utils::compare_strings_ignore_case(a.album.as_deref().unwrap_or(""), b.album.as_deref().unwrap_or("")))
                            .then(tn_a.cmp(&tn_b))
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
        } else {
            songs.sort_by(|a, b| crate::utils::compare_songs_for_listing(a, b));
        }
    }

    pub fn sort_songs(&self, songs: &mut [std::sync::Arc<crate::db::database::SongData>]) {
        Self::sort_songs_static(songs, self.sort_column, self.sort_ascending);
    }

    pub fn sort_albums_static(albums: &mut [AlbumEntry], sort_column: Option<SortColumn>, sort_ascending: Option<bool>) {
        if let Some(col_ref) = sort_column {
            let is_asc = sort_ascending.unwrap_or(true);
            albums.sort_by(|a, b| {
                let res = match col_ref {
                    SortColumn::Album => crate::utils::compare_strings_ignore_case(&a.title, &b.title)
                        .then(crate::utils::compare_strings_ignore_case(&a.artist, &b.artist))
                        .then(crate::utils::compare_strings_ignore_case(&a.year, &b.year)),
                    SortColumn::Artist | SortColumn::AlbumArtist => crate::utils::compare_albums_for_listing(
                        &a.artist, &a.year, &a.title,
                        &b.artist, &b.year, &b.title,
                    ),
                    SortColumn::Genre => crate::utils::compare_strings_ignore_case(&a.genre, &b.genre)
                        .then(crate::utils::compare_strings_ignore_case(&a.artist, &b.artist))
                        .then(crate::utils::compare_strings_ignore_case(&a.year, &b.year))
                        .then(crate::utils::compare_strings_ignore_case(&a.title, &b.title)),
                    SortColumn::Year => crate::utils::compare_strings_ignore_case(&a.year, &b.year)
                        .then(crate::utils::compare_strings_ignore_case(&a.artist, &b.artist))
                        .then(crate::utils::compare_strings_ignore_case(&a.title, &b.title)),
                    _ => std::cmp::Ordering::Equal,
                };
                if is_asc { res } else { res.reverse() }
            });
        } else {
            // Usar el ordenamiento canónico global de Audoxidy por defecto
            albums.sort_by(|a, b| {
                crate::utils::compare_albums_for_listing(
                    &a.artist, &a.year, &a.title,
                    &b.artist, &b.year, &b.title,
                )
            });
        }
    }

    pub fn sort_albums(&self, albums: &mut [AlbumEntry]) {
        Self::sort_albums_static(albums, self.sort_column, self.sort_ascending);
    }

    /// Ordena todos los datos cargados en memoria basándose en el estado actual de ordenamiento.
    pub fn sort_all_data(&mut self) {
        let sc = self.sort_column;
        let sa = self.sort_ascending;
        
        if let Some(songs) = self.cached_all_songs.as_mut() {
            Self::sort_songs_static(songs, sc, sa);
        }
        if let Some(albums) = self.cached_albums.as_mut() {
            Self::sort_albums_static(albums, sc, sa);
        }
    }

    /// Actualiza los datos maestros de la biblioteca, re-ordenando y aplicando filtros.
    pub fn update_data(
        &mut self, 
        songs: Vec<std::sync::Arc<crate::db::database::SongData>>, 
        albums: Vec<AlbumEntry>
    ) {
        self.cached_all_songs = Some(songs);
        self.cached_albums = Some(albums);
        
        self.sort_all_data();
        self.apply_filter();
    }


    /// Maneja la navegación por teclado unificada entre cabeceras y canciones.
    /// Devuelve (NuevaCabecera, NuevoIndiceCanción, FocoEnCanción).
    pub fn get_row_height(&self) -> f32 {
        match self.view_mode {
            LibraryViewMode::ThumbnailList => 42.0,
            _ => 32.0,
        }
    }

    pub fn get_header_height(&self) -> f32 {
        match self.view_mode {
            LibraryViewMode::ThumbnailList => 42.0,
            _ => 32.0,
        }
    }

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
        let header_h: f32 = if self.view_mode == LibraryViewMode::ThumbnailList { 42.0 } else { 32.0 };
        let row_height: f32 = if self.view_mode == LibraryViewMode::ThumbnailList { 42.0 } else { 32.0 };

        if self.is_list_mode() {
            for group in &self.artist_groups {
                items.push((LibraryListItem::Artist(group.name.clone()), current_y, header_h));
                current_y += header_h;

                let is_collapsed = self.collapsed_artists.contains(&group.name);
                
                let mut albums_info: Vec<(String, String, Vec<i64>)> = Vec::new(); // name, hash, song_ids
                for song in &group.songs {
                    let alb_name = song.album.clone().unwrap_or_else(|| "Desconocido".to_string());
                    
                    if let Some(last) = albums_info.last_mut() {
                        if last.0 == alb_name {
                            last.2.push(song.id);
                            continue;
                        }
                    }

                    let alb_hash = if let Some(cache) = &self.cached_albums {
                        cache.iter().find(|a| a.title == alb_name && a.artist == group.name)
                            .map(|a| a.id.clone())
                            .unwrap_or_else(|| alb_name.clone())
                    } else {
                        alb_name.clone()
                    };
                    albums_info.push((alb_name, alb_hash, vec![song.id]));
                }

                if !is_collapsed {
                    for (alb_name, alb_hash, song_ids) in albums_info {
                        let composite_id = format!("{}|{}", group.name, alb_hash);
                        let is_album_expanded = !self.collapsed_albums.contains(&composite_id);
                        
                        let album_header_h = if self.view_mode == LibraryViewMode::ThumbnailList { 42.0 } else { 32.0 };
                        let card_h: f32 = if self.view_mode == LibraryViewMode::DetailedList && is_album_expanded { 323.0 } else { 0.0 };
                        let right_h = if is_album_expanded {
                            album_header_h + song_ids.len() as f32 * row_height
                        } else {
                            album_header_h
                        };
                        let block_h = card_h.max(right_h) + if self.view_mode == LibraryViewMode::DetailedList { 10.0 } else { 0.0 };

                        items.push((LibraryListItem::Album(composite_id.clone()), current_y, album_header_h));
                        
                        if is_album_expanded {
                            let mut song_y = current_y + album_header_h;
                            for song_id in song_ids {
                                items.push((LibraryListItem::Song(song_id), song_y, row_height));
                                song_y += row_height;
                            }
                        }
                        
                        current_y += block_h;
                    }
                }
            }
        } else {
            // Modo Grid: Los elementos son álbumes y opcionalmente canciones si está expandido
            let albums_to_show = self.filtered_albums.as_ref()
                .or(self.cached_albums.as_ref());
            
            if let Some(albums) = albums_to_show {
                let per_row = self.albums_per_row.get().max(1);
                for chunk in albums.chunks(per_row) {
                    let mut row_h = 252.0;
                    let mut expanded_id = None;
                    
                    for album_entry in chunk {
                        let composite_id = format!("{}|{}", album_entry.artist, album_entry.id);
                        items.push((LibraryListItem::Album(composite_id.clone()), current_y, 252.0)); // Placeholder para la posición de la carta

                        if self.expanded_album.as_deref() == Some(composite_id.as_str()) {
                            expanded_id = Some(composite_id);
                        }
                    }

                    if let Some(_exp_id) = expanded_id {
                    if let Some(songs) = self.expanded_album_songs.as_ref() {
                        let query = self.search_query.trim();
                        let filtered = if query.is_empty() {
                            songs.clone()
                        } else {
                            songs.iter().filter(|s| crate::utils::song_matches_search(s, query)).cloned().collect()
                        };

                        let mut song_y = current_y + 252.0 + 32.0; // aprox padding
                        for song in filtered {
                            items.push((LibraryListItem::Song(song.id), song_y, 32.0));
                            song_y += 32.0;
                        }
                        row_h += 60.0 + (songs.len() as f32 * 32.0); // Ajuste de altura de fila para navegación
                    }
                    }
                    current_y += row_h;
                }
            }
        }
        items
    }

    /// Returns the new navigation item and its exact Y position from the items list.
    pub fn handle_key_nav(&mut self, dir: LibraryNavDir, modifiers: iced::keyboard::Modifiers) -> Option<(LibraryListItem, f32, f32)> {
        let items = self.get_visible_items();
        if items.is_empty() { return None; }

        let mut current_idx = 0;
        let mut found = false;

        // 1. Intentar encontrar por focused_item
        if let Some(focused) = &self.focused_item {
            if let Some(pos) = items.iter().position(|(it, _, _)| it == focused) {
                current_idx = pos;
                found = true;
            }
        }

        // 2. Fallback a campos legados si no se encontró foco explícito
        if !found {
            for (i, (item, _, _)) in items.iter().enumerate() {
                let match_found = match item {
                    LibraryListItem::Song(id) => {
                         // Si tenemos un list mode con filtered_songs, buscamos el índice
                         if let Some(sel_idx) = self.selected_song_idx {
                             if let Some(songs) = self.filtered_songs.as_ref() {
                                 songs.get(sel_idx).map(|s| s.id == *id).unwrap_or(false)
                             } else if let Some(songs) = self.expanded_album_songs.as_ref() {
                                 songs.get(sel_idx).map(|s| s.id == *id).unwrap_or(false)
                             } else { false }
                         } else { false }
                    }
                    LibraryListItem::Album(id) => Some(id) == self.selected_album.as_ref(),
                    LibraryListItem::Artist(name) => Some(name) == self.selected_header.as_ref(),
                };
                if match_found {
                    current_idx = i;
                    found = true;
                    break;
                }
            }
        }

        if !found { current_idx = 0; }

        // Navegación en Grid vs Lista
        let next_idx = if self.view_mode == LibraryViewMode::Grid {
            let per_row = self.albums_per_row.get().max(1);
            let is_current_song = matches!(items.get(current_idx), Some((LibraryListItem::Song(_), _, _)));
            
            match dir {
                LibraryNavDir::Up => {
                    if is_current_song {
                        let prev = current_idx.saturating_sub(1);
                        if let Some((LibraryListItem::Song(_), _, _)) = items.get(prev) {
                            prev
                        } else {
                            // Al subir desde la primera canción, ir al álbum que está expandido
                            if let Some(exp_id) = &self.expanded_album {
                                items.iter().position(|(it, _, _)| {
                                    if let LibraryListItem::Album(id) = it { id == exp_id }
                                    else { false }
                                }).unwrap_or(prev)
                            } else {
                                prev
                            }
                        }
                    } else {
                        current_idx.saturating_sub(per_row)
                    }
                },
                LibraryNavDir::Down => {
                    if is_current_song {
                        let next = (current_idx + 1).min(items.len() - 1);
                        if let Some((LibraryListItem::Song(_), _, _)) = items.get(next) {
                            next
                        } else {
                            current_idx // Se DETIENE al final de la lista de canciones
                        }
                    } else {
                        (current_idx + per_row).min(items.len() - 1)
                    }
                },
                LibraryNavDir::Left => current_idx.saturating_sub(1),
                LibraryNavDir::Right => (current_idx + 1).min(items.len() - 1),
                _ => current_idx,
            }
        } else {
            match dir {
                LibraryNavDir::Up => current_idx.saturating_sub(1),
                LibraryNavDir::Down => (current_idx + 1).min(items.len() - 1),
                _ => current_idx,
            }
        };

        if items.is_empty() { return None; }
        let (new_item, y, h) = items[next_idx.min(items.len()-1)].clone();

        if modifiers.shift() {
            let pivot = self.selection_pivot.clone().unwrap_or_else(|| items[current_idx].0.clone());
            self.selection_pivot = Some(pivot.clone());
            self.select_range(&pivot, &new_item);
        } else {
            self.selected_items.clear();
            self.selected_items.insert(new_item.clone());
            self.selection_pivot = Some(new_item.clone());
        }

        self.focused_item = Some(new_item.clone());
        self.selected_item_hint = Some((y, h));

        // Sincronizar legacy fields para compatibilidad con el resto de la app
        self.sync_legacy_selection(&new_item, next_idx);
        self.update_selection_stats();

        Some((new_item, y, h))
    }

    pub fn select_range(&mut self, start_item: &LibraryListItem, end_item: &LibraryListItem) {
        let items = self.get_visible_items();
        let mut start_v = None;
        let mut end_v = None;

        for (i, (it, _, _)) in items.iter().enumerate() {
            if it == start_item { start_v = Some(i); }
            if it == end_item { end_v = Some(i); }
        }

        if let (Some(s), Some(e)) = (start_v, end_v) {
            let (min, max) = if s < e { (s, e) } else { (e, s) };
            self.selected_items.clear();
            for i in min..=max {
                let (item, _, _) = &items[i];
                self.selected_items.insert(item.clone());
                
                // Si seleccionamos una cabecera, expandimos la selección a sus hijos según el requisito del usuario
                match item {
                    LibraryListItem::Artist(name) => {
                        if let Some(group) = self.artist_groups.iter().find(|g| g.name == *name) {
                            for song in &group.songs {
                                self.selected_items.insert(LibraryListItem::Song(song.id));
                            }
                        }
                    }
                    LibraryListItem::Album(id) => {
                        // Intentar encontrar las canciones del álbum
                        if let Some(songs) = self.get_songs_for_album_id(id) {
                            for song in songs {
                                self.selected_items.insert(LibraryListItem::Song(song.id));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn get_songs_for_album_id(&self, composite_id: &str) -> Option<Vec<std::sync::Arc<crate::db::database::SongData>>> {
        // Separar artista y álbum si es un composite ID (Artist|AlbumID)
        let parts: Vec<&str> = composite_id.split('|').collect();
        if parts.len() == 2 {
            let artist = parts[0];
            let album_id = parts[1];
            
            if let Some(all_songs) = &self.cached_all_songs {
                // Primero intentar coincidencia exacta con el cache de álbumes para obtener el tìtulo real
                if let Some(alb_entry) = self.cached_albums.as_ref().and_then(|all| 
                    all.iter().find(|a| a.id == album_id && a.artist == artist)
                ) {
                    return Some(all_songs.iter()
                        .filter(|s| s.album.as_deref() == Some(&alb_entry.title) && crate::utils::get_effective_artist(s) == artist)
                        .cloned().collect());
                }
            }
        }
        None
    }

    fn sync_legacy_selection(&mut self, item: &LibraryListItem, _current_idx_in_visible: usize) {
        match item {
            LibraryListItem::Artist(name) => {
                self.selected_header = Some(name.clone());
                self.selected_album = None;
                self.selected_song_idx = None;
            }
            LibraryListItem::Album(id) => {
                self.selected_header = None;
                self.selected_album = Some(id.clone());
                self.selected_song_idx = None;
            }
            LibraryListItem::Song(id) => {
                self.selected_header = None;
                if self.view_mode != LibraryViewMode::Grid {
                    self.selected_album = None;
                } else {
                    self.selected_album = self.expanded_album.clone();
                }
                // Encontrar el índice local en la vista actual
                let songs_list = if self.view_mode == LibraryViewMode::Grid {
                    self.expanded_album_songs.as_ref()
                } else {
                    self.filtered_songs.as_ref()
                };

                if let Some(songs) = songs_list {
                    self.selected_song_idx = songs.iter().position(|s| s.id == *id);
                }
            }
        }
    }

    pub fn update_selection_stats(&mut self) {
        let selected_songs = self.get_selected_songs();
        if selected_songs.is_empty() {
            self.selection_stats = None;
            return;
        }

        let mut stats = LibraryStats::default();
        let mut unique_albums = std::collections::HashSet::new();
        let mut unique_artists = std::collections::HashSet::new();

        for song in &selected_songs {
            stats.songs += 1;
            stats.duration_secs += song.duration_secs.unwrap_or(0.0);
            stats.size_bytes += song.size.unwrap_or(0) as f64;
            
            let art = crate::utils::get_effective_artist(song);
            let alb = song.album.as_deref().unwrap_or("Desconocido");
            unique_artists.insert(art.to_string());
            unique_albums.insert((art.to_string(), alb.to_string()));
        }

        stats.albums = unique_albums.len() as u64;
        stats.artists = unique_artists.len() as u64;
        self.selection_stats = Some(stats);
    }

    pub fn get_selected_songs(&self) -> Vec<Arc<crate::db::database::SongData>> {
        if self.selected_items.is_empty() { return Vec::new(); }

        let mut song_ids = std::collections::HashSet::new();
        for item in &self.selected_items {
            match item {
                LibraryListItem::Song(id) => { song_ids.insert(*id); }
                LibraryListItem::Album(id) => {
                    if let Some(songs) = self.get_songs_for_album_id(id) {
                        for s in songs { song_ids.insert(s.id); }
                    }
                }
                LibraryListItem::Artist(name) => {
                    if let Some(group) = self.artist_groups.iter().find(|g| g.name == *name) {
                        for s in &group.songs { song_ids.insert(s.id); }
                    }
                }
            }
        }

        let mut results = Vec::new();
        if let Some(all_songs) = &self.cached_all_songs {
             for song in all_songs {
                 if song_ids.contains(&song.id) {
                     results.push(song.clone());
                 }
             }
        }
        results
    }

    /// Calcula la tarea de scroll para asegurar que el elemento seleccionado sea visible.
    /// Si 'force_top' es true, el elemento se moverá directamente a la parte superior.
    pub fn get_scroll_task<Message: 'static>(&self, force_top: bool) -> Task<Message> {
        let scroll_id = LIBRARY_SCROLL_ID.clone();
        let margin_top = match self.view_mode {
            LibraryViewMode::DetailedList => 32.0_f32,
            LibraryViewMode::ThumbnailList => 42.0_f32,
            LibraryViewMode::Grid => 70.0, // Barra superior + tabs
            _ => 32.0_f32,
        };

        // FAST PATH: use the exact hint coordinates stored during keyboard navigation.
        if let Some((hint_y, hint_h)) = self.selected_item_hint {
            let is_header = match self.focused_item {
                Some(LibraryListItem::Artist(_)) | Some(LibraryListItem::Album(_)) => true,
                _ => false,
            };
            let is_song = matches!(self.focused_item, Some(LibraryListItem::Song(_)));
            let current_margin_top = if self.view_mode == LibraryViewMode::Grid && is_song { 32.0 } else { margin_top };
            
            let effective_margin_top = if is_header { 0.0 } else { current_margin_top };
            let margin_bottom: f32 = 0.0; // Desactivamos el margen dinámico que causaba saltos/espacios vacíos

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

        // FALLBACK: name-based search (used when clicking with mouse)
        let items = self.get_visible_items();
        for (item, y, h) in &items {
            let is_match = if let Some(focused) = &self.focused_item {
                item == focused
            } else {
                match item {
                    LibraryListItem::Song(id) => {
                        if let Some(idx) = self.selected_song_idx {
                            let list = if self.view_mode == LibraryViewMode::Grid { self.expanded_album_songs.as_ref() } else { self.filtered_songs.as_ref() };
                            list.and_then(|l| l.get(idx)).map(|s| s.id == *id).unwrap_or(false)
                        } else { false }
                    }
                    LibraryListItem::Album(id) => Some(id) == self.selected_album.as_ref(),
                    LibraryListItem::Artist(name) => Some(name) == self.selected_header.as_ref(),
                }
            };

            if is_match {
                let is_header = matches!(item, LibraryListItem::Artist(_) | LibraryListItem::Album(_));
                let is_song = matches!(item, LibraryListItem::Song(_));
                let current_margin_top = if self.view_mode == LibraryViewMode::Grid && is_song { 32.0 } else { margin_top };
                
                let effective_margin_top = if is_header { 0.0 } else { current_margin_top };
                let margin_bottom: f32 = 0.0;

                if force_top {
                    return iced::widget::operation::scroll_to(
                        scroll_id,
                        iced::widget::operation::AbsoluteOffset { x: 0.0, y: (*y - effective_margin_top).max(0.0) }
                    );
                }

                if let Some(viewport) = &self.last_viewport {
                    let view_min = viewport.absolute_offset().y;
                    let view_max = view_min + viewport.bounds().height;
                    let bottom = *y + *h;

                    if *y < view_min + effective_margin_top {
                        return iced::widget::operation::scroll_to(scroll_id, iced::widget::operation::AbsoluteOffset { x: 0.0, y: (*y - effective_margin_top).max(0.0) });
                    } else if bottom > view_max - margin_bottom {
                        let offset = (bottom - viewport.bounds().height + margin_bottom).max(0.0);
                        return iced::widget::operation::scroll_to(scroll_id, iced::widget::operation::AbsoluteOffset { x: 0.0, y: offset });
                    }
                } else {
                    return iced::widget::operation::scroll_to(scroll_id, iced::widget::operation::AbsoluteOffset { x: 0.0, y: (*y - effective_margin_top).max(0.0) });
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

    pub fn get_album_to_toggle(&self, dir: LibraryNavDir) -> Option<String> {
        let album_id = if let Some(alb) = &self.selected_album {
            Some(alb.clone())
        } else if let Some(song_idx) = self.selected_song_idx {
            if let Some(songs) = &self.filtered_songs {
                if let Some(s) = songs.get(song_idx) {
                    let art = crate::utils::get_effective_artist(s).to_string();
                    let alb_name = s.album.clone().unwrap_or_else(|| "Desconocido".to_string());
                    
                    // Buscar hash para ID compuesto
                    let alb_hash = if let Some(cache) = &self.cached_albums {
                        cache.iter().find(|a| a.title == alb_name && a.artist == art)
                            .map(|a| a.id.clone())
                            .unwrap_or_else(|| alb_name.clone())
                    } else {
                        alb_name.clone()
                    };
                    Some(format!("{}|{}", art, alb_hash))
                } else { None }
            } else { None }
        } else { None };

        if let Some(id) = album_id {
            let is_collapsed = self.collapsed_albums.contains(&id);
            if (dir == LibraryNavDir::Left && !is_collapsed) || (dir == LibraryNavDir::Right && is_collapsed) {
                return Some(id);
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
                        let art_a = s.artist.as_deref().unwrap_or("");
                        let art_b = s.album_artist.as_deref().unwrap_or("");
                        if !art_a.eq_ignore_ascii_case(f_art) && !art_b.eq_ignore_ascii_case(f_art) { return false; }
                    }
                    if let Some(f_alb) = &self.filter_album {
                        let alb = s.album.as_deref().unwrap_or("");
                        if !alb.eq_ignore_ascii_case(f_alb) { return false; }
                    }
                    if let Some(f_gen) = &self.filter_genre {
                        let genre_val = s.genre.as_deref().unwrap_or("");
                        if !genre_val.eq_ignore_ascii_case(f_gen) { return false; }
                    }
                    if let Some(f_year) = &self.filter_year {
                        let year = s.release_year.as_deref().unwrap_or("");
                        if !year.eq_ignore_ascii_case(f_year) { return false; }
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
                    let art = crate::utils::get_effective_artist(s);
                    albums_matching_songs.insert((art.to_lowercase(), alb.trim().to_lowercase()));
                }
            }
            
            // Unificado: Usar el motor de ordenamiento centralizado
            self.sort_songs(&mut filtered_songs);
            
            let mut groups_map: std::collections::BTreeMap<String, ArtistGroup> = std::collections::BTreeMap::new();
            for song in &filtered_songs {
                let artist_name = crate::utils::get_effective_artist(song);
                let key = artist_name.to_lowercase();
                
                let group = groups_map.entry(key).or_insert(ArtistGroup {
                    name: artist_name.to_string(), // Preserve cases from first encountered song
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

        // 2. Filtrado de Álbumes (Grid) - Unificado: Derivado directamente de los grupos para sincronización total
        if has_any_filter {
            let mut grid_filtered = Vec::new();
            let mut seen_albums = std::collections::HashSet::new();
            
            if let Some(all_albums) = &self.cached_albums {
                for group in &self.artist_groups {
                    for song in &group.songs {
                        if let Some(alb_title) = &song.album {
                            // Usamos el nombre del grupo y el título del álbum como clave única
                            let key = (group.name.to_lowercase(), alb_title.trim().to_lowercase());
                            if !seen_albums.contains(&key) {
                                seen_albums.insert(key);
                                
                                // Buscar el AlbumEntry original para conservar el cover_path y el ID (hash)
                                // Intentamos coincidencia por título y artista del grupo
                                if let Some(entry) = all_albums.iter().find(|a| a.title == *alb_title && a.artist == group.name)
                                    .or_else(|| all_albums.iter().find(|a| a.title == *alb_title)) // Fallback por título
                                {
                                    let mut unified_entry = entry.clone();
                                    unified_entry.artist = group.name.clone(); // Garantizar que el artista del Grid sea idéntico al de la Lista
                                    grid_filtered.push(unified_entry);
                                }
                            }
                        }
                    }
                }
            }
            
            self.sort_albums(&mut grid_filtered);
            self.filtered_albums = Some(grid_filtered);
        } else {
            self.filtered_albums = None;
        }

        // 3. Calcular estadísticas del filtro activo
        if has_any_filter {
            if let Some(songs) = &self.filtered_songs {
                let total_dur: f64 = songs.iter().map(|s| s.duration_secs.unwrap_or(0.0)).sum();
                let total_size: f64 = songs.iter().map(|s| s.size.unwrap_or(0) as f64).sum();
                let mut unique_albums: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
                let mut unique_artists = std::collections::HashSet::new();
                for s in songs {
                    let art = crate::utils::get_effective_artist(s);
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
            SortColumn::AlbumCard, SortColumn::TrackNumber, SortColumn::Title, SortColumn::Artist,
            SortColumn::AlbumArtist, SortColumn::Duration, SortColumn::Format, SortColumn::SampleRate,
            SortColumn::BitDepth, SortColumn::Bitrate, SortColumn::Channels,
            SortColumn::Size
        ]
    } else if manager.view_mode == LibraryViewMode::ThumbnailList {
        vec![
            SortColumn::AlbumThumbnail,
            SortColumn::TrackNumber, SortColumn::Title, SortColumn::Artist, SortColumn::AlbumArtist,
            SortColumn::Album, SortColumn::Genre, SortColumn::Year, SortColumn::Duration,
            SortColumn::Format, SortColumn::SampleRate, SortColumn::BitDepth, SortColumn::Bitrate,
            SortColumn::Channels, SortColumn::Size
        ]
    } else {
        vec![
            SortColumn::TrackNumber, SortColumn::Title, SortColumn::Artist, SortColumn::AlbumArtist,
            SortColumn::Album, SortColumn::Genre, SortColumn::Year, SortColumn::Duration,
            SortColumn::Format, SortColumn::SampleRate, SortColumn::BitDepth, SortColumn::Bitrate,
            SortColumn::Channels, SortColumn::Size
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
                container(text(if manager.search_query.is_empty() { "La biblioteca está vacía." } else { "No se encontraron álbumes." })
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
                        
                        for album_entry in chunk {
                            let album_id = &album_entry.id;
                            let artist = &album_entry.artist;
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
                    let view_h = manager.last_viewport.as_ref().map(|v| v.bounds().height).unwrap_or(1000.0);
                    let lazy_margin = if crate::utils::is_low_resource() { 600.0 } else { 900.0 };
                    
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
                        
                        for album_entry in row_chunk_data {
                            let (album_id, album, artist, genre, year, cover_path) = (
                                &album_entry.id, &album_entry.title, &album_entry.artist,
                                &album_entry.genre, &album_entry.year, &album_entry.cover_path
                            );
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
                                crate::gui::widgets::smart_truncate_text(artist.clone(), 12.0, FONT_INTER_SANS_MEDIUM, COLOR_TEXT_PRIMARY),
                                crate::gui::widgets::smart_truncate_text(album.clone(), 12.0, FONT_INTER_SANS_MEDIUM, COLOR_TEXT_PRIMARY),
                                crate::gui::widgets::smart_truncate_text(genre.clone(), 12.0, FONT_INTER_SANS_MEDIUM, COLOR_TEXT_PRIMARY),
                                text(year.as_str()).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM)
                                    .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(14.0))),
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
                            let is_selected = manager.selected_items.contains(&LibraryListItem::Album(composite_id.clone()));
                            
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
                            let mut album_songs_col = column![].spacing(0).padding([32, 0]);
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
                                        let is_song_selected = manager.selected_items.contains(&LibraryListItem::Song(song.id));
                                        let columns = [
                                            SortColumn::TrackNumber, SortColumn::Title, SortColumn::Artist,
                                            SortColumn::AlbumArtist, SortColumn::Album, SortColumn::Genre,
                                            SortColumn::Year, SortColumn::Duration, SortColumn::Format,
                                            SortColumn::SampleRate, SortColumn::BitDepth, SortColumn::Bitrate,
                                            SortColumn::Channels, SortColumn::Size,
                                        ];

                                        let song_row = crate::gui::widgets::universal_song_row_widget(
                                            song, 0, is_song_selected, &columns, &manager.column_widths,
                                            Message::SelectSong(Some(song.id)),
                                            playing_path, 32.0, false
                                        );
                                        album_songs_col = album_songs_col.push(song_row);
                                    }
                                } else {
                                    album_songs_col = album_songs_col.push(text("No se encontraron canciones.").color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM));
                                }
                            } else {
                                album_songs_col = album_songs_col.push(text("Cargando...").color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM));
                            }
                            grid_col = grid_col.push(container(album_songs_col)
                                .width(Length::Fill)
                                .padding(iced::Padding { top: 5.0, right: 10.0, bottom: 5.0, left: 5.0 })
                                .style(|_t| container::Style::default().background(COLOR_BG)));
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
                let scrollable_grid = standard_scrollable(
                    scroll_id,
                    res_grid,
                    iced::widget::scrollable::Direction::Vertical(standard_scrollbar())
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .on_scroll(Message::LibraryScroll);

                container(scrollable_grid).width(Length::Fill).height(Length::Fill).into()
            }
        },
        LibraryViewMode::SimpleList => {
            let cols = columns.clone();
            crate::gui::widgets::universal_song_list(
                manager,
                move |song, _song_idx, is_selected, playing_path| {
                    crate::gui::widgets::library_song_row_widget(
                        song, 0, is_selected, &cols, &manager.column_widths,
                        Message::SelectSong(Some(song.id)), 
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
                move |song, _song_idx, is_selected, playing_path| {
                    crate::gui::widgets::library_song_row_widget(
                        song, 0, is_selected, &cols, &manager.column_widths,
                        Message::SelectSong(Some(song.id)),
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
                move |song, _song_idx, is_selected, playing_path| {
                    crate::gui::widgets::thumbnail_song_row_widget(
                        song, 0, is_selected, &cols, &manager.column_widths,
                        Message::SelectSong(Some(song.id)),
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
        icon_btn_size("radio-button-on.svg", Message::LibraryShowPlaying, 30.0),
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

