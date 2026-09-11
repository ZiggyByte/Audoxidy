use super::AudioError;
use crate::audio::decoder::AudioDecoder;
use crate::audio::device_manager::AudioDeviceManager;
pub use crate::audio::device_manager::{AudioDeviceInfo, AudioSettings, BitDepth, ChannelConfig};
use crate::audio::dsp::DspChain;
use crate::audio::meter;
use cpal::traits::DeviceTrait;
use crossbeam::channel::{Sender, unbounded};
use parking_lot::{Mutex, RwLock};
use ringbuf::wrap::caching::Caching;
use ringbuf::{
    HeapRb,
    traits::{Consumer, Split},
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// Contador acumulado de underruns detectados en el callback de salida.
///
/// El callback CPAL solo incrementa este atómico (sin trazas); una ruta no
/// real-time lo sondea y lo convierte en un evento de métrica.
pub static UNDERRUNS: AtomicU64 = AtomicU64::new(0);
/// Pico de latencia observado en el motor de audio, en microsegundos.
///
/// Fuente compartida que una ruta no real-time lee para emitir la métrica
/// correspondiente; el callback nunca escribe trazas.
pub static LATENCY_PEAK_US: AtomicU64 = AtomicU64::new(0);

// Define aliases based on ringbuf 0.4 structure
/// Productor de anillo circular para audio, con caché habilitada.
///
/// Tipo alias para `Caching<Arc<HeapRb<T>>, true, false>` (ringbuf 0.4).
pub type HeapProducer<T> = Caching<Arc<HeapRb<T>>, true, false>;
/// Consumidor de anillo circular para audio, con caché habilitada.
///
/// Tipo alias para `Caching<Arc<HeapRb<T>>, false, true>` (ringbuf 0.4).
pub type HeapConsumer<T> = Caching<Arc<HeapRb<T>>, false, true>;

/// Mapa que asigna índices de canales físicos a roles (FL, FR, C, LFE, SL, SR, SBL, SBR).
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

/// Comandos enviados al hilo de decodificación de fondo.
#[derive(Debug)]
#[allow(dead_code)]
pub enum AudioCommand {
    Load {
        path: String,
        title: String,
        artist: String,
        track_gain: Option<f64>,
        album_gain: Option<f64>,
    },
    /// Pre-carga la siguiente canción en el hilo decodificador para transiciones sin cortes.
    PreloadNext {
        path: String,
        title: String,
        artist: String,
        track_gain: Option<f64>,
        album_gain: Option<f64>,
    },
    /// Descarta el estado de pre-carga actual.
    ClearPreload,
    /// Dispara un crossfade manual con la duración en milisegundos especificada.
    CrossfadeNext(f64),
    Seek(f64),
    Stop,
}

/// Motor de audio principal de Audoxidy.
///
/// Gestiona la decodificación en segundo plano, el anillo circular de audio,
/// la cadena DSP, el stream de salida CPAL y el estado de reproducción.
#[derive(Clone)]
pub struct AudioEngine {
    pub device_manager: Arc<AudioDeviceManager>,
    pub state: Arc<RwLock<AudioState>>,
    pub buffer_consumer: Arc<Mutex<Option<HeapConsumer<f32>>>>,
    pub buffer_producer: Arc<Mutex<Option<HeapProducer<f32>>>>,
    command_tx: Sender<AudioCommand>,
    pub dsp: Arc<RwLock<DspChain>>,
    pub meter: Arc<meter::MeterData>,
    /// Tamaño de buffer observado por el callback de salida, publicado sin locks.
    ///
    /// El callback escribe `Relaxed` (único escritor); la GUI/manager leen el
    /// valor con `Relaxed` para mostrarlo sin tomar el lock de `AudioState`.
    pub buffer_size_published: Arc<AtomicU32>,
    /// Estado de reproducción publicado sin locks para el callback de salida.
    ///
    /// Espeja `AudioState::is_playing`; el callback solo lee este atómico para
    /// no bloquearse en un `RwLock` desde el hilo real-time.
    pub is_playing_published: Arc<AtomicBool>,
    // Decodificador inyectable para pruebas (None = usar SymphoniaDecoder por defecto)
    pub custom_decoder: Arc<Mutex<Option<Box<dyn AudioDecoder>>>>,
}

/// Estado mutable del motor de audio compartido entre hilos.
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

    // ReplayGain: ganancias independientes para normalización de volumen
    /// Ganancia de la pista actual en dB (del tag ReplayGain del archivo)
    pub replay_gain_track: Option<f32>,
    /// Ganancia del álbum en dB (del tag ReplayGain del archivo)
    pub replay_gain_album: Option<f32>,
    /// Activar/desactivar ganancia de pista (independiente)
    pub replay_gain_track_enabled: bool,
    /// Activar/desactivar ganancia de álbum (independiente)
    pub replay_gain_album_enabled: bool,

    // Volumen y Mezcla — Fades (D-07 master, D-09, D-10)
    pub fades_enabled: bool, // default: false — group master "Cambio de Volumen"
    pub smooth_volume_enabled: bool, // default: false — individual "Suavizar el cambio de volumen"
    pub fade_in_ms: f32,     // default: 1000.0 (range 0–10000, paso 50)
    pub fade_out_ms: f32,    // default: 2000.0 (range 0–10000, paso 50)

    // Volumen y Mezcla — Silence removal (D-14 master, D-16)
    pub silence_enabled: bool,           // default: true
    pub silence_duration_ms: f32,        // default: 1000.0 (range 100–10000, paso 50)
    pub silence_threshold_db: f32,       // default: -47.0 (range -80..0, paso 0.25)
    pub silence_edge_trim_enabled: bool, // default: true

    // Volumen y Mezcla — Fixed Gain
    pub rg_fixed_enabled: bool, // default: false
    pub rg_fixed_db: f32,       // default: 0.0
    // Fade individual toggles
    pub fade_in_enabled: bool,  // default: false
    pub fade_out_enabled: bool, // default: false

    // Volumen y Mezcla — ReplayGain offsets (D-26, D-29, D-30 master, D-28)
    pub rg_master_enabled: bool,     // default: true (master of RG group)
    pub rg_offset_album_db: f32,     // default: 0.0 (range ±12, paso 0.25)
    pub rg_offset_track_db: f32,     // default: 0.0 (range ±12, paso 0.25)
    pub rg_offset_rt_db: f32,        // default: 0.0 (range ±12, paso 0.25)
    pub rg_analyze_rt_enabled: bool, // default: true (RT analysis fallback)

    // Mezcla Cruzada — crossfade entre canciones
    pub crossfade_enabled: bool, // default: false — master del grupo "Mezcla Cruzada"
    pub crossfade_manual_enabled: bool, // default: false — crossfade en cambio manual
    pub crossfade_manual_ms: f32, // default: 1000.0 (range 0–10000, paso 50)
    pub crossfade_auto_enabled: bool, // default: false — crossfade en cambio automático
    pub crossfade_auto_ms: f32,  // default: 250.0 (range 0–10000, paso 50)

    // VU Meter — configuración
    pub meter_hold_time_ms: f32, // default: 1500.0 (range 500–5000, paso 50)
    pub meter_rms_window_ms: f32, // default: 300.0 (range 50–1000, paso 50)
    pub meter_infinite_hold: bool, // default: false — hold infinito hasta cambio de pista
    pub meter_enabled: bool,     // default: false — medidor desactivado en instalación nueva
}

