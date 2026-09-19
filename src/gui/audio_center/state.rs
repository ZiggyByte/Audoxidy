//! Audio center manager state, persistence, and engine synchronization.

use crate::audio::AudioManager;
use crate::audio::device_manager::{AudioDeviceInfo, BitDepth};

use super::messages::SystemSelection;

/// Registra en el log el fallo de una escritura de ajuste, conservando la clave
/// afectada y el error para poder diagnosticar por qué no se persistió.
pub(in crate::gui::audio_center) fn log_persist(key: &str, result: rusqlite::Result<()>) {
    if let Err(e) = result {
        tracing::warn!(
            target: "audoxidy::persistence",
            key,
            error = %e,
            "No se pudo persistir el ajuste"
        );
    }
}

pub struct AudioCenterManager {
    pub open: bool,
    pub selected_tab: usize,

    // Dragging state
    pub drag_start: Option<iced::Point>,
    pub window_pos: Option<iced::Point>,
    pub is_dragging: bool,

    // Caché para panel 1
    pub cached_hosts: Vec<String>,
    pub cached_devices: Vec<AudioDeviceInfo>,

    pub selected_host: Option<String>,
    pub selected_device: Option<String>,
    pub selected_sample_rate: Option<u32>,
    pub selected_bit_depth: BitDepth,
    pub selected_buffer_size: Option<u32>,
    pub selected_channels_manual: u16,
    pub apply_enabled: bool,

    pub preset_name_input: String,

    // Equalizer State
    pub equalizer_enabled: bool,
    pub equalizer_bands_31: bool,
    pub preamp_gain: f32,
    pub eq_band_gains: Vec<f32>,
    pub equalizer_presets: Vec<crate::audio::preset::EqPreset>,
    pub selected_preset: Option<crate::audio::preset::EqPreset>,

    // EQ Preset Management
    pub custom_presets: Vec<crate::audio::preset::EqPreset>,
    pub hidden_builtins: Vec<String>,
    pub pending_preset_name: String,

    pub first_open: bool,

    // Configuración del servidor de audio de sistema (Pipewire / PulseAudio)
    pub system_rate: SystemSelection<u32>,
    pub system_quantum: SystemSelection<u32>,

    // Cache for system_sound_status (refresh every 3s, not every frame)
    pub cached_system_status: (String, String),
    pub cached_system_status_ts: std::time::Instant,

    // Volumen y Mezcla state — mirrors AudioState for UI display
    pub volumen_fades_enabled: bool,
    pub volumen_smooth_volume_enabled: bool,
    pub volumen_fade_in_ms: f64,
    pub volumen_fade_out_ms: f64,
    pub volumen_silence_enabled: bool,
    pub volumen_silence_duration_ms: f64,
    pub volumen_silence_threshold_db: f64,
    pub volumen_silence_edge_trim_enabled: bool,
    pub volumen_rg_fixed_enabled: bool,
    pub volumen_rg_fixed_db: f64,
    pub volumen_fade_in_enabled: bool,
    pub volumen_fade_out_enabled: bool,
    pub volumen_rg_master_enabled: bool,
    pub volumen_rg_track_enabled: bool,
    pub volumen_rg_album_enabled: bool,
    pub volumen_rg_analyze_rt_enabled: bool,
    pub volumen_rg_offset_album_db: f64,
    pub volumen_rg_offset_track_db: f64,
    pub volumen_rg_offset_rt_db: f64,

    // Mezcla Cruzada (crossfade) — espejo de AudioState para la UI
    pub crossfade_enabled: bool,
    pub crossfade_manual_enabled: bool,
    pub crossfade_manual_ms: f64,
    pub crossfade_auto_enabled: bool,
    pub crossfade_auto_ms: f64,
}

impl Default for AudioCenterManager {
    fn default() -> Self {
        Self {
            open: false,
            selected_tab: 0,

            drag_start: None,
            window_pos: None,
            is_dragging: false,

            cached_hosts: Vec::new(),
            cached_devices: Vec::new(),

            selected_host: None,
            selected_device: None,
            selected_sample_rate: None,
            selected_bit_depth: BitDepth::Bits32Float,
            selected_buffer_size: None,
            selected_channels_manual: 2,
            apply_enabled: false,

            preset_name_input: String::new(),

            equalizer_enabled: false,
            equalizer_bands_31: false,
            preamp_gain: 0.0,
            eq_band_gains: vec![0.0; 20], // Default to 20 bands
            equalizer_presets: crate::audio::preset::EqPreset::default_presets(),
            selected_preset: None,

            custom_presets: Vec::new(),
            hidden_builtins: Vec::new(),
            pending_preset_name: String::new(),

            first_open: true,

            system_rate: SystemSelection::Default,
            system_quantum: SystemSelection::Default,

            cached_system_status: ("--".to_string(), "--".to_string()),
            cached_system_status_ts: std::time::Instant::now(),

            volumen_fades_enabled: false,
            volumen_smooth_volume_enabled: false,
            volumen_fade_in_ms: 1000.0,
            volumen_fade_out_ms: 2000.0,
            volumen_silence_enabled: true,
            volumen_silence_duration_ms: 1000.0,
            volumen_silence_threshold_db: -47.0,
            volumen_silence_edge_trim_enabled: true,
            volumen_rg_fixed_enabled: false,
            volumen_rg_fixed_db: 0.0,
            volumen_fade_in_enabled: false,
            volumen_fade_out_enabled: false,
            volumen_rg_master_enabled: true,
            volumen_rg_track_enabled: true,
            volumen_rg_album_enabled: true,
            volumen_rg_analyze_rt_enabled: true,
            volumen_rg_offset_album_db: 0.0,
            volumen_rg_offset_track_db: 0.0,
            volumen_rg_offset_rt_db: 0.0,
            crossfade_enabled: false,
            crossfade_manual_enabled: false,
            crossfade_manual_ms: 1000.0,
            crossfade_auto_enabled: false,
            crossfade_auto_ms: 250.0,
        }
    }
}

