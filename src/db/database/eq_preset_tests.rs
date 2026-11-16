//! Area tests for equalizer presets: the serialized band blobs and the
//! name-keyed save, load, delete and clear paths.
//!
//! Every test runs against an in-memory database, so no test touches the
//! application database.

use super::*;

/// Serializes a slice of f32 to a little-endian byte blob.
fn serialize(bands: Option<&[f32]>) -> Option<Vec<u8>> {
    bands.map(|v| v.iter().flat_map(|f| f.to_le_bytes()).collect())
}

/// Deserializes a little-endian byte blob to a Vec<f32>.
fn deserialize(blob: Option<Vec<u8>>) -> Option<Vec<f32>> {
    blob.and_then(|bytes| {
        if bytes.is_empty() || bytes.len() % 4 != 0 {
            return None;
        }
        Some(
            bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                .collect(),
        )
    })
}

#[test]
fn test_save_and_load_eq_presets() {
    let db = Database::new_memory().unwrap();
    let b20 = serialize(Some(&[1.0; 20]));
    let b31 = serialize(Some(&[2.0; 31]));
    db.save_eq_preset_raw("Test Preset", 2.5, b20, b31).unwrap();

    let loaded = db.load_eq_presets_raw().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].0, "Test Preset");
    assert_eq!(loaded[0].1, 2.5);
    assert_eq!(deserialize(loaded[0].2.clone()).unwrap().len(), 20);
    assert_eq!(deserialize(loaded[0].3.clone()).unwrap().len(), 31);
    assert_eq!(deserialize(loaded[0].2.clone()).unwrap()[0], 1.0);
    assert_eq!(deserialize(loaded[0].3.clone()).unwrap()[0], 2.0);
}

#[test]
fn test_save_and_load_eq_preset_with_none_bands() {
    let db = Database::new_memory().unwrap();
    db.save_eq_preset_raw("No Bands", 0.0, None, None).unwrap();

    let loaded = db.load_eq_presets_raw().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].0, "No Bands");
    assert!(loaded[0].2.is_none());
    assert!(loaded[0].3.is_none());
}

#[test]
fn test_delete_eq_preset() {
    let db = Database::new_memory().unwrap();
    db.save_eq_preset_raw("To Delete", 0.0, None, None).unwrap();
    db.save_eq_preset_raw("Keep", 0.0, None, None).unwrap();
    assert_eq!(db.load_eq_presets_raw().unwrap().len(), 2);

    db.delete_eq_preset_by_name("To Delete").unwrap();
    let loaded = db.load_eq_presets_raw().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].0, "Keep");
}

#[test]
fn test_clear_eq_presets() {
    let db = Database::new_memory().unwrap();
    db.save_eq_preset_raw("A", 0.0, None, None).unwrap();
    db.save_eq_preset_raw("B", 0.0, None, None).unwrap();
    assert_eq!(db.load_eq_presets_raw().unwrap().len(), 2);

    db.clear_all_eq_presets().unwrap();
    assert!(db.load_eq_presets_raw().unwrap().is_empty());
}

#[test]
fn test_insert_or_replace_eq_preset() {
    let db = Database::new_memory().unwrap();
    db.save_eq_preset_raw("Same", 1.0, None, None).unwrap();
    db.save_eq_preset_raw("Same", 5.0, None, None).unwrap();

    let loaded = db.load_eq_presets_raw().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].1, 5.0);
}