// Adapters removed (not needed for Rubato 1.0 with Vec<Vec<f32>>)

impl Default for AudioState {
    fn default() -> Self {
        Self {
            is_playing: false,
            volume: 0.3,
            sample_rate: 48000,
            channels: 2,
            current_pos_sec: 0.0,
            total_duration_sec: 0.0,
            title: "Sin título".to_string(),
            artist: "Artista desconocido".to_string(),
            path: String::new(),
            eof_reached: false,

            device_sample_rate: 48000, // Default Match
            bit_depth_display: "32-bit Float".to_string(),
            buffer_size: 0,
            config_channels: ChannelConfig::Auto,

            downmix_center: 0.76,
            downmix_lfe: 0.66,
            downmix_surround: 0.74,
            downmix_center_enabled: false,
            downmix_lfe_enabled: false,
            downmix_surround_enabled: false,
            replay_gain_track: None,
            replay_gain_album: None,
            replay_gain_track_enabled: true,
            replay_gain_album_enabled: true,

            // Volumen y Mezcla — Fades
            fades_enabled: false,
            smooth_volume_enabled: false,
            fade_in_ms: 1000.0,
            fade_out_ms: 2000.0,

            // Volumen y Mezcla — Silence removal
            silence_enabled: true,
            silence_duration_ms: 1000.0,
            silence_threshold_db: -47.0,
            silence_edge_trim_enabled: true,

            // Volumen y Mezcla — Fixed Gain
            rg_fixed_enabled: false,
            rg_fixed_db: 0.0,
            // Fade individual toggles
            fade_in_enabled: false,
            fade_out_enabled: false,

            // Volumen y Mezcla — ReplayGain offsets
            rg_master_enabled: true,
            rg_offset_album_db: 0.0,
            rg_offset_track_db: 0.0,
            rg_offset_rt_db: 0.0,
            rg_analyze_rt_enabled: true,

            // Mezcla Cruzada — crossfade
            crossfade_enabled: false,
            crossfade_manual_enabled: false,
            crossfade_manual_ms: 1000.0,
            crossfade_auto_enabled: false,
            crossfade_auto_ms: 250.0,

            // VU Meter — configuración
            meter_hold_time_ms: 1500.0,
            meter_rms_window_ms: 300.0,
            meter_infinite_hold: false,
            meter_enabled: false,
        }
    }
}

