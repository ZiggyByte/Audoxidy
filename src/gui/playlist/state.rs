use crate::db::database::{PlaylistFolderGroup, PlaylistSongRef, ShuffleSession};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum PlaylistItemType {
    Separator(usize, i64),   // group_idx, first_item_id
    Song(usize, usize, i64), // group_idx, song_local_idx, first_item_id
}

// ============================================================
// Estado del PlaylistManager
// ============================================================

#[derive(Clone)]
pub struct PlaylistManager {
    /// ID de la playlist activa (referencia a BD)
    pub active_playlist_id: i64,

    /// Canciones de la playlist activa, agrupadas por carpeta
    pub groups: Vec<PlaylistFolderGroup>,

    /// Índice de la canción que se está reproduciendo (dentro del grupo aplanado)
    pub playing_song_idx: Option<usize>,

    /// Selected items in the current playlist
    pub selected_idxs: std::collections::HashSet<usize>,

    /// Currently focused item for keyboard navigation
    pub focused_idx: Option<usize>,

    /// Selection pivot for Shift-range selection
    pub selection_pivot: Option<usize>,

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

    /// Lista de todas las playlists disponibles (para la barra de pestañas)
    pub playlists: Vec<crate::db::database::PlaylistData>,

    /// Carpetas plegadas (IDs de la primera canción del grupo como anchor)
    pub collapsed_groups: std::collections::HashSet<i64>,

    /// Información del último clic para detectar doble clic (linear_idx, instant)
    pub last_click_info: Option<(usize, std::time::Instant)>,

    /// Guardar el último viewport (ventana visible) para autoscroll
    pub last_viewport: Option<iced::Rectangle>,

    /// Guardar el último viewport de la barra de pestañas
    pub tabs_viewport: Option<iced::Rectangle>,

    /// Caché de ítems visibles (aplanados y filtrados por colapso) compartida vía Arc
    pub cached_visible_items: std::cell::RefCell<Option<Arc<Vec<(PlaylistItemType, usize)>>>>,

    /// Indica si los datos pesados han sido descargados de RAM por inactividad
    pub data_unloaded: bool,
}

impl Default for PlaylistManager {
    fn default() -> Self {
        Self {
            active_playlist_id: 1,
            groups: Vec::new(),
            playing_song_idx: None,
            selected_idxs: std::collections::HashSet::new(),
            focused_idx: None,
            selection_pivot: None,
            shuffle_active: false,
            repeat_mode: 0,
            search_query: String::new(),
            filtered_groups: None,
            show_tab_dropdown: false,
            visible_tabs_count: None,
            shuffle_session: None,
            playlists: Vec::new(),
            collapsed_groups: std::collections::HashSet::new(),
            last_click_info: None,
            last_viewport: None,
            tabs_viewport: None,
            cached_visible_items: std::cell::RefCell::new(None),
            data_unloaded: false,
        }
    }
}

/// Aplanar todos los grupos en un solo índice lineal
pub fn flatten_groups(groups: &[PlaylistFolderGroup]) -> Vec<&PlaylistSongRef> {
    groups.iter().flat_map(|g| g.songs.iter()).collect()
}

/// Contar items lineales (separadores + canciones)
pub fn count_linear_items(groups: &[PlaylistFolderGroup]) -> usize {
    groups.iter().map(|g| 1 + g.songs.len()).sum()
}
