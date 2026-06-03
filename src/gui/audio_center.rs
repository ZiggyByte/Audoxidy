use crate::audio::AudioManager;
use crate::audio::device_manager::{AudioDeviceInfo, AudioSettings, BitDepth, ChannelConfig};
use iced::{
    Alignment, Color, Element, Length, Theme,
    widget::{Space, button, column, container, mouse_area, opaque, pick_list, row, svg, text},
};
use std::sync::Arc;

// Constantes de color de acento temporales
use crate::gui::theme::*;

#[derive(Debug, Clone)]
pub enum AudioCenterMessage {
    TabSelected(usize),
    DragStart,
    // Tab 1: Config
    HostSelected(String),
    DeviceSelected(String),
    SampleRateSelected(Option<u32>),
    BitDepthSelected(BitDepth),
    ChannelsManualSelected(u16),
    BufferSizeSelected(Option<u32>),
    SystemRateSelected(SystemSelection<u32>),
    SystemQuantumSelected(SystemSelection<u32>),
    ResetToDefaults,
    ApplySettings,
    RestartService,
    Close,
    AutoUpsampleToggled(bool),
    EqToggleSelected(bool),
    EqBandsSelected(bool), // true = 31, false = 20
    EqPreampChanged(f32),
    EqBandChanged(usize, f32),
    EqPresetSelected(crate::audio::preset::EqPreset),

    // Tab 3: Efectos
    DspToggle(DspEffect, bool),
    DspValueChanged(DspEffect, f32),
    AudioStateToggle(AudioStateToggle, bool),
    AudioStateValueChanged(AudioStateToggle, f32),
    StereoExpanderModeToggled(bool),
    SliderHoverActive(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DspEffect {
    SubBass,
    MidBass,
    VoiceBoost,
    NoiseGate,
    StereoExpander,
    StereoBalance,
    Compressor,
    CompressorIntensity,
    Limiter,
    Reverb,
    ReverbRoomSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioStateToggle {
    DownmixCenter,
    DownmixLfe,
    DownmixSurround,
}

// Opciones para los pick_list (dropdowns)
#[derive(Debug, Clone, PartialEq, Eq)]
struct OptionWrapper<T> {
    label: String,
    value: T,
}
impl<T: PartialEq + Clone> OptionWrapper<T> {
    fn new(label: impl Into<String>, value: T) -> Self {
        Self {
            label: label.into(),
            value,
        }
    }
}
impl<T> std::fmt::Display for OptionWrapper<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemSelection<Val> {
    Default,
    Automatic,
    Fixed(Val),
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

    pub first_open: bool,
    pub auto_upsample: bool,

    // Configuración del servidor de audio de sistema (Pipewire / PulseAudio)
    pub system_rate: SystemSelection<u32>,
    pub system_quantum: SystemSelection<u32>,
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

            first_open: true,
            auto_upsample: false,

            system_rate: SystemSelection::Default,
            system_quantum: SystemSelection::Default,
        }
    }
}

impl AudioCenterManager {
    pub fn sync_from_engine(&mut self, audio_manager: &AudioManager) {
        if let Some(db_arc) = audio_manager.get_database() {
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
            }
        }

        let state = audio_manager.state();
        let state_read = state.read();

        self.cached_hosts = audio_manager.get_available_hosts();
        self.cached_devices = audio_manager.get_devices();

        self.selected_sample_rate = Some(state_read.device_sample_rate);
        self.selected_channels_manual = state_read.channels;
        self.auto_upsample = state_read.auto_upsample;

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

