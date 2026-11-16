//! `AudioState` and `AudioSettings` defaults, cloning and error text.
//!
//! Pins the values a fresh state starts from — volume, fades, silence removal,
//! replay gain, crossfade — and the Spanish message every `AudioError` variant
//! renders. No audio device is opened, no playback happens and no database is
//! touched.

use crate::audio::AudioError;
use crate::audio::engine::{AudioSettings, AudioState, ChannelConfig};

// --- AudioState ---

#[test]
fn test_audio_state_defaults() {
    let state = AudioState::default();
    assert!(!state.is_playing);
    assert!((state.volume - 0.3).abs() < f64::EPSILON as f32);
    assert_eq!(state.sample_rate, 48000);
    assert_eq!(state.channels, 2);
    assert_eq!(state.current_pos_sec, 0.0);
    assert_eq!(state.total_duration_sec, 0.0);
    assert_eq!(state.title, "Sin título");
    assert_eq!(state.artist, "Artista desconocido");
    assert!(state.album.is_empty());
    assert!(state.cover_path.is_none());
    assert!(state.path.is_empty());
    assert!(!state.eof_reached);
    assert_eq!(state.replay_gain_track_enabled, true);
    assert_eq!(state.replay_gain_album_enabled, true);

    // Volumen y Mezcla — Fades
    assert_eq!(state.fades_enabled, false);
    assert_eq!(state.smooth_volume_enabled, false);
    assert!((state.fade_in_ms - 1000.0).abs() < f64::EPSILON as f32);
    assert!((state.fade_out_ms - 2000.0).abs() < f64::EPSILON as f32);
    assert_eq!(state.fade_in_enabled, false);
    assert_eq!(state.fade_out_enabled, false);

    // Volumen y Mezcla — Silence removal
    assert_eq!(state.silence_enabled, true);
    assert_eq!(state.silence_edge_trim_enabled, true);
    assert!((state.silence_duration_ms - 1000.0).abs() < f64::EPSILON as f32);
    assert!((state.silence_threshold_db - (-47.0)).abs() < f64::EPSILON as f32);

    // Volumen y Mezcla — Replay gain fijo (reemplaza Normalización, UAT)
    assert_eq!(state.rg_fixed_enabled, false);
    assert!((state.rg_fixed_db - 0.0).abs() < f64::EPSILON as f32);

    // Volumen y Mezcla — ReplayGain offsets
    assert_eq!(state.rg_master_enabled, true);
    assert!((state.rg_offset_album_db - 0.0).abs() < f64::EPSILON as f32);
    assert!((state.rg_offset_track_db - 0.0).abs() < f64::EPSILON as f32);
    assert!((state.rg_offset_rt_db - 0.0).abs() < f64::EPSILON as f32);
    assert_eq!(state.rg_analyze_rt_enabled, true);
}

#[test]
fn test_audio_state_clone_preserves_album_cover() {
    let mut state = AudioState::default();
    state.album = "Test Album".to_string();
    state.cover_path = Some("cache/covers/test.avif".to_string());

    let cloned = state.clone();
    assert_eq!(cloned.album, state.album);
    assert_eq!(cloned.cover_path, state.cover_path);
}

// --- Volumen y Mezcla: AudioState defaults ---

#[test]
fn test_audio_state_volumen_defaults() {
    let state = AudioState::default();

    // Fades
    assert_eq!(state.fades_enabled, false);
    assert_eq!(state.smooth_volume_enabled, false);
    assert!((state.fade_in_ms - 1000.0).abs() < f32::EPSILON);
    assert!((state.fade_out_ms - 2000.0).abs() < f32::EPSILON);
    assert_eq!(state.fade_in_enabled, false);
    assert_eq!(state.fade_out_enabled, false);

    // Silence removal
    assert_eq!(state.silence_enabled, true);
    assert_eq!(state.silence_edge_trim_enabled, true);
    assert!((state.silence_duration_ms - 1000.0).abs() < f32::EPSILON);
    assert!((state.silence_threshold_db - (-47.0)).abs() < f32::EPSILON);

    // Replay gain fijo (reemplaza Normalización, UAT)
    assert_eq!(state.rg_fixed_enabled, false);
    assert!((state.rg_fixed_db - 0.0).abs() < f32::EPSILON);

    // ReplayGain offsets
    assert_eq!(state.rg_master_enabled, true);
    assert!((state.rg_offset_album_db - 0.0).abs() < f32::EPSILON);
    assert!((state.rg_offset_track_db - 0.0).abs() < f32::EPSILON);
    assert!((state.rg_offset_rt_db - 0.0).abs() < f32::EPSILON);
    assert_eq!(state.rg_analyze_rt_enabled, true);

    // Existing replay_gain fields still true
    assert_eq!(state.replay_gain_track_enabled, true);
    assert_eq!(state.replay_gain_album_enabled, true);
}

