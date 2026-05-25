use super::AudioError;
use crate::audio::dsp::DspChain;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam::channel::{Sender, unbounded};
use parking_lot::{Mutex, RwLock};
use ringbuf::wrap::caching::Caching;
use ringbuf::{
    HeapRb,
    traits::{Consumer, Split},
};
use std::sync::Arc;

// Define aliases based on ringbuf 0.4 structure
pub type HeapProducer<T> = Caching<Arc<HeapRb<T>>, true, false>;
pub type HeapConsumer<T> = Caching<Arc<HeapRb<T>>, false, true>;

#[derive(Default, Clone, Copy, Debug)]
pub(crate) struct ChannelMap {
    pub(crate) fl: Option<usize>,
    pub(crate) fr: Option<usize>,
    pub(crate) c: Option<usize>,
    pub(crate) lfe: Option<usize>,
    pub(crate) sl: Option<usize>,
    pub(crate) sr: Option<usize>,
    pub(crate) sbl: Option<usize>,
    pub(crate) sbr: Option<usize>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum AudioCommand {
    // ... (lines 39-470 skipped for brevity in replace_file_content, target only changed lines)
    // wait, I can't skip lines in ReplacementContent if they are part of the block I am replacing.
    // I should make multiple small edits or one precise edit.
    // I'll make multiple edits.
    Load {
        path: String,
        title: String,
        artist: String,
        track_gain: Option<f64>,
        album_gain: Option<f64>,
    },
    Seek(f64),
    Stop,
}

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

#[derive(Clone)]
pub struct AudioEngine {
    output: Arc<RwLock<Option<AudioOutput>>>,
    pub state: Arc<RwLock<AudioState>>,
    pub buffer_consumer: Arc<Mutex<Option<HeapConsumer<f32>>>>,
    pub buffer_producer: Arc<Mutex<Option<HeapProducer<f32>>>>,
    command_tx: Sender<AudioCommand>,
    pub dsp: Arc<RwLock<DspChain>>,
}

#[derive(Clone)]
pub struct AudioState {
    pub is_playing: bool,
    pub volume: f32,
    pub sample_rate: u32,
    pub channels: u16,
    pub current_pos_sec: f64,
    pub total_duration_sec: f64,
    pub title: String,
    pub artist: String,
    pub path: String,
    pub eof_reached: bool,

    pub device_sample_rate: u32,
    pub bit_depth_display: String,
    pub buffer_size: u32,
    pub config_channels: ChannelConfig, // Store intent

    // Downmix Variables Config
    pub downmix_center: f32,
    pub downmix_lfe: f32,
    pub downmix_surround: f32,
    pub downmix_center_enabled: bool,
    pub downmix_lfe_enabled: bool,
    pub downmix_surround_enabled: bool,

    /// Cuando está activado, el motor siempre resamplea a la tasa máxima
    /// soportada por el dispositivo, haciendo que Rubato FFT realice la
    /// reconstrucción de señal en software con calidad superior al DAC.
    pub auto_upsample: bool,

