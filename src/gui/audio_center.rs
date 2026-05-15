use iced::{
    widget::{button, checkbox, column, container, pick_list, row, slider, text, toggler, Space},
    Alignment, Color, Element, Length, Rectangle, Theme,
};
use std::sync::Arc;
use crate::audio::engine::{AudioSettings, BitDepth, ChannelConfig, AudioDeviceInfo};
use crate::audio::AudioManager;

// Constantes de color de acento temporales
use crate::gui::theme::*;

#[derive(Debug, Clone)]
pub enum AudioCenterMessage {
    TabSelected(usize),
    // Tab 1: Config
    HostSelected(String),
    DeviceSelected(String),
    SampleRateSelected(Option<u32>),
    BitDepthSelected(BitDepth),
    ChannelsManualSelected(u16),
    BufferSizeSelected(Option<u32>),
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
    Limiter,
    Reverb,
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
        Self { label: label.into(), value }
    }
}
impl<T> std::fmt::Display for OptionWrapper<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)
    }
}

pub struct AudioCenterManager {
    pub open: bool,
    pub selected_tab: usize,
    
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
}

impl Default for AudioCenterManager {
    fn default() -> Self {
        Self {
            open: false,
            selected_tab: 0,
            
            cached_hosts: Vec::new(),
            cached_devices: Vec::new(),
            
            selected_host: None,
            selected_device: None,
            selected_sample_rate: None,
            selected_bit_depth: BitDepth::Bits32Float,
            selected_buffer_size: Some(1024),
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
        }
    }
}

impl AudioCenterManager {
    pub fn sync_from_engine(&mut self, audio_manager: &AudioManager) {
        let state = audio_manager.state();
        let state_read = state.read();
        
        self.cached_hosts = audio_manager.get_available_hosts();
        self.cached_devices = audio_manager.get_devices();

        self.selected_sample_rate = Some(state_read.sample_rate);
        self.selected_buffer_size = if state_read.buffer_size > 0 { Some(state_read.buffer_size) } else { None };
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
             if self.selected_host.is_none() || !self.cached_hosts.contains(self.selected_host.as_ref().unwrap()) {
                 if self.cached_hosts.contains(&"ALSA".to_string()) {
                     self.selected_host = Some("ALSA".to_string());
                 } else {
                     self.selected_host = Some(self.cached_hosts[0].clone());
                 }
             }
        }
        
