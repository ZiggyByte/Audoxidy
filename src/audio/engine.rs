use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::Arc;
use std::fs::File;
use parking_lot::{RwLock, Mutex};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;
use symphonia::core::formats::FormatOptions;
use symphonia::core::meta::{MetadataOptions, Limit};
use symphonia::core::codecs::DecoderOptions;
use ringbuf::{HeapRb, traits::{Split, Consumer, Producer, Observer}};
use ringbuf::wrap::caching::Caching;
use symphonia::core::audio::Signal as SymphoniaSignal;
use crossbeam::channel::{Sender, Receiver, unbounded};
use rubato::{Resampler, Async, FixedAsync, SincInterpolationType, SincInterpolationParameters, WindowFunction};
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use crate::audio::dsp::DspChain;

// Define aliases based on ringbuf 0.4 structure
pub type HeapProducer<T> = Caching<Arc<HeapRb<T>>, true, false>;
pub type HeapConsumer<T> = Caching<Arc<HeapRb<T>>, false, true>;

#[derive(Default, Clone, Copy, Debug)]
pub(crate) struct ChannelMap {
    pub(crate) fl: Option<usize>,
    pub(crate) fr: Option<usize>,
    pub(crate) c:  Option<usize>,
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

    Load { path: String, title: String, artist: String, track_gain: Option<f64>, album_gain: Option<f64> },
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
    fn default() -> Self { Self::Bits32Float }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChannelConfig {
    Auto,
    Manual(u16),
}

impl Default for ChannelConfig {
    fn default() -> Self { Self::Manual(2) }
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
    pub fn new() -> Result<Self, String> {
        let (command_tx, command_rx) = unbounded::<AudioCommand>();
        
        // Aumentado significativamente para evitar underruns a 384kHz 7.1ch (~2 segundos de audio)
        let rb = HeapRb::<f32>::new(8 * 1024 * 1024); 
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
            Self::audio_decode_loop(command_rx, engine_clone);
        });
        
        // Iniciar output por defecto
        engine.init_default_output()?;