impl AudioCenterManager {
    /// Refresh cached system_sound_status every 3 seconds (not every frame).
    pub fn refresh_system_status_cache(&mut self) {
        if self.cached_system_status_ts.elapsed() >= std::time::Duration::from_secs(3) {
            self.cached_system_status = crate::integrations::system_audio::system_sound_status();
            self.cached_system_status_ts = std::time::Instant::now();
        }
    }

    /// Load custom presets from SQLite into custom_presets vec.
    pub fn load_custom_presets(&mut self, db: &std::sync::Mutex<crate::db::Database>) {
        use crate::audio::preset::EqPreset;

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

        // db-lock: solo lectura — refresco best-effort, se omite si el lock falla
        if let Ok(db_lock) = db.lock() {
            if let Ok(raw) = db_lock.load_eq_presets_raw() {
                self.custom_presets = raw
                    .into_iter()
                    .map(|(name, preamp_gain, b20, b31)| {
                        EqPreset::new(&name, preamp_gain, deserialize(b20), deserialize(b31))
                    })
                    .collect();
            }
        }
    }
    /// Build unified preset list for Load dialog:
    /// 1. "Default" always first
    /// 2. Custom presets (A-Z, case-insensitive)
    /// 3. Built-in presets (A-Z, excluding Default and hidden)
    pub fn get_unified_presets(&self) -> Vec<&crate::audio::preset::EqPreset> {
        let mut all: Vec<&crate::audio::preset::EqPreset> = Vec::new();

        // 1. "Default" always first
        if let Some(default) = self.equalizer_presets.iter().find(|p| p.name == "Default") {
            all.push(default);
        }

        // 2. Custom presets (sorted A-Z)
        let mut custom: Vec<&crate::audio::preset::EqPreset> = self.custom_presets.iter().collect();
        custom.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        all.extend(custom);

        // 3. Built-in presets excluding Default and hidden
        let mut builtin: Vec<&crate::audio::preset::EqPreset> = self
            .equalizer_presets
            .iter()
            .filter(|p| p.name != "Default" && !self.hidden_builtins.contains(&p.name))
            .collect();
        builtin.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        all.extend(builtin);

        all
    }

    /// Guarda todos los ajustes del ecualizador en la base de datos.
    pub fn save_eq_settings_to_db(&self, db: &std::sync::Mutex<crate::db::Database>) {
        // db-lock: persistencia — el guardado se omite si el lock falla
        if let Ok(db_lock) = db.lock() {
            log_persist(
                "eq_enabled",
                db_lock.set_setting("eq_enabled", if self.equalizer_enabled { "1" } else { "0" }),
            );
            log_persist(
                "eq_bands_31",
                db_lock.set_setting(
                    "eq_bands_31",
                    if self.equalizer_bands_31 { "1" } else { "0" },
                ),
            );
            log_persist(
                "preamp_gain",
                db_lock.set_setting("preamp_gain", &format!("{:.1}", self.preamp_gain)),
            );
            // Save selected preset name
            let preset_name = self
                .selected_preset
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_default();
            log_persist(
                "eq_selected_preset",
                db_lock.set_setting("eq_selected_preset", &preset_name),
            );
            // Guardar gains de 20 y 31 bandas como CSV
            let bands_20_str = self
                .eq_band_gains
                .iter()
                .take(20)
                .map(|g| format!("{:.1}", g))
                .collect::<Vec<_>>()
                .join(",");
            log_persist(
                "eq_band_gains_20",
                db_lock.set_setting("eq_band_gains_20", &bands_20_str),
            );
            if self.eq_band_gains.len() > 20 {
                let bands_31_str = self
                    .eq_band_gains
                    .iter()
                    .map(|g| format!("{:.1}", g))
                    .collect::<Vec<_>>()
                    .join(",");
                log_persist(
                    "eq_band_gains_31",
                    db_lock.set_setting("eq_band_gains_31", &bands_31_str),
                );
            }
        } else {
            tracing::warn!("db-lock: lock no disponible");
        }
    }

