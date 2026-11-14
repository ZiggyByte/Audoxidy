//! Area tests for the whole-table settings load and the settings delete.
//!
//! Both run against an in-memory database, so no file and no application
//! database is involved.

use super::Database;

#[test]
fn test_load_all_settings_returns_every_row() {
    let db = Database::new_memory().unwrap();
    db.set_setting("audio_host", "pipewire").unwrap();
    db.set_setting("player_volume", "0.6").unwrap();

    let settings = db.load_all_settings().unwrap();

    assert_eq!(
        settings.get("audio_host").map(String::as_str),
        Some("pipewire")
    );
    assert_eq!(
        settings.get("player_volume").map(String::as_str),
        Some("0.6")
    );
}

#[test]
fn test_load_all_settings_omits_null_values() {
    let db = Database::new_memory().unwrap();
    db.set_setting("present", "value").unwrap();
    db.conn
        .execute(
            "INSERT INTO APP_SETTINGS (key, value) VALUES (?1, NULL)",
            ["null_value"],
        )
        .unwrap();

    let settings = db.load_all_settings().unwrap();

    assert_eq!(settings.get("present").map(String::as_str), Some("value"));
    assert!(
        !settings.contains_key("null_value"),
        "a null value must not reach the map"
    );
    assert!(
        db.get_setting("null_value").is_none(),
        "the single-key read already reports a null value as absent"
    );
    assert!(
        db.get_setting("absent").is_none(),
        "an absent key is indistinguishable from a null value"
    );
}

#[test]
fn test_delete_setting_removes_the_key() {
    let db = Database::new_memory().unwrap();
    db.set_setting("keep", "1").unwrap();
    db.set_setting("remove", "2").unwrap();

    db.delete_setting("remove").unwrap();

    let settings = db.load_all_settings().unwrap();
    assert!(!settings.contains_key("remove"), "the deleted key is gone");
    assert_eq!(settings.get("keep").map(String::as_str), Some("1"));
    assert!(db.get_setting("remove").is_none());
}
