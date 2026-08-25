//! Background audio decoding thread — handles format reading, resampling, DSP,
//! and pushing decoded audio into the ring buffer shared with the CPAL callback.

use audioadapter_buffers::direct::SequentialSliceOfVecs;
use crossbeam::channel::Receiver;
use ringbuf::traits::{Consumer, Observer, Producer};
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use symphonia::core::audio::{AudioBuffer, Signal};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::{Limit, MetadataOptions};
use symphonia::core::probe::Hint;

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
        Self {
            format: None,
            decoder: None,
            track_id: 0,
        }
    }
}

impl AudioDecoder for SymphoniaDecoder {
    fn open(&mut self, path: &str) -> Result<DecodeStreamInfo, super::AudioError> {
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::Limit;

        let source = open_audio_source(path).map_err(super::AudioError::IoError)?;
        let mss = MediaSourceStream::new(source, Default::default());
        let hint = symphonia::core::probe::Hint::new();
        let metadata_opts = symphonia::core::meta::MetadataOptions {
            limit_metadata_bytes: Limit::Maximum(0),
            limit_visual_bytes: Limit::Maximum(0),
        };

        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &symphonia::core::formats::FormatOptions::default(),
                &metadata_opts,
            )
            .map_err(|e| super::AudioError::ConfigError(e.to_string()))?;

        let track = probed
            .format
            .default_track()
            .ok_or(super::AudioError::ConfigError("No default track".into()))?;
        self.track_id = track.id;
        let sr = track.codec_params.sample_rate.unwrap_or(44100);
        let dur = track
            .codec_params
            .n_frames
            .map(|f| f as f64 / sr as f64)
            .unwrap_or(0.0);
        let ch_count = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);

        let dec = symphonia::default::get_codecs()
            .make(
                &track.codec_params,
                &symphonia::core::codecs::DecoderOptions::default(),
            )
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

        let fmt = self
            .format
            .as_mut()
            .ok_or(super::AudioError::ConfigError("No format loaded".into()))?;
        let dec = self
            .decoder
            .as_mut()
            .ok_or(super::AudioError::ConfigError("No decoder loaded".into()))?;

        loop {
            let packet = match fmt.next_packet() {
                Ok(p) => p,
                Err(symphonia::core::errors::Error::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None);
                }
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
            )
            .ok();
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

use crate::audio::engine::{AudioCommand, AudioEngine, ChannelMap};
use std::time::Instant;

// Volumen y Mezcla (Phase 03): Fade state machine (D-07-D-13)
#[derive(Debug, Clone, Copy, PartialEq)]
enum FadeState {
    Idle,
    FadingIn { coeff: f64, rate_per_sec: f64 }, // rising 0→1, equal-power; coeff advances by rate_per_sec * batch_dt
    FadingOut { coeff: f64, rate_per_sec: f64 }, // falling 1→0, equal-power
}

// Volumen smoothing (UAT round 7): waits for the user to finish adjusting the
// volume (debounce), then ramps from the current applied level to the target.
// Debounce = 400ms of inactivity; ramp = 1500ms linear.
const SMOOTH_DEBOUNCE_SECS: f64 = 0.4;
const SMOOTH_DURATION_SECS: f64 = 1.5;

#[derive(Debug, Clone, Copy)]
struct SmoothRamp {
    from: f64,
    to: f64,
    elapsed: f64,
    duration: f64,
}

