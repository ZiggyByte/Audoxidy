//! Library manager state: fields, defaults, shared identifiers, and redraw counters.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::utils::SortColumn;

use super::types::{
    AlbumEntry, ArtistGroup, LibraryListItem, LibrarySource, LibraryStats, LibraryViewMode,
    LibraryVirtualArtist, LibraryVirtualRow,
};

// Contadores de redraw para diagnóstico — se incrementan cada vez que la vista se reconstruye
static LIBRARY_VIEW_REDRAWS: AtomicU64 = AtomicU64::new(0);
pub fn get_library_redraw_count() -> u64 {
    LIBRARY_VIEW_REDRAWS.load(Ordering::Relaxed)
}
pub fn library_redraw_count_log() {
    let count = LIBRARY_VIEW_REDRAWS.fetch_add(1, Ordering::Relaxed) + 1;
    if count % 100 == 0 {
        tracing::info!("Library view redraws: {}", count);
    }
}

/// ID estático para el scrollable de la biblioteca — garantiza que view y update usan EXACTAMENTE el mismo ID
pub static LIBRARY_SCROLL_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::unique());
pub static GRID_ID_A: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::new("grid_view_a"));
pub static GRID_ID_B: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::new("grid_view_b"));
pub static LIBRARY_SEARCH_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::new("library_search_input"));

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
    pub selection_stats: Option<LibraryStats>,
    pub filter_stats: Option<LibraryStats>,
    pub artist_last_selection: std::collections::HashMap<String, usize>,
    pub selected_item_hint: Option<(f32, f32)>,
    pub last_viewport: Option<iced::Rectangle>,
    pub scroll_offset: iced::Vector,
    pub marquee_start: Option<iced::Point>,
    pub marquee_end: Option<iced::Point>,
    pub marquee_start_pos: Option<iced::Point>, // Posición inicial en pantalla para el umbral
    pub is_dragging: bool,

    // Caché de virtualización (Interior Mutability para permitir actualización en view)
    pub cached_detailed_view: std::cell::RefCell<Option<(f32, Vec<LibraryVirtualArtist>)>>,
    pub cached_visible_elements: std::cell::RefCell<
        Option<(
            f32,
            f32,
            f32,
            Vec<LibraryVirtualRow>,
            Option<(String, bool, usize, usize, f64)>,
        )>,
    >, // (top_space, bottom_space, total_h, rows, sticky_info)
    pub data_unloaded: bool,
    pub pending_reveal_path: Option<String>,
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
            marquee_start: None,
            marquee_end: None,
            marquee_start_pos: None,
            is_dragging: false,
            scroll_offset: iced::Vector::new(0.0, 0.0),
            cached_detailed_view: std::cell::RefCell::new(None),
            cached_visible_elements: std::cell::RefCell::new(None),
            data_unloaded: false,
            pending_reveal_path: None,
        }
    }
}

pub const UNLOAD_SAFETY_LIMIT: usize = 60;
