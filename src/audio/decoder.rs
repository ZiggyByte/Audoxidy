//! Background audio decoding thread — handles format reading, resampling, DSP,
//! and pushing decoded audio into the ring buffer shared with the CPAL callback.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::meta::{MetadataOptions, Limit};
use symphonia::core::codecs::{DecoderOptions, Decoder};
use symphonia::core::audio::{Signal, AudioBuffer};
use ringbuf::traits::{Consumer, Producer, Observer};
use crossbeam::channel::Receiver;
use rubato::{Async, FixedAsync, Resampler, SincInterpolationType, SincInterpolationParameters, WindowFunction};
use audioadapter_buffers::direct::SequentialSliceOfVecs;

/// Umbral para usar memoria mapeada en lugar de File normal (> 10 MB)
const MEMMAP_THRESHOLD: u64 = 10 * 1024 * 1024;

/// Información técnica del stream de audio decodificado.
#[derive(Clone, Debug)]
pub struct DecodeStreamInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub total_duration_sec: f64,
    pub channel_count: usize,
}

/// Paquete de audio decodificado listo para procesamiento.
pub struct DecodedPacket {
    pub data: Vec<f64>,
    pub frames: usize,
    pub channels: usize,
    pub sample_rate: u32,
}

/// Trait que abstrae la decodificación de archivos de audio.
/// Permite usar diferentes backends (Symphonia, miniaudio, etc.)
/// y facilita el mocking en pruebas unitarias.
pub trait AudioDecoder: Send {
    /// Abre un archivo de audio y devuelve información del stream.
    fn open(&mut self, path: &str) -> Result<DecodeStreamInfo, super::AudioError>;

    /// Decodifica el siguiente paquete de audio.
    /// Devuelve `Ok(None)` cuando se alcanza el final del archivo.
    fn decode_next(&mut self) -> Result<Option<DecodedPacket>, super::AudioError>;

    /// Busca a una posición específica en segundos.
    fn seek(&mut self, time_secs: f64) -> Result<(), super::AudioError>;

    /// Reinicia el estado interno del decodificador.
    fn reset(&mut self);
}

/// Decodificador basado en Symphonia (backend por defecto).
///
/// Soporta MP3, FLAC, WAV, OGG, M4A, AAC, APE, Opus, WavPack y más.
pub struct SymphoniaDecoder {
    format: Option<Box<dyn FormatReader>>,
    decoder: Option<Box<dyn Decoder>>,
    track_id: u32,
}

impl SymphoniaDecoder {
    /// Crea un nuevo decodificador Symphonia sin estado interno.
    pub fn new() -> Self {
        Self { format: None, decoder: None, track_id: 0 }
    }
}

impl AudioDecoder for SymphoniaDecoder {
    fn open(&mut self, path: &str) -> Result<DecodeStreamInfo, super::AudioError> {
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::Limit;

        let source = open_audio_source(path)
            .map_err(super::AudioError::IoError)?;
        let mss = MediaSourceStream::new(source, Default::default());
        let hint = symphonia::core::probe::Hint::new();
        let metadata_opts = symphonia::core::meta::MetadataOptions {
            limit_metadata_bytes: Limit::Maximum(0),
            limit_visual_bytes: Limit::Maximum(0),
        };

        let probed = symphonia::default::get_probe().format(
            &hint, mss, &symphonia::core::formats::FormatOptions::default(), &metadata_opts,
        ).map_err(|e| super::AudioError::ConfigError(e.to_string()))?;

        let track = probed.format.default_track()
            .ok_or(super::AudioError::ConfigError("No default track".into()))?;
        self.track_id = track.id;
        let sr = track.codec_params.sample_rate.unwrap_or(44100);
        let dur = track.codec_params.n_frames
            .map(|f| f as f64 / sr as f64)
            .unwrap_or(0.0);
        let ch_count = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);

        let dec = symphonia::default::get_codecs()
            .make(&track.codec_params, &symphonia::core::codecs::DecoderOptions::default())
            .map_err(|e| super::AudioError::ConfigError(e.to_string()))?;

        self.format = Some(probed.format);
        self.decoder = Some(dec);