// --- Mezcla Cruzada / Crossfade: AudioState defaults ---

#[test]
fn test_audio_state_crossfade_defaults() {
    let state = AudioState::default();

    // Master del grupo Crossfade y sus sub-funciones.
    assert_eq!(state.crossfade_enabled, false);
    assert_eq!(state.crossfade_manual_enabled, false);
    assert_eq!(state.crossfade_auto_enabled, false);
    assert!((state.crossfade_manual_ms - 1000.0).abs() < f32::EPSILON);
    assert!((state.crossfade_auto_ms - 250.0).abs() < f32::EPSILON);
}

#[test]
fn test_audio_state_clone() {
    let mut state = AudioState::default();
    state.title = "Test Song".to_string();
    state.is_playing = true;
    let cloned = state.clone();
    assert_eq!(cloned.title, "Test Song");
    assert!(cloned.is_playing);
}

// --- AudioSettings ---

#[test]
fn test_audio_settings_default() {
    let settings = AudioSettings::default();
    assert!(settings.host_id.is_none());
    assert!(settings.device_name.is_none());
    assert!(settings.sample_rate.is_none());
    assert!(settings.bit_depth.is_none());
    assert_eq!(settings.channels, ChannelConfig::Manual(2));
}

// --- AudioError ---

#[test]
fn test_audio_error_display_no_device() {
    let err = AudioError::NoDevice;
    let msg = format!("{err}");
    assert!(msg.contains("No audio device"));
}

#[test]
fn test_audio_error_display_unsupported_format() {
    let err = AudioError::UnsupportedSampleFormat {
        device: "DAC".into(),
        format: "I24".into(),
    };
    let msg = format!("{err}");
    assert!(msg.contains("Formato de muestra no soportado"));
    assert!(msg.contains("DAC"));
    assert!(msg.contains("I24"));
}

#[test]
fn test_audio_error_display_unsupported_file_format() {
    let err = AudioError::UnsupportedFormat("contenedor desconocido".into());
    let msg = format!("{err}");
    assert!(msg.contains("Formato de audio no soportado"));
    assert!(msg.contains("contenedor desconocido"));
}

#[test]
fn test_audio_error_display_host_not_found() {
    let err = AudioError::HostNotFound;
    let msg = format!("{err}");
    assert!(msg.contains("Host no encontrado"));
}

#[test]
fn test_audio_error_display_device_not_found() {
    let err = AudioError::DeviceNotFound;
    let msg = format!("{err}");
    assert!(msg.contains("Dispositivo no encontrado"));
}

#[test]
fn test_audio_error_display_no_active_output() {
    let err = AudioError::NoActiveOutput;
    let msg = format!("{err}");
    assert!(msg.contains("No hay una salida de audio activa"));
}

#[test]
fn test_audio_error_display_config() {
    let err = AudioError::ConfigError("test config".into());
    let msg = format!("{err}");
    assert!(msg.contains("test config"));
}

#[test]
fn test_audio_error_display_stream() {
    let err = AudioError::StreamError("test stream".into());
    let msg = format!("{err}");
    assert!(msg.contains("test stream"));
}

#[test]
fn test_audio_error_display_device() {
    let err = AudioError::DeviceError("test device".into());
    let msg = format!("{err}");
    assert!(msg.contains("test device"));
}

#[test]
fn test_audio_error_display_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err = AudioError::IoError(io_err);
    let msg = format!("{err}");
    assert!(msg.contains("file not found"));
}

#[test]
fn test_audio_error_io_source() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
    let err = AudioError::IoError(io_err);
    let source = std::error::Error::source(&err);
    assert!(source.is_some());
}