    pub fn update(&mut self, message: AudioCenterMessage, audio_manager: &AudioManager) {
        match message {
            AudioCenterMessage::TabSelected(tab) => {
                self.selected_tab = tab;
            }
            AudioCenterMessage::DragStart => {}
            AudioCenterMessage::HostSelected(host) => {
                self.selected_host = Some(host);
                self.apply_enabled = true;
            }
            AudioCenterMessage::DeviceSelected(device) => {
                self.selected_device = Some(device);
                self.apply_enabled = true;
            }
            AudioCenterMessage::SampleRateSelected(rate) => {
                self.selected_sample_rate = rate;
                self.apply_enabled = true;
            }
            AudioCenterMessage::BitDepthSelected(depth) => {
                self.selected_bit_depth = depth;
                self.apply_enabled = true;
            }
            AudioCenterMessage::ChannelsManualSelected(ch) => {
                self.selected_channels_manual = ch;
                self.apply_enabled = true;
            }
            AudioCenterMessage::BufferSizeSelected(size) => {
                self.selected_buffer_size = size;
                self.apply_enabled = true;
            }
            AudioCenterMessage::SystemRateSelected(rate) => {
                self.system_rate = rate;
                self.apply_enabled = true;
            }
            AudioCenterMessage::SystemQuantumSelected(quantum) => {
                self.system_quantum = quantum;
                self.apply_enabled = true;
            }
            AudioCenterMessage::ResetToDefaults => {
                self.selected_sample_rate = Some(48000);
                self.selected_bit_depth = BitDepth::Bits32Float;
                self.selected_buffer_size = None;
                self.selected_channels_manual = 2;
                self.system_rate = SystemSelection::Default;
                self.system_quantum = SystemSelection::Default;

                if self.cached_hosts.contains(&"ALSA".to_string()) {
                    self.selected_host = Some("ALSA".to_string());
                } else if let Some(first) = self.cached_hosts.first() {
                    self.selected_host = Some(first.clone());
                }

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
                } else if let Some(first) = self.cached_devices.first() {
                    self.selected_device = Some(first.name.clone());
                }

                self.apply_enabled = true;
            }
            AudioCenterMessage::ApplySettings => {
                // 1. Primero aplicamos la configuración del reproductor
                let settings = AudioSettings {
                    host_id: self.selected_host.clone(),
                    device_name: self.selected_device.clone(),
                    sample_rate: self.selected_sample_rate,
                    bit_depth: Some(self.selected_bit_depth.clone()),
                    channels: ChannelConfig::Manual(self.selected_channels_manual),
                    buffer_size: self.selected_buffer_size,
                    auto_upsample: self.auto_upsample,
                };
                let _ = audio_manager.apply_audio_settings(settings);

                // Guardar los ajustes en la base de datos para la persistencia
                if let Some(db_arc) = audio_manager.get_database() {
                    if let Ok(db) = db_arc.lock() {
                        let _ = db
                            .set_setting("audio_host", self.selected_host.as_deref().unwrap_or(""));
                        let _ = db.set_setting(
                            "audio_device",
                            self.selected_device.as_deref().unwrap_or(""),
                        );
                        let _ = db.set_setting(
                            "audio_sample_rate",
                            &self
                                .selected_sample_rate
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "auto".to_string()),
                        );
                        let _ = db.set_setting(
                            "audio_bit_depth",
                            match self.selected_bit_depth {
                                BitDepth::Bits16 => "16",
                                BitDepth::Bits24 => "24",
                                BitDepth::Bits32Float => "32",
                            },
                        );
                        let _ = db.set_setting(
                            "audio_channels",
                            &self.selected_channels_manual.to_string(),
                        );
                        let _ = db.set_setting(
                            "audio_buffer_size",
                            &self
                                .selected_buffer_size
                                .map(|b| b.to_string())
                                .unwrap_or_else(|| "auto".to_string()),
                        );
                        let _ = db.set_setting(
                            "audio_auto_upsample",
                            if self.auto_upsample { "true" } else { "false" },
                        );

                        let sys_rate_str = match self.system_rate {
                            SystemSelection::Default => "default".to_string(),
                            SystemSelection::Automatic => "auto".to_string(),
                            SystemSelection::Fixed(r) => format!("fixed:{}", r),
                        };
                        let _ = db.set_setting("audio_system_rate", &sys_rate_str);

                        let sys_quantum_str = match self.system_quantum {
                            SystemSelection::Default => "default".to_string(),
                            SystemSelection::Automatic => "auto".to_string(),
                            SystemSelection::Fixed(q) => format!("fixed:{}", q),
                        };
                        let _ = db.set_setting("audio_system_quantum", &sys_quantum_str);
                    }
                }

                // 2. Clonamos variables necesarias para el hilo secundario
                let state_clone = audio_manager.state();
                let system_rate = self.system_rate;
                let system_quantum = self.system_quantum;
                let selected_sample_rate = self.selected_sample_rate;
                let selected_buffer_size = self.selected_buffer_size;

                // 3. Deferimos el ajuste de Pipewire / PulseAudio al hilo secundario para esperar a que el stream se inicialice
                std::thread::spawn(move || {
                    // Esperar/Hacer polling hasta que el motor de audio inicialice el nuevo flujo
                    // y obtenga el quantum negociado de forma real (máximo 200ms, comprobando cada 10ms)
                    let mut active_state = state_clone.read().clone();
                    for _ in 0..20 {
                        if active_state.buffer_size > 0 {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        active_state = state_clone.read().clone();
                    }

                    let auto_rate = selected_sample_rate.unwrap_or(active_state.sample_rate);
                    let auto_quantum = if let Some(q) = selected_buffer_size {
                        q
                    } else if active_state.buffer_size > 0 {
                        active_state.buffer_size
                    } else {
                        // Fallback de seguridad si no hay reproducción activa en absoluto
                        let base_latency_ms = 10.0;
                        let calculated_frames =
                            (auto_rate as f64 * base_latency_ms / 1000.0) as u32;
                        if calculated_frames < 256 {
                            256
                        } else if calculated_frames < 512 {
                            512
                        } else if calculated_frames < 1024 {
                            1024
                        } else if calculated_frames < 2048 {
                            2048
                        } else if calculated_frames < 4096 {
                            4096
                        } else {
                            8192
                        }
                    };

                    // Determinar si estamos usando Pipewire
                    let check_pw = std::process::Command::new("systemctl")
                        .args(["--user", "is-active", "pipewire"])
                        .output();

                    let has_pw = match check_pw {
                        Ok(out) => String::from_utf8_lossy(&out.stdout).trim() == "active",
                        Err(_) => false,
                    };

                    if has_pw {
                        if let Ok(home) = std::env::var("HOME") {
                            let dir_path = format!("{}/.config/pipewire/pipewire.conf.d", home);
                            let file_path = format!("{}/audoxidy.conf", dir_path);

                            let rate_val = match system_rate {
                                SystemSelection::Default => None,
                                SystemSelection::Automatic => Some(auto_rate),
                                SystemSelection::Fixed(r) => Some(r),
                            };
                            let quantum_val = match system_quantum {
                                SystemSelection::Default => None,
                                SystemSelection::Automatic => Some(auto_quantum),
                                SystemSelection::Fixed(q) => Some(q),
                            };

                            if system_rate == SystemSelection::Default
                                && system_quantum == SystemSelection::Default
                            {
                                let _ = std::fs::remove_file(&file_path);
                            } else {
                                let _ = std::fs::create_dir_all(&dir_path);

                                let mut content = String::new();
                                content.push_str("context.properties = {\n");
                                if let Some(r) = rate_val {
                                    content.push_str(&format!("    default.clock.rate = {}\n", r));
                                    content.push_str(&format!(
                                        "    default.clock.allowed-rates = [ {} ]\n",
                                        r
                                    ));
                                }
                                if let Some(q) = quantum_val {
                                    content
                                        .push_str(&format!("    default.clock.quantum = {}\n", q));
                                }
                                content.push_str("}\n");

                                let _ = std::fs::write(&file_path, content);
                            }

                            // Aplicar en tiempo real a Pipewire
                            let rate_str = match rate_val {
                                Some(r) => r.to_string(),
                                None => "0".to_string(), // 0 libera/resetea la frecuencia
                            };
                            let quantum_str = match quantum_val {
                                Some(q) => q.to_string(),
                                None => "0".to_string(), // 0 libera/resetea el quantum
                            };

                            let _ = std::process::Command::new("pw-metadata")
                                .args(["-n", "settings", "0", "clock.force-rate", &rate_str])
                                .status();
                            let _ = std::process::Command::new("pw-metadata")
                                .args(["-n", "settings", "0", "clock.force-quantum", &quantum_str])
                                .status();
                        }
                    } else {
                        if let Ok(home) = std::env::var("HOME") {
                            let dir_path = format!("{}/.config/pulse", home);
                            let file_path = format!("{}/daemon.conf", dir_path);

                            let mut lines = Vec::new();
                            if std::path::Path::new(&file_path).exists() {
                                if let Ok(file_content) = std::fs::read_to_string(&file_path) {
                                    for line in file_content.lines() {
                                        let trimmed = line.trim();
                                        if trimmed.starts_with("default-sample-rate")
                                            || trimmed.starts_with("alternate-sample-rate")
                                            || trimmed.starts_with("default-fragments")
                                            || trimmed.starts_with("default-fragment-size-msec")
                                            || trimmed.starts_with("; Generated by Audoxidy")
                                        {
                                            continue;
                                        }
                                        lines.push(line.to_string());
                                    }
                                }
                            } else {
                                let _ = std::fs::create_dir_all(&dir_path);
                            }

                            let rate_val = match system_rate {
                                SystemSelection::Default => None,
                                SystemSelection::Automatic => Some(auto_rate),
                                SystemSelection::Fixed(r) => Some(r),
                            };
                            let quantum_val = match system_quantum {
                                SystemSelection::Default => None,
                                SystemSelection::Automatic => Some(auto_quantum),
                                SystemSelection::Fixed(q) => Some(q),
                            };

                            if rate_val.is_some() || quantum_val.is_some() {
                                lines.push("; Generated by Audoxidy".to_string());
                                if let Some(r) = rate_val {
                                    lines.push(format!("default-sample-rate = {}", r));
                                    lines.push(format!("alternate-sample-rate = {}", r));
                                }
                                if let Some(q) = quantum_val {
                                    let r = rate_val.unwrap_or(48000);
                                    let msec =
                                        ((q as f64 / r as f64) * 1000.0 / 2.0).max(1.0) as u32;
                                    lines.push("default-fragments = 2".to_string());
                                    lines.push(format!("default-fragment-size-msec = {}", msec));
                                }
                            }

                            let _ = std::fs::write(&file_path, lines.join("\n") + "\n");
                        }
                    }
                });

                self.apply_enabled = false;
            }
            AudioCenterMessage::RestartService => {
                // Función de reinicio de sysctl o pipewire (adaptado de legacy)
                std::thread::spawn(|| {
                    let _ = std::process::Command::new("systemctl")
                        .args([
                            "--user",
                            "restart",
                            "pipewire.service",
                            "pipewire-pulse.service",
                            "wireplumber.service",
                        ])
                        .status();
                });
            }
            AudioCenterMessage::Close => {
                self.open = false;
                self.window_pos = None;
            }
            AudioCenterMessage::AutoUpsampleToggled(enabled) => {
                self.auto_upsample = enabled;
                self.apply_enabled = true;
            }
            AudioCenterMessage::EqToggleSelected(b) => {
                self.equalizer_enabled = b;
                audio_manager.set_eq_enabled(b);
                if b {
                    audio_manager.set_preamp_gain(self.preamp_gain);
                }
            }
            AudioCenterMessage::EqBandsSelected(is_31) => {
                self.equalizer_bands_31 = is_31;
                audio_manager.set_eq_mode(if is_31 { 31 } else { 20 });
                // Resync
                let count = if is_31 { 31 } else { 20 };
                let mut bands = Vec::new();
                for i in 0..count {
                    bands.push(audio_manager.get_eq_band_info(i).unwrap_or((0.0, 0.0)).1);
                }
                self.eq_band_gains = bands;
            }
            AudioCenterMessage::EqPreampChanged(val) => {
                let clamped = val.clamp(-9.0, 9.0);
                self.preamp_gain = clamped;
                if self.equalizer_enabled {
                    audio_manager.set_preamp_gain(clamped);
                }
            }
            AudioCenterMessage::EqBandChanged(idx, val) => {
                let clamped = val.clamp(-9.0, 9.0);
                if idx < self.eq_band_gains.len() {
                    self.eq_band_gains[idx] = clamped;
                    audio_manager.set_eq_band_gain(idx, clamped);
                }
            }
            AudioCenterMessage::EqPresetSelected(preset) => {
                self.selected_preset = Some(preset.clone());
                self.preamp_gain = preset.preamp_gain;
                audio_manager.set_preamp_gain(preset.preamp_gain);
                let gains = if self.equalizer_bands_31 {
                    preset.get_gains_31()
                } else {
                    preset.get_gains_20()
                };
                for (i, &val) in gains.iter().enumerate() {
                    if i < self.eq_band_gains.len() {
                        self.eq_band_gains[i] = val;
                        audio_manager.set_eq_band_gain(i, val);
                    }
                }
            }

            // Tab 3: Effects Messages Handlers
            AudioCenterMessage::DspToggle(effect, enabled) => match effect {
                DspEffect::SubBass => {
                    audio_manager.with_dsp_mut(|dsp| dsp.sub_bass.enabled = enabled)
                }
                DspEffect::MidBass => {
                    audio_manager.with_dsp_mut(|dsp| dsp.mid_bass.enabled = enabled)
                }
                DspEffect::VoiceBoost => {
                    audio_manager.with_dsp_mut(|dsp| dsp.voice_boost.enabled = enabled)
                }
                DspEffect::NoiseGate => {
                    audio_manager.with_dsp_mut(|dsp| dsp.noise_gate.enabled = enabled)
                }
                DspEffect::StereoExpander => {
                    audio_manager.with_dsp_mut(|dsp| dsp.stereo_expander.enabled = enabled)
                }
                DspEffect::StereoBalance => {
                    audio_manager.with_dsp_mut(|dsp| dsp.stereo_balance.enabled = enabled)
                }
                DspEffect::Compressor => {
                    audio_manager.with_dsp_mut(|dsp| dsp.compressor.enabled = enabled)
                }
                DspEffect::Limiter => {
                    audio_manager.with_dsp_mut(|dsp| dsp.limiter.enabled = enabled)
                }
                DspEffect::Reverb => audio_manager.with_dsp_mut(|dsp| dsp.reverb.enabled = enabled),
                DspEffect::CompressorIntensity | DspEffect::ReverbRoomSize => {} // Secondary sliders — no independent toggle
            },
            AudioCenterMessage::DspValueChanged(effect, val) => match effect {
                DspEffect::SubBass => audio_manager.with_dsp_mut(|dsp| dsp.sub_bass.gain = val),
                DspEffect::MidBass => audio_manager.with_dsp_mut(|dsp| dsp.mid_bass.gain = val),
                DspEffect::VoiceBoost => {
                    audio_manager.with_dsp_mut(|dsp| dsp.voice_boost.gain = val)
                }
                DspEffect::NoiseGate => {
                    audio_manager.with_dsp_mut(|dsp| dsp.noise_gate.threshold = val)
                }
                DspEffect::StereoExpander => {
                    audio_manager.with_dsp_mut(|dsp| dsp.stereo_expander.width = val)
                }
                DspEffect::StereoBalance => {
                    audio_manager.with_dsp_mut(|dsp| dsp.stereo_balance.balance = val)
                }
                DspEffect::Compressor => {
                    audio_manager.with_dsp_mut(|dsp| dsp.compressor.threshold = val)
                }
                DspEffect::CompressorIntensity => audio_manager.with_dsp_mut(|dsp| {
                    dsp.compressor.intensity = val / 100.0;
                    dsp.compressor.update_intensity_params();
                }),
                DspEffect::Limiter => audio_manager.with_dsp_mut(|dsp| dsp.limiter.ceiling = val),
                DspEffect::Reverb => audio_manager.with_dsp_mut(|dsp| dsp.reverb.set_wet(val)),
                DspEffect::ReverbRoomSize => {
                    audio_manager.with_dsp_mut(|dsp| dsp.reverb.set_room_size(val))
                }
            },
            AudioCenterMessage::AudioStateToggle(toggle, enabled) => {
                let state_arc = audio_manager.state();
                let mut state = state_arc.write();
                match toggle {
                    AudioStateToggle::DownmixCenter => state.downmix_center_enabled = enabled,
                    AudioStateToggle::DownmixLfe => state.downmix_lfe_enabled = enabled,
                    AudioStateToggle::DownmixSurround => state.downmix_surround_enabled = enabled,
                }
            }
            AudioCenterMessage::AudioStateValueChanged(toggle, val) => {
                let state_arc = audio_manager.state();
                let mut state = state_arc.write();
                match toggle {
                    AudioStateToggle::DownmixCenter => state.downmix_center = val,
                    AudioStateToggle::DownmixLfe => state.downmix_lfe = val,
                    AudioStateToggle::DownmixSurround => state.downmix_surround = val,
                }
            }
            AudioCenterMessage::StereoExpanderModeToggled(is_surround) => {
                audio_manager.with_dsp_mut(|dsp| {
                    dsp.stereo_expander.mode = if is_surround {
                        crate::audio::dsp::ExpanderMode::Surround
                    } else {
                        crate::audio::dsp::ExpanderMode::Hybrid
                    };
                });
            }
            AudioCenterMessage::SliderHoverActive(_) => {
                // Manejado en app.rs para AppFocus routing
            }
        }
    }
}

