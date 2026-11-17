//! Area tests for the playlist manager: expansion, selection and ordering.
//!
//! They pin the next-song lookup under sequential and shuffle order, the skip
//! of a disabled song, and the filter's effect on the active list, the playing
//! index and the stored positions. Each test builds its own `PlaylistManager`
//! from in-memory groups, so no database is opened.

use super::*;
use crate::db::database::{PlaylistFolderGroup, PlaylistSongRef, ShuffleSession};

fn make_song(song_id: i64, path: &str) -> std::sync::Arc<PlaylistSongRef> {
    std::sync::Arc::new(PlaylistSongRef {
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
    })
}

fn manager_with_songs(songs: Vec<std::sync::Arc<PlaylistSongRef>>) -> PlaylistManager {
    let mut manager = PlaylistManager::default();
    manager.set_groups(vec![PlaylistFolderGroup {
        folder_path: "/".into(),
        folder_name: "Raíz".into(),
        songs,
        total_duration: 0.0,
        first_item_id: 1,
    }]);
    manager
}

#[test]
fn test_get_next_song_ref_sequential_skips_disabled() {
    let mut songs = vec![make_song(1, "/a.flac"), make_song(2, "/b.flac")];
    std::sync::Arc::make_mut(&mut songs[0]).enabled = false; // La actual está deshabilitada
    let manager = manager_with_songs(songs);
    // playing_song_idx apunta a la canción 1 (índice lineal 1: separador + 0)
    let playing_manager = PlaylistManager {
        playing_song_idx: Some(1),
        ..manager
    };
    let next = playing_manager.get_next_song_ref();
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
    std::sync::Arc::make_mut(&mut songs[0]).enabled = false;
    let mut manager = manager_with_songs(songs);
    manager.shuffle_active = true;
    manager.shuffle_session = Some(ShuffleSession {
        session_id: "s1".into(),
        shuffle_order: vec![1, 3, 2],
        current_position: 0,
        history: vec![4], // canción previa
    });
    manager.playing_song_idx = Some(1); // La canción 1 (índice lineal 1)
    let next = manager.get_next_song_ref();
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
    let mut manager = manager_with_songs(songs);
    manager.shuffle_active = true;
    manager.shuffle_session = Some(ShuffleSession {
        session_id: "s1".into(),
        shuffle_order: vec![1],
        current_position: 1, // Ya al final del orden
        history: vec![1],
    });
    manager.playing_song_idx = Some(1);
    assert!(manager.get_next_song_ref().is_none());
}

#[test]
fn test_filter_with_no_matches_yields_an_empty_active_list() {
    let mut manager = manager_with_songs(vec![make_song(1, "/a.flac"), make_song(2, "/b.flac")]);
    manager.search_query = "no-existe".to_string();
    manager.apply_filter();

    assert!(manager.active_groups().is_empty());
    assert!(manager.filtered_groups.is_some());
}

#[test]
fn test_filter_remaps_the_playing_index_by_song_id() {
    let mut manager = manager_with_songs(vec![
        make_song(1, "/a.flac"),
        make_song(2, "/b.flac"),
        make_song(3, "/c.flac"),
    ]);
    // Canción 2: separador + índice 1.
    manager.playing_song_idx = Some(2);
    manager.search_query = "Título 2".to_string();
    manager.apply_filter();

    assert_eq!(manager.playing_song_idx, Some(1));
    assert_eq!(
        manager.get_song_at_linear_index(1).map(|song| song.song_id),
        Some(2)
    );
}

#[test]
fn test_filter_clears_positions_that_belong_to_the_previous_list() {
    let mut manager = manager_with_songs(vec![make_song(1, "/a.flac"), make_song(2, "/b.flac")]);
    manager.selected_idxs.insert(1);
    manager.focused_idx = Some(1);
    manager.selection_pivot = Some(1);
    manager.search_query = "Título 2".to_string();
    manager.apply_filter();

    assert!(manager.selected_idxs.is_empty());
    assert_eq!(manager.focused_idx, None);
    assert_eq!(manager.selection_pivot, None);
}
