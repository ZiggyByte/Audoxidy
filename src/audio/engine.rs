use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::Arc;
use std::fs::File;
use parking_lot::{RwLock, Mutex};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;
use symphonia::core::formats::FormatOptions;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::codecs::DecoderOptions;
use ringbuf::{HeapRb, traits::{Split, Consumer, Producer, Observer}};
use ringbuf::wrap::caching::Caching;

// Define aliases based on ringbuf 0.4 structure
pub type HeapProducer<T> = Caching<Arc<HeapRb<T>>, true, false>;
pub type HeapConsumer<T> = Caching<Arc<HeapRb<T>>, false, true>;

use audioadapter_buffers::direct::SequentialSliceOfVecs;
use symphonia::core::audio::Signal as SymphoniaSignal;
use crossbeam::channel::{Sender, Receiver, unbounded};
use lofty::file::TaggedFileExt;
use lofty::tag::Accessor;
use rubato::{Resampler, Fft, FixedSync};
// use audioadapter::Adapter; // Simplificado
use crate::audio::dsp::DspChain;

#[derive(Debug)]
pub enum AudioCommand {
    Load(String),
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
    fn default() -> Self { Self::Auto }
}

#[derive(Clone, Debug, Default)]
pub struct AudioSettings {
    pub host_id: Option<String>,
    pub device_name: Option<String>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<BitDepth>,
    pub channels: ChannelConfig,
    pub bit_perfect: bool,
    pub buffer_size: Option<u32>,
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

pub struct AudioEngine {
    output: Arc<RwLock<Option<AudioOutput>>>,
    pub state: Arc<RwLock<AudioState>>,
    // Ringbuf parts
    // Producer se mueve al hilo de decodificación
    // Consumer se guarda aquí para pasarlo al stream de cpal
    pub buffer_consumer: Arc<Mutex<Option<HeapConsumer<f32>>>>,

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
    pub album_art: Option<Vec<u8>>,
    pub bit_perfect: bool,
    pub bit_depth_display: String,
    pub buffer_size: u32,
}

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
            album_art: None,
            bit_perfect: false,
            bit_depth_display: "Unknown".to_string(),
            buffer_size: 0,
        }
    }
}

impl AudioEngine {
    pub fn new() -> Result<Self, String> {
        let (command_tx, command_rx) = unbounded::<AudioCommand>();
        
        // RingBuffer: Ajustado a 8192 samples para latencia ultra-baja (~185ms)
        let rb = HeapRb::<f32>::new(8192); 
        let (producer, consumer) = rb.split();

        let state = Arc::new(RwLock::new(AudioState::default()));
        let dsp = Arc::new(RwLock::new(DspChain::default()));
        
        let output = Arc::new(RwLock::new(None));

        // Hilo de decodificación recibe el Producer
        let decode_state = state.clone();
        std::thread::spawn(move || {
            Self::audio_decode_loop(command_rx, producer, decode_state);
        });

        let engine = Self {
            output,
            state,
            buffer_consumer: Arc::new(Mutex::new(Some(consumer))), 
            command_tx,
            dsp,
        };
        
        // Iniciar output por defecto
        engine.init_default_output()?;

        Ok(engine)
    }