    /// Guarda todos los ajustes de efectos de audio en la base de datos.
    pub fn save_dsp_settings_to_db(
        &self,
        audio_manager: &AudioManager,
        db: &std::sync::Mutex<crate::db::Database>,
    ) {
        // db-lock: persistencia — el guardado se omite si el lock falla
        if let Ok(db_lock) = db.lock() {
            audio_manager.with_dsp(|dsp| {
                // Enabled states
                log_persist(
                    "dsp_sub_bass_enabled",
                    db_lock.set_setting(
                        "dsp_sub_bass_enabled",
                        if dsp.sub_bass.enabled { "1" } else { "0" },
                    ),
                );
                log_persist(
                    "dsp_mid_bass_enabled",
                    db_lock.set_setting(
                        "dsp_mid_bass_enabled",
                        if dsp.mid_bass.enabled { "1" } else { "0" },
                    ),
                );
                log_persist(
                    "dsp_voice_boost_enabled",
                    db_lock.set_setting(
                        "dsp_voice_boost_enabled",
                        if dsp.voice_boost.enabled { "1" } else { "0" },
                    ),
                );
                log_persist(
                    "dsp_noise_gate_enabled",
                    db_lock.set_setting(
                        "dsp_noise_gate_enabled",
                        if dsp.noise_gate.enabled { "1" } else { "0" },
                    ),
                );
                log_persist(
                    "dsp_stereo_expander_enabled",
                    db_lock.set_setting(
                        "dsp_stereo_expander_enabled",
                        if dsp.stereo_expander.enabled {
                            "1"
                        } else {
                            "0"
                        },
                    ),
                );
                log_persist(
                    "dsp_stereo_balance_enabled",
                    db_lock.set_setting(
                        "dsp_stereo_balance_enabled",
                        if dsp.stereo_balance.enabled { "1" } else { "0" },
                    ),
                );
                log_persist(
                    "dsp_compressor_enabled",
                    db_lock.set_setting(
                        "dsp_compressor_enabled",
                        if dsp.compressor.enabled { "1" } else { "0" },
                    ),
                );
                log_persist(
                    "dsp_limiter_enabled",
                    db_lock.set_setting(
                        "dsp_limiter_enabled",
                        if dsp.limiter.enabled { "1" } else { "0" },
                    ),
                );
                log_persist(
                    "dsp_reverb_enabled",
                    db_lock.set_setting(
                        "dsp_reverb_enabled",
                        if dsp.reverb.enabled { "1" } else { "0" },
                    ),
                );
                // Slider values
                log_persist(
                    "dsp_sub_bass_gain",
                    db_lock.set_setting("dsp_sub_bass_gain", &format!("{:.1}", dsp.sub_bass.gain)),
                );
                log_persist(
                    "dsp_mid_bass_gain",
                    db_lock.set_setting("dsp_mid_bass_gain", &format!("{:.1}", dsp.mid_bass.gain)),
                );
                log_persist(
                    "dsp_voice_boost_gain",
                    db_lock.set_setting(
                        "dsp_voice_boost_gain",
                        &format!("{:.1}", dsp.voice_boost.gain),
                    ),
                );
                log_persist(
                    "dsp_noise_gate_threshold",
                    db_lock.set_setting(
                        "dsp_noise_gate_threshold",
                        &format!("{:.1}", dsp.noise_gate.threshold),
                    ),
                );
                log_persist(
                    "dsp_stereo_expander_width",
                    db_lock.set_setting(
                        "dsp_stereo_expander_width",
                        &format!("{:.1}", dsp.stereo_expander.width),
                    ),
                );
                log_persist(
                    "dsp_stereo_expander_mode",
                    db_lock.set_setting(
                        "dsp_stereo_expander_mode",
                        if dsp.stereo_expander.mode == crate::audio::dsp::ExpanderMode::Surround {
                            "surround"
                        } else {
                            "hybrid"
                        },
                    ),
                );
                log_persist(
                    "dsp_stereo_balance_balance",
                    db_lock.set_setting(
                        "dsp_stereo_balance_balance",
                        &format!("{:.1}", dsp.stereo_balance.balance),
                    ),
                );
                log_persist(
                    "dsp_compressor_threshold",
                    db_lock.set_setting(
                        "dsp_compressor_threshold",
                        &format!("{:.1}", dsp.compressor.threshold),
                    ),
                );
                log_persist(
                    "dsp_compressor_intensity",
                    db_lock.set_setting(
                        "dsp_compressor_intensity",
                        &format!("{:.1}", dsp.compressor.intensity),
                    ),
                );
                log_persist(
                    "dsp_limiter_ceiling",
                    db_lock.set_setting(
                        "dsp_limiter_ceiling",
                        &format!("{:.1}", dsp.limiter.ceiling),
                    ),
                );
                log_persist(
                    "dsp_reverb_wet",
                    db_lock.set_setting("dsp_reverb_wet", &format!("{:.2}", dsp.reverb.wet)),
                );
                log_persist(
                    "dsp_reverb_room_size",
                    db_lock.set_setting(
                        "dsp_reverb_room_size",
                        &format!("{:.2}", dsp.reverb.room_size),
                    ),
                );
            });

            let state = audio_manager.state();
            let state_read = state.read();
            log_persist(
                "audio_downmix_center_enabled",
                db_lock.set_setting(
                    "audio_downmix_center_enabled",
                    if state_read.downmix_center_enabled {
                        "1"
                    } else {
                        "0"
                    },
                ),
            );
            log_persist(
                "audio_downmix_lfe_enabled",
                db_lock.set_setting(
                    "audio_downmix_lfe_enabled",
                    if state_read.downmix_lfe_enabled {
                        "1"
                    } else {
                        "0"
                    },
                ),
            );
            log_persist(
                "audio_downmix_surround_enabled",
                db_lock.set_setting(
                    "audio_downmix_surround_enabled",
                    if state_read.downmix_surround_enabled {
                        "1"
                    } else {
                        "0"
                    },
                ),
            );
            log_persist(
                "audio_downmix_center",
                db_lock.set_setting(
                    "audio_downmix_center",
                    &format!("{:.2}", state_read.downmix_center),
                ),
            );
            log_persist(
                "audio_downmix_lfe",
                db_lock.set_setting(
                    "audio_downmix_lfe",
                    &format!("{:.2}", state_read.downmix_lfe),
                ),
            );
            log_persist(
                "audio_downmix_surround",
                db_lock.set_setting(
                    "audio_downmix_surround",
                    &format!("{:.2}", state_read.downmix_surround),
                ),
            );
        } else {
            tracing::warn!("db-lock: lock no disponible");
        }
    }

