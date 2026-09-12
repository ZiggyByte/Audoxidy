use super::AudioError;
use parking_lot::RwLock;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Returns the recommended base latency in milliseconds for the current platform.
/// - Linux (ALSA/PipeWire): 10ms (stable)
/// - Windows (WASAPI): 5ms (exclusive mode)
/// - Windows (WASAPI shared): 10ms
/// - macOS (Core Audio): 5ms (very stable)
/// - Others: 10ms (conservative)
fn platform_base_latency_ms() -> f64 {
    #[cfg(target_os = "linux")]
    {
        10.0
    }
    #[cfg(target_os = "windows")]
    {
        5.0
    }
    #[cfg(target_os = "macos")]
    {
        5.0
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        10.0
    }
}

/// Devuelve el tamaño de buffer mínimo (en frames) recomendado para la plataforma.
/// Valores más bajos = menor latencia pero más riesgo de underruns.
fn platform_min_buffer_frames() -> u32 {
    #[cfg(target_os = "linux")]
    {
        64
    } // PipeWire/ALSA pueden manejar 64 frames
    #[cfg(target_os = "windows")]
    {
        96
    } // WASAPI exclusivo soporta 96
    #[cfg(target_os = "macos")]
    {
        64
    } // Core Audio muy estable a 64
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        128
    } // Default conservador
}

// --- Device-related types extracted from engine.rs ---

/// Profundidad de bits para la salida de audio.
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

/// Configuración de canales de salida.
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

/// Configuración completa del dispositivo de audio.
#[derive(Clone, Debug, Default)]
pub struct AudioSettings {
    pub host_id: Option<String>,
    /// Nombre visible heredado del dispositivo; se conserva para la coincidencia
    /// de compatibilidad cuando aún no hay un id estable guardado.
    pub device_name: Option<String>,
    /// Identificador estable del dispositivo (`DeviceId` serializado), preferido
    /// sobre el nombre porque sobrevive a renombrados y reconexiones.
    pub device_id: Option<String>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<BitDepth>,
    pub channels: ChannelConfig,
    pub buffer_size: Option<u32>,
}

/// Información de un dispositivo de audio disponible.
#[derive(Clone, Debug)]
pub struct AudioDeviceInfo {
    pub name: String,
    /// Id estable del dispositivo (`DeviceId` serializado), usado para persistir
    /// la selección de salida.
    pub id: Option<String>,
    #[allow(dead_code)]
    pub supported_configs: Vec<cpal::SupportedStreamConfigRange>,
}

/// Indica si el callback de salida sabe convertir un formato de muestra.
///
/// Solo se aceptan los formatos que `cpal::Sample::from_sample::<f32>()` puede
/// convertir: `F32` (copia directa desde el anillo `f32`), `F64`, `I16`, `I32`
/// y `U16`. Los formatos de 24/8 bits, los enteros sin signo de 32 bits y los
/// flujos DSD quedan fuera y se rechazan con un error nombrado.
fn accepted_sample_format(fmt: cpal::SampleFormat) -> bool {
    matches!(
        fmt,
        cpal::SampleFormat::F32
            | cpal::SampleFormat::F64
            | cpal::SampleFormat::I16
            | cpal::SampleFormat::I32
            | cpal::SampleFormat::U16
    )
}

/// Nombre legible del dispositivo para los errores de formato de muestra.
fn device_display_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|desc| desc.name().to_string())
        .unwrap_or_else(|_| "desconocido".into())
}

/// Resultado de resolver la salida guardada frente a los dispositivos presentes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceLookup {
    /// El id estable guardado coincide con un dispositivo (índice).
    StableId(usize),
    /// El nombre heredado coincide; conviene volver a persistir el id estable.
    LegacyName(usize),
    /// No hay una salida guardada: se usa la predeterminada.
    Default,
    /// La salida guardada ya no existe: se usa la predeterminada y se avisa.
    Missing,
}

