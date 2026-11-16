//! Tests for persisted settings and the EQ-preset commands.
//!
//! Each test builds a `Core` over an in-memory database, so the key/value row
//! and the preset rows live for the length of one test and never reach the
//! application database. The audio manager is constructed because `Core` owns
//! one, but no preset is applied to it here, and no configuration file is
//! written or read.

use super::*;
use crate::db::Database;
use std::sync::{Arc, Mutex};

fn make_core() -> Core {
    let db = Database::new_memory().unwrap();
    let database = Arc::new(Mutex::new(db));
    let scanner = Arc::new(crate::db::scanner::Scanner::new(Arc::new(
        std::sync::Mutex::new(Database::new_memory().expect("in-memory db")),
    )));
    let audio_manager = Arc::new(crate::audio::AudioManager::new().expect("audio manager init"));
    let paths = super::super::types::CorePaths::desktop_default();
    Core::new(audio_manager, database, scanner, paths)
}

#[test]
fn test_settings_load_persist_round_trip() {
    let core = make_core();

    // Persist a setting
    core.persist_audio_setting("test_key", "test_value")
        .unwrap();

    // Load it back via the raw DB
    let value = {
        let db = core.database.lock().expect("lock");
        db.get_setting("test_key")
    };
    assert_eq!(value.as_deref(), Some("test_value"));
}

#[test]
fn test_eq_preset_save_load_delete() {
    let core = make_core();

    // Save a preset
    core.save_eq_preset("Test", 2.5, Some(&[1.0; 20]), Some(&[2.0; 31]))
        .unwrap();

    // Load presets
    let presets = core.load_eq_presets();
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0].name, "Test");
    assert_eq!(presets[0].preamp_gain, 2.5);
    assert_eq!(presets[0].bands_20.as_ref().unwrap().len(), 20);
    assert_eq!(presets[0].bands_31.as_ref().unwrap().len(), 31);

    // Delete
    core.delete_eq_preset("Test").unwrap();
    assert!(core.load_eq_presets().is_empty());
}

#[test]
fn test_eq_preset_clear() {
    let core = make_core();

    core.save_eq_preset("A", 0.0, None, None).unwrap();
    core.save_eq_preset("B", 0.0, None, None).unwrap();
    assert_eq!(core.load_eq_presets().len(), 2);

    core.clear_eq_presets().unwrap();
    assert!(core.load_eq_presets().is_empty());
}
