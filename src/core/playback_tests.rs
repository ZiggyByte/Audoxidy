//! Tests for the transport delegation, the preload trigger and the crossfade rules.
//!
//! The transport tests drive a `Core` over an in-memory database and a live audio
//! manager, so they assert that a call returns or does not panic instead of
//! interrogating a device: a headless machine legitimately reports no outputs and
//! no test opens the application database. The decision tests are pure — they feed
//! the trigger helpers the values the tick would pass and read the result back.

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
fn test_transport_delegates_to_audio_manager() {
    let core = make_core_for_test();
    // toggle_play_pause should not panic
    core.toggle_play_pause();
    // set_volume should not panic
    core.set_volume(0.5);
    // is_playing should return a bool
    let _ = core.is_playing();
}

#[test]
fn test_clear_eof_does_not_panic() {
    let core = make_core_for_test();
    core.clear_eof();
}

#[test]
fn test_stop_does_not_panic() {
    let core = make_core_for_test();
    core.stop();
}

#[test]
fn test_preload_trigger_below_15s() {
    let decision = check_preload_trigger(10.0, Some("/path/to/song.flac"), None);
    assert!(decision.should_preload);
    assert_eq!(decision.next_path.as_deref(), Some("/path/to/song.flac"));
    assert!(decision.clear_preload_finished);
}

#[test]
fn test_preload_trigger_no_dedup_when_same_path() {
    let decision =
        check_preload_trigger(10.0, Some("/path/to/song.flac"), Some("/path/to/song.flac"));
    assert!(!decision.should_preload);
}

#[test]
fn test_preload_trigger_above_15s_no_preload() {
    let decision = check_preload_trigger(20.0, Some("/path/to/song.flac"), None);
    assert!(!decision.should_preload);
}

#[test]
fn test_preload_trigger_no_next_song() {
    let decision = check_preload_trigger(10.0, None, None);
    assert!(!decision.should_preload);
}

#[test]
fn test_crossfade_trigger_all_enabled() {
    assert!(check_crossfade_trigger(true, true, 2000.0));
}

#[test]
fn test_crossfade_trigger_disabled() {
    assert!(!check_crossfade_trigger(false, true, 2000.0));
    assert!(!check_crossfade_trigger(true, false, 2000.0));
    assert!(!check_crossfade_trigger(true, true, 0.0));
}

#[test]
fn test_manual_next_crossfade_skips_when_disabled() {
    let core = make_core_for_test();
    let next_song = NextSongInfo {
        file_path: Some("/path/to/song.flac".to_string()),
        title: Some("Title".to_string()),
        artist: Some("Artist".to_string()),
        album: Some("Album".to_string()),
        cover_path: None,
    };
    let triggered =
        manual_next_crossfade(&core.audio_manager, false, true, 2000.0, &next_song, None);
    assert!(!triggered);
}