/// Determina qué dispositivo usar a partir de la salida guardada y del conjunto
/// de dispositivos presentes, sin realizar E/S.
///
/// Orden de resolución: id estable -> nombre heredado -> predeterminado. Si
/// había una salida guardada y ninguna coincide, devuelve `Missing` para que el
/// llamador use el predeterminado y muestre un aviso no modal.
fn choose_device_lookup(
    saved_id: Option<&str>,
    saved_name: Option<&str>,
    present: &[(String, String)],
) -> DeviceLookup {
    let has_saved_id = saved_id.is_some_and(|s| !s.is_empty());
    let has_saved_name = saved_name.is_some_and(|s| !s.is_empty());

    if let Some(id) = saved_id.filter(|s| !s.is_empty()) {
        if let Some(index) = present.iter().position(|(present_id, _)| present_id == id) {
            return DeviceLookup::StableId(index);
        }
    }

    if let Some(name) = saved_name.filter(|s| !s.is_empty()) {
        if let Some(index) = present
            .iter()
            .position(|(_, present_name)| present_name == name)
        {
            return DeviceLookup::LegacyName(index);
        }
    }

    if has_saved_id || has_saved_name {
        DeviceLookup::Missing
    } else {
        DeviceLookup::Default
    }
}

/// Puntúa una configuración de salida aceptada para elegir la mejor entre las
/// alternativas cuando no hay una coincidencia exacta.
///
/// Prioriza el formato sin conversión (`F32`/`F64`) por encima de los enteros,
/// y dentro de cada grupo la cercanía al número de canales y a la tasa pedidos.
/// Los pesos garantizan que un grupo de formato nunca quede por debajo de otro
/// (la diferencia entre grupos supera la suma de los otros dos criterios).
fn score_output_range(
    range: &cpal::SupportedStreamConfigRange,
    preferred_rate: u32,
    preferred_channels: u16,
) -> u32 {
    let format_score = match range.sample_format() {
        cpal::SampleFormat::F32 => 400,
        cpal::SampleFormat::F64 => 300,
        _ => 200,
    };

    let channels = range.channels();
    let channel_score = if channels == preferred_channels {
        40
    } else if channels > preferred_channels {
        20
    } else {
        20u32.saturating_sub(u32::from(preferred_channels - channels))
    };

    let rate_score = if range.contains_rate(preferred_rate) {
        40
    } else {
        let min = range.min_sample_rate();
        let max = range.max_sample_rate();
        let distance = if preferred_rate < min {
            min - preferred_rate
        } else {
            preferred_rate - max
        };
        // ~1 punto por cada kHz de cercanía, con un mínimo de 1.
        (40u32.saturating_sub(distance / 1000)).max(1)
    };

    format_score + channel_score + rate_score
}

