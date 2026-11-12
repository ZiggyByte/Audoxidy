//! Library domain types shared by the library submodules.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LibraryViewMode {
    Grid,
    DetailedList,
    ThumbnailList,
    SimpleList,
}

impl LibraryViewMode {
    /// Returns the stable settings value persisted for this view mode.
    pub fn as_setting_value(self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::DetailedList => "detailed_list",
            Self::ThumbnailList => "thumbnail_list",
            Self::SimpleList => "simple_list",
        }
    }

    /// Resolves a stored settings value back to its view mode.
    ///
    /// A missing, empty or unrecognised value resolves to the grid, which is
    /// the first-launch default.
    pub fn from_setting_value(value: Option<&str>) -> Self {
        match value {
            Some("detailed_list") => Self::DetailedList,
            Some("thumbnail_list") => Self::ThumbnailList,
            Some("simple_list") => Self::SimpleList,
            _ => Self::Grid,
        }
    }
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

/// Estructura para agrupar canciones por álbum en la vista de biblioteca
#[derive(Debug, Clone)]
pub struct LibraryAlbumGroup {
    pub album_name: String,
    pub album_hash: String,
    /// Ruta de carátula ya resuelta para este álbum, tomada del snapshot.
    pub cover_path: Option<String>,
    pub songs: Vec<(std::sync::Arc<crate::db::database::SongData>, usize)>, // (song, global_idx)
    pub duration_secs: f64,
}

/// Estructura para agrupar la jerarquía completa de un artista para virtualización
#[derive(Debug, Clone)]
pub struct LibraryVirtualArtist {
    pub artist_name: String,
    pub albums_count: usize,
    pub songs_count: usize,
    pub duration_secs: f64,
    pub is_collapsed: bool,
    pub top_y: f32,
    pub height: f32,
    pub albums: Vec<LibraryAlbumGroup>,
}

/// Elementos visibles en la cuadrícula virtualizada
#[derive(Debug, Clone)]
pub enum LibraryGridElement {
    AlbumRow {
        albums: Vec<AlbumEntry>,
        active_expansion_id: Option<String>,
    },
    ExpandedSongs {
        album_id: String,
        songs: Vec<(std::sync::Arc<crate::db::database::SongData>, usize)>,
        top_space: f32,
        bottom_space: f32,
    },
}

/// Elementos que pueden aparecer en una lista virtualizada de la biblioteca
#[derive(Debug, Clone)]
pub enum LibraryVirtualRow {
    ArtistHeader {
        name: String,
        is_collapsed: bool,
        albums_count: usize,
        songs_count: usize,
        duration_secs: f64,
    },
    AlbumBlock {
        album_name: String,
        album_hash: String,
        artist_name: String,
        /// Ruta de carátula ya resuelta para este álbum, tomada del snapshot.
        cover_path: Option<String>,
        genre: String,
        year: String,
        is_expanded: bool,
        songs: Vec<(std::sync::Arc<crate::db::database::SongData>, usize)>,
        songs_count: usize,
        duration_secs: f64,
    },
    SimpleSong {
        song: std::sync::Arc<crate::db::database::SongData>,
        global_idx: usize,
    },
}
