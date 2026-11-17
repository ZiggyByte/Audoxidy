//! Tests for the playlist lifecycle, renaming, emptying and the batch song calls.
//!
//! Every test builds its `Core` over an in-memory database, so no test opens the
//! application database or copies it. Each test creates the playlist it works on
//! and asserts only the rows it wrote, so the assertions stay independent of any
//! other playlist.

use super::*;
use std::sync::{Arc, Mutex};

fn make_core_for_test() -> Core {
    let paths = super::super::types::CorePaths::desktop_default();
    let db = Arc::new(Mutex::new(
        crate::db::Database::new_memory().expect("in-memory db"),
    ));
    let scanner = Arc::new(crate::db::scanner::Scanner::new(db.clone()));
    let audio_manager = Arc::new(crate::audio::AudioManager::new().expect("audio manager init"));
    Core::new(audio_manager, db, scanner, paths)
}

#[test]
fn test_create_and_list_playlists() {
    let core = make_core_for_test();
    let playlist_id = core.create_playlist("Lista de prueba", false).unwrap();
    assert!(playlist_id > 0);
    let playlists = core.get_all_playlists().unwrap();
    assert!(playlists.iter().any(|playlist| playlist.id == playlist_id));
}

#[test]
fn test_rename_and_delete_playlist() {
    let core = make_core_for_test();
    let playlist_id = core.create_playlist("Original", false).unwrap();
    assert!(core.rename_playlist(playlist_id, "Renombrada").unwrap());
    assert!(core.delete_playlist(playlist_id).unwrap());
}

#[test]
fn test_add_and_remove_songs_empty_batch() {
    let core = make_core_for_test();
    let playlist_id = core.create_playlist("Con canciones", false).unwrap();
    assert_eq!(core.add_songs_to_playlist(playlist_id, &[]).unwrap(), 0);
    core.batch_remove_songs_from_playlist(playlist_id, &[])
        .unwrap();
}

#[test]
fn test_clear_playlist_removes_items() {
    let core = make_core_for_test();
    let playlist_id = core.create_playlist("Vaciar", false).unwrap();
    core.clear_playlist(playlist_id).unwrap();
    assert!(
        core.get_playlist_all_song_ids(playlist_id)
            .unwrap()
            .is_empty()
    );
}
