//! Constructor and initial load for `AudoxidyApp`.

use crate::core::Core;
use crate::gui::audio_center::AudioCenterManager;
use crate::gui::library::LibraryManager;
use crate::gui::library_filters::LibraryFiltersManager;
use crate::gui::playlist::PlaylistManager;
use crate::integrations::control::external_control_channel;
use crate::integrations::media_controls::SystemMediaControls;
use iced::Task;
use std::sync::Arc;

use super::state::AudoxidyApp;
use super::{ActiveDialog, AppFocus, Message};

/// Persisted audio-engine and PipeWire selection read under one database guard
/// and applied once the guard is released.
struct StartupAudioConfig {
    /// Whether the stable device id should be backfilled after applying.
    had_device_id: bool,
    /// The engine settings to apply, or `None` when nothing was persisted.
    settings: Option<crate::audio::engine::AudioSettings>,
    /// Resolved PipeWire/PulseAudio clock rate, or `None` for the default.
    pipewire_rate: Option<u32>,
    /// Resolved PipeWire/PulseAudio quantum, or `None` for the default.
    pipewire_quantum: Option<u32>,
    /// Last song to restore, if any.
    restore: Option<StartupPlaylistRestore>,
}

/// Last-song state read under one database guard and handed to the audio
/// manager after the guard is released.
struct StartupPlaylistRestore {
    path: String,
    title: String,
    artist: String,
    album: String,
    cover_path: Option<String>,
    track_gain: Option<f64>,
    album_gain: Option<f64>,
    last_pos_sec: f64,
    is_playing: bool,
}

/// Persisted audio-state fields read under one database guard and written to
/// the shared audio state after the guard is released.
struct StartupAudioState {
    fades_enabled: bool,
    smooth_volume_enabled: bool,
    fade_in_ms: f32,
    fade_out_ms: f32,
    fade_in_enabled: bool,
    fade_out_enabled: bool,
    silence_enabled: bool,
    silence_duration_ms: f32,
    silence_threshold_db: f32,
    silence_edge_trim_enabled: bool,
    rg_fixed_enabled: bool,
    rg_fixed_db: f32,
    rg_master_enabled: bool,
    replay_gain_track_enabled: bool,
    replay_gain_album_enabled: bool,
    rg_analyze_rt_enabled: bool,
    rg_offset_album_db: f32,
    rg_offset_track_db: f32,
    rg_offset_rt_db: f32,
    crossfade_enabled: bool,
    crossfade_manual_enabled: bool,
    crossfade_manual_ms: f32,
    crossfade_auto_enabled: bool,
    crossfade_auto_ms: f32,
    meter_hold_time_ms: f32,
    meter_rms_window_ms: f32,
    meter_infinite_hold: bool,
    meter_enabled: bool,
}

/// Persisted EQ state, presets and selection read under one database guard.
struct StartupEq {
    eq_enabled: bool,
    eq_bands_31: bool,
    preamp_gain: f32,
    bands_20: Vec<f32>,
    bands_31: Vec<f32>,
    custom_presets: Vec<crate::audio::preset::EqPreset>,
    selected_preset_name: Option<String>,
}

/// Persisted DSP effect parameters read under one database guard.
struct StartupDsp {
    sub_bass_enabled: Option<bool>,
    mid_bass_enabled: Option<bool>,
    voice_boost_enabled: Option<bool>,
    noise_gate_enabled: Option<bool>,
    stereo_expander_enabled: Option<bool>,
    stereo_balance_enabled: Option<bool>,
    compressor_enabled: Option<bool>,
    limiter_enabled: Option<bool>,
    reverb_enabled: Option<bool>,
    sub_bass_gain: Option<f32>,
    mid_bass_gain: Option<f32>,
    voice_boost_gain: Option<f32>,
    noise_gate_threshold: Option<f32>,
    stereo_expander_width: Option<f32>,
    stereo_expander_surround: Option<bool>,
    stereo_balance_balance: Option<f32>,
    compressor_threshold: Option<f32>,
    compressor_intensity: Option<f32>,
    limiter_ceiling: Option<f32>,
    reverb_wet: Option<f32>,
    reverb_room_size: Option<f32>,
}

/// Persisted volume and crossfade settings read under one database guard.
struct StartupVolume {
    fades_enabled: Option<bool>,
    smooth_volume_enabled: Option<bool>,
    fade_in_ms: Option<f32>,
    fade_out_ms: Option<f32>,
    silence_enabled: Option<bool>,
    silence_duration_ms: Option<f32>,
    silence_threshold_db: Option<f32>,
    silence_edge_trim_enabled: Option<bool>,
    rg_master_enabled: Option<bool>,
    rg_track_enabled: Option<bool>,
    rg_album_enabled: Option<bool>,
    rg_analyze_rt_enabled: Option<bool>,
    rg_offset_album_db: Option<f32>,
    rg_offset_track_db: Option<f32>,
    rg_offset_rt_db: Option<f32>,
}