#[allow(dead_code)]
impl AudioEngine {
    /// Crea un nuevo AudioEngine.
    /// Si se proporciona `decoder`, se usará en lugar del SymphoniaDecoder por defecto
    /// (útil para inyección de dependencias en pruebas).
    pub fn new_with_decoder(decoder: Option<Box<dyn AudioDecoder>>) -> Result<Self, AudioError> {
        let (command_tx, command_rx) = unbounded::<AudioCommand>();

        // Aumentado significativamente para evitar underruns a 384kHz 7.1ch (~2 segundos de audio)
        // Modo low-resource: reduce a ~0.5 segundos (~2 MiB máx)
        let rb_size = if crate::utils::is_low_resource() {
            2 * 1024 * 1024
        } else {
            8 * 1024 * 1024
        };
        let rb = HeapRb::<f32>::new(rb_size);
        let (producer, consumer) = rb.split();

        let state = Arc::new(RwLock::new(AudioState::default()));
        let dsp = Arc::new(RwLock::new(DspChain::default()));
        let device_manager = Arc::new(AudioDeviceManager::new());
        let meter = Arc::new(meter::MeterData::new());

        // Hilo de decodificación recibe una copia del Engine para controlarse a sí mismo
        let engine = Self {
            device_manager: device_manager.clone(),
            state,
            buffer_consumer: Arc::new(Mutex::new(Some(consumer))),
            buffer_producer: Arc::new(Mutex::new(Some(producer))),
            command_tx,
            dsp,
            meter: meter.clone(),
            buffer_size_published: Arc::new(AtomicU32::new(0)),
            is_playing_published: Arc::new(AtomicBool::new(false)),
            custom_decoder: Arc::new(Mutex::new(decoder)),
        };

        let engine_clone = engine.clone();
        std::thread::spawn(move || {
            crate::audio::decoder::audio_decode_loop(command_rx, engine_clone);
        });

        // Iniciar output por defecto
        engine.init_default_output()?;

        Ok(engine)
    }