    fn init_default_output(&self) -> Result<(), String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("No audio device found")?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        
        self.configure_and_start_stream(host, device, config.clone().into(), config.sample_format())?;
        Ok(())
    }

    // Helper para configurar y arrancar stream


    pub fn start(&self) -> Result<(), String> {
        // Alias para start_stream público si es necesario, pero start_stream es interno
        // Si el stream ya existe, cpal::Stream.play() ya lo maneja.
        // Pero aquí start() re-crea el stream si no existe.
        self.start_stream()
    }

    fn start_stream(&self) -> Result<(), String> {
        // Necesitamos mover el Consumer al closure del stream.
        // El Consumer NO es clonable.
        // Estrategia: "Robamos" el consumer del backup o del mutex, lo movemos al stream, 
        // y cuando el stream muere (drop), perdemos el consumer.
        // PERO: Si cambiamos de configuración, necesitamos recuperar el consumer o usar un mecanismo compartido.
        // Ringbuf SPSC es estricto. Solución: Mutex<Option<Consumer>> wrapper is shared. 
        // Inside callback: lock mutex, pop.
        // Esto añade overhead de bloqueo en cada callback, pero es seguro.
        
        // Asegurar que el consumer está en el Arc<Mutex> compartido
        // Unir consumer_backup al Arc si existe
       
        // Nota: self es inmutable aquí, pero necesitamos modificar campos internos
        // Usamos interior mutability ya existente en buffer_consumer (Mutex).
        
        // TRICK: Si tenemos el consumer en `buffer_consumer_backup` (porque es Self mut o init),
        // lo movemos al Arc. Pero `self` es &self.
        // Problema de diseño con SPSC en hot-reload.
        // Solución: AudioEngine tiene `buffer_consumer: Arc<Mutex<Option<Consumer>>>`.
        // Al inicio (new), metemos el consumer ahí.
        // El callback recibe un clon del Arc.
        
        // Solo necesitamos hacerlo una vez en new(), pero como `buffer_consumer_backup` es un campo
        // que añadí para "guardarlo", y `self` es `&self` aquí, no podemos moverlo fuera de `self`.
        // CORRECCIÓN: Usar `Mutex` para el `buffer_consumer_backup` también o simplemente 
        // inicializar el `buffer_consumer` Arc en `new` y olvidarnos del backup.
        
        // Refactor en `new` ya hecho: `buffer_consumer: Arc::new(Mutex::new(Some(consumer)))`.
        // Elimino `buffer_consumer_backup` de la estructura struct abajo.
         
        let mut out_lock = self.output.write();
        if let Some(output) = out_lock.as_mut() {
            if output.stream.is_some() { return Ok(()); } // Stop if already running

            let state = self.state.clone();
            let dsp = self.dsp.clone();
            let consumer_arc = self.buffer_consumer.clone();
            let channels = output.stream_config.channels as usize;

            let err_fn = |err| tracing::error!("Stream error: {}", err);

            let stream = match output.sample_format {
                cpal::SampleFormat::F32 => output.device.build_output_stream(
                    &output.stream_config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(data, channels, &state, &dsp, &consumer_arc)
                    },
                    err_fn,
                    None
                ),
                cpal::SampleFormat::I16 => output.device.build_output_stream(
                    &output.stream_config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(data, channels, &state, &dsp, &consumer_arc)
                    },
                    err_fn,
                    None
                ),
                cpal::SampleFormat::U16 => output.device.build_output_stream(
                    &output.stream_config,
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(data, channels, &state, &dsp, &consumer_arc)
                    },
                    err_fn,
                    None
                ),
                cpal::SampleFormat::I32 => output.device.build_output_stream(
                    &output.stream_config,
                    move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                         Self::write_data(data, channels, &state, &dsp, &consumer_arc)
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

    // Generic Data Writer using Sample trait
    fn write_data<T>(output: &mut [T], channels: usize, state: &Arc<RwLock<AudioState>>, dsp: &Arc<RwLock<DspChain>>, consumer_mutex: &Arc<Mutex<Option<HeapConsumer<f32>>>>)
    where T: cpal::Sample + cpal::FromSample<f32>
    {
        // Check playing info first to avoid lock contention if stopped
        // Using read lock is cheap
        let (is_playing, is_hifi, vol) = {
             let s = state.read();
             (s.is_playing, s.bit_perfect, s.volume)
        };

        if !is_playing {
            output.fill(T::from_sample(0.0));
            return;
        }

        // Lock consumer (Mutex ensures thread safety for the specific consumer access)
        // With SPSC and one stream, contention is low.
        if let Some(consumer) = consumer_mutex.lock().as_mut() {
             // Read from ringbuf
             // We need 'output.len()' samples
             
             if is_hifi {
                 // Bit Perfect: Pass-through (Volume is ignored in true Pass-through, but usually applied by player if not hardware encoded. 
                 // User request: "sin ningun procesamiento". So Volume = 1.0 implicit? or user volume ignored?
                 // "tampoco no podrá controlar el volumen del audio". So YES, ignore volume.
                 
                 for frame in output.chunks_mut(channels) {
                     for sample in frame {
                         *sample = T::from_sample(consumer.try_pop().unwrap_or(0.0f32));
                     }
                 }
             } else {
                 // DSP Process
                 let mut dsp_lock = dsp.write(); // Lock DSP once per callback block ideally
                 
                 for frame in output.chunks_mut(channels) {
                     for sample in frame {
                         if let Some(val) = consumer.try_pop() {
                             let mut p = val * vol;
                             dsp_lock.process(&mut p);
                             *sample = T::from_sample(p);
                         } else {
                             *sample = T::from_sample(0.0);
                         }
                     }
                 }
             }
        } else {
             // Consumer not available (should not happen)
             output.fill(T::from_sample(0.0));
        }
    }


    pub fn get_available_hosts(&self) -> Vec<String> {
        if cfg!(target_os = "linux") {
            // Check specifically for PulseAudio and PipeWire via ALSA plugin or native if enabled
            // CPAL on Linux uses ALSA by default. Jack is separate.
            // HostId::Alsa is usually available.
            // We want to return "PulseAudio", "PipeWire" if they are accessible via ALSA or other means.
            // But cpal only enumerates Hosts (Drivers).
            // On Linux: ALSA, Jack.
            // PulseAudio runs on top of ALSA usually, or ALSA has a pulse plugin.
            // If the user wants to select audio server, they usually select output device *within* ALSA that maps to Pulse/PipeWire.
            // OR they use the JACK host.
            
            // Let's list available CPAL hosts first.
            let hosts: Vec<String> = cpal::available_hosts().iter().map(|id| id.name().to_string()).collect();
            
            // Should we add fake "hosts" if we want to filter devices?
            // User request: "agrega las opciones de pipewire y pulse audio... muestren las salidas asociadas"
            // If we are using ALSA host, we can look for devices named "pulse", "pipewire", "default".
            // We'll handle this purely in device listing logic, but `get_available_hosts` needs to signal this capability?
            // Better: Return "ALSA", "JACK" (if compiled).
            // Filter strategy: Config UI logic (already done in plan).
            
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
        
        // Refetch host
        let host_id = cpal::available_hosts().into_iter().find(|h| h.name() == host_name).unwrap_or(cpal::default_host().id());
        let host = cpal::host_from_id(host_id).unwrap_or(cpal::default_host());

        if let Ok(devices) = host.output_devices() {
            devices.map(|d| {
                let name = d.name().unwrap_or("Unknown".into());
                let supported_configs = d.supported_output_configs().map(|c| c.collect()).unwrap_or_default();
                AudioDeviceInfo { name, supported_configs }
            }).collect()
        } else {
            Vec::new()
        }
    }

    pub fn apply_settings(&self, settings: AudioSettings) -> Result<(), String> {
        // Drop current stream
        {
            let mut out = self.output.write();
            if let Some(o) = out.as_mut() {
                o.stream = None; 
            }
        }

        // Host Selection Logic
        let target_host_id = if settings.bit_perfect && cfg!(target_os = "linux") {
             tracing::info!("Hi-Fi (Bit-Perfect): Forzando ALSA");
             cpal::HostId::Alsa
        } else if let Some(ref name) = settings.host_id {
             cpal::available_hosts().into_iter().find(|h| h.name() == name).ok_or("Host no encontrado")?
        } else {
             cpal::default_host().id()
        };

        let host = cpal::host_from_id(target_host_id).map_err(|e| e.to_string())?;

        // Device Selection
        let device = if let Some(ref dev_name) = settings.device_name {
            host.output_devices().map_err(|e| e.to_string())?
                .find(|d| d.name().unwrap_or_default() == *dev_name)
                .ok_or("Dispositivo no encontrado")?
        } else {
            host.default_output_device().ok_or("No default device")?
        };

        // Format/Config Selection
        let mut supported_configs: Vec<_> = device.supported_output_configs().map_err(|e| e.to_string())?.collect();
        
        // Filter by Bit Depth request if any
        if let Some(bd) = settings.bit_depth {
            let _ = match bd { // unused but keeps logic check structure if valuable or just remove
                BitDepth::Bits16 => [cpal::SampleFormat::I16, cpal::SampleFormat::U16].as_slice(),
                BitDepth::Bits24 => [cpal::SampleFormat::I32].as_slice(),
                BitDepth::Bits32Float => [cpal::SampleFormat::F32].as_slice(),
            };
            
            // Retain only supported formats
            // We use a separate vector to filter
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
                tracing::warn!("Requested bit depth not supported by device, falling back to auto");
            }
        }

        // Sort/Pick best config
        // Priority: 
        // 1. Channel count match (Stereo > Mono if Auto)
        // 2. Sample rate match
        
        // Requested Channels
        let req_channels = match settings.channels {
            ChannelConfig::Auto => 2, // Prefer Stereo
            ChannelConfig::Manual(c) => c,
        };

        let req_rate = settings.sample_rate;

        // Find best match
        let best_config_range = supported_configs.iter().fold(None, |best, current| {
             let cur_channels = current.channels();
             let cur_min_rate = current.min_sample_rate();
             let cur_max_rate = current.max_sample_rate();

             // Check channel match score
             let channel_score = if cur_channels == req_channels { 100 } else if cur_channels > req_channels { 50 } else { 0 };
             
             // Check rate match score
             let rate_score = if let Some(r) = req_rate {
                 if r >= cur_min_rate && r <= cur_max_rate { 100 } else { 0 }
             } else {
                 // Default preference: 44100 or 48000
                 if (cur_min_rate..=cur_max_rate).contains(&44100) { 10 } else { 0 }
             };

             let total_score = channel_score + rate_score;
             
             match best {
                 Some((score, cfg)) => if total_score > score { Some((total_score, current)) } else { Some((score, cfg)) },
                 None => Some((total_score, current))
             }
        }).map(|(_, c)| c).ok_or("No valid config found")?;

     // Construct final config
     let target_rate = if let Some(r) = req_rate {
          r
     } else {
          // Auto Algo
          // Note: .0 removed from min/max access
             let min = best_config_range.min_sample_rate();
             let max = best_config_range.max_sample_rate();
             
             if min <= 44100 && max >= 44100 { 44100 }
             else if min <= 48000 && max >= 48000 { 48000 }
             else { max }
        };

        // Create StreamConfig from SupportedStreamConfig
        let config = best_config_range.with_sample_rate(target_rate);
        let mut stream_config: cpal::StreamConfig = config.into();

     // Buffer Size / Quantum Rounding
     if let Some(frames) = settings.buffer_size {
          stream_config.buffer_size = cpal::BufferSize::Fixed(frames);
     } else {
          // Auto logic with Quantum Rounding described by user
          // "Si al finalizar el conteo del quantum... enviar al rango mayor mas cercano"
          // Calculate approx buffer for 10ms - 20ms latency as base
          let base_latency_ms = 10.0;
          let calculated_frames = (target_rate as f64 * base_latency_ms / 1000.0) as u32;
          
          // Next power of 2
          let mut quantum = 256;
          while quantum < calculated_frames && quantum < 8192 {
               quantum *= 2;
          }
           // Valid ranges: 256, 512, 1024, 2048, 4096, 8192
           if quantum > 8192 { quantum = 8192; }
           
           stream_config.buffer_size = cpal::BufferSize::Fixed(quantum);
     }

     // Fix borrowing key issue: config moved in into().
     // We need sample_format from best_config_range (which is Copy? No, it's reference/clone?) 
     // best_config_range is SupportedStreamConfigRange.
     // We can get sample_format from it.
     
     self.configure_and_start_stream(host, device, stream_config, best_config_range.sample_format())
    }

    // Adjusted signature to take StreamConfig directly + SampleFormat
    fn configure_and_start_stream(&self, host: cpal::Host, device: cpal::Device, stream_config: cpal::StreamConfig, sample_format: cpal::SampleFormat) -> Result<(), String> {
        // Actualizar State
        {
             let mut s = self.state.write();
            s.sample_rate = stream_config.sample_rate; 
            s.channels = stream_config.channels;
            s.bit_depth_display = match sample_format {
                cpal::SampleFormat::I16 => "16-bit Int",
                cpal::SampleFormat::U16 => "16-bit Int (U)",
                cpal::SampleFormat::I32 => "24/32-bit Int",
                cpal::SampleFormat::F32 => "32-bit Float",
                _ => "Unknown"
            }.to_string();
        }

        // Guardar Output info
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

        // Iniciar Stream
        self.start_stream()?;
        
        Ok(())
    }

    pub fn decode_file(&self, path: &str) -> Result<(), String> {
        let _ = self.command_tx.send(AudioCommand::Load(path.to_string()));
        Ok(())
    }

    // --- DECODER LOOP CORREGIDO ---
    // Updated signature: removed `self` (it's static), fixed Producer type.
    // Wait, I tried to add `&self` before, but errors said "audio_decode_loop doesn't have a self parameter".
    // I should NOT add `&self` because it is spawned in a thread with strict lifetime issues (unless Main Thread).
    // I will KEEP it static but pass `output` and `state`.
    // Actually, I removed the `configure_and_start_stream` call, so I DO NOT NEED `self` or `output` anymore?
    // YES. If I removed that call, `audio_decode_loop` works with `producer` and `command_rx`.
    // So current signature: `fn audio_decode_loop(..., producer: ..., state: ...)` is fine.
    // JUST FIX THE TYPE `ringbuf::heap::Producer`.
    fn audio_decode_loop(command_rx: Receiver<AudioCommand>, mut producer: HeapProducer<f32>, state: Arc<RwLock<AudioState>>) {
        let mut current_format: Option<Box<dyn symphonia::core::formats::FormatReader>> = None;
        let mut current_decoder: Option<Box<dyn symphonia::core::codecs::Decoder>> = None;
        let mut track_id = 0;
        


        let mut resampler: Option<Fft<f32>> = None;
        let mut resampler_in_buf: Vec<Vec<f32>> = Vec::new();
        let mut _resampler_out_buf: Vec<Vec<f32>> = Vec::new(); // Unused?
        
        // Buffer intermedio para acumular antes de enviar al ringbuf
        // Ayuda a reducir overhead de calls pequeños
        let mut output_accumulator: Vec<f32> = Vec::with_capacity(4096);

        loop {
            // Check commands
            let cmd_result = if current_format.is_none() {
                 command_rx.recv().map_err(|_| ())
            } else {
                 command_rx.try_recv().map_err(|_| ())
            };

            if let Ok(cmd) = cmd_result {
                match cmd {
                    AudioCommand::Load(path) => {
                         // ... (Load logic)
                         Self::update_metadata(&path, &state);
                         
                         // Intentar abrir archivo
                         match File::open(&path) {
                             Ok(file) => {
                                 let mss = MediaSourceStream::new(Box::new(file), Default::default());
                                 let hint = Hint::new();
                                 if let Ok(probed) = symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default()) {
                                     let track = probed.format.default_track().unwrap();
                                     track_id = track.id;
                                     let track_info_rate = track.codec_params.sample_rate.unwrap_or(44100);
                                     let _track_info_channels = track.codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);
                                     
                                     let dur = track.codec_params.n_frames.map(|f| f as f64 / track_info_rate as f64).unwrap_or(0.0);
                                     state.write().total_duration_sec = dur;

                                     if let Ok(decoder) = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()) {
                                         current_format = Some(probed.format);
                                         current_decoder = Some(decoder);
                                         resampler = None; // Reset resampler
                                     }
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
                             state.write().current_pos_sec = time;
                             resampler = None; 
                        }
                    }
                    AudioCommand::Stop => {
                        current_format = None;
                        state.write().is_playing = false;
                        state.write().current_pos_sec = 0.0;
                    }
                }
            }

            if let (Some(fmt), Some(dec)) = (current_format.as_mut(), current_decoder.as_mut()) {
                 if let Ok(packet) = fmt.next_packet() {
                     if packet.track_id() != track_id { continue; }
                     
                     if let Ok(decoded) = dec.decode(&packet) {
                         let spec = *decoded.spec();
                         state.write().current_pos_sec = packet.ts() as f64 / spec.rate as f64;
                         
                         let mut audio_buf = symphonia::core::audio::AudioBuffer::<f32>::new(decoded.capacity() as u64, spec);
                         decoded.convert(&mut audio_buf);
                         
                         // Get Output Config
                         let (out_rate, out_channels) = {
                             let s = state.read();
                             (s.sample_rate, s.channels)
                         };

                         // Init Resampler if needed
                         if resampler.is_none() && spec.rate != out_rate {
                             let rs = Fft::<f32>::new(spec.rate as usize, out_rate as usize, 1024, spec.channels.count(), spec.channels.count(), FixedSync::Both);
                             match rs {
                                 Ok(r) => {
                                      resampler = Some(r);
                                      resampler_in_buf = vec![Vec::with_capacity(2048); spec.channels.count()];
                                      // resampler_out_buf no se usa con process_into
                                      tracing::info!("Resampler init: {} -> {}", spec.rate, out_rate);
                                 },
                                 Err(e) => tracing::error!("Resampler init failed: {}", e)
                             }
                         }

                         // Process Audio
                         let planes = audio_buf.planes();
                         let frames = audio_buf.frames();
                         let src_channels = spec.channels.count();

                         output_accumulator.clear();

                         if let Some(rs) = resampler.as_mut() {
                             // Resampling Path
                             for c in 0..src_channels {
                                 if let Some(plane) = planes.planes().get(c) {
                                     resampler_in_buf[c].extend_from_slice(&plane[..frames]);
                                 }
                             }

                             let frames_needed = rs.input_frames_next();
                             while resampler_in_buf[0].len() >= frames_needed {
                                  let mut chucks: Vec<Vec<f32>> = Vec::new();
                                  for c in 0..src_channels {
                                      chucks.push(resampler_in_buf[c].drain(0..frames_needed).collect());
                                  }
                                  
                                  let max_out = (frames_needed as f64 * out_rate as f64 / spec.rate as f64).ceil() as usize + 10;
                                  let mut out_workspace = vec![vec![0.0; max_out]; src_channels];
                                  
                                  // Rubato 1.0+ process takes wrappers implementing Adapter
                                  let input_adapter = SequentialSliceOfVecs::new(&chucks, src_channels, frames_needed).unwrap();
                                  let mut output_adapter = SequentialSliceOfVecs::new_mut(&mut out_workspace, src_channels, max_out).unwrap();
                                  
                                  // use process_into_buffer
                                  match rs.process_into_buffer(&input_adapter, &mut output_adapter, None) {
                                      Ok(_) => {
                                          let out_written = (frames_needed as f64 * out_rate as f64 / spec.rate as f64).ceil() as usize; 
                                          let mixed = Self::mix_channels_planar(&out_workspace, out_written, src_channels, out_channels as usize);
                                          output_accumulator.extend(mixed);
                                      },
                                      Err(e) => tracing::error!("Resample error: {}", e)
                                  }

                                  
                                  if resampler_in_buf[0].len() < rs.input_frames_next() { break; }
                             }

                         } else {
                             // Pass AudioBuffer ref to mix helper
                             let mixed = Self::mix_channels_direct(&audio_buf, frames, src_channels, out_channels as usize);
                             output_accumulator.extend(mixed);
                         }

                         let mut pos = 0;
                         while pos < output_accumulator.len() {
                             if producer.is_full() {
                                 std::thread::sleep(std::time::Duration::from_millis(5));
                                 if !state.read().is_playing { break; } 
                                 continue;
                             }
                             let count = producer.push_slice(&output_accumulator[pos..]);
                             pos += count;
                         }
                     }
                 } else {
                     state.write().is_playing = false;
                     current_format = None;
                 }
            } else {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }

    // Mix Helpers
    fn mix_channels_direct(buffer: &symphonia::core::audio::AudioBuffer<f32>, frames: usize, src_ch: usize, dst_ch: usize) -> Vec<f32> {
         let planes = buffer.planes();
         let mut out = Vec::with_capacity(frames * dst_ch);
         for i in 0..frames {
             if src_ch == dst_ch {
                 // Copia directa
                 for c in 0..src_ch {
                     out.push(planes.planes()[c][i]);
                 }
             } else if src_ch == 2 && dst_ch == 3 { // Stereo -> 2.1
                 let l = planes.planes()[0][i];
                 let r = planes.planes()[1][i];
                 out.push(l); // L
                 out.push(r); // R
                 out.push((l + r) * 0.5); // Sub (L+R/2) estimate if hardware allows or expects LFE
             } else if src_ch == 2 && dst_ch > 2 { // Stereo -> 5.1/7.1 Upmix (Simple duplication/silence strategy)
                 let l = planes.planes()[0][i];
                 let r = planes.planes()[1][i];
                 out.push(l); // L
                 out.push(r); // R
                 // Center? Sub? Surround? Usually silent or duplicated depending on desired effect.
                 // User mentioned: "no se mezclan bien... aleatorio".
                 // Standard Upmix: Center = (L+R)*0.5, Sub = (L+R)*0.1?
                 // Or Keep silent for strict stereo?
                 // Let's implement active upmix for Center/Sub if requested for 2.1+ but mainly L/R.
                 // Standard for 5.1: L, R, C, LFE, SL, SR.
                 // For now, fill rest with 0.0 to avoid noise, unless 2.1 where user wants sub?
                 // Let's implement simple L/R and silence others for safety, user complained about random noise.
                 for _ in 2..dst_ch {
                     out.push(0.0);
                 }
             } else if src_ch > 2 && dst_ch == 2 { // Downmix Multi -> Stereo
                 // Standard ITU-R BS.775 Downmix
                 // 5.1 -> Stereo:
                 // L' = L + 0.707*C + 0.707*SL
                 // R' = R + 0.707*C + 0.707*SR
                 // Simplified Generic Downmix: Average all to L/R based on odd/even? No.
                 // Just take first two if mapped correctly? No, center channel info lost.
                 // Let's do a basic average downmix:
                 // L = Sum(Even Channels) * Scale? No.
                 // Safe approach: L = Ch0, R = Ch1 (if standard mapping).
                 // PLUS: Mix Center (Ch2бычно) into both. Mix LFE (Ch3) into both?
                 // If > 2, assume standard layout?
                 // Safe fallback: Just use L/R (0,1).
                 // User complaint: "no se mezclan bien... a veces uno...".
                 // Let's stick to L=Plane0, R=Plane1 + 0.5*Plane2 (Center) + 0.5*Plane3(LFE)? 
                 // Assuming standard 5.1 order: L, R, C, LFE, SL, SR.
                 let l = planes.planes()[0][i];
                 let r = planes.planes()[1][i];
                 let c = if src_ch > 2 { planes.planes()[2][i] } else { 0.0 };
                 // let lfe = if src_ch > 3 { planes.planes()[3][i] } else { 0.0 };

                 out.push((l + c * 0.707).clamp(-1.0, 1.0));
                 out.push((r + c * 0.707).clamp(-1.0, 1.0));
             } else if src_ch == 1 && dst_ch >= 2 { // Mono -> Stereo/Multi
                 let m = planes.planes()[0][i];
                 out.push(m);
                 out.push(m);
                 for _ in 2..dst_ch { out.push(0.0); }
             } else {
                 // Generic Truncate/Zero fill
                 for c in 0..dst_ch {
                     if c < src_ch { out.push(planes.planes()[c][i]); } else { out.push(0.0); }
                 }
             }
         }
         out
    }


    fn mix_channels_planar(planar: &Vec<Vec<f32>>, frames: usize, src_ch: usize, dst_ch: usize) -> Vec<f32> {
        let mut out = Vec::with_capacity(frames * dst_ch);
         for i in 0..frames {
             if src_ch == dst_ch {
                 for c in 0..src_ch {
                     out.push(planar[c][i]);
                 }
             } else if src_ch == 2 && dst_ch == 3 { // Stereo -> 2.1
                 let l = planar[0][i];
                 let r = planar[1][i];
                 out.push(l);
                 out.push(r);
                 out.push((l + r) * 0.5);
             } else if src_ch == 2 && dst_ch > 2 { // Stereo -> 5.1/7.1 Upmix
                 let l = planar[0][i];
                 let r = planar[1][i];
                 out.push(l);
                 out.push(r);
                 for _ in 2..dst_ch { out.push(0.0); }
             } else if src_ch > 2 && dst_ch == 2 { // Downmix Multi -> Stereo
                 let l = planar[0][i];
                 let r = planar[1][i];
                 let c = if src_ch > 2 { planar[2][i] } else { 0.0 };
                 
                 out.push((l + c * 0.707).clamp(-1.0, 1.0));
                 out.push((r + c * 0.707).clamp(-1.0, 1.0));
             } else if src_ch == 1 && dst_ch >= 2 { // Mono -> Stereo/Multi
                 let m = planar[0][i];
                 out.push(m);
                 out.push(m);
                 for _ in 2..dst_ch { out.push(0.0); }
             } else {
                 for c in 0..dst_ch {
                     if c < src_ch { out.push(planar[c][i]); } else { out.push(0.0); }
                 }
             }
         }
         out
    }

    // ... (Seek, Stop, etc. siguen igual o similares)
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
    
    fn update_metadata(path: &str, state: &Arc<RwLock<AudioState>>) {
        let mut title = "Sin título".to_string();
        let mut artist = "Artista desconocido".to_string();
        let mut album_art = None;
        if let Ok(tagged_file) = lofty::read_from_path(path) {
            if let Some(tag) = tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) {
                title = tag.title().as_deref().unwrap_or("Sin título").to_string();
                artist = tag.artist().as_deref().unwrap_or("Artista desconocido").to_string();
                 if let Some(picture) = tag.pictures().first() {
                    album_art = Some(picture.data().to_vec());
                }
            }
        }
        let mut s = state.write();
        s.title = title;
        s.artist = artist;
        s.album_art = album_art;
    }
}