    // ReplayGain: ganancias independientes para normalización de volumen
    /// Ganancia de la pista actual en dB (del tag ReplayGain del archivo)
    pub replay_gain_track: Option<f32>,
    /// Ganancia del álbum en dB (del tag ReplayGain del archivo)
    pub replay_gain_album: Option<f32>,
    /// Activar/desactivar ganancia de pista (independiente)
    pub replay_gain_track_enabled: bool,
    /// Activar/desactivar ganancia de álbum (independiente)
    pub replay_gain_album_enabled: bool,
}

// Adapters removed (not needed for Rubato 1.0 with Vec<Vec<f32>>)

impl Default for AudioState {
    fn default() -> Self {
        Self {
            is_playing: false,
            volume: 0.3,
            sample_rate: 44100,
            channels: 2,
            current_pos_sec: 0.0,
            total_duration_sec: 0.0,
            title: "Sin título".to_string(),
            artist: "Artista desconocido".to_string(),
            path: String::new(),
            eof_reached: false,

            device_sample_rate: 44100, // Default Match
            bit_depth_display: "Unknown".to_string(),
            buffer_size: 0,
            config_channels: ChannelConfig::Auto,

            downmix_center: 0.81,
            downmix_lfe: 0.66,
            downmix_surround: 0.73,
            downmix_center_enabled: false,
            downmix_lfe_enabled: false,
            downmix_surround_enabled: false,
            auto_upsample: false,
            replay_gain_track: None,
            replay_gain_album: None,
            replay_gain_track_enabled: true,
            replay_gain_album_enabled: true,
        }
    }
}

#[allow(dead_code)]
impl AudioEngine {
    pub fn new() -> Result<Self, AudioError> {
        let (command_tx, command_rx) = unbounded::<AudioCommand>();

        // Aumentado significativamente para evitar underruns a 384kHz 7.1ch (~2 segundos de audio)
        // Modo low-resource: reduce a ~0.5 segundos (~2 MiB máx)
        let rb_size = if crate::utils::is_low_resource() { 2 * 1024 * 1024 } else { 8 * 1024 * 1024 };
        let rb = HeapRb::<f32>::new(rb_size);
        let (producer, consumer) = rb.split();

        let state = Arc::new(RwLock::new(AudioState::default()));
        let dsp = Arc::new(RwLock::new(DspChain::default()));
        let output = Arc::new(RwLock::new(None));

        // Hilo de decodificación recibe una copia del Engine para controlarse a sí mismo
        let engine = Self {
            output,
            state,
            buffer_consumer: Arc::new(Mutex::new(Some(consumer))),
            buffer_producer: Arc::new(Mutex::new(Some(producer))),
            command_tx,
            dsp,
        };

        let engine_clone = engine.clone();
        std::thread::spawn(move || {
            crate::audio::decoder::audio_decode_loop(command_rx, engine_clone);
        });

        // Iniciar output por defecto
        engine.init_default_output()?;

        Ok(engine)
    }

    fn init_default_output(&self) -> Result<(), AudioError> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or(AudioError::NoDevice)?;
        let config = device
            .default_output_config()
            .map_err(|e| AudioError::DeviceError(e.to_string()))?;

        let mut stream_config: cpal::StreamConfig = config.clone().into();
        stream_config.buffer_size = cpal::BufferSize::Default;
        stream_config.channels = 2; // Default to Stereo

        // Intentar iniciar con defaults
        // Host no es Clone, asi que creamos uno nuevo para el fallback si hace falta
        // transferimos ownership de 'host' aqui
        if let Err(_) = self.configure_and_start_stream(
            host,
            device.clone(),
            stream_config.clone(),
            config.sample_format(),
        ) {
            // Fallback al default del dispositivo
            let host_fallback = cpal::default_host();
            let sample_fmt = config.sample_format();
            let def_conf: cpal::StreamConfig = config.into();
            self.state.write().device_sample_rate = def_conf.sample_rate;
            self.configure_and_start_stream(host_fallback, device, def_conf, sample_fmt)?;
        } else {
            self.state.write().device_sample_rate = stream_config.sample_rate;
        }
        Ok(())
    }

    pub fn start(&self) -> Result<(), AudioError> {
        self.start_stream()
    }