pub fn view<'a>(
    manager: &'a AudioCenterManager,
    audio_manager: &'a Arc<AudioManager>,
) -> Element<'a, crate::gui::app::Message> {
    // 1. Logotipo de marca estilizado con la tipografía "Stage Wander" a 18px
    let logo = row![
        text("A")
            .color(COLOR_ACCENT)
            .font(FONT_STAGE_WANDER)
            .size(18),
        text("uD")
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_STAGE_WANDER)
            .size(18),
        text("o")
            .color(COLOR_ACCENT)
            .font(FONT_STAGE_WANDER)
            .size(18),
        text("xiD")
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_STAGE_WANDER)
            .size(18),
        text("Y")
            .color(COLOR_ACCENT)
            .font(FONT_STAGE_WANDER)
            .size(18),
    ]
    .spacing(0);

    // Logo con padding-top 9px
    let logo = container(logo).padding(iced::Padding {
        top: 9.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    });

    // 2. Botón de cerrar con icono SVG "close-big.svg" de 24px
    let close_icon = svg(iced::widget::svg::Handle::from_path(
        "assets/icons/close-big.svg",
    ))
    .width(Length::Fixed(24.0))
    .height(Length::Fixed(24.0))
    .style(|_t: &Theme, status| {
        if status == iced::widget::svg::Status::Hovered {
            iced::widget::svg::Style {
                color: Some(COLOR_ACCENT),
            }
        } else {
            iced::widget::svg::Style {
                color: Some(COLOR_TEXT_PRIMARY),
            }
        }
    });

    let close_btn = button(close_icon)
        .padding(4)
        .style(|_t: &Theme, _status| button::Style {
            background: Some(Color::TRANSPARENT.into()),
            border: iced::Border::default(),
            ..Default::default()
        })
        .on_press(crate::gui::app::Message::AudioCenterMsg(
            AudioCenterMessage::Close,
        ));

    // 3. Contenido de la cabecera con espacio y alineación vertical centrada
    let header_content = row![logo, Space::new().width(Length::Fill), close_btn,]
        .align_y(Alignment::Center)
        .padding(iced::Padding {
            top: 0.0,
            right: 6.0,
            bottom: 0.0,
            left: 15.0,
        });

    let centered_title = container(
        text("Centro de Audio Avanzado")
            .size(16)
            .font(FONT_INTER_SANS_MEDIUM)
            .color(COLOR_TEXT_PRIMARY),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(iced::Fill)
    .center_y(iced::Fill);

    // Cabecera con 40px de altura, con esquinas superiores redondeadas (8px) para acoplarse al contenedor principal
    let header = container(
        iced::widget::Stack::new()
            .push(centered_title)
            .push(header_content),
    )
    .height(Length::Fixed(40.0))
    .width(Length::Fill)
    .style(|_t: &Theme| {
        container::Style::default()
            .background(COLOR_BG)
            .border(iced::Border {
                radius: iced::border::Radius {
                    top_left: 8.0,
                    top_right: 8.0,
                    bottom_left: 0.0,
                    bottom_right: 0.0,
                },
                width: 0.0,
                color: Color::TRANSPARENT,
            })
    });

    // Barra de título arrastrable
    let header_block = mouse_area(header).on_press(crate::gui::app::Message::AudioCenterMsg(
        AudioCenterMessage::DragStart,
    ));

    // 4. Barra de pestañas (Tab Bar)
    let tab_names = ["Configuración de Audio", "Ecualizador", "Efectos de Audio"];
    let mut tab_row = row![].spacing(10);

    for (i, name) in tab_names.iter().enumerate() {
        let is_selected = manager.selected_tab == i;

        let tab_btn = button(text(*name).size(14).font(FONT_INTER_SANS_MEDIUM))
            .padding([5, 15])
            .style(move |_t: &Theme, status| {
                let is_hovered = matches!(status, button::Status::Hovered);
                button::Style {
                    background: if is_selected {
                        Some(COLOR_ACCENT.into())
                    } else {
                        Some(Color::TRANSPARENT.into())
                    },
                    text_color: if is_selected {
                        COLOR_TEXT_PRIMARY
                    } else if is_hovered {
                        COLOR_TEXT_PRIMARY
                    } else {
                        COLOR_TEXT_SECONDARY
                    },
                    border: iced::Border {
                        radius: iced::border::Radius {
                            top_left: 2.0,
                            top_right: 2.0,
                            bottom_left: 0.0,
                            bottom_right: 0.0,
                        },
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                }
            })
            .on_press(crate::gui::app::Message::AudioCenterMsg(
                AudioCenterMessage::TabSelected(i),
            ));

        tab_row = tab_row.push(tab_btn);
    }

    // Divisor
    let divider = container(Space::new().width(Length::Fill).height(Length::Fixed(2.0)))
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    // 5. Contenido de pestaña activa
    let content: Element<'a, crate::gui::app::Message> = match manager.selected_tab {
        0 => view_audio_config(manager, audio_manager),
        1 => view_equalizer(manager, audio_manager),
        2 => view_audio_effects(manager, audio_manager),
        _ => Space::new().into(),
    };

    // Contenido interno con padding de 15px en laterales y fondo
    let inner_content = column![
        tab_row,
        divider,
        content,
    ]
    .padding(iced::Padding {
        top: 0.0,
        right: 15.0,
        bottom: 15.0,
        left: 15.0,
    })
    .width(Length::Fill)
    .height(Length::Fill);

    // Contenedor principal de la ventana
    let window_layout = column![header_block, inner_content,]
        .width(Length::Fill)
        .height(Length::Fill);

    // Evitamos el traspaso de clics capturando todo en un mouse_area con NoOp
    let non_pass_through_window = mouse_area(
        container(window_layout)
            .width(Length::Fixed(940.0))
            .height(Length::Fixed(480.0))
            .padding(2.0) // Inset de 2px para que el borde del contenedor principal no sea tapado por los hijos
            .style(|_t: &Theme| {
                container::Style::default()
                    .background(COLOR_BG)
                    .border(iced::Border {
                        color: COLOR_ACCENT,
                        width: 2.0,
                        radius: 8.0.into(),
                    })
            }),
    )
    .on_press(crate::gui::app::Message::NoOp);

    // Envolvemos con opaque para que no se traspase el "focus" / "hover" del puntero a la interfaz de abajo en esta área
    opaque(non_pass_through_window).into()
}

// ==========================================
// VISTA: PESTAÑA 1 - CONFIGURACIÓN DE AUDIO
// ==========================================