    /// Guarda todos los ajustes de volumen y mezcla en la base de datos.
    pub fn save_volumen_settings_to_db(
        &self,
        audio_manager: &AudioManager,
        db: &std::sync::Mutex<crate::db::Database>,
    ) {
        // db-lock: persistencia — el guardado se omite si el lock falla
        if let Ok(db_lock) = db.lock() {
            let state = audio_manager.state();
            let s = state.read();
            log_persist(
                "vol_fades_enabled",
                db_lock.set_setting("vol_fades_enabled", if s.fades_enabled { "1" } else { "0" }),
            );
            log_persist(
                "vol_smooth_volume_enabled",
                db_lock.set_setting(
                    "vol_smooth_volume_enabled",
                    if s.smooth_volume_enabled { "1" } else { "0" },
                ),
            );
            log_persist(
                "vol_fade_in_ms",
                db_lock.set_setting("vol_fade_in_ms", &format!("{:.0}", s.fade_in_ms)),
            );
            log_persist(
                "vol_fade_out_ms",
                db_lock.set_setting("vol_fade_out_ms", &format!("{:.0}", s.fade_out_ms)),
            );
            log_persist(
                "vol_silence_enabled",
                db_lock.set_setting(
                    "vol_silence_enabled",
                    if s.silence_enabled { "1" } else { "0" },
                ),
            );
            log_persist(
                "vol_silence_duration_ms",
                db_lock.set_setting(
                    "vol_silence_duration_ms",
                    &format!("{:.0}", s.silence_duration_ms),
                ),
            );
            log_persist(
                "vol_silence_threshold_db",
                db_lock.set_setting(
                    "vol_silence_threshold_db",
                    &format!("{:.2}", s.silence_threshold_db),
                ),
            );
            log_persist(
                "vol_silence_edge_trim_enabled",
                db_lock.set_setting(
                    "vol_silence_edge_trim_enabled",
                    if s.silence_edge_trim_enabled {
                        "1"
                    } else {
                        "0"
                    },
                ),
            );
            log_persist(
                "vol_rg_fixed_enabled",
                db_lock.set_setting(
                    "vol_rg_fixed_enabled",
                    if s.rg_fixed_enabled { "1" } else { "0" },
                ),
            );
            log_persist(
                "vol_rg_fixed_db",
                db_lock.set_setting("vol_rg_fixed_db", &format!("{:.2}", s.rg_fixed_db)),
            );
            log_persist(
                "vol_fade_in_enabled",
                db_lock.set_setting(
                    "vol_fade_in_enabled",
                    if s.fade_in_enabled { "1" } else { "0" },
                ),
            );
            log_persist(
                "vol_fade_out_enabled",
                db_lock.set_setting(
                    "vol_fade_out_enabled",
                    if s.fade_out_enabled { "1" } else { "0" },
                ),
            );
            log_persist(
                "vol_rg_master_enabled",
                db_lock.set_setting(
                    "vol_rg_master_enabled",
                    if s.rg_master_enabled { "1" } else { "0" },
                ),
            );
            log_persist(
                "vol_rg_track_enabled",
                db_lock.set_setting(
                    "vol_rg_track_enabled",
                    if s.replay_gain_track_enabled {
                        "1"
                    } else {
                        "0"
                    },
                ),
            );
            log_persist(
                "vol_rg_album_enabled",
                db_lock.set_setting(
                    "vol_rg_album_enabled",
                    if s.replay_gain_album_enabled {
                        "1"
                    } else {
                        "0"
                    },
                ),
            );
            log_persist(
                "vol_rg_analyze_rt_enabled",
                db_lock.set_setting(
                    "vol_rg_analyze_rt_enabled",
                    if s.rg_analyze_rt_enabled { "1" } else { "0" },
                ),
            );
            log_persist(
                "vol_rg_offset_album_db",
                db_lock.set_setting(
                    "vol_rg_offset_album_db",
                    &format!("{:.2}", s.rg_offset_album_db),
                ),
            );
            log_persist(
                "vol_rg_offset_track_db",
                db_lock.set_setting(
                    "vol_rg_offset_track_db",
                    &format!("{:.2}", s.rg_offset_track_db),
                ),
            );
            log_persist(
                "vol_rg_offset_rt_db",
                db_lock.set_setting("vol_rg_offset_rt_db", &format!("{:.2}", s.rg_offset_rt_db)),
            );
        } else {
            tracing::warn!("db-lock: lock no disponible");
        }
    }

