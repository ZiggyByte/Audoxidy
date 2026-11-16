//! Tests for the audio routing commands and the settings blob decoding.
//!
//! Each test builds a `Core` over an in-memory database and a live audio
//! manager, so every routing call runs through the same delegation and lock
//! order the application uses. The assertions stay at the contract level: a
//! call succeeds, or it fails with the typed error the command documents. The
//! host's actual device inventory is never asserted, because a headless
//! machine legitimately reports no outputs. No test writes a database file,
//! a cover or a configuration file.

use super::*;
use crate::core::Core;
use crate::core::types::CorePaths;
use crate::db::Database;
use crate::db::scanner::Scanner;
use std::sync::{Arc, Mutex};

fn make_core_for_test() -> Core {
    let paths = CorePaths::desktop_default();
    let db = Arc::new(Mutex::new(Database::new_memory().expect("in-memory db")));
    let scanner = Arc::new(Scanner::new(db.clone()));
    let audio_manager = Arc::new(crate::audio::AudioManager::new().expect("audio manager init"));
    Core::new(audio_manager, db, scanner, paths)
}

#[test]
fn test_get_available_hosts_returns_vec() {
    let core = make_core_for_test();
    let hosts = core.get_available_hosts();
    assert!(!hosts.is_empty(), "should have at least one audio host");
}

#[test]
fn test_get_available_devices_returns_vec() {
    let core = make_core_for_test();
    let devices = core.get_available_devices();
    // On CI or headless, there might be no devices — that's fine.
    let _ = devices;
}

#[test]
fn test_apply_audio_settings_does_not_panic() {
    let core = make_core_for_test();
    let snapshot = AudioSettingsSnapshot {
        volume: 0.8,
        preamp_gain: 0.0,
        ..Default::default()
    };
    core.apply_audio_settings(&snapshot).unwrap();
}

#[test]
fn test_apply_eq_preset_not_found() {
    let core = make_core_for_test();
    let result = core.apply_eq_preset("Nonexistent Preset");
    assert!(result.is_err());
    match result.unwrap_err() {
        CoreError::NotFound(name) => assert_eq!(name, "Nonexistent Preset"),
        other => panic!("expected NotFound, got: {other}"),
    }
}

#[test]
fn test_apply_eq_preset_found() {
    let core = make_core_for_test();
    // Save a preset first.
    core.save_eq_preset("Test", 2.5, Some(&[1.0; 20]), Some(&[2.0; 31]))
        .unwrap();
    // Apply it — should succeed.
    core.apply_eq_preset("Test").unwrap();
}

#[test]
fn test_apply_system_audio_config_noop_when_none() {
    let core = make_core_for_test();
    // Both None → no-op.
    core.apply_system_audio_config(None, None).unwrap();
}

#[test]
fn test_deserialize_f32_blob_valid() {
    let data: Vec<u8> = vec![0.0f32, 1.0f32]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let result = deserialize_f32_blob(Some(data));
    assert_eq!(result, Some(vec![0.0, 1.0]));
}

#[test]
fn test_deserialize_f32_blob_empty() {
    assert_eq!(deserialize_f32_blob(Some(vec![])), None);
}

#[test]
fn test_deserialize_f32_blob_none() {
    assert_eq!(deserialize_f32_blob(None), None);
}