fn view_audio_config<'a>(
    manager: &'a AudioCenterManager,
    audio_manager: &'a Arc<AudioManager>,
) -> Element<'a, crate::gui::app::Message> {
    // Preparar opciones de Dropdowns
    let mut host_options = Vec::new();
    for host in &manager.cached_hosts {
        host_options.push(OptionWrapper::new(host.clone(), host.clone()));
    }
    let selected_host_opt = manager
        .selected_host
        .as_ref()
        .map(|h| OptionWrapper::new(h.clone(), h.clone()));

    let mut device_options = Vec::new();
    for dev in &manager.cached_devices {
        device_options.push(OptionWrapper::new(&dev.name, dev.name.clone()));
    }
    let selected_dev_opt = manager
        .selected_device
        .as_ref()
        .map(|d| OptionWrapper::new(d, d.clone()));

    let rates = [44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000];
    let rate_options: Vec<_> = rates
        .iter()
        .map(|&r| OptionWrapper::new(format!("{} Hz", r), Some(r)))
        .collect();
    let selected_rate_opt = manager
        .selected_sample_rate
        .map(|r| OptionWrapper::new(format!("{} Hz", r), Some(r)));

    let bits = [BitDepth::Bits16, BitDepth::Bits24, BitDepth::Bits32Float];
    let bit_options: Vec<_> = bits
        .iter()
        .map(|b| {
            let label = match b {
                BitDepth::Bits16 => "16 bits Int",
                BitDepth::Bits24 => "24 bits Int",
                BitDepth::Bits32Float => "32 bits Float",
            };
            OptionWrapper::new(label, b.clone())
        })
        .collect();
    let selected_bit_opt = {
        let label = match manager.selected_bit_depth {
            BitDepth::Bits16 => "16 bits Int",
            BitDepth::Bits24 => "24 bits Int",
            BitDepth::Bits32Float => "32 bits Float",
        };
        Some(OptionWrapper::new(
            label,
            manager.selected_bit_depth.clone(),
        ))
    };

    let channels_options = vec![
        OptionWrapper::new("1.0 Mono", 1),
        OptionWrapper::new("2.0 Stereo", 2),
        OptionWrapper::new("2.1 Stereo + Sub", 3),
        OptionWrapper::new("4.0 Quad", 4),
        OptionWrapper::new("5.1 Surround", 6),
        OptionWrapper::new("7.1 Surround", 8),
    ];
    let selected_ch_opt = channels_options
        .iter()
        .find(|o| o.value == manager.selected_channels_manual)
        .cloned();

    let mut buffer_options = vec![OptionWrapper::new("Automático", None)];
    let sizes = [256, 512, 1024, 2048, 4096, 8192];
    for &s in &sizes {
        buffer_options.push(OptionWrapper::new(format!("{}", s), Some(s)));
    }
    let selected_buf_opt = if let Some(s) = manager.selected_buffer_size {
        Some(OptionWrapper::new(format!("{}", s), Some(s)))
    } else {
        Some(OptionWrapper::new("Automático", None))
    };

    let host_dropdown = crate::gui::widgets::standard_pick_list(
        host_options,
        selected_host_opt,
        |o: OptionWrapper<String>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::HostSelected(o.value))
        },
        Length::Fixed(125.0),
    );
    let device_dropdown = crate::gui::widgets::standard_pick_list(
        device_options,
        selected_dev_opt,
        |o: OptionWrapper<String>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DeviceSelected(o.value))
        },
        Length::Fixed(260.0),
    );
    let rate_dropdown = crate::gui::widgets::standard_pick_list(
        rate_options,
        selected_rate_opt,
        |o: OptionWrapper<Option<u32>>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SampleRateSelected(
                o.value,
            ))
        },
        Length::Fixed(125.0),
    );
    let bit_dropdown = crate::gui::widgets::standard_pick_list(
        bit_options,
        selected_bit_opt,
        |o: OptionWrapper<BitDepth>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::BitDepthSelected(o.value))
        },
        Length::Fixed(125.0),
    );
    let ch_dropdown = crate::gui::widgets::standard_pick_list(
        channels_options,
        selected_ch_opt,
        |o: OptionWrapper<u16>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::ChannelsManualSelected(
                o.value,
            ))
        },
        Length::Fixed(125.0),
    );
    let buffer_dropdown = crate::gui::widgets::standard_pick_list(
        buffer_options,
        selected_buf_opt,
        |o: OptionWrapper<Option<u32>>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::BufferSizeSelected(
                o.value,
            ))
        },
        Length::Fixed(125.0),
    );

    // --- PICKLISTS DE SISTEMA OPERATIVO (Pipewire | PulseAudio) ---
    let mut system_rate_options = vec![
        OptionWrapper::new("Default", SystemSelection::Default),
        OptionWrapper::new("Automatico", SystemSelection::Automatic),
    ];
    let sys_rates = [44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000];
    for &r in &sys_rates {
        system_rate_options.push(OptionWrapper::new(
            format!("{} Hz", r),
            SystemSelection::Fixed(r),
        ));
    }
    let selected_sys_rate_opt = Some(OptionWrapper::new(
        match manager.system_rate {
            SystemSelection::Default => "Default".to_string(),
            SystemSelection::Automatic => "Automatico".to_string(),
            SystemSelection::Fixed(r) => format!("{} Hz", r),
        },
        manager.system_rate,
    ));

    let mut system_quantum_options = vec![
        OptionWrapper::new("Default", SystemSelection::Default),
        OptionWrapper::new("Automatico", SystemSelection::Automatic),
    ];
    let sys_quantums = [256, 512, 1024, 2048, 4096, 8192];
    for &q in &sys_quantums {
        system_quantum_options.push(OptionWrapper::new(
            format!("{}", q),
            SystemSelection::Fixed(q),
        ));
    }
    let selected_sys_quantum_opt = Some(OptionWrapper::new(
        match manager.system_quantum {
            SystemSelection::Default => "Default".to_string(),
            SystemSelection::Automatic => "Automatico".to_string(),
            SystemSelection::Fixed(q) => format!("{}", q),
        },
        manager.system_quantum,
    ));

    let system_rate_dropdown = crate::gui::widgets::standard_pick_list(
        system_rate_options,
        selected_sys_rate_opt,
        |o: OptionWrapper<SystemSelection<u32>>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SystemRateSelected(
                o.value,
            ))
        },
        Length::Fixed(125.0),
    );

    let system_quantum_dropdown = crate::gui::widgets::standard_pick_list(
        system_quantum_options,
        selected_sys_quantum_opt,
        |o: OptionWrapper<SystemSelection<u32>>| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SystemQuantumSelected(
                o.value,
            ))
        },
        Length::Fixed(125.0),
    );

    let system_config_label = column![
        text("Pipewire | PulseAudio")
            .color(COLOR_TEXT_PRIMARY)
            .size(14)
            .font(FONT_INTER_SANS_MEDIUM),
        text("Frecuencia | Quantum")
            .color(COLOR_TEXT_SECONDARY)
            .size(13)
            .font(FONT_INTER_SANS_MEDIUM),
    ]
    .spacing(2);

    let upsampling_switch = crate::gui::widgets::standard_toggler(
        manager.auto_upsample,
        |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AutoUpsampleToggled(b)),
        16.0,
        COLOR_ACCENT,
        COLOR_TEXT_SECONDARY,
        COLOR_TEXT_PRIMARY,
        COLOR_TEXT_SECONDARY,
    );

    let left_col = column![
        row![
            container(
                text("Núcleo de Audio:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(180.0)),
            host_dropdown
        ]
        .align_y(Alignment::Center)
        .spacing(10),
        row![
            container(
                text("Dispositivo de Salida:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(180.0)),
            device_dropdown
        ]
        .align_y(Alignment::Center)
        .spacing(10),
        row![
            container(
                text("Frecuencia de Muestreo:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(180.0)),
            rate_dropdown
        ]
        .align_y(Alignment::Center)
        .spacing(10),
        row![
            container(
                text("Profundidad de Bits:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(180.0)),
            bit_dropdown
        ]
        .align_y(Alignment::Center)
        .spacing(10),
        row![
            container(
                text("Canales de Salida:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(180.0)),
            ch_dropdown
        ]
        .align_y(Alignment::Center)
        .spacing(10),
        row![
            container(
                text("Quantum (Buffer):")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(180.0)),
            buffer_dropdown
        ]
        .align_y(Alignment::Center)
        .spacing(10),
        row![
            container(
                text("Frecuencia Automática:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(180.0)),
            upsampling_switch,
            text("Alta Fidelidad (Frecuencia Máxima del Dispositivo)")
                .size(12)
                .color(COLOR_TEXT_SECONDARY)
                .font(FONT_INTER_SANS_MEDIUM)
        ]
        .align_y(Alignment::Center)
        .spacing(10),
        row![
            container(system_config_label).width(Length::Fixed(180.0)),
            row![system_rate_dropdown, system_quantum_dropdown].spacing(10)
        ]
        .align_y(Alignment::Center)
        .spacing(10),
    ]
    .spacing(15);

    // Estado del Audio Derecho - Sacamos las variables del Guard inmediatamente
    let (
        sample_rate,
        channels,
        bit_depth_display,
        buffer_size,
        current_song_title,
        current_path,
        device_sample_rate,
    ) = {
        let state_arc = audio_manager.state();
        let state_read = state_arc.read();
        (
            state_read.sample_rate,
            state_read.channels,
            state_read.bit_depth_display.clone(),
            state_read.buffer_size,
            state_read.title.clone(),
            state_read.path.clone(),
            state_read.device_sample_rate,
        )
    };

    let check_pw = std::process::Command::new("systemctl")
        .args(["--user", "is-active", "pipewire"])
        .output();

    let has_pw = match check_pw {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim() == "active",
        Err(_) => false,
    };

    let audio_server = manager
        .selected_host
        .as_deref()
        .unwrap_or("ALSA")
        .to_string();

    let db_opt = audio_manager.get_database();

    let mut file_sample_rate = sample_rate;
    let mut file_bit_depth = 0;
    let mut file_channels = 0;
    let mut track_gain = None;
    let mut album_gain = None;

    if !current_path.is_empty() {
        if let Some(ref db_arc) = db_opt {
            if let Ok(db) = db_arc.try_lock() {
                if let Ok(Some((sr, bd, ch))) = db.get_song_technical_meta_by_path(&current_path) {
                    if sr > 0 {
                        file_sample_rate = sr;
                    }
                    file_bit_depth = bd;
                    file_channels = ch;
                }
                if let Ok((tg, ag)) = db.get_replay_gain_by_path(&current_path) {
                    track_gain = tg;
                    album_gain = ag;
                }
            }
        }
    }

    let file_sr_str = if file_sample_rate > 0 {
        format!("{} Hz", file_sample_rate)
    } else {
        "--".to_string()
    };

    let file_bd_str = if file_bit_depth > 0 {
        format!("{} bits", file_bit_depth)
    } else {
        "--".to_string()
    };

    let format_channels = |ch: u16| -> String {
        match ch {
            1 => "Mono".to_string(),
            2 => "Stereo".to_string(),
            3 => "Stereo + Sub".to_string(),
            4 => "4.0 Quad".to_string(),
            6 => "5.1 Surround".to_string(),
            8 => "7.1 Surround".to_string(),
            other => format!("{} canales", other),
        }
    };

    let file_ch_str = if file_channels > 0 {
        format_channels(file_channels as u16)
    } else {
        "--".to_string()
    };

    let output_ch_str = if channels > 0 {
        format_channels(channels)
    } else {
        "--".to_string()
    };

    let bit_depth_display_clean = match bit_depth_display.as_str() {
        "24/32-bit Int" | "24/32-bits Int" | "24-bit Int" | "24 bits Int" => {
            "24 bits Int".to_string()
        }
        other => other.replace("-bit", " bits"),
    };

    let gain_str = match (album_gain, track_gain) {
        (Some(ag), Some(tg)) => format!("{:.2} dB | {:.2} dB", ag, tg),
        (Some(ag), None) => format!("{:.2} dB | --", ag),
        (None, Some(tg)) => format!("-- | {:.2} dB", tg),
        (None, None) => "-- | --".to_string(),
    };

    // Extraemos frecuencia y quantum del servidor de sonido actual
    let mut sys_rate = None;
    let mut sys_force_rate = None;
    let mut sys_quantum = None;
    let mut sys_force_quantum = None;

    if has_pw {
        if let Ok(output) = std::process::Command::new("pw-metadata")
            .args(["-n", "settings"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let extract_value = |line_str: &str| -> Option<String> {
                    if let Some(pos) = line_str.find("value:'") {
                        let start = pos + 7;
                        if let Some(end) = line_str[start..].find("'") {
                            return Some(line_str[start..start + end].to_string());
                        }
                    }
                    None
                };

                if line.contains("clock.rate") {
                    if let Some(val) = extract_value(line) {
                        sys_rate = Some(val);
                    }
                } else if line.contains("clock.force-rate") {
                    if let Some(val) = extract_value(line) {
                        sys_force_rate = Some(val);
                    }
                } else if line.contains("clock.quantum") {
                    if let Some(val) = extract_value(line) {
                        sys_quantum = Some(val);
                    }
                } else if line.contains("clock.force-quantum") {
                    if let Some(val) = extract_value(line) {
                        sys_force_quantum = Some(val);
                    }
                }
            }
        }
    }

    let active_sys_rate = if let Some(ref fr) = sys_force_rate {
        if fr != "0" {
            Some(fr.clone())
        } else {
            sys_rate.clone()
        }
    } else {
        sys_rate.clone()
    };

    let active_sys_quantum = if let Some(ref fq) = sys_force_quantum {
        if fq != "0" {
            Some(fq.clone())
        } else {
            sys_quantum.clone()
        }
    } else {
        sys_quantum.clone()
    };

    let (system_sound_status, latency_str) = if has_pw {
        let status = match (active_sys_rate.clone(), active_sys_quantum.clone()) {
            (Some(r), Some(q)) => format!("{} Hz | {}", r, q),
            (Some(r), None) => format!("{} Hz | --", r),
            (None, Some(q)) => format!("-- | {}", q),
            _ => "--".to_string(),
        };
        let lat = match (active_sys_rate, active_sys_quantum) {
            (Some(r_str), Some(q_str)) => {
                if let (Ok(r), Ok(q)) = (r_str.parse::<f64>(), q_str.parse::<f64>()) {
                    if r > 0.0 {
                        format!("Latencia: {:.2} ms", (q / r) * 1000.0)
                    } else {
                        "Latencia: -- ms".to_string()
                    }
                } else {
                    "Latencia: -- ms".to_string()
                }
            }
            _ => "Latencia: -- ms".to_string(),
        };
        (status, lat)
    } else {
        let mut pulse_rate = None;
        if let Ok(output) = std::process::Command::new("pactl").arg("info").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("Default Sample Specification:") {
                    let parts = line.split_whitespace().collect::<Vec<_>>();
                    if let Some(last) = parts.last() {
                        pulse_rate = Some(last.replace("Hz", ""));
                    }
                }
            }
        }

        let mut pulse_quantum = None;
        if let Ok(home) = std::env::var("HOME") {
            let file_path = format!("{}/.config/pulse/daemon.conf", home);
            if let Ok(content) = std::fs::read_to_string(file_path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("default-fragment-size-msec") {
                        if let Some(val) = trimmed.split('=').nth(1) {
                            pulse_quantum = Some(val.trim().to_string());
                        }
                    }
                }
            }
        }

        let status = match (pulse_rate.clone(), pulse_quantum.clone()) {
            (Some(r), Some(q)) => format!("{} Hz | {} ms", r, q),
            (Some(r), None) => format!("{} Hz | --", r),
            (None, Some(q)) => format!("-- | {} ms", q),
            _ => "--".to_string(),
        };
        let lat = if let Some(q) = pulse_quantum {
            if let Ok(q_f) = q.parse::<f64>() {
                format!("Latencia: {:.2} ms", q_f)
            } else {
                format!("Latencia: {} ms", q)
            }
        } else {
            "Latencia: -- ms".to_string()
        };
        (status, lat)
    };

    let title_el = if current_song_title.is_empty() {
        text("Sin reproducción")
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM)
            .into()
    } else {
        crate::gui::widgets::smart_truncate_text(
            current_song_title,
            14.0,
            FONT_INTER_SANS_MEDIUM,
            COLOR_TEXT_PRIMARY,
        )
    };

    let device_name_str = manager
        .selected_device
        .as_deref()
        .unwrap_or("Predeterminado");
    let device_el = crate::gui::widgets::smart_truncate_text(
        device_name_str,
        14.0,
        FONT_INTER_SANS_MEDIUM,
        COLOR_TEXT_PRIMARY,
    );

    let quantum_repr = if buffer_size > 0 {
        format!("{}", buffer_size)
    } else {
        "Auto".to_string()
    };

    let labels_col = column![
        text("").size(12),
        text("Frecuencia:")
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text("Profundidad:")
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text("Canales:")
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
    ]
    .spacing(5);

    let entrada_col = column![
        text("Entrada")
            .size(12)
            .color(COLOR_TEXT_SECONDARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text(file_sr_str)
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text(file_bd_str)
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text(file_ch_str)
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
    ]
    .spacing(5)
    .align_x(Alignment::Center);

    let vertical_divider = container(
        Space::new()
            .width(Length::Fixed(2.0))
            .height(Length::Fixed(80.0)),
    )
    .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    let salida_col = column![
        text("Salida")
            .size(12)
            .color(COLOR_TEXT_SECONDARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text(format!("{} Hz", device_sample_rate))
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text(bit_depth_display_clean)
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        text(output_ch_str)
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
    ]
    .spacing(5)
    .align_x(Alignment::Center);

    let table_row = row![
        container(labels_col).width(Length::Fixed(90.0)),
        container(entrada_col)
            .width(Length::FillPortion(1))
            .align_x(Alignment::Center),
        container(vertical_divider)
            .align_x(Alignment::Center)
            .width(Length::Fixed(20.0)),
        container(salida_col)
            .width(Length::FillPortion(1))
            .align_x(Alignment::Center),
    ]
    .align_y(Alignment::Center);

    let title_container = container(
        text("Información de Audio")
            .color(COLOR_TEXT_PRIMARY)
            .size(16)
            .font(FONT_INTER_SANS_MEDIUM),
    )
    .width(Length::Fill)
    .align_x(Alignment::Center);

    let r_col = column![
        title_container,
        row![
            container(
                text("Canción:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(100.0)),
            container(title_el).width(Length::Fill)
        ]
        .align_y(Alignment::Center),
        row![
            container(
                text("Núcleo:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(100.0)),
            text(audio_server)
                .color(COLOR_TEXT_PRIMARY)
                .size(14)
                .font(FONT_INTER_SANS_MEDIUM)
        ]
        .align_y(Alignment::Center),
        row![
            container(
                text("Dispositivo:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(100.0)),
            container(device_el).width(Length::Fill)
        ]
        .align_y(Alignment::Center),
        table_row,
        row![
            container(
                column![
                    text("Ganancia:")
                        .color(COLOR_TEXT_PRIMARY)
                        .size(14)
                        .font(FONT_INTER_SANS_MEDIUM),
                    text("Album | Cancion")
                        .color(COLOR_TEXT_SECONDARY)
                        .size(13)
                        .font(FONT_INTER_SANS_MEDIUM),
                ]
                .spacing(2)
            )
            .width(Length::Fixed(170.0)),
            text(gain_str)
                .color(COLOR_TEXT_PRIMARY)
                .size(14)
                .font(FONT_INTER_SANS_MEDIUM)
        ]
        .align_y(Alignment::Center),
        row![
            container(
                text("Quantum de Audoxidy:")
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fixed(170.0)),
            text(quantum_repr)
                .color(COLOR_TEXT_PRIMARY)
                .size(14)
                .font(FONT_INTER_SANS_MEDIUM)
        ]
        .align_y(Alignment::Center),
        row![
            container(
                column![
                    text("Pipewire | PulseAudio:")
                        .color(COLOR_TEXT_PRIMARY)
                        .size(14)
                        .font(FONT_INTER_SANS_MEDIUM),
                    text("Frecuencia | Quantum")
                        .color(COLOR_TEXT_SECONDARY)
                        .size(13)
                        .font(FONT_INTER_SANS_MEDIUM),
                ]
                .spacing(2)
            )
            .width(Length::Fixed(170.0)),
            column![
                text(system_sound_status)
                    .color(COLOR_TEXT_PRIMARY)
                    .size(14)
                    .font(FONT_INTER_SANS_MEDIUM),
                text(latency_str)
                    .color(COLOR_TEXT_SECONDARY)
                    .size(13)
                    .font(FONT_INTER_SANS_MEDIUM),
            ]
            .spacing(2)
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(10);

    let restart_btn = button(
        text("Reiniciar Servicio de Audio")
            .size(15)
            .font(FONT_INTER_SANS_MEDIUM),
    )
    .padding([8, 12])
    .on_press(crate::gui::app::Message::AudioCenterMsg(
        AudioCenterMessage::RestartService,
    ))
    .style(|_t: &Theme, status: iced::widget::button::Status| {
        let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
        button::Style {
            background: if is_hovered {
                Some(COLOR_ACCENT.into())
            } else {
                Some(COLOR_CONTRAST.into())
            },
            text_color: if is_hovered {
                COLOR_TEXT_PRIMARY
            } else {
                COLOR_TEXT_SECONDARY
            },
            border: iced::Border {
                radius: 6.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            ..Default::default()
        }
    });

    let reset_btn = button(text("Predeterminado").size(15).font(FONT_INTER_SANS_MEDIUM))
        .padding([8, 12])
        .on_press(crate::gui::app::Message::AudioCenterMsg(
            AudioCenterMessage::ResetToDefaults,
        ))
        .style(|_t: &Theme, status: iced::widget::button::Status| {
            let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
            button::Style {
                background: if is_hovered {
                    Some(COLOR_ACCENT.into())
                } else {
                    Some(COLOR_CONTRAST.into())
                },
                text_color: if is_hovered {
                    COLOR_TEXT_PRIMARY
                } else {
                    COLOR_TEXT_SECONDARY
                },
                border: iced::Border {
                    radius: 6.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..Default::default()
            }
        });

    let apply_btn = if manager.apply_enabled {
        button(text("Aplicar").size(15).font(FONT_INTER_SANS_MEDIUM))
            .padding([8, 12])
            .on_press(crate::gui::app::Message::AudioCenterMsg(
                AudioCenterMessage::ApplySettings,
            ))
            .style(|_t: &Theme, status: iced::widget::button::Status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                button::Style {
                    background: if is_hovered {
                        Some(COLOR_ACCENT.into())
                    } else {
                        Some(COLOR_CONTRAST.into())
                    },
                    text_color: if is_hovered {
                        COLOR_TEXT_PRIMARY
                    } else {
                        COLOR_TEXT_SECONDARY
                    },
                    border: iced::Border {
                        radius: 6.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                }
            })
    } else {
        button(text("Aplicar").size(15).font(FONT_INTER_SANS_MEDIUM))
            .padding([8, 12])
            .style(
                |_t: &Theme, _status: iced::widget::button::Status| button::Style {
                    background: Some(COLOR_CONTRAST.into()),
                    text_color: COLOR_TEXT_SECONDARY.scale_alpha(0.5),
                    border: iced::Border {
                        radius: 6.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                },
            )
    };

    let main_divider = container(
        Space::new()
            .width(Length::Fixed(2.0))
            .height(Length::Fixed(240.0)),
    )
    .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    let bottom_actions = row![restart_btn, reset_btn, apply_btn].spacing(10);

    column![
        row![
            container(left_col).width(Length::FillPortion(6))
            .padding(iced::Padding {
                    top: -10.0,
                    bottom: 0.0,
                    left: 0.0,
                    right: 0.0
                }),
            container(main_divider)
                .width(Length::Fixed(20.0))
                .align_x(Alignment::Center)
                .align_y(Alignment::Center),
            container(r_col)
                .width(Length::FillPortion(4))
                .padding(iced::Padding {
                    top: 0.0,
                    bottom: 0.0,
                    left: 5.0,
                    right: 1.0
                })
                .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
        ]
        .height(Length::Fill)
        .align_y(Alignment::Center),
        bottom_actions
    ]
    .into()
}

// ==========================================
// VISTA: PESTAÑA 2 - ECUALIZADOR (CUSTOM VERTICAL SLIDER)
// ==========================================

fn view_equalizer<'a>(
    manager: &'a AudioCenterManager,
    audio_manager: &'a AudioManager,
) -> Element<'a, crate::gui::app::Message> {
    let toggle_eq = row![
        text("Activar Ecualizador")
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        crate::gui::widgets::standard_toggler(
            manager.equalizer_enabled,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqToggleSelected(b)),
            18.0,
            COLOR_ACCENT,
            COLOR_TEXT_SECONDARY,
            COLOR_TEXT_PRIMARY,
            COLOR_TEXT_SECONDARY,
        )
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let bands_mode = row![
        text("Bandas:")
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        crate::gui::widgets::standard_radio("20", false, Some(manager.equalizer_bands_31), |b| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqBandsSelected(b))
        }),
        crate::gui::widgets::standard_radio("31", true, Some(manager.equalizer_bands_31), |b| {
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqBandsSelected(b))
        }),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let preset_selector = row![
        text("Preset:")
            .size(14)
            .color(COLOR_TEXT_PRIMARY)
            .font(FONT_INTER_SANS_MEDIUM),
        pick_list(
            manager.equalizer_presets.clone(),
            manager.selected_preset.clone(),
            |p| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqPresetSelected(p))
        )
        .text_size(12)
        .padding(4)
        .font(FONT_INTER_SANS_MEDIUM),
        button(text("Default").size(12).font(FONT_INTER_SANS_MEDIUM))
            .style(button::secondary)
            .on_press(crate::gui::app::Message::AudioCenterMsg(
                AudioCenterMessage::EqPresetSelected(manager.equalizer_presets[0].clone())
            )),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let top_row = row![
        container(toggle_eq)
            .width(Length::FillPortion(1))
            .align_x(Alignment::Start),
        container(bands_mode)
            .width(Length::FillPortion(1))
            .align_x(Alignment::Center),
        container(preset_selector)
            .width(Length::FillPortion(1))
            .align_x(Alignment::End),
    ]
    .align_y(Alignment::Center);

    // Contenedor Principal de Sliders
    // Preamp (1) + Eq Bands (20 / 31)

    // Custom Canvas Vertical Slider Helper (usando nuevo widget global)
    fn vertical_slider<'a>(
        label_top: String,
        label_bot: String,
        value: f32,
        on_change: impl Fn(f32) -> crate::gui::app::Message + 'a,
        on_right_click: impl Fn() -> crate::gui::app::Message + 'a,
        on_hover: impl Fn(bool) -> crate::gui::app::Message + 'a,
        eq_disabled: bool,
    ) -> Element<'a, crate::gui::app::Message> {
        let slider_base =
            crate::gui::widgets::CustomSlider::new(value, -9.0..=9.0, on_change, on_right_click)
                .orientation(crate::gui::widgets::SliderOrientation::Vertical)
                .with_arrow_keys(true)
                .format_value(|v| format!("{:.1} dB", v))
                .show_tooltip(true)
                .tooltip_font_size(12.0)
                .width(Length::Fixed(24.0))
                .height(Length::Fixed(240.0))
                .on_hover_state_change(on_hover);

        let slider = if eq_disabled {
            slider_base
                .handle_color(COLOR_CONTRAST)
                .border(0.0, COLOR_ACCENT)
                .handle_focus_color(COLOR_ACCENT)
                .handle_hover_color(COLOR_CONTRAST)
                .border_hover(1.0, COLOR_TEXT_SECONDARY)
        } else {
            slider_base
        };

        column![
            text(label_top)
                .size(10.4)
                .color(COLOR_TEXT_PRIMARY)
                .font(FONT_INTER_SANS_MEDIUM),
            Space::new().height(Length::Fixed(10.0)),
            slider,
            Space::new().height(Length::Fixed(10.0)),
            text(label_bot)
                .size(10.4)
                .color(COLOR_TEXT_PRIMARY)
                .font(FONT_INTER_SANS_MEDIUM),
        ]
        .align_x(Alignment::Center)
        .into()
    }

    let preamp_col = vertical_slider(
        "Pre".to_string(),
        format!("{:.1}", manager.preamp_gain),
        manager.preamp_gain,
        |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqPreampChanged(v)),
        || crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqPreampChanged(0.0)),
        |active| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(active)),
        !manager.equalizer_enabled,
    );

    // Bandas
    let bands_count = if manager.equalizer_bands_31 { 31 } else { 20 };
    let mut bands_row = row![].spacing(if manager.equalizer_bands_31 {
        3.98
    } else {
        20.18
    });

    for i in 0..bands_count {
        let info = audio_manager.get_eq_band_info(i).unwrap_or((0.0, 0.0));
        let freq = info.0;
        let gain = manager.eq_band_gains.get(i).copied().unwrap_or(0.0);

        let freq_label = if freq >= 1000.0 {
            format!("{:.1}k", freq / 1000.0).replace(".0k", "k")
        } else {
            format!("{:.0}", freq)
        };
        let show_top = i % 2 == 0;

        let label_top = if show_top {
            freq_label.clone()
        } else {
            " ".to_string()
        };
        let label_bot = if !show_top {
            freq_label
        } else {
            " ".to_string()
        };

        bands_row = bands_row.push(vertical_slider(
            label_top,
            label_bot,
            gain,
            move |v| {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqBandChanged(i, v))
            },
            move || {
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqBandChanged(i, 0.0))
            },
            |active| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(active)),
            !manager.equalizer_enabled,
        ));
    }

    let legend = text(
        "*Puede restablecer al valor por defecto haciendo clic secundario sobre un deslizador.",
    )
    .size(12)
    .color(COLOR_TEXT_SECONDARY)
    .font(FONT_INTER_SANS_MEDIUM);

    // Guia vertical de nivel de ganancia: -9 dB (mínimo) a +9 dB (máximo), con 0 dB como referencia de "sin cambio".
    let guide_gain = column![
        Space::new().height(Length::Fixed(25.0)),
        text("+9")
            .size(11)
            .color(COLOR_TEXT_SECONDARY)
            .font(FONT_INTER_SANS_MEDIUM),
        Space::new().height(Length::Fixed(97.0)),
        text("0")
            .size(12)
            .color(COLOR_TEXT_SECONDARY)
            .font(FONT_INTER_SANS_MEDIUM),
        Space::new().height(Length::Fixed(97.0)),
        text("-9")
            .size(11)
            .color(COLOR_TEXT_SECONDARY)
            .font(FONT_INTER_SANS_MEDIUM),
    ]
    .align_x(Alignment::Center);

    column![
        Space::new().height(Length::Fixed(15.0)),
        top_row,
        Space::new().height(Length::Fixed(25.0)),
        row![
            preamp_col,
            Space::new().width(Length::Fixed(2.0)),
            guide_gain,
            Space::new().width(Length::Fixed(2.0)),
            bands_row
        ],
        Space::new().height(Length::Fill),
        legend
    ]
    .into()
}

// ==========================================
// VISTA: PESTAÑA 3 - EFECTOS DE AUDIO
// ==========================================

fn view_audio_effects<'a>(
    _manager: &'a AudioCenterManager,
    audio_manager: &'a AudioManager,
) -> Element<'a, crate::gui::app::Message> {
    // Helper manual inline effect param constructor
    fn view_effect<'a>(
        title: &'a str,
        param_label: &'a str,
        val: f32,
        range: std::ops::RangeInclusive<f32>,
        enabled: bool,
        _default_val: f32,
        on_toggle: impl Fn(bool) -> crate::gui::app::Message + 'a,
        on_change: impl Fn(f32) -> crate::gui::app::Message + 'a,
        on_reset: crate::gui::app::Message,
        extra_widget: Option<Element<'a, crate::gui::app::Message>>,
    ) -> Element<'a, crate::gui::app::Message> {
        view_effect_with_secondary(
            title, param_label, val, range, enabled, _default_val,
            on_toggle, on_change, on_reset, extra_widget, None, None, 0.1, "{:.1}", 0.0,
        )
    }

    fn view_effect_with_secondary<'a>(
        title: &'a str,
        param_label: &'a str,
        val: f32,
        range: std::ops::RangeInclusive<f32>,
        enabled: bool,
        _default_val: f32,
        on_toggle: impl Fn(bool) -> crate::gui::app::Message + 'a,
        on_change: impl Fn(f32) -> crate::gui::app::Message + 'a,
        on_reset: crate::gui::app::Message,
        extra_widget: Option<Element<'a, crate::gui::app::Message>>,
        fixed_height: Option<f32>,
        secondary: Option<(f32, std::ops::RangeInclusive<f32>, Box<dyn Fn(f32) -> crate::gui::app::Message + 'a>, crate::gui::app::Message, &'a str, f32, &'a str)>,
        primary_step_size: f32,
        primary_fmt: &'a str,
        extra_padding_top: f32,
    ) -> Element<'a, crate::gui::app::Message> {
        let stroke_color = if enabled {
            COLOR_ACCENT
        } else {
            COLOR_CONTRAST
        };

        // Cabecera con Toggle Switch a la derecha
        let top_row = row![
            text(title)
                .size(13)
                .color(COLOR_TEXT_PRIMARY)
                .font(FONT_INTER_SANS_MEDIUM),
            Space::new().width(Length::Fixed(6.0)),
            if let Some(w) = extra_widget {
                w
            } else {
                Space::new().into()
            },
            Space::new().width(Length::Fill),
            crate::gui::widgets::standard_toggler(
                enabled,
                on_toggle,
                17.0,
                COLOR_ACCENT,
                COLOR_TEXT_SECONDARY,
                COLOR_BG,
                COLOR_TEXT_SECONDARY,
            )
        ]
        .align_y(Alignment::Center);

        // --- NUEVO CUSTOM SLIDER ---
        let on_reset_msg = on_reset.clone();
        let mut primary_opts = crate::gui::widgets::CustomSliderOptions::default();
        primary_opts.step_size = primary_step_size;
        primary_opts.enable_colored_track = true;
        primary_opts.enable_arrow_keys = true;
        primary_opts.track_color = Some(COLOR_BG);
        let param_slider = crate::gui::widgets::CustomSlider::new(
            val,
            range.clone(),
            on_change,
            move || on_reset_msg.clone(), // Reset con clic derecho
        )
        .orientation(crate::gui::widgets::SliderOrientation::Horizontal)
        .width(Length::Fill)
        .height(Length::Fixed(18.0))
        .options(primary_opts)
        .with_keyboard_input(true)
        .input_width_fixed(40.0)
        .input_height_fixed(18.0)
        .input_align(crate::gui::widgets::InputAlign::Center)
        .input_border_color(COLOR_BG)
        .on_hover_state_change(|active| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(active)));
        let param_slider = if primary_fmt == "{:.2}" {
            param_slider.format_value(|v| format!("{:.2}", v))
        } else {
            param_slider
        };

        // --- FILA INFERIOR REESTRUCTURADA ---
        let bottom_row = row![
            text(param_label)
                .size(11)
                .color(COLOR_TEXT_SECONDARY)
                .font(FONT_INTER_SANS_MEDIUM),
            Space::new().width(Length::Fixed(10.0)),
            param_slider,
        ]
        .align_y(Alignment::Center);

        // Secondary slider row (optional)
        let secondary_row: Option<Element<_>> = secondary.map(|(s_val, s_range, s_change, s_reset, s_label, s_step, s_fmt)| {
            let mut sec_opts = crate::gui::widgets::CustomSliderOptions::default();
            sec_opts.step_size = s_step;
            sec_opts.enable_colored_track = true;
            sec_opts.enable_arrow_keys = true;
            sec_opts.track_color = Some(COLOR_BG);
            let sec_slider = crate::gui::widgets::CustomSlider::new(
                s_val,
                s_range,
                s_change,
                move || s_reset.clone(),
            )
            .orientation(crate::gui::widgets::SliderOrientation::Horizontal)
            .width(Length::Fill)
            .height(Length::Fixed(18.0))
            .options(sec_opts)
            .with_keyboard_input(true)
            .input_width_fixed(40.0)
            .input_height_fixed(18.0)
            .input_align(crate::gui::widgets::InputAlign::Center)
            .input_border_color(COLOR_BG)
            .on_hover_state_change(|active| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SliderHoverActive(active)));
            let sec_slider = if s_fmt == "{:.2}" {
                sec_slider.format_value(|v| format!("{:.2}", v))
            } else {
                sec_slider
            };
            row![
                text(s_label)
                    .size(11)
                    .color(COLOR_TEXT_SECONDARY)
                    .font(FONT_INTER_SANS_MEDIUM),
                Space::new().width(Length::Fixed(10.0)),
                sec_slider,
            ]
            .align_y(Alignment::Center)
            .into()
        });

        // Build column - top row then spacer then slider rows
        let mut col = column![
            top_row,
            Space::new().height(Length::Fixed(9.0 + extra_padding_top)),
            bottom_row
        ];
        if let Some(sr) = secondary_row {
            col = col.push(iced::widget::Space::new().height(Length::Fixed(6.0)));
            col = col.push(sr);
        }

        let c = container(col)
        .padding([12, 10])
        .style(move |_t: &Theme| {
            container::Style::default()
                .background(COLOR_CONTRAST)
                .border(iced::Border {
                    color: stroke_color,
                    width: 1.0,
                    radius: 8.0.into(),
                })
         });
        let c: Element<_> = if let Some(h) = fixed_height {
            c.height(Length::Fixed(h)).into()
        } else {
            c.into()
        };
        c
    }

    let (
        sub_bass_enabled,
        sub_bass_gain,
        mid_bass_enabled,
        mid_bass_gain,
        voice_boost_enabled,
        voice_boost_gain,
        noise_gate_enabled,
        noise_gate_threshold,
        stereo_expander_enabled,
        stereo_expander_width,
        stereo_balance_enabled,
        stereo_balance_balance,
        compressor_enabled,
        compressor_threshold,
        compressor_intensity,
        limiter_enabled,
        limiter_ceiling,
        reverb_enabled,
        reverb_wet,
        reverb_room_size,
        stereo_expander_mode,
    ) = audio_manager.with_dsp(|dsp| {
        (
            dsp.sub_bass.enabled,
            dsp.sub_bass.gain,
            dsp.mid_bass.enabled,
            dsp.mid_bass.gain,
            dsp.voice_boost.enabled,
            dsp.voice_boost.gain,
            dsp.noise_gate.enabled,
            dsp.noise_gate.threshold,
            dsp.stereo_expander.enabled,
            dsp.stereo_expander.width,
            dsp.stereo_balance.enabled,
            dsp.stereo_balance.balance,
            dsp.compressor.enabled,
            dsp.compressor.threshold,
            dsp.compressor.intensity,
            dsp.limiter.enabled,
            dsp.limiter.ceiling,
            dsp.reverb.enabled,
            dsp.reverb.wet,
            dsp.reverb.room_size,
            dsp.stereo_expander.mode,
        )
    });

    let state_arc = audio_manager.state();
    let audio_s = state_arc.read();

    // Construir Sub-Paneles manuales a través del helper.
    let col1 = column![
        view_effect(
            "Refuerzo de Sub-Graves",
            "Nivel dB",
            sub_bass_gain,
            -4.0..=24.0,
            sub_bass_enabled,
            0.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::SubBass,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::SubBass,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::SubBass,
                0.0
            )),
            None
        ),
        view_effect(
            "Reducción de Ruido",
            "Umbral dB",
            noise_gate_threshold,
            -85.0..=-10.0,
            noise_gate_enabled,
            -60.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::NoiseGate,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::NoiseGate,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::NoiseGate,
                -60.0
            )),
            None
        ),
        view_effect_with_secondary(
            "Compresor",
            "Umbral dB",
            compressor_threshold,
            -24.0..=0.0,
            compressor_enabled,
            -3.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::Compressor,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::Compressor,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::Compressor,
                -3.0
            )),
            None,
            Some(92.0),
            Some((compressor_intensity * 100.0, 0.0..=100.0,
                Box::new(|v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                    DspEffect::CompressorIntensity, v
                ))),
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                    DspEffect::CompressorIntensity, 50.0
                )),
                "Intensidad %",
                1.0,
                "{:.1}",
            )),
            0.1,
            "{:.1}",
            0.0,
        ),
        container(
            container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
                .style(|_t: &Theme| container::Style::default().background(COLOR_TEXT_SECONDARY))
        )
        .padding(iced::Padding {
            top: 8.0,
            right: 0.0,
            bottom: 8.0,
            left: 0.0,
        }),
        view_effect_with_secondary(
            "Canal de Subwoofer",
            "Mix %",
            audio_s.downmix_lfe,
            0.0..=2.0,
            audio_s.downmix_lfe_enabled,
            0.66,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateToggle(
                AudioStateToggle::DownmixLfe,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(
                AudioCenterMessage::AudioStateValueChanged(AudioStateToggle::DownmixLfe, v)
            ),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateValueChanged(
                AudioStateToggle::DownmixLfe,
                0.66
            )),
            None,
            None,
            None,
            0.01,
            "{:.2}",
            0.0,
        )
    ]        
    .spacing(15)
    .width(Length::FillPortion(1));

    let col2 = column![
        view_effect(
            "Refuerzo de Graves",
            "Nivel dB",
            mid_bass_gain,
            -4.0..=15.0,
            mid_bass_enabled,
            0.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::MidBass,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::MidBass,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::MidBass,
                0.0
            )),
            None
        ),
        view_effect(
            "Expansor Estéreo",
            "Ancho %",
            stereo_expander_width * 100.0,
            0.0..=260.0,
            stereo_expander_enabled,
            100.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::StereoExpander,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::StereoExpander,
                v / 100.0
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::StereoExpander,
                1.0
            )),
            Some(
                row![
                    text("Natural").size(10).color(COLOR_TEXT_SECONDARY),
                    crate::gui::widgets::standard_toggler(
                        stereo_expander_mode == crate::audio::dsp::ExpanderMode::Surround,
                        |b| crate::gui::app::Message::AudioCenterMsg(
                            AudioCenterMessage::StereoExpanderModeToggled(b)
                        ),
                        13.0,
                        COLOR_ACCENT,
                        COLOR_TEXT_SECONDARY,
                        COLOR_TEXT_PRIMARY,
                        COLOR_TEXT_SECONDARY,
                    ),
                    text("Surround").size(10).color(COLOR_TEXT_SECONDARY),
                ]
                .spacing(5)
                .align_y(Alignment::Center)
                .into()
            )
        ),
        view_effect_with_secondary(
            "Limitador",
            "Techo dB",
            limiter_ceiling,
            -12.0..=0.0,
            limiter_enabled,
            -1.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::Limiter,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::Limiter,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::Limiter,
                -1.0
            )),
            None,
            Some(92.0),
            None,
            0.1,
            "{:.1}",
            12.0,
        ),
        container(
            text("Volumen de canales en mezcla menor a 5.1")
                .size(12)
                .color(COLOR_TEXT_PRIMARY)
                .font(FONT_INTER_SANS_MEDIUM)
        )
        .width(Length::Fill)
        .center_x(Length::Fill)
        .padding(iced::Padding {
            top: 1.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        }),
        view_effect_with_secondary(
            "Canal Central",
            "Mix %",
            audio_s.downmix_center,
            0.0..=2.0,
            audio_s.downmix_center_enabled,
            0.74,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateToggle(
                AudioStateToggle::DownmixCenter,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(
                AudioCenterMessage::AudioStateValueChanged(AudioStateToggle::DownmixCenter, v)
            ),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateValueChanged(
                AudioStateToggle::DownmixCenter,
                0.74
            )),
            None,
            None,
            None,
            0.01,
            "{:.2}",
            0.0,
        )
    ]
    .spacing(15)
    .width(Length::FillPortion(1));

    let col3 = column![
        view_effect(
            "Refuerzo de Voces",
            "Nivel dB",
            voice_boost_gain,
            -4.0..=13.0,
            voice_boost_enabled,
            0.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::VoiceBoost,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::VoiceBoost,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::VoiceBoost,
                0.0
            )),
            None
        ),
        view_effect(
            "Balance Estéreo",
            "Left | Right",
            stereo_balance_balance,
            -1.0..=1.0,
            stereo_balance_enabled,
            0.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::StereoBalance,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::StereoBalance,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::StereoBalance,
                0.0
            )),
            None
        ),
        view_effect_with_secondary(
            "Reverberación",
            "Mix",
            reverb_wet,
            0.0..=1.0,
            reverb_enabled,
            0.5,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(
                DspEffect::Reverb,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::Reverb,
                v
            )),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                DspEffect::Reverb,
                0.5
            )),
            None,
            Some(92.0),
            Some((reverb_room_size, 0.0..=1.0,
                Box::new(|v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                    DspEffect::ReverbRoomSize, v
                ))),
                crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(
                    DspEffect::ReverbRoomSize, 0.5
                )),
                "Tamaño",
                0.01,
                "{:.2}",
            )),
            0.01,
            "{:.2}",
            0.0,
        ),
        container(
            container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
                .style(|_t: &Theme| container::Style::default().background(COLOR_TEXT_SECONDARY))
        )
        .padding(iced::Padding {
            top: 8.0,
            right: 0.0,
            bottom: 8.0,
            left: 0.0,
        }),
        view_effect_with_secondary(
            "Canales Surround  SL/SR | SBL/SBR",
            "Mix %",
            audio_s.downmix_surround,
            0.0..=2.0,
            audio_s.downmix_surround_enabled,
            0.81,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateToggle(
                AudioStateToggle::DownmixSurround,
                b
            )),
            |v| crate::gui::app::Message::AudioCenterMsg(
                AudioCenterMessage::AudioStateValueChanged(AudioStateToggle::DownmixSurround, v)
            ),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateValueChanged(
                AudioStateToggle::DownmixSurround,
                0.81
            )),
            None,
            None,
            None,
            0.01,
            "{:.2}",
            0.0,
        )
    ]
    .spacing(15)
    .width(Length::FillPortion(1));

    column![
        Space::new().height(Length::Fixed(15.0)),
        iced::widget::scrollable(
            row![
                col1,
                Space::new().width(Length::Fixed(15.0)),
                col2,
                Space::new().width(Length::Fixed(15.0)),
                col3
            ]
            .width(Length::Fill)
        )
        .height(Length::Fill)
    ]
    .padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    })
    .into()
}