impl AudoxidyApp {
    pub fn new(core: Arc<Core>) -> (Self, Task<Message>) {
        // Inicializar temporizadores de memoria (Ciclo de 2 minutos)
        crate::utils::memory_manager::MemoryManager::init();

        let mut library_manager = LibraryManager::default();

        let (total_songs, total_albums, total_duration, total_size, total_artists) =
            core.get_library_stats_raw();
        library_manager.total_songs = total_songs;
        library_manager.total_albums = total_albums;
        library_manager.total_artists = total_artists;
        library_manager.total_duration_secs = total_duration;
        library_manager.total_size_bytes = total_size;

        if let Ok(folders) = core.get_all_folders() {
            library_manager.cached_folders = Some(folders);
        }

        // Obtener puntos de montaje para el sistema de rutas inteligente
        let mut mount_points = Vec::new();
        if let Ok(content) = std::fs::read_to_string("/proc/mounts") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let mnt = parts[1];
                    if mnt.starts_with("/mnt/") || mnt.starts_with("/media/") || mnt == "/" {
                        mount_points.push((mnt.to_string(), mnt.to_string()));
                    }
                }
            }
        }
        library_manager.mount_points = mount_points;

        // Cargar datos iniciales de forma unificada (Orden y Filtros)
        let initial_songs = core.get_all_songs_shared();
        let initial_albums_raw = core.get_albums();
        let initial_albums = initial_albums_raw
            .into_iter()
            .map(|a| crate::gui::library::AlbumEntry {
                id: a.0,
                title: a.1,
                artist: a.2,
                genre: a.3,
                year: a.4,
                cover_path: a.5,
            })
            .collect();

        library_manager.update_data(initial_songs, initial_albums);

        let (external_control_tx, external_control_rx) = external_control_channel();
        let media_controls = SystemMediaControls::new(external_control_tx)
            .expect("Error al inicializar controles multimedia del sistema");

        // CoreEvent channel: Core owns the sender; the GUI consumes the receiver.
        let core_event_rx = core.event_receiver();

        // Detección de Hardware para modo de bajos recursos (RAM < 8GB o Cores < 4)
        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();
        let total_ram_gb = sys.total_memory() / 1024 / 1024 / 1024;
        let cpu_cores = sys.cpus().len();
        let low_resource_mode = total_ram_gb < 8 || cpu_cores < 4;

        // Configurar flag global para que todos los módulos puedan consultarlo
        crate::utils::LOW_RESOURCE_MODE
            .store(low_resource_mode, std::sync::atomic::Ordering::Relaxed);

        if low_resource_mode {
            #[rustfmt::skip]
            tracing::info!("Audoxidy Performance: Low resource mode ENABLED (RAM: {}GB, Cores: {})", total_ram_gb, cpu_cores);
        }

        // Construir el índice de filtros con las canciones ya cargadas
        let mut filters_manager = LibraryFiltersManager::default();
        filters_manager.build_filter_index(core.database());

        let mut playlist_manager = PlaylistManager::default();
        // db-lock: snapshot de solo lectura; el guard se libera antes de tocar
        // el gestor de audio y de la E/S de PipeWire.
        let startup_audio = if let Ok(db_lock) = core.database().lock() {
            // 0. Cargar persistencia de ajustes de audio del reproductor.
            //    En instalación fresca (sin ajustes en DB), aplicar defaults 48000 Hz.
            let host_id = db_lock.get_setting("audio_host").filter(|s| !s.is_empty());
            let device_name = db_lock
                .get_setting("audio_device")
                .filter(|s| !s.is_empty());
            let device_id = db_lock
                .get_setting("audio_device_id")
                .filter(|s| !s.is_empty());
            let sample_rate = db_lock.get_setting("audio_sample_rate").and_then(|s| {
                if s == "auto" {
                    None
                } else {
                    s.parse::<u32>().ok()
                }
            });
            let bit_depth = db_lock
                .get_setting("audio_bit_depth")
                .and_then(|s| match s.as_str() {
                    "16" => Some(crate::audio::engine::BitDepth::Bits16),
                    "24" => Some(crate::audio::engine::BitDepth::Bits24),
                    "32" => Some(crate::audio::engine::BitDepth::Bits32Float),
                    _ => None,
                });
            let channels_val = db_lock
                .get_setting("audio_channels")
                .and_then(|s| s.parse::<u16>().ok());
            let buffer_size = db_lock.get_setting("audio_buffer_size").and_then(|s| {
                if s == "auto" {
                    None
                } else {
                    s.parse::<u32>().ok()
                }
            });
            // SIEMPRE aplicar settings: en instalación fresca usa defaults (48000 Hz,
            // 32-bit Float, 2ch), en reinstalación carga desde DB.
            // Solo aplicar si hay al menos un ajuste guardado o es reinstalación.
            let had_device_id = device_id.is_none();
            let settings = if host_id.is_some()
                || device_name.is_some()
                || device_id.is_some()
                || sample_rate.is_some()
                || bit_depth.is_some()
                || buffer_size.is_some()
            {
                Some(crate::audio::engine::AudioSettings {
                    host_id,
                    device_name,
                    device_id,
                    sample_rate, // None → auto → 48000 en resolve_settings
                    bit_depth,   // None → fallback del device
                    channels: crate::audio::engine::ChannelConfig::Manual(
                        channels_val.unwrap_or(2),
                    ),
                    buffer_size,
                })
            } else {
                None
            };

            // 0b. Resolver los ajustes de PipeWire/PulseAudio guardados.
            let parse_system_sel = |s: &str| -> crate::gui::audio_center::SystemSelection<u32> {
                match s {
                    "default" => crate::gui::audio_center::SystemSelection::Default,
                    "auto" => crate::gui::audio_center::SystemSelection::Automatic,
                    s if s.starts_with("fixed:") => s["fixed:".len()..]
                        .parse::<u32>()
                        .map(crate::gui::audio_center::SystemSelection::Fixed)
                        .unwrap_or(crate::gui::audio_center::SystemSelection::Default),
                    _ => crate::gui::audio_center::SystemSelection::Default,
                }
            };
            let sys_rate = db_lock
                .get_setting("audio_system_rate")
                .map(|s| parse_system_sel(&s))
                .unwrap_or(crate::gui::audio_center::SystemSelection::Default);
            let sys_quantum = db_lock
                .get_setting("audio_system_quantum")
                .map(|s| parse_system_sel(&s))
                .unwrap_or(crate::gui::audio_center::SystemSelection::Default);

            let pipewire_rate = match sys_rate {
                crate::gui::audio_center::SystemSelection::Default => None,
                crate::gui::audio_center::SystemSelection::Automatic => {
                    // Usar la tasa del reproductor si está configurada, sino 48000.
                    db_lock
                        .get_setting("audio_sample_rate")
                        .and_then(|s| {
                            if s == "auto" {
                                None
                            } else {
                                s.parse::<u32>().ok()
                            }
                        })
                        .or(Some(48000))
                }
                crate::gui::audio_center::SystemSelection::Fixed(r) => Some(r),
            };
            let pipewire_quantum = match sys_quantum {
                crate::gui::audio_center::SystemSelection::Default => None,
                crate::gui::audio_center::SystemSelection::Automatic => {
                    let sr = pipewire_rate.unwrap_or(48000);
                    let frames = (sr as f64 * 10.0 / 1000.0) as u32;
                    Some(if frames < 256 {
                        256
                    } else if frames < 512 {
                        512
                    } else if frames < 1024 {
                        1024
                    } else if frames < 2048 {
                        2048
                    } else if frames < 4096 {
                        4096
                    } else {
                        8192
                    })
                }
                crate::gui::audio_center::SystemSelection::Fixed(q) => Some(q),
            };

            // 1. Cargar última playlist activa
            if let Some(last_id_str) = db_lock.get_setting("last_active_playlist_id") {
                if let Ok(last_id) = last_id_str.parse::<i64>() {
                    playlist_manager.active_playlist_id = last_id;
                }
            }

            let active_id = playlist_manager.active_playlist_id;
            if let Ok(groups) = db_lock.get_playlist_songs_grouped_by_folder(active_id) {
                playlist_manager.set_groups(groups);
            }

            if let Ok(all_p) = db_lock.get_all_playlists() {
                playlist_manager.playlists = all_p;
            }

            // Cargar persistencia inicial
            let mut restore = None;
            if let Ok(Some(p_data)) = db_lock.get_playlist_by_id(active_id) {
                playlist_manager.shuffle_active = p_data.shuffle_active;
                playlist_manager.repeat_mode = p_data.repeat_mode as u8;

                if p_data.shuffle_active {
                    if let Ok(Some(mut session)) = db_lock.load_shuffle_session(active_id) {
                        // Sincronizar posición de sesión con la canción restaurada
                        if let Some(song_id) = p_data.last_song_id {
                            if let Some(pos) =
                                session.shuffle_order.iter().position(|&id| id == song_id)
                            {
                                session.current_position = pos + 1; // Apuntar a la siguiente en la cola
                                if !session.history.contains(&song_id) {
                                    session.history.push(song_id);
                                }
                            }
                        }
                        playlist_manager.shuffle_session = Some(session);
                    }
                }

                // Restaurar canción y posición (esto se carga pero el 'play' lo haremos vía Task para mayor seguridad)
                if let Some(song_id) = p_data.last_song_id {
                    if let Some(l_idx) = playlist_manager.get_linear_index_by_song_id(song_id) {
                        if let Some(song) = playlist_manager.get_song_at_linear_index(l_idx) {
                            let path = song.file_path.clone();
                            let title = song.title.clone();
                            let artist = song.artist_name.to_string();
                            let album = song.album_title.to_string();
                            let cover_path = song.cover_path.as_deref().map(str::to_string);

                            playlist_manager.playing_song_idx = Some(l_idx);
                            let (track_gain, album_gain) = db_lock
                                .get_replay_gain_by_path(&path)
                                .unwrap_or((None, None));
                            restore = Some(StartupPlaylistRestore {
                                path: path.to_string(),
                                title: title.to_string(),
                                artist,
                                album,
                                cover_path,
                                track_gain,
                                album_gain,
                                last_pos_sec: p_data.last_pos_sec,
                                is_playing: p_data.is_playing,
                            });
                        }
                    }
                }
            }

            Some(StartupAudioConfig {
                had_device_id,
                settings,
                pipewire_rate,
                pipewire_quantum,
                restore,
            })
        } else {
            tracing::warn!("db-lock: lock no disponible");
            None
        };

        // Aplicar la configuración con la base de datos ya liberada.
        if let Some(StartupAudioConfig {
            had_device_id,
            settings,
            pipewire_rate,
            pipewire_quantum,
            restore,
        }) = startup_audio
        {
            if let Some(settings) = settings {
                let apply_ok = core.audio_manager().apply_audio_settings(settings).is_ok();
                // Backfill del id estable cuando solo había coincidencia por nombre
                // heredado: así la próxima carga resuelve sin depender del nombre.
                if apply_ok && had_device_id {
                    if let Some(resolved) = core.audio_manager().get_state().device_id.clone() {
                        // db-lock: scope de escritura; el guard no cubre ninguna
                        // llamada al gestor de audio.
                        if let Ok(db_lock) = core.database().lock() {
                            match db_lock.set_setting("audio_device_id", &resolved) {
                                Ok(()) => tracing::info!(
                                    "Persistido el id estable del dispositivo de salida."
                                ),
                                Err(e) => tracing::warn!(
                                    "No se pudo persistir el id estable del dispositivo: {e}"
                                ),
                            }
                        }
                    }
                }
            }

            // 0b. Aplicar ajustes de PipeWire/PulseAudio guardados ANTES de reproducir.
            //     Si no se aplican aquí, el stream se conecta al sink con la configuración
            //     anterior y la reconfiguración llega tarde (después de la primera nota).
            let has_pw = crate::integrations::system_audio::is_pipewire_active();
            if has_pw {
                crate::integrations::system_audio::apply_pipewire_clock(
                    pipewire_rate,
                    pipewire_quantum,
                );
                std::thread::sleep(std::time::Duration::from_millis(250));
                crate::integrations::system_audio::persist_pipewire_conf(
                    pipewire_rate,
                    pipewire_quantum,
                );
            } else {
                crate::integrations::system_audio::persist_pulse_conf(
                    pipewire_rate,
                    pipewire_quantum,
                );
            }

            if let Some(restore) = restore {
                let _ = core.audio_manager().load_file(
                    &restore.path,
                    restore.title,
                    restore.artist,
                    restore.album,
                    restore.cover_path,
                    restore.track_gain,
                    restore.album_gain,
                );
                core.audio_manager().seek(restore.last_pos_sec);
                core.audio_manager().set_playing(restore.is_playing);
            }
        }

        let mut audio_center_manager = AudioCenterManager::default();

        // Cargar persistencia de EQ y DSP al inicio para que se apliquen al audio inmediatamente.
        // db-lock: cada lectura se hace en un scope propio y el guard se libera
        // antes de tocar el gestor de audio.
        let player_volume = core
            .database()
            .lock()
            .ok()
            .and_then(|db_lock| db_lock.get_setting("player_volume"))
            .and_then(|vol_str| vol_str.parse::<f32>().ok());
        if let Some(vol) = player_volume {
            core.audio_manager().set_volume(vol.clamp(0.0, 1.0));
        }

        // Cargar Volumen y Mezcla al inicio (aplicar al motor inmediatamente)
        // db-lock: snapshot del estado de audio; el guard se libera antes de
        // tomar el lock de escritura del estado.
        let startup_state = core
            .database()
            .lock()
            .ok()
            .map(|db_lock| StartupAudioState {
                fades_enabled: db_lock
                    .get_setting("vol_fades_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false),
                smooth_volume_enabled: db_lock
                    .get_setting("vol_smooth_volume_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false),
                fade_in_ms: db_lock
                    .get_setting("vol_fade_in_ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(2000.0),
                fade_out_ms: db_lock
                    .get_setting("vol_fade_out_ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(3000.0),
                fade_in_enabled: db_lock
                    .get_setting("vol_fade_in_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false),
                fade_out_enabled: db_lock
                    .get_setting("vol_fade_out_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false),
                silence_enabled: db_lock
                    .get_setting("vol_silence_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(true),
                silence_duration_ms: db_lock
                    .get_setting("vol_silence_duration_ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(1000.0),
                silence_threshold_db: db_lock
                    .get_setting("vol_silence_threshold_db")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(-47.0),
                silence_edge_trim_enabled: db_lock
                    .get_setting("vol_silence_edge_trim_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(true),
                rg_fixed_enabled: db_lock
                    .get_setting("vol_rg_fixed_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false),
                rg_fixed_db: db_lock
                    .get_setting("vol_rg_fixed_db")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0),
                rg_master_enabled: db_lock
                    .get_setting("vol_rg_master_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(true),
                replay_gain_track_enabled: db_lock
                    .get_setting("vol_rg_track_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(true),
                replay_gain_album_enabled: db_lock
                    .get_setting("vol_rg_album_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(true),
                rg_analyze_rt_enabled: db_lock
                    .get_setting("vol_rg_analyze_rt_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(true),
                rg_offset_album_db: db_lock
                    .get_setting("vol_rg_offset_album_db")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0),
                rg_offset_track_db: db_lock
                    .get_setting("vol_rg_offset_track_db")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0),
                rg_offset_rt_db: db_lock
                    .get_setting("vol_rg_offset_rt_db")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0),
                // Mezcla Cruzada / Crossfade: cargar al arranque (aplicar al motor inmediatamente)
                crossfade_enabled: db_lock
                    .get_setting("crossfade_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false),
                crossfade_manual_enabled: db_lock
                    .get_setting("crossfade_manual_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false),
                crossfade_manual_ms: db_lock
                    .get_setting("crossfade_manual_ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(1000.0),
                crossfade_auto_enabled: db_lock
                    .get_setting("crossfade_auto_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false),
                crossfade_auto_ms: db_lock
                    .get_setting("crossfade_auto_ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(250.0),
                // VU Meter — cargar configuración al arranque
                meter_hold_time_ms: db_lock
                    .get_setting("meter_hold_time_ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(1500.0),
                meter_rms_window_ms: db_lock
                    .get_setting("meter_rms_window_ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(300.0),
                meter_infinite_hold: db_lock
                    .get_setting("meter_infinite_hold")
                    .map(|v| v == "1")
                    .unwrap_or(false),
                meter_enabled: db_lock
                    .get_setting("meter_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false),
            });
        if let Some(s) = startup_state {
            let state_arc = core.audio_manager().state();
            let mut state = state_arc.write();
            state.fades_enabled = s.fades_enabled;
            state.smooth_volume_enabled = s.smooth_volume_enabled;
            state.fade_in_ms = s.fade_in_ms;
            state.fade_out_ms = s.fade_out_ms;
            state.fade_in_enabled = s.fade_in_enabled;
            state.fade_out_enabled = s.fade_out_enabled;
            state.silence_enabled = s.silence_enabled;
            state.silence_duration_ms = s.silence_duration_ms;
            state.silence_threshold_db = s.silence_threshold_db;
            state.silence_edge_trim_enabled = s.silence_edge_trim_enabled;
            state.rg_fixed_enabled = s.rg_fixed_enabled;
            state.rg_fixed_db = s.rg_fixed_db;
            state.rg_master_enabled = s.rg_master_enabled;
            state.replay_gain_track_enabled = s.replay_gain_track_enabled;
            state.replay_gain_album_enabled = s.replay_gain_album_enabled;
            state.rg_analyze_rt_enabled = s.rg_analyze_rt_enabled;
            state.rg_offset_album_db = s.rg_offset_album_db;
            state.rg_offset_track_db = s.rg_offset_track_db;
            state.rg_offset_rt_db = s.rg_offset_rt_db;
            state.crossfade_enabled = s.crossfade_enabled;
            state.crossfade_manual_enabled = s.crossfade_manual_enabled;
            state.crossfade_manual_ms = s.crossfade_manual_ms;
            state.crossfade_auto_enabled = s.crossfade_auto_enabled;
            state.crossfade_auto_ms = s.crossfade_auto_ms;
            state.meter_hold_time_ms = s.meter_hold_time_ms;
            state.meter_rms_window_ms = s.meter_rms_window_ms;
            state.meter_infinite_hold = s.meter_infinite_hold;
            state.meter_enabled = s.meter_enabled;
        }

        // EQ enable / bands_31 / preamp_gain
        // db-lock: snapshot de EQ y presets; el guard se libera antes de aplicar
        // los ajustes al gestor de audio.
        let startup_eq = core.database().lock().ok().map(|db_lock| {
            let eq_enabled = db_lock
                .get_setting("eq_enabled")
                .map(|v| v == "1")
                .unwrap_or(false);
            let eq_bands_31 = db_lock
                .get_setting("eq_bands_31")
                .map(|v| v == "1")
                .unwrap_or(false);
            let preamp_gain: f32 = db_lock
                .get_setting("preamp_gain")
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(0.0);
            // EQ band gains (20 y 31)
            let bands_20: Vec<f32> = db_lock
                .get_setting("eq_band_gains_20")
                .map(|v| v.split(',').filter_map(|s| s.parse::<f32>().ok()).collect())
                .filter(|v: &Vec<f32>| v.len() == 20)
                .unwrap_or_else(|| vec![0.0; 20]);
            let bands_31: Vec<f32> = db_lock
                .get_setting("eq_band_gains_31")
                .map(|v| v.split(',').filter_map(|s| s.parse::<f32>().ok()).collect())
                .filter(|v: &Vec<f32>| v.len() == 31)
                .unwrap_or_else(|| vec![0.0; 31]);

            // Cargar presets personalizados
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
            let custom_presets = db_lock
                .load_eq_presets_raw()
                .map(|raw| {
                    raw.into_iter()
                        .map(|(name, preamp_gain, b20, b31)| {
                            crate::audio::preset::EqPreset::new(
                                &name,
                                preamp_gain,
                                deserialize(b20),
                                deserialize(b31),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            let selected_preset_name = db_lock.get_setting("eq_selected_preset");

            StartupEq {
                eq_enabled,
                eq_bands_31,
                preamp_gain,
                bands_20,
                bands_31,
                custom_presets,
                selected_preset_name,
            }
        });
        if let Some(eq) = startup_eq {
            // Aplicar al motor de audio inmediatamente
            core.audio_manager()
                .set_eq_mode(if eq.eq_bands_31 { 31 } else { 20 });
            core.audio_manager().set_eq_enabled(eq.eq_enabled);
            core.audio_manager().set_preamp_gain(eq.preamp_gain);
            core.audio_manager()
                .apply_eq_preset_gains(&eq.bands_20, &eq.bands_31);

            // Update AudioCenterManager state
            audio_center_manager.custom_presets = eq.custom_presets;
            audio_center_manager.equalizer_enabled = eq.eq_enabled;
            audio_center_manager.equalizer_bands_31 = eq.eq_bands_31;
            audio_center_manager.preamp_gain = eq.preamp_gain;
            audio_center_manager.eq_band_gains = if eq.eq_bands_31 {
                eq.bands_31.clone()
            } else {
                eq.bands_20.clone()
            };

            // Cargar preset seleccionado
            if let Some(preset_name) = eq.selected_preset_name {
                if !preset_name.is_empty() && preset_name != "Default" {
                    // Buscar primero en custom presets, luego en default_presets
                    let custom = audio_center_manager
                        .custom_presets
                        .iter()
                        .find(|p| p.name == preset_name)
                        .cloned();
                    let default = crate::audio::preset::EqPreset::default_presets()
                        .into_iter()
                        .find(|p| p.name == preset_name);
                    if let Some(p) = custom.or(default) {
                        audio_center_manager.selected_preset = Some(p);
                    }
                }
            }
        }

        // DSP effects persistence
        // db-lock: snapshot de los parámetros DSP; el guard se libera antes de
        // aplicar los efectos a la cadena.
        let startup_dsp = core.database().lock().ok().map(|db_lock| StartupDsp {
            sub_bass_enabled: db_lock
                .get_setting("dsp_sub_bass_enabled")
                .map(|v| v == "1"),
            mid_bass_enabled: db_lock
                .get_setting("dsp_mid_bass_enabled")
                .map(|v| v == "1"),
            voice_boost_enabled: db_lock
                .get_setting("dsp_voice_boost_enabled")
                .map(|v| v == "1"),
            noise_gate_enabled: db_lock
                .get_setting("dsp_noise_gate_enabled")
                .map(|v| v == "1"),
            stereo_expander_enabled: db_lock
                .get_setting("dsp_stereo_expander_enabled")
                .map(|v| v == "1"),
            stereo_balance_enabled: db_lock
                .get_setting("dsp_stereo_balance_enabled")
                .map(|v| v == "1"),
            compressor_enabled: db_lock
                .get_setting("dsp_compressor_enabled")
                .map(|v| v == "1"),
            limiter_enabled: db_lock.get_setting("dsp_limiter_enabled").map(|v| v == "1"),
            reverb_enabled: db_lock.get_setting("dsp_reverb_enabled").map(|v| v == "1"),
            sub_bass_gain: db_lock
                .get_setting("dsp_sub_bass_gain")
                .and_then(|v| v.parse::<f32>().ok()),
            mid_bass_gain: db_lock
                .get_setting("dsp_mid_bass_gain")
                .and_then(|v| v.parse::<f32>().ok()),
            voice_boost_gain: db_lock
                .get_setting("dsp_voice_boost_gain")
                .and_then(|v| v.parse::<f32>().ok()),
            noise_gate_threshold: db_lock
                .get_setting("dsp_noise_gate_threshold")
                .and_then(|v| v.parse::<f32>().ok()),
            stereo_expander_width: db_lock
                .get_setting("dsp_stereo_expander_width")
                .and_then(|v| v.parse::<f32>().ok()),
            stereo_expander_surround: db_lock
                .get_setting("dsp_stereo_expander_mode")
                .map(|v| v == "surround"),
            stereo_balance_balance: db_lock
                .get_setting("dsp_stereo_balance_balance")
                .and_then(|v| v.parse::<f32>().ok()),
            compressor_threshold: db_lock
                .get_setting("dsp_compressor_threshold")
                .and_then(|v| v.parse::<f32>().ok()),
            compressor_intensity: db_lock
                .get_setting("dsp_compressor_intensity")
                .and_then(|v| v.parse::<f32>().ok()),
            limiter_ceiling: db_lock
                .get_setting("dsp_limiter_ceiling")
                .and_then(|v| v.parse::<f32>().ok()),
            reverb_wet: db_lock
                .get_setting("dsp_reverb_wet")
                .and_then(|v| v.parse::<f32>().ok()),
            reverb_room_size: db_lock
                .get_setting("dsp_reverb_room_size")
                .and_then(|v| v.parse::<f32>().ok()),
        });
        if let Some(dsp) = startup_dsp {
            core.audio_manager().with_dsp_mut(|chain| {
                if let Some(v) = dsp.sub_bass_enabled {
                    chain.sub_bass.enabled = v;
                }
                if let Some(v) = dsp.mid_bass_enabled {
                    chain.mid_bass.enabled = v;
                }
                if let Some(v) = dsp.voice_boost_enabled {
                    chain.voice_boost.enabled = v;
                }
                if let Some(v) = dsp.noise_gate_enabled {
                    chain.noise_gate.enabled = v;
                }
                if let Some(v) = dsp.stereo_expander_enabled {
                    chain.stereo_expander.enabled = v;
                }
                if let Some(v) = dsp.stereo_balance_enabled {
                    chain.stereo_balance.enabled = v;
                }
                if let Some(v) = dsp.compressor_enabled {
                    chain.compressor.enabled = v;
                }
                if let Some(v) = dsp.limiter_enabled {
                    chain.limiter.enabled = v;
                }
                if let Some(v) = dsp.reverb_enabled {
                    chain.reverb.enabled = v;
                }
            });
            core.audio_manager().with_dsp_mut(|chain| {
                // Slider values
                if let Some(v) = dsp.sub_bass_gain {
                    chain.sub_bass.gain = v;
                }
                if let Some(v) = dsp.mid_bass_gain {
                    chain.mid_bass.gain = v;
                }
                if let Some(v) = dsp.voice_boost_gain {
                    chain.voice_boost.gain = v;
                }
                if let Some(v) = dsp.noise_gate_threshold {
                    chain.noise_gate.threshold = v;
                }
                if let Some(v) = dsp.stereo_expander_width {
                    chain.stereo_expander.width = v;
                }
                if let Some(v) = dsp.stereo_expander_surround {
                    chain.stereo_expander.mode = if v {
                        crate::audio::dsp::ExpanderMode::Surround
                    } else {
                        crate::audio::dsp::ExpanderMode::Hybrid
                    };
                }
                if let Some(v) = dsp.stereo_balance_balance {
                    chain.stereo_balance.balance = v;
                }
                if let Some(v) = dsp.compressor_threshold {
                    chain.compressor.threshold = v;
                }
                if let Some(v) = dsp.compressor_intensity {
                    chain.compressor.intensity = v;
                }
                if let Some(v) = dsp.limiter_ceiling {
                    chain.limiter.ceiling = v;
                }
                if let Some(v) = dsp.reverb_wet {
                    chain.reverb.wet = v;
                }
                if let Some(v) = dsp.reverb_room_size {
                    chain.reverb.room_size = v;
                }
            });
        }

        // --- Volumen y Mezcla settings ---
        // Load into AudioCenterManager fields AND apply to AudioState
        // db-lock: snapshot de volumen y crossfade; el guard se libera antes de
        // escribir el estado de audio y el espejo del centro de audio.
        let startup_volume = core.database().lock().ok().map(|db_lock| StartupVolume {
            fades_enabled: db_lock.get_setting("vol_fades_enabled").map(|v| v == "1"),
            smooth_volume_enabled: db_lock
                .get_setting("vol_smooth_volume_enabled")
                .map(|v| v == "1"),
            fade_in_ms: db_lock
                .get_setting("vol_fade_in_ms")
                .and_then(|v| v.parse::<f32>().ok()),
            fade_out_ms: db_lock
                .get_setting("vol_fade_out_ms")
                .and_then(|v| v.parse::<f32>().ok()),
            silence_enabled: db_lock.get_setting("vol_silence_enabled").map(|v| v == "1"),
            silence_duration_ms: db_lock
                .get_setting("vol_silence_duration_ms")
                .and_then(|v| v.parse::<f32>().ok()),
            silence_threshold_db: db_lock
                .get_setting("vol_silence_threshold_db")
                .and_then(|v| v.parse::<f32>().ok()),
            silence_edge_trim_enabled: db_lock
                .get_setting("vol_silence_edge_trim_enabled")
                .map(|v| v == "1"),
            rg_master_enabled: db_lock
                .get_setting("vol_rg_master_enabled")
                .map(|v| v == "1"),
            rg_track_enabled: db_lock
                .get_setting("vol_rg_track_enabled")
                .map(|v| v == "1"),
            rg_album_enabled: db_lock
                .get_setting("vol_rg_album_enabled")
                .map(|v| v == "1"),
            rg_analyze_rt_enabled: db_lock
                .get_setting("vol_rg_analyze_rt_enabled")
                .map(|v| v == "1"),
            rg_offset_album_db: db_lock
                .get_setting("vol_rg_offset_album_db")
                .and_then(|v| v.parse::<f32>().ok()),
            rg_offset_track_db: db_lock
                .get_setting("vol_rg_offset_track_db")
                .and_then(|v| v.parse::<f32>().ok()),
            rg_offset_rt_db: db_lock
                .get_setting("vol_rg_offset_rt_db")
                .and_then(|v| v.parse::<f32>().ok()),
        });
        if let Some(vol) = startup_volume {
            if let Some(v) = vol.fades_enabled {
                audio_center_manager.volumen_fades_enabled = v;
                core.audio_manager().state().write().fades_enabled = v;
            }
            if let Some(v) = vol.smooth_volume_enabled {
                audio_center_manager.volumen_smooth_volume_enabled = v;
                core.audio_manager().state().write().smooth_volume_enabled = v;
            }
            if let Some(v) = vol.fade_in_ms {
                audio_center_manager.volumen_fade_in_ms = v as f64;
                core.audio_manager().state().write().fade_in_ms = v;
            }
            if let Some(v) = vol.fade_out_ms {
                audio_center_manager.volumen_fade_out_ms = v as f64;
                core.audio_manager().state().write().fade_out_ms = v;
            }
            if let Some(v) = vol.silence_enabled {
                audio_center_manager.volumen_silence_enabled = v;
                core.audio_manager().state().write().silence_enabled = v;
            }
            if let Some(v) = vol.silence_duration_ms {
                audio_center_manager.volumen_silence_duration_ms = v as f64;
                core.audio_manager().state().write().silence_duration_ms = v;
            }
            if let Some(v) = vol.silence_threshold_db {
                audio_center_manager.volumen_silence_threshold_db = v as f64;
                core.audio_manager().state().write().silence_threshold_db = v;
            }
            if let Some(v) = vol.silence_edge_trim_enabled {
                audio_center_manager.volumen_silence_edge_trim_enabled = v;
                core.audio_manager()
                    .state()
                    .write()
                    .silence_edge_trim_enabled = v;
            }
            if let Some(v) = vol.rg_master_enabled {
                audio_center_manager.volumen_rg_master_enabled = v;
                core.audio_manager().state().write().rg_master_enabled = v;
            }
            if let Some(v) = vol.rg_track_enabled {
                audio_center_manager.volumen_rg_track_enabled = v;
                core.audio_manager()
                    .state()
                    .write()
                    .replay_gain_track_enabled = v;
            }
            if let Some(v) = vol.rg_album_enabled {
                audio_center_manager.volumen_rg_album_enabled = v;
                core.audio_manager()
                    .state()
                    .write()
                    .replay_gain_album_enabled = v;
            }
            if let Some(v) = vol.rg_analyze_rt_enabled {
                audio_center_manager.volumen_rg_analyze_rt_enabled = v;
                core.audio_manager().state().write().rg_analyze_rt_enabled = v;
            }
            if let Some(v) = vol.rg_offset_album_db {
                audio_center_manager.volumen_rg_offset_album_db = v as f64;
                core.audio_manager().state().write().rg_offset_album_db = v;
            }
            if let Some(v) = vol.rg_offset_track_db {
                audio_center_manager.volumen_rg_offset_track_db = v as f64;
                core.audio_manager().state().write().rg_offset_track_db = v;
            }
            if let Some(v) = vol.rg_offset_rt_db {
                audio_center_manager.volumen_rg_offset_rt_db = v as f64;
                core.audio_manager().state().write().rg_offset_rt_db = v;
            }
        }

        // VU Meter — cargar modo M/S para PlayerUiState
        // db-lock: solo lectura — refresco best-effort, se omite si el lock falla
        let meter_ms_mode = if let Ok(db) = core.database().lock() {
            db.get_setting("meter_ms_mode")
                .map(|v| v == "1")
                .unwrap_or(false)
        } else {
            false
        };

        (
            Self {
                core,
                media_controls,
                external_control_rx,
                core_event_rx,
                playlist_manager,
                filters_manager,
                library_manager,
                audio_center_manager,
                player_ui_state: {
                    let mut p = crate::gui::player::PlayerUiState::default();
                    p.meter.ms_mode = meter_ms_mode;
                    p
                },
                window_id: None,
                last_artist_header_click: None,
                last_album_header_click: None,
                last_playlist_click: None,
                focus: AppFocus::Library,
                previous_focus: AppFocus::Library,
                is_mouse_over_playlist: false,
                low_resource_mode,
                db_needs_refresh: false,
                active_dialog: ActiveDialog::None,
                last_mouse_pos: iced::Point::ORIGIN,
                dialog_pos: None,
                context_menu: None,
                window_size: (1360, 880),
                modifiers: iced::keyboard::Modifiers::default(),
                was_scanning: false,
                scan_finished_at: None,
            },
            Task::perform(
                async {
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                },
                |_| Message::InitStartup,
            ),
        )
    }
}