        Ok(DecodeStreamInfo {
            sample_rate: sr,
            channels: ch_count as u16,
            total_duration_sec: dur,
            channel_count: ch_count,
        })
    }

    fn decode_next(&mut self) -> Result<Option<DecodedPacket>, super::AudioError> {
        use symphonia::core::audio::AudioBuffer;

        let fmt = self.format.as_mut()
            .ok_or(super::AudioError::ConfigError("No format loaded".into()))?;
        let dec = self.decoder.as_mut()
            .ok_or(super::AudioError::ConfigError("No decoder loaded".into()))?;

        loop {
            let packet = match fmt.next_packet() {
                Ok(p) => p,
                Err(symphonia::core::errors::Error::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
                Err(e) => {
                    tracing::warn!("Symphonia decode error: {}", e);
                    continue;
                }
            };

            if packet.track_id() != self.track_id {
                continue;
            }

            if let Ok(decoded) = dec.decode(&packet) {
                let spec = *decoded.spec();
                let frames = decoded.frames();
                let channels = spec.channels.count();
                let sr = spec.rate;

                // Convertir a f64 plano (interleaved)
                let mut buf = AudioBuffer::<f64>::new(frames as u64, spec);
                decoded.convert(&mut buf);
                let planes = buf.planes();

                let mut data = Vec::with_capacity(frames * channels);
                for i in 0..frames {
                    for ch in 0..channels {
                        data.push(planes.planes()[ch][i]);
                    }
                }

                return Ok(Some(DecodedPacket {
                    data,
                    frames,
                    channels,
                    sample_rate: sr,
                }));
            }
        }
    }

    fn seek(&mut self, time_secs: f64) -> Result<(), super::AudioError> {
        if let Some(fmt) = self.format.as_mut() {
            fmt.seek(
                symphonia::core::formats::SeekMode::Accurate,
                symphonia::core::formats::SeekTo::Time {
                    time: symphonia::core::units::Time::from(time_secs),
                    track_id: Some(self.track_id),
                },
            ).ok();
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.format = None;
        self.decoder = None;
        self.track_id = 0;
    }
}

/// Wrapper que presenta un Mmap como un MediaSource para Symphonia.
/// Los archivos grandes (>10MB) se mapean a memoria para evitar copias
/// y reducir presión en el page cache del kernel.
struct MmapSource {
    mmap: memmap2::Mmap,
    pos: usize,
}

unsafe impl Send for MmapSource {}
unsafe impl Sync for MmapSource {}

impl Read for MmapSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self.mmap.len().saturating_sub(self.pos);
        let to_read = buf.len().min(remaining);
        buf[..to_read].copy_from_slice(&self.mmap[self.pos..self.pos + to_read]);
        self.pos += to_read;
        Ok(to_read)
    }
}

impl Seek for MmapSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.pos = match pos {
            SeekFrom::Start(p) => p as usize,
            SeekFrom::End(p) => self.mmap.len().saturating_add_signed(p as isize),
            SeekFrom::Current(p) => self.pos.saturating_add_signed(p as isize),
        };
        self.pos = self.pos.min(self.mmap.len());
        Ok(self.pos as u64)
    }
}

impl MediaSource for MmapSource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.mmap.len() as u64)
    }
}

/// Abre un archivo de audio usando memoria mapeada si es grande, o File normal si no.
fn open_audio_source(path: &str) -> std::io::Result<Box<dyn MediaSource>> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > MEMMAP_THRESHOLD {
        // unsafe: el mapeo es válido mientras el archivo exista (lo hacemos para lectura)
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        tracing::debug!(
            "MemMap audio source ({} MB) para {}",
            metadata.len() / (1024 * 1024),
            path
        );
        Ok(Box::new(MmapSource { mmap, pos: 0 }))
    } else {
        Ok(Box::new(file))
    }
}

use crate::audio::engine::{AudioEngine, AudioCommand, ChannelMap};