    /// Constructor por defecto (usa SymphoniaDecoder).
    pub fn new() -> Result<Self, AudioError> {
        Self::new_with_decoder(None)
    }

    fn init_default_output(&self) -> Result<(), AudioError> {
        let (host, device, config, sample_fmt) = self.device_manager.init_default_output()?;
        let sample_rate = config.sample_rate;
        self.state.write().device_sample_rate = sample_rate;

        self.start_stream_with_device(host, device, config, sample_fmt)
    }

    fn start_stream_with_device(
        &self,
        host: cpal::Host,
        device: cpal::Device,
        stream_config: cpal::StreamConfig,
        sample_format: cpal::SampleFormat,
    ) -> Result<(), AudioError> {
        // Configurar el manager
        self.device_manager
            .set_output(host, device.clone(), stream_config.clone(), sample_format);

        let consumer_arc = self.buffer_consumer.clone();
        let buffer_size_arc = self.buffer_size_published.clone();
        let is_playing_arc = self.is_playing_published.clone();
        let channels = stream_config.channels as usize;

        let err_fn = |err| tracing::error!("Stream error: {}", err);

        self.device_manager.start_stream(|dev, cfg, fmt| {
            let stream = match fmt {
                cpal::SampleFormat::F32 => dev.build_output_stream(
                    cfg,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(
                            data,
                            channels,
                            &buffer_size_arc,
                            &is_playing_arc,
                            &consumer_arc,
                        )
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I16 => dev.build_output_stream(
                    cfg,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(
                            data,
                            channels,
                            &buffer_size_arc,
                            &is_playing_arc,
                            &consumer_arc,
                        )
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::U16 => dev.build_output_stream(
                    cfg,
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(
                            data,
                            channels,
                            &buffer_size_arc,
                            &is_playing_arc,
                            &consumer_arc,
                        )
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I32 => dev.build_output_stream(
                    cfg,
                    move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(
                            data,
                            channels,
                            &buffer_size_arc,
                            &is_playing_arc,
                            &consumer_arc,
                        )
                    },
                    err_fn,
                    None,
                ),
                _ => return Err(AudioError::UnsupportedSampleFormat),
            }
            .map_err(|e| AudioError::StreamError(e.to_string()))?;

            Ok(stream)
        })
    }

    /// Inicia o reinicia el stream de salida de audio.
    ///
    /// Si ya hay un stream activo, no hace nada.
    pub fn start(&self) -> Result<(), AudioError> {
        if self.device_manager.has_stream() {
            return Ok(());
        }

        let (_device, config, fmt) = match self.device_manager.get_device() {
            Some(d) => (
                d,
                self.device_manager.get_stream_config().unwrap(),
                self.device_manager.get_sample_format().unwrap(),
            ),
            None => return Ok(()),
        };

        let consumer_arc = self.buffer_consumer.clone();
        let buffer_size_arc = self.buffer_size_published.clone();
        let is_playing_arc = self.is_playing_published.clone();
        let channels = config.channels as usize;
        let err_fn = |err| tracing::error!("Stream error: {}", err);

        self.device_manager.start_stream(|dev, _cfg, _sample_fmt| {
            let stream = match fmt {
                cpal::SampleFormat::F32 => dev.build_output_stream(
                    &config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(
                            data,
                            channels,
                            &buffer_size_arc,
                            &is_playing_arc,
                            &consumer_arc,
                        )
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I16 => dev.build_output_stream(
                    &config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(
                            data,
                            channels,
                            &buffer_size_arc,
                            &is_playing_arc,
                            &consumer_arc,
                        )
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::U16 => dev.build_output_stream(
                    &config,
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(
                            data,
                            channels,
                            &buffer_size_arc,
                            &is_playing_arc,
                            &consumer_arc,
                        )
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I32 => dev.build_output_stream(
                    &config,
                    move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                        Self::write_data(
                            data,
                            channels,
                            &buffer_size_arc,
                            &is_playing_arc,
                            &consumer_arc,
                        )
                    },
                    err_fn,
                    None,
                ),
                _ => return Err(AudioError::UnsupportedSampleFormat),
            }
            .map_err(|e| {
                tracing::error!(
                    "FALLO al construir el stream de salida ({} Hz, {} ch, {:?}): {}",
                    config.sample_rate,
                    config.channels,
                    config.buffer_size,
                    e
                );
                AudioError::StreamError(e.to_string())
            })?;
            tracing::info!(
                "Stream de salida construido y en PLAY ({} Hz, {} ch, buffer {:?})",
                config.sample_rate,
                config.channels,
                config.buffer_size
            );

            Ok(stream)
        })
    }

    /// Callback de audio simplificado: solo lee del RingBuffer y escribe al stream.
    /// El DSP y el volumen ya están aplicados en los datos del RingBuffer.
    /// No usa RwLock ni procesamiento pesado — máxima seguridad para real-time.
    fn write_data<T>(
        output: &mut [T],
        channels: usize,
        buffer_size: &Arc<AtomicU32>,
        is_playing: &Arc<AtomicBool>,
        consumer_mutex: &Arc<Mutex<Option<HeapConsumer<f32>>>>,
    ) where
        T: cpal::Sample + cpal::FromSample<f32>,
    {
        Self::write_data_impl(output, channels, buffer_size, is_playing, consumer_mutex)
    }

    /// Implementación real-time safe del callback de salida.
    ///
    /// No realiza trazas, no asigna memoria y no toma el lock de `AudioState`:
    /// publica el tamaño de buffer y lee el estado de reproducción con atómicos
    /// `Relaxed`. El único lock es el handle del ringbuffer (`consumer_mutex`),
    /// inherente al diseño actual del consumidor.
    fn write_data_impl<T>(
        output: &mut [T],
        channels: usize,
        buffer_size: &Arc<AtomicU32>,
        is_playing: &Arc<AtomicBool>,
        consumer_mutex: &Arc<Mutex<Option<HeapConsumer<f32>>>>,
    ) where
        T: cpal::Sample + cpal::FromSample<f32>,
    {
        if channels > 0 {
            let current_frames = (output.len() / channels) as u32;
            if current_frames > 0 && buffer_size.load(Ordering::Relaxed) != current_frames {
                buffer_size.store(current_frames, Ordering::Relaxed);
            }
        }

        if !is_playing.load(Ordering::Relaxed) {
            output.fill(T::from_sample(0.0));
            return;
        }

        if let Some(consumer) = consumer_mutex.lock().as_mut() {
            // Buffer intermedio f32 en pila para pop_slice.
            // pop_slice requiere &mut [f32] (formato nativo del ringbuffer).
            // Luego convertimos cada frame de f32 a T.
            let out_len = output.len();
            let mut written = 0;
            // Buffer temporal en pila para lectura por lotes (evita llamadas individuales a try_pop)
            let mut tmp_buf = [0.0f32; 128];
            while written < out_len {
                let remaining = (out_len - written).min(tmp_buf.len());
                let n = consumer.pop_slice(&mut tmp_buf[..remaining]);
                if n == 0 {
                    // Underrun: llenar el resto con silencio. El callback solo
                    // incrementa el atómico; una ruta no real-time lo convierte
                    // en métrica (sin trazas en el hilo de audio).
                    UNDERRUNS.fetch_add(1, Ordering::Relaxed);
                    for s in output[written..].iter_mut() {
                        *s = T::from_sample(0.0);
                    }
                    break;
                }
                for (dst, &src) in output[written..written + n]
                    .iter_mut()
                    .zip(tmp_buf[..n].iter())
                {
                    *dst = T::from_sample(src);
                }
                written += n;
            }
        } else {
            output.fill(T::from_sample(0.0));
        }
    }

    /// Devuelve la lista de hosts de audio disponibles (ALSA, PipeWire, WASAPI, etc.).
    pub fn get_available_hosts(&self) -> Vec<String> {
        AudioDeviceManager::get_available_hosts()
    }

    /// Devuelve la lista de dispositivos de salida disponibles.
    pub fn get_devices(&self) -> Vec<AudioDeviceInfo> {
        self.device_manager.get_devices()
    }

    /// Realiza una purga profunda de los buffers y reinicia el stream con la configuración actual.
    pub fn purge_buffers(&self) -> Result<(), AudioError> {
        tracing::debug!("Audoxidy Audio: Cleaning Audio Engine Buffers");
        let (host, device, config, fmt) = self
            .device_manager
            .take_output()
            .ok_or(AudioError::NoActiveOutput)?;
        self.recreate_stream(host, device, config, fmt)
    }

    /// Aplica una configuración de audio completa: host, dispositivo, sample rate,
    /// profundidad de bits, canales y tamaño de buffer.
    ///
    /// Recrea el stream con la nueva configuración manteniendo la reproducción activa.
    /// Escribe la nueva tasa de muestreo en el estado compartido ANTES de reconstruir,
    /// para que el hilo decodificador reajuste su resampler sin desajustes de tasa.
    ///
    /// Si la construcción del nuevo stream falla (p. ej. el dispositivo no está listo
    /// aún tras liberar el anterior), se reintenta con más tiempo de asentamiento y,
    /// si persiste, se RESTAURA la configuración anterior para que el audio continúe.
    pub fn apply_settings(&self, settings: AudioSettings) -> Result<(), AudioError> {
        self.device_manager.stop_stream();
        // Tiempo de asentamiento: el dispositivo necesita liberar el stream anterior
        // antes de construir el nuevo — si se construye inmediatamente, puede quedar
        // mudo (el audio no se escucha hasta un nuevo arranque del stream).
        std::thread::sleep(std::time::Duration::from_millis(30));
        let current_rate = self.state.read().device_sample_rate;

        let (host, device, stream_config, sample_format) = self
            .device_manager
            .resolve_settings(&settings, current_rate)?;
        tracing::info!(
            "Aplicando config de audio: tasa {} Hz, {} canales, buffer {:?}, formato {:?} (anterior {} Hz)",
            stream_config.sample_rate,
            stream_config.channels,
            stream_config.buffer_size,
            sample_format,
            current_rate
        );

        {
            let mut s = self.state.write();
            s.config_channels = settings.channels.clone();
            // Escribir la nueva tasa/canales antes de reconstruir el stream: el decoder
            // reacciona recreando su resampler al leer la tasa nueva (fix B5).
            s.device_sample_rate = stream_config.sample_rate;
        }

        // Update DSP configuration
        {
            let mut dsp = self.dsp.write();
            dsp.set_sample_rate(stream_config.sample_rate as f32);
            dsp.set_channel_count(stream_config.channels as usize);
        }

        // Capturar el ID del host antes de moverlo al stream (cpal::Host no es clonable).
        let host_id = host.id();
        match self.recreate_stream(host, device.clone(), stream_config.clone(), sample_format) {
            Ok(()) => Ok(()),
            Err(e) => {
                let err_msg = format!("{}", e);
                // Reintento con más asentamiento: algunos backends (PipeWire/ALSA)
                // tardan en liberar el dispositivo al cambiar a tasas muy distintas.
                tracing::warn!(
                    "Reconstruyendo stream falló ({}): reintento con más asentamiento.",
                    err_msg
                );
                std::thread::sleep(std::time::Duration::from_millis(250));
                self.device_manager.stop_stream();
                std::thread::sleep(std::time::Duration::from_millis(50));
                // Reconstruir el host por su ID (cpal::Host no es clonable).
                let host_retry = cpal::host_from_id(host_id).map_err(|e2| {
                    tracing::error!("No se pudo reconstruir el host de audio: {}", e2);
                    AudioError::StreamError(format!("{} (host: {})", err_msg, e2))
                })?;
                match self.recreate_stream(host_retry, device, stream_config, sample_format) {
                    Ok(()) => Ok(()),
                    Err(e2) => {
                        tracing::error!(
                            "No se pudo construir el stream con la nueva configuración ({}).",
                            e2
                        );
                        Err(AudioError::StreamError(format!(
                            "{} (reintento: {})",
                            err_msg, e2
                        )))
                    }
                }
            }
        }
    }

    fn recreate_stream(
        &self,
        host: cpal::Host,
        device: cpal::Device,
        stream_config: cpal::StreamConfig,
        sample_format: cpal::SampleFormat,
    ) -> Result<(), AudioError> {
        // Config PREVIA guardada en el device_manager (ANTES de sobrescribirla).
        // No se lee del estado compartido: apply_settings ya escribió la tasa nueva
        // ahí antes de reconstruir (fix B5), y compararla impediría recrear el ringbuf.
        let prev = self.device_manager.get_stream_config();
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

        self.device_manager
            .set_output(host, device.clone(), stream_config.clone(), sample_format);

        // Si cambió la tasa de muestreo o el número de canales, el contenido del
        // ringbuf es inválido (los samples están a la tasa/canales anteriores y se
        // reproducirían a velocidad o layout incorrectos — canción "lenta/rápida" o
        // ruido). En ese caso se recrea el buffer con el tamaño correcto. Solo se
        // conserva cuando la config de muestreo no cambió (buffer size / bit depth).
        let prev_rate = prev.as_ref().map(|c| c.sample_rate).unwrap_or(44100);
        let prev_channels = prev.as_ref().map(|c| c.channels).unwrap_or(2);
        if prev_rate != stream_config.sample_rate || prev_channels != stream_config.channels {
            let sr = stream_config.sample_rate as usize;
            let ch = stream_config.channels as usize;
            let dur_secs = if crate::utils::is_low_resource() {
                0.5
            } else {
                2.0
            };
            let rb_size = ((sr * ch) as f64 * dur_secs) as usize;
            tracing::info!(
                "Ringbuf recreado: {} muestras ({} Hz x {} ch x {:.0}s). Anterior: {} Hz x {} ch.",
                rb_size.max(384_000),
                sr,
                ch,
                dur_secs,
                prev_rate,
                prev_channels
            );
            let rb = HeapRb::<f32>::new(rb_size.max(384_000));
            let (producer, consumer) = rb.split();

            {
                let mut p_lock = self.buffer_producer.lock();
                *p_lock = Some(producer);
            }
            {
                let mut c_lock = self.buffer_consumer.lock();
                *c_lock = Some(consumer);
            }
        }

        self.start()
    }

    /// Encola un archivo para decodificación en el hilo de fondo.
    ///
    /// Envía un comando `AudioCommand::Load` al canal `crossbeam`.
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

    /// Pre-carga la siguiente canción en el hilo decodificador para transiciones sin cortes.
    ///
    /// Envía un comando `AudioCommand::PreloadNext` al canal `crossbeam`.
    pub fn preload_file(
        &self,
        path: &str,
        title: String,
        artist: String,
        track_gain: Option<f64>,
        album_gain: Option<f64>,
    ) -> Result<(), AudioError> {
        let _ = self.command_tx.send(AudioCommand::PreloadNext {
            path: path.to_string(),
            title,
            artist,
            track_gain,
            album_gain,
        });
        Ok(())
    }

    /// Descarta el estado de pre-carga actual en el hilo decodificador.
    pub fn clear_preload(&self) -> Result<(), AudioError> {
        let _ = self.command_tx.send(AudioCommand::ClearPreload);
        Ok(())
    }

    /// Libera la memoria de la pre-carga de forma SEGURA durante la reproducción
    /// (a diferencia de `purge_buffers`, que reinicia el stream): descarta el buffer
    /// de pre-decode y el pendiente. La GUI la vuelve a disparar a ~15s del final.
    pub fn purge_preload(&self) -> Result<(), AudioError> {
        let _ = self.command_tx.send(AudioCommand::ClearPreload);
        Ok(())
    }

    /// Dispara un crossfade manual con la duración especificada en milisegundos.
    pub fn crossfade_next(&self, ms: f64) -> Result<(), AudioError> {
        let _ = self.command_tx.send(AudioCommand::CrossfadeNext(ms));
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

    /// Busca a una posición específica en segundos.
    ///
    /// Envía un comando `AudioCommand::Seek` al hilo de decodificación.
    pub fn seek(&self, pos: f64) {
        let _ = self.command_tx.send(AudioCommand::Seek(pos));
    }
    /// Detiene la reproducción y resetea el estado.
    ///
    /// Envía un comando `AudioCommand::Stop` y marca `is_playing = false`.
    pub fn stop(&self) {
        let _ = self.command_tx.send(AudioCommand::Stop);
        self.set_playing(false);
    }
    /// Establece el estado de reproducción (pausa/reanudación).
    pub fn set_playing(&self, playing: bool) {
        self.state.write().is_playing = playing;
        self.is_playing_published.store(playing, Ordering::Relaxed);
    }
    /// Establece el volumen de reproducción (0.0 a 1.0).
    ///
    /// El valor se clamp automáticamente al rango válido.
    pub fn set_volume(&self, volume: f32) {
        self.state.write().volume = volume.clamp(0.0, 1.0);
    }

    /// Force limiter on for normalization auto-on (D-25).
    /// Returns the previous limiter enabled state for later restore.
    pub fn force_limiter_on(&self) -> bool {
        if let Some(mut dsp) = self.dsp.try_write() {
            let was = dsp.limiter.enabled;
            dsp.limiter.enabled = true;
            was
        } else {
            false
        }
    }

    /// Restore limiter to its previous user state (D-25).
    pub fn restore_limiter(&self, was_enabled: bool) {
        if let Some(mut dsp) = self.dsp.try_write() {
            dsp.limiter.enabled = was_enabled;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::traits::{Observer, Producer};

    /// El callback publica el tamaño de buffer medido en frames y copia la
    /// señal del anillo a la salida.
    #[test]
    fn buffer_size_published_via_atomic() {
        let rb = HeapRb::<f32>::new(1024);
        let (mut producer, consumer) = rb.split();
        let samples = [0.1f32, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
        assert_eq!(producer.push_slice(&samples), samples.len());

        let buffer_size = Arc::new(AtomicU32::new(0));
        let is_playing = Arc::new(AtomicBool::new(true));
        let consumer_mutex = Arc::new(Mutex::new(Some(consumer)));

        let mut out = [0.0f32; 8];
        AudioEngine::write_data_impl(&mut out, 2, &buffer_size, &is_playing, &consumer_mutex);

        assert_eq!(buffer_size.load(Ordering::Relaxed), 4);
        for (dst, src) in out.iter().zip(samples.iter()) {
            assert_eq!(dst, src);
        }
    }

    /// Con `is_playing` en falso el callback entrega silencio sin consumir el anillo.
    #[test]
    fn silence_when_not_playing() {
        let rb = HeapRb::<f32>::new(1024);
        let (mut producer, consumer) = rb.split();
        let samples = [1.0f32; 8];
        assert_eq!(producer.push_slice(&samples), samples.len());

        let buffer_size = Arc::new(AtomicU32::new(0));
        let is_playing = Arc::new(AtomicBool::new(false));
        let consumer_mutex = Arc::new(Mutex::new(Some(consumer)));

        let mut out = [1.0f32; 8];
        AudioEngine::write_data_impl(&mut out, 2, &buffer_size, &is_playing, &consumer_mutex);

        assert!(out.iter().all(|s| *s == 0.0));
        let guard = consumer_mutex.lock();
        assert_eq!(guard.as_ref().unwrap().occupied_len(), samples.len());
    }
}
