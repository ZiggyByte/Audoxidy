//! Playlist panel: state, list logic, rendering, and context-menu construction.
//!
//! The module root declares the concern submodules and re-exports the public
//! surface so existing `crate::gui::playlist::*` paths keep resolving.

mod context_menu;
mod logic;
mod state;
mod view;

pub use context_menu::get_item_context_menu_entries;
pub use state::{PlaylistItemType, PlaylistManager};
pub use state::{count_linear_items, flatten_groups};
pub use view::PLAYLIST_SCROLL_ID;
pub use view::PLAYLIST_SEARCH_ID;
pub use view::PLAYLIST_TABS_SCROLL_ID;
pub use view::estimate_tab_width;
pub use view::view;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::database::{PlaylistFolderGroup, PlaylistSongRef, ShuffleSession};

    fn make_song(song_id: i64, path: &str) -> PlaylistSongRef {
        PlaylistSongRef {
            item_id: song_id,
            song_id,
            sequence_order: song_id as f64,
            enabled: true,
            title: format!("Título {}", song_id).into(),
            artist_name: "Artista".into(),
            album_title: "Álbum".into(),
            album_artist_name: "Artista".into(),
            year: None,
            genre: None,
            track_number: None,
            duration: 180.0,
            file_path: path.into(),
            folder_path: "/".into(),
            folder_name: "Raíz".into(),
            cover_path: None,
            cover_override: None,
        }
    }

    fn manager_with_songs(songs: Vec<PlaylistSongRef>) -> PlaylistManager {
        let mut pm = PlaylistManager::default();
        pm.set_groups(vec![PlaylistFolderGroup {
            folder_path: "/".into(),
            folder_name: "Raíz".into(),
            songs,
            total_duration: 0.0,
            first_item_id: 1,
        }]);
        pm
    }

    #[test]
    fn test_get_next_song_ref_sequential_skips_disabled() {
        let mut songs = vec![make_song(1, "/a.flac"), make_song(2, "/b.flac")];
        songs[0].enabled = false; // La actual está deshabilitada
        let pm = manager_with_songs(songs);
        // playing_song_idx apunta a la canción 1 (índice lineal 1: separador + 0)
        let pm2 = PlaylistManager {
            playing_song_idx: Some(1),
            ..pm
        };
        let next = pm2.get_next_song_ref();
        assert!(next.is_some());
        assert_eq!(next.unwrap().song_id, 2, "Debe saltar la deshabilitada");
    }

    #[test]
    fn test_get_next_song_ref_shuffle_skips_disabled() {
        // Orden shuffle: [1 (deshabilitada), 3, 2]
        let mut songs = vec![
            make_song(1, "/a.flac"),
            make_song(2, "/b.flac"),
            make_song(3, "/c.flac"),
        ];
        songs[0].enabled = false;
        let mut pm = manager_with_songs(songs);
        pm.shuffle_active = true;
        pm.shuffle_session = Some(ShuffleSession {
            session_id: "s1".into(),
            shuffle_order: vec![1, 3, 2],
            current_position: 0,
            history: vec![4], // canción previa
        });
        pm.playing_song_idx = Some(1); // La canción 1 (índice lineal 1)
        let next = pm.get_next_song_ref();
        assert!(next.is_some());
        assert_eq!(
            next.unwrap().song_id,
            3,
            "En shuffle debe saltar la deshabilitada y tomar la siguiente del orden"
        );
    }

    #[test]
    fn test_get_next_song_ref_shuffle_end_returns_none() {
        let songs = vec![make_song(1, "/a.flac")];
        let mut pm = manager_with_songs(songs);
        pm.shuffle_active = true;
        pm.shuffle_session = Some(ShuffleSession {
            session_id: "s1".into(),
            shuffle_order: vec![1],
            current_position: 1, // Ya al final del orden
            history: vec![1],
        });
        pm.playing_song_idx = Some(1);
        assert!(pm.get_next_song_ref().is_none());
    }
}
