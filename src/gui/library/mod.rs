//! Library panel: state, virtualization, selection, data processing, and rendering.
//!
//! The module root declares the concern submodules and re-exports the public
//! surface so existing `crate::gui::library::*` paths keep resolving.

pub(crate) mod cover_cache;
mod data;
pub(crate) mod grid_metrics;
pub(crate) mod lists;
mod selection;
mod state;
mod types;
mod view;
mod virtualization;

pub use grid_metrics::{GridMetrics, RowSpan};
pub use state::{
    GRID_ID_A, GRID_ID_B, LIBRARY_SCROLL_ID, LIBRARY_SEARCH_ID, LibraryManager,
    UNLOAD_SAFETY_LIMIT, get_library_redraw_count, library_redraw_count_log,
};
pub use types::{
    AlbumEntry, ArtistGroup, LibraryAlbumGroup, LibraryGridElement, LibraryListItem, LibraryNavDir,
    LibrarySource, LibraryStats, LibraryViewMode, LibraryVirtualArtist, LibraryVirtualRow,
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

    #[test]
    fn test_refresh_expanded_album_songs_keeps_versions_separate() {
        let mut manager = LibraryManager::default();
        manager.view_mode = LibraryViewMode::Grid;

        let mut first = SongData::default();
        first.title = Some(Arc::from("Primera"));
        first.album = Some(Arc::from("Greatest Hits"));
        first.artist = Some(Arc::from("Artista"));
        first.album_hash = Some(Arc::from("hash-a"));

        let mut second = SongData::default();
        second.title = Some(Arc::from("Segunda"));
        second.album = Some(Arc::from("Greatest Hits"));
        second.artist = Some(Arc::from("Artista"));
        second.album_hash = Some(Arc::from("hash-b"));

        manager.filtered_songs = Some(vec![Arc::new(first), Arc::new(second)]);
        manager.cached_albums = Some(vec![
            AlbumEntry {
                id: "hash-a".into(),
                title: "Greatest Hits".into(),
                artist: "Artista".into(),
                genre: String::new(),
                year: String::new(),
                cover_path: Some("a.avif".into()),
            },
            AlbumEntry {
                id: "hash-b".into(),
                title: "Greatest Hits".into(),
                artist: "Artista".into(),
                genre: String::new(),
                year: String::new(),
                cover_path: Some("b.avif".into()),
            },
        ]);
        manager.expanded_album = Some("Artista|hash-a".into());

        manager.refresh_expanded_album_songs();

        let songs = manager
            .expanded_album_songs
            .as_ref()
            .expect("el álbum expandido debe tener canciones");
        assert_eq!(songs.len(), 1, "solo la versión expandida debe aparecer");
        assert_eq!(songs[0].title.as_deref(), Some("Primera"));
    }
}

#[cfg(test)]
mod data_tests;

#[cfg(test)]
mod grid_metrics_tests;

#[cfg(test)]
mod sort_tests;

#[cfg(test)]
mod cover_cache_tests;