        if !self.cached_devices.is_empty() && self.selected_device.is_none() {
             let target = self.cached_devices.iter().find(|d| d.name.to_lowercase().contains("pipewire"))
                     .or_else(|| self.cached_devices.iter().find(|d| d.name.to_lowercase().contains("pulse")))
                     .or_else(|| self.cached_devices.iter().find(|d| d.name.to_lowercase().contains("default")));
             
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
            AudioCenterMessage::ResetToDefaults => {
                self.selected_sample_rate = Some(48000);
                self.selected_bit_depth = BitDepth::Bits32Float;
                self.selected_buffer_size = None;
                self.selected_channels_manual = 2;
                
                if self.cached_hosts.contains(&"ALSA".to_string()) {
                    self.selected_host = Some("ALSA".to_string());
                } else if let Some(first) = self.cached_hosts.first() {
                    self.selected_host = Some(first.clone());
                }
                
                let target = self.cached_devices.iter().find(|d| d.name.to_lowercase().contains("pipewire"))
                    .or_else(|| self.cached_devices.iter().find(|d| d.name.to_lowercase().contains("pulse")))
                    .or_else(|| self.cached_devices.iter().find(|d| d.name.to_lowercase().contains("default")));
                
                if let Some(d) = target { 
                    self.selected_device = Some(d.name.clone()); 
                } else if let Some(first) = self.cached_devices.first() { 
                    self.selected_device = Some(first.name.clone()); 
                }
                
                self.apply_enabled = true;
            }
            AudioCenterMessage::ApplySettings => {
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
                self.apply_enabled = false;
            }
            AudioCenterMessage::RestartService => {
                // Función de reinicio de sysctl o pipewire (adaptado de legacy)
                std::thread::spawn(|| {
                    let _ = std::process::Command::new("systemctl")
                        .args(["--user", "restart", "pipewire.service", "pipewire-pulse.service", "wireplumber.service"])
                        .status();
                });
            }
            AudioCenterMessage::Close => {
                self.open = false;
            }
            AudioCenterMessage::AutoUpsampleToggled(enabled) => {
                self.auto_upsample = enabled;
                self.apply_enabled = true;
            }
            AudioCenterMessage::EqToggleSelected(b) => {
                self.equalizer_enabled = b;
                audio_manager.set_eq_enabled(b);
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
                if self.equalizer_enabled { audio_manager.set_preamp_gain(clamped); }
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
                let gains = if self.equalizer_bands_31 { preset.get_gains_31() } else { preset.get_gains_20() };
                for (i, &val) in gains.iter().enumerate() {
                    if i < self.eq_band_gains.len() {
                        self.eq_band_gains[i] = val;
                        audio_manager.set_eq_band_gain(i, val);
                    }
                }
            }
            
            // Tab 3: Effects Messages Handlers
            AudioCenterMessage::DspToggle(effect, enabled) => {
                match effect {
                    DspEffect::SubBass => audio_manager.with_dsp_mut(|dsp| dsp.sub_bass.enabled = enabled),
                    DspEffect::MidBass => audio_manager.with_dsp_mut(|dsp| dsp.mid_bass.enabled = enabled),
                    DspEffect::VoiceBoost => audio_manager.with_dsp_mut(|dsp| dsp.voice_boost.enabled = enabled),
                    DspEffect::NoiseGate => audio_manager.with_dsp_mut(|dsp| dsp.noise_gate.enabled = enabled),
                    DspEffect::StereoExpander => audio_manager.with_dsp_mut(|dsp| dsp.stereo_expander.enabled = enabled),
                    DspEffect::StereoBalance => audio_manager.with_dsp_mut(|dsp| dsp.stereo_balance.enabled = enabled),
                    DspEffect::Compressor => audio_manager.with_dsp_mut(|dsp| dsp.compressor.enabled = enabled),
                    DspEffect::Limiter => audio_manager.with_dsp_mut(|dsp| dsp.limiter.enabled = enabled),
                    DspEffect::Reverb => audio_manager.with_dsp_mut(|dsp| dsp.reverb.enabled = enabled),
                }
            }
            AudioCenterMessage::DspValueChanged(effect, val) => {
                match effect {
                    DspEffect::SubBass => audio_manager.with_dsp_mut(|dsp| dsp.sub_bass.gain = val),
                    DspEffect::MidBass => audio_manager.with_dsp_mut(|dsp| dsp.mid_bass.gain = val),
                    DspEffect::VoiceBoost => audio_manager.with_dsp_mut(|dsp| dsp.voice_boost.gain = val),
                    DspEffect::NoiseGate => audio_manager.with_dsp_mut(|dsp| dsp.noise_gate.threshold = val),
                    DspEffect::StereoExpander => audio_manager.with_dsp_mut(|dsp| dsp.stereo_expander.width = val),
                    DspEffect::StereoBalance => audio_manager.with_dsp_mut(|dsp| dsp.stereo_balance.balance = val),
                    DspEffect::Compressor => audio_manager.with_dsp_mut(|dsp| dsp.compressor.threshold = val),
                    DspEffect::Limiter => audio_manager.with_dsp_mut(|dsp| dsp.limiter.ceiling = val),
                    DspEffect::Reverb => audio_manager.with_dsp_mut(|dsp| dsp.reverb.set_wet(val)),
                }
            }
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
        }
    }
}


pub fn view<'a>(
    manager: &'a AudioCenterManager,
    audio_manager: &'a Arc<AudioManager>
) -> Element<'a, crate::gui::app::Message> {
    
    let tabs = row![
        button(text("Configuración de Audio").size(16).font(FONT_INTER_SANS_MEDIUM))
            .style(if manager.selected_tab == 0 { button::primary } else { button::secondary })
            .on_press(crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::TabSelected(0))),
        button(text("Ecualizador").size(16).font(FONT_INTER_SANS_MEDIUM))
            .style(if manager.selected_tab == 1 { button::primary } else { button::secondary })
            .on_press(crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::TabSelected(1))),
        button(text("Efectos de Audio").size(16).font(FONT_INTER_SANS_MEDIUM))
            .style(if manager.selected_tab == 2 { button::primary } else { button::secondary })
            .on_press(crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::TabSelected(2))),
    ].spacing(10);
    
    let top_bar = row![
        tabs,
        Space::new().width(Length::Fill),
        button(text("X").size(20).color(Color::WHITE))
            .style(button::danger)
            .on_press(crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::Close))
    ].align_y(Alignment::Center);

    let content: Element<'a, crate::gui::app::Message> = match manager.selected_tab {
        0 => view_audio_config(manager, audio_manager),
        1 => view_equalizer(manager, audio_manager),
        2 => view_audio_effects(manager, audio_manager),
        _ => Space::new().into(),
    };

    container(
        column![
            top_bar,
            Space::new().height(Length::Fixed(20.0)),
            content,
        ].width(Length::Fixed(900.0)).height(Length::Fixed(500.0)).padding(30)
    )
    .width(Length::Fixed(900.0))
    .height(Length::Fixed(500.0))
    .style(|_t: &Theme| container::Style::default().background(COLOR_BG).border(iced::Border::default().rounded(10.0).width(2.0).color(COLOR_ACCENT)))
    .into()
}

