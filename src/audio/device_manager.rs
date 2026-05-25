use super::AudioError;
use parking_lot::RwLock;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Devuelve la latencia base recomendada en milisegundos para la plataforma actual.
/// - Linux (PipeWire): 5ms (baja latencia nativa)
/// - Linux (ALSA): 10ms (estable)
/// - Windows (WASAPI): 5ms (exclusive mode)
/// - Windows (WASAPI shared): 10ms
/// - macOS (Core Audio): 5ms (muy estable)
/// - Otros: 10ms (conservador)
fn platform_base_latency_ms() -> f64 {
    #[cfg(all(target_os = "linux", feature = "pipewire"))]
    { 5.0 }
    #[cfg(all(target_os = "linux", not(feature = "pipewire")))]
    { 10.0 }
    #[cfg(target_os = "windows")]
    { 5.0 }
    #[cfg(target_os = "macos")]
    { 5.0 }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    { 10.0 }
}

/// Devuelve el tamaño de buffer mínimo (en frames) recomendado para la plataforma.
/// Valores más bajos = menor latencia pero más riesgo de underruns.
fn platform_min_buffer_frames() -> u32 {
    #[cfg(target_os = "linux")]
    { 64 }   // PipeWire/ALSA pueden manejar 64 frames
    #[cfg(target_os = "windows")]
    { 96 }   // WASAPI exclusivo soporta 96
    #[cfg(target_os = "macos")]
    { 64 }   // Core Audio muy estable a 64
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    { 128 }  // Default conservador
}

// --- Device-related types extracted from engine.rs ---

#[derive(Clone, Debug, PartialEq, Copy)]
pub enum BitDepth {
    Bits16,
    Bits24,
    Bits32Float,
}

impl Default for BitDepth {
    fn default() -> Self {
        Self::Bits32Float
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChannelConfig {
    Auto,
    Manual(u16),
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self::Manual(2)
    }
}

#[derive(Clone, Debug, Default)]
pub struct AudioSettings {
    pub host_id: Option<String>,
    pub device_name: Option<String>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<BitDepth>,
    pub channels: ChannelConfig,
    pub buffer_size: Option<u32>,
    pub auto_upsample: bool,
}

#[derive(Clone, Debug)]
pub struct AudioDeviceInfo {
    pub name: String,
    #[allow(dead_code)]
    pub supported_configs: Vec<cpal::SupportedStreamConfigRange>,
}

struct AudioOutput {
    host: cpal::Host,
    device: cpal::Device,
    stream_config: cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    stream: Option<cpal::Stream>,
}

/// Gestor de dispositivos de audio CPAL.
/// Encapsula toda la lógica de selección de host, dispositivo, formato de stream,
/// y creación de streams de salida de audio.
pub struct AudioDeviceManager {
    output: RwLock<Option<AudioOutput>>,
}

impl AudioDeviceManager {
    pub fn new() -> Self {
        Self {
            output: RwLock::new(None),
        }
    }

    pub fn set_output(&self, host: cpal::Host, device: cpal::Device, config: cpal::StreamConfig, fmt: cpal::SampleFormat) {
        let mut out = self.output.write();
        *out = Some(AudioOutput {
            host,
            device,
            stream_config: config,
            sample_format: fmt,
            stream: None,
        });
    }

    pub fn take_output(&self) -> Option<(cpal::Host, cpal::Device, cpal::StreamConfig, cpal::SampleFormat)> {
        self.output.write().take().map(|o| (o.host, o.device, o.stream_config, o.sample_format))
    }

    pub fn get_stream_config(&self) -> Option<cpal::StreamConfig> {
        self.output.read().as_ref().map(|o| o.stream_config.clone())
    }

    pub fn get_device(&self) -> Option<cpal::Device> {
        self.output.read().as_ref().map(|o| o.device.clone())
    }

    pub fn get_sample_format(&self) -> Option<cpal::SampleFormat> {
        self.output.read().as_ref().map(|o| o.sample_format)
    }

    pub fn has_stream(&self) -> bool {
        self.output.read().as_ref().and_then(|o| o.stream.as_ref()).is_some()
    }

