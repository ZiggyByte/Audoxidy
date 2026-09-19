//! Settings service: audio settings load/persist, EQ preset CRUD.
//!
//! All methods follow the acquire→copy→release pattern:
//! they acquire the database lock, read/write data, release the lock,
//! and return plain data. No lock is held across audio manager calls.

use std::sync::Mutex;

use crate::db::Database;

use super::Core;
use super::error::CoreError;
use super::types::EqPresetSnapshot;

// ── BLOB serialization helpers ─────────────────────────────────
// These are pure functions for converting between f32 slices and
// little-endian byte blobs. They live here to avoid the audio type
// dependency at the Core boundary.

fn serialize_f32_blob(bands: Option<&[f32]>) -> Option<Vec<u8>> {
    bands.map(|v| v.iter().flat_map(|f| f.to_le_bytes()).collect())
}

fn deserialize_f32_blob(blob: Option<Vec<u8>>) -> Option<Vec<f32>> {
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

impl Core {
    /// Loads all audio settings into a snapshot.
    ///
    /// Acquires the DB lock, reads all relevant keys, releases the lock,
    /// and returns the plain-data snapshot.
    pub fn load_audio_settings(&self) -> super::types::AudioSettingsSnapshot {
        use super::types::AudioSettingsSnapshot;

        let db = self.database.lock().expect("database lock poisoned");
        AudioSettingsSnapshot {
            eq_preset_name: db.get_setting("eq_preset_name"),
            preamp_gain: db.get_setting("preamp_gain")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            compressor_threshold: db.get_setting("compressor_threshold")
                .and_then(|v| v.parse().ok())
                .unwrap_or(-20.0),
            compressor_ratio: db.get_setting("compressor_ratio")
                .and_then(|v| v.parse().ok())
                .unwrap_or(4.0),
            compressor_attack_ms: db.get_setting("compressor_attack_ms")
                .and_then(|v| v.parse().ok())
                .unwrap_or(10.0),
            compressor_release_ms: db.get_setting("compressor_release_ms")
                .and_then(|v| v.parse().ok())
                .unwrap_or(100.0),
            reverb_room_size: db.get_setting("reverb_room_size")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.5),
            reverb_damping: db.get_setting("reverb_damping")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.5),
            reverb_width: db.get_setting("reverb_width")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            reverb_mix: db.get_setting("reverb_mix")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.3),
            volume: db.get_setting("volume")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.8),
            fade_in_ms: db.get_setting("fade_in_ms")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            fade_out_ms: db.get_setting("fade_out_ms")
                .and_then(|v| v.parse().ok())
                .unwrap_or(2000),
            crossfade_enabled: db.get_setting("crossfade_enabled")
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            crossfade_manual_enabled: db.get_setting("crossfade_manual_enabled")
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            crossfade_manual_ms: db.get_setting("crossfade_manual_ms")
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000),
            crossfade_auto_enabled: db.get_setting("crossfade_auto_enabled")
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            crossfade_auto_ms: db.get_setting("crossfade_auto_ms")
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000),
        }
    }

    /// Persists a single audio setting.
    pub fn persist_audio_setting(
        &self,
        key: &str,
        value: &str,
    ) -> Result<(), CoreError> {
        let db = self.database.lock().expect("database lock poisoned");
        db.set_setting(key, value)?;
        Ok(())
    }

    /// Loads DSP effect parameters into a snapshot.
    pub fn load_dsp_effects(&self) -> super::types::DspEffectsSnapshot {
        use super::types::DspEffectsSnapshot;

        let db = self.database.lock().expect("database lock poisoned");
        DspEffectsSnapshot {
            compressor_threshold: db.get_setting("compressor_threshold")
                .and_then(|v| v.parse().ok())
                .unwrap_or(-20.0),
            compressor_ratio: db.get_setting("compressor_ratio")
                .and_then(|v| v.parse().ok())
                .unwrap_or(4.0),
            compressor_attack_ms: db.get_setting("compressor_attack_ms")
                .and_then(|v| v.parse().ok())
                .unwrap_or(10.0),
            compressor_release_ms: db.get_setting("compressor_release_ms")
                .and_then(|v| v.parse().ok())
                .unwrap_or(100.0),
            reverb_room_size: db.get_setting("reverb_room_size")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.5),
            reverb_damping: db.get_setting("reverb_damping")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.5),
            reverb_width: db.get_setting("reverb_width")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            reverb_mix: db.get_setting("reverb_mix")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.3),
            sub_bass_cutoff: db.get_setting("sub_bass_cutoff")
                .and_then(|v| v.parse().ok())
                .unwrap_or(80.0),
            sub_bass_gain: db.get_setting("sub_bass_gain")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            mid_bass_cutoff: db.get_setting("mid_bass_cutoff")
                .and_then(|v| v.parse().ok())
                .unwrap_or(200.0),
            mid_bass_gain: db.get_setting("mid_bass_gain")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            voice_boost_cutoff: db.get_setting("voice_boost_cutoff")
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000.0),
            voice_boost_gain: db.get_setting("voice_boost_gain")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
        }
    }

    /// Persists a single DSP effect parameter.
    pub fn persist_dsp_effect(
        &self,
        key: &str,
        value: &str,
    ) -> Result<(), CoreError> {
        let db = self.database.lock().expect("database lock poisoned");
        db.set_setting(key, value)?;
        Ok(())
    }

    /// Loads all EQ presets as plain-data snapshots.
    ///
    /// Acquires the DB lock, reads all preset rows using raw f32 blob
    /// deserialization, releases the lock, and returns the snapshots.
    pub fn load_eq_presets(&self) -> Vec<EqPresetSnapshot> {
        let db = self.database.lock().expect("database lock poisoned");
        let raw = db.load_eq_presets_raw().expect("failed to load eq presets");

        raw.into_iter()
            .map(|(name, preamp_gain, bands_20_blob, bands_31_blob)| {
                EqPresetSnapshot {
                    name,
                    preamp_gain,
                    bands_20: deserialize_f32_blob(bands_20_blob),
                    bands_31: deserialize_f32_blob(bands_31_blob),
                }
            })
            .collect()
    }

    /// Saves or overwrites an EQ preset.
    pub fn save_eq_preset(
        &self,
        name: &str,
        preamp_gain: f32,
        bands_20: Option<&[f32]>,
        bands_31: Option<&[f32]>,
    ) -> Result<(), CoreError> {
        let bands_20_bytes = serialize_f32_blob(bands_20);
        let bands_31_bytes = serialize_f32_blob(bands_31);

        let db = self.database.lock().expect("database lock poisoned");
        db.save_eq_preset_raw(name, preamp_gain, bands_20_bytes, bands_31_bytes)?;
        Ok(())
    }

    /// Deletes an EQ preset by name.
    pub fn delete_eq_preset(&self, name: &str) -> Result<(), CoreError> {
        let db = self.database.lock().expect("database lock poisoned");
        db.delete_eq_preset_by_name(name)?;
        Ok(())
    }

    /// Deletes all EQ presets.
    pub fn clear_eq_presets(&self) -> Result<(), CoreError> {
        let db = self.database.lock().expect("database lock poisoned");
        db.clear_all_eq_presets()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use std::sync::Arc;

    fn make_core() -> Core {
        let db = Database::new_memory().unwrap();
        let database = Arc::new(Mutex::new(db));
        let scanner = Arc::new(crate::db::scanner::Scanner::new(
            Arc::new(std::sync::Mutex::new(Database::default())),
        ));
        let audio_manager = Arc::new(
            crate::audio::AudioManager::new().expect("audio manager init"),
        );
        let paths = super::super::types::CorePaths::desktop_default();
        Core::new(audio_manager, database, scanner, paths)
    }

    #[test]
    fn test_settings_load_persist_round_trip() {
        let core = make_core();

        // Persist a setting
        core.persist_audio_setting("test_key", "test_value").unwrap();

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
}