fn view_audio_config<'a>(
    manager: &'a AudioCenterManager,
    audio_manager: &'a Arc<AudioManager>
) -> Element<'a, crate::gui::app::Message> {
    
    // Preparar opciones de Dropdowns
    let mut host_options = Vec::new();
    for host in &manager.cached_hosts {
        host_options.push(OptionWrapper::new(host.clone(), host.clone()));
    }
    let selected_host_opt = manager.selected_host.as_ref().map(|h| OptionWrapper::new(h.clone(), h.clone()));

    let mut device_options = Vec::new();
    for dev in &manager.cached_devices {
        device_options.push(OptionWrapper::new(&dev.name, dev.name.clone()));
    }
    let selected_dev_opt = manager.selected_device.as_ref().map(|d| OptionWrapper::new(d, d.clone()));

    let rates = [44100, 48000, 88200, 96000, 176400, 192000, 352800, 384000];
    let rate_options: Vec<_> = rates.iter().map(|&r| OptionWrapper::new(format!("{} Hz", r), Some(r))).collect();
    let selected_rate_opt = manager.selected_sample_rate.map(|r| OptionWrapper::new(format!("{} Hz", r), Some(r)));

    let bits = [BitDepth::Bits16, BitDepth::Bits24, BitDepth::Bits32Float];
    let bit_options: Vec<_> = bits.iter().map(|b| {
        let label = match b {
            BitDepth::Bits16 => "16-bit Int",
            BitDepth::Bits24 => "24-bit Int",
            BitDepth::Bits32Float => "32-bit Float",
        };
        OptionWrapper::new(label, b.clone())
    }).collect();
    let selected_bit_opt = {
        let label = match manager.selected_bit_depth {
            BitDepth::Bits16 => "16-bit Int",
            BitDepth::Bits24 => "24-bit Int",
            BitDepth::Bits32Float => "32-bit Float",
        };
        Some(OptionWrapper::new(label, manager.selected_bit_depth.clone()))
    };

    let channels_options = vec![
        OptionWrapper::new("1.0 Mono", 1),
        OptionWrapper::new("2.0 Stereo", 2),
        OptionWrapper::new("2.1 Stereo + Sub", 3),
        OptionWrapper::new("4.0 Quad", 4),
        OptionWrapper::new("5.1 Surround", 6),
        OptionWrapper::new("7.1 Surround", 8),
    ];
    let selected_ch_opt = channels_options.iter().find(|o| o.value == manager.selected_channels_manual).cloned();

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

    let host_dropdown = pick_list(host_options, selected_host_opt, |o: OptionWrapper<String>| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::HostSelected(o.value))).width(Length::Fixed(150.0));
    let device_dropdown = pick_list(device_options, selected_dev_opt, |o: OptionWrapper<String>| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DeviceSelected(o.value))).width(Length::Fixed(250.0));
    let rate_dropdown = pick_list(rate_options, selected_rate_opt, |o: OptionWrapper<Option<u32>>| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::SampleRateSelected(o.value))).width(Length::Fixed(150.0));
    let bit_dropdown = pick_list(bit_options, selected_bit_opt, |o: OptionWrapper<BitDepth>| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::BitDepthSelected(o.value))).width(Length::Fixed(150.0));
    let ch_dropdown = pick_list(channels_options, selected_ch_opt, |o: OptionWrapper<u16>| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::ChannelsManualSelected(o.value))).width(Length::Fixed(150.0));
    let buffer_dropdown = pick_list(buffer_options, selected_buf_opt, |o: OptionWrapper<Option<u32>>| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::BufferSizeSelected(o.value))).width(Length::Fixed(150.0));

    let left_col = column![
        row![container(text("Servidor de Audio:").color(COLOR_TEXT_PRIMARY).size(14).font(FONT_INTER_SANS_MEDIUM)).width(Length::Fixed(180.0)), host_dropdown].align_y(Alignment::Center).spacing(10),
        row![container(text("Dispositivo de Salida:").color(COLOR_TEXT_PRIMARY).size(14).font(FONT_INTER_SANS_MEDIUM)).width(Length::Fixed(180.0)), device_dropdown].align_y(Alignment::Center).spacing(10),
        row![container(text("Frecuencia de Muestreo:").color(COLOR_TEXT_PRIMARY).size(14).font(FONT_INTER_SANS_MEDIUM)).width(Length::Fixed(180.0)), rate_dropdown].align_y(Alignment::Center).spacing(10),
        row![container(text("Profundidad de Bits:").color(COLOR_TEXT_PRIMARY).size(14).font(FONT_INTER_SANS_MEDIUM)).width(Length::Fixed(180.0)), bit_dropdown].align_y(Alignment::Center).spacing(10),
        row![container(text("Canales de Salida:").color(COLOR_TEXT_PRIMARY).size(14).font(FONT_INTER_SANS_MEDIUM)).width(Length::Fixed(180.0)), ch_dropdown].align_y(Alignment::Center).spacing(10),
        row![container(text("Quantum (Buffer):").color(COLOR_TEXT_PRIMARY).size(14).font(FONT_INTER_SANS_MEDIUM)).width(Length::Fixed(180.0)), buffer_dropdown].align_y(Alignment::Center).spacing(10),
        row![
            container(text("Upsampling Automático:").color(COLOR_TEXT_PRIMARY).size(14).font(FONT_INTER_SANS_MEDIUM)).width(Length::Fixed(180.0)),
            checkbox(manager.auto_upsample).on_toggle(|b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AutoUpsampleToggled(b))),
            text("Alta fidelidad (reconstrucción FFT)").size(12).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM)
        ].align_y(Alignment::Center).spacing(10),
    ].spacing(20);

    // Estado del Audio Derecho - Sacamos las variables del Guard inmediatamente
    let (sample_rate, channels, bit_depth_display, buffer_size) = {
        let state_arc = audio_manager.state();
        let state_read = state_arc.read();
        (
            state_read.sample_rate,
            state_read.channels,
            state_read.bit_depth_display.clone(),
            state_read.buffer_size,
        )
    };

    let r_col = column![
        text("Estado del Audio").color(COLOR_TEXT_PRIMARY).size(18).font(FONT_INTER_SANS_MEDIUM),
        Space::new().height(Length::Fixed(10.0)),
        row![text("Backend:").color(COLOR_TEXT_SECONDARY).width(Length::Fixed(100.0)).font(FONT_INTER_SANS_MEDIUM), text("Compartido").color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM)],
        row![text("Dispositivo:").color(COLOR_TEXT_SECONDARY).width(Length::Fixed(100.0)).font(FONT_INTER_SANS_MEDIUM), text("Sistema").color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM)],
        row![text("Frecuencia:").color(COLOR_TEXT_SECONDARY).width(Length::Fixed(100.0)).font(FONT_INTER_SANS_MEDIUM), text(format!("{} Hz", sample_rate)).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM)],
        row![text("Profundidad:").color(COLOR_TEXT_SECONDARY).width(Length::Fixed(100.0)).font(FONT_INTER_SANS_MEDIUM), text(bit_depth_display).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM)],
        row![text("Canales:").color(COLOR_TEXT_SECONDARY).width(Length::Fixed(100.0)).font(FONT_INTER_SANS_MEDIUM), text(format!("{}", channels)).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM)],
        row![text("Quantum:").color(COLOR_TEXT_SECONDARY).width(Length::Fixed(100.0)).font(FONT_INTER_SANS_MEDIUM), text(if buffer_size > 0 { format!("{}", buffer_size) } else { "Auto".to_string() }).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM)],
    ].spacing(10);

    let bottom_actions = row![
        button(text("Reiniciar servicio de Audio").font(FONT_INTER_SANS_MEDIUM)).on_press(crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::RestartService)),
        button(text("Predeterminado").font(FONT_INTER_SANS_MEDIUM)).on_press(crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::ResetToDefaults)),
        if manager.apply_enabled {
            button(text("Aplicar").font(FONT_INTER_SANS_MEDIUM))
                .style(button::primary)
                .on_press(crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::ApplySettings))
        } else {
            button(text("Aplicar").font(FONT_INTER_SANS_MEDIUM))
        }
    ].spacing(15);

    column![
        row![
            container(left_col).width(Length::FillPortion(6)), 
            container(r_col).width(Length::FillPortion(4)).padding(20).style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST))
        ].height(Length::Fill),
        bottom_actions
    ].into()
}