/// Decodifica un lote de la pista pre-cargada y lo añade (f64 interleaved,
/// ya resampleado a `out_rate × out_channels`) al buffer de pre-decode.
///
/// Devuelve `true` si queda más audio por decodificar, `false` si se alcanzó
/// el EOF, hubo un error o el buffer alcanzó su capacidad máxima.
#[allow(clippy::too_many_arguments)]
fn preload_decode_batch(
    preload_format: &mut Option<Box<dyn FormatReader>>,
    preload_decoder: &mut Option<Box<dyn Decoder>>,
    preload_track_id: &mut u32,
    preload_audio_buf: &mut Option<AudioBuffer<f64>>,
    preload_resampler: &mut Option<Async<f64>>,
    preload_resampler_rates: &mut Option<(u32, u32, usize)>,
    preload_resampler_in_buf: &mut Vec<std::collections::VecDeque<f64>>,
    preload_input_pool: &mut Vec<Vec<f64>>,
    preload_output_pool: &mut Vec<Vec<f64>>,
    preload_channel_map: &mut ChannelMap,
    predecode_buffer: &mut std::collections::VecDeque<f64>,
    predecode_cap_frames: usize,
    out_rate: u32,
    out_channels: usize,
) -> bool {
    let (Some(fmt), Some(dec)) = (preload_format.as_mut(), preload_decoder.as_mut()) else {
        return false;
    };

    if predecode_buffer.len() >= predecode_cap_frames {
        return true; // Lleno por ahora: se retoma cuando haya espacio.
    }

    let packet = match fmt.next_packet() {
        Ok(p) => p,
        Err(symphonia::core::errors::Error::IoError(e))
            if e.kind() == std::io::ErrorKind::UnexpectedEof =>
        {
            return false; // EOF de la pista pre-cargada.
        }
        Err(e) => {
            tracing::warn!("Pre-carga decode error (skipping packet): {}", e);
            return true;
        }
    };

    if packet.track_id() != *preload_track_id {
        return true;
    }

    let Ok(decoded) = dec.decode(&packet) else {
        return true;
    };
    let spec = *decoded.spec();

    let needs_new_buf = preload_audio_buf
        .as_ref()
        .map(|b| b.spec() != &spec || b.capacity() < decoded.capacity())
        .unwrap_or(true);
    if needs_new_buf {
        *preload_audio_buf = Some(AudioBuffer::<f64>::new(decoded.capacity() as u64, spec));
    }
    let Some(buf) = preload_audio_buf.as_mut() else {
        return false;
    };
    decoded.convert(buf);
    let src_channels = spec.channels.count();

    if spec.rate != out_rate {
        let recreate = preload_resampler_rates
            .map(|stored| stored != (spec.rate, out_rate, src_channels))
            .unwrap_or(true);
        if recreate {
            let is_low = crate::utils::is_low_resource();
            let params = if is_low {
                SincInterpolationParameters {
                    sinc_len: 64,
                    f_cutoff: 0.95,
                    interpolation: SincInterpolationType::Linear,
                    oversampling_factor: 64,
                    window: WindowFunction::BlackmanHarris2,
                }
            } else {
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
                src_channels,
                FixedAsync::Input,
            ) {
                Ok(r) => {
                    *preload_resampler = Some(r);
                    *preload_resampler_rates = Some((spec.rate, out_rate, src_channels));
                    *preload_resampler_in_buf = (0..src_channels)
                        .map(|_| {
                            std::collections::VecDeque::with_capacity(if is_low {
                                1024
                            } else {
                                4096
                            })
                        })
                        .collect();
                }
                Err(e) => {
                    tracing::error!("Pre-carga resampler init failed: {}", e);
                    *preload_resampler = None;
                    return false;
                }
            }
        }
    } else {
        if preload_resampler.is_some() {
            *preload_resampler = None;
            preload_resampler_in_buf.clear();
            *preload_resampler_rates = None;
        }
    }

    let downmix_conf = {
        let side_val = 0.81f64;
        (0.74f64, 0.66f64, side_val, (side_val + 0.10).min(2.0))
    };

    let mut batch_out: Vec<f64> = Vec::new();
    if let Some(rs) = preload_resampler.as_mut() {
        let planes = buf.planes();
        let frames = buf.frames();
        for c in 0..src_channels {
            if c < preload_resampler_in_buf.len() {
                let plane = &planes.planes()[c][..frames];
                preload_resampler_in_buf[c].extend(plane.iter().map(|&s| s as f64));
            }
        }
        if preload_input_pool.len() < src_channels {
            preload_input_pool.resize(src_channels, Vec::new());
        }
        if preload_output_pool.len() < src_channels {
            preload_output_pool.resize(src_channels, Vec::new());
        }
        loop {
            let needed = rs.input_frames_next();
            if preload_resampler_in_buf.is_empty() || preload_resampler_in_buf[0].len() < needed {
                break;
            }
            let out_frames = rs.output_frames_next();
            for c in 0..src_channels {
                preload_input_pool[c].clear();
                if c < preload_resampler_in_buf.len() {
                    preload_input_pool[c].extend(preload_resampler_in_buf[c].drain(0..needed));
                } else {
                    preload_input_pool[c].resize(needed, 0.0);
                }
                preload_output_pool[c].clear();
                preload_output_pool[c].resize(out_frames, 0.0);
            }
            let input_adapter =
                SequentialSliceOfVecs::new(preload_input_pool, src_channels, needed).unwrap();
            let mut output_adapter =
                SequentialSliceOfVecs::new_mut(preload_output_pool, src_channels, out_frames)
                    .unwrap();
            if let Ok(_) = rs.process_into_buffer(&input_adapter, &mut output_adapter, None) {
                AudioEngine::mix_channels_planar(
                    preload_output_pool,
                    out_frames,
                    src_channels,
                    out_channels,
                    preload_channel_map,
                    downmix_conf,
                    &mut batch_out,
                );
            }
        }
    } else {
        AudioEngine::mix_channels_direct(
            buf,
            buf.frames(),
            src_channels,
            out_channels,
            preload_channel_map,
            downmix_conf,
            &mut batch_out,
        );
    }

    let remaining = predecode_cap_frames.saturating_sub(predecode_buffer.len());
    let to_append = batch_out.len().min(remaining);
    predecode_buffer.extend(batch_out.drain(..to_append));
    true
}

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

    // === Volumen y Mezcla state (Phase 03) ===
    let mut fade_state: FadeState = FadeState::Idle;
    // Volume smoothing state (UAT round 7). `applied_vol` is the level actually
    // applied to the audio; `pending_vol` is the user's target (s.volume).
    // The volume does NOT change while the user is adjusting; after 400ms of
    // inactivity a 1500ms ramp takes applied_vol from its current level to the
    // target. Initialized from the engine's real volume (not a hardcoded 0.3).
    let mut applied_vol: f64 = state.read().volume as f64;
    let mut pending_vol: f64 = applied_vol;
    let mut debounce_remaining: f64 = 0.0;
    let mut smooth_ramp: Option<SmoothRamp> = None;

    // Silence detection (D-14-D-20)
    let mut silence_samples: usize = 0;
    let mut in_silence: bool = false;
    let mut track_start_trimmed: bool = false;

    // RG offset smoothing (D-29): ~100 ms EMA ramp to eliminate clicks on UI offset changes.
    // Time constant computes as: alpha = 1 - exp(-dt / 0.100) where dt is batch duration.
    let mut smoothed_rg_offset_album_db: f64 = 0.0;
    let mut smoothed_rg_offset_track_db: f64 = 0.0;
    let mut smoothed_rg_offset_rt_db: f64 = 0.0;

    // === Pre-carga de la siguiente canción (transiciones sin cortes) ===
    // Decodifica la siguiente pista por adelantado a un buffer f64 ya resampleado
    // (out_rate × out_channels) para promoverla sin pausa cuando termina la actual.
    let mut preload_format: Option<Box<dyn FormatReader>> = None;
    let mut preload_decoder: Option<Box<dyn Decoder>> = None;
    let mut preload_track_id: u32 = 0;
    let mut preload_sr: u32 = 0;
    let mut preload_total_duration_sec: f64 = 0.0;
    let mut preload_channel_map = ChannelMap::default();
    let mut preload_path: Option<String> = None;
    let mut preload_title: Option<String> = None;
    let mut preload_artist: Option<String> = None;
    let mut preload_rg: (Option<f32>, Option<f32>) = (None, None);
    let mut predecode_buffer: std::collections::VecDeque<f64> = std::collections::VecDeque::new();
    let mut predecode_cap_frames: usize = 0;
    let mut preloaded_pending: Vec<f64> = Vec::new();
    // Resampler propio de la pre-carga (independiente del de la pista actual).
    let mut preload_resampler: Option<Async<f64>> = None;
    let mut preload_resampler_rates: Option<(u32, u32, usize)> = None;
    let mut preload_resampler_in_buf: Vec<std::collections::VecDeque<f64>> = Vec::new();
    let mut preload_audio_buf: Option<AudioBuffer<f64>> = None;
    let mut preload_input_pool: Vec<Vec<f64>> = Vec::new();
    let mut preload_output_pool: Vec<Vec<f64>> = Vec::new();

    // === Crossfade (Mezcla Cruzada) — estado de la mezcla f64 ===
    let mut crossfade_active: bool = false;
    let mut crossfade_elapsed_sec: f64 = 0.0;
    let mut crossfade_duration_sec: f64 = 0.0;
    // Distingue el crossfade manual (la GUI ya avanzó la playlist al dispararlo)
    // del automático (el decoder debe marcar EOF para que la GUI avance el índice).
    let mut crossfade_is_manual: bool = false;

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
                    // Dedup: si el path ya es la pista actual y no hemos llegado al EOF,
                    // ignoramos la orden — evita re-abrir/re-probar el archivo (y el corte
                    // asociado) cuando la GUI confirma un avance que el decoder ya manejó
                    // vía promoción de pre-carga. También se ignora cuando el path es la
                    // pista pre-cargada: el decoder ya la tiene abierta y la promoverá con
                    // el crossfade (manual) o en el EOF natural — un Load aquí purgaría
                    // el estado y destruiría la mezcla en curso.
                    {
                        let s = state.read();
                        let is_current = s.path == path && !s.eof_reached && s.is_playing;
                        let is_preloaded = preload_path.as_deref() == Some(path.as_str())
                            && preload_format.is_some();
                        if is_current || is_preloaded {
                            tracing::debug!(
                                "Load dedup: '{}' ya en reproducción/pre-cargada.",
                                path
                            );
                            continue;
                        }
                    }

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
                            let mss = MediaSourceStream::new(source, Default::default());
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

                                // Volumen y Mezcla: State resets + fade-in trigger (D-09, D-20)
                                silence_samples = 0;
                                in_silence = false;
                                track_start_trimmed = false;

                                // Reset del crossfade: una pista nueva arranca sin mezcla.
                                crossfade_active = false;
                                crossfade_elapsed_sec = 0.0;
                                crossfade_duration_sec = 0.0;
                                crossfade_is_manual = false;
                                preloaded_pending.clear();

                                let mut s = state.read();
                                // Fade-in on EVERY track start when enabled (D-09).
                                // (Not gated on natural EOF — user expects a smooth rise
                                // whenever a song begins.)
                                if s.fades_enabled && s.fade_in_enabled && s.fade_in_ms > 0.0 {
                                    let fade_ms = s.fade_in_ms as f64;
                                    if fade_ms > 0.0 {
                                        // rate_per_sec: fraction of the fade completed per
                                        // second of *real* audio time. Independent of sample
                                        // rate and batch size (robust timing, UAT round 4).
                                        let rate_per_sec = 1.0 / (fade_ms / 1000.0);
                                        fade_state = FadeState::FadingIn {
                                            coeff: 0.0,
                                            rate_per_sec,
                                        };
                                    }
                                } else {
                                    fade_state = FadeState::Idle;
                                }
                                drop(s);

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
                AudioCommand::PreloadNext {
                    path,
                    title,
                    artist,
                    track_gain,
                    album_gain,
                } => {
                    // Si ya tenemos esa misma canción pre-cargada, no re-abrimos.
                    if preload_path.as_deref() == Some(path.as_str())
                        && !predecode_buffer.is_empty()
                    {
                        tracing::debug!("Preload dedup: '{}' ya pre-cargada.", path);
                        continue;
                    }

                    // Descartar cualquier pre-carga anterior antes de abrir la nueva.
                    preload_format = None;
                    preload_decoder = None;
                    preload_resampler = None;
                    preload_resampler_rates = None;
                    preload_resampler_in_buf.clear();
                    preload_audio_buf = None;
                    preload_input_pool.clear();
                    preload_output_pool.clear();
                    predecode_buffer.clear();

                    match open_audio_source(&path) {
                        Ok(source) => {
                            let mss = MediaSourceStream::new(source, Default::default());
                            let hint = Hint::new();
                            let metadata_opts = MetadataOptions {
                                limit_metadata_bytes: Limit::Maximum(0),
                                limit_visual_bytes: Limit::Maximum(0),
                            };
                            if let Ok(probed) = symphonia::default::get_probe().format(
                                &hint,
                                mss,
                                &FormatOptions::default(),
                                &metadata_opts,
                            ) {
                                let track = probed.format.default_track().unwrap();
                                preload_track_id = track.id;
                                preload_sr = track.codec_params.sample_rate.unwrap_or(44100);
                                preload_total_duration_sec = track
                                    .codec_params
                                    .n_frames
                                    .map(|f| f as f64 / preload_sr as f64)
                                    .unwrap_or(0.0);

                                if let Ok(decoder) = symphonia::default::get_codecs()
                                    .make(&track.codec_params, &DecoderOptions::default())
                                {
                                    if let Some(channels) = track.codec_params.channels {
                                        preload_channel_map =
                                            AudioEngine::get_channel_map(channels);
                                    } else {
                                        let count = track
                                            .codec_params
                                            .channels
                                            .map(|c| c.count())
                                            .unwrap_or(2);
                                        preload_channel_map = ChannelMap::default();
                                        if count >= 1 {
                                            preload_channel_map.fl = Some(0);
                                        }
                                        if count >= 2 {
                                            preload_channel_map.fr = Some(1);
                                        }
                                    }
                                    preload_decoder = Some(decoder);
                                    preload_format = Some(probed.format);
                                    preload_path = Some(path.clone());
                                    preload_title = Some(title);
                                    preload_artist = Some(artist);
                                    preload_rg = (
                                        track_gain.map(|g| g as f32),
                                        album_gain.map(|g| g as f32),
                                    );

                                    // Capacidad del buffer de pre-decode: cubre la mezcla
                                    // más larga posible + margen de seguridad.
                                    let (out_rate, out_channels) = {
                                        let s = state.read();
                                        (s.device_sample_rate, s.channels as usize)
                                    };
                                    let xfade_manual_ms = {
                                        let s = state.read();
                                        s.crossfade_manual_ms.max(0.0)
                                    };
                                    let xfade_auto_ms = {
                                        let s = state.read();
                                        s.crossfade_auto_ms.max(0.0)
                                    };
                                    let cap_ms = xfade_manual_ms.max(xfade_auto_ms) + 2000.0;
                                    predecode_cap_frames =
                                        ((cap_ms as f64 / 1000.0) * out_rate as f64) as usize
                                            * out_channels.max(1);
                                    tracing::info!(
                                        "Pre-carga iniciada para '{}' ({} Hz, {} canales).",
                                        path,
                                        preload_sr,
                                        out_channels
                                    );
                                } else {
                                    preload_format = None;
                                    tracing::error!("Pre-carga falló (codec): {}", path);
                                }
                            }
                        }
                        Err(e) => tracing::error!("Pre-carga falló (open): {} — {}", path, e),
                    }
                }
                AudioCommand::ClearPreload => {
                    preload_format = None;
                    preload_decoder = None;
                    preload_resampler = None;
                    preload_resampler_rates = None;
                    preload_resampler_in_buf.clear();
                    preload_audio_buf = None;
                    preload_input_pool.clear();
                    preload_output_pool.clear();
                    predecode_buffer.clear();
                    preload_path = None;
                    preload_title = None;
                    preload_artist = None;
                    preload_rg = (None, None);
                    preloaded_pending.clear();
                }
                AudioCommand::CrossfadeNext(ms) => {
                    // Activa un crossfade manual con la duración indicada (solo si hay
                    // una pista pre-cargada disponible para mezclar).
                    let ms = if ms.is_finite() {
                        ms.clamp(0.0, 10000.0)
                    } else {
                        0.0
                    };
                    if ms > 0.0 && (preload_format.is_some() || !predecode_buffer.is_empty()) {
                        // Si ya hay una mezcla en curso, el usuario quiere cambiar
                        // AHORA: completar la mezcla activa de inmediato (el cambio
                        // de pista no debe quedar bloqueado hasta que termine).
                        if crossfade_active {
                            crossfade_elapsed_sec = crossfade_duration_sec;
                        } else {
                            crossfade_duration_sec = ms / 1000.0;
                            crossfade_elapsed_sec = 0.0;
                            crossfade_active = true;
                            crossfade_is_manual = true;
                            tracing::info!("Crossfade manual iniciado: {} ms", ms);
                        }
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

                        // Volumen y Mezcla: Reset on seek (D-20)
                        fade_state = FadeState::Idle;
                        silence_samples = 0;
                        in_silence = false;
                        track_start_trimmed = false;

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

                    // Volumen y Mezcla: Reset on stop (D-20)
                    fade_state = FadeState::Idle;
                    silence_samples = 0;
                    in_silence = false;
                    track_start_trimmed = false;
                }
            }
        }

        // Drenar audio pre-cargado (promovido tras EOF/crossfade): se procesa por la
        // cadena normal (gain→DSP→volumen→ringbuf) en esta iteración, sin decodificar
        // la nueva pista hasta que el buffer pre-cargado se haya agotado.
        let process_preloaded = !preloaded_pending.is_empty();
        if process_preloaded {
            output_accumulator.extend(preloaded_pending.drain(..));
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
            // Tiempo de espera (el ringbuf está al target de latencia): aprovechamos
            // este momento —que es donde el decoder pasa la mayor parte del tiempo—
            // para decodificar la pista pre-cargada en segundo plano. Sin esto, el
            // buffer de pre-decode nunca se llena y ni el crossfade ni la promoción
            // sin cortes pueden funcionar.
            if preload_format.is_some() {
                let _ = preload_decode_batch(
                    &mut preload_format,
                    &mut preload_decoder,
                    &mut preload_track_id,
                    &mut preload_audio_buf,
                    &mut preload_resampler,
                    &mut preload_resampler_rates,
                    &mut preload_resampler_in_buf,
                    &mut preload_input_pool,
                    &mut preload_output_pool,
                    &mut preload_channel_map,
                    &mut predecode_buffer,
                    predecode_cap_frames,
                    out_rate,
                    out_channels,
                );
            }
            let sleep_ms = if crate::utils::is_low_resource() {
                5
            } else {
                1
            };
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
        if can_push && !process_preloaded && engine.custom_decoder.lock().is_some() {
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
                } else {
                    true
                }
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
                // No purge_buffers aquí — eso recrea el stream. Solo limpiar estado del decoder.
                current_format = None;
                *engine.custom_decoder.lock() = None;
            }
            continue;
        }

        if can_push && !process_preloaded {
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
                    // Si hay una pista pre-cargada lista, promovemos sin purgar:
                    // el audio ya está decodificado/resampleado en predecode_buffer,
                    // por lo que la transición es sin cortes (la GUI solo avanza el
                    // índice al ver eof_reached y su Load redundante es dedup).
                    let has_preload = preload_format.is_some()
                        || !predecode_buffer.is_empty()
                        || !preloaded_pending.is_empty();
                    if has_preload {
                        let preload_ready = preload_format
                            .as_ref()
                            .and_then(|_| preload_decoder.as_ref())
                            .is_some();

                        if preload_ready || !predecode_buffer.is_empty() {
                            // Promoción: la pista pre-cargada pasa a ser la actual.
                            current_format = preload_format.take();
                            current_decoder = preload_decoder.take();
                            track_id = preload_track_id;
                            channel_map = preload_channel_map;

                            // Si el buffer de pre-decode aún no se ha drenado, pasamos
                            // su contenido al buffer pendiente para procesarlo en la
                            // siguiente iteración por la cadena gain→DSP→volumen.
                            if !predecode_buffer.is_empty() {
                                preloaded_pending.extend(predecode_buffer.drain(..));
                            }

                            {
                                let mut s = state.write();
                                if let Some(t) = preload_title.take() {
                                    s.title = t;
                                }
                                if let Some(a) = preload_artist.take() {
                                    s.artist = a;
                                }
                                if let Some(p) = preload_path.take() {
                                    s.path = p;
                                }
                                s.replay_gain_track = preload_rg.0;
                                s.replay_gain_album = preload_rg.1;
                                s.total_duration_sec = preload_total_duration_sec;
                                s.sample_rate = preload_sr;
                                s.current_pos_sec = 0.0;
                                s.eof_reached = true; // La GUI avanza el índice/playlist
                                // is_playing se mantiene true: la nueva pista continúa.
                            }

                            // Reiniciar el resampler de la cadena actual para la nueva
                            // pista (se recrea reactivamente con su spec al decodificar).
                            resampler = None;
                            resampler_rates = None;
                            resampler_in_buf.clear();
                            resample_input_pool.clear();
                            resample_output_pool.clear();
                            audio_buf = None;
                            {
                                if let Some(mut dsp_lock) = engine.dsp.try_write() {
                                    dsp_lock.reset_state();
                                }
                            }

                            // Estado de fades para la NUEVA pista: se limpia el fade-out
                            // de la anterior (si se heredara, la pista quedaría muda) y
                            // se arranca el fade-in de la columna izquierda si está
                            // activado (el Load es dedup, así que no lo dispararía).
                            fade_state = FadeState::Idle;
                            silence_samples = 0;
                            in_silence = false;
                            track_start_trimmed = false;
                            {
                                let s = state.read();
                                if s.fades_enabled && s.fade_in_enabled && s.fade_in_ms > 0.0 {
                                    let rate_per_sec = 1.0 / ((s.fade_in_ms as f64) / 1000.0);
                                    fade_state = FadeState::FadingIn {
                                        coeff: 0.0,
                                        rate_per_sec,
                                    };
                                }
                            }

                            // Limpiar el estado de pre-carga ya consumido.
                            preload_resampler = None;
                            preload_resampler_rates = None;
                            preload_resampler_in_buf.clear();
                            preload_audio_buf = None;
                            preload_input_pool.clear();
                            preload_output_pool.clear();
                            preload_channel_map = ChannelMap::default();
                            preload_rg = (None, None);

                            tracing::info!(
                                "Transición sin cortes: pista pre-cargada promovida ('{}').",
                                state.read().title
                            );
                            continue;
                        }
                    }

                    // Sin pre-carga: comportamiento original (purgar al llegar al EOF).
                    state.write().is_playing = false;
                    state.write().eof_reached = true;

                    // Autoclean on EOF
                    let s = state.read();
                    if !s.is_playing {
                        drop(s);
                        // --- PURGA EN EOF (solo estado, no stream) ---
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
                        audio_buf = Some(AudioBuffer::<f64>::new(decoded.capacity() as u64, spec));
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
                                    resampler_in_buf =
                                        (0..spec.channels.count())
                                            .map(|_| {
                                                std::collections::VecDeque::with_capacity(
                                                    if is_low { 1024 } else { 4096 },
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
                        let side_val = if s.downmix_surround_enabled {
                            s.downmix_surround as f64
                        } else {
                            0.81
                        };
                        (
                            if s.downmix_center_enabled {
                                s.downmix_center as f64
                            } else {
                                0.74
                            },
                            if s.downmix_lfe_enabled {
                                s.downmix_lfe as f64
                            } else {
                                0.66
                            },
                            side_val,
                            (side_val + 0.10).min(2.0),
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

                        if resample_input_pool.len() < src_channels {
                            resample_input_pool.resize(src_channels, Vec::new());
                        }
                        if resample_output_pool.len() < src_channels {
                            resample_output_pool.resize(src_channels, Vec::new());
                        }

                        loop {
                            let needed = rs.input_frames_next();
                            if resampler_in_buf.is_empty() || resampler_in_buf[0].len() < needed {
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

                            if let Ok(_) =
                                rs.process_into_buffer(&input_adapter, &mut output_adapter, None)
                            {
                                tracing::trace!("Resampled: {} -> {} frames", needed, out_frames);
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
                }
            }
        } else {
            // Tiempo idle (buffer del stream lleno o sin pista activa): aprovechamos
            // para decodificar la pista pre-cargada en segundo plano.
            if preload_format.is_some() {
                // Decodificamos un lote por iteración; el loop controla la frecuencia.
                let _ = preload_decode_batch(
                    &mut preload_format,
                    &mut preload_decoder,
                    &mut preload_track_id,
                    &mut preload_audio_buf,
                    &mut preload_resampler,
                    &mut preload_resampler_rates,
                    &mut preload_resampler_in_buf,
                    &mut preload_input_pool,
                    &mut preload_output_pool,
                    &mut preload_channel_map,
                    &mut predecode_buffer,
                    predecode_cap_frames,
                    out_rate,
                    out_channels as usize,
                );
            }
            // Si el buffer está suficientemente lleno, dormimos poco para reaccionar rápido
            let idle_ms = if crate::utils::is_low_resource() {
                5
            } else {
                2
            };
            std::thread::sleep(std::time::Duration::from_millis(idle_ms));
        }
        if !output_accumulator.is_empty() {
            output_accumulator_f32.clear();

            // Crossfade automático (D-16): cuando la canción actual se acerca a su
            // final y hay una pista pre-cargada disponible, se inicia la mezcla.
            if !crossfade_active {
                let s = state.read();
                if s.crossfade_enabled
                    && s.crossfade_auto_enabled
                    && s.crossfade_auto_ms > 0.0
                    && s.total_duration_sec > 0.0
                    && !predecode_buffer.is_empty()
                {
                    let remaining = s.total_duration_sec - s.current_pos_sec;
                    if remaining <= (s.crossfade_auto_ms as f64 / 1000.0) {
                        crossfade_duration_sec = s.crossfade_auto_ms as f64 / 1000.0;
                        crossfade_elapsed_sec = 0.0;
                        crossfade_active = true;
                        crossfade_is_manual = false;
                        tracing::info!("Crossfade automático iniciado: {} ms", s.crossfade_auto_ms);
                    }
                }
            }

            // Mezcla f64: superpone la canción actual con el inicio de la siguiente
            // pre-cargada SIN desvanecimiento de volumen — ambas se mezclan al nivel
            // del reproductor (los fades de la columna izquierda son independientes).
            // El factor 0.5 evita el recorte al sumar dos señales a pleno nivel.
            if crossfade_active {
                let out_ch = out_channels as usize;
                if out_ch > 0 && !predecode_buffer.is_empty() {
                    let batch_dt_cf =
                        output_accumulator.len() as f64 / out_ch as f64 / out_rate as f64;
                    let t = (crossfade_elapsed_sec / crossfade_duration_sec.max(1e-9)).min(1.0);
                    for frame in output_accumulator.chunks_mut(out_ch) {
                        for s in frame.iter_mut() {
                            let Some(next) = predecode_buffer.pop_front() else {
                                break;
                            };
                            let mixed = (*s + next) * 0.5;
                            *s = if mixed.is_finite() { mixed } else { 0.0 };
                        }
                    }
                    crossfade_elapsed_sec += batch_dt_cf;

                    if t >= 1.0 {
                        // Mezcla completada: el resto de la pre-carga pasa a ser la
                        // pista actual (promoción) y se procesa por la cadena normal.
                        crossfade_active = false;
                        if !predecode_buffer.is_empty() {
                            preloaded_pending.extend(predecode_buffer.drain(..));
                        }
                        if let (Some(fmt), Some(dec)) =
                            (preload_format.take(), preload_decoder.take())
                        {
                            current_format = Some(fmt);
                            current_decoder = Some(dec);
                            track_id = preload_track_id;
                            channel_map = preload_channel_map;
                            {
                                let mut s = state.write();
                                if let Some(t) = preload_title.take() {
                                    s.title = t;
                                }
                                if let Some(a) = preload_artist.take() {
                                    s.artist = a;
                                }
                                if let Some(p) = preload_path.take() {
                                    s.path = p;
                                }
                                s.replay_gain_track = preload_rg.0;
                                s.replay_gain_album = preload_rg.1;
                                s.total_duration_sec = preload_total_duration_sec;
                                s.sample_rate = preload_sr;
                                s.current_pos_sec = 0.0;
                                // En el crossfade MANUAL la GUI ya avanzó la playlist
                                // al dispararlo — no marcar EOF (evita el doble avance).
                                // En el automático, EOF permite que la GUI avance el índice.
                                s.eof_reached = !crossfade_is_manual;
                            }
                            resampler = None;
                            resampler_rates = None;
                            resampler_in_buf.clear();
                            resample_input_pool.clear();
                            resample_output_pool.clear();
                            audio_buf = None;
                            {
                                if let Some(mut dsp_lock) = engine.dsp.try_write() {
                                    dsp_lock.reset_state();
                                }
                            }

                            // Estado de fades para la NUEVA pista: limpiar el fade-out
                            // de la anterior (si se heredara, quedaría muda) y arrancar
                            // el fade-in de la columna izquierda si está activado.
                            fade_state = FadeState::Idle;
                            silence_samples = 0;
                            in_silence = false;
                            track_start_trimmed = false;
                            {
                                let s = state.read();
                                if s.fades_enabled && s.fade_in_enabled && s.fade_in_ms > 0.0 {
                                    let rate_per_sec = 1.0 / ((s.fade_in_ms as f64) / 1000.0);
                                    fade_state = FadeState::FadingIn {
                                        coeff: 0.0,
                                        rate_per_sec,
                                    };
                                }
                            }

                            preload_resampler = None;
                            preload_resampler_rates = None;
                            preload_resampler_in_buf.clear();
                            preload_audio_buf = None;
                            preload_input_pool.clear();
                            preload_output_pool.clear();
                            preload_channel_map = ChannelMap::default();
                            preload_rg = (None, None);
                        }
                    }
                }
            }

            // Single loudness gain point (D-01): sum all sources in dB,
            // convert to linear once, apply before DspChain.
            let gain_linear: f64;
            let vol: f64;
            let fades_enabled: bool;
            let smooth_volume_enabled: bool;
            {
                let s = state.read();
                let mut gain_db: f64 = 0.0;

                // 1. ReplayGain base from tags (D-27: album + track sum)
                if s.rg_master_enabled {
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

                    // 2. RG offsets per source (D-26, D-29):
                    //    Use EMA-smoothed values to avoid clicks.
                    gain_db += smoothed_rg_offset_album_db;
                    gain_db += smoothed_rg_offset_track_db;

                    // 3. RT Analysis fallback (D-28):
                    //    Only when no tags AND analyze enabled
                    let has_tags = s.replay_gain_track.is_some() || s.replay_gain_album.is_some();
                    if !has_tags && s.rg_analyze_rt_enabled {
                        gain_db += smoothed_rg_offset_rt_db;
                    }
                }

                // 4. Replay gain fijo (applies ALWAYS when enabled, unlike the
                //    RT-analysis fallback which only acts when no tags exist).
                if s.rg_fixed_enabled {
                    gain_db += s.rg_fixed_db as f64;
                }

                // 5. Clamp to safety ceiling (+12 dB existing, D-26)
                gain_linear = 10.0f64.powf(gain_db.min(12.0) / 20.0);
                vol = s.volume as f64;
                fades_enabled = s.fades_enabled;
                smooth_volume_enabled = s.smooth_volume_enabled;
            } // Release AudioState lock before DSP processing

            // D-29: ~100 ms EMA anti-click ramp for RG offsets.
            // alpha = 1 - exp(-dt / tau) where tau = 0.100s
            let batch_dt = output_accumulator.len() as f64 / out_channels as f64 / out_rate as f64;
            if batch_dt > 0.0 {
                let alpha = 1.0 - (-batch_dt / 0.100).exp();
                // Re-read target values from AudioState (brief lock).
                {
                    let s = state.read();
                    let diff_album = s.rg_offset_album_db as f64 - smoothed_rg_offset_album_db;
                    let diff_track = s.rg_offset_track_db as f64 - smoothed_rg_offset_track_db;
                    let diff_rt = s.rg_offset_rt_db as f64 - smoothed_rg_offset_rt_db;

                    smoothed_rg_offset_album_db += diff_album * alpha;
                    smoothed_rg_offset_track_db += diff_track * alpha;
                    smoothed_rg_offset_rt_db += diff_rt * alpha;

                    // NaN guard: reset to 0.0 on corrupted state.
                    if !smoothed_rg_offset_album_db.is_finite() {
                        smoothed_rg_offset_album_db = 0.0;
                    }
                    if !smoothed_rg_offset_track_db.is_finite() {
                        smoothed_rg_offset_track_db = 0.0;
                    }
                    if !smoothed_rg_offset_rt_db.is_finite() {
                        smoothed_rg_offset_rt_db = 0.0;
                    }
                }
            }

            // ============================================================
            // Volumen y Mezcla: Silence detection + Edge trimming (D-14-D-20)
            // Measure PRE-fade (D-19) on output_accumulator before DSP/fades.
            // ============================================================
            let current_pos_sec: f64;
            {
                let s = state.read();
                current_pos_sec = s.current_pos_sec;

                // Edge trimming — start (D-18): fixed -50dB threshold, no minimum duration.
                if !track_start_trimmed && s.silence_enabled && s.silence_edge_trim_enabled {
                    let edge_threshold = 10.0f64.powf(-50.0 / 20.0);
                    // f64::max ignora NaN (nunca paniquea ante frames contaminados).
                    let peak = output_accumulator
                        .iter()
                        .map(|sample| sample.abs())
                        .fold(0.0_f64, f64::max);
                    if peak < edge_threshold {
                        // Leading silence: discard this batch entirely.
                        output_accumulator.clear();
                        output_accumulator_f32.clear();
                        continue;
                    }
                    // First non-silent frame found.
                    track_start_trimmed = true;
                }

                // Silence detection — main body (D-14-D-17)
                if s.silence_enabled {
                    let silence_enter = 10.0f64.powf(s.silence_threshold_db as f64 / 20.0);
                    let silence_exit = 10.0f64.powf((s.silence_threshold_db as f64 + 3.0) / 20.0);
                    let threshold = if in_silence {
                        silence_exit
                    } else {
                        silence_enter
                    };

                    let peak = output_accumulator
                        .iter()
                        .map(|sample| sample.abs())
                        .fold(0.0_f64, f64::max);

                    if peak < threshold {
                        // Silent frame (D-14): accumulate and potentially drop.
                        let frame_samples = output_accumulator.len() / out_channels as usize;
                        silence_samples += frame_samples;
                        let silence_duration_ms =
                            (silence_samples as f64 / out_rate as f64) * 1000.0;
                        in_silence = true;

                        if silence_duration_ms >= s.silence_duration_ms as f64 {
                            // Drop this batch entirely (D-14: no seeks needed).
                            output_accumulator.clear();
                            output_accumulator_f32.clear();
                            continue;
                        }
                    } else {
                        // Non-silent frame: reset counter.
                        silence_samples = 0;
                        in_silence = false;
                        if !track_start_trimmed {
                            track_start_trimmed = true;
                        }
                    }
                }

                // Fade-out trigger (D-10): at total_duration - fade_out_ms.
                // NOTE: must use total_duration_sec (NOT effective_end_sec, which
                // tracks live position) — otherwise the fade-out fires on every
                // batch and the volume oscillates during the whole song.
                // El fade-out de la columna izquierda corre SIEMPRE (independiente del
                // crossfade): la mezcla cruzada solo mezcla, no gestiona el volumen.
                if s.fades_enabled && s.fade_out_enabled && s.fade_out_ms > 0.0 {
                    let end_pos = s.total_duration_sec;
                    let fade_start = end_pos - s.fade_out_ms as f64 / 1000.0;
                    if current_pos_sec >= fade_start
                        && !matches!(fade_state, FadeState::FadingOut { .. })
                    {
                        let fade_ms = s.fade_out_ms as f64;
                        if fade_ms > 0.0 {
                            // rate_per_sec: fraction per second of real audio time.
                            let rate_per_sec = 1.0 / (fade_ms / 1000.0);
                            fade_state = FadeState::FadingOut {
                                coeff: 1.0,
                                rate_per_sec,
                            };
                        }
                    }
                }
            } // Release state lock

            // Volume smoothing (D-08, UAT round 7): debounce + ramp.
            // While the user adjusts the volume, applied_vol stays frozen.
            // After 400ms of inactivity, a 1500ms linear ramp moves
            // applied_vol from its current level to the new target.
            // Gated by the group master (fades_enabled) AND the individual
            // smooth_volume_enabled flag — disabling the group disables
            // smoothing too (UAT round 13).
            let user_target = vol;
            if fades_enabled && smooth_volume_enabled {
                if (user_target - pending_vol).abs() > 1e-10 {
                    // User changed the target → restart debounce, cancel any
                    // in-flight ramp (applied_vol keeps its current level).
                    pending_vol = user_target;
                    debounce_remaining = SMOOTH_DEBOUNCE_SECS;
                    smooth_ramp = None;
                }
                if let Some(ref mut ramp) = smooth_ramp {
                    ramp.elapsed += batch_dt;
                    let t = (ramp.elapsed / ramp.duration).min(1.0);
                    applied_vol = ramp.from + (ramp.to - ramp.from) * t;
                    if t >= 1.0 {
                        applied_vol = ramp.to;
                        smooth_ramp = None;
                    }
                } else if debounce_remaining > 0.0 {
                    debounce_remaining -= batch_dt;
                    if debounce_remaining <= 0.0 {
                        debounce_remaining = 0.0;
                        if (applied_vol - pending_vol).abs() > 1e-10 {
                            smooth_ramp = Some(SmoothRamp {
                                from: applied_vol,
                                to: pending_vol,
                                elapsed: 0.0,
                                duration: SMOOTH_DURATION_SECS,
                            });
                        } else {
                            applied_vol = pending_vol;
                        }
                    }
                } else {
                    applied_vol = pending_vol;
                }
            } else {
                // Smoothing off: apply the user volume immediately.
                applied_vol = user_target;
                pending_vol = user_target;
                debounce_remaining = 0.0;
                smooth_ramp = None;
            }

            // Fade envelope coefficient (D-12, D-13): equal-power for fades,
            // linear for volume smoothing.
            // coeff advances by rate_per_sec * batch_dt — i.e. by REAL audio
            // time. This makes the configured ms exact regardless of sample
            // rate or batch size (UAT round 4).
            let batch_advance = batch_dt;
            let fade_coeff = match fade_state {
                FadeState::Idle => 1.0,
                FadeState::FadingIn {
                    mut coeff,
                    rate_per_sec,
                } => {
                    if coeff >= 1.0 {
                        fade_state = FadeState::Idle;
                        1.0
                    } else {
                        let current = coeff;
                        // Equal-power fade-in (D-12): sin²(π/2 * t)
                        let ep_coeff = (std::f64::consts::PI / 2.0 * current).sin().powi(2);
                        coeff += rate_per_sec * batch_advance;
                        if coeff > 1.0 {
                            coeff = 1.0;
                        }
                        fade_state = FadeState::FadingIn {
                            coeff,
                            rate_per_sec,
                        };
                        ep_coeff
                    }
                }
                FadeState::FadingOut {
                    mut coeff,
                    rate_per_sec,
                } => {
                    if coeff <= 0.0 {
                        // Fade-out complete: STAY at 0 (do not return to Idle,
                        // which would snap volume back up). The track is ending.
                        fade_state = FadeState::FadingOut {
                            coeff: 0.0,
                            rate_per_sec,
                        };
                        0.0
                    } else {
                        let current = coeff;
                        let ep_coeff = (std::f64::consts::PI / 2.0 * current).sin().powi(2);
                        coeff -= rate_per_sec * batch_advance;
                        if coeff < 0.0 {
                            coeff = 0.0;
                        }
                        fade_state = FadeState::FadingOut {
                            coeff,
                            rate_per_sec,
                        };
                        ep_coeff
                    }
                }
            };

            // 1. Aplicar gain → DSP → fades×volume (D-03) al accumulator.
            {
                let _span = tracing::debug_span!("dsp_process", frames = %(output_accumulator.len() / out_channels.max(1) as usize)).entered();
                let out_ch = out_channels as usize;
                // combined = applied_vol (smoothed or immediate user volume)
                // × fade_coeff (equal-power musical fade, 1.0 when Idle).
                let combined = applied_vol * fade_coeff;

                // Fix B1 (D-44): Spin-wait for DSP lock up to 5ms before falling back.
                // Prevents DSP dropout during brief GUI lock contention.
                let dsp_lock = {
                    let deadline = Instant::now() + std::time::Duration::from_millis(5);
                    loop {
                        if let Some(lock) = engine.dsp.try_write() {
                            break Some(lock);
                        }
                        if Instant::now() >= deadline {
                            break None;
                        }
                        std::hint::spin_loop();
                    }
                };

                if let Some(mut dsp_lock) = dsp_lock {
                    for frame in output_accumulator.chunks_mut(out_ch) {
                        for s in frame.iter_mut() {
                            *s *= gain_linear;
                        }
                        dsp_lock.process_frame(frame);
                        for s in frame.iter_mut() {
                            *s *= combined;
                        }
                    }
                } else {
                    // Fallback (rare: GUI held lock >5ms):
                    // Apply gain×volume without DSP to maintain playback.
                    tracing::debug!(
                        "DSP lock timeout: applying gain×volume without effects to maintain playback."
                    );
                    let bypass_gain = gain_linear * combined;
                    for frame in output_accumulator.chunks_mut(out_ch) {
                        for s in frame.iter_mut() {
                            *s *= bypass_gain;
                        }
                    }
                }
            }

            // Conversión limpia de f64 a f32 (Zero-Allocation pool)
            for &sample in output_accumulator.iter() {
                output_accumulator_f32.push(sample.clamp(-1.0, 1.0) as f32);
            }
            if !output_accumulator_f32.is_empty() {
                // Push al RingBuffer (lock breve por iteración)
                // --- PUSHING ATÓMICO (FRAME ALIGNMENT) ---
                // Aseguramos que solo se envíen múltiplos exactos de `out_channels`.
                let mut pos = 0;
                let mut stalled = 0;
                while pos < output_accumulator_f32.len() {
                    let mut pushed = 0;
                    if let Some(producer) = producer_mutex.lock().as_mut() {
                        let out_ch_usize = out_channels as usize;
                        let remaining = output_accumulator_f32.len() - pos;
                        let available = producer.capacity().get() - producer.occupied_len();

                        let max_frames = available / out_ch_usize;
                        let frames_to_push = (remaining / out_ch_usize).min(max_frames);

                        if frames_to_push > 0 {
                            pushed = producer.push_slice(
                                &output_accumulator_f32[pos..pos + (frames_to_push * out_ch_usize)],
                            );
                        }
                    }

                    if pushed == 0 {
                        // Sin espacio para un frame completo: esperar (backoff).
                        // Si el stream no drena (p. ej. falló su construcción), se
                        // abandona el push tras un límite para no bloquear el decoder.
                        stalled += 1;
                        if stalled >= 50 {
                            tracing::warn!(
                                "Push stalled persistentemente: se descartan {} frames sobrantes.",
                                output_accumulator_f32.len() - pos
                            );
                            break;
                        }
                        let backoff_ms = if crate::utils::is_low_resource() {
                            10
                        } else {
                            2
                        };
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
