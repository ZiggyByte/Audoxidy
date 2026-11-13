//! Data types persisted by the music library database.

/// The library-row projection the grid, the list modes and the player draw.
///
/// It carries only the columns those consumers read: a column no consumer renders
/// or groups by is not part of the row. `import_order` stays because the scanner
/// writes it as part of the insert payload, not because a read selects it.
#[derive(Debug, Default, Clone)]
pub struct SongData {
    pub id: i64,
    /// Identity of the album row (`ALBUMS.hash_id`), carried so a view groups
    /// two versions that share a title and artist as separate albums.
    pub album_hash: Option<std::sync::Arc<str>>,
    pub full_file_path: std::sync::Arc<str>,
    pub title: Option<std::sync::Arc<str>>,
    pub artist: Option<std::sync::Arc<str>>,
    pub album: Option<std::sync::Arc<str>>,
    pub album_artist: Option<std::sync::Arc<str>>,
    pub genre: Option<std::sync::Arc<str>>,
    pub release_year: Option<std::sync::Arc<str>>,
    pub track_number: Option<std::sync::Arc<str>>,
    pub duration_secs: Option<f64>,
    pub format: Option<std::sync::Arc<str>>,
    pub bit_depth: Option<i64>,
    pub sample_rate: Option<i64>,
    pub size: Option<i64>,
    pub channels: Option<i64>,
    pub compressed_cached_cover_root: Option<std::sync::Arc<str>>,
    pub import_order: i64,
}

/// Metadatos extendidos para edición y visualización de propiedades detalladas.
#[derive(Debug, Default, Clone)]
pub struct SongMetadataExtended {
    pub song_id: i64,
    pub lyrics: Option<String>,
    pub comments: Option<String>,
    pub composer: Option<String>,
    pub lyricist: Option<String>,
    pub publisher: Option<String>,
    pub url: Option<String>,
    pub copyright: Option<String>,
    pub encoded_by: Option<String>,
    pub catalog: Option<String>,
    pub isrc: Option<String>,
    pub key: Option<String>,
    pub bpm: Option<String>,
    pub track_gain: Option<f64>,
    pub album_gain: Option<f64>,
}

/// Datos esenciales de una lista de reproducción.
#[derive(Debug, Clone)]
pub struct PlaylistData {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
    pub is_system: bool,
    pub created_at: i64,
    pub last_song_id: Option<i64>,
    pub last_pos_sec: f64,
    pub is_playing: bool,
    pub shuffle_active: bool,
    pub repeat_mode: i32,
    pub shuffle_pos: usize,
    pub shuffle_id: Option<String>,
}

/// Item de una lista de reproducción (referencia + estado, sin duplicar metadata).
#[derive(Debug, Clone)]
pub struct PlaylistSongRef {
    pub item_id: i64,
    pub song_id: i64,
    pub sequence_order: f64,
    pub enabled: bool,

    // Metadata JOIN de SONGS + ALBUMS + ARTISTS
    pub title: std::sync::Arc<str>,
    pub artist_name: std::sync::Arc<str>,
    pub album_title: std::sync::Arc<str>,
    pub album_artist_name: std::sync::Arc<str>,
    pub year: Option<std::sync::Arc<str>>,
    pub genre: Option<std::sync::Arc<str>>,
    pub track_number: Option<std::sync::Arc<str>>,
    pub duration: f64,
    pub file_path: std::sync::Arc<str>,
    pub folder_path: std::sync::Arc<str>,
    pub folder_name: std::sync::Arc<str>,
    pub cover_path: Option<std::sync::Arc<str>>,
    pub cover_override: Option<std::sync::Arc<str>>,
}

/// Grupo de canciones de playlist agrupadas por carpeta/álbum.
#[derive(Debug, Clone)]
pub struct PlaylistFolderGroup {
    pub folder_path: std::sync::Arc<str>,
    pub folder_name: std::sync::Arc<str>,
    pub songs: Vec<PlaylistSongRef>,
    pub total_duration: f64,
    pub first_item_id: i64,
}

/// Sesión de shuffle: orden aleatorio + historial para navegación backward.
#[derive(Debug, Clone)]
pub struct ShuffleSession {
    pub session_id: String,
    pub shuffle_order: Vec<i64>, // song_ids en orden aleatorio
    pub current_position: usize,
    pub history: Vec<i64>, // song_ids ya reproducidos en orden
}

/// Parámetros para la búsqueda y filtrado de la biblioteca.
#[derive(Debug, Default, Clone)]
pub struct LibrarySearchParams {
    pub query: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<String>,
    pub folder_id: Option<i64>,
}