// ==========================================
// VISTA: PESTAÑA 2 - ECUALIZADOR (CUSTOM VERTICAL SLIDER)
// ==========================================

fn view_equalizer<'a>(
    manager: &'a AudioCenterManager,
    audio_manager: &'a AudioManager,
) -> Element<'a, crate::gui::app::Message> {
    let toggle_eq = column![
        row![
            checkbox(manager.equalizer_enabled).on_toggle(|b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqToggleSelected(b))),
            text("Activar Ecualizador").size(14).font(FONT_INTER_SANS_MEDIUM)
        ].spacing(5)
    ];
        
    let bands_mode = row![
        text("Bandas:").size(14).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
        iced::widget::radio("20", false, Some(manager.equalizer_bands_31), |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqBandsSelected(b))).size(16).text_size(14).font(FONT_INTER_SANS_MEDIUM),
        iced::widget::radio("31", true, Some(manager.equalizer_bands_31), |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqBandsSelected(b))).size(16).text_size(14).font(FONT_INTER_SANS_MEDIUM),
    ].spacing(10).align_y(Alignment::Center);

    let preset_selector = row![
        text("Preset:").size(14).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
        pick_list(
            manager.equalizer_presets.clone(),
            manager.selected_preset.clone(),
            |p| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqPresetSelected(p))
        ).text_size(12).padding(4).font(FONT_INTER_SANS_MEDIUM)
    ].spacing(10).align_y(Alignment::Center);

    let default_btn = button(text("Default").size(12).font(FONT_INTER_SANS_MEDIUM)).style(button::secondary)
        .on_press(crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqPresetSelected(manager.equalizer_presets[0].clone())));
        
    let top_row = row![
        toggle_eq,
        Space::new().width(Length::Fill),
        preset_selector,
        default_btn,
        Space::new().width(Length::Fixed(20.0)),
        bands_mode
    ].spacing(15).align_y(Alignment::Center);

    // Contenedor Prinicpal de Sliders
    // Preamp (1) + Eq Bands (20 / 31)
    
    // Custom Canvas Vertical Slider Helper
    fn vertical_slider<'a>(
        label_top: String,
        label_bot: String,
        value: f32,
        on_change: impl Fn(f32) -> crate::gui::app::Message + 'a,
        on_right_click: impl Fn() -> crate::gui::app::Message + 'a,
    ) -> Element<'a, crate::gui::app::Message> {
        let slider = VerticalSlider::new(value, -9.0..=9.0, on_change, on_right_click)
            .width(Length::Fixed(24.0))
            .height(Length::Fixed(240.0));
            
        column![
            text(label_top).size(10).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
            Space::new().height(Length::Fixed(10.0)),
            slider,
            Space::new().height(Length::Fixed(10.0)),
            text(label_bot).size(10).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM),
        ].align_x(Alignment::Center).into()
    }

    let preamp_col = vertical_slider(
        "Pre".to_string(),
        format!("{:.1}", manager.preamp_gain),
        manager.preamp_gain,
        |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqPreampChanged(v)),
        || crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqPreampChanged(0.0)),
    );

    // Bandas
    let bands_count = if manager.equalizer_bands_31 { 31 } else { 20 };
    let mut bands_row = row![].spacing(if manager.equalizer_bands_31 { 6 } else { 12 });

    for i in 0..bands_count {
        let info = audio_manager.get_eq_band_info(i).unwrap_or((0.0, 0.0));
        let freq = info.0;
        let gain = manager.eq_band_gains.get(i).copied().unwrap_or(0.0);
        
        let freq_label = if freq >= 1000.0 { format!("{:.1}k", freq/1000.0).replace(".0k", "k") } else { format!("{:.0}", freq) };
        let show_top = i % 2 == 0;
        
        let label_top = if show_top { freq_label.clone() } else { " ".to_string() };
        let label_bot = if !show_top { freq_label } else { " ".to_string() };

        bands_row = bands_row.push(vertical_slider(
            label_top,
            label_bot,
            gain,
            move |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqBandChanged(i, v)),
            move || crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::EqBandChanged(i, 0.0)),
        ));
    }

    let legend = text("* Restablecer a 0 dB haciendo clic derecho sobre el deslizador.")
        .size(11)
        .color(COLOR_TEXT_SECONDARY)
        .font(FONT_INTER_SANS_MEDIUM);

    column![
        top_row,
        Space::new().height(Length::Fixed(30.0)),
        row![
            preamp_col,
            Space::new().width(Length::Fixed(20.0)),
            container(Space::new().width(Length::Fixed(1.0)).height(Length::Fixed(260.0))).style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST)),
            Space::new().width(Length::Fixed(20.0)),
            bands_row
        ],
        Space::new().height(Length::Fill),
        legend
    ].into()
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
        
        let stroke_color = if enabled { COLOR_ACCENT } else { COLOR_CONTRAST };
        
        // Cabecera con Checkbox Custom (Mock por nativo por ahora)
        let top_row = row![
            text(title).size(13).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
            Space::new().width(Length::Fixed(10.0)),
            if let Some(w) = extra_widget { w } else { Space::new().into() },
            Space::new().width(Length::Fill),
            checkbox(enabled).on_toggle(on_toggle)
        ].align_y(Alignment::Center);
        
        // Slider de Iced estándar (Horizontal) acoplado a la derecha
        let param_slider = iced::widget::slider(range, val.clone(), on_change)
            .step(0.1)
            .width(Length::Fill);
        
        let bottom_row = row![
            text(param_label).size(11).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM),
            Space::new().width(Length::Fixed(15.0)),
            param_slider,
            Space::new().width(Length::Fixed(10.0)),
            text(format!("{:.1}", val)).size(11).color(COLOR_TEXT_PRIMARY).width(Length::Fixed(25.0)).font(FONT_INTER_SANS_MEDIUM),
            button(text("R").size(8).font(FONT_INTER_SANS_MEDIUM)).padding(2).on_press(on_reset)
        ].align_y(Alignment::Center);

        container(
            column![
                top_row,
                Space::new().height(Length::Fixed(10.0)),
                bottom_row
            ]
        )
        .padding(10)
        .style(move |_t: &Theme| {
            container::Style::default()
                .background(COLOR_CONTRAST)
                .border(iced::Border {
                    color: stroke_color,
                    width: 1.0,
                    radius: 8.0.into(),
                })
        })
        .into()
    }
    
    let (
        sub_bass_enabled, sub_bass_gain,
        mid_bass_enabled, mid_bass_gain,
        voice_boost_enabled, voice_boost_gain,
        noise_gate_enabled, noise_gate_threshold,
        stereo_expander_enabled, stereo_expander_width,
        stereo_balance_enabled, stereo_balance_balance,
        compressor_enabled, compressor_threshold,
        limiter_enabled, limiter_ceiling,
        reverb_enabled, reverb_wet,
        stereo_expander_mode,
    ) = audio_manager.with_dsp(|dsp| {
        (
            dsp.sub_bass.enabled, dsp.sub_bass.gain,
            dsp.mid_bass.enabled, dsp.mid_bass.gain,
            dsp.voice_boost.enabled, dsp.voice_boost.gain,
            dsp.noise_gate.enabled, dsp.noise_gate.threshold,
            dsp.stereo_expander.enabled, dsp.stereo_expander.width,
            dsp.stereo_balance.enabled, dsp.stereo_balance.balance,
            dsp.compressor.enabled, dsp.compressor.threshold,
            dsp.limiter.enabled, dsp.limiter.ceiling,
            dsp.reverb.enabled, dsp.reverb.wet,
            dsp.stereo_expander.mode,
        )
    });
    
    let state_arc = audio_manager.state();
    let audio_s = state_arc.read();
    
    // Construir Sub-Paneles manuales a través del helper.
    let col1 = column![
        view_effect("Refuerzo de Sub-Graves", "Nivel (dB)", sub_bass_gain, -4.0..=24.0, sub_bass_enabled, 0.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(DspEffect::SubBass, b)),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::SubBass, v)),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::SubBass, 0.0)),
            None),
            
        view_effect("Reducción de Ruido", "Umbral (dB)", noise_gate_threshold, -85.0..=-10.0, noise_gate_enabled, -60.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(DspEffect::NoiseGate, b)),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::NoiseGate, v)),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::NoiseGate, -60.0)),
            None),
            
        view_effect("Compresor", "Umbral (dB)", compressor_threshold, -40.0..=0.0, compressor_enabled, -10.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(DspEffect::Compressor, b)),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::Compressor, v)),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::Compressor, -10.0)),
            None),
            
        container(Space::new().height(Length::Fixed(15.0))),
        text("Volumen Canal Central").size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
        Space::new().height(Length::Fixed(5.0)),
        view_effect("Canal Central", "Nivel (%)", audio_s.downmix_center, 0.0..=2.0, audio_s.downmix_center_enabled, 0.81,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateToggle(AudioStateToggle::DownmixCenter, b)),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateValueChanged(AudioStateToggle::DownmixCenter, v)),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateValueChanged(AudioStateToggle::DownmixCenter, 0.81)),
            None)
        
    ].spacing(15).width(Length::FillPortion(1));

    let col2 = column![
        view_effect("Refuerzo de Graves", "Nivel (dB)", mid_bass_gain, -4.0..=15.0, mid_bass_enabled, 0.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(DspEffect::MidBass, b)),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::MidBass, v)),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::MidBass, 0.0)),
            None),
            
        view_effect("Expansor Estéreo", "Ancho (%)", stereo_expander_width * 100.0, 0.0..=260.0, stereo_expander_enabled, 100.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(DspEffect::StereoExpander, b)),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::StereoExpander, v / 100.0)),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::StereoExpander, 1.0)),
            Some(row![
                text("Híbrido").size(10).color(COLOR_TEXT_SECONDARY),
                toggler(stereo_expander_mode == crate::audio::dsp::ExpanderMode::Surround)
                    .size(14)
                    .on_toggle(|b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::StereoExpanderModeToggled(b))),
                text("Surround").size(10).color(COLOR_TEXT_SECONDARY),
            ].spacing(5).align_y(Alignment::Center).into())),
            
        view_effect("Limitador", "Techo (dB)", limiter_ceiling, -12.0..=0.0, limiter_enabled, -6.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(DspEffect::Limiter, b)),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::Limiter, v)),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::Limiter, -6.0)),
            None),

        container(Space::new().height(Length::Fixed(15.0))),
        text("Volumen Subwoofer (Mezcla > 5.1)").size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
        Space::new().height(Length::Fixed(5.0)),
        view_effect("Canal de Subwoofer", "Nivel (%)", audio_s.downmix_lfe, 0.0..=2.0, audio_s.downmix_lfe_enabled, 0.66,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateToggle(AudioStateToggle::DownmixLfe, b)),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateValueChanged(AudioStateToggle::DownmixLfe, v)),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateValueChanged(AudioStateToggle::DownmixLfe, 0.66)),
            None)
            
    ].spacing(15).width(Length::FillPortion(1));

    let col3 = column![
        view_effect("Refuerzo de Voces", "Nivel (dB)", voice_boost_gain, -4.0..=13.0, voice_boost_enabled, 0.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(DspEffect::VoiceBoost, b)),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::VoiceBoost, v)),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::VoiceBoost, 0.0)),
            None),
            
        view_effect("Balance Estéreo", "L/R", stereo_balance_balance, -1.0..=1.0, stereo_balance_enabled, 0.0,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(DspEffect::StereoBalance, b)),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::StereoBalance, v)),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::StereoBalance, 0.0)),
            None),
            
        view_effect("Reverberación", "Nivel / Wet", reverb_wet, 0.0..=1.0, reverb_enabled, 0.5,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspToggle(DspEffect::Reverb, b)),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::Reverb, v)),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::DspValueChanged(DspEffect::Reverb, 0.5)),
            None),
            
        container(Space::new().height(Length::Fixed(15.0))),
        text("Volumen Surround (SL/SR SBL/SBR)").size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
        Space::new().height(Length::Fixed(5.0)),
        view_effect("Canales Surround", "Nivel (%)", audio_s.downmix_surround, 0.0..=2.0, audio_s.downmix_surround_enabled, 0.73,
            |b| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateToggle(AudioStateToggle::DownmixSurround, b)),
            |v| crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateValueChanged(AudioStateToggle::DownmixSurround, v)),
            crate::gui::app::Message::AudioCenterMsg(AudioCenterMessage::AudioStateValueChanged(AudioStateToggle::DownmixSurround, 0.73)),
            None)
            
    ].spacing(15).width(Length::FillPortion(1));

    column![
        iced::widget::scrollable(
            row![
                col1,
                Space::new().width(Length::Fixed(20.0)),
                col2,
                Space::new().width(Length::Fixed(20.0)),
                col3
            ].width(Length::Fill)
        ).height(Length::Fill)
    ]
    .padding(iced::Padding { top: 0.0, right: 10.0, bottom: 0.0, left: 10.0 })
    .into()
}

