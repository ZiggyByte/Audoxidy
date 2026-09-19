//! Audio routing service: device settings, EQ preset apply, system audio config.
//!
//! All methods delegate to [`AudioManager`] with correct lock order. No lock
//! is held across audio manager calls. The EQ preset apply acquires the DB
//! lock to load the preset, releases it, then applies to the audio manager.

use super::Core;
use super::error::CoreError;
use super::types::AudioSettingsSnapshot;

impl Core {
    /// Returns the list of available audio hosts (ALSA, PipeWire, etc.).
    pub fn get_available_hosts(&self) -> Vec<String> {
        self.audio_manager.get_available_hosts()
    }

    /// Returns the list of available output devices.
    pub fn get_available_devices(&self) -> Vec<crate::audio::device_manager::AudioDeviceInfo> {
        self.audio_manager.get_devices()
    }

    /// Applies a full audio settings snapshot to the audio engine.
    ///
    /// Converts the snapshot into the engine's [`AudioSettings`] and delegates.
    /// The DB lock is never held across the audio manager call.
    pub fn apply_audio_settings(&self, snapshot: &AudioSettingsSnapshot) -> Result<(), CoreError> {
        // Map snapshot fields to the engine's AudioSettings.
        // The snapshot carries user-level volume and DSP parameters;
        // the AudioSettings struct carries device/routing configuration.
        // For a full settings apply, the caller should first persist the
        // values via `persist_audio_setting`, then apply device routing
        // via `apply_device_settings`.
        //
        // Volume and DSP parameters are applied directly to the audio manager.
        self.audio_manager.set_volume(snapshot.volume as f32);
        self.audio_manager
            .set_preamp_gain(snapshot.preamp_gain as f32);
        self.audio_manager.set_compressor_params(
            snapshot.compressor_threshold as f32,
            snapshot.compressor_ratio as f32,
            snapshot.compressor_attack_ms as f32,
            snapshot.compressor_release_ms as f32,
        );
        self.audio_manager
            .set_reverb_room_size(snapshot.reverb_room_size as f32);
        self.audio_manager
            .set_reverb_damping(snapshot.reverb_damping as f32);
        self.audio_manager
            .set_reverb_wet(snapshot.reverb_mix as f32);
        self.audio_manager
            .set_reverb_dry(1.0 - snapshot.reverb_mix as f32);

        Ok(())
    }

    /// Loads an EQ preset by name from the database and applies it.
    ///
    /// Acquires the DB lock, loads the preset, releases the lock, then
    /// applies the gains to the audio engine. The DB lock is never held
    /// across the audio manager call.
    pub fn apply_eq_preset(&self, name: &str) -> Result<(), CoreError> {
        // Acquire→copy→release: load preset from DB.
        let preset = {
            let db = self.database.lock().expect("database lock poisoned");
            let raw = db.load_eq_presets_raw().map_err(CoreError::Database)?;
            raw.into_iter()
                .find(|(n, _, _, _)| n == name)
                .ok_or_else(|| CoreError::NotFound(name.to_string()))
        }?;

        // Unpack the raw preset.
        let (_name, preamp_gain, bands_20_blob, bands_31_blob) = preset;

        // Deserialize f32 blobs.
        let bands_20 = deserialize_f32_blob(bands_20_blob);
        let bands_31 = deserialize_f32_blob(bands_31_blob);

        // Apply to audio engine (lock released before this call).
        self.audio_manager.set_preamp_gain(preamp_gain);
        if let (Some(b20), Some(b31)) = (&bands_20, &bands_31) {
            self.audio_manager.apply_eq_preset_gains(b20, b31);
        }

        Ok(())
    }

    /// Applies system audio configuration (sample rate, buffer quantum).
    ///
    /// Rebuilds the audio stream with the new parameters. If both parameters
    /// are `None`, this is a no-op.
    pub fn apply_system_audio_config(
        &self,
        rate: Option<u32>,
        quantum: Option<u32>,
    ) -> Result<(), CoreError> {
        if rate.is_none() && quantum.is_none() {
            return Ok(());
        }

        // Read current device settings from the DB.
        let device_settings = {
            let db = self.database.lock().expect("database lock poisoned");
            let host_id = db.get_setting("audio_host");
            let device_name = db.get_setting("audio_device");
            let device_id = db.get_setting("audio_device_id");
            let bit_depth_str = db.get_setting("audio_bit_depth");
            let channels_str = db.get_setting("audio_channels");

            let bit_depth = bit_depth_str.and_then(|s| match s.as_str() {
                "16" => Some(crate::audio::device_manager::BitDepth::Bits16),
                "24" => Some(crate::audio::device_manager::BitDepth::Bits24),
                "32" => Some(crate::audio::device_manager::BitDepth::Bits32Float),
                _ => None,
            });

            let channels = channels_str.and_then(|s| {
                s.parse::<u16>()
                    .ok()
                    .map(crate::audio::device_manager::ChannelConfig::Manual)
            });

            crate::audio::device_manager::AudioSettings {
                host_id,
                device_name,
                device_id,
                sample_rate: rate,
                bit_depth,
                channels: channels.unwrap_or_default(),
                buffer_size: quantum,
            }
        };

        self.audio_manager
            .apply_audio_settings(device_settings)
            .map_err(|e| CoreError::Audio(e.to_string()))
    }
}

/// Deserializes a little-endian f32 byte blob into a `Vec<f32>`.
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

#[cfg(test)]
mod tests {
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
        let audio_manager =
            Arc::new(crate::audio::AudioManager::new().expect("audio manager init"));
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
}