        Ok(engine)
    }

    fn init_default_output(&self) -> Result<(), String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("No audio device found")?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        
        let supported_buffer = config.buffer_size();
        let supports_1024 = if let cpal::SupportedBufferSize::Range { min, max } = supported_buffer {
             *min <= 1024 && *max >= 1024
        } else {
             false
        };

        let mut stream_config: cpal::StreamConfig = config.clone().into();
        if supports_1024 {
             stream_config.buffer_size = cpal::BufferSize::Fixed(1024);
        }
        stream_config.channels = 2; // Default to Stereo

        // Intentar iniciar con defaults
        // Host no es Clone, asi que creamos uno nuevo para el fallback si hace falta
        // transferimos ownership de 'host' aqui
        if let Err(_) = self.configure_and_start_stream(host, device.clone(), stream_config.clone(), config.sample_format()) {
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

    pub fn start(&self) -> Result<(), String> {
        self.start_stream()
    }

    fn start_stream(&self) -> Result<(), String> {
        let mut out_lock = self.output.write();
        if let Some(output) = out_lock.as_mut() {
            if output.stream.is_some() { return Ok(()); } 

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
                    None
                ),
                cpal::SampleFormat::I16 => output.device.build_output_stream(
                    &output.stream_config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(data, channels, &state, &consumer_arc)
                    },
                    err_fn,
                    None
                ),
                cpal::SampleFormat::U16 => output.device.build_output_stream(
                    &output.stream_config,
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(data, channels, &state, &consumer_arc)
                    },
                    err_fn,
                    None
                ),
                cpal::SampleFormat::I32 => output.device.build_output_stream(
                    &output.stream_config,
                    move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                         Self::write_data(data, channels, &state, &consumer_arc)
                    },
                    err_fn,
                    None
                ),
                _ => return Err("Formato de muestra no soportado".to_string()),
            }.map_err(|e| e.to_string())?;

            stream.play().map_err(|e| e.to_string())?;
            output.stream = Some(stream);
        }
        Ok(())
    }

    /// Callback de audio simplificado: solo lee del RingBuffer y escribe al stream.
    /// El DSP y el volumen ya están aplicados en los datos del RingBuffer.
    /// No usa RwLock ni procesamiento pesado — máxima seguridad para real-time.
    fn write_data<T>(output: &mut [T], channels: usize, state: &Arc<RwLock<AudioState>>, consumer_mutex: &Arc<Mutex<Option<HeapConsumer<f32>>>>)
    where T: cpal::Sample + cpal::FromSample<f32>
    {
        Self::write_data_impl(output, channels, state, consumer_mutex)
    }

    fn write_data_impl<T>(output: &mut [T], channels: usize, state: &Arc<RwLock<AudioState>>, consumer_mutex: &Arc<Mutex<Option<HeapConsumer<f32>>>>)
    where T: cpal::Sample + cpal::FromSample<f32> {
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
                 if !valid { break; }
             }
        } else {
             output.fill(T::from_sample(0.0));
        }
    }

    pub fn get_available_hosts(&self) -> Vec<String> {
        if cfg!(target_os = "linux") {
            let hosts: Vec<String> = cpal::available_hosts().iter().map(|id| id.name().to_string()).collect();
            hosts
        } else {
            cpal::available_hosts().iter().map(|id| id.name().to_string()).collect()
        }
    }

    pub fn get_devices(&self) -> Vec<AudioDeviceInfo> {
        let host_name = if let Some(out) = self.output.read().as_ref() {
            out.host.id().name()
        } else {
            cpal::default_host().id().name()
        };
        
        let host_id = cpal::available_hosts().into_iter().find(|h| h.name() == host_name).unwrap_or(cpal::default_host().id());
        let host = cpal::host_from_id(host_id).unwrap_or(cpal::default_host());

        if let Ok(devices) = host.output_devices() {
            devices.map(|d| {
                #[allow(deprecated)]
                let name = d.name().unwrap_or_else(|_| "Unknown".into());
                let supported_configs = d.supported_output_configs().map(|c| c.collect()).unwrap_or_default();
                AudioDeviceInfo { name, supported_configs }
            }).collect()
        } else {
            Vec::new()
        }
    }

    /// Realiza una purga profunda de los buffers y reinicia el stream con la configuración actual.
    /// Útil para liberar memoria RAM cuando la reproducción se detiene o pausa.
    pub fn purge_buffers(&self) -> Result<(), String> {
        println!("Audoxidy Audio: Cleaning Audio Engine Buffers");
        let (host, device, config, format) = {
            let mut out_lock = self.output.write();
            // Extraemos los valores actuales (esto libera el stream anterior y sus buffers de hardware)
            let out = out_lock.take().ok_or("No hay una salida de audio activa para purgar")?;
            (out.host, out.device, out.stream_config, out.sample_format)
        };
        
        // Reconfigura el motor (recrea el RingBuffer) y arranca un stream fresco con los mismos parámetros
        self.configure_and_start_stream(host, device, config, format)
    }

    pub fn apply_settings(&self, settings: AudioSettings) -> Result<(), String> {
        {
            let mut out = self.output.write();
            if let Some(o) = out.as_mut() {
                o.stream = None; 
            }
        }

        let target_host_id = if let Some(ref name) = settings.host_id {
             cpal::available_hosts().into_iter().find(|h| h.name() == name).ok_or("Host no encontrado")?
        } else {
             cpal::default_host().id()
        };

        let host = cpal::host_from_id(target_host_id).map_err(|e| e.to_string())?;

        let device = if let Some(ref dev_name) = settings.device_name {
            #[allow(deprecated)]
            host.output_devices().map_err(|e| e.to_string())?
                .find(|d| d.name().unwrap_or_default() == *dev_name)
                .ok_or("Dispositivo no encontrado")?
        } else {
            host.default_output_device().ok_or("No default device")?
        };

        let mut supported_configs: Vec<_> = device.supported_output_configs().map_err(|e| e.to_string())?.collect();
        
        if let Some(bd) = settings.bit_depth {
            let filtered: Vec<_> = supported_configs.iter().cloned().filter(|c| {
                match bd {
                     BitDepth::Bits16 => c.sample_format() == cpal::SampleFormat::I16 || c.sample_format() == cpal::SampleFormat::U16,
                     BitDepth::Bits24 => c.sample_format() == cpal::SampleFormat::I32,
                     BitDepth::Bits32Float => c.sample_format() == cpal::SampleFormat::F32,
                }
            }).collect();
            
            if !filtered.is_empty() {
                supported_configs = filtered;
            } else {
                tracing::warn!("Requested bit depth not supported by device, falling back to auto selection from supported formats");
            }
        }

        let req_channels = match settings.channels {
            ChannelConfig::Auto => {
                2 // Default to Stereo for Auto
            }, 
            ChannelConfig::Manual(c) => c,
        };

        let req_rate = settings.sample_rate;

        let best_config_range = supported_configs.iter().fold(None, |best, current| {
             let cur_channels = current.channels();
             let cur_min_rate = current.min_sample_rate();
             let cur_max_rate = current.max_sample_rate();

             let channel_score = if cur_channels == req_channels { 100 } else if cur_channels > req_channels { 50 } else { 0 };
             
             let rate_score = if let Some(r) = req_rate {
                 if r >= cur_min_rate && r <= cur_max_rate { 100 } else { 0 }
             } else {
                 if (cur_min_rate..=cur_max_rate).contains(&44100) { 10 } else { 0 }
             };

             let total_score = channel_score + rate_score;
             
             match best {
                 Some((score, cfg)) => if total_score > score { Some((total_score, current)) } else { Some((score, cfg)) },
                 None => Some((total_score, current))
             }
        }).map(|(_, c)| c).ok_or("No valid config found")?;

     let target_rate = if let Some(r) = req_rate {
          r
     } else {
             let min = best_config_range.min_sample_rate();
             let max = best_config_range.max_sample_rate();
             
             if min <= 44100 && max >= 44100 { 44100 }
             else if min <= 48000 && max >= 48000 { 48000 }
             else { max }
        };

     // Upsampling automático: cuando está activado, usar la tasa máxima
     // soportada por el dispositivo para que Rubato FFT siempre realice
     // la reconstrucción de señal en software con calidad superior al DAC.
     let target_rate = if settings.auto_upsample {
         let max_rate = best_config_range.max_sample_rate();
         if max_rate > target_rate {
             tracing::info!("Auto-upsample: {} Hz -> {} Hz (máximo del dispositivo)", target_rate, max_rate);
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
           
           let quantum = if calculated_frames < 256 { 256 }
           else if calculated_frames < 512 { 512 }
           else if calculated_frames < 1024 { 1024 }
           else if calculated_frames < 2048 { 2048 }
           else if calculated_frames < 4096 { 4096 }
           else { 8192 };
           
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
     
     self.configure_and_start_stream(host, device, stream_config, best_config_range.sample_format())
    }

    fn configure_and_start_stream(&self, host: cpal::Host, device: cpal::Device, stream_config: cpal::StreamConfig, sample_format: cpal::SampleFormat) -> Result<(), String> {
        {
             let mut s = self.state.write();
            s.device_sample_rate = stream_config.sample_rate; 
            s.channels = stream_config.channels;
            s.buffer_size = match stream_config.buffer_size { cpal::BufferSize::Fixed(f) => f, _ => 0 };
            s.bit_depth_display = match sample_format {
                cpal::SampleFormat::I16 => "16-bit Int",
                cpal::SampleFormat::U16 => "16-bit Int (U)",
                cpal::SampleFormat::I32 => "24/32-bit Int",
                cpal::SampleFormat::F32 => "32-bit Float",
                _ => "Unknown"
            }.to_string();
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

        // Recreate RingBuffer scaled to sample rate (~2 seconds of audio)
        let sr = stream_config.sample_rate as usize;
        let ch = stream_config.channels as usize;
        let rb_size = (sr * ch * 2).max(384_000); // Mínimo 384k muestras
        let rb = HeapRb::<f32>::new(rb_size);
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

    pub fn decode_file(&self, path: &str, title: String, artist: String, track_gain: Option<f64>, album_gain: Option<f64>) -> Result<(), String> {
        let _ = self.command_tx.send(AudioCommand::Load { 
            path: path.to_string(),
            title,
            artist,
            track_gain,
            album_gain,
        });
        Ok(())
    }



    fn audio_decode_loop(command_rx: Receiver<AudioCommand>, engine: AudioEngine) {
        let mut current_format: Option<Box<dyn symphonia::core::formats::FormatReader>> = None;
        let mut current_decoder: Option<Box<dyn symphonia::core::codecs::Decoder>> = None;
        let mut track_id = 0;

        let mut resampler: Option<Async<f64>> = None;
        let mut resampler_rates: Option<(u32, u32, usize)> = None; 
        let mut resampler_in_buf: Vec<std::collections::VecDeque<f64>> = Vec::new(); 

        let state = engine.state.clone(); 
        let producer_mutex = engine.buffer_producer.clone();
        let mut channel_map = ChannelMap::default();

        // Zero-Allocation Pool Buffers: Pre-asignados fuera del bucle para evitar GC pressure.
        let mut audio_buf: Option<symphonia::core::audio::AudioBuffer<f64>> = None;
        let mut resample_input_pool: Vec<Vec<f64>> = Vec::new();
        let mut resample_output_pool: Vec<Vec<f64>> = Vec::new();
        let mut output_accumulator: Vec<f64> = Vec::with_capacity(131072);
        let mut output_accumulator_f32: Vec<f32> = Vec::with_capacity(131072);

        loop {
            // Limpieza TOTAL por ciclo para evitar que datos fantasmas (basura residual) se queden en el acumulador.
            // Esto erradica los pitidos y zumbidos al cambiar de canción o al procesar OGG irregulares.
            output_accumulator.clear();
            output_accumulator_f32.clear();

            // Check for commands
            let cmd_result = if current_format.is_none() {
                 command_rx.recv().map_err(|_| ())
            } else {
                 command_rx.try_recv().map_err(|_| ())
            };

            if let Ok(cmd) = cmd_result {
                match cmd {
                    AudioCommand::Load { path, title, artist, track_gain, album_gain } => {
                         {
                             let mut s = state.write();
                             s.title = title;
                             s.artist = artist;
                             s.path = path.clone();
                             s.replay_gain_track = track_gain.map(|g| g as f32);
                             s.replay_gain_album = album_gain.map(|g| g as f32);
                             
                             if track_gain.is_some() || album_gain.is_some() {
                                 tracing::info!("ReplayGain cargado para '{}': Track: {:?} dB, Album: {:?} dB", 
                                     s.title, track_gain, album_gain);
                             } else {
                                 tracing::debug!("No se encontraron metadatos de ReplayGain para '{}'", s.title);
                             }
                         }
                         
                         match File::open(&path) {
                             Ok(file) => {
                                 let mss = MediaSourceStream::new(Box::new(file), Default::default());
                                 let hint = Hint::new();
                                 let metadata_opts = MetadataOptions {
                                     limit_metadata_bytes: Limit::Maximum(0), // No cargar metadatos, ya los tenemos en la DB
                                     limit_visual_bytes: Limit::Maximum(0),
                                 };
                                 
                                 if let Ok(probed) = symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &metadata_opts) {
                                     let track = probed.format.default_track().unwrap();
                                     track_id = track.id;
                                     let sr = track.codec_params.sample_rate.unwrap_or(44100);
                                     let dur = track.codec_params.n_frames.map(|f| f as f64 / sr as f64).unwrap_or(0.0);
                                     state.write().total_duration_sec = dur;
                                     state.write().sample_rate = sr;

                                     // --- PURGA MAESTRA DE ESTADO (Anti-Residuos) ---
                                     resampler = None;
                                     resampler_rates = None;
                                     resampler_in_buf.clear();
                                     resample_input_pool.clear();
                                     resample_output_pool.clear();
                                     output_accumulator.clear();
                                     output_accumulator_f32.clear();
                                     audio_buf = None; // CRITICO: Evita reusar layout de buffer de canción anterior
                                     channel_map = ChannelMap::default();
                                     {
                                         if let Some(mut dsp_lock) = engine.dsp.try_write() {
                                             dsp_lock.reset_state();
                                         }
                                     }
                                     tracing::info!("Audio Engine State Purged (Load): Buffers & DSP Reset.");

                                     if let Ok(decoder) = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()) {
                                         current_decoder = Some(decoder);
                                         
                                         // Update Channel Map
                                         if let Some(channels) = track.codec_params.channels {
                                             channel_map = Self::get_channel_map(channels);
                                         } else {
                                             // Fallback for no layout
                                             let count = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);
                                             channel_map = ChannelMap::default();
                                             if count >= 1 { channel_map.fl = Some(0); }
                                             if count >= 2 { channel_map.fr = Some(1); }
                                         }
                                         
                                         current_format = Some(probed.format);
                                         let mut s = state.write();
                                         s.eof_reached = false;
                                     }
                                 }

                                 // Clear RingBuffer to remove old audio
                                 if let Some(consumer) = engine.buffer_consumer.lock().as_mut() {
                                     // Drain all available samples
                                     consumer.skip(usize::MAX);
                                 }
                             },
                             Err(e) => tracing::error!("Error abriendo archivo: {}", e),
                         }
                    }
                    AudioCommand::Seek(time) => {
                        if let Some(fmt) = current_format.as_mut() {
                             let _ = fmt.seek(symphonia::core::formats::SeekMode::Accurate, symphonia::core::formats::SeekTo::Time { 
                                 time: symphonia::core::units::Time::from(time), track_id: Some(track_id) 
                             });
                             {
                                 let mut s = state.write();
                                 s.current_pos_sec = time;
                             }
                             // Reset buffers (Purga en Seek)
                             resampler = None; 
                             resampler_rates = None;
                             resampler_in_buf.clear();
                             resample_input_pool.clear();
                             resample_output_pool.clear();
                             output_accumulator.clear();
                             output_accumulator_f32.clear();
                             audio_buf = None; // Reset buffer layout
                             {
                                 if let Some(mut dsp_lock) = engine.dsp.try_write() {
                                     dsp_lock.reset_state();
                                 }
                             }
                             tracing::info!("Audio Engine State Purged (Seek).");
                             
                             // Clear RingBuffer
                             if let Some(consumer) = engine.buffer_consumer.lock().as_mut() {
                                 consumer.skip(usize::MAX);
                             }
                        }
                    }
                    AudioCommand::Stop => {
                        current_format = None;
                        state.write().is_playing = false;
                        state.write().current_pos_sec = 0.0;
                    }
                }
            }

             // Control de Latencia (Virtual Buffer Size) limitando el RingBuffer
             let (out_rate, out_channels) = {
                 let s = state.read();
                 (s.device_sample_rate, s.channels as usize)
             };
             
             // Target: 100ms of safety margin to absorb CPU spikes at 384kHz
             let target_latency_samples = (out_rate as usize * out_channels * 100) / 1000;
             
             let should_wait = if let Some(producer) = engine.buffer_producer.lock().as_ref() {
                 producer.occupied_len() >= target_latency_samples
             } else { false };
             
              if should_wait {
                 std::thread::sleep(std::time::Duration::from_millis(1));
                 continue;
             }

             // 2. Llenar buffer si hay espacio
             let can_push = {
                  let prod = producer_mutex.lock();
                  if let Some(p) = prod.as_ref() {
                      // Umbral dinámico: Asegurar al menos 40ms de espacio libre para evitar starvation en MP3/7.1ch
                      let space_needed = (out_rate as usize * out_channels * 40) / 1000;
                      p.capacity().get() - p.occupied_len() > space_needed.max(2048)
                  } else { false }
             };

             if can_push {
                  let mut eof = false;
                  let dec_opt = current_decoder.as_mut();
                  let fmt_opt = current_format.as_mut();
                  
                  if let (Some(dec), Some(fmt)) = (dec_opt, fmt_opt) {
                      let packet = match fmt.next_packet() {
                          Ok(p) => p,
                          Err(symphonia::core::errors::Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                              eof = true;
                              symphonia::core::formats::Packet::new_from_slice(0, 0, 0, &[])
                          },
                          Err(e) => {
                              tracing::warn!("Symphonia decode error (skipping packet): {}", e);
                              continue; // Saltar paquetes corruptos en lugar de detener la canción
                          }
                      };
                      tracing::trace!("Packet next: ts={}, frames={}", packet.ts(), packet.dur());

                      if eof {
                          state.write().is_playing = false;
                          state.write().eof_reached = true;
                          
                          // Autoclean on EOF
                          let s = state.read();
                          if !s.is_playing {
                              drop(s);
                               // --- PURGA EN EOF ---
                               resampler = None;
                               resampler_rates = None;
                               resampler_in_buf.clear();
                               resample_input_pool.clear();
                               resample_output_pool.clear();
                               audio_buf = None; // Reset buffer layout
                               channel_map = ChannelMap::default();
                               {
                                   if let Some(mut dsp_lock) = engine.dsp.try_write() {
                                       dsp_lock.reset_state();
                                   }
                               }
                               let _ = engine.purge_buffers();
                               current_decoder = None;
                               current_format = None;
                               tracing::info!("Audio Engine State Purged (EOF).");
                          }
                          continue; 
                      }

                      if packet.track_id() != track_id { continue; }
                      
                      if let Ok(decoded) = dec.decode(&packet) {
                          let spec = *decoded.spec();
                          state.write().current_pos_sec = packet.ts() as f64 / spec.rate as f64;
                          
                           // Reuse or allocate audio_buf only when spec changes or capacity is insufficient
                           let needs_new_buf = audio_buf.as_ref().map(|b| b.spec() != &spec || b.capacity() < decoded.capacity()).unwrap_or(true);
                           if needs_new_buf {
                               audio_buf = Some(symphonia::core::audio::AudioBuffer::<f64>::new(decoded.capacity() as u64, spec));
                           }
                           
                           if let Some(ref mut buf) = audio_buf {
                               decoded.convert(buf);
                           }

                          let (out_rate, out_channels) = {
                              let s = state.read();
                              (s.device_sample_rate, s.channels)
                          };

                          // Upsampling de Ultra Alta Fidelidad: Sinc Interpolation a 64 bits.
                          // La interpolación Sinc con ventana Blackman-Harris y sobremuestreo masivo
                          // es teóricamente "perfecta", eliminando por completo el aliasing.
                           if spec.rate != out_rate {
                                let recreate = if let Some(stored) = resampler_rates {
                                    stored != (spec.rate, out_rate, spec.channels.count())
                                } else { true };
                               
                               if recreate {
                                   let params = SincInterpolationParameters {
                                       sinc_len: 256,
                                       f_cutoff: 0.99,
                                       interpolation: SincInterpolationType::Cubic,
                                       oversampling_factor: 256,
                                       window: WindowFunction::BlackmanHarris2,
                                   };
                                   
                                   match Async::<f64>::new_sinc(
                                       out_rate as f64 / spec.rate as f64,
                                       2.0,
                                       &params,
                                       1024,
                                       spec.channels.count(),
                                       FixedAsync::Input
                                   ) {
                                        Ok(r) => {
                                             resampler = Some(r);
                                             resampler_rates = Some((spec.rate, out_rate, spec.channels.count()));
                                             resampler_in_buf = (0..spec.channels.count())
                                                 .map(|_| std::collections::VecDeque::with_capacity(4096))
                                                 .collect();
                                            tracing::info!("Audiophile Resampler initialized: {} -> {} (SincFixedIn, 64-bit)", spec.rate, out_rate);
                                        },
                                        Err(e) => {
                                            tracing::error!("Resampler init failed: {}", e);
                                            resampler = None; 
                                        }
                                   }
                               }
                          } else {
                               if resampler.is_some() {
                                   resampler = None;
                                   resampler_in_buf.clear();
                                   resampler_rates = None;
                                }
                           }
 
                           // Ya no limpiamos aquí, se limpia al inicio del loop

                          let downmix_conf = {
                              let s = state.read();
                              (
                                  if s.downmix_center_enabled { s.downmix_center as f64 } else { 0.7071 },
                                  if s.downmix_lfe_enabled { s.downmix_lfe as f64 } else { 0.6666 },
                                  if s.downmix_surround_enabled { s.downmix_surround as f64 } else { 0.7671 },
                                  if s.downmix_surround_enabled { s.downmix_surround as f64 } else { 0.8071 },
                              )
                          };

                          if let (Some(rs), Some(ref buf)) = (resampler.as_mut(), audio_buf.as_ref()) {
                                      let planes = buf.planes();
                                      let frames = buf.frames();
                                      let src_channels = spec.channels.count();
                                      
                                      for c in 0..src_channels {
                                          if c < resampler_in_buf.len() {
                                              let plane = &planes.planes()[c][..frames];
                                              resampler_in_buf[c].extend(plane.iter().map(|&s| s as f64));
                                          }
                                      }
                                      
                                      if resample_input_pool.len() < src_channels { resample_input_pool.resize(src_channels, Vec::new()); }
                                      if resample_output_pool.len() < src_channels { resample_output_pool.resize(src_channels, Vec::new()); }

                                      loop {
                                          let needed = rs.input_frames_next();
                                          if resampler_in_buf.is_empty() || resampler_in_buf[0].len() < needed {
                                              break;
                                          }
                                          
                                          let out_frames = rs.output_frames_next();
                                          
                                          for c in 0..src_channels {
                                              resample_input_pool[c].clear();
                                              if c < resampler_in_buf.len() {
                                                  resample_input_pool[c].extend(resampler_in_buf[c].drain(0..needed));
                                              } else {
                                                  resample_input_pool[c].resize(needed, 0.0);
                                              }
                                              
                                              resample_output_pool[c].clear();
                                              resample_output_pool[c].resize(out_frames, 0.0);
                                          }
                                          
                                          let input_adapter = SequentialSliceOfVecs::new(&resample_input_pool, src_channels, needed).unwrap();
                                          let mut output_adapter = SequentialSliceOfVecs::new_mut(&mut resample_output_pool, src_channels, out_frames).unwrap();
                                          
                                          if let Ok(_) = rs.process_into_buffer(&input_adapter, &mut output_adapter, None) {
                                               tracing::trace!("Resampled: {} -> {} frames", needed, out_frames);
                                               Self::mix_channels_planar(&resample_output_pool, out_frames, src_channels, out_channels as usize, &channel_map, downmix_conf, &mut output_accumulator);
                                          }
                                      }
                           } else if let Some(ref buf) = audio_buf {
                               Self::mix_channels_direct(buf, buf.frames(), spec.channels.count(), out_channels as usize, &channel_map, downmix_conf, &mut output_accumulator);
                           }

                                   if !output_accumulator.is_empty() {
                                       output_accumulator_f32.clear();
                                       {
                                           let s = state.read();
                                           let mut gain_db: f64 = 0.0;
                                           if s.replay_gain_track_enabled {
                                               if let Some(tg) = s.replay_gain_track { gain_db += tg as f64; }
                                           }
                                           if s.replay_gain_album_enabled {
                                               if let Some(ag) = s.replay_gain_album { gain_db += ag as f64; }
                                           }

                                           let gain_linear = 10.0f64.powf(gain_db.min(12.0) / 20.0);
                                           let vol = s.volume as f64;
                                           
                                           // 1. Aplicar ReplayGain, DSP (EQ, etc.), y Volumen a `output_accumulator` en f64 nativo.
                                           {
                                               let out_ch = out_channels as usize;
                                               if let Some(mut dsp_lock) = engine.dsp.try_write() {
                                                   for frame in output_accumulator.chunks_mut(out_ch) {
                                                       for s in frame.iter_mut() { *s *= gain_linear; }
                                                       dsp_lock.process_frame(frame);
                                                       for s in frame.iter_mut() { *s *= vol; }
                                                   }
                                               } else {
                                                   // Si el DSP está bloqueado por la UI, aplicamos ganancia y volumen sin efectos para evitar tartamudeo (Stutter)
                                                   for frame in output_accumulator.chunks_mut(out_ch) {
                                                       for s in frame.iter_mut() { *s *= gain_linear * vol; }
                                                   }
                                                   tracing::debug!("DSP Lock busy: skipping effects to maintain real-time playback.");
                                               }
                                           }
                                           
                                           // Conversión limpia de f64 a f32 (Zero-Allocation pool)
                                           for &sample in output_accumulator.iter() {
                                               output_accumulator_f32.push(sample.clamp(-1.0, 1.0) as f32);
                                           }
                                       }
                                     if !output_accumulator_f32.is_empty() {
                                       // Push al RingBuffer (lock breve por iteración)
                                       // --- PUSHING ATÓMICO (FRAME ALIGNMENT) ---
                                       // Aseguramos que solo se envíen múltiplos exactos de `out_channels`.
                                       let mut pos = 0;
                                       while pos < output_accumulator_f32.len() {
                                           let mut pushed = 0;
                                           if let Some(producer) = producer_mutex.lock().as_mut() {
                                               let out_ch_usize = out_channels as usize;
                                               let remaining = output_accumulator_f32.len() - pos;
                                               let available = producer.capacity().get() - producer.occupied_len();
                                               
                                               let max_frames = available / out_ch_usize;
                                               let frames_to_push = (remaining / out_ch_usize).min(max_frames);
                                               
                                               if frames_to_push > 0 {
                                                   pushed = producer.push_slice(&output_accumulator_f32[pos..pos + (frames_to_push * out_ch_usize)]);
                                               }
                                           }
                                           
                                           if pushed == 0 {
                                               // Si no hay espacio para un frame completo, esperamos (Backoff)
                                               tracing::trace!("Push stalled: available < out_channels. Waiting...");
                                               std::thread::sleep(std::time::Duration::from_millis(2));
                                               if !state.read().is_playing { break; }
                                           } else {
                                               pos += pushed;
                                           }
                                       }
                                   }
                              }
                         }
                    }
               } else {
                          // Si el buffer está suficientemente lleno, dormimos poco (2ms) para reaccionar rápido
                          std::thread::sleep(std::time::Duration::from_millis(2));
                      }
        }
    }
    
    pub(crate) fn mix_channels_planar(input: &Vec<Vec<f64>>, frames: usize, in_channels: usize, out_channels: usize, map: &ChannelMap, dm_conf: (f64, f64, f64, f64), out_buf: &mut Vec<f64>) {
        for i in 0..frames {
             let fl = map.fl.and_then(|idx| input.get(idx)).map(|v| v[i]).unwrap_or(0.0);
             let fr = map.fr.and_then(|idx| input.get(idx)).map(|v| v[i]).unwrap_or(0.0); 
             let c = map.c.and_then(|idx| input.get(idx)).map(|v| v[i]).unwrap_or(0.0);
             let lfe = map.lfe.and_then(|idx| input.get(idx)).map(|v| v[i]).unwrap_or(0.0);
             let sl = map.sl.and_then(|idx| input.get(idx)).map(|v| v[i]).unwrap_or(0.0);
             let sr = map.sr.and_then(|idx| input.get(idx)).map(|v| v[i]).unwrap_or(0.0);
             let sbl = map.sbl.and_then(|idx| input.get(idx)).map(|v| v[i]).unwrap_or(0.0);
             let sbr = map.sbr.and_then(|idx| input.get(idx)).map(|v| v[i]).unwrap_or(0.0);

             Self::mix_sample_into_vec(out_buf, out_channels, fl, fr, c, lfe, sl, sr, sbl, sbr, in_channels, dm_conf);
        }
    }

    fn mix_channels_direct(buffer: &symphonia::core::audio::AudioBuffer<f64>, frames: usize, src_ch: usize, dst_ch: usize, map: &ChannelMap, dm_conf: (f64, f64, f64, f64), out_buf: &mut Vec<f64>) {
         let planes = buffer.planes();
         
         let get_sample = |plane_idx: usize, frame_idx: usize| -> f64 {
             if plane_idx < src_ch { planes.planes()[plane_idx][frame_idx] as f64 } else { 0.0 }
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
              
              Self::mix_sample_into_vec(out_buf, dst_ch, fl, fr, c, lfe, sl, sr, sbl, sbr, src_ch, dm_conf);
         }
    }


    // Helper para mapear canales de entrada a roles
    fn get_channel_map(channels: symphonia::core::audio::Channels) -> ChannelMap {
         use symphonia::core::audio::Channels;
         let mut map = ChannelMap::default();
         
         let mut index = 0;
         
         // Standard iterator order in Symphonia (WAVEFORMATEXTENSIBLE order)
         if channels.contains(Channels::FRONT_LEFT) { map.fl = Some(index); index += 1; }
         if channels.contains(Channels::FRONT_RIGHT) { map.fr = Some(index); index += 1; }
         if channels.contains(Channels::FRONT_CENTRE) { map.c = Some(index); index += 1; }
         if channels.contains(Channels::LFE1) { map.lfe = Some(index); index += 1; }
         if channels.contains(Channels::REAR_LEFT) { map.sbl = Some(index); index += 1; }
         if channels.contains(Channels::REAR_RIGHT) { map.sbr = Some(index); index += 1; }
         if channels.contains(Channels::FRONT_LEFT_CENTRE) { index += 1; }
         if channels.contains(Channels::FRONT_RIGHT_CENTRE) { index += 1; }
         if channels.contains(Channels::REAR_CENTRE) { index += 1; }
         if channels.contains(Channels::SIDE_LEFT) { map.sl = Some(index); index += 1; }
         if channels.contains(Channels::SIDE_RIGHT) { map.sr = Some(index); index += 1; }
         // Ignore other channels but increment index to keep sequence correct if they exist in valid stream
         // But here we are mapping logic logic purely for map construction.
         // If we don't use 'index' afterwards, and we don't use the map entries for these channels, we can just increment index or do nothing if we want to "skip" them in our map but they are present in the stream.
         // Actually, 'index' tracks the channel position in the planar/interleaved stream.
         // So we MUST increment index for every channel present in 'channels'.
         if channels.contains(Channels::TOP_CENTRE) { index += 1; }
         if channels.contains(Channels::TOP_FRONT_LEFT) { index += 1; }
         if channels.contains(Channels::TOP_FRONT_CENTRE) { index += 1; }
         if channels.contains(Channels::TOP_FRONT_RIGHT) { index += 1; }
         if channels.contains(Channels::TOP_REAR_LEFT) { index += 1; }
         if channels.contains(Channels::TOP_REAR_CENTRE) { index += 1; }
         if channels.contains(Channels::TOP_REAR_RIGHT) { index += 1; }
         
         let _ = index; // Silence unused variable warning at the end

         map
    }

    fn mix_sample_into_vec(out: &mut Vec<f64>, dst_ch: usize, mut fl: f64, mut fr: f64, c: f64, lfe: f64, sl: f64, sr: f64, sbl: f64, sbr: f64, src_ch_count: usize, dm_conf: (f64, f64, f64, f64)) {
        // Mono Expansion: Si el archivo es mono, distribuir a ambos canales frontales
        if src_ch_count == 1 {
            if fl == 0.0 && c != 0.0 { fl = c; }
            fr = fl;
        }

        let is_downmix = src_ch_count >= 6 && dst_ch < 6;
        let center_mix = if is_downmix { c * dm_conf.0 } else { c };
        let sur_mix_l = if is_downmix { sl * dm_conf.2 + sbl * dm_conf.3 } else { sl };
        let sur_mix_r = if is_downmix { sr * dm_conf.2 + sbr * dm_conf.3 } else { sr };
        let lfe_mix = if is_downmix { lfe * dm_conf.1 } else { lfe };

        match dst_ch {
             1 => { 
                 let sum = fl + fr + center_mix + lfe_mix + sur_mix_l + sur_mix_r;
                 out.push(sum * 0.5); 
             },
             2 => { 
                 out.push(fl + center_mix + lfe_mix + sur_mix_l);
                 out.push(fr + center_mix + lfe_mix + sur_mix_r);
             },
             3 => { 
                 out.push(fl + center_mix + sur_mix_l);
                 out.push(fr + center_mix + sur_mix_r);
                 out.push(lfe + (fl+fr+c+sl+sr)*0.1); 
             },
             4 => { 
                 out.push(fl + center_mix + lfe_mix);
                 out.push(fr + center_mix + lfe_mix);
                 out.push(sl + sbl);
                 out.push(sr + sbr);
             },
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
             },
             8 => { 
                 out.push(fl); 
                 out.push(fr); 
                 out.push(c); 
                 out.push(lfe);
                 out.push(sbl); // Back Left
                 out.push(sbr); // Back Right
                 out.push(sl);  // Side Left
                 out.push(sr);  // Side Right
             },
             _ => { 
                 let l = fl + c * 0.7 + sl + sbl;
                 let r = fr + c * 0.7 + sr + sbr;
                 out.push(l);
                 out.push(r); 
                 for _ in 2..dst_ch { out.push(0.0); }
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