// ==========================================
// CUSTOM WIDGET: VERTICAL SLIDER (CANVAS)
// ==========================================

struct VerticalSlider<'a, Message> {
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    on_change: Box<dyn Fn(f32) -> Message + 'a>,
    on_right_click: Box<dyn Fn() -> Message + 'a>,
    width: Length,
    height: Length,
}

impl<'a, Message> VerticalSlider<'a, Message> {
    pub fn new(
        value: f32,
        range: std::ops::RangeInclusive<f32>,
        on_change: impl Fn(f32) -> Message + 'a,
        on_right_click: impl Fn() -> Message + 'a,
    ) -> Self {
        Self {
            value,
            range,
            on_change: Box::new(on_change),
            on_right_click: Box::new(on_right_click),
            width: Length::Fixed(20.0),
            height: Length::Fill,
        }
    }
    pub fn width(mut self, width: Length) -> Self { self.width = width; self }
    pub fn height(mut self, height: Length) -> Self { self.height = height; self }
}

impl<'a, Message> iced::advanced::Widget<Message, Theme, iced::Renderer> for VerticalSlider<'a, Message> {
    fn size(&self) -> iced::Size<Length> {
        iced::Size { width: self.width, height: self.height }
    }

    fn layout(&mut self, _tree: &mut iced::advanced::widget::Tree, _renderer: &iced::Renderer, limits: &iced::advanced::layout::Limits) -> iced::advanced::layout::Node {
        let size = limits.resolve(self.width, self.height, iced::Size::ZERO);
        iced::advanced::layout::Node::new(size)
    }

