//! Shared domain types for the Core layer.
//!
//! These types are passed between Core services and returned to the GUI
//! as plain data. They carry no iced or GUI types.

use std::path::PathBuf;

/// Filesystem locations that Core needs.
///
/// Injected at construction so a future terminal or mobile front-end can
/// supply its own locations. The desktop default preserves the current
/// relative-path behavior.
#[derive(Debug, Clone)]
pub struct CorePaths {
    /// Path to the library SQLite database.
    pub db_path: PathBuf,
    /// Directory where cover art is cached.
    pub cover_cache_dir: PathBuf,
    /// Path to the application configuration file.
    pub config_path: PathBuf,
}

impl CorePaths {
    /// Returns the desktop default paths that match the current behavior.
    ///
    /// The defaults are single-sourced from `utils::paths` so the registry and
    /// the injected Core paths can never disagree.
    pub fn desktop_default() -> Self {
        let paths = crate::utils::paths::AppPaths::desktop_default();

        Self {
            db_path: paths.db_path,
            cover_cache_dir: paths.cover_cache_dir,
            config_path: paths.config_path,
        }
    }
}

/// Snapshot of audio settings for applying to the engine.
///
/// Carries all fields needed to apply audio settings without holding
/// a database lock across the audio manager call.
#[derive(Debug, Clone, Default)]
pub struct AudioSettingsSnapshot {
    pub eq_preset_name: Option<String>,
    pub preamp_gain: f64,
    pub compressor_threshold: f64,
    pub compressor_ratio: f64,
    pub compressor_attack_ms: f64,
    pub compressor_release_ms: f64,
    pub reverb_room_size: f64,
    pub reverb_damping: f64,
    pub reverb_width: f64,
    pub reverb_mix: f64,
    pub volume: f64,
    pub fade_in_ms: u64,
    pub fade_out_ms: u64,
    pub crossfade_enabled: bool,
    pub crossfade_manual_enabled: bool,
    pub crossfade_manual_ms: u64,
    pub crossfade_auto_enabled: bool,
    pub crossfade_auto_ms: u64,
}

/// Snapshot of DSP effect parameters.
#[derive(Debug, Clone, Default)]
pub struct DspEffectsSnapshot {
    pub compressor_threshold: f64,
    pub compressor_ratio: f64,
    pub compressor_attack_ms: f64,
    pub compressor_release_ms: f64,
    pub reverb_room_size: f64,
    pub reverb_damping: f64,
    pub reverb_width: f64,
    pub reverb_mix: f64,
    pub sub_bass_cutoff: f64,
    pub sub_bass_gain: f64,
    pub mid_bass_cutoff: f64,
    pub mid_bass_gain: f64,
    pub voice_boost_cutoff: f64,
    pub voice_boost_gain: f64,
}

/// High-level library statistics.
#[derive(Debug, Clone, Default)]
pub struct LibraryStats {
    pub total_songs: usize,
    pub total_albums: usize,
    pub total_artists: usize,
    pub total_duration_secs: f64,
    pub total_size_bytes: u64,
}

/// Plain-data version of an EQ preset, replacing `audio::preset::EqPreset`
/// at the Core boundary to remove the cross-layer dependency.
#[derive(Debug, Clone)]
pub struct EqPresetSnapshot {
    pub name: String,
    pub preamp_gain: f32,
    pub bands_20: Option<Vec<f32>>,
    pub bands_31: Option<Vec<f32>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_paths_desktop_default() {
        let paths = CorePaths::desktop_default();
        assert_eq!(paths.db_path, PathBuf::from("library.db"));
        assert_eq!(paths.cover_cache_dir, PathBuf::from("cache/covers"));
        assert!(paths.config_path.to_string_lossy().contains("audoxidy"));
        assert!(paths.config_path.to_string_lossy().ends_with("config.ron"));
    }

    #[test]
    fn test_core_paths_clone() {
        let paths = CorePaths::desktop_default();
        let cloned = paths.clone();
        assert_eq!(paths.db_path, cloned.db_path);
        assert_eq!(paths.cover_cache_dir, cloned.cover_cache_dir);
        assert_eq!(paths.config_path, cloned.config_path);
    }
}