    fn start_stream(&self) -> Result<(), AudioError> {
        let mut out_lock = self.output.write();
        if let Some(output) = out_lock.as_mut() {
            if output.stream.is_some() {
                return Ok(());
            }

            let state = self.state.clone();
            let consumer_arc = self.buffer_consumer.clone();
            let channels = output.stream_config.channels as usize;

            let err_fn = |err| tracing::error!("Stream error: {}", err);

            // DSP ya no se procesa aquí — se procesa en el hilo de decodificación
            // El callback solo lee del RingBuffer y escribe al stream (zero-lock passthrough)
            let stream = match output.sample_format {
                cpal::SampleFormat::F32 => output.device.build_output_stream(
                    &output.stream_config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(data, channels, &state, &consumer_arc)
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I16 => output.device.build_output_stream(
                    &output.stream_config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(data, channels, &state, &consumer_arc)
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::U16 => output.device.build_output_stream(
                    &output.stream_config,
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(data, channels, &state, &consumer_arc)
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I32 => output.device.build_output_stream(
                    &output.stream_config,
                    move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(data, channels, &state, &consumer_arc)
                    },
                    err_fn,
                    None,
                ),
                _ => return Err(AudioError::UnsupportedSampleFormat),
            }
            .map_err(|e| AudioError::StreamError(e.to_string()))?;

            stream
                .play()
                .map_err(|e| AudioError::StreamError(e.to_string()))?;
            output.stream = Some(stream);
        }
        Ok(())
    }

    /// Callback de audio simplificado: solo lee del RingBuffer y escribe al stream.
    /// El DSP y el volumen ya están aplicados en los datos del RingBuffer.
    /// No usa RwLock ni procesamiento pesado — máxima seguridad para real-time.
    fn write_data<T>(
        output: &mut [T],
        channels: usize,
        state: &Arc<RwLock<AudioState>>,
        consumer_mutex: &Arc<Mutex<Option<HeapConsumer<f32>>>>,
    ) where
        T: cpal::Sample + cpal::FromSample<f32>,
    {
        Self::write_data_impl(output, channels, state, consumer_mutex)
    }

    fn write_data_impl<T>(
        output: &mut [T],
        channels: usize,
        state: &Arc<RwLock<AudioState>>,
        consumer_mutex: &Arc<Mutex<Option<HeapConsumer<f32>>>>,
    ) where
        T: cpal::Sample + cpal::FromSample<f32>,
    {
        if channels > 0 {
            let current_frames = (output.len() / channels) as u32;
            if current_frames > 0 {
                let mut needs_update = false;
                {
                    let s = state.read();
                    if s.buffer_size != current_frames {
                        needs_update = true;
                    }
                }
                if needs_update {
                    let mut s = state.write();
                    s.buffer_size = current_frames;
                }
            }
        }

        let is_playing = state.read().is_playing;

        if !is_playing {
            output.fill(T::from_sample(0.0));
            return;
        }

        if let Some(consumer) = consumer_mutex.lock().as_mut() {
            for frame_out in output.chunks_mut(channels) {
                let mut valid = true;
                for sample_out in frame_out.iter_mut() {
                    if let Some(val) = consumer.try_pop() {
                        *sample_out = T::from_sample(val);
                    } else {
                        *sample_out = T::from_sample(0.0);
                        valid = false;
                    }
                }
                if !valid {
                    break;
                }
            }
        } else {
            output.fill(T::from_sample(0.0));
        }
    }

    pub fn get_available_hosts(&self) -> Vec<String> {
        if cfg!(target_os = "linux") {
            let hosts: Vec<String> = cpal::available_hosts()
                .iter()
                .map(|id| id.name().to_string())
                .collect();
            hosts
        } else {
            cpal::available_hosts()
                .iter()
                .map(|id| id.name().to_string())
                .collect()
        }
    }

    pub fn get_devices(&self) -> Vec<AudioDeviceInfo> {
        let host_name = if let Some(out) = self.output.read().as_ref() {
            out.host.id().name()
        } else {
            cpal::default_host().id().name()
        };

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
                    AudioDeviceInfo {
                        name,
                        supported_configs,
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Realiza una purga profunda de los buffers y reinicia el stream con la configuración actual.
    /// Útil para liberar memoria RAM cuando la reproducción se detiene o pausa.
    pub fn purge_buffers(&self) -> Result<(), AudioError> {
        println!("Audoxidy Audio: Cleaning Audio Engine Buffers");
        let (host, device, config, format) = {
            let mut out_lock = self.output.write();
            // Extraemos los valores actuales (esto libera el stream anterior y sus buffers de hardware)
            let out = out_lock.take().ok_or(AudioError::NoActiveOutput)?;
            (out.host, out.device, out.stream_config, out.sample_format)
        };

        // Reconfigura el motor (recrea el RingBuffer) y arranca un stream fresco con los mismos parámetros
        self.configure_and_start_stream(host, device, config, format)
    }

    pub fn apply_settings(&self, settings: AudioSettings) -> Result<(), AudioError> {
        {
            let mut out = self.output.write();
            if let Some(o) = out.as_mut() {
                o.stream = None;
            }
        }

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

        if let Some(bd) = settings.bit_depth {
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
                tracing::warn!(
                    "Requested bit depth not supported by device, falling back to auto selection from supported formats"
                );
            }
        }

        let req_channels = match settings.channels {
            ChannelConfig::Auto => {
                2 // Default to Stereo for Auto
            }
            ChannelConfig::Manual(c) => c,
        };

        let req_rate = settings.sample_rate;

        let best_config_range = supported_configs
            .iter()
            .fold(None, |best, current| {
                let cur_channels = current.channels();
                let cur_min_rate = current.min_sample_rate();
                let cur_max_rate = current.max_sample_rate();

                let channel_score = if cur_channels == req_channels {
                    100
                } else if cur_channels > req_channels {
                    50
                } else {
                    0
                };

                let rate_score = if let Some(r) = req_rate {
                    if r >= cur_min_rate && r <= cur_max_rate {
                        100
                    } else {
                        0
                    }
                } else {
                    if (cur_min_rate..=cur_max_rate).contains(&44100) {
                        10
                    } else {
                        0
                    }
                };

                let total_score = channel_score + rate_score;

                match best {
                    Some((score, cfg)) => {
                        if total_score > score {
                            Some((total_score, current))
                        } else {
                            Some((score, cfg))
                        }
                    }
                    None => Some((total_score, current)),
                }
            })
            .map(|(_, c)| c)
            .ok_or(AudioError::ConfigError("No valid config found".into()))?;

        let target_rate = if let Some(r) = req_rate {
            r
        } else {
            let min = best_config_range.min_sample_rate();
            let max = best_config_range.max_sample_rate();

            if min <= 44100 && max >= 44100 {
                44100
            } else if min <= 48000 && max >= 48000 {
                48000
            } else {
                max
            }
        };

        // Upsampling automático: cuando está activado, usar la tasa máxima
        // soportada por el dispositivo para que Rubato FFT siempre realice
        // la reconstrucción de señal en software con calidad superior al DAC.
        let target_rate = if settings.auto_upsample {
            let max_rate = best_config_range.max_sample_rate();
            if max_rate > target_rate {
                tracing::info!(
                    "Auto-upsample: {} Hz -> {} Hz (máximo del dispositivo)",
                    target_rate,
                    max_rate
                );
                max_rate
            } else {
                target_rate
            }
        } else {
            target_rate
        };

        // Almacenar estado de auto-upsample
        self.state.write().auto_upsample = settings.auto_upsample;

        let config = best_config_range.with_sample_rate(target_rate);
        let mut stream_config: cpal::StreamConfig = config.clone().into();

        if let Some(frames) = settings.buffer_size {
            stream_config.buffer_size = cpal::BufferSize::Fixed(frames);
        } else {
            let base_latency_ms = 10.0;
            let calculated_frames = (target_rate as f64 * base_latency_ms / 1000.0) as u32;

            let quantum = if calculated_frames < 256 {
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
            };

            stream_config.buffer_size = cpal::BufferSize::Fixed(quantum);
        }

        {
            let mut s = self.state.write();
            s.config_channels = settings.channels.clone();
        }

        // Update DSP configuration
        {
            let mut dsp = self.dsp.write();
            dsp.set_sample_rate(config.sample_rate() as f32);
            dsp.set_channel_count(stream_config.channels as usize);
        }

        self.configure_and_start_stream(
            host,
            device,
            stream_config,
            best_config_range.sample_format(),
        )
    }

    fn configure_and_start_stream(
        &self,
        host: cpal::Host,
        device: cpal::Device,
        stream_config: cpal::StreamConfig,
        sample_format: cpal::SampleFormat,
    ) -> Result<(), AudioError> {
        {
            let mut s = self.state.write();
            s.device_sample_rate = stream_config.sample_rate;
            s.channels = stream_config.channels;
            s.buffer_size = match stream_config.buffer_size {
                cpal::BufferSize::Fixed(f) => f,
                _ => 0,
            };
            s.bit_depth_display = match sample_format {
                cpal::SampleFormat::I16 => "16-bit Int",
                cpal::SampleFormat::U16 => "16-bit Int (U)",
                cpal::SampleFormat::I32 => "24/32-bit Int",
                cpal::SampleFormat::F32 => "32-bit Float",
                _ => "Unknown",
            }
            .to_string();
        }

        {
            let mut out = self.output.write();
            *out = Some(AudioOutput {
                host,
                device,
                stream_config: stream_config.clone(),
                sample_format,
                stream: None,
            });
        }

        // Recreate RingBuffer scaled to sample rate (~2 seconds of audio, ~0.5s en low-resource)
        let sr = stream_config.sample_rate as usize;
        let ch = stream_config.channels as usize;
        let dur_secs = if crate::utils::is_low_resource() { 0.5 } else { 2.0 };
        let rb_size = ((sr * ch) as f64 * dur_secs) as usize;
        let rb = HeapRb::<f32>::new(rb_size.max(384_000)); // Mínimo 384k muestras
        let (producer, consumer) = rb.split();

        // Update handles
        {
            let mut p_lock = self.buffer_producer.lock();
            *p_lock = Some(producer);
        }

        {
            let mut c_lock = self.buffer_consumer.lock();
            *c_lock = Some(consumer);
        }

        self.start_stream()?;
        Ok(())
    }

    pub fn decode_file(
        &self,
        path: &str,
        title: String,
        artist: String,
        track_gain: Option<f64>,
        album_gain: Option<f64>,
    ) -> Result<(), AudioError> {
        let _ = self.command_tx.send(AudioCommand::Load {
            path: path.to_string(),
            title,
            artist,
            track_gain,
            album_gain,
        });
        Ok(())
    }

    pub(crate) fn mix_channels_planar(
        input: &Vec<Vec<f64>>,
        frames: usize,
        in_channels: usize,
        out_channels: usize,
        map: &ChannelMap,
        dm_conf: (f64, f64, f64, f64),
        out_buf: &mut Vec<f64>,
    ) {
        for i in 0..frames {
            let fl = map
                .fl
                .and_then(|idx| input.get(idx))
                .map(|v| v[i])
                .unwrap_or(0.0);
            let fr = map
                .fr
                .and_then(|idx| input.get(idx))
                .map(|v| v[i])
                .unwrap_or(0.0);
            let c = map
                .c
                .and_then(|idx| input.get(idx))
                .map(|v| v[i])
                .unwrap_or(0.0);
            let lfe = map
                .lfe
                .and_then(|idx| input.get(idx))
                .map(|v| v[i])
                .unwrap_or(0.0);
            let sl = map
                .sl
                .and_then(|idx| input.get(idx))
                .map(|v| v[i])
                .unwrap_or(0.0);
            let sr = map
                .sr
                .and_then(|idx| input.get(idx))
                .map(|v| v[i])
                .unwrap_or(0.0);
            let sbl = map
                .sbl
                .and_then(|idx| input.get(idx))
                .map(|v| v[i])
                .unwrap_or(0.0);
            let sbr = map
                .sbr
                .and_then(|idx| input.get(idx))
                .map(|v| v[i])
                .unwrap_or(0.0);

            Self::mix_sample_into_vec(
                out_buf,
                out_channels,
                fl,
                fr,
                c,
                lfe,
                sl,
                sr,
                sbl,
                sbr,
                in_channels,
                dm_conf,
            );
        }
    }

    pub(crate) fn mix_channels_direct(
        buffer: &symphonia::core::audio::AudioBuffer<f64>,
        frames: usize,
        src_ch: usize,
        dst_ch: usize,
        map: &ChannelMap,
        dm_conf: (f64, f64, f64, f64),
        out_buf: &mut Vec<f64>,
    ) {
        let planes = buffer.planes();

        let get_sample = |plane_idx: usize, frame_idx: usize| -> f64 {
            if plane_idx < src_ch {
                planes.planes()[plane_idx][frame_idx] as f64
            } else {
                0.0
            }
        };

        for i in 0..frames {
            let fl = map.fl.map(|idx| get_sample(idx, i)).unwrap_or(0.0);
            let fr = map.fr.map(|idx| get_sample(idx, i)).unwrap_or(0.0);
            let c = map.c.map(|idx| get_sample(idx, i)).unwrap_or(0.0);
            let lfe = map.lfe.map(|idx| get_sample(idx, i)).unwrap_or(0.0);
            let sl = map.sl.map(|idx| get_sample(idx, i)).unwrap_or(0.0);
            let sr = map.sr.map(|idx| get_sample(idx, i)).unwrap_or(0.0);
            let sbl = map.sbl.map(|idx| get_sample(idx, i)).unwrap_or(0.0);
            let sbr = map.sbr.map(|idx| get_sample(idx, i)).unwrap_or(0.0);

            Self::mix_sample_into_vec(
                out_buf, dst_ch, fl, fr, c, lfe, sl, sr, sbl, sbr, src_ch, dm_conf,
            );
        }
    }

    // Helper para mapear canales de entrada a roles
    pub(crate) fn get_channel_map(channels: symphonia::core::audio::Channels) -> ChannelMap {
        use symphonia::core::audio::Channels;
        let mut map = ChannelMap::default();

        let mut index = 0;

        // Standard iterator order in Symphonia (WAVEFORMATEXTENSIBLE order)
        if channels.contains(Channels::FRONT_LEFT) {
            map.fl = Some(index);
            index += 1;
        }
        if channels.contains(Channels::FRONT_RIGHT) {
            map.fr = Some(index);
            index += 1;
        }
        if channels.contains(Channels::FRONT_CENTRE) {
            map.c = Some(index);
            index += 1;
        }
        if channels.contains(Channels::LFE1) {
            map.lfe = Some(index);
            index += 1;
        }
        if channels.contains(Channels::REAR_LEFT) {
            map.sbl = Some(index);
            index += 1;
        }
        if channels.contains(Channels::REAR_RIGHT) {
            map.sbr = Some(index);
            index += 1;
        }
        if channels.contains(Channels::FRONT_LEFT_CENTRE) {
            index += 1;
        }
        if channels.contains(Channels::FRONT_RIGHT_CENTRE) {
            index += 1;
        }
        if channels.contains(Channels::REAR_CENTRE) {
            index += 1;
        }
        if channels.contains(Channels::SIDE_LEFT) {
            map.sl = Some(index);
            index += 1;
        }
        if channels.contains(Channels::SIDE_RIGHT) {
            map.sr = Some(index);
            index += 1;
        }
        // Ignore other channels but increment index to keep sequence correct if they exist in valid stream
        // But here we are mapping logic logic purely for map construction.
        // If we don't use 'index' afterwards, and we don't use the map entries for these channels, we can just increment index or do nothing if we want to "skip" them in our map but they are present in the stream.
        // Actually, 'index' tracks the channel position in the planar/interleaved stream.
        // So we MUST increment index for every channel present in 'channels'.
        if channels.contains(Channels::TOP_CENTRE) {
            index += 1;
        }
        if channels.contains(Channels::TOP_FRONT_LEFT) {
            index += 1;
        }
        if channels.contains(Channels::TOP_FRONT_CENTRE) {
            index += 1;
        }
        if channels.contains(Channels::TOP_FRONT_RIGHT) {
            index += 1;
        }
        if channels.contains(Channels::TOP_REAR_LEFT) {
            index += 1;
        }
        if channels.contains(Channels::TOP_REAR_CENTRE) {
            index += 1;
        }
        if channels.contains(Channels::TOP_REAR_RIGHT) {
            index += 1;
        }

        let _ = index; // Silence unused variable warning at the end

        map
    }

    fn mix_sample_into_vec(
        out: &mut Vec<f64>,
        dst_ch: usize,
        mut fl: f64,
        mut fr: f64,
        c: f64,
        lfe: f64,
        sl: f64,
        sr: f64,
        sbl: f64,
        sbr: f64,
        src_ch_count: usize,
        dm_conf: (f64, f64, f64, f64),
    ) {
        // Mono Expansion: Si el archivo es mono, distribuir a ambos canales frontales
        if src_ch_count == 1 {
            if fl == 0.0 && c != 0.0 {
                fl = c;
            }
            fr = fl;
        }

        let is_downmix = src_ch_count >= 6 && dst_ch < 6;
        let center_mix = if is_downmix { c * dm_conf.0 } else { c };
        let sur_mix_l = if is_downmix {
            sl * dm_conf.2 + sbl * dm_conf.3
        } else {
            sl
        };
        let sur_mix_r = if is_downmix {
            sr * dm_conf.2 + sbr * dm_conf.3
        } else {
            sr
        };
        let lfe_mix = if is_downmix { lfe * dm_conf.1 } else { lfe };

        match dst_ch {
            1 => {
                let sum = fl + fr + center_mix + lfe_mix + sur_mix_l + sur_mix_r;
                out.push(sum * 0.5);
            }
            2 => {
                out.push(fl + center_mix + lfe_mix + sur_mix_l);
                out.push(fr + center_mix + lfe_mix + sur_mix_r);
            }
            3 => {
                out.push(fl + center_mix + sur_mix_l);
                out.push(fr + center_mix + sur_mix_r);
                out.push(lfe + (fl + fr + c + sl + sr) * 0.1);
            }
            4 => {
                out.push(fl + center_mix + lfe_mix);
                out.push(fr + center_mix + lfe_mix);
                out.push(sl + sbl);
                out.push(sr + sbr);
            }
            6 => {
                if src_ch_count >= 7 {
                    out.push(fl);
                    out.push(fr);
                    out.push(c);
                    out.push(lfe);
                    out.push(sl + sbl);
                    out.push(sr + sbr);
                } else {
                    out.push(fl);
                    out.push(fr);
                    out.push(c);
                    out.push(lfe);
                    out.push(sl);
                    out.push(sr);
                }
            }
            8 => {
                out.push(fl);
                out.push(fr);
                out.push(c);
                out.push(lfe);
                out.push(sbl); // Back Left
                out.push(sbr); // Back Right
                out.push(sl); // Side Left
                out.push(sr); // Side Right
            }
            _ => {
                let l = fl + c * 0.7 + sl + sbl;
                let r = fr + c * 0.7 + sr + sbr;
                out.push(l);
                out.push(r);
                for _ in 2..dst_ch {
                    out.push(0.0);
                }
            }
        }
    }

    pub fn seek(&self, pos: f64) {
        let _ = self.command_tx.send(AudioCommand::Seek(pos));
    }
    pub fn stop(&self) {
        let _ = self.command_tx.send(AudioCommand::Stop);
        self.set_playing(false);
    }
    pub fn set_playing(&self, playing: bool) {
        self.state.write().is_playing = playing;
    }
    pub fn set_volume(&self, volume: f32) {
        self.state.write().volume = volume.clamp(0.0, 1.0);
    }
}
