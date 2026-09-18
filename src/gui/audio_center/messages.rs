//! Message vocabulary and value types for the audio center panel.

use crate::audio::device_manager::BitDepth;

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
    EqToggleSelected(bool),
    EqBandsSelected(bool), // true = 31, false = 20
    EqPreampChanged(f32),
    EqBandChanged(usize, f32),
    EqPresetSelected(crate::audio::preset::EqPreset),

    // EQ Preset Icon Buttons
    EqPresetIconLoad,
    EqPresetIconSave,
    EqPresetIconReset,
    EqPresetLoadSelected(Option<usize>),
    EqPresetLoadConfirm,
    EqPresetSaveInput(String),
    EqPresetSaveConfirm,
    EqPresetDelete(String),
    EqPresetRestoreDefaults,
    EqPresetFileImported(Option<(std::path::PathBuf, Vec<u8>)>),
    EqPresetFileExported(Option<std::path::PathBuf>),

    // Tab 3: Efectos
    DspToggle(DspEffect, bool),
    DspValueChanged(DspEffect, f32),
    AudioStateToggle(AudioStateToggle, bool),
    AudioStateValueChanged(AudioStateToggle, f32),
    StereoExpanderModeToggled(bool),
    SliderHoverActive(bool),

    // Tab 4: Volumen y Mezcla
    VolumenFadesToggle(bool),
    VolumenSmoothVolumeToggle(bool),
    VolumenFadeInToggle(bool),
    VolumenFadeOutToggle(bool),
    VolumenFadeInChanged(f64),
    VolumenFadeOutChanged(f64),
    VolumenSilenceToggle(bool),
    VolumenSilenceDurationChanged(f64),
    VolumenSilenceThresholdChanged(f64),
    VolumenSilenceEdgeTrimToggle(bool),
    VolumenRgFixedToggle(bool),
    VolumenRgFixedChanged(f64),
    VolumenRgMasterToggle(bool),
    VolumenRgTrackToggle(bool),
    VolumenRgAlbumToggle(bool),
    VolumenRgAnalyzeRtToggle(bool),
    VolumenRgOffsetAlbumChanged(f64),
    VolumenRgOffsetTrackChanged(f64),
    VolumenRgOffsetRtChanged(f64),

    // Tab 4: Mezcla Cruzada (crossfade)
    CrossfadeToggle(bool),
    CrossfadeManualToggle(bool),
    CrossfadeManualChanged(f64),
    CrossfadeAutoToggle(bool),
    CrossfadeAutoChanged(f64),
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
pub(in crate::gui::audio_center) struct OptionWrapper<T> {
    label: String,
    pub(in crate::gui::audio_center) value: T,
}
impl<T: PartialEq + Clone> OptionWrapper<T> {
    pub(in crate::gui::audio_center) fn new(label: impl Into<String>, value: T) -> Self {
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

/// Backup del estado del EQ antes de aplicar un preset en preview.
/// Se restaura si el usuario cancela el diálogo de carga.
#[derive(Debug, Clone, PartialEq)]
pub struct EqStateBackup {
    pub preamp_gain: f32,
    pub eq_band_gains: Vec<f32>,
    pub selected_preset: Option<crate::audio::preset::EqPreset>,
}