    pub fn init_default_output(&self) -> Result<(cpal::Host, cpal::Device, cpal::StreamConfig, cpal::SampleFormat), AudioError> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or(AudioError::NoDevice)?;
        let config = device
            .default_output_config()
            .map_err(|e| AudioError::DeviceError(e.to_string()))?;

        let mut stream_config: cpal::StreamConfig = config.clone().into();
        stream_config.buffer_size = cpal::BufferSize::Default;
        stream_config.channels = 2;

        // Intentar con defaults, fallback a config del dispositivo
        let sample_format = config.sample_format();
        if self.configure_output(
            host,
            device.clone(),
            stream_config.clone(),
            sample_format,
        ).is_err() {
            let host_fallback = cpal::default_host();
            let fmt = config.sample_format();
            let def_conf: cpal::StreamConfig = config.into();
            self.configure_output(host_fallback, device.clone(), def_conf.clone(), fmt)?;
            return Ok((cpal::default_host(), device, def_conf, fmt));
        }

        Ok((cpal::default_host(), device, stream_config, sample_format))
    }

    fn configure_output(
        &self,
        host: cpal::Host,
        device: cpal::Device,
        config: cpal::StreamConfig,
        sample_format: cpal::SampleFormat,
    ) -> Result<(), AudioError> {
        let mut out = self.output.write();
        *out = Some(AudioOutput {
            host,
            device,
            stream_config: config,
            sample_format,
            stream: None,
        });
        Ok(())
    }

    pub fn start_stream<F>(
        &self,
        build_stream: F,
    ) -> Result<(), AudioError>
    where
        F: FnOnce(&cpal::Device, &cpal::StreamConfig, cpal::SampleFormat) -> Result<cpal::Stream, AudioError>,
    {
        let mut out_lock = self.output.write();
        if let Some(output) = out_lock.as_mut() {
            if output.stream.is_some() {
                return Ok(());
            }
            let stream = build_stream(&output.device, &output.stream_config, output.sample_format)?;
            stream.play().map_err(|e| AudioError::StreamError(e.to_string()))?;
            output.stream = Some(stream);
        }
        Ok(())
    }

    pub fn stop_stream(&self) {
        let mut out = self.output.write();
        if let Some(o) = out.as_mut() {
            o.stream = None;
        }
    }

    pub fn get_available_hosts() -> Vec<String> {
        cpal::available_hosts()
            .iter()
            .map(|id| id.name().to_string())
            .collect()
    }

