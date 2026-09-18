//! Constructor and initial load for `AudoxidyApp`.

use crate::audio::AudioManager;
use crate::db::{Database, scanner::Scanner};
use crate::gui::audio_center::AudioCenterManager;
use crate::gui::library::LibraryManager;
use crate::gui::library_filters::LibraryFiltersManager;
use crate::gui::playlist::PlaylistManager;
use crate::integrations::control::external_control_channel;
use crate::integrations::media_controls::SystemMediaControls;
use iced::Task;
use std::sync::{Arc, Mutex};

use super::state::AudoxidyApp;
use super::{ActiveDialog, AppFocus, Message};

impl AudoxidyApp {
    pub fn new(audio_manager: Arc<AudioManager>) -> (Self, Task<Message>) {
        // Inicializar temporizadores de memoria (Ciclo de 2 minutos)
        crate::utils::memory_manager::MemoryManager::init();

        let db = Database::new().expect("Error crítico al crear/iniciar la base de datos.");
        let mut library_manager = LibraryManager::default();

        if let Ok((total_songs, total_albums, total_duration, total_size, total_artists)) =
            db.get_library_stats()
        {
            library_manager.total_songs = total_songs;
            library_manager.total_albums = total_albums;
            library_manager.total_artists = total_artists;
            library_manager.total_duration_secs = total_duration;
            library_manager.total_size_bytes = total_size;
        }

        if let Ok(folders) = db.get_all_folders() {
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
        let initial_songs = db.get_all_songs().unwrap_or_default();
        let initial_albums_raw = db.get_grid_items_by_artist().unwrap_or_default();
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

        let database_arc = Arc::new(Mutex::new(db));
        audio_manager.set_database(database_arc.clone());
        let db_scanner =
            Database::new().expect("Error al inicializar la base de datos del escáner.");
        let scanner_arc = Arc::new(Scanner::new(Arc::new(Mutex::new(db_scanner))));

        let (external_control_tx, external_control_rx) = external_control_channel();
        let media_controls = SystemMediaControls::new(external_control_tx)
            .expect("Error al inicializar controles multimedia del sistema");

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
        filters_manager.build_filter_index(&database_arc);

        let mut playlist_manager = PlaylistManager::default();
        // db-lock: solo lectura + backfill best-effort — se omite si el lock falla
        if let Ok(db_lock) = database_arc.lock() {
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
            if host_id.is_some()
                || device_name.is_some()
                || device_id.is_some()
                || sample_rate.is_some()
                || bit_depth.is_some()
                || buffer_size.is_some()
            {
                let had_device_id = device_id.is_none();
                let settings = crate::audio::engine::AudioSettings {
                    host_id,
                    device_name,
                    device_id,
                    sample_rate, // None → auto → 48000 en resolve_settings
                    bit_depth,   // None → fallback del device
                    channels: crate::audio::engine::ChannelConfig::Manual(
                        channels_val.unwrap_or(2),
                    ),
                    buffer_size,
                };
                let apply_ok = audio_manager.apply_audio_settings(settings).is_ok();
                // Backfill del id estable cuando solo había coincidencia por nombre
                // heredado: así la próxima carga resuelve sin depender del nombre.
                if apply_ok && had_device_id {
                    if let Some(resolved) = audio_manager.get_state().device_id.clone() {
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

            // 0b. Aplicar ajustes de PipeWire/PulseAudio guardados ANTES de reproducir.
            //     Si no se aplican aquí, el stream se conecta al sink con la configuración
            //     anterior y la reconfiguración llega tarde (después de la primera nota).
            {
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

                let rate_val = match sys_rate {
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
                let quantum_val = match sys_quantum {
                    crate::gui::audio_center::SystemSelection::Default => None,
                    crate::gui::audio_center::SystemSelection::Automatic => {
                        let sr = rate_val.unwrap_or(48000);
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

                let has_pw = crate::integrations::system_audio::is_pipewire_active();
                if has_pw {
                    crate::integrations::system_audio::apply_pipewire_clock(rate_val, quantum_val);
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    crate::integrations::system_audio::persist_pipewire_conf(rate_val, quantum_val);
                } else {
                    crate::integrations::system_audio::persist_pulse_conf(rate_val, quantum_val);
                }
            }

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
                            let (tg, ag) = db_lock
                                .get_replay_gain_by_path(&path)
                                .unwrap_or((None, None));
                            let _ = audio_manager.load_file(
                                &path,
                                title.to_string(),
                                artist.to_string(),
                                album,
                                cover_path,
                                tg,
                                ag,
                            );
                            audio_manager.seek(p_data.last_pos_sec);
                            audio_manager.set_playing(p_data.is_playing);
                        }
                    }
                }
            }
        } else {
            tracing::warn!("db-lock: lock no disponible");
        }

        let mut audio_center_manager = AudioCenterManager::default();

        // Cargar persistencia de EQ y DSP al inicio para que se apliquen al audio inmediatamente
        // db-lock: solo lectura — refresco best-effort, se omite si el lock falla
        if let Ok(db_lock) = database_arc.lock() {
            // Cargar volumen del player desde APP_SETTINGS
            if let Some(vol_str) = db_lock.get_setting("player_volume") {
                if let Ok(vol) = vol_str.parse::<f32>() {
                    audio_manager.set_volume(vol.clamp(0.0, 1.0));
                }
            }

            // Cargar Volumen y Mezcla al inicio (aplicar al motor inmediatamente)
            {
                let state_arc = audio_manager.state();
                let mut s = state_arc.write();
                s.fades_enabled = db_lock
                    .get_setting("vol_fades_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false);
                s.smooth_volume_enabled = db_lock
                    .get_setting("vol_smooth_volume_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false);
                s.fade_in_ms = db_lock
                    .get_setting("vol_fade_in_ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(2000.0);
                s.fade_out_ms = db_lock
                    .get_setting("vol_fade_out_ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(3000.0);
                s.fade_in_enabled = db_lock
                    .get_setting("vol_fade_in_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false);
                s.fade_out_enabled = db_lock
                    .get_setting("vol_fade_out_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false);
                s.silence_enabled = db_lock
                    .get_setting("vol_silence_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(true);
                s.silence_duration_ms = db_lock
                    .get_setting("vol_silence_duration_ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(1000.0);
                s.silence_threshold_db = db_lock
                    .get_setting("vol_silence_threshold_db")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(-47.0);
                s.silence_edge_trim_enabled = db_lock
                    .get_setting("vol_silence_edge_trim_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(true);
                s.rg_fixed_enabled = db_lock
                    .get_setting("vol_rg_fixed_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false);
                s.rg_fixed_db = db_lock
                    .get_setting("vol_rg_fixed_db")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0);
                s.rg_master_enabled = db_lock
                    .get_setting("vol_rg_master_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(true);
                s.replay_gain_track_enabled = db_lock
                    .get_setting("vol_rg_track_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(true);
                s.replay_gain_album_enabled = db_lock
                    .get_setting("vol_rg_album_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(true);
                s.rg_analyze_rt_enabled = db_lock
                    .get_setting("vol_rg_analyze_rt_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(true);
                s.rg_offset_album_db = db_lock
                    .get_setting("vol_rg_offset_album_db")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0);
                s.rg_offset_track_db = db_lock
                    .get_setting("vol_rg_offset_track_db")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0);
                s.rg_offset_rt_db = db_lock
                    .get_setting("vol_rg_offset_rt_db")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0);
                // Mezcla Cruzada / Crossfade: cargar al arranque (aplicar al motor inmediatamente)
                s.crossfade_enabled = db_lock
                    .get_setting("crossfade_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false);
                s.crossfade_manual_enabled = db_lock
                    .get_setting("crossfade_manual_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false);
                s.crossfade_manual_ms = db_lock
                    .get_setting("crossfade_manual_ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(1000.0);
                s.crossfade_auto_enabled = db_lock
                    .get_setting("crossfade_auto_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false);
                s.crossfade_auto_ms = db_lock
                    .get_setting("crossfade_auto_ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(250.0);

                // VU Meter — cargar configuración al arranque
                s.meter_hold_time_ms = db_lock
                    .get_setting("meter_hold_time_ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(1500.0);
                s.meter_rms_window_ms = db_lock
                    .get_setting("meter_rms_window_ms")
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(300.0);
                s.meter_infinite_hold = db_lock
                    .get_setting("meter_infinite_hold")
                    .map(|v| v == "1")
                    .unwrap_or(false);
                s.meter_enabled = db_lock
                    .get_setting("meter_enabled")
                    .map(|v| v == "1")
                    .unwrap_or(false);
            }

            // EQ enable / bands_31 / preamp_gain
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

            // Aplicar al motor de audio inmediatamente
            audio_manager.set_eq_mode(if eq_bands_31 { 31 } else { 20 });
            audio_manager.set_eq_enabled(eq_enabled);
            audio_manager.set_preamp_gain(preamp_gain);
            audio_manager.apply_eq_preset_gains(&bands_20, &bands_31);

            // Cargar presets personalizados y el preset seleccionado
            audio_center_manager.custom_presets = db_lock.load_eq_presets().unwrap_or_default();

            // Update AudioCenterManager state
            audio_center_manager.equalizer_enabled = eq_enabled;
            audio_center_manager.equalizer_bands_31 = eq_bands_31;
            audio_center_manager.preamp_gain = preamp_gain;
            audio_center_manager.eq_band_gains = if eq_bands_31 {
                bands_31.clone()
            } else {
                bands_20.clone()
            };

            // Cargar preset seleccionado
            if let Some(preset_name) = db_lock.get_setting("eq_selected_preset") {
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

            // DSP effects persistence
            let set_bool = |dsp: &mut crate::audio::dsp::DspChain, effect: &str, key: &str| {
                if let Some(v) = db_lock.get_setting(key) {
                    if let Some(field) = match effect {
                        "sub_bass" => Some(&mut dsp.sub_bass.enabled),
                        "mid_bass" => Some(&mut dsp.mid_bass.enabled),
                        "voice_boost" => Some(&mut dsp.voice_boost.enabled),
                        "noise_gate" => Some(&mut dsp.noise_gate.enabled),
                        "stereo_expander" => Some(&mut dsp.stereo_expander.enabled),
                        "stereo_balance" => Some(&mut dsp.stereo_balance.enabled),
                        "compressor" => Some(&mut dsp.compressor.enabled),
                        "limiter" => Some(&mut dsp.limiter.enabled),
                        "reverb" => Some(&mut dsp.reverb.enabled),
                        _ => None,
                    } {
                        *field = v == "1";
                    }
                }
            };
            audio_manager.with_dsp_mut(|dsp| {
                set_bool(dsp, "sub_bass", "dsp_sub_bass_enabled");
                set_bool(dsp, "mid_bass", "dsp_mid_bass_enabled");
                set_bool(dsp, "voice_boost", "dsp_voice_boost_enabled");
                set_bool(dsp, "noise_gate", "dsp_noise_gate_enabled");
                set_bool(dsp, "stereo_expander", "dsp_stereo_expander_enabled");
                set_bool(dsp, "stereo_balance", "dsp_stereo_balance_enabled");
                set_bool(dsp, "compressor", "dsp_compressor_enabled");
                set_bool(dsp, "limiter", "dsp_limiter_enabled");
                set_bool(dsp, "reverb", "dsp_reverb_enabled");
            });
            audio_manager.with_dsp_mut(|dsp| {
                // Slider values
                if let Some(v) = db_lock.get_setting("dsp_sub_bass_gain") {
                    if let Ok(t) = v.parse::<f32>() {
                        dsp.sub_bass.gain = t;
                    }
                }
                if let Some(v) = db_lock.get_setting("dsp_mid_bass_gain") {
                    if let Ok(t) = v.parse::<f32>() {
                        dsp.mid_bass.gain = t;
                    }
                }
                if let Some(v) = db_lock.get_setting("dsp_voice_boost_gain") {
                    if let Ok(t) = v.parse::<f32>() {
                        dsp.voice_boost.gain = t;
                    }
                }
                if let Some(v) = db_lock.get_setting("dsp_noise_gate_threshold") {
                    if let Ok(t) = v.parse::<f32>() {
                        dsp.noise_gate.threshold = t;
                    }
                }
                if let Some(v) = db_lock.get_setting("dsp_stereo_expander_width") {
                    if let Ok(t) = v.parse::<f32>() {
                        dsp.stereo_expander.width = t;
                    }
                }
                if let Some(v) = db_lock.get_setting("dsp_stereo_expander_mode") {
                    dsp.stereo_expander.mode = if v == "surround" {
                        crate::audio::dsp::ExpanderMode::Surround
                    } else {
                        crate::audio::dsp::ExpanderMode::Hybrid
                    };
                }
                if let Some(v) = db_lock.get_setting("dsp_stereo_balance_balance") {
                    if let Ok(t) = v.parse::<f32>() {
                        dsp.stereo_balance.balance = t;
                    }
                }
                if let Some(v) = db_lock.get_setting("dsp_compressor_threshold") {
                    if let Ok(t) = v.parse::<f32>() {
                        dsp.compressor.threshold = t;
                    }
                }
                if let Some(v) = db_lock.get_setting("dsp_compressor_intensity") {
                    if let Ok(t) = v.parse::<f32>() {
                        dsp.compressor.intensity = t;
                    }
                }
                if let Some(v) = db_lock.get_setting("dsp_limiter_ceiling") {
                    if let Ok(t) = v.parse::<f32>() {
                        dsp.limiter.ceiling = t;
                    }
                }
                if let Some(v) = db_lock.get_setting("dsp_reverb_wet") {
                    if let Ok(t) = v.parse::<f32>() {
                        dsp.reverb.wet = t;
                    }
                }
                if let Some(v) = db_lock.get_setting("dsp_reverb_room_size") {
                    if let Ok(t) = v.parse::<f32>() {
                        dsp.reverb.room_size = t;
                    }
                }
            });

            // --- Volumen y Mezcla settings ---
            // Load into AudioCenterManager fields AND apply to AudioState
            if let Some(val) = db_lock.get_setting("vol_fades_enabled") {
                let v = val == "1";
                audio_center_manager.volumen_fades_enabled = v;
                audio_manager.state().write().fades_enabled = v;
            }
            if let Some(val) = db_lock.get_setting("vol_smooth_volume_enabled") {
                let v = val == "1";
                audio_center_manager.volumen_smooth_volume_enabled = v;
                audio_manager.state().write().smooth_volume_enabled = v;
            }
            if let Some(val) = db_lock.get_setting("vol_fade_in_ms") {
                if let Ok(v) = val.parse::<f32>() {
                    audio_center_manager.volumen_fade_in_ms = v as f64;
                    audio_manager.state().write().fade_in_ms = v;
                }
            }
            if let Some(val) = db_lock.get_setting("vol_fade_out_ms") {
                if let Ok(v) = val.parse::<f32>() {
                    audio_center_manager.volumen_fade_out_ms = v as f64;
                    audio_manager.state().write().fade_out_ms = v;
                }
            }
            if let Some(val) = db_lock.get_setting("vol_silence_enabled") {
                let v = val == "1";
                audio_center_manager.volumen_silence_enabled = v;
                audio_manager.state().write().silence_enabled = v;
            }
            if let Some(val) = db_lock.get_setting("vol_silence_duration_ms") {
                if let Ok(v) = val.parse::<f32>() {
                    audio_center_manager.volumen_silence_duration_ms = v as f64;
                    audio_manager.state().write().silence_duration_ms = v;
                }
            }
            if let Some(val) = db_lock.get_setting("vol_silence_threshold_db") {
                if let Ok(v) = val.parse::<f32>() {
                    audio_center_manager.volumen_silence_threshold_db = v as f64;
                    audio_manager.state().write().silence_threshold_db = v;
                }
            }
            if let Some(val) = db_lock.get_setting("vol_silence_edge_trim_enabled") {
                let v = val == "1";
                audio_center_manager.volumen_silence_edge_trim_enabled = v;
                audio_manager.state().write().silence_edge_trim_enabled = v;
            }
            if let Some(val) = db_lock.get_setting("vol_rg_master_enabled") {
                let v = val == "1";
                audio_center_manager.volumen_rg_master_enabled = v;
                audio_manager.state().write().rg_master_enabled = v;
            }
            if let Some(val) = db_lock.get_setting("vol_rg_track_enabled") {
                let v = val == "1";
                audio_center_manager.volumen_rg_track_enabled = v;
                audio_manager.state().write().replay_gain_track_enabled = v;
            }
            if let Some(val) = db_lock.get_setting("vol_rg_album_enabled") {
                let v = val == "1";
                audio_center_manager.volumen_rg_album_enabled = v;
                audio_manager.state().write().replay_gain_album_enabled = v;
            }
            if let Some(val) = db_lock.get_setting("vol_rg_analyze_rt_enabled") {
                let v = val == "1";
                audio_center_manager.volumen_rg_analyze_rt_enabled = v;
                audio_manager.state().write().rg_analyze_rt_enabled = v;
            }
            if let Some(val) = db_lock.get_setting("vol_rg_offset_album_db") {
                if let Ok(v) = val.parse::<f32>() {
                    audio_center_manager.volumen_rg_offset_album_db = v as f64;
                    audio_manager.state().write().rg_offset_album_db = v;
                }
            }
            if let Some(val) = db_lock.get_setting("vol_rg_offset_track_db") {
                if let Ok(v) = val.parse::<f32>() {
                    audio_center_manager.volumen_rg_offset_track_db = v as f64;
                    audio_manager.state().write().rg_offset_track_db = v;
                }
            }
            if let Some(val) = db_lock.get_setting("vol_rg_offset_rt_db") {
                if let Ok(v) = val.parse::<f32>() {
                    audio_center_manager.volumen_rg_offset_rt_db = v as f64;
                    audio_manager.state().write().rg_offset_rt_db = v;
                }
            }
        }

        // VU Meter — cargar modo M/S para PlayerUiState
        // db-lock: solo lectura — refresco best-effort, se omite si el lock falla
        let meter_ms_mode = if let Ok(db) = database_arc.lock() {
            db.get_setting("meter_ms_mode")
                .map(|v| v == "1")
                .unwrap_or(false)
        } else {
            false
        };

        (
            Self {
                audio_manager,
                database: database_arc,
                scanner: scanner_arc,
                media_controls,
                external_control_rx,
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