    /// Guarda los ajustes de mezcla cruzada (crossfade) en la base de datos.
    pub fn save_crossfade_settings_to_db(
        &self,
        audio_manager: &AudioManager,
        db: &std::sync::Mutex<crate::db::Database>,
    ) {
        // db-lock: persistencia — el guardado se omite si el lock falla
        if let Ok(db_lock) = db.lock() {
            let state = audio_manager.state();
            let s = state.read();
            log_persist(
                "crossfade_enabled",
                db_lock.set_setting(
                    "crossfade_enabled",
                    if s.crossfade_enabled { "1" } else { "0" },
                ),
            );
            log_persist(
                "crossfade_manual_enabled",
                db_lock.set_setting(
                    "crossfade_manual_enabled",
                    if s.crossfade_manual_enabled { "1" } else { "0" },
                ),
            );
            log_persist(
                "crossfade_manual_ms",
                db_lock.set_setting(
                    "crossfade_manual_ms",
                    &format!("{:.0}", s.crossfade_manual_ms),
                ),
            );
            log_persist(
                "crossfade_auto_enabled",
                db_lock.set_setting(
                    "crossfade_auto_enabled",
                    if s.crossfade_auto_enabled { "1" } else { "0" },
                ),
            );
            log_persist(
                "crossfade_auto_ms",
                db_lock.set_setting("crossfade_auto_ms", &format!("{:.0}", s.crossfade_auto_ms)),
            );
        } else {
            tracing::warn!("db-lock: lock no disponible");
        }
    }