/// Elige una salida cuyo formato de muestra sea convertible.
///
/// Orden de preferencia: una configuración `F32` en la tasa y canales pedidos
/// (sin conversión en el callback); la configuración por defecto del
/// dispositivo si su formato es aceptable; la mejor configuración aceptada
/// según [`score_output_range`]. Si ninguna lo es, devuelve
/// `UnsupportedSampleFormat` nombrando el dispositivo y el formato rechazado,
/// en lugar de dejar la salida muda.
fn select_accepted_output(
    device: &cpal::Device,
    preferred_rate: u32,
    preferred_channels: u16,
) -> Result<(cpal::StreamConfig, cpal::SampleFormat), AudioError> {
    let mut fallback: Option<(u32, cpal::SupportedStreamConfigRange)> = None;
    let mut rejected: Option<cpal::SampleFormat> = None;

    if let Ok(ranges) = device.supported_output_configs() {
        for range in ranges {
            if !accepted_sample_format(range.sample_format()) {
                rejected.get_or_insert(range.sample_format());
                continue;
            }
            if range.sample_format() == cpal::SampleFormat::F32
                && range.channels() == preferred_channels
                && range.contains_rate(preferred_rate)
            {
                let cfg: cpal::StreamConfig = range.with_sample_rate(preferred_rate).into();
                return Ok((cfg, cpal::SampleFormat::F32));
            }
            // El orden de `supported_output_configs` no está garantizado
            // best-first: se guarda la mejor candidata por puntuación en vez de
            // la primera que aparezca.
            let score = score_output_range(&range, preferred_rate, preferred_channels);
            match fallback {
                Some((best_score, _)) if best_score >= score => {}
                _ => fallback = Some((score, range)),
            }
        }
    }

    if let Ok(default) = device.default_output_config() {
        if accepted_sample_format(default.sample_format()) {
            return Ok((default.config(), default.sample_format()));
        }
        rejected.get_or_insert(default.sample_format());
    }

    if let Some((_, range)) = fallback {
        let rate = if range.contains_rate(preferred_rate) {
            preferred_rate
        } else if preferred_rate < range.min_sample_rate() {
            range.min_sample_rate()
        } else {
            range.max_sample_rate()
        };
        let cfg: cpal::StreamConfig = range.with_sample_rate(rate).into();
        return Ok((cfg, range.sample_format()));
    }

    Err(AudioError::UnsupportedSampleFormat {
        device: device_display_name(device),
        format: rejected
            .map(|fmt| format!("{fmt:?}"))
            .unwrap_or_else(|| "desconocido".to_string()),
    })
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
    /// Crea un nuevo gestor de dispositivos de audio sin salida activa.
    pub fn new() -> Self {
        Self {
            output: RwLock::new(None),
        }
    }

    /// Establece la salida de audio activa con el host, dispositivo y formato dados.
    pub fn set_output(
        &self,
        host: cpal::Host,
        device: cpal::Device,
        config: cpal::StreamConfig,
        fmt: cpal::SampleFormat,
    ) {
        let mut out = self.output.write();
        *out = Some(AudioOutput {
            host,
            device,
            stream_config: config,
            sample_format: fmt,
            stream: None,
        });
    }

    /// Extrae y retorna la salida activa actual (host, dispositivo, configuración, formato).
    pub fn take_output(
        &self,
    ) -> Option<(
        cpal::Host,
        cpal::Device,
        cpal::StreamConfig,
        cpal::SampleFormat,
    )> {
        self.output
            .write()
            .take()
            .map(|o| (o.host, o.device, o.stream_config, o.sample_format))
    }

    /// Devuelve la configuración del stream activo, si existe.
    pub fn get_stream_config(&self) -> Option<cpal::StreamConfig> {
        self.output.read().as_ref().map(|o| o.stream_config)
    }

    /// Devuelve el dispositivo de salida activo, si existe.
    pub fn get_device(&self) -> Option<cpal::Device> {
        self.output.read().as_ref().map(|o| o.device.clone())
    }

    /// Devuelve el formato de muestra del stream activo, si existe.
    pub fn get_sample_format(&self) -> Option<cpal::SampleFormat> {
        self.output.read().as_ref().map(|o| o.sample_format)
    }

    /// Verifica si hay un stream de audio activo.
    pub fn has_stream(&self) -> bool {
        self.output
            .read()
            .as_ref()
            .and_then(|o| o.stream.as_ref())
            .is_some()
    }

    /// Inicializa la salida de audio por defecto del sistema.
    ///
    /// Usa el host y dispositivo predeterminados de CPAL y selecciona un formato
    /// de muestra que el callback de salida sepa convertir, prefiriendo F32 para
    /// evitar conversiones en el camino caliente.
    pub fn init_default_output(
        &self,
    ) -> Result<
        (
            cpal::Host,
            cpal::Device,
            cpal::StreamConfig,
            cpal::SampleFormat,
        ),
        AudioError,
    > {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or(AudioError::NoDevice)?;

        // Preferir 48000 Hz: el selector busca primero una salida F32 en esa
        // tasa y recurre a la configuración por defecto del sistema.
        let (stream_config, sample_format) = select_accepted_output(&device, 48000, 2)?;

        self.configure_output(
            cpal::default_host(),
            device.clone(),
            stream_config,
            sample_format,
        )?;

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

    /// Inicia el stream de audio usando un closure que construye el stream CPAL.
    ///
    /// Si ya hay un stream activo, no hace nada.
    pub fn start_stream<F>(&self, build_stream: F) -> Result<(), AudioError>
    where
        F: FnOnce(
            &cpal::Device,
            cpal::StreamConfig,
            cpal::SampleFormat,
        ) -> Result<cpal::Stream, AudioError>,
    {
        let mut out_lock = self.output.write();
        if let Some(output) = out_lock.as_mut() {
            if output.stream.is_some() {
                return Ok(());
            }
            let stream = build_stream(&output.device, output.stream_config, output.sample_format)?;
            stream
                .play()
                .map_err(|e| AudioError::StreamError(e.to_string()))?;
            output.stream = Some(stream);
        }
        Ok(())
    }

    /// Detiene el stream de audio activo (si existe).
    pub fn stop_stream(&self) {
        let mut out = self.output.write();
        if let Some(o) = out.as_mut() {
            o.stream = None;
        }
    }

    /// Devuelve la lista de hosts de audio disponibles en el sistema.
    pub fn get_available_hosts() -> Vec<String> {
        cpal::available_hosts()
            .iter()
            .map(|id| id.name().to_string())
            .collect()
    }

    /// Devuelve la lista de dispositivos de salida del host activo.
    pub fn get_devices(&self) -> Vec<AudioDeviceInfo> {
        let host_name = self
            .output
            .read()
            .as_ref()
            .map(|o| o.host.id().name())
            .unwrap_or_else(|| cpal::default_host().id().name());

        let host_id = cpal::available_hosts()
            .into_iter()
            .find(|h| h.name() == host_name)
            .unwrap_or(cpal::default_host().id());
        let host = cpal::host_from_id(host_id).unwrap_or(cpal::default_host());

        if let Ok(devices) = host.output_devices() {
            devices
                .map(|d| {
                    let name = d
                        .description()
                        .map(|desc| desc.name().to_string())
                        .unwrap_or_else(|_| "Unknown".into());
                    let id = d.id().map(|id| id.to_string()).ok();
                    let supported_configs = d
                        .supported_output_configs()
                        .map(|c| c.collect())
                        .unwrap_or_default();
                    AudioDeviceInfo {
                        name,
                        id,
                        supported_configs,
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Configuración avanzada del device según preferencias de AudioSettings.
    ///
    /// Devuelve el host, el dispositivo resuelto, la mejor config de stream y el
    /// aviso no modal cuando la salida guardada ya no está disponible.
    pub fn resolve_settings(
        &self,
        settings: &AudioSettings,
        _current_out_rate: u32,
    ) -> Result<
        (
            cpal::Host,
            cpal::Device,
            cpal::StreamConfig,
            cpal::SampleFormat,
            Option<String>,
        ),
        AudioError,
    > {
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

        // Resolver la salida guardada: id estable -> nombre heredado -> predeterminado.
        // El id se parsea con `DeviceId::from_str` para normalizarlo antes de
        // compararlo con los ids presentes.
        let canonical_id: Option<String> = settings
            .device_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .and_then(|raw| raw.parse::<cpal::DeviceId>().ok())
            .map(|id| id.to_string());

        let present_devices: Vec<cpal::Device> = host
            .output_devices()
            .map_err(|e| AudioError::DeviceError(e.to_string()))?
            .collect();
        let present: Vec<(String, String)> = present_devices
            .iter()
            .map(|d| {
                (
                    d.id().map(|id| id.to_string()).unwrap_or_default(),
                    device_display_name(d),
                )
            })
            .collect();

        let (device, audio_notice) = match choose_device_lookup(
            canonical_id.as_deref(),
            settings.device_name.as_deref(),
            &present,
        ) {
            DeviceLookup::StableId(index) => (present_devices[index].clone(), None),
            DeviceLookup::LegacyName(index) => {
                let resolved_id = present[index].0.clone();
                if !resolved_id.is_empty() {
                    tracing::info!(
                        "Dispositivo de salida resuelto por nombre heredado; se volverá a \
                         persistir su id estable ('{resolved_id}')."
                    );
                }
                (present_devices[index].clone(), None)
            }
            DeviceLookup::Default => (
                host.default_output_device().ok_or(AudioError::NoDevice)?,
                None,
            ),
            DeviceLookup::Missing => {
                let missing = settings
                    .device_id
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .or_else(|| settings.device_name.as_deref().filter(|s| !s.is_empty()))
                    .unwrap_or("desconocido")
                    .to_string();
                let fallback = host.default_output_device().ok_or(AudioError::NoDevice)?;
                let notice = format!(
                    "El dispositivo de salida guardado ('{missing}') ya no está disponible; \
                     se usa el predeterminado."
                );
                tracing::warn!("{notice}");
                (fallback, Some(notice))
            }
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

        // Rechazar los formatos que el callback no sabe convertir antes de
        // puntuar, para que `best` nunca devuelva un formato no soportado. cpal
        // 0.18 enumera un `I24` distinto que este camino excluye a propósito:
        // esos dispositivos reciben el error nombrado de abajo. El mapeo
        // `Bits24` -> `I32` de arriba se mantiene.
        let rejected_format = supported_configs
            .iter()
            .find(|c| !accepted_sample_format(c.sample_format()))
            .map(|c| c.sample_format());
        supported_configs.retain(|c| accepted_sample_format(c.sample_format()));
        if supported_configs.is_empty() {
            return Err(AudioError::UnsupportedSampleFormat {
                device: device_display_name(&device),
                format: rejected_format
                    .map(|fmt| format!("{fmt:?}"))
                    .unwrap_or_else(|| "desconocido".to_string()),
            });
        }

        let req_channels = match settings.channels {
            ChannelConfig::Auto => 2,
            ChannelConfig::Manual(c) => c,
        };
        let req_rate = settings.sample_rate;

        let best = supported_configs
            .iter()
            .fold(None, |best, current| {
                let channel_score = if current.channels() == req_channels {
                    100
                } else if current.channels() > req_channels {
                    50
                } else {
                    0
                };
                let rate_score = req_rate.map_or(
                    if (current.min_sample_rate()..=current.max_sample_rate()).contains(&48000) {
                        10
                    } else {
                        0
                    },
                    |r| {
                        if r >= current.min_sample_rate() && r <= current.max_sample_rate() {
                            100
                        } else {
                            0
                        }
                    },
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

        let target_rate = match req_rate {
            // Si la tasa pedida está dentro del rango del dispositivo, se usa tal cual.
            Some(r) if r >= best.min_sample_rate() && r <= best.max_sample_rate() => r,
            // Si NO está soportada, se fija la más cercana dentro del rango (antes se
            // usaba la pedida aunque fuera inválida → el stream no se construía y el
            // audio quedaba mudo hasta pausa/play).
            Some(r) if r < best.min_sample_rate() => best.min_sample_rate(),
            Some(r) => best.max_sample_rate(),
            None => {
                let min = best.min_sample_rate();
                let max = best.max_sample_rate();
                if min <= 48000 && max >= 48000 {
                    48000
                } else if min <= 44100 && max >= 44100 {
                    44100
                } else {
                    max
                }
            }
        };

        let config = best.with_sample_rate(target_rate);
        let mut stream_config: cpal::StreamConfig = config.into();

        if let Some(frames) = settings.buffer_size {
            stream_config.buffer_size = cpal::BufferSize::Fixed(frames);
        } else {
            // Latencia adaptativa por plataforma (5.2)
            let base_latency_ms = platform_base_latency_ms();
            let min_frames = platform_min_buffer_frames();
            let calculated = (target_rate as f64 * base_latency_ms / 1000.0) as u32;
            let quantum = calculated.max(min_frames);
            // Redondear al quantum estándar más cercano
            let quantum = if quantum <= 64 {
                64
            } else if quantum <= 96 {
                96
            } else if quantum <= 128 {
                128
            } else if quantum <= 192 {
                192
            } else if quantum <= 256 {
                256
            } else if quantum <= 512 {
                512
            } else if quantum <= 1024 {
                1024
            } else if quantum <= 2048 {
                2048
            } else if quantum <= 4096 {
                4096
            } else {
                8192
            };
            stream_config.buffer_size = cpal::BufferSize::Fixed(quantum);
        }

        Ok((
            host,
            device,
            stream_config,
            best.sample_format(),
            audio_notice,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_base_latency_is_10ms() {
        assert_eq!(platform_base_latency_ms(), 10.0);
    }

    /// `accepted_sample_format` acepta exactamente los formatos que el callback
    /// puede convertir y rechaza el resto. `SampleFormat` es `#[non_exhaustive]`,
    /// así que las variantes conocidas se enumeran explícitamente.
    #[test]
    fn test_cpal_format_mapping_exhaustive() {
        let all_formats = [
            cpal::SampleFormat::I8,
            cpal::SampleFormat::I16,
            cpal::SampleFormat::I24,
            cpal::SampleFormat::I32,
            cpal::SampleFormat::I64,
            cpal::SampleFormat::U8,
            cpal::SampleFormat::U16,
            cpal::SampleFormat::U24,
            cpal::SampleFormat::U32,
            cpal::SampleFormat::U64,
            cpal::SampleFormat::F32,
            cpal::SampleFormat::F64,
            cpal::SampleFormat::DsdU8,
            cpal::SampleFormat::DsdU16,
            cpal::SampleFormat::DsdU32,
        ];

        for fmt in all_formats {
            let expected = matches!(
                fmt,
                cpal::SampleFormat::F32
                    | cpal::SampleFormat::F64
                    | cpal::SampleFormat::I16
                    | cpal::SampleFormat::I32
                    | cpal::SampleFormat::U16
            );
            assert_eq!(
                accepted_sample_format(fmt),
                expected,
                "mapeo inesperado para {fmt:?}"
            );
        }

        assert!(accepted_sample_format(cpal::SampleFormat::F32));
        assert!(!accepted_sample_format(cpal::SampleFormat::I24));
        assert!(!accepted_sample_format(cpal::SampleFormat::U32));
    }

    /// La resolución prefiere el id estable sobre el nombre heredado y cae al
    /// predeterminado cuando la salida guardada ya no está.
    #[test]
    fn test_resolve_device_id_prefers_stable() {
        let present = vec![
            ("alsa:one".to_string(), "DAC Uno".to_string()),
            ("alsa:two".to_string(), "DAC Dos".to_string()),
        ];

        // Con id estable y nombre heredado presentes, gana el id.
        assert_eq!(
            choose_device_lookup(Some("alsa:two"), Some("DAC Uno"), &present),
            DeviceLookup::StableId(1)
        );
        // Sin id, cae al nombre heredado.
        assert_eq!(
            choose_device_lookup(None, Some("DAC Uno"), &present),
            DeviceLookup::LegacyName(0)
        );
        // Con un id no presente pero un nombre válido, usa el nombre.
        assert_eq!(
            choose_device_lookup(Some("alsa:missing"), Some("DAC Dos"), &present),
            DeviceLookup::LegacyName(1)
        );
        // Con algo guardado sin coincidencia, avisa del fallback.
        assert_eq!(
            choose_device_lookup(Some("alsa:missing"), None, &present),
            DeviceLookup::Missing
        );
        // Sin nada guardado, usa el predeterminado.
        assert_eq!(
            choose_device_lookup(None, None, &present),
            DeviceLookup::Default
        );
    }
}