    fn update(
        &mut self,
        _tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        _viewport: &iced::Rectangle,
    ) {
        let bounds = layout.bounds();
        let Some(cursor_pos) = cursor.position() else { return };

        let is_hovered = bounds.contains(cursor_pos);

        match event {
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) |
            iced::Event::Touch(iced::touch::Event::FingerPressed { .. }) => {
                if is_hovered {
                    let percent = 1.0 - ((cursor_pos.y - bounds.y) / bounds.height).clamp(0.0, 1.0);
                    let new_value = self.range.start() + percent * (self.range.end() - self.range.start());
                    let snapped = (new_value * 10.0).round() / 10.0;
                    shell.publish((self.on_change)(snapped));
                    return;
                }
            }
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: _ }) |
            iced::Event::Touch(iced::touch::Event::FingerMoved { position: _, .. }) => {
                if is_hovered && cursor.is_over(bounds) {
                    let is_left_clicked = true; 
                    if is_left_clicked { 
                       // No-op for now unless stateful 
                    }
                }
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Right)) => {
                if is_hovered {
                    shell.publish((self.on_right_click)());
                    return;
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        _tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        _theme: &Theme,
        _style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        _viewport: &iced::Rectangle,
    ) {
        use iced::advanced::Renderer as _;
        let bounds = layout.bounds();
        let is_hovered = cursor.position().map(|p| bounds.contains(p)).unwrap_or(false);

        // Draw Track
        let track_width = 8.0;
        let track_x = bounds.x + (bounds.width - track_width) / 2.0;
        let track_rect = Rectangle { x: track_x, y: bounds.y, width: track_width, height: bounds.height };
        
        renderer.fill_quad(
            iced::advanced::graphics::core::renderer::Quad { bounds: track_rect, border: iced::Border { radius: 4.0.into(), ..Default::default() }, ..Default::default() },
            COLOR_CONTRAST
        );

        // Draw Handle
        let percent = (self.value - self.range.start()) / (self.range.end() - self.range.start());
        let percent = percent.clamp(0.0, 1.0);
        let handle_height = 16.0;
        let handle_width = 16.0;
        let handle_y = bounds.y + bounds.height - (percent * bounds.height) - handle_height / 2.0;
        let handle_x = bounds.x + (bounds.width - handle_width) / 2.0;
        
        let handle_rect = Rectangle { x: handle_x, y: handle_y.clamp(bounds.y, bounds.y + bounds.height - handle_height), width: handle_width, height: handle_height };

        let color = if is_hovered { COLOR_TEXT_PRIMARY } else { COLOR_ACCENT };

        renderer.fill_quad(
            iced::advanced::graphics::core::renderer::Quad { bounds: handle_rect, border: iced::Border { radius: 2.0.into(), ..Default::default() }, ..Default::default() },
            color
        );

        // Tooltip Zero-Latency
        if is_hovered {
            let val_display = format!("{:.1} dB", self.value);
            
            let tooltip_rect = Rectangle {
                x: handle_rect.x + handle_width + 8.0,
                y: handle_rect.y - 4.0,
                width: 45.0,
                height: 20.0,
            };

            renderer.fill_quad(
                iced::advanced::graphics::core::renderer::Quad { bounds: tooltip_rect, border: iced::Border { radius: 2.0.into(), ..Default::default() }, ..Default::default() },
                COLOR_CONTRAST
            );

            use iced::advanced::text::Renderer as _;

            renderer.fill_text(
                iced::advanced::text::Text {
                    content: val_display,
                    bounds: iced::Size::new(40.0, 16.0),
                    size: 11.0.into(),
                    line_height: iced::advanced::text::LineHeight::default(),
                    font: iced::Font::default(),
                    align_x: iced::alignment::Horizontal::Center.into(),
                    align_y: iced::alignment::Vertical::Center,
                    shaping: iced::advanced::text::Shaping::Basic,
                    wrapping: iced::advanced::text::Wrapping::default(),
                },
                iced::Point::new(tooltip_rect.x + 4.0, tooltip_rect.y + 2.0),
                COLOR_TEXT_PRIMARY,
                tooltip_rect,
            );
        }
    }
}

impl<'a, Message> From<VerticalSlider<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(slider: VerticalSlider<'a, Message>) -> Self {
        Element::new(slider)
    }
}
