//! Library panel: state, virtualization, selection, data processing, and rendering.
//!
//! The module root declares the concern submodules and re-exports the public
//! surface so existing `crate::gui::library::*` paths keep resolving.

mod data;
pub(crate) mod lists;
mod selection;
mod state;
mod types;
mod view;
mod virtualization;

pub use state::{
    GRID_ID_A, GRID_ID_B, LIBRARY_SCROLL_ID, LIBRARY_SEARCH_ID, LibraryManager,
    UNLOAD_SAFETY_LIMIT, get_library_redraw_count, library_redraw_count_log,
};
pub use types::{
    AlbumEntry, ArtistGroup, LibraryAlbumGroup, LibraryGridElement, LibraryGridRowData,
    LibraryListItem, LibraryNavDir, LibrarySource, LibraryStats, LibraryViewMode,
    LibraryVirtualArtist, LibraryVirtualRow,
};
pub use view::view;

#[cfg(test)]
mod library_tests {
    use super::*;
    use crate::db::database::SongData;
    use crate::utils::SortColumn;
    use std::sync::Arc;

    fn song_with(duration: Option<f64>, size: Option<i64>) -> Arc<SongData> {
        let mut song = SongData::default();
        song.duration_secs = duration;
        song.size = size;
        Arc::new(song)
    }

    /// Guarda de regresión: el comparador de bitrate calcula la tasa con
    /// `filter` en lugar de `unwrap`, de modo que una duración ausente (`None`)
    /// o cero no lo rompe. El orden debe ser estable entre llamadas.
    #[test]
    fn test_bitrate_sort_handles_missing_duration() {
        let mut songs = vec![
            song_with(None, Some(10_000)),
            song_with(Some(0.0), Some(10_000)),
            song_with(Some(120.0), Some(4_000_000)),
            song_with(Some(60.0), Some(500_000)),
        ];

        let sort_snapshot = |songs: &mut Vec<Arc<SongData>>| {
            LibraryManager::sort_songs_static(songs, Some(SortColumn::Bitrate), Some(true));
            songs
                .iter()
                .map(|s| (s.duration_secs, s.size))
                .collect::<Vec<_>>()
        };

        let first = sort_snapshot(&mut songs.clone());
        let second = sort_snapshot(&mut songs);
        assert_eq!(
            first, second,
            "el orden por bitrate con duración ausente/cero debe ser estable"
        );

        // Las entradas sin duración válida quedan al principio (tasa 0.0).
        assert_eq!(first[0].0, None);
        assert_eq!(first[1].0, Some(0.0));
    }
}