    pub fn get_devices(&self) -> Vec<AudioDeviceInfo> {
        let host_name = self.output.read().as_ref().map(|o| o.host.id().name())
            .unwrap_or_else(|| cpal::default_host().id().name());

        let host_id = cpal::available_hosts()
            .into_iter()
            .find(|h| h.name() == host_name)
            .unwrap_or(cpal::default_host().id());
        let host = cpal::host_from_id(host_id).unwrap_or(cpal::default_host());

        if let Ok(devices) = host.output_devices() {
            devices
                .map(|d| {
                    #[allow(deprecated)]
                    let name = d.name().unwrap_or_else(|_| "Unknown".into());
                    let supported_configs = d
                        .supported_output_configs()
                        .map(|c| c.collect())
                        .unwrap_or_default();
                    AudioDeviceInfo { name, supported_configs }
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Configuración avanzada del device según preferencias de AudioSettings.
    /// Devuelve la mejor config range encontrada y el stream_config resultante.
    pub fn resolve_settings(
        &self,
        settings: &AudioSettings,
        _current_out_rate: u32,
    ) -> Result<(cpal::Host, cpal::Device, cpal::StreamConfig, cpal::SampleFormat), AudioError> {
        let target_host_id = if let Some(ref name) = settings.host_id {
            cpal::available_hosts()
                .into_iter()
                .find(|h| h.name() == name)
                .ok_or(AudioError::HostNotFound)?
        } else {
            cpal::default_host().id()
        };

        let host = cpal::host_from_id(target_host_id)
            .map_err(|e| AudioError::DeviceError(e.to_string()))?;

        let device = if let Some(ref dev_name) = settings.device_name {
            #[allow(deprecated)]
            host.output_devices()
                .map_err(|e| AudioError::DeviceError(e.to_string()))?
                .find(|d| d.name().unwrap_or_default() == *dev_name)
                .ok_or(AudioError::DeviceNotFound)?
        } else {
            host.default_output_device().ok_or(AudioError::NoDevice)?
        };

        let mut supported_configs: Vec<_> = device
            .supported_output_configs()
            .map_err(|e| AudioError::DeviceError(e.to_string()))?
            .collect();

        if let Some(bd) = &settings.bit_depth {
            let filtered: Vec<_> = supported_configs
                .iter()
                .cloned()
                .filter(|c| match bd {
                    BitDepth::Bits16 => {
                        c.sample_format() == cpal::SampleFormat::I16
                            || c.sample_format() == cpal::SampleFormat::U16
                    }
                    BitDepth::Bits24 => c.sample_format() == cpal::SampleFormat::I32,
                    BitDepth::Bits32Float => c.sample_format() == cpal::SampleFormat::F32,
                })
                .collect();
            if !filtered.is_empty() {
                supported_configs = filtered;
            } else {
                tracing::warn!("Requested bit depth not supported, falling back");
            }
        }

        let req_channels = match settings.channels {
            ChannelConfig::Auto => 2,
            ChannelConfig::Manual(c) => c,
        };
        let req_rate = settings.sample_rate;

        let best = supported_configs
            .iter()
            .fold(None, |best, current| {
                let channel_score = if current.channels() == req_channels { 100 }
                    else if current.channels() > req_channels { 50 } else { 0 };
                let rate_score = req_rate.map_or(
                    if (current.min_sample_rate()..=current.max_sample_rate()).contains(&44100) { 10 } else { 0 },
                    |r| if r >= current.min_sample_rate() && r <= current.max_sample_rate() { 100 } else { 0 },
                );
                let total = channel_score + rate_score;
                match best {
                    Some((s, _)) if total > s => Some((total, current)),
                    None => Some((total, current)),
                    _ => best,
                }
            })
            .map(|(_, c)| c)
            .ok_or(AudioError::ConfigError("No valid config found".into()))?;

        let target_rate = req_rate.unwrap_or_else(|| {
            let min = best.min_sample_rate();
            let max = best.max_sample_rate();
            if min <= 44100 && max >= 44100 { 44100 }
            else if min <= 48000 && max >= 48000 { 48000 }
            else { max }
        });

        let target_rate = if settings.auto_upsample {
            let max_rate = best.max_sample_rate();
            if max_rate > target_rate {
                tracing::info!("Auto-upsample: {} Hz -> {} Hz", target_rate, max_rate);
                max_rate
            } else { target_rate }
        } else { target_rate };

        let config = best.with_sample_rate(target_rate);
        let mut stream_config: cpal::StreamConfig = config.clone().into();

        if let Some(frames) = settings.buffer_size {
            stream_config.buffer_size = cpal::BufferSize::Fixed(frames);
        } else {
            // Latencia adaptativa por plataforma (5.2)
            let base_latency_ms = platform_base_latency_ms();
            let min_frames = platform_min_buffer_frames();
            let calculated = (target_rate as f64 * base_latency_ms / 1000.0) as u32;
            let quantum = calculated.max(min_frames);
            // Redondear al quantum estándar más cercano
            let quantum = if quantum <= 64 { 64 }
                else if quantum <= 96 { 96 }
                else if quantum <= 128 { 128 }
                else if quantum <= 192 { 192 }
                else if quantum <= 256 { 256 }
                else if quantum <= 512 { 512 }
                else if quantum <= 1024 { 1024 }
                else if quantum <= 2048 { 2048 }
                else if quantum <= 4096 { 4096 }
                else { 8192 };
            stream_config.buffer_size = cpal::BufferSize::Fixed(quantum);
        }

        Ok((host, device, stream_config, best.sample_format()))
    }
}
