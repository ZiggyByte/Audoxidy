//! Serialization and band-set conversion tests for the equalizer preset.
//!
//! Each test builds a preset or a JSON document in memory, runs it through the
//! serialization helpers and asserts the band sets the importer and the exporter
//! produce. Nothing here touches the filesystem: no preset file is read or written, so
//! the round trip is pinned without depending on a preset directory.

use super::*;

#[test]
fn test_preset_to_json_round_trip() {
    let preset = EqPreset::new(
        "Test",
        1.5,
        Some(vec![
            -3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 2.0, 1.0, 0.0, -1.0, -2.0, -3.0, -2.0, -1.0, 0.0,
            1.0, 2.0, 3.0, 4.0,
        ]),
        None, // Only 20-band set — conversion will happen on import
    );

    let json = preset_to_json(&preset).unwrap();
    let parsed = preset_from_json(&json).unwrap();

    assert_eq!(parsed.name, "Test");
    assert!((parsed.preamp_gain - 1.5).abs() < 0.001);
    // Both band sets should be present after serialization
    assert!(parsed.bands_20.is_some());
    assert!(parsed.bands_31.is_some());
    // bands_20 should match exactly
    let original_20 = preset.get_gains_20();
    let parsed_20 = parsed.get_gains_20();
    assert_eq!(original_20.len(), parsed_20.len());
    for (a, b) in original_20.iter().zip(parsed_20.iter()) {
        assert!((a - b).abs() < 0.001);
    }
}

#[test]
fn test_import_single_band_set_conversion() {
    // Import a file with only bands_20
    let json = serde_json::json!({
        "version": 1,
        "name": "Converted",
        "preamp_gain": 0.0,
        "bands_20": [1.0, 1.5, 2.0, 2.5, 3.0, 2.5, 2.0, 1.5, 1.0, 0.5,
                     0.0, -0.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5],
        "bands_31": null
    })
    .to_string();

    let preset = preset_from_json(&json).unwrap();
    assert_eq!(preset.name, "Converted");
    assert!(preset.bands_20.is_some());
    assert!(preset.bands_31.is_some()); // Should be auto-generated
    assert_eq!(preset.bands_20.as_ref().unwrap().len(), 20);
    assert_eq!(preset.bands_31.as_ref().unwrap().len(), 31);
}

#[test]
fn test_import_with_both_bands() {
    let json = serde_json::json!({
        "version": 1,
        "name": "Full Preset",
        "preamp_gain": -2.0,
        "bands_20": vec![1.0; 20],
        "bands_31": vec![2.0; 31]
    })
    .to_string();

    let preset = preset_from_json(&json).unwrap();
    assert_eq!(preset.name, "Full Preset");
    assert_eq!(preset.bands_20.as_ref().unwrap().len(), 20);
    assert_eq!(preset.bands_31.as_ref().unwrap().len(), 31);
    assert!((preset.bands_20.as_ref().unwrap()[0] - 1.0).abs() < 0.001);
    assert!((preset.bands_31.as_ref().unwrap()[0] - 2.0).abs() < 0.001);
}

#[test]
fn test_json_contains_both_band_sets_on_export() {
    let preset = EqPreset::new("ExportTest", 0.0, Some(vec![0.5; 20]), None);
    let json = preset_to_json(&preset).unwrap();
    let parsed: EqPresetFile = serde_json::from_str(&json).unwrap();

    // Both should be present in the JSON file
    assert!(parsed.bands_20.is_some());
    assert!(parsed.bands_31.is_some()); // Auto-converted on export
    assert_eq!(parsed.version, 1);
}