    pub fn sync_from_engine(&mut self, audio_manager: &AudioManager) {
        if let Some(db_arc) = audio_manager.get_database() {
            // db-lock: solo lectura — refresco best-effort, se omite si el lock falla
            if let Ok(db) = db_arc.try_lock() {
                if self.selected_host.is_none() {
                    self.selected_host = db.get_setting("audio_host").filter(|s| !s.is_empty());
                }
                if self.selected_device.is_none() {
                    self.selected_device = db.get_setting("audio_device").filter(|s| !s.is_empty());
                }
                if let Some(bd_str) = db.get_setting("audio_bit_depth") {
                    if let Some(bd) = match bd_str.as_str() {
                        "16" => Some(BitDepth::Bits16),
                        "24" => Some(BitDepth::Bits24),
                        "32" => Some(BitDepth::Bits32Float),
                        _ => None,
                    } {
                        self.selected_bit_depth = bd;
                    }
                }
                if self.system_rate == SystemSelection::Default {
                    if let Some(sr_str) = db.get_setting("audio_system_rate") {
                        self.system_rate = match sr_str.as_str() {
                            "default" => SystemSelection::Default,
                            "auto" => SystemSelection::Automatic,
                            s if s.starts_with("fixed:") => {
                                if let Ok(val) = s["fixed:".len()..].parse::<u32>() {
                                    SystemSelection::Fixed(val)
                                } else {
                                    SystemSelection::Default
                                }
                            }
                            _ => SystemSelection::Default,
                        };
                    }
                }
                if self.system_quantum == SystemSelection::Default {
                    if let Some(sq_str) = db.get_setting("audio_system_quantum") {
                        self.system_quantum = match sq_str.as_str() {
                            "default" => SystemSelection::Default,
                            "auto" => SystemSelection::Automatic,
                            s if s.starts_with("fixed:") => {
                                if let Ok(val) = s["fixed:".len()..].parse::<u32>() {
                                    SystemSelection::Fixed(val)
                                } else {
                                    SystemSelection::Default
                                }
                            }
                            _ => SystemSelection::Default,
                        };
                    }
                }
                if let Some(buf_str) = db.get_setting("audio_buffer_size") {
                    self.selected_buffer_size = if buf_str == "auto" {
                        None
                    } else {
                        buf_str.parse::<u32>().ok()
                    };
                }
                // Load EQ settings from DB
                if let Some(val) = db.get_setting("eq_enabled") {
                    self.equalizer_enabled = val == "1";
                }
                if let Some(val) = db.get_setting("eq_bands_31") {
                    self.equalizer_bands_31 = val == "1";
                }
                if let Some(val) = db.get_setting("preamp_gain") {
                    if let Ok(g) = val.parse::<f32>() {
                        self.preamp_gain = g;
                    }
                }
                if let Some(val) = db.get_setting("eq_band_gains_20") {
                    let gains: Vec<f32> = val
                        .split(',')
                        .filter_map(|s| s.parse::<f32>().ok())
                        .collect();
                    if gains.len() == 20 {
                        self.eq_band_gains = gains;
                    }
                }
                if self.equalizer_bands_31 {
                    if let Some(val) = db.get_setting("eq_band_gains_31") {
                        let gains: Vec<f32> = val
                            .split(',')
                            .filter_map(|s| s.parse::<f32>().ok())
                            .collect();
                        if gains.len() == 31 {
                            self.eq_band_gains = gains;
                        }
                    }
                }
                // Load selected preset name from DB
                if let Some(preset_name) = db.get_setting("eq_selected_preset") {
                    if !preset_name.is_empty() && preset_name != "Default" {
                        // Find the preset in the unified list and select it
                        let all_presets = self.get_unified_presets();
                        if let Some(p) = all_presets.iter().find(|p| p.name == preset_name) {
                            self.selected_preset = Some((*p).clone());
                        }
                    } else {
                        self.selected_preset = None;
                    }
                }
            }
            // Load custom EQ presets from SQLite
            self.load_custom_presets(db_arc.as_ref());
        }

        // Apply loaded EQ settings to DSP engine
        audio_manager.set_eq_enabled(self.equalizer_enabled);
        audio_manager.set_preamp_gain(self.preamp_gain);
        audio_manager.set_eq_mode(if self.equalizer_bands_31 { 31 } else { 20 });

        // Apply EQ band gains to DSP (both active and saved band sets)
        let db_arc_apply = audio_manager.get_database();
        let (mut bands_20, mut bands_31) = (None::<Vec<f32>>, None::<Vec<f32>>);
        if let Some(ref db_arc) = db_arc_apply {
            // db-lock: solo lectura — refresco best-effort, se omite si el lock falla
            if let Ok(db) = db_arc.try_lock() {
                if let Some(val) = db.get_setting("eq_band_gains_20") {
                    let g: Vec<f32> = val
                        .split(',')
                        .filter_map(|s| s.parse::<f32>().ok())
                        .collect();
                    if g.len() == 20 {
                        bands_20 = Some(g);
                    }
                }
                if let Some(val) = db.get_setting("eq_band_gains_31") {
                    let g: Vec<f32> = val
                        .split(',')
                        .filter_map(|s| s.parse::<f32>().ok())
                        .collect();
                    if g.len() == 31 {
                        bands_31 = Some(g);
                    }
                }
            }
        }
        let b20 = bands_20.unwrap_or_else(|| vec![0.0; 20]);
        let b31 = bands_31.unwrap_or_else(|| vec![0.0; 31]);
        audio_manager.apply_eq_preset_gains(&b20, &b31);
        // Sync active band set into self.eq_band_gains
        let active_gains = if self.equalizer_bands_31 { &b31 } else { &b20 };
        self.eq_band_gains = active_gains.clone();

        // Load DSP and audio state settings from DB and apply
        if let Some(db_arc) = audio_manager.get_database() {
            // db-lock: solo lectura — refresco best-effort, se omite si el lock falla
            if let Ok(db) = db_arc.try_lock() {
                // DSP effects enabled states
                if let Some(val) = db.get_setting("dsp_sub_bass_enabled") {
                    audio_manager.with_dsp_mut(|dsp| dsp.sub_bass.enabled = val == "1");
                }
                if let Some(val) = db.get_setting("dsp_mid_bass_enabled") {
                    audio_manager.with_dsp_mut(|dsp| dsp.mid_bass.enabled = val == "1");
                }
                if let Some(val) = db.get_setting("dsp_voice_boost_enabled") {
                    audio_manager.with_dsp_mut(|dsp| dsp.voice_boost.enabled = val == "1");
                }
                if let Some(val) = db.get_setting("dsp_noise_gate_enabled") {
                    audio_manager.with_dsp_mut(|dsp| dsp.noise_gate.enabled = val == "1");
                }
                if let Some(val) = db.get_setting("dsp_noise_gate_threshold") {
                    if let Ok(t) = val.parse::<f32>() {
                        audio_manager.with_dsp_mut(|dsp| dsp.noise_gate.threshold = t);
                    }
                }
                if let Some(val) = db.get_setting("dsp_stereo_expander_enabled") {
                    audio_manager.with_dsp_mut(|dsp| dsp.stereo_expander.enabled = val == "1");
                }
                if let Some(val) = db.get_setting("dsp_stereo_balance_enabled") {
                    audio_manager.with_dsp_mut(|dsp| dsp.stereo_balance.enabled = val == "1");
                }
                if let Some(val) = db.get_setting("dsp_compressor_enabled") {
                    audio_manager.with_dsp_mut(|dsp| dsp.compressor.enabled = val == "1");
                }
                if let Some(val) = db.get_setting("dsp_compressor_threshold") {
                    if let Ok(t) = val.parse::<f32>() {
                        audio_manager.with_dsp_mut(|dsp| dsp.compressor.threshold = t);
                    }
                }
                if let Some(val) = db.get_setting("dsp_compressor_intensity") {
                    if let Ok(v) = val.parse::<f32>() {
                        audio_manager.with_dsp_mut(|dsp| dsp.compressor.intensity = v);
                    }
                }
                if let Some(val) = db.get_setting("dsp_limiter_enabled") {
                    audio_manager.with_dsp_mut(|dsp| dsp.limiter.enabled = val == "1");
                }
                if let Some(val) = db.get_setting("dsp_limiter_ceiling") {
                    if let Ok(t) = val.parse::<f32>() {
                        audio_manager.with_dsp_mut(|dsp| dsp.limiter.ceiling = t);
                    }
                }
                if let Some(val) = db.get_setting("dsp_reverb_enabled") {
                    audio_manager.with_dsp_mut(|dsp| dsp.reverb.enabled = val == "1");
                }
                if let Some(val) = db.get_setting("dsp_reverb_wet") {
                    if let Ok(v) = val.parse::<f32>() {
                        audio_manager.with_dsp_mut(|dsp| dsp.reverb.wet = v);
                    }
                }
                if let Some(val) = db.get_setting("dsp_reverb_room_size") {
                    if let Ok(v) = val.parse::<f32>() {
                        audio_manager.with_dsp_mut(|dsp| dsp.reverb.room_size = v);
                    }
                }

                // Audio state settings
                let state = audio_manager.state();
                let mut state_write = state.write();
                if let Some(val) = db.get_setting("audio_downmix_center_enabled") {
                    state_write.downmix_center_enabled = val == "1";
                }
                if let Some(val) = db.get_setting("audio_downmix_lfe_enabled") {
                    state_write.downmix_lfe_enabled = val == "1";
                }
                if let Some(val) = db.get_setting("audio_downmix_surround_enabled") {
                    state_write.downmix_surround_enabled = val == "1";
                }
                if let Some(val) = db.get_setting("audio_downmix_center") {
                    if let Ok(v) = val.parse::<f32>() {
                        state_write.downmix_center = v;
                    }
                }
                if let Some(val) = db.get_setting("audio_downmix_lfe") {
                    if let Ok(v) = val.parse::<f32>() {
                        state_write.downmix_lfe = v;
                    }
                }
                if let Some(val) = db.get_setting("audio_downmix_surround") {
                    if let Ok(v) = val.parse::<f32>() {
                        state_write.downmix_surround = v;
                    }
                }

                // Load Volumen y Mezcla settings
                if let Some(val) = db.get_setting("vol_fades_enabled") {
                    let enabled = val == "1";
                    state_write.fades_enabled = enabled;
                    self.volumen_fades_enabled = enabled;
                }
                if let Some(val) = db.get_setting("vol_smooth_volume_enabled") {
                    let enabled = val == "1";
                    state_write.smooth_volume_enabled = enabled;
                    self.volumen_smooth_volume_enabled = enabled;
                }
                if let Some(val) = db.get_setting("vol_fade_in_ms") {
                    if let Ok(v) = val.parse::<f64>() {
                        state_write.fade_in_ms = v as f32;
                        self.volumen_fade_in_ms = v;
                    }
                }
                if let Some(val) = db.get_setting("vol_fade_out_ms") {
                    if let Ok(v) = val.parse::<f64>() {
                        state_write.fade_out_ms = v as f32;
                        self.volumen_fade_out_ms = v;
                    }
                }
                if let Some(val) = db.get_setting("vol_silence_enabled") {
                    let enabled = val == "1";
                    state_write.silence_enabled = enabled;
                    self.volumen_silence_enabled = enabled;
                }
                if let Some(val) = db.get_setting("vol_silence_duration_ms") {
                    if let Ok(v) = val.parse::<f64>() {
                        state_write.silence_duration_ms = v as f32;
                        self.volumen_silence_duration_ms = v;
                    }
                }
                if let Some(val) = db.get_setting("vol_silence_threshold_db") {
                    if let Ok(v) = val.parse::<f64>() {
                        state_write.silence_threshold_db = v as f32;
                        self.volumen_silence_threshold_db = v;
                    }
                }
                if let Some(val) = db.get_setting("vol_silence_edge_trim_enabled") {
                    let enabled = val == "1";
                    state_write.silence_edge_trim_enabled = enabled;
                    self.volumen_silence_edge_trim_enabled = enabled;
                }
                if let Some(val) = db.get_setting("vol_rg_fixed_enabled") {
                    let enabled = val == "1";
                    state_write.rg_fixed_enabled = enabled;
                    self.volumen_rg_fixed_enabled = enabled;
                }
                if let Some(val) = db.get_setting("vol_rg_fixed_db") {
                    if let Ok(v) = val.parse::<f64>() {
                        state_write.rg_fixed_db = v as f32;
                        self.volumen_rg_fixed_db = v;
                    }
                }
                if let Some(val) = db.get_setting("vol_fade_in_enabled") {
                    let enabled = val == "1";
                    state_write.fade_in_enabled = enabled;
                    self.volumen_fade_in_enabled = enabled;
                }
                if let Some(val) = db.get_setting("vol_fade_out_enabled") {
                    let enabled = val == "1";
                    state_write.fade_out_enabled = enabled;
                    self.volumen_fade_out_enabled = enabled;
                }
                if let Some(val) = db.get_setting("vol_rg_master_enabled") {
                    let enabled = val == "1";
                    state_write.rg_master_enabled = enabled;
                    self.volumen_rg_master_enabled = enabled;
                }
                if let Some(val) = db.get_setting("vol_rg_track_enabled") {
                    let enabled = val == "1";
                    state_write.replay_gain_track_enabled = enabled;
                    self.volumen_rg_track_enabled = enabled;
                }
                if let Some(val) = db.get_setting("vol_rg_album_enabled") {
                    let enabled = val == "1";
                    state_write.replay_gain_album_enabled = enabled;
                    self.volumen_rg_album_enabled = enabled;
                }
                if let Some(val) = db.get_setting("vol_rg_analyze_rt_enabled") {
                    let enabled = val == "1";
                    state_write.rg_analyze_rt_enabled = enabled;
                    self.volumen_rg_analyze_rt_enabled = enabled;
                }
                if let Some(val) = db.get_setting("vol_rg_offset_album_db") {
                    if let Ok(v) = val.parse::<f64>() {
                        state_write.rg_offset_album_db = v as f32;
                        self.volumen_rg_offset_album_db = v;
                    }
                }
                if let Some(val) = db.get_setting("vol_rg_offset_track_db") {
                    if let Ok(v) = val.parse::<f64>() {
                        state_write.rg_offset_track_db = v as f32;
                        self.volumen_rg_offset_track_db = v;
                    }
                }
                if let Some(val) = db.get_setting("vol_rg_offset_rt_db") {
                    if let Ok(v) = val.parse::<f64>() {
                        state_write.rg_offset_rt_db = v as f32;
                        self.volumen_rg_offset_rt_db = v;
                    }
                }

                // Mezcla Cruzada / Crossfade: cargar claves desde APP_SETTINGS.
                if let Some(val) = db.get_setting("crossfade_enabled") {
                    let v = val == "1";
                    state_write.crossfade_enabled = v;
                    self.crossfade_enabled = v;
                }
                if let Some(val) = db.get_setting("crossfade_manual_enabled") {
                    let v = val == "1";
                    state_write.crossfade_manual_enabled = v;
                    self.crossfade_manual_enabled = v;
                }
                if let Some(val) = db.get_setting("crossfade_manual_ms") {
                    if let Ok(v) = val.parse::<f64>() {
                        state_write.crossfade_manual_ms = v as f32;
                        self.crossfade_manual_ms = v;
                    }
                }
                if let Some(val) = db.get_setting("crossfade_auto_enabled") {
                    let v = val == "1";
                    state_write.crossfade_auto_enabled = v;
                    self.crossfade_auto_enabled = v;
                }
                if let Some(val) = db.get_setting("crossfade_auto_ms") {
                    if let Ok(v) = val.parse::<f64>() {
                        state_write.crossfade_auto_ms = v as f32;
                        self.crossfade_auto_ms = v;
                    }
                }
            }
        }

        let state = audio_manager.state();
        let state_read = state.read();

        self.cached_hosts = audio_manager.get_available_hosts();
        self.cached_devices = audio_manager.get_devices();

        self.selected_sample_rate = Some(state_read.device_sample_rate);
        self.selected_channels_manual = state_read.channels;
        // Eq Sync
        // Presets are now initialized in default(), but we might want to select one if active
        // let eq_presets = crate::audio::preset::EqPreset::default_presets();
        // let eq_current_preset = eq_presets.first().cloned();

        self.equalizer_enabled = audio_manager.get_eq_enabled();
        let eq_bands = audio_manager.get_eq_bands_count();
        self.equalizer_bands_31 = if eq_bands == 31 { true } else { false };
        self.preamp_gain = audio_manager.get_preamp_gain();

        let mut bands = Vec::new();
        for i in 0..eq_bands {
            bands.push(audio_manager.get_eq_band_info(i).unwrap_or((0.0, 0.0)).1);
        }
        self.eq_band_gains = bands;

        // Determinar host real en base al backend de cpal, o adivinar por nombre
        if !self.cached_hosts.is_empty() {
            if self.selected_host.is_none()
                || !self
                    .cached_hosts
                    .contains(self.selected_host.as_ref().unwrap())
            {
                if self.cached_hosts.contains(&"ALSA".to_string()) {
                    self.selected_host = Some("ALSA".to_string());
                } else {
                    self.selected_host = Some(self.cached_hosts[0].clone());
                }
            }
        }

        if !self.cached_devices.is_empty() && self.selected_device.is_none() {
            let target = self
                .cached_devices
                .iter()
                .find(|d| d.name.to_lowercase().contains("pipewire"))
                .or_else(|| {
                    self.cached_devices
                        .iter()
                        .find(|d| d.name.to_lowercase().contains("pulse"))
                })
                .or_else(|| {
                    self.cached_devices
                        .iter()
                        .find(|d| d.name.to_lowercase().contains("default"))
                });

            if let Some(d) = target {
                self.selected_device = Some(d.name.clone());
            } else {
                self.selected_device = Some(self.cached_devices[0].name.clone());
            }
        }
    }
}
