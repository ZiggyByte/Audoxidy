//! Settings service: audio settings load/persist, EQ preset CRUD.
//!
//! All methods follow the acquire→copy→release pattern:
//! they acquire the database lock, read/write data, release the lock,
//! and return plain data. No lock is held across audio manager calls.

use crate::utils::settings_keys::{
    KEY_COMPRESSOR_ATTACK_MS, KEY_COMPRESSOR_RATIO, KEY_COMPRESSOR_RELEASE_MS,
    KEY_COMPRESSOR_THRESHOLD, KEY_CROSSFADE_AUTO_ENABLED, KEY_CROSSFADE_AUTO_MS,
    KEY_CROSSFADE_ENABLED, KEY_CROSSFADE_MANUAL_ENABLED, KEY_CROSSFADE_MANUAL_MS,
    KEY_EQ_PRESET_NAME, KEY_FADE_IN_MS, KEY_FADE_OUT_MS, KEY_MID_BASS_CUTOFF, KEY_MID_BASS_GAIN,
    KEY_PREAMP_GAIN, KEY_REVERB_DAMPING, KEY_REVERB_MIX, KEY_REVERB_ROOM_SIZE, KEY_REVERB_WIDTH,
    KEY_SUB_BASS_CUTOFF, KEY_SUB_BASS_GAIN, KEY_VOICE_BOOST_CUTOFF, KEY_VOICE_BOOST_GAIN,
    KEY_VOLUME,
};

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
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| f32::from_le_bytes(*c))
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
            eq_preset_name: db.get_setting(KEY_EQ_PRESET_NAME),
            preamp_gain: db
                .get_setting(KEY_PREAMP_GAIN)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            compressor_threshold: db
                .get_setting(KEY_COMPRESSOR_THRESHOLD)
                .and_then(|v| v.parse().ok())
                .unwrap_or(-20.0),
            compressor_ratio: db
                .get_setting(KEY_COMPRESSOR_RATIO)
                .and_then(|v| v.parse().ok())
                .unwrap_or(4.0),
            compressor_attack_ms: db
                .get_setting(KEY_COMPRESSOR_ATTACK_MS)
                .and_then(|v| v.parse().ok())
                .unwrap_or(10.0),
            compressor_release_ms: db
                .get_setting(KEY_COMPRESSOR_RELEASE_MS)
                .and_then(|v| v.parse().ok())
                .unwrap_or(100.0),
            reverb_room_size: db
                .get_setting(KEY_REVERB_ROOM_SIZE)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.5),
            reverb_damping: db
                .get_setting(KEY_REVERB_DAMPING)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.5),
            reverb_width: db
                .get_setting(KEY_REVERB_WIDTH)
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            reverb_mix: db
                .get_setting(KEY_REVERB_MIX)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.3),
            volume: db
                .get_setting(KEY_VOLUME)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.8),
            fade_in_ms: db
                .get_setting(KEY_FADE_IN_MS)
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            fade_out_ms: db
                .get_setting(KEY_FADE_OUT_MS)
                .and_then(|v| v.parse().ok())
                .unwrap_or(2000),
            crossfade_enabled: db
                .get_setting(KEY_CROSSFADE_ENABLED)
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            crossfade_manual_enabled: db
                .get_setting(KEY_CROSSFADE_MANUAL_ENABLED)
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            crossfade_manual_ms: db
                .get_setting(KEY_CROSSFADE_MANUAL_MS)
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000),
            crossfade_auto_enabled: db
                .get_setting(KEY_CROSSFADE_AUTO_ENABLED)
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            crossfade_auto_ms: db
                .get_setting(KEY_CROSSFADE_AUTO_MS)
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000),
        }
    }

    /// Persists a single audio setting.
    pub fn persist_audio_setting(&self, key: &str, value: &str) -> Result<(), CoreError> {
        let db = self.database.lock().expect("database lock poisoned");
        db.set_setting(key, value)?;
        Ok(())
    }

    /// Loads DSP effect parameters into a snapshot.
    pub fn load_dsp_effects(&self) -> super::types::DspEffectsSnapshot {
        use super::types::DspEffectsSnapshot;

        let db = self.database.lock().expect("database lock poisoned");
        DspEffectsSnapshot {
            compressor_threshold: db
                .get_setting(KEY_COMPRESSOR_THRESHOLD)
                .and_then(|v| v.parse().ok())
                .unwrap_or(-20.0),
            compressor_ratio: db
                .get_setting(KEY_COMPRESSOR_RATIO)
                .and_then(|v| v.parse().ok())
                .unwrap_or(4.0),
            compressor_attack_ms: db
                .get_setting(KEY_COMPRESSOR_ATTACK_MS)
                .and_then(|v| v.parse().ok())
                .unwrap_or(10.0),
            compressor_release_ms: db
                .get_setting(KEY_COMPRESSOR_RELEASE_MS)
                .and_then(|v| v.parse().ok())
                .unwrap_or(100.0),
            reverb_room_size: db
                .get_setting(KEY_REVERB_ROOM_SIZE)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.5),
            reverb_damping: db
                .get_setting(KEY_REVERB_DAMPING)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.5),
            reverb_width: db
                .get_setting(KEY_REVERB_WIDTH)
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            reverb_mix: db
                .get_setting(KEY_REVERB_MIX)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.3),
            sub_bass_cutoff: db
                .get_setting(KEY_SUB_BASS_CUTOFF)
                .and_then(|v| v.parse().ok())
                .unwrap_or(80.0),
            sub_bass_gain: db
                .get_setting(KEY_SUB_BASS_GAIN)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            mid_bass_cutoff: db
                .get_setting(KEY_MID_BASS_CUTOFF)
                .and_then(|v| v.parse().ok())
                .unwrap_or(200.0),
            mid_bass_gain: db
                .get_setting(KEY_MID_BASS_GAIN)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            voice_boost_cutoff: db
                .get_setting(KEY_VOICE_BOOST_CUTOFF)
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000.0),
            voice_boost_gain: db
                .get_setting(KEY_VOICE_BOOST_GAIN)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
        }
    }

    /// Persists a single DSP effect parameter.
    pub fn persist_dsp_effect(&self, key: &str, value: &str) -> Result<(), CoreError> {
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
            .map(
                |(name, preamp_gain, bands_20_blob, bands_31_blob)| EqPresetSnapshot {
                    name,
                    preamp_gain,
                    bands_20: deserialize_f32_blob(bands_20_blob),
                    bands_31: deserialize_f32_blob(bands_31_blob),
                },
            )
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
#[path = "settings_tests.rs"]
mod tests;