pub(crate) fn audio_decode_loop(command_rx: Receiver<AudioCommand>, engine: AudioEngine) {
    let mut current_format: Option<Box<dyn FormatReader>> = None;
    let mut current_decoder: Option<Box<dyn Decoder>> = None;
    let mut track_id = 0;

    let mut resampler: Option<Async<f64>> = None;
    let mut resampler_rates: Option<(u32, u32, usize)> = None;
    let mut resampler_in_buf: Vec<std::collections::VecDeque<f64>> = Vec::new();

    let state = engine.state.clone();
    let producer_mutex = engine.buffer_producer.clone();
    let mut channel_map = ChannelMap::default();

    // Zero-Allocation Pool Buffers: Pre-asignados fuera del bucle para evitar GC pressure.
    let mut audio_buf: Option<AudioBuffer<f64>> = None;
    let mut resample_input_pool: Vec<Vec<f64>> = Vec::new();
    let mut resample_output_pool: Vec<Vec<f64>> = Vec::new();
    let mut output_accumulator: Vec<f64> = Vec::with_capacity(131072);
    let mut output_accumulator_f32: Vec<f32> = Vec::with_capacity(131072);

    // bumpalo arena para asignaciones temporales por ciclo.
    // Se resetea completo al inicio de cada iteración, liberando toda la memoria
    // sin necesidad de drop individual. Ideal para pequeños Vecs temporales.
    let mut arena = bumpalo::Bump::new();

    loop {
        // Resetear arena bumpalo: todas las asignaciones temporales del ciclo anterior
        // se liberan en O(1) — sin recorrer cada Vec individualmente.
        arena.reset();

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
                AudioCommand::Load {
                    path,
                    title,
                    artist,
                    track_gain,
                    album_gain,
                } => {
                    // Check for custom decoder (DI/mock support)
                    let has_custom_decoder = engine.custom_decoder.lock().is_some();

                    {
                        let mut s = state.write();
                        s.title = title;
                        s.artist = artist;
                        s.path = path.clone();
                        s.replay_gain_track = track_gain.map(|g| g as f32);
                        s.replay_gain_album = album_gain.map(|g| g as f32);

                        if track_gain.is_some() || album_gain.is_some() {
                            tracing::info!(
                                "ReplayGain cargado para '{}': Track: {:?} dB, Album: {:?} dB",
                                s.title,
                                track_gain,
                                album_gain
                            );
                        } else {
                            tracing::debug!(
                                "No se encontraron metadatos de ReplayGain para '{}'",
                                s.title
                            );
                        }
                    }

                    if has_custom_decoder {
                        if let Some(dec) = engine.custom_decoder.lock().as_mut() {
                            let _ = dec.open(&path);
                        }
                        state.write().eof_reached = false;
                        if let Some(consumer) = engine.buffer_consumer.lock().as_mut() {
                            consumer.skip(usize::MAX);
                        }
                        continue;
                    }

                    match open_audio_source(&path) {
                        Ok(source) => {
                            let mss =
                                MediaSourceStream::new(source, Default::default());
                            let hint = Hint::new();
                            let metadata_opts = MetadataOptions {
                                limit_metadata_bytes: Limit::Maximum(0), // No cargar metadatos, ya los tenemos en la DB
                                limit_visual_bytes: Limit::Maximum(0),
                            };

                            if let Ok(probed) = symphonia::default::get_probe().format(
                                &hint,
                                mss,
                                &FormatOptions::default(),
                                &metadata_opts,
                            ) {
                                let track = probed.format.default_track().unwrap();
                                track_id = track.id;
                                let sr = track.codec_params.sample_rate.unwrap_or(44100);
                                let dur = track
                                    .codec_params
                                    .n_frames
                                    .map(|f| f as f64 / sr as f64)
                                    .unwrap_or(0.0);
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
                                tracing::info!(
                                    "Audio Engine State Purged (Load): Buffers & DSP Reset."
                                );

                                if let Ok(decoder) = symphonia::default::get_codecs()
                                    .make(&track.codec_params, &DecoderOptions::default())
                                {
                                    current_decoder = Some(decoder);

                                    // Update Channel Map
                                    if let Some(channels) = track.codec_params.channels {
                                        channel_map = AudioEngine::get_channel_map(channels);
                                    } else {
                                        // Fallback for no layout
                                        let count = track
                                            .codec_params
                                            .channels
                                            .map(|c| c.count())
                                            .unwrap_or(2);
                                        channel_map = ChannelMap::default();
                                        if count >= 1 {
                                            channel_map.fl = Some(0);
                                        }
                                        if count >= 2 {
                                            channel_map.fr = Some(1);
                                        }
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
                        }
                        Err(e) => tracing::error!("Error abriendo archivo: {}", e),
                    }
                }
                AudioCommand::Seek(time) => {
                    if let Some(fmt) = current_format.as_mut() {
                        let _ = fmt.seek(
                            symphonia::core::formats::SeekMode::Accurate,
                            symphonia::core::formats::SeekTo::Time {
                                time: symphonia::core::units::Time::from(time),
                                track_id: Some(track_id),
                            },
                        );
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
        } else {
            false
        };

        if should_wait {
            let sleep_ms = if crate::utils::is_low_resource() { 5 } else { 1 };
            std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
            continue;
        }

        // 2. Llenar buffer si hay espacio
        let can_push = {
            let prod = producer_mutex.lock();
            if let Some(p) = prod.as_ref() {
                // Umbral dinámico: Asegurar al menos 40ms de espacio libre para evitar starvation en MP3/7.1ch
                let space_needed = (out_rate as usize * out_channels * 40) / 1000;
                p.capacity().get() - p.occupied_len() > space_needed.max(2048)
            } else {
                false
            }
        };

        // Ruta rápida para decoder custom (DI/mock): leer del trait y pushear directamente
        if can_push && engine.custom_decoder.lock().is_some() {
            let custom_eof = {
                let mut dec_lock = engine.custom_decoder.lock();
                if let Some(dec) = dec_lock.as_mut() {
                    match dec.decode_next() {
                        Ok(Some(packet)) => {
                            let vol = state.read().volume;
                            let gain = vol as f64;
                            for s in packet.data.iter() {
                                output_accumulator_f32.push((*s * gain).clamp(-1.0, 1.0) as f32);
                            }
                            false
                        }
                        Ok(None) => true,
                        Err(_) => true,
                    }
                } else { true }
            };
            if custom_eof {
                state.write().is_playing = false;
                state.write().eof_reached = true;
            }
            // Push custom decoded data
            if !output_accumulator_f32.is_empty() {
                if let Some(producer) = producer_mutex.lock().as_mut() {
                    let _ = producer.push_slice(&output_accumulator_f32);
                }
                output_accumulator_f32.clear();
            }
            if custom_eof {
                let _ = engine.purge_buffers();
                current_format = None;
                *engine.custom_decoder.lock() = None;
            }
            continue;
        }

        if can_push {
            let mut eof = false;
            let dec_opt = current_decoder.as_mut();
            let fmt_opt = current_format.as_mut();

            if let (Some(dec), Some(fmt)) = (dec_opt, fmt_opt) {
                let packet = match fmt.next_packet() {
                    Ok(p) => p,
                    Err(symphonia::core::errors::Error::IoError(e))
                        if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                    {
                        eof = true;
                        symphonia::core::formats::Packet::new_from_slice(0, 0, 0, &[])
                    }
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

                if packet.track_id() != track_id {
                    continue;
                }

                if let Ok(decoded) = dec.decode(&packet) {
                    let spec = *decoded.spec();
                    state.write().current_pos_sec = packet.ts() as f64 / spec.rate as f64;

                    // Reuse or allocate audio_buf only when spec changes or capacity is insufficient
                    let needs_new_buf = audio_buf
                        .as_ref()
                        .map(|b| b.spec() != &spec || b.capacity() < decoded.capacity())
                        .unwrap_or(true);
                    if needs_new_buf {
                        audio_buf = Some(AudioBuffer::<f64>::new(
                            decoded.capacity() as u64,
                            spec,
                        ));
                    }

                    if let Some(ref mut buf) = audio_buf {
                        decoded.convert(buf);
                    }

                    let (out_rate, out_channels) = {
                        let s = state.read();
                        (s.device_sample_rate, s.channels)
                    };

                    // Resampleo adaptativo: calidad superior vs velocidad según recursos disponibles.
                    // En modo normal: SincInterpolation con oversampling masivo (calidad audiófila).
                    // En low-resource: Linear interpolation rápida con menor overhead de CPU.
                    if spec.rate != out_rate {
                        let recreate = if let Some(stored) = resampler_rates {
                            stored != (spec.rate, out_rate, spec.channels.count())
                        } else {
                            true
                        };

                        if recreate {
                            let is_low = crate::utils::is_low_resource();
                            let params = if is_low {
                                // Perfil rápido: Linear + Sinc corto, mínimo overhead de CPU
                                SincInterpolationParameters {
                                    sinc_len: 64,
                                    f_cutoff: 0.95,
                                    interpolation: SincInterpolationType::Linear,
                                    oversampling_factor: 64,
                                    window: WindowFunction::BlackmanHarris2,
                                }
                            } else {
                                // Perfil audiófilo: Cubic + Sinc largo, aliasing eliminado
                                SincInterpolationParameters {
                                    sinc_len: 256,
                                    f_cutoff: 0.99,
                                    interpolation: SincInterpolationType::Cubic,
                                    oversampling_factor: 256,
                                    window: WindowFunction::BlackmanHarris2,
                                }
                            };
                            let chunk_size = if is_low { 256 } else { 1024 };

                            match Async::<f64>::new_sinc(
                                out_rate as f64 / spec.rate as f64,
                                2.0,
                                &params,
                                chunk_size,
                                spec.channels.count(),
                                FixedAsync::Input,
                            ) {
                                Ok(r) => {
                                    resampler = Some(r);
                                    resampler_rates =
                                        Some((spec.rate, out_rate, spec.channels.count()));
                                    resampler_in_buf = (0..spec.channels.count())
                                        .map(|_| {
                                            std::collections::VecDeque::with_capacity(
                                                if is_low { 1024 } else { 4096 }
                                            )
                                        })
                                        .collect();
                                    tracing::info!(
                                        "Resampler initialized: {} -> {} ({} mode, 64-bit)",
                                        spec.rate,
                                        out_rate,
                                        if is_low { "low-resource" } else { "audiophile" }
                                    );
                                }
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
                            if s.downmix_center_enabled {
                                s.downmix_center as f64
                            } else {
                                0.7071
                            },
                            if s.downmix_lfe_enabled {
                                s.downmix_lfe as f64
                            } else {
                                0.6666
                            },
                            if s.downmix_surround_enabled {
                                s.downmix_surround as f64
                            } else {
                                0.7671
                            },
                            if s.downmix_surround_enabled {
                                s.downmix_surround as f64
                            } else {
                                0.8071
                            },
                        )
                    };

                    if let (Some(rs), Some(ref buf)) = (resampler.as_mut(), audio_buf.as_ref())
                    {
                        let planes = buf.planes();
                        let frames = buf.frames();
                        let src_channels = spec.channels.count();

                        for c in 0..src_channels {
                            if c < resampler_in_buf.len() {
                                let plane = &planes.planes()[c][..frames];
                                resampler_in_buf[c].extend(plane.iter().map(|&s| s as f64));
                            }
                        }

                        if resample_input_pool.len() < src_channels {
                            resample_input_pool.resize(src_channels, Vec::new());
                        }
                        if resample_output_pool.len() < src_channels {
                            resample_output_pool.resize(src_channels, Vec::new());
                        }

                        loop {
                            let needed = rs.input_frames_next();
                            if resampler_in_buf.is_empty() || resampler_in_buf[0].len() < needed
                            {
                                break;
                            }

                            let out_frames = rs.output_frames_next();

                            for c in 0..src_channels {
                                resample_input_pool[c].clear();
                                if c < resampler_in_buf.len() {
                                    resample_input_pool[c]
                                        .extend(resampler_in_buf[c].drain(0..needed));
                                } else {
                                    resample_input_pool[c].resize(needed, 0.0);
                                }

                                resample_output_pool[c].clear();
                                resample_output_pool[c].resize(out_frames, 0.0);
                            }

                            let input_adapter = SequentialSliceOfVecs::new(
                                &resample_input_pool,
                                src_channels,
                                needed,
                            )
                            .unwrap();
                            let mut output_adapter = SequentialSliceOfVecs::new_mut(
                                &mut resample_output_pool,
                                src_channels,
                                out_frames,
                            )
                            .unwrap();

                            if let Ok(_) = rs.process_into_buffer(
                                &input_adapter,
                                &mut output_adapter,
                                None,
                            ) {
                                tracing::trace!(
                                    "Resampled: {} -> {} frames",
                                    needed,
                                    out_frames
                                );
                                AudioEngine::mix_channels_planar(
                                    &resample_output_pool,
                                    out_frames,
                                    src_channels,
                                    out_channels as usize,
                                    &channel_map,
                                    downmix_conf,
                                    &mut output_accumulator,
                                );
                            }
                        }
                    } else if let Some(ref buf) = audio_buf {
                        AudioEngine::mix_channels_direct(
                            buf,
                            buf.frames(),
                            spec.channels.count(),
                            out_channels as usize,
                            &channel_map,
                            downmix_conf,
                            &mut output_accumulator,
                        );
                    }

                    if !output_accumulator.is_empty() {
                        output_accumulator_f32.clear();
                        {
                            let s = state.read();
                            let mut gain_db: f64 = 0.0;
                            if s.replay_gain_track_enabled {
                                if let Some(tg) = s.replay_gain_track {
                                    gain_db += tg as f64;
                                }
                            }
                            if s.replay_gain_album_enabled {
                                if let Some(ag) = s.replay_gain_album {
                                    gain_db += ag as f64;
                                }
                            }

                            let gain_linear = 10.0f64.powf(gain_db.min(12.0) / 20.0);
                            let vol = s.volume as f64;

                            // 1. Aplicar ReplayGain, DSP (EQ, etc.), y Volumen a `output_accumulator` en f64 nativo.
                            {
                                let _span = tracing::debug_span!("dsp_process", frames = %(output_accumulator.len() / out_channels.max(1) as usize)).entered();
                                let out_ch = out_channels as usize;
                                if let Some(mut dsp_lock) = engine.dsp.try_write() {
                                    for frame in output_accumulator.chunks_mut(out_ch) {
                                        for s in frame.iter_mut() {
                                            *s *= gain_linear;
                                        }
                                        dsp_lock.process_frame(frame);
                                        for s in frame.iter_mut() {
                                            *s *= vol;
                                        }
                                    }
                                } else {
                                    // Si el DSP está bloqueado por la UI, aplicamos ganancia y volumen sin efectos para evitar tartamudeo (Stutter)
                                    for frame in output_accumulator.chunks_mut(out_ch) {
                                        for s in frame.iter_mut() {
                                            *s *= gain_linear * vol;
                                        }
                                    }
                                    tracing::debug!(
                                        "DSP Lock busy: skipping effects to maintain real-time playback."
                                    );
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
                                    let available =
                                        producer.capacity().get() - producer.occupied_len();

                                    let max_frames = available / out_ch_usize;
                                    let frames_to_push =
                                        (remaining / out_ch_usize).min(max_frames);

                                    if frames_to_push > 0 {
                                        pushed = producer.push_slice(
                                            &output_accumulator_f32
                                                [pos..pos + (frames_to_push * out_ch_usize)],
                                        );
                                    }
                                }

                                if pushed == 0 {
                                    // Si no hay espacio para un frame completo, esperamos (Backoff)
                                    tracing::trace!(
                                        "Push stalled: available < out_channels. Waiting..."
                                    );
                                    let backoff_ms = if crate::utils::is_low_resource() { 10 } else { 2 };
                                    std::thread::sleep(std::time::Duration::from_millis(backoff_ms));
                                    if !state.read().is_playing {
                                        break;
                                    }
                                } else {
                                    pos += pushed;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Si el buffer está suficientemente lleno, dormimos poco para reaccionar rápido
            let idle_ms = if crate::utils::is_low_resource() { 5 } else { 2 };
            std::thread::sleep(std::time::Duration::from_millis(idle_ms));
        }
    }
}
