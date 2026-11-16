//! Tests for the library queries, the statistics and the scan event flow.
//!
//! Every test builds its `Core` over an in-memory database, so no test opens the
//! application database or copies it. The scan test points at a path that does
//! not exist, which exercises the start/finish event flow without walking a real
//! folder.

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
fn test_get_all_songs_returns_empty_for_fresh_db() {
    let core = make_core_for_test();
    let songs = core.get_all_songs();
    assert!(songs.is_empty());
}

#[test]
fn test_get_library_stats_returns_zeros_for_fresh_db() {
    let core = make_core_for_test();
    let stats = core.get_library_stats().unwrap();
    assert_eq!(stats.total_songs, 0);
    assert_eq!(stats.total_albums, 0);
    assert_eq!(stats.total_artists, 0);
}

#[test]
fn test_get_albums_returns_empty_for_fresh_db() {
    let core = make_core_for_test();
    let albums = core.get_albums();
    assert!(albums.is_empty());
}

#[test]
fn test_get_all_folders_returns_empty_for_fresh_db() {
    let core = make_core_for_test();
    let folders = core.get_all_folders().unwrap();
    assert!(folders.is_empty());
}

#[test]
fn test_delete_songs_on_empty_db_succeeds() {
    let core = make_core_for_test();
    core.delete_songs(&[1, 2, 3]).unwrap();
}

#[test]
fn test_is_scanning_returns_false_initially() {
    let core = make_core_for_test();
    assert!(!core.is_scanning());
}

#[test]
fn test_scan_folder_emits_started_and_completed() {
    let core = make_core_for_test();
    let events = core.event_receiver();
    core.scan_folder("/nonexistent/audoxidy-scan-path".to_string());

    assert!(matches!(
        events
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("ScanStarted must be emitted"),
        super::super::events::CoreEvent::ScanStarted
    ));
    match events
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("ScanCompleted must be emitted")
    {
        super::super::events::CoreEvent::ScanCompleted { songs_added } => {
            assert_eq!(songs_added, 0);
        }
        other => panic!("expected ScanCompleted, got {other:?}"),
    }
}
