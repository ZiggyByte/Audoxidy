//! Background audio decoding thread — handles format reading, resampling, DSP,
//! and pushing decoded audio into the ring buffer shared with the CPAL callback.

use crate::utils::observability::METRICS_TARGET;
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use crossbeam::channel::Receiver;
use ringbuf::traits::{Observer, Producer};
use rubato::{
    Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
    WindowFunction,
};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::Ordering;
use symphonia::core::codecs::audio::{AudioDecoder as SymphoniaAudioDecoder, AudioDecoderOptions};
use symphonia::core::common::Limit;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;

/// Umbral para usar memoria mapeada en lugar de File normal (> 10 MB)
const MEMMAP_THRESHOLD: u64 = 10 * 1024 * 1024;

/// Preload request queued until the decoder loop opens the file:
/// `(path, title, artist, album, cover path, track gain, album gain)`.
type PendingPreload = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<f64>,
    Option<f64>,
);

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
/// Soporta MP3, FLAC, WAV, OGG, M4A, AAC, ALAC, AIFF, CAF y MKV.
pub struct SymphoniaDecoder {
    format: Option<Box<dyn FormatReader>>,
    decoder: Option<Box<dyn SymphoniaAudioDecoder>>,
    track_id: u32,
    /// Pool planar reutilizado por las copias de decodificación: evita
    /// asignar un buffer por paquete en `decode_next`.
    decode_plane_pool: Vec<Vec<f64>>,
}

impl SymphoniaDecoder {
    /// Crea un nuevo decodificador Symphonia sin estado interno.
    // A decoder is a state machine whose only legal path is construction followed by
    // `open`; an all-`None` value carrying a sentinel track id would advertise a
    // constructor for an instance that is useless until it is opened.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            format: None,
            decoder: None,
            track_id: 0,
            decode_plane_pool: Vec::new(),
        }
    }
}

impl AudioDecoder for SymphoniaDecoder {
    fn open(&mut self, path: &str) -> Result<DecodeStreamInfo, super::AudioError> {
        let source = open_audio_source(path).map_err(super::AudioError::IoError)?;
        let mss = MediaSourceStream::new(source, Default::default());
        let hint = Hint::new();
        let metadata_opts = MetadataOptions::default()
            .limit_tag_bytes(Limit::Maximum(0))
            .limit_visual_bytes(Limit::Maximum(0));

        let format = symphonia::default::get_probe()
            .probe(&hint, mss, FormatOptions::default(), metadata_opts)
            .map_err(|e| super::AudioError::UnsupportedFormat(e.to_string()))?;

        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| super::AudioError::UnsupportedFormat("sin pista de audio".into()))?;
        self.track_id = track.id;
        let audio_params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or_else(|| {
                super::AudioError::UnsupportedFormat("parámetros de audio ausentes".into())
            })?;
        let sample_rate = audio_params.sample_rate.unwrap_or(44100);
        let duration = derive_duration_sec(track, sample_rate).unwrap_or(0.0);
        let ch_count = audio_params
            .channels
            .as_ref()
            .map(|c| c.count())
            .unwrap_or(2);

        let dec = symphonia::default::get_codecs()
            .make_audio_decoder(audio_params, &AudioDecoderOptions::default())
            .map_err(|e| super::AudioError::UnsupportedFormat(e.to_string()))?;

        self.format = Some(format);
        self.decoder = Some(dec);

        Ok(DecodeStreamInfo {
            sample_rate,
            channels: ch_count as u16,
            total_duration_sec: duration,
            channel_count: ch_count,
        })
    }

    fn decode_next(&mut self) -> Result<Option<DecodedPacket>, super::AudioError> {
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
                Ok(Some(p)) => p,
                Ok(None) => return Ok(None),
                Err(e) => {
                    tracing::warn!("Symphonia decode error: {}", e);
                    continue;
                }
            };

            if packet.track_id != self.track_id {
                continue;
            }

            if let Ok(decoded) = dec.decode(&packet) {
                let spec = decoded.spec().clone();
                let frames = decoded.frames();
                let channels = spec.channels().count();
                let sample_rate = spec.rate();

                // Copiar a un pool planar f64 reutilizado: sin asignación por paquete.
                decoded.copy_to_vecs_planar::<f64>(&mut self.decode_plane_pool);

                let mut data = Vec::with_capacity(frames * channels);
                for i in 0..frames {
                    for channel in 0..channels {
                        data.push(self.decode_plane_pool[channel][i]);
                    }
                }

                return Ok(Some(DecodedPacket {
                    data,
                    frames,
                    channels,
                    sample_rate,
                }));
            }
        }
    }

    fn seek(&mut self, time_secs: f64) -> Result<(), super::AudioError> {
        let time = symphonia::core::units::Time::try_from_secs_f64(time_secs)
            .ok_or_else(|| super::AudioError::ConfigError("tiempo de búsqueda inválido".into()))?;
        if let Some(fmt) = self.format.as_mut() {
            fmt.seek(
                symphonia::core::formats::SeekMode::Accurate,
                symphonia::core::formats::SeekTo::Time {
                    time,
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

// ⚠️ ESTABLE — NO MODIFICAR sin re-medir RAM y CPU.
// Este bloque corrige el crecimiento de RSS durante la reproducción (un mapeo
// secuencial acumulaba en RSS todas las páginas tocadas hasta `munmap`) y agrupa
// la liberación para no penalizar la CPU. Cambiar el umbral, el tamaño de bloque
// o la lógica de liberación puede reintroducir el crecimiento de memoria o un
// coste de CPU alto. Ver `.planning/codebase/MEMORY-AND-CPU.md`.
/// Bloque de lectura tras el cual se liberan de RSS las páginas ya consumidas.
///
/// Agrupar el `MADV_DONTNEED` evita pagar un syscall por cada lectura y mantiene
/// el RSS acotado a ~este tamaño más el búfer de trabajo del decodificador.
const MMAP_ADVISE_CHUNK: usize = 8 * 1024 * 1024;

/// Wrapper que presenta un Mmap como un MediaSource para Symphonia.
/// Los archivos grandes (>10MB) se mapean a memoria para evitar copias.
///
/// ⚠️ ESTABLE — NO MODIFICAR: la liberación de páginas en `read` mantiene el RSS
/// acotado; quitarla o cambiarla reintroduce el crecimiento de memoria.
struct MmapSource {
    mmap: memmap2::Mmap,
    pos: usize,
    /// Posición hasta la que ya se aconsejó al kernel liberar páginas.
    advised_until: usize,
}

// SAFETY: `MmapSource` solo expone lecturas inmutables sobre el mapeo y el
// `File` que lo respalda permanece dentro del propio `Mmap`, de modo que la
// memoria mapeada sigue siendo válida mientras la instancia viva. Compartirla
// entre hilos es seguro porque `pos` es el único estado mutable y cada
// `MmapSource` se consume desde un único hilo decodificador.
unsafe impl Send for MmapSource {}
unsafe impl Sync for MmapSource {}

impl Read for MmapSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self.mmap.len().saturating_sub(self.pos);
        let to_read = buf.len().min(remaining);
        buf[..to_read].copy_from_slice(&self.mmap[self.pos..self.pos + to_read]);
        self.pos += to_read;
        // ⚠️ ESTABLE — NO MODIFICAR: liberar en bloques grandes las páginas ya
        // consumidas es lo que mantiene el RSS plano durante toda la canción sin
        // penalizar la CPU. Un mapeo secuencial acumula en RSS todas las páginas
        // tocadas hasta `munmap`, así que la memoria del proceso crecería con el
        // tamaño del archivo durante toda la reproducción. Se liberan con
        // `MADV_DONTNEED` sobre el tramo ya consumido (una relectura lo repuebla
        // desde el archivo); agrupar el aviso evita un syscall por lectura, que era
        // el coste de CPU de liberar página a página. Ver `.planning/codebase/MEMORY-AND-CPU.md`.
        #[cfg(unix)]
        {
            let consumed = self.pos.saturating_sub(self.advised_until);
            if consumed >= MMAP_ADVISE_CHUNK {
                // SAFETY: el rango ya se copió a `buf` y no queda ningún préstamo
                // activo sobre él; el mapeo es de solo lectura, así que descartar
                // las páginas solo fuerza a repoblarlas desde el archivo en un
                // acceso posterior.
                unsafe {
                    let _ = self.mmap.unchecked_advise_range(
                        memmap2::UncheckedAdvice::DontNeed,
                        self.advised_until,
                        consumed,
                    );
                }
                self.advised_until = self.pos;
            }
        }
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
        // Un retroceso invalida el tramo ya liberado: se reabre el bloque para
        // que las páginas repobladas puedan volver a liberarse al avanzar.
        if self.pos < self.advised_until {
            self.advised_until = self.pos;
        }
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
        Ok(Box::new(MmapSource {
            mmap,
            pos: 0,
            advised_until: 0,
        }))
    } else {
        Ok(Box::new(file))
    }
}

/// Deriva la duración total de una pista en segundos.
///
/// Prefiere la duración declarada por el contenedor (`Track::time_base` +
/// `Track::duration`) y recurre a `num_frames / sample_rate` cuando el
/// contenedor no la declara. Devuelve `None` si ninguna fuente es utilizable
/// (por ejemplo, una tasa de muestreo de 0): el llamador lo traduce a una
/// duración de `0.0`, que desactiva el disparo temporal de la pre-carga y deja
/// que el decodificador dirija la transición por buffer/EOF.
fn derive_duration_sec(track: &symphonia::core::formats::Track, fallback_rate: u32) -> Option<f64> {
    if let Some(tb) = track.time_base
        && let Some(duration) = track.duration
        && let Some(t) = tb.calc_duration(duration)
    {
        return Some(t.as_secs_f64());
    }
    let sample_rate = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .and_then(|a| a.sample_rate)
        .unwrap_or(fallback_rate);
    if sample_rate == 0 {
        return None;
    }
    track.num_frames.map(|f| f as f64 / sample_rate as f64)
}

/// Calcula la relación de resampleo `out_rate / in_rate`.
///
/// Devuelve `None` cuando alguna de las tasas es 0: una relación no finita
/// pasada a `Async::new_sinc` produce un resampler inválido, así que el
/// llamador debe omitir el resampleo en ese caso.
fn resample_ratio(in_rate: u32, out_rate: u32) -> Option<f64> {
    if in_rate == 0 || out_rate == 0 {
        return None;
    }
    Some(out_rate as f64 / in_rate as f64)
}

use crate::audio::engine::{AudioCommand, AudioEngine, ChannelMap, LATENCY_PEAK_US};
use std::time::Instant;

/// Construye un span de diagnóstico por lote solo cuando el nivel DEBUG está
/// activo.
///
/// Los campos escalares se declaran vacíos y se rellenan al terminar el
/// trabajo con [`record_batch`], de modo que con el nivel apagado no se
/// construye el span ni se formatea nada por lote.
macro_rules! debug_batch_span {
    ($name:literal) => {
        if tracing::enabled!(tracing::Level::DEBUG) {
            Some(tracing::debug_span!(
                $name,
                frames = tracing::field::Empty,
                channels = tracing::field::Empty,
                elapsed_us = tracing::field::Empty
            ))
        } else {
            None
        }
    };
}

/// Registra los contadores del lote y su latencia en el span de diagnóstico.
/// Es un no-op cuando el span no existe (nivel DEBUG apagado).
#[inline]
fn record_batch(
    span: &Option<tracing::Span>,
    start: Option<Instant>,
    frames: usize,
    channels: usize,
) {
    if let (Some(span), Some(start)) = (span.as_ref(), start) {
        span.record("frames", frames);
        span.record("channels", channels);
        span.record("elapsed_us", start.elapsed().as_micros() as u64);
    }
}

// Volumen y Mezcla: Fade state machine
#[derive(Debug, Clone, Copy, PartialEq)]
enum FadeState {
    Idle,
    FadingIn { coeff: f64, rate_per_sec: f64 }, // rising 0→1, equal-power; coeff advances by rate_per_sec * batch_dt
    FadingOut { coeff: f64, rate_per_sec: f64 }, // falling 1→0, equal-power
}

// Volumen smoothing: waits for the user to finish adjusting the
// volume (debounce), then ramps from the current applied level to the target.
// Debounce = 400ms of inactivity; ramp = 1500ms linear.
pub(in crate::audio) const SMOOTH_DEBOUNCE_SECS: f64 = 0.4;
pub(in crate::audio) const SMOOTH_DURATION_SECS: f64 = 1.5;

#[derive(Debug, Clone, Copy)]
struct SmoothRamp {
    from: f64,
    to: f64,
    elapsed: f64,
    duration: f64,
}

/// Abre y prepara la pista de pre-carga pendiente (probe de symphonia + decoder).
///
/// Se ejecuta en el tiempo idle del loop (no en el handler del comando) para no
/// bloquear la reproducción de la pista actual durante la apertura del archivo.
#[allow(clippy::too_many_arguments)]
/// A preload chain opened off the decoder thread.
///
/// Opening and probing the next file can take tens of milliseconds on a slow
/// mount. Keeping that work on the decoder thread stalled the refill and could
/// underrun the ring; this thread only hands the finished chain back.
struct OpenedPreload {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn SymphoniaAudioDecoder>,
    track_id: u32,
    sample_rate: u32,
    duration_sec: f64,
    channel_map: ChannelMap,
    path: String,
    title: String,
    artist: String,
    album: String,
    cover: Option<String>,
    replay_gain: (Option<f32>, Option<f32>),
}

/// Where the audio for a manual crossfade comes from.
///
/// A manual change can arrive while the next track is still queued for its
/// blocking open, while the helper thread is already opening it, or with the
/// head already decoded. Keeping the four states apart is what lets the decoder
/// wait for an in-flight open instead of dropping the mix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManualPreloadSource {
    /// The head is decoded or the next track is already open.
    Ready,
    /// The open request is still queued on the decoder loop.
    Queued,
    /// The helper thread is opening the next track.
    InFlight,
    /// Nothing was requested, so the mix cannot start.
    Unavailable,
}

fn manual_preload_source(
    preload_open: bool,
    predecoded_frames: usize,
    queued: bool,
    in_flight: bool,
) -> ManualPreloadSource {
    if preload_open || predecoded_frames > 0 {
        ManualPreloadSource::Ready
    } else if queued {
        ManualPreloadSource::Queued
    } else if in_flight {
        ManualPreloadSource::InFlight
    } else {
        ManualPreloadSource::Unavailable
    }
}

fn open_preload_track_blocking(
    request: (
        String,
        String,
        String,
        String,
        Option<String>,
        Option<f64>,
        Option<f64>,
    ),
) -> Result<OpenedPreload, super::AudioError> {
    let (path, title, artist, album, cover_path, track_gain, album_gain) = request;

    let source = open_audio_source(&path).map_err(super::AudioError::IoError)?;
    let mss = MediaSourceStream::new(source, Default::default());
    let hint = Hint::new();
    let metadata_opts = MetadataOptions::default()
        .limit_tag_bytes(Limit::Maximum(0))
        .limit_visual_bytes(Limit::Maximum(0));
    let format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), metadata_opts)
        .map_err(|e| super::AudioError::UnsupportedFormat(format!("{path}: {e}")))?;
    let track = format.default_track(TrackType::Audio).ok_or_else(|| {
        super::AudioError::UnsupportedFormat(format!("{path}: sin pista de audio"))
    })?;
    let track_id = track.id;
    let audio_params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| {
            super::AudioError::UnsupportedFormat(format!("{path}: parámetros de audio ausentes"))
        })?;
    let sample_rate = audio_params.sample_rate.unwrap_or(44100);
    let duration_sec = derive_duration_sec(track, sample_rate).unwrap_or(0.0);

    let decoder = symphonia::default::get_codecs()
        .make_audio_decoder(audio_params, &AudioDecoderOptions::default())
        .map_err(|e| super::AudioError::UnsupportedFormat(format!("{path}: {e}")))?;

    let channel_map = if let Some(channels) = audio_params.channels.as_ref() {
        AudioEngine::get_channel_map(channels.clone())
    } else {
        ChannelMap {
            fl: Some(0),
            fr: Some(1),
            ..ChannelMap::default()
        }
    };

    Ok(OpenedPreload {
        format,
        decoder,
        track_id,
        sample_rate,
        duration_sec,
        channel_map,
        path,
        title,
        artist,
        album,
        cover: cover_path,
        replay_gain: (track_gain.map(|g| g as f32), album_gain.map(|g| g as f32)),
    })
}

#[allow(clippy::too_many_arguments)]
fn install_opened_preload(
    opened: OpenedPreload,
    preload_format: &mut Option<Box<dyn FormatReader>>,
    preload_decoder: &mut Option<Box<dyn SymphoniaAudioDecoder>>,
    preload_track_id: &mut u32,
    preload_sr: &mut u32,
    preload_total_duration_sec: &mut f64,
    preload_channel_map: &mut ChannelMap,
    preload_path: &mut Option<String>,
    preload_title: &mut Option<String>,
    preload_artist: &mut Option<String>,
    preload_album: &mut Option<String>,
    preload_cover: &mut Option<Option<String>>,
    preload_rg: &mut (Option<f32>, Option<f32>),
) {
    tracing::info!(
        "Pre-carga iniciada para '{}' ({} Hz).",
        opened.path,
        opened.sample_rate
    );
    *preload_format = Some(opened.format);
    *preload_decoder = Some(opened.decoder);
    *preload_track_id = opened.track_id;
    *preload_sr = opened.sample_rate;
    *preload_total_duration_sec = opened.duration_sec;
    *preload_channel_map = opened.channel_map;
    *preload_path = Some(opened.path);
    *preload_title = Some(opened.title);
    *preload_artist = Some(opened.artist);
    *preload_album = Some(opened.album);
    *preload_cover = Some(opened.cover);
    *preload_rg = opened.replay_gain;
}

/// Decodifica un lote de la pista pre-cargada y lo añade (f64 interleaved,
/// ya resampleado a `out_rate × out_channels`) al buffer de pre-decode.
///
/// Devuelve `true` si queda más audio por decodificar, `false` si se alcanzó
/// el EOF, hubo un error o el buffer alcanzó su capacidad máxima.
#[allow(clippy::too_many_arguments)]
fn preload_decode_batch(
    preload_format: &mut Option<Box<dyn FormatReader>>,
    preload_decoder: &mut Option<Box<dyn SymphoniaAudioDecoder>>,
    preload_track_id: &mut u32,
    preload_plane_pool: &mut Vec<Vec<f64>>,
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

    let span = debug_batch_span!("preload_batch");
    let start = span.as_ref().map(|_| Instant::now());
    let _enter = span.as_ref().map(|s| s.enter());

    if predecode_buffer.len() >= predecode_cap_frames {
        return true; // Lleno por ahora: se retoma cuando haya espacio.
    }

    let packet = match fmt.next_packet() {
        Ok(Some(p)) => p,
        Ok(None) => return false, // EOF de la pista pre-cargada.
        Err(e) => {
            tracing::warn!("Pre-carga decode error (skipping packet): {}", e);
            return true;
        }
    };

    if packet.track_id != *preload_track_id {
        return true;
    }

    let Ok(decoded) = dec.decode(&packet) else {
        return true;
    };
    let spec = decoded.spec().clone();

    // Copiar a un pool planar f64 reutilizado: sin asignación por paquete.
    decoded.copy_to_vecs_planar::<f64>(preload_plane_pool);
    let src_channels = spec.channels().count();

    if spec.rate() != out_rate {
        let recreate = preload_resampler_rates
            .map(|stored| stored != (spec.rate(), out_rate, src_channels))
            .unwrap_or(true);
        if recreate {
            let is_low = crate::utils::is_low_resource();
            let params = if is_low {
                SincInterpolationParameters {
                    sinc_len: 64,
                    f_cutoff: Some(0.95),
                    interpolation: SincInterpolationType::Linear,
                    oversampling_factor: 64,
                    window: WindowFunction::BlackmanHarris2,
                }
            } else {
                SincInterpolationParameters {
                    sinc_len: 256,
                    f_cutoff: Some(0.99),
                    interpolation: SincInterpolationType::Cubic,
                    oversampling_factor: 256,
                    window: WindowFunction::BlackmanHarris2,
                }
            };
            let chunk_size = if is_low { 256 } else { 1024 };
            let Some(ratio) = resample_ratio(spec.rate(), out_rate) else {
                tracing::warn!("Pre-carga sin resampleo: tasa de entrada inválida (0).");
                *preload_resampler = None;
                *preload_resampler_rates = None;
                return false;
            };
            match Async::<f64>::new_sinc(
                ratio,
                2.0,
                &params,
                chunk_size,
                src_channels,
                FixedAsync::Input,
            ) {
                Ok(r) => {
                    *preload_resampler = Some(r);
                    *preload_resampler_rates = Some((spec.rate(), out_rate, src_channels));
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
    if let Some(resampler) = preload_resampler.as_mut() {
        for c in 0..src_channels {
            if c < preload_resampler_in_buf.len()
                && let Some(plane) = preload_plane_pool.get(c)
            {
                preload_resampler_in_buf[c].extend(plane.iter().copied());
            }
        }
        if preload_input_pool.len() < src_channels {
            preload_input_pool.resize(src_channels, Vec::new());
        }
        if preload_output_pool.len() < src_channels {
            preload_output_pool.resize(src_channels, Vec::new());
        }
        loop {
            let needed = resampler.input_frames_next();
            if preload_resampler_in_buf.is_empty() || preload_resampler_in_buf[0].len() < needed {
                break;
            }
            let out_frames = resampler.output_frames_next();
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
            if resampler
                .process_into_buffer(&input_adapter, &mut output_adapter, None)
                .is_ok()
            {
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
        AudioEngine::mix_channels_planar(
            preload_plane_pool,
            decoded.frames(),
            src_channels,
            out_channels,
            preload_channel_map,
            downmix_conf,
            &mut batch_out,
        );
    }

    let appended = append_preload_batch(predecode_buffer, &mut batch_out);
    record_batch(&span, start, appended / out_channels.max(1), out_channels);
    true
}

/// Moves the predecoded head into the pending queue without copying it.
///
/// The head from the previous track has already drained, so the two buffers can
/// exchange roles: the filled one becomes the pending queue and the empty one is
/// ready for the next preload. Only a track shorter than the head leaves data in
/// the pending queue, and then the tail is appended (the one case that allocates).
fn promote_preload_head(
    pending: &mut std::collections::VecDeque<f64>,
    predecode: &mut std::collections::VecDeque<f64>,
) {
    if predecode.is_empty() {
        return;
    }
    if pending.is_empty() {
        std::mem::swap(pending, predecode);
    } else {
        pending.extend(predecode.drain(..));
    }
}

/// Hands the drained pending buffer to the preloader when it is the larger one.
fn reuse_drained_preload_buffer(
    pending: &mut std::collections::VecDeque<f64>,
    predecode: &mut std::collections::VecDeque<f64>,
) {
    if pending.is_empty() && predecode.is_empty() && pending.capacity() > predecode.capacity() {
        std::mem::swap(predecode, pending);
    }
}

/// Appends one decoded preload batch to the pre-decode head.
///
/// The batch is the unit the decoder and its resampler have already advanced
/// past, so the whole batch is kept even when it makes the head slightly larger
/// than the soft cap. Truncating it would drop the tail and leave a hole exactly
/// where the live decode resumes — audible as a cut at the end of the head. The
/// caller stops decoding once the head reaches the cap, so the overshoot is at
/// most one batch.
fn append_preload_batch(
    predecode_buffer: &mut std::collections::VecDeque<f64>,
    batch_out: &mut Vec<f64>,
) -> usize {
    let appended = batch_out.len();
    predecode_buffer.extend(batch_out.drain(..));
    appended
}

/// Indica si la mezcla cruzada automática debe dispararse: falta menos (o igual)
/// que la duración configurada para que termine la canción actual.
fn crossfade_auto_due(total_duration_sec: f64, current_pos_sec: f64, auto_ms: f64) -> bool {
    total_duration_sec > 0.0
        && auto_ms > 0.0
        && (total_duration_sec - current_pos_sec) <= (auto_ms / 1000.0)
}

/// Capacidad (en muestras f64 interleaved) del buffer de pre-decode.
///
/// Cubre la mezcla más larga configurada + margen de seguridad (máx 8s) para que
/// el inicio de la canción pre-cargada esté listo al iniciarse la mezcla, con un
/// tope de memoria de ~64 MB de f64 (~8M muestras): en configs normales el cap por
/// tiempo cubre la mezcla completa; en configs extremas (384kHz × 8 ch) el tope de
/// memoria acota el adelanto y el decoder cubre el resto en vivo.
pub(crate) fn predecode_cap_frames_for(cap_ms: f64, out_rate: u32, out_channels: usize) -> usize {
    const PRELOAD_MAX_SAMPLES: usize = 8_000_000;
    let cap_ms = (cap_ms + 2000.0).clamp(0.0, 8000.0);
    let sec_cap = ((cap_ms / 1000.0) * out_rate as f64) as usize * out_channels;
    sec_cap.min(PRELOAD_MAX_SAMPLES).max(out_channels)
}

/// Capacidad (en muestras f64 interleaved) del buffer de la cola de la mezcla:
/// ~200ms de la canción anterior, decodificada en flujo.
fn tail_buffer_cap_for(out_rate: u32, out_channels: usize) -> usize {
    (out_rate as usize * out_channels * 200) / 1000
}

/// Superpone la cola de la canción anterior sobre un frame de la primaria (mezcla
/// aditiva sin desvanecimiento, f64) con escala adaptativa de pico: solo si un
/// canal del frame supera 1.0 se normaliza el pico (evita recorte sin bajar el
/// nivel de las canciones durante la mezcla).
/// Mezcla la cola de la canción anterior sobre un frame de la primaria
/// (f64, aditiva) con un coeficiente de desvanecimiento aplicado a la cola.
/// Se usa durante la mezcla cruzada cuando el fade-out está activo para
/// la canción anterior: la cola se suma a la primaria pero con el volumen
/// reducido según la curva de fade-out.
fn mix_tail_into_frame_faded(
    frame: &mut [f64],
    tail: &mut std::collections::VecDeque<f64>,
    fade_coeff: f64,
) {
    for s in frame.iter_mut() {
        let Some(t) = tail.pop_front() else {
            return;
        };
        *s += t * fade_coeff;
    }
}

/// Mezcla la cola de la canción anterior sobre un frame de la primaria
/// (f64, aditiva sin desvanecimiento). Se usa cuando no hay fade activo
/// durante la mezcla cruzada (la cola suena a volumen completo).
#[cfg(test)]
fn mix_tail_into_frame(frame: &mut [f64], tail: &mut std::collections::VecDeque<f64>) {
    for s in frame.iter_mut() {
        let Some(t) = tail.pop_front() else {
            return;
        };
        *s += t;
    }
}

/// Decodifica la CANCIÓN ANTERIOR (cola de la mezcla) y añade su audio
/// (f64 interleaved, ya resampleado a `out_rate × out_channels`) al buffer
/// de cola HASTA LLENAR SU CAPACIDAD (~200ms) o alcanzar el EOF.
///
/// Una sola llamada rellena el buffer completo (decodifica varios paquetes):
/// el consumo de la mezcla (~100ms por iteración del decoder) queda cubierto
/// con cada relleno y la cola nunca se queda seca durante la ventana — antes
/// solo se decodificaba UN paquete por llamada (~43ms en FLAC 96 kHz, ~26ms
/// en WAV), lo que causaba el underrun de la cola (sonido entrecortado,
/// peor a 384 kHz).
///
/// Devuelve `true` si queda más audio, `false` si se alcanzó el EOF de la
/// pista anterior (la mezcla se acorta naturalmente) o hubo un error.
#[allow(clippy::too_many_arguments)]
fn tail_decode_batch(
    tail_format: &mut Option<Box<dyn FormatReader>>,
    tail_decoder: &mut Option<Box<dyn SymphoniaAudioDecoder>>,
    tail_track_id: &mut u32,
    tail_plane_pool: &mut Vec<Vec<f64>>,
    tail_resampler: &mut Option<Async<f64>>,
    tail_resampler_rates: &mut Option<(u32, u32, usize)>,
    tail_resampler_in_buf: &mut Vec<std::collections::VecDeque<f64>>,
    tail_input_pool: &mut Vec<Vec<f64>>,
    tail_output_pool: &mut Vec<Vec<f64>>,
    tail_channel_map: &mut ChannelMap,
    tail_buffer: &mut std::collections::VecDeque<f64>,
    tail_buffer_cap: usize,
    out_rate: u32,
    out_channels: usize,
) -> bool {
    let (Some(fmt), Some(dec)) = (tail_format.as_mut(), tail_decoder.as_mut()) else {
        return false;
    };

    let span = debug_batch_span!("tail_batch");
    let start = span.as_ref().map(|_| Instant::now());
    let _enter = span.as_ref().map(|s| s.enter());

    if tail_buffer.len() >= tail_buffer_cap {
        return true; // Cola llena por ahora: se retoma cuando haya espacio.
    }

    // Rellenar hasta la capacidad: cada llamada decodifica todos los paquetes
    // necesarios para cubrir ~200ms de cola, de modo que la mezcla nunca
    // consume más rápido de lo que se recarga (fix del entrecortado).
    let mut batch_out: Vec<f64> = Vec::new();
    let mut appended_frames = 0usize;
    while tail_buffer.len() < tail_buffer_cap {
        batch_out.clear();
        let packet = match fmt.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => return false, // La canción anterior terminó antes de la ventana de mezcla.
            Err(e) => {
                tracing::warn!("Cola decode error (skipping packet): {}", e);
                continue;
            }
        };

        if packet.track_id != *tail_track_id {
            continue;
        }

        let Ok(decoded) = dec.decode(&packet) else {
            continue;
        };
        let spec = decoded.spec().clone();

        // Copiar a un pool planar f64 reutilizado: sin asignación por paquete.
        decoded.copy_to_vecs_planar::<f64>(tail_plane_pool);
        let src_channels = spec.channels().count();

        if spec.rate() != out_rate {
            let recreate = tail_resampler_rates
                .map(|stored| stored != (spec.rate(), out_rate, src_channels))
                .unwrap_or(true);
            if recreate {
                let is_low = crate::utils::is_low_resource();
                let params = if is_low {
                    SincInterpolationParameters {
                        sinc_len: 64,
                        f_cutoff: Some(0.95),
                        interpolation: SincInterpolationType::Linear,
                        oversampling_factor: 64,
                        window: WindowFunction::BlackmanHarris2,
                    }
                } else {
                    SincInterpolationParameters {
                        sinc_len: 256,
                        f_cutoff: Some(0.99),
                        interpolation: SincInterpolationType::Cubic,
                        oversampling_factor: 256,
                        window: WindowFunction::BlackmanHarris2,
                    }
                };
                let chunk_size = if is_low { 256 } else { 1024 };
                let Some(ratio) = resample_ratio(spec.rate(), out_rate) else {
                    tracing::warn!("Cola sin resampleo: tasa de entrada inválida (0).");
                    *tail_resampler = None;
                    *tail_resampler_rates = None;
                    return false;
                };
                match Async::<f64>::new_sinc(
                    ratio,
                    2.0,
                    &params,
                    chunk_size,
                    src_channels,
                    FixedAsync::Input,
                ) {
                    Ok(r) => {
                        *tail_resampler = Some(r);
                        *tail_resampler_rates = Some((spec.rate(), out_rate, src_channels));
                        *tail_resampler_in_buf = (0..src_channels)
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
                        tracing::error!("Cola resampler init failed: {}", e);
                        *tail_resampler = None;
                        return false;
                    }
                }
            }
        } else {
            if tail_resampler.is_some() {
                *tail_resampler = None;
                tail_resampler_in_buf.clear();
                *tail_resampler_rates = None;
            }
        }

        let downmix_conf = {
            let side_val = 0.81f64;
            (0.74f64, 0.66f64, side_val, (side_val + 0.10).min(2.0))
        };

        if let Some(resampler) = tail_resampler.as_mut() {
            for c in 0..src_channels {
                if c < tail_resampler_in_buf.len()
                    && let Some(plane) = tail_plane_pool.get(c)
                {
                    tail_resampler_in_buf[c].extend(plane.iter().copied());
                }
            }
            if tail_input_pool.len() < src_channels {
                tail_input_pool.resize(src_channels, Vec::new());
            }
            if tail_output_pool.len() < src_channels {
                tail_output_pool.resize(src_channels, Vec::new());
            }
            loop {
                let needed = resampler.input_frames_next();
                if tail_resampler_in_buf.is_empty() || tail_resampler_in_buf[0].len() < needed {
                    break;
                }
                let out_frames = resampler.output_frames_next();
                for c in 0..src_channels {
                    tail_input_pool[c].clear();
                    if c < tail_resampler_in_buf.len() {
                        tail_input_pool[c].extend(tail_resampler_in_buf[c].drain(0..needed));
                    } else {
                        tail_input_pool[c].resize(needed, 0.0);
                    }
                    tail_output_pool[c].clear();
                    tail_output_pool[c].resize(out_frames, 0.0);
                }
                let input_adapter =
                    SequentialSliceOfVecs::new(tail_input_pool, src_channels, needed).unwrap();
                let mut output_adapter =
                    SequentialSliceOfVecs::new_mut(tail_output_pool, src_channels, out_frames)
                        .unwrap();
                if resampler
                    .process_into_buffer(&input_adapter, &mut output_adapter, None)
                    .is_ok()
                {
                    AudioEngine::mix_channels_planar(
                        tail_output_pool,
                        out_frames,
                        src_channels,
                        out_channels,
                        tail_channel_map,
                        downmix_conf,
                        &mut batch_out,
                    );
                }
            }
        } else {
            AudioEngine::mix_channels_planar(
                tail_plane_pool,
                decoded.frames(),
                src_channels,
                out_channels,
                tail_channel_map,
                downmix_conf,
                &mut batch_out,
            );
        }

        // Agregar TODO el audio decodificado al buffer de cola (sin cap).
        // Antes se descartaba el excedente cuando tail_buffer alcanzaba su
        // capacidad, causando pérdida PERMANENTE de audio → la cola sonaba
        // entrecortada porque cada paquete perdía samples al final. El mix
        // block consume la cola a tasa real, así que no crece indefinidamente.
        appended_frames += batch_out.len();
        tail_buffer.extend(batch_out.drain(..));
    }
    record_batch(
        &span,
        start,
        appended_frames / out_channels.max(1),
        out_channels,
    );
    true
}

/// Libera la cadena de decodificación de la COLA (decoder, resampler, pools)
/// pero PRESERVA el tail_buffer: el audio ya decodificado puede seguir
/// mezclándose hasta agotarse naturalmente. Se usa cuando la canción anterior
/// llega a EOF antes de completar la ventana de la mezcla.
#[allow(clippy::too_many_arguments)]
fn clear_tail_decoder_chain(
    tail_format: &mut Option<Box<dyn FormatReader>>,
    tail_decoder: &mut Option<Box<dyn SymphoniaAudioDecoder>>,
    tail_track_id: &mut u32,
    tail_channel_map: &mut ChannelMap,
    tail_resampler: &mut Option<Async<f64>>,
    tail_resampler_rates: &mut Option<(u32, u32, usize)>,
    tail_resampler_in_buf: &mut Vec<std::collections::VecDeque<f64>>,
    tail_plane_pool: &mut Vec<Vec<f64>>,
    tail_input_pool: &mut Vec<Vec<f64>>,
    tail_output_pool: &mut Vec<Vec<f64>>,
) {
    *tail_format = None;
    *tail_decoder = None;
    *tail_track_id = 0;
    *tail_channel_map = ChannelMap::default();
    *tail_resampler = None;
    *tail_resampler_rates = None;
    tail_resampler_in_buf.clear();
    tail_plane_pool.clear();
    tail_input_pool.clear();
    tail_output_pool.clear();
}

/// Libera el buffer de la cola y su capacidad retenida: se usa al completar
/// la ventana de la mezcla o al purgar por Load/Stop.
fn clear_tail_buffer(
    tail_buffer: &mut std::collections::VecDeque<f64>,
    tail_buffer_cap: &mut usize,
) {
    tail_buffer.clear();
    tail_buffer.shrink_to_fit();
    *tail_buffer_cap = 0;
}

/// Libera la cadena completa de la COLA de la mezcla (decoder, resampler,
/// pools y buffer) y su capacidad retenida: se usa al purgar por Load/Stop
/// donde todo el estado de la mezcla debe desaparecer inmediatamente.
#[allow(clippy::too_many_arguments)]
fn clear_tail_state(
    tail_format: &mut Option<Box<dyn FormatReader>>,
    tail_decoder: &mut Option<Box<dyn SymphoniaAudioDecoder>>,
    tail_track_id: &mut u32,
    tail_channel_map: &mut ChannelMap,
    tail_resampler: &mut Option<Async<f64>>,
    tail_resampler_rates: &mut Option<(u32, u32, usize)>,
    tail_resampler_in_buf: &mut Vec<std::collections::VecDeque<f64>>,
    tail_plane_pool: &mut Vec<Vec<f64>>,
    tail_input_pool: &mut Vec<Vec<f64>>,
    tail_output_pool: &mut Vec<Vec<f64>>,
    tail_buffer: &mut std::collections::VecDeque<f64>,
    tail_buffer_cap: &mut usize,
) {
    clear_tail_decoder_chain(
        tail_format,
        tail_decoder,
        tail_track_id,
        tail_channel_map,
        tail_resampler,
        tail_resampler_rates,
        tail_resampler_in_buf,
        tail_plane_pool,
        tail_input_pool,
        tail_output_pool,
    );
    clear_tail_buffer(tail_buffer, tail_buffer_cap);
}

/// Vuelca en el estado compartido los metadatos de la pista pre-cargada al promoverla
/// como primaria. Consume (`take`) cada opción pendiente — title/artist/album/cover/path —
/// para que los metadatos de la canción anterior no puedan resurgir, y fija las ganancias
/// ReplayGain, la duración, la tasa de muestras, la posición a 0.0 y el flag de EOF.
///
/// Lo usan tanto la promoción del crossfade (con `eof_reached = signal_eof`) como la
/// promoción por fin de pista natural (con `eof_reached = true`), de modo que ambas
/// rutas escriben exactamente el mismo conjunto de campos.
#[allow(clippy::too_many_arguments)]
fn apply_promotion_metadata(
    state: &mut crate::audio::engine::AudioState,
    preload_title: &mut Option<String>,
    preload_artist: &mut Option<String>,
    preload_album: &mut Option<String>,
    preload_cover: &mut Option<Option<String>>,
    preload_path: &mut Option<String>,
    preload_rg: (Option<f32>, Option<f32>),
    preload_total_duration_sec: f64,
    preload_sr: u32,
    eof_reached: bool,
) {
    if let Some(t) = preload_title.take() {
        state.title = t;
    }
    if let Some(a) = preload_artist.take() {
        state.artist = a;
    }
    if let Some(al) = preload_album.take() {
        state.album = al;
    }
    if let Some(c) = preload_cover.take() {
        state.cover_path = c;
    }
    if let Some(p) = preload_path.take() {
        state.path = p;
    }
    state.replay_gain_track = preload_rg.0;
    state.replay_gain_album = preload_rg.1;
    state.total_duration_sec = preload_total_duration_sec;
    state.sample_rate = preload_sr;
    state.current_pos_sec = 0.0;
    state.eof_reached = eof_reached;
}

/// Inicia el crossfade "hacia adelante": la pista pre-cargada pasa a ser la PRIMARIA
/// (suena desde el segundo 0; su inicio ya decodificado se mueve al buffer pendiente)
/// y la canción actual pasa a ser la COLA, que se decodifica en flujo y se mezcla por
/// encima de la primaria durante la ventana de la mezcla.
///
/// `signal_eof` indica si la GUI debe avanzar a la canción nueva (true en el cambio
/// automático; false en el manual, donde la GUI ya avanzó al pulsar Siguiente).
#[allow(clippy::too_many_arguments)]
fn begin_forward_crossfade(
    current_format: &mut Option<Box<dyn FormatReader>>,
    current_decoder: &mut Option<Box<dyn SymphoniaAudioDecoder>>,
    track_id: &mut u32,
    channel_map: &mut ChannelMap,
    resampler: &mut Option<Async<f64>>,
    resampler_rates: &mut Option<(u32, u32, usize)>,
    resampler_in_buf: &mut Vec<std::collections::VecDeque<f64>>,
    resample_input_pool: &mut Vec<Vec<f64>>,
    resample_output_pool: &mut Vec<Vec<f64>>,
    decode_plane_pool: &mut Vec<Vec<f64>>,
    tail_format: &mut Option<Box<dyn FormatReader>>,
    tail_decoder: &mut Option<Box<dyn SymphoniaAudioDecoder>>,
    tail_track_id: &mut u32,
    tail_channel_map: &mut ChannelMap,
    tail_resampler: &mut Option<Async<f64>>,
    tail_resampler_rates: &mut Option<(u32, u32, usize)>,
    tail_resampler_in_buf: &mut Vec<std::collections::VecDeque<f64>>,
    tail_plane_pool: &mut Vec<Vec<f64>>,
    tail_input_pool: &mut Vec<Vec<f64>>,
    tail_output_pool: &mut Vec<Vec<f64>>,
    tail_buffer: &mut std::collections::VecDeque<f64>,
    tail_buffer_cap: &mut usize,
    preload_format: &mut Option<Box<dyn FormatReader>>,
    preload_decoder: &mut Option<Box<dyn SymphoniaAudioDecoder>>,
    preload_track_id: &mut u32,
    preload_sr: &mut u32,
    preload_total_duration_sec: &mut f64,
    preload_channel_map: &mut ChannelMap,
    preload_resampler: &mut Option<Async<f64>>,
    preload_resampler_rates: &mut Option<(u32, u32, usize)>,
    preload_resampler_in_buf: &mut Vec<std::collections::VecDeque<f64>>,
    preload_plane_pool: &mut Vec<Vec<f64>>,
    preload_input_pool: &mut Vec<Vec<f64>>,
    preload_output_pool: &mut Vec<Vec<f64>>,
    predecode_buffer: &mut std::collections::VecDeque<f64>,
    preload_path: &mut Option<String>,
    preload_title: &mut Option<String>,
    preload_artist: &mut Option<String>,
    preload_album: &mut Option<String>,
    preload_cover: &mut Option<Option<String>>,
    preload_rg: &mut (Option<f32>, Option<f32>),
    preload_pending: &mut Option<PendingPreload>,
    preloaded_pending: &mut std::collections::VecDeque<f64>,
    fade_state: &mut FadeState,
    silence_samples: &mut usize,
    in_silence: &mut bool,
    track_start_trimmed: &mut bool,
    engine: &AudioEngine,
    state: &std::sync::Arc<parking_lot::RwLock<crate::audio::engine::AudioState>>,
    out_rate: u32,
    out_channels: usize,
    signal_eof: bool,
) {
    // 1. La canción actual pasa a ser la cola (tail), conservando su cadena completa
    //    (decoder, resampler, pools) para continuar decodificando desde su posición.
    *tail_format = current_format.take();
    *tail_decoder = current_decoder.take();
    *tail_track_id = *track_id;
    *tail_channel_map = *channel_map;
    *tail_resampler = resampler.take();
    *tail_resampler_rates = resampler_rates.take();
    *tail_resampler_in_buf = std::mem::take(resampler_in_buf);
    *tail_plane_pool = std::mem::take(decode_plane_pool);
    *tail_input_pool = std::mem::take(resample_input_pool);
    *tail_output_pool = std::mem::take(resample_output_pool);
    tail_buffer.clear();
    *tail_buffer_cap = tail_buffer_cap_for(out_rate, out_channels);

    // 2. La pista pre-cargada pasa a ser la primaria.
    *current_format = preload_format.take();
    *current_decoder = preload_decoder.take();
    *track_id = *preload_track_id;
    *channel_map = *preload_channel_map;
    *resampler = preload_resampler.take();
    *resampler_rates = preload_resampler_rates.take();
    *resampler_in_buf = std::mem::take(preload_resampler_in_buf);
    *decode_plane_pool = std::mem::take(preload_plane_pool);
    *resample_input_pool = std::mem::take(preload_input_pool);
    *resample_output_pool = std::mem::take(preload_output_pool);

    // 3. El inicio de la primaria (0→X) ya está decodificado en el predecode.
    //    La cabeza anterior ya se drenó, así que los dos buffers pueden intercambiar
    //    su papel sin copiar ni reservar: el mismo bloque de memoria pasa a ser la
    //    cola pendiente y el buffer vacío queda listo para la siguiente precarga.
    promote_preload_head(preloaded_pending, predecode_buffer);

    // 4. Actualizar el estado compartido a la primaria.
    {
        let mut s = state.write();
        // En el automático, EOF hace que la GUI avance el índice a la canción nueva;
        // en el manual la GUI ya avanzó al disparar la mezcla.
        apply_promotion_metadata(
            &mut s,
            preload_title,
            preload_artist,
            preload_album,
            preload_cover,
            preload_path,
            *preload_rg,
            *preload_total_duration_sec,
            *preload_sr,
            signal_eof,
        );
    }

    // Limpiar el resto de la pre-carga ya consumida (el path pendiente NO debe
    // reabrirse como pre-carga: la GUI dispara la siguiente cuando corresponda).
    *preload_channel_map = ChannelMap::default();
    *preload_rg = (None, None);
    *preload_pending = None;

    // 5. La primaria arranca con DSP limpio y fades según la columna izquierda.
    if let Some(mut dsp_lock) = engine.dsp.try_write() {
        dsp_lock.reset_state();
    }
    *fade_state = FadeState::Idle;
    *silence_samples = 0;
    *in_silence = false;
    *track_start_trimmed = false;
    {
        let s = state.read();
        if s.fades_enabled && s.fade_in_enabled && s.fade_in_ms > 0.0 {
            let rate_per_sec = 1.0 / ((s.fade_in_ms as f64) / 1000.0);
            *fade_state = FadeState::FadingIn {
                coeff: 0.0,
                rate_per_sec,
            };
        }
    }

    // 6. Pre-llenar el buffer de la cola: decodificar los primeros ~200ms de la
    //    canción anterior para que la primera iteración del mix tenga datos
    //    inmediatamente. Sin esto, la cola empieza vacía y el crossfade se
    //    retrasa una iteración (inaudible en manual, entrecortado en automático).
    if tail_decoder.is_some() {
        let tail_has_more = tail_decode_batch(
            tail_format,
            tail_decoder,
            tail_track_id,
            tail_plane_pool,
            tail_resampler,
            tail_resampler_rates,
            tail_resampler_in_buf,
            tail_input_pool,
            tail_output_pool,
            tail_channel_map,
            tail_buffer,
            *tail_buffer_cap,
            out_rate,
            out_channels,
        );
        if !tail_has_more {
            tracing::debug!(
                "Cola del crossfade sin más audio al pre-rellenar (EOF o error de decode)."
            );
        }
    }
}

pub(crate) fn audio_decode_loop(command_rx: Receiver<AudioCommand>, engine: AudioEngine) {
    let mut current_format: Option<Box<dyn FormatReader>> = None;
    let mut current_decoder: Option<Box<dyn SymphoniaAudioDecoder>> = None;
    let mut track_id = 0;

    let mut resampler: Option<Async<f64>> = None;
    let mut resampler_rates: Option<(u32, u32, usize)> = None;
    let mut resampler_in_buf: Vec<std::collections::VecDeque<f64>> = Vec::new();

    let state = engine.state.clone();
    let producer_mutex = engine.buffer_producer.clone();
    let mut channel_map = ChannelMap::default();
    // Última tasa de salida vista por el decoder (diagnóstico del cambio de config).
    let mut last_out_rate: u32 = state.read().device_sample_rate;
    // Heartbeat de diagnóstico: contador de iteraciones y última vez reportada.
    let mut heartbeat_iters: u64 = 0;
    let mut last_heartbeat: std::time::Instant = std::time::Instant::now();

    // Instrumentación de crossfade: timestamps para diagnóstico de entrecortado
    let mut crossfade_trace_enabled = false;
    let mut crossfade_start_time: Option<std::time::Instant> = None;
    let mut last_trace_time: Option<std::time::Instant> = None;
    let mut trace_iteration: u64 = 0;

    // Zero-Allocation Pool Buffers: Pre-asignados fuera del bucle para evitar GC pressure.
    let mut decode_plane_pool: Vec<Vec<f64>> = Vec::new();
    let mut resample_input_pool: Vec<Vec<f64>> = Vec::new();
    let mut resample_output_pool: Vec<Vec<f64>> = Vec::new();
    let mut output_accumulator: Vec<f64> = Vec::with_capacity(131072);
    let mut output_accumulator_f32: Vec<f32> = Vec::with_capacity(131072);
    // Acumulador de frames para la métrica `audio_frames_processed`: se agrega y se
    // emite un evento como mucho ~1/s (cuando se alcanza `out_rate` frames), en vez
    // de emitir por cada batch decodificado.
    let mut frames_since_metric: u64 = 0;

    // === Volumen y Mezcla state ===
    let mut fade_state: FadeState = FadeState::Idle;
    // Volume smoothing state. `applied_vol` is the level actually
    // applied to the audio; `pending_vol` is the user's target (s.volume).
    // The volume does NOT change while the user is adjusting; after 400ms of
    // inactivity a 1500ms ramp takes applied_vol from its current level to the
    // target. Initialized from the engine's real volume (not a hardcoded 0.3).
    let mut applied_vol: f64 = state.read().volume as f64;
    let mut pending_vol: f64 = applied_vol;
    let mut debounce_remaining: f64 = 0.0;
    let mut smooth_ramp: Option<SmoothRamp> = None;

    // Silence detection
    let mut silence_samples: usize = 0;
    let mut in_silence: bool = false;
    let mut track_start_trimmed: bool = false;

    // RG offset smoothing: ~100 ms EMA ramp to eliminate clicks on UI offset changes.
    // Time constant computes as: alpha = 1 - exp(-dt / 0.100) where dt is batch duration.
    let mut smoothed_rg_offset_album_db: f64 = 0.0;
    let mut smoothed_rg_offset_track_db: f64 = 0.0;
    let mut smoothed_rg_offset_rt_db: f64 = 0.0;

    // === Meter state — lock-free peak/RMS for VU meter ===
    // Peak accumulators: max |sample| per channel across the current batch.
    let mut meter_peak_l: f64 = 0.0;
    let mut meter_peak_r: f64 = 0.0;
    // RMS accumulators: sum-of-squares and count per channel (running accumulator).
    let mut rms_sum_l: f64 = 0.0;
    let mut rms_count_l: u64 = 0;
    let mut rms_sum_r: f64 = 0.0;
    let mut rms_count_r: u64 = 0;
    let mut rms_last_update: std::time::Instant = std::time::Instant::now();

    // === Pre-carga de la siguiente canción (transiciones sin cortes) ===
    // Decodifica la siguiente pista por adelantado a un buffer f64 ya resampleado
    // (out_rate × out_channels) para promoverla sin pausa cuando termina la actual.
    let mut preload_format: Option<Box<dyn FormatReader>> = None;
    let mut preload_decoder: Option<Box<dyn SymphoniaAudioDecoder>> = None;
    let mut preload_track_id: u32 = 0;
    let mut preload_sr: u32 = 0;
    let mut preload_total_duration_sec: f64 = 0.0;
    let mut preload_channel_map = ChannelMap::default();
    let mut preload_path: Option<String> = None;
    let mut preload_title: Option<String> = None;
    let mut preload_artist: Option<String> = None;
    let mut preload_album: Option<String> = None;
    let mut preload_cover: Option<Option<String>> = None;
    let mut preload_rg: (Option<f32>, Option<f32>) = (None, None);
    // Solicitud de pre-carga pendiente: la apertura del archivo (probe de symphonia)
    // NO se hace en el handler del comando (bloquearía la reproducción) sino en el
    // tiempo idle del loop, donde no interrumpe el flujo de audio.
    let mut preload_pending: Option<PendingPreload> = None;
    let mut preload_open_rx: Option<crossbeam::channel::Receiver<Result<OpenedPreload, String>>> =
        None;
    let mut predecode_buffer: std::collections::VecDeque<f64> = std::collections::VecDeque::new();
    let mut predecode_cap_frames: usize = 0;
    let mut preloaded_pending: std::collections::VecDeque<f64> = std::collections::VecDeque::new();
    // Resampler propio de la pre-carga (independiente del de la pista actual).
    let mut preload_resampler: Option<Async<f64>> = None;
    let mut preload_resampler_rates: Option<(u32, u32, usize)> = None;
    let mut preload_resampler_in_buf: Vec<std::collections::VecDeque<f64>> = Vec::new();
    let mut preload_plane_pool: Vec<Vec<f64>> = Vec::new();
    let mut preload_input_pool: Vec<Vec<f64>> = Vec::new();
    let mut preload_output_pool: Vec<Vec<f64>> = Vec::new();

    // === Crossfade (Mezcla Cruzada) — estado de la mezcla f64 ===
    let mut crossfade_active: bool = false;
    let mut crossfade_elapsed_sec: f64 = 0.0;
    let mut crossfade_duration_sec: f64 = 0.0;

    // Cola (tail) de la canción anterior durante la mezcla: al iniciarse el crossfade
    // la canción pre-cargada pasa a ser la PRIMARIA (suena desde el segundo 0) y la
    // canción anterior solo aporta su cola como capa, decodificada en flujo (~200ms
    // de buffer) para no retener minutos de audio si el salto es a mitad de canción.
    let mut tail_format: Option<Box<dyn FormatReader>> = None;
    let mut tail_decoder: Option<Box<dyn SymphoniaAudioDecoder>> = None;
    let mut tail_track_id: u32 = 0;
    let mut tail_channel_map = ChannelMap::default();
    let mut tail_resampler: Option<Async<f64>> = None;
    let mut tail_resampler_rates: Option<(u32, u32, usize)> = None;
    let mut tail_resampler_in_buf: Vec<std::collections::VecDeque<f64>> = Vec::new();
    let mut tail_plane_pool: Vec<Vec<f64>> = Vec::new();
    let mut tail_input_pool: Vec<Vec<f64>> = Vec::new();
    let mut tail_output_pool: Vec<Vec<f64>> = Vec::new();
    let mut tail_buffer: std::collections::VecDeque<f64> = std::collections::VecDeque::new();
    let mut tail_buffer_cap: usize = 0;

    loop {
        // Limpieza TOTAL por ciclo para evitar que datos fantasmas (basura residual) se queden en el acumulador.
        // Esto erradica los pitidos y zumbidos al cambiar de canción o al procesar OGG irregulares.
        output_accumulator.clear();
        output_accumulator_f32.clear();
        // Si los acumuladores retienen mucha más capacidad de la necesaria (p. ej.
        // tras un volcado de pre-carga de varios segundos), se libera al asignador.
        if output_accumulator.capacity() > 1_000_000 {
            output_accumulator.shrink_to_fit();
        }
        if output_accumulator_f32.capacity() > 1_000_000 {
            output_accumulator_f32.shrink_to_fit();
        }

        // === TRAZA DE CROSSFADE: log de inicio de iteración ===
        // El guard de nivel evita calcular timestamps y tomar el lock del productor
        // cuando el nivel DEBUG está desactivado (era 2 locks por iteración).
        if crossfade_active && tracing::enabled!(tracing::Level::DEBUG) {
            let now = std::time::Instant::now();
            let ms_since_start = crossfade_start_time
                .map(|t| now.duration_since(t).as_millis())
                .unwrap_or(0);
            let ms_since_last = last_trace_time
                .map(|t| now.duration_since(t).as_millis())
                .unwrap_or(0);
            last_trace_time = Some(now);
            trace_iteration += 1;

            let ringbuf_occupied = engine
                .buffer_producer
                .lock()
                .as_ref()
                .map(|p| p.occupied_len())
                .unwrap_or(0);

            tracing::debug!(
                "[XFADE_TRACE] iter={} t={}ms Δ={}ms | ringbuf={}/{} | tail_buf={}/{} | preloaded_pending={}",
                trace_iteration,
                ms_since_start,
                ms_since_last,
                ringbuf_occupied,
                engine
                    .buffer_producer
                    .lock()
                    .as_ref()
                    .map(|p| p.capacity().get())
                    .unwrap_or(0),
                tail_buffer.len(),
                tail_buffer_cap,
                preloaded_pending.len()
            );
        } else {
            // Reset trace state when crossfade ends
            if crossfade_start_time.is_some() {
                tracing::debug!("[XFADE_TRACE] Crossfade ended at iter={}", trace_iteration);
                crossfade_start_time = None;
                last_trace_time = None;
                trace_iteration = 0;
            }
        }

        // Check for commands. La búsqueda pendiente vive en el slot atómico
        // compartido y tiene prioridad sobre la cola: se aplica antes que el
        // comando en lote y no consume capacidad del canal.
        let seek_cmd = engine.take_pending_seek().map(AudioCommand::Seek);
        let cmd_result = if let Some(cmd) = seek_cmd {
            Ok(cmd)
        } else if current_format.is_none() {
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
                    album,
                    cover_path,
                    track_gain,
                    album_gain,
                } => {
                    // Dedup: si el path ya es la pista actual y no hemos llegado al EOF,
                    // ignoramos la orden — evita re-abrir/re-probar el archivo (y el corte
                    // asociado) cuando la GUI confirma un avance que el decoder ya manejó
                    // vía promoción de pre-carga. También se ignora cuando el path es la
                    // pista pre-cargada Y hay una mezcla en curso (el decoder la está
                    // promoviendo con el crossfade manual; un Load aquí purgaría el
                    // estado y destruiría la mezcla). Sin mezcla activa, el Load debe
                    // PROCEDER: es un cambio manual normal y el dedup dejaría la
                    // reproducción bloqueada hasta el final de la canción actual.
                    {
                        let s = state.read();
                        let is_current = s.path == path && !s.eof_reached && s.is_playing;
                        let is_preloaded = crossfade_active
                            && preload_path.as_deref() == Some(path.as_str())
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
                        s.album = album;
                        s.cover_path = cover_path;
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
                        let open_err = {
                            let mut custom = engine.custom_decoder.lock();
                            custom.as_mut().and_then(|dec| dec.open(&path).err())
                        };
                        if let Some(e) = open_err {
                            let msg =
                                format!("No se pudo abrir el archivo en el decodificador: {e}");
                            tracing::error!("dec.open falló para '{}': {}", path, e);
                            state.write().audio_notice = Some(msg);
                        }
                        state.write().eof_reached = false;
                        engine.drain_ring_buffer();
                        continue;
                    }

                    match open_audio_source(&path) {
                        Ok(source) => {
                            let mss = MediaSourceStream::new(source, Default::default());
                            let hint = Hint::new();
                            let metadata_opts = MetadataOptions::default()
                                .limit_tag_bytes(Limit::Maximum(0)) // No cargar metadatos, ya los tenemos en la DB
                                .limit_visual_bytes(Limit::Maximum(0));

                            match symphonia::default::get_probe().probe(
                                &hint,
                                mss,
                                FormatOptions::default(),
                                metadata_opts,
                            ) {
                                Ok(format) => {
                                    let Some(track) = format.default_track(TrackType::Audio) else {
                                        let msg =
                                            "Formato de audio no soportado: sin pista de audio"
                                                .to_string();
                                        tracing::error!("{}", msg);
                                        state.write().audio_notice = Some(msg);
                                        continue;
                                    };
                                    track_id = track.id;
                                    let Some(audio_params) =
                                        track.codec_params.as_ref().and_then(|p| p.audio())
                                    else {
                                        let msg = "Formato de audio no soportado: parámetros de audio ausentes".to_string();
                                        tracing::error!("{}", msg);
                                        state.write().audio_notice = Some(msg);
                                        continue;
                                    };
                                    let sample_rate = audio_params.sample_rate.unwrap_or(44100);
                                    let duration =
                                        derive_duration_sec(track, sample_rate).unwrap_or(0.0);
                                    state.write().total_duration_sec = duration;
                                    state.write().sample_rate = sample_rate;

                                    // --- PURGA MAESTRA DE ESTADO (Anti-Residuos) ---
                                    resampler = None;
                                    resampler_rates = None;
                                    resampler_in_buf.clear();
                                    resample_input_pool.clear();
                                    resample_output_pool.clear();
                                    output_accumulator.clear();
                                    output_accumulator_f32.clear();
                                    decode_plane_pool.clear(); // CRITICO: Evita reusar layout de buffer de canción anterior
                                    channel_map = ChannelMap::default();
                                    {
                                        if let Some(mut dsp_lock) = engine.dsp.try_write() {
                                            dsp_lock.reset_state();
                                        }
                                    }
                                    // Liberar también la pre-carga (decenas/cientos de MB del
                                    // predecode + el mmap del archivo): el Load manual inicia
                                    // una pista nueva y la pre-carga vieja ya no sirve.
                                    preload_format = None;
                                    preload_decoder = None;
                                    preload_resampler = None;
                                    preload_resampler_rates = None;
                                    preload_resampler_in_buf.clear();
                                    preload_plane_pool.clear();
                                    preload_input_pool.clear();
                                    preload_output_pool.clear();
                                    predecode_buffer.clear();
                                    preload_path = None;
                                    preload_title = None;
                                    preload_artist = None;
                                    preload_album = None;
                                    preload_cover = None;
                                    preload_rg = (None, None);
                                    preload_pending = None;
                                    tracing::info!(
                                        "Audio Engine State Purged (Load): Buffers & DSP Reset."
                                    );

                                    // Volumen y mezcla: reinicio del estado y disparo del fade-in
                                    silence_samples = 0;
                                    in_silence = false;
                                    track_start_trimmed = false;

                                    // Reset del crossfade: una pista nueva arranca sin mezcla.
                                    crossfade_active = false;
                                    crossfade_elapsed_sec = 0.0;
                                    crossfade_duration_sec = 0.0;
                                    preloaded_pending.clear();

                                    // Limpiar la cola (tail) de una mezcla en curso: la
                                    // pista cargada manualmente no debe heredarla.
                                    clear_tail_state(
                                        &mut tail_format,
                                        &mut tail_decoder,
                                        &mut tail_track_id,
                                        &mut tail_channel_map,
                                        &mut tail_resampler,
                                        &mut tail_resampler_rates,
                                        &mut tail_resampler_in_buf,
                                        &mut tail_plane_pool,
                                        &mut tail_input_pool,
                                        &mut tail_output_pool,
                                        &mut tail_buffer,
                                        &mut tail_buffer_cap,
                                    );

                                    let s = state.read();
                                    // Fade-in on EVERY track start when enabled.
                                    // (Not gated on natural EOF — user expects a smooth rise
                                    // whenever a song begins.)
                                    if s.fades_enabled && s.fade_in_enabled && s.fade_in_ms > 0.0 {
                                        let fade_ms = s.fade_in_ms as f64;
                                        if fade_ms > 0.0 {
                                            // rate_per_sec: fraction of the fade completed per
                                            // second of *real* audio time. Independent of sample
                                            // rate and batch size (robust timing).
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

                                    match symphonia::default::get_codecs().make_audio_decoder(
                                        audio_params,
                                        &AudioDecoderOptions::default(),
                                    ) {
                                        Ok(decoder) => {
                                            current_decoder = Some(decoder);

                                            // Update Channel Map
                                            if let Some(channels) = audio_params.channels.as_ref() {
                                                channel_map =
                                                    AudioEngine::get_channel_map(channels.clone());
                                            } else {
                                                // Fallback for no layout
                                                channel_map = ChannelMap::default();
                                                channel_map.fl = Some(0);
                                                channel_map.fr = Some(1);
                                            }

                                            current_format = Some(format);
                                            let mut s = state.write();
                                            s.eof_reached = false;
                                        }
                                        Err(e) => {
                                            let msg = format!("Formato de audio no soportado: {e}");
                                            tracing::error!("{}", msg);
                                            state.write().audio_notice = Some(msg);
                                        }
                                    }
                                }
                                Err(e) => {
                                    let msg = format!("Formato de audio no soportado: {e}");
                                    tracing::error!("{}", msg);
                                    state.write().audio_notice = Some(msg);
                                }
                            }

                            // Descartar el audio anterior del anillo en lotes
                            // acotados para no bloquear el callback real-time.
                            engine.drain_ring_buffer();
                        }
                        Err(e) => tracing::error!("Error abriendo archivo: {}", e),
                    }
                }
                AudioCommand::PreloadNext {
                    path,
                    title,
                    artist,
                    album,
                    cover_path,
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

                    // Descartar cualquier pre-carga anterior y registrar la solicitud.
                    // La apertura del archivo se difiere al tiempo idle del loop para
                    // no bloquear la reproducción de la pista actual.
                    preload_format = None;
                    preload_decoder = None;
                    preload_resampler = None;
                    preload_resampler_rates = None;
                    preload_resampler_in_buf.clear();
                    preload_plane_pool.clear();
                    preload_input_pool.clear();
                    preload_output_pool.clear();
                    predecode_buffer.clear();
                    preload_path = None;
                    preload_title = None;
                    preload_artist = None;
                    preload_album = None;
                    preload_cover = None;
                    preload_rg = (None, None);
                    // A new request supersedes any open still running for an older
                    // one; the helper thread sends into a dropped receiver.
                    preload_open_rx = None;
                    preload_pending = Some((
                        path, title, artist, album, cover_path, track_gain, album_gain,
                    ));
                }
                AudioCommand::ClearPreload => {
                    preload_format = None;
                    preload_decoder = None;
                    preload_resampler = None;
                    preload_resampler_rates = None;
                    preload_resampler_in_buf.clear();
                    preload_plane_pool.clear();
                    preload_input_pool.clear();
                    preload_output_pool.clear();
                    predecode_buffer.clear();
                    preload_path = None;
                    preload_title = None;
                    preload_artist = None;
                    preload_album = None;
                    preload_cover = None;
                    preload_rg = (None, None);
                    preload_pending = None;
                    preloaded_pending.clear();
                }
                AudioCommand::CrossfadeNext(ms) => {
                    // Activa un crossfade manual con la duración indicada.
                    let ms = if ms.is_finite() {
                        ms.clamp(0.0, 10000.0)
                    } else {
                        0.0
                    };
                    let preload_source = if ms > 0.0 {
                        manual_preload_source(
                            preload_format.is_some(),
                            predecode_buffer.len(),
                            preload_pending.is_some(),
                            preload_open_rx.is_some(),
                        )
                    } else {
                        ManualPreloadSource::Unavailable
                    };
                    if preload_source == ManualPreloadSource::Ready {
                        // Si ya hay una mezcla en curso, el usuario quiere cambiar
                        // AHORA: completar la mezcla activa de inmediato (el cambio
                        // de pista no debe quedar bloqueado hasta que termine).
                        if crossfade_active {
                            crossfade_elapsed_sec = crossfade_duration_sec;
                        } else {
                            // Crossfade hacia adelante: la pre-cargada pasa a ser la
                            // primaria (suena desde 0) y la actual pasa a la cola.
                            let (cmd_out_rate, cmd_out_channels) = {
                                let s = state.read();
                                (s.device_sample_rate, s.channels as usize)
                            };
                            begin_forward_crossfade(
                                &mut current_format,
                                &mut current_decoder,
                                &mut track_id,
                                &mut channel_map,
                                &mut resampler,
                                &mut resampler_rates,
                                &mut resampler_in_buf,
                                &mut resample_input_pool,
                                &mut resample_output_pool,
                                &mut decode_plane_pool,
                                &mut tail_format,
                                &mut tail_decoder,
                                &mut tail_track_id,
                                &mut tail_channel_map,
                                &mut tail_resampler,
                                &mut tail_resampler_rates,
                                &mut tail_resampler_in_buf,
                                &mut tail_plane_pool,
                                &mut tail_input_pool,
                                &mut tail_output_pool,
                                &mut tail_buffer,
                                &mut tail_buffer_cap,
                                &mut preload_format,
                                &mut preload_decoder,
                                &mut preload_track_id,
                                &mut preload_sr,
                                &mut preload_total_duration_sec,
                                &mut preload_channel_map,
                                &mut preload_resampler,
                                &mut preload_resampler_rates,
                                &mut preload_resampler_in_buf,
                                &mut preload_plane_pool,
                                &mut preload_input_pool,
                                &mut preload_output_pool,
                                &mut predecode_buffer,
                                &mut preload_path,
                                &mut preload_title,
                                &mut preload_artist,
                                &mut preload_album,
                                &mut preload_cover,
                                &mut preload_rg,
                                &mut preload_pending,
                                &mut preloaded_pending,
                                &mut fade_state,
                                &mut silence_samples,
                                &mut in_silence,
                                &mut track_start_trimmed,
                                &engine,
                                &state,
                                cmd_out_rate,
                                cmd_out_channels,
                                false, // manual: la GUI ya avanzó
                            );
                            crossfade_duration_sec = ms / 1000.0;
                            crossfade_elapsed_sec = 0.0;
                            crossfade_active = true;
                            // Inicializar traza de diagnóstico
                            crossfade_trace_enabled = true;
                            crossfade_start_time = Some(std::time::Instant::now());
                            last_trace_time = crossfade_start_time;
                            trace_iteration = 0;
                            tracing::debug!("[XFADE_TRACE] Crossfade manual iniciado: {} ms", ms);
                        }
                    } else if matches!(
                        preload_source,
                        ManualPreloadSource::Queued | ManualPreloadSource::InFlight
                    ) {
                        // The GUI asks for the preload right before this command, so
                        // at this point the request is either still queued or already
                        // handed to the helper thread. Cover both: the ring buffer
                        // absorbs the short wait and the mix starts immediately.
                        let (cmd_out_rate, cmd_out_channels) = {
                            let s = state.read();
                            (s.device_sample_rate, s.channels as usize)
                        };
                        if let Some(request) = preload_pending.take() {
                            // A queued request wins; any older in-flight open is stale.
                            preload_open_rx = None;
                            match open_preload_track_blocking(request) {
                                Ok(opened) => install_opened_preload(
                                    opened,
                                    &mut preload_format,
                                    &mut preload_decoder,
                                    &mut preload_track_id,
                                    &mut preload_sr,
                                    &mut preload_total_duration_sec,
                                    &mut preload_channel_map,
                                    &mut preload_path,
                                    &mut preload_title,
                                    &mut preload_artist,
                                    &mut preload_album,
                                    &mut preload_cover,
                                    &mut preload_rg,
                                ),
                                Err(e) => tracing::error!("Pre-carga falló: {}", e),
                            }
                        } else if let Some(receiver) = preload_open_rx.take() {
                            // The helper thread is opening the next track; block for
                            // its result instead of dropping the mix.
                            match receiver.recv_timeout(std::time::Duration::from_secs(2)) {
                                Ok(Ok(opened)) => install_opened_preload(
                                    opened,
                                    &mut preload_format,
                                    &mut preload_decoder,
                                    &mut preload_track_id,
                                    &mut preload_sr,
                                    &mut preload_total_duration_sec,
                                    &mut preload_channel_map,
                                    &mut preload_path,
                                    &mut preload_title,
                                    &mut preload_artist,
                                    &mut preload_album,
                                    &mut preload_cover,
                                    &mut preload_rg,
                                ),
                                Ok(Err(e)) => tracing::error!("Pre-carga falló: {}", e),
                                Err(_) => tracing::error!(
                                    "Pre-carga no terminó a tiempo para la mezcla manual"
                                ),
                            }
                        }
                        if preload_format.is_some() {
                            let xfade_max_ms = {
                                let s = state.read();
                                s.crossfade_manual_ms.max(s.crossfade_auto_ms).max(0.0)
                            };
                            predecode_cap_frames = predecode_cap_frames_for(
                                xfade_max_ms as f64,
                                cmd_out_rate,
                                cmd_out_channels.max(1),
                            );
                        }
                        if preload_format.is_some() {
                            let preload_has_more = preload_decode_batch(
                                &mut preload_format,
                                &mut preload_decoder,
                                &mut preload_track_id,
                                &mut preload_plane_pool,
                                &mut preload_resampler,
                                &mut preload_resampler_rates,
                                &mut preload_resampler_in_buf,
                                &mut preload_input_pool,
                                &mut preload_output_pool,
                                &mut preload_channel_map,
                                &mut predecode_buffer,
                                predecode_cap_frames,
                                cmd_out_rate,
                                cmd_out_channels,
                            );
                            if !preload_has_more {
                                tracing::debug!(
                                    "Pre-carga sin más audio al iniciar la mezcla (EOF o error de decode)."
                                );
                            }
                        }
                        if preload_format.is_some() || !predecode_buffer.is_empty() {
                            begin_forward_crossfade(
                                &mut current_format,
                                &mut current_decoder,
                                &mut track_id,
                                &mut channel_map,
                                &mut resampler,
                                &mut resampler_rates,
                                &mut resampler_in_buf,
                                &mut resample_input_pool,
                                &mut resample_output_pool,
                                &mut decode_plane_pool,
                                &mut tail_format,
                                &mut tail_decoder,
                                &mut tail_track_id,
                                &mut tail_channel_map,
                                &mut tail_resampler,
                                &mut tail_resampler_rates,
                                &mut tail_resampler_in_buf,
                                &mut tail_plane_pool,
                                &mut tail_input_pool,
                                &mut tail_output_pool,
                                &mut tail_buffer,
                                &mut tail_buffer_cap,
                                &mut preload_format,
                                &mut preload_decoder,
                                &mut preload_track_id,
                                &mut preload_sr,
                                &mut preload_total_duration_sec,
                                &mut preload_channel_map,
                                &mut preload_resampler,
                                &mut preload_resampler_rates,
                                &mut preload_resampler_in_buf,
                                &mut preload_plane_pool,
                                &mut preload_input_pool,
                                &mut preload_output_pool,
                                &mut predecode_buffer,
                                &mut preload_path,
                                &mut preload_title,
                                &mut preload_artist,
                                &mut preload_album,
                                &mut preload_cover,
                                &mut preload_rg,
                                &mut preload_pending,
                                &mut preloaded_pending,
                                &mut fade_state,
                                &mut silence_samples,
                                &mut in_silence,
                                &mut track_start_trimmed,
                                &engine,
                                &state,
                                cmd_out_rate,
                                cmd_out_channels,
                                false, // manual: la GUI ya avanzó
                            );
                            crossfade_duration_sec = ms / 1000.0;
                            crossfade_elapsed_sec = 0.0;
                            crossfade_active = true;
                            // Inicializar traza de diagnóstico
                            crossfade_trace_enabled = true;
                            crossfade_start_time = Some(std::time::Instant::now());
                            last_trace_time = crossfade_start_time;
                            trace_iteration = 0;
                            tracing::debug!(
                                "[XFADE_TRACE] Crossfade manual iniciado: {} ms (pre-carga abierta)",
                                ms
                            );
                        } else {
                            // La apertura falló: se descarta la pre-carga para que el
                            // Load que envía la GUI haga la transición normal.
                            preload_pending = None;
                            predecode_buffer.clear();
                            tracing::info!("Crossfade manual sin pre-carga: transición inmediata.");
                        }
                    } else if ms > 0.0 {
                        // No hay audio pre-cargado listo (el usuario saltó antes de que
                        // la pre-carga abriera/decodificara): NO se puede mezclar. Se
                        // descarta la pre-carga para que el Load que envía la GUI haga
                        // la transición normal INMEDIATA (si se conservara, el dedup
                        // bloquearía el cambio hasta el final de la canción actual).
                        preload_format = None;
                        preload_decoder = None;
                        preload_path = None;
                        preload_pending = None;
                        predecode_buffer.clear();
                        tracing::info!("Crossfade manual sin pre-carga: transición inmediata.");
                    }
                }
                AudioCommand::Seek(time) => {
                    let seek_time = match symphonia::core::units::Time::try_from_secs_f64(time) {
                        Some(t) => t,
                        None => {
                            tracing::warn!("Búsqueda ignorada: tiempo inválido ({time})");
                            continue;
                        }
                    };
                    if let Some(fmt) = current_format.as_mut() {
                        if let Err(e) = fmt.seek(
                            symphonia::core::formats::SeekMode::Accurate,
                            symphonia::core::formats::SeekTo::Time {
                                time: seek_time,
                                track_id: Some(track_id),
                            },
                        ) {
                            tracing::warn!("Seek falló ({time} s): {e}");
                        }
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
                        decode_plane_pool.clear(); // Reset buffer layout
                        {
                            if let Some(mut dsp_lock) = engine.dsp.try_write() {
                                dsp_lock.reset_state();
                            }
                        }
                        tracing::info!("Audio Engine State Purged (Seek).");

                        // Volumen y Mezcla: Reset on seek
                        fade_state = FadeState::Idle;
                        silence_samples = 0;
                        in_silence = false;
                        track_start_trimmed = false;

                        // Descartar el audio anterior del anillo en lotes acotados.
                        engine.drain_ring_buffer();
                    }
                }
                AudioCommand::Stop => {
                    current_format = None;
                    engine.set_playing(false);
                    state.write().current_pos_sec = 0.0;

                    // Volumen y Mezcla: Reset on stop
                    fade_state = FadeState::Idle;
                    silence_samples = 0;
                    in_silence = false;
                    track_start_trimmed = false;
                }
            }
        }

        // Control de Latencia (Virtual Buffer Size) limitando el RingBuffer
        let (out_rate, out_channels) = {
            let s = state.read();
            (s.device_sample_rate, s.channels as usize)
        };

        // Heartbeat de diagnóstico (1/s, solo en RUST_LOG=audoxidy=debug): permite ver si
        // el decoder está vivo y qué ve en el ringbuf (diagnóstico del cambio de tasa).
        heartbeat_iters += 1;
        if last_heartbeat.elapsed().as_millis() >= 1000 {
            let (occupied, capacity) = engine
                .buffer_producer
                .lock()
                .as_ref()
                .map(|p| (p.occupied_len(), p.capacity().get()))
                .unwrap_or((0, 0));
            tracing::debug!(
                "HEARTBEAT decoder: iteraciones/s={}, ringbuf ocupado={}/{} ({} Hz, {} ch), \
                 resampler={}, decodificando={}",
                heartbeat_iters,
                occupied,
                capacity,
                out_rate,
                out_channels,
                resampler.is_some(),
                current_decoder.is_some()
            );
            heartbeat_iters = 0;
            last_heartbeat = std::time::Instant::now();
        }
        // Diagnóstico: registrar el cambio de tasa de salida detectado por el decoder.
        if out_rate != last_out_rate {
            tracing::info!(
                "Decoder detectó cambio de tasa de salida: {} -> {} Hz ({} canales)",
                last_out_rate,
                out_rate,
                out_channels
            );
            // La pre-carga existente quedó decodificada a la tasa ANTERIOR: si se
            // promoviera mezclaría tasas (basura/silencio). Se descarta; la GUI
            // vuelve a pre-cargar a ~15s del final de la canción.
            preload_format = None;
            preload_decoder = None;
            preload_resampler = None;
            preload_resampler_rates = None;
            preload_resampler_in_buf.clear();
            preload_plane_pool.clear();
            preload_input_pool.clear();
            preload_output_pool.clear();
            predecode_buffer.clear();
            preload_path = None;
            preload_title = None;
            preload_artist = None;
            preload_album = None;
            preload_cover = None;
            preload_rg = (None, None);
            preload_pending = None;
            preloaded_pending.clear();
            preloaded_pending.shrink_to_fit();
            last_out_rate = out_rate;
        }

        // Open and probe the next file off the decoder thread. The helper
        // only sends the finished chain; the ring keeps refilling meanwhile.
        if preload_pending.is_some() && preload_format.is_none() && preload_open_rx.is_none() {
            // The previous head drained long ago; reuse its empty buffer for
            // this preload instead of allocating a second one.
            reuse_drained_preload_buffer(&mut preloaded_pending, &mut predecode_buffer);
            if let Some(request) = preload_pending.take() {
                let (sender, receiver) = crossbeam::channel::bounded(1);
                std::thread::spawn(move || {
                    let opened =
                        open_preload_track_blocking(request).map_err(|error| error.to_string());
                    let _ = sender.send(opened);
                });
                preload_open_rx = Some(receiver);
            }
        }

        if let Some(receiver) = preload_open_rx.take() {
            match receiver.try_recv() {
                Ok(Ok(opened)) => {
                    let (out_rate_now, out_channels_now) = {
                        let s = state.read();
                        (s.device_sample_rate, s.channels as usize)
                    };
                    let xfade_max_ms = {
                        let s = state.read();
                        s.crossfade_manual_ms.max(s.crossfade_auto_ms).max(0.0)
                    };
                    predecode_cap_frames = predecode_cap_frames_for(
                        xfade_max_ms as f64,
                        out_rate_now,
                        out_channels_now.max(1),
                    );
                    tracing::info!(
                        "Pre-carga iniciada para '{}' ({} Hz, {} canales).",
                        opened.path,
                        opened.sample_rate,
                        out_channels_now
                    );
                    preload_format = Some(opened.format);
                    preload_decoder = Some(opened.decoder);
                    preload_track_id = opened.track_id;
                    preload_sr = opened.sample_rate;
                    preload_total_duration_sec = opened.duration_sec;
                    preload_channel_map = opened.channel_map;
                    preload_path = Some(opened.path);
                    preload_title = Some(opened.title);
                    preload_artist = Some(opened.artist);
                    preload_album = Some(opened.album);
                    preload_cover = Some(opened.cover);
                    preload_rg = opened.replay_gain;
                }
                Ok(Err(error)) => {
                    tracing::error!("Pre-carga falló: {}", error);
                }
                Err(crossbeam::channel::TryRecvError::Empty) => {
                    preload_open_rx = Some(receiver);
                }
                Err(crossbeam::channel::TryRecvError::Disconnected) => {
                    tracing::error!("Pre-carga: el hilo de apertura terminó sin resultado");
                }
            }
        }

        // Target: 100ms of safety margin to absorb CPU spikes at 384kHz
        let target_latency_samples = (out_rate as usize * out_channels * 100) / 1000;

        let should_wait = if let Some(producer) = engine.buffer_producer.lock().as_ref() {
            producer.occupied_len() >= target_latency_samples
        } else {
            false
        };

        if should_wait {
            // === TRAZA DE CROSSFADE: rama should_wait ===
            if crossfade_trace_enabled && tracing::enabled!(tracing::Level::DEBUG) {
                let ringbuf_occ = engine
                    .buffer_producer
                    .lock()
                    .as_ref()
                    .map(|p| p.occupied_len())
                    .unwrap_or(0);
                tracing::debug!(
                    "[XFADE_TRACE]   → should_wait (ringbuf {}/{} >= target {})",
                    ringbuf_occ,
                    engine
                        .buffer_producer
                        .lock()
                        .as_ref()
                        .map(|p| p.capacity().get())
                        .unwrap_or(0),
                    target_latency_samples
                );
            }

            // Tiempo de espera (el ringbuf está al target de latencia): aprovechamos
            // este momento —que es donde el decoder pasa la mayor parte del tiempo—
            // para decodificar la pista pre-cargada en segundo plano. Sin esto, el
            // buffer de pre-decode nunca se llena y ni el crossfade ni la promoción
            // sin cortes pueden funcionar.
            // Primero se abre la pista pendiente (diferido del handler de comandos
            // para no bloquear la reproducción) y luego se decodifica un lote.
            if preload_format.is_some() {
                let _ = preload_decode_batch(
                    &mut preload_format,
                    &mut preload_decoder,
                    &mut preload_track_id,
                    &mut preload_plane_pool,
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
            // Durante la mezcla, mantener la cola de la canción anterior LLENA
            // (~200ms): el decoder pasa la mayor parte del tiempo en esta rama
            // (ringbuf al target de latencia), así que es aquí donde la cola se
            // recarga mientras la primaria consume su buffer en la mezcla. Sin
            // esta recarga la cola se agota y el crossfade se corta (sonido
            // entrecortado, peor a 384 kHz; inaudible en el salto manual).
            let tail_exhausted = if crossfade_active && tail_decoder.is_some() {
                let before = tail_buffer.len();
                let result = tail_decode_batch(
                    &mut tail_format,
                    &mut tail_decoder,
                    &mut tail_track_id,
                    &mut tail_plane_pool,
                    &mut tail_resampler,
                    &mut tail_resampler_rates,
                    &mut tail_resampler_in_buf,
                    &mut tail_input_pool,
                    &mut tail_output_pool,
                    &mut tail_channel_map,
                    &mut tail_buffer,
                    tail_buffer_cap,
                    out_rate,
                    out_channels,
                );
                if crossfade_trace_enabled && tracing::enabled!(tracing::Level::DEBUG) {
                    tracing::debug!(
                        "[XFADE_TRACE]   tail_decode_batch (should_wait): {} → {} samples (result={})",
                        before,
                        tail_buffer.len(),
                        result
                    );
                }
                !result
            } else {
                false
            };
            if tail_exhausted {
                // La canción anterior llegó a EOF: se libera la cadena de
                // decodificación pero se PRESERVA el tail_buffer para que el
                // audio ya decodificado se mezcle hasta agotarse naturalmente.
                clear_tail_decoder_chain(
                    &mut tail_format,
                    &mut tail_decoder,
                    &mut tail_track_id,
                    &mut tail_channel_map,
                    &mut tail_resampler,
                    &mut tail_resampler_rates,
                    &mut tail_resampler_in_buf,
                    &mut tail_plane_pool,
                    &mut tail_input_pool,
                    &mut tail_output_pool,
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

        // Drenar audio pre-cargado (promovido tras EOF/crossfade): se procesa por la
        // cadena normal (gain→DSP→volumen→ringbuf) en esta iteración, sin decodificar
        // la nueva pista hasta que el buffer pre-cargado se haya agotado. El drenado
        // es POR LOTES ACOTADOS (~100ms por iteración) y se ejecuta SOLO cuando el
        // ringbuf tiene espacio (después del chequeo should_wait): si se drenara
        // antes, el audio se descartaría al limpiar el acumulador en la siguiente
        // iteración (causa de los saltos de 2-3s al inicio de la canción).
        let process_preloaded = !preloaded_pending.is_empty();
        if process_preloaded {
            let max_batch = ((out_rate as usize * out_channels) * 100) / 1000;
            let batch = max_batch.max(out_channels).min(preloaded_pending.len());
            output_accumulator.extend(preloaded_pending.drain(..batch));
            // Avanzar la posición de reproducción mientras drena la pre-carga: durante
            // el drenado el decoder NO decodifica (no actualiza current_pos_sec por
            // paquete), así que sin este avance la interfaz se quedaría en 00:00 hasta
            // que el drenado termina y el decoder reanuda (salto de ~8s en la UI).
            if batch > 0 {
                let batch_secs = batch as f64 / (out_rate as usize * out_channels).max(1) as f64;
                let mut s = state.write();
                s.current_pos_sec += batch_secs;
            }
            // NOTA: la liberación forzada de capacidad de preloaded_pending fue
            // eliminada — causaba un corte audible al desalocar decenas de MB justo
            // en la transición DRAIN→DECODE. El buffer se libera naturalmente en la
            // siguiente pre-carga o en Load.
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

        // === TRAZA DE CROSSFADE: decisión de rama ===
        if crossfade_trace_enabled && tracing::enabled!(tracing::Level::DEBUG) {
            tracing::debug!(
                "[XFADE_TRACE]   can_push={} process_preloaded={} → branch: {}",
                can_push,
                process_preloaded,
                if !can_push && !process_preloaded {
                    "IDLE"
                } else if !process_preloaded {
                    "DECODE"
                } else {
                    "DRAIN"
                }
            );
        }

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
                engine.set_playing(false);
                state.write().eof_reached = true;
            }
            // Push custom decoded data. Reintenta hasta encolar el paquete
            // completo; si el ringbuf deja de drenar se abandona tras un límite
            // acotado (mismo criterio que la ruta Symphonia) en lugar de
            // descartar muestras en silencio.
            if !output_accumulator_f32.is_empty() {
                let mut pos = 0;
                let mut stalled = 0;
                while pos < output_accumulator_f32.len() {
                    let mut pushed = 0;
                    if let Some(producer) = producer_mutex.lock().as_mut() {
                        pushed = producer.push_slice(&output_accumulator_f32[pos..]);
                    }
                    if pushed == 0 {
                        stalled += 1;
                        if stalled >= 50 {
                            tracing::warn!(
                                "Push incompleto al ringbuf: se descartan {} muestras sobrantes",
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
                frames_since_metric += (output_accumulator_f32.len() / out_channels.max(1)) as u64;
                if frames_since_metric >= out_rate as u64 {
                    tracing::info!(
                        target: METRICS_TARGET,
                        metric = "audio_frames_processed",
                        frames = frames_since_metric
                    );
                    frames_since_metric = 0;
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
            let dec_opt = current_decoder.as_mut();
            let fmt_opt = current_format.as_mut();

            if let (Some(dec), Some(fmt)) = (dec_opt, fmt_opt) {
                let decode_span = debug_batch_span!("batch_decode");
                let decode_start = decode_span.as_ref().map(|_| Instant::now());
                let _decode_enter = decode_span.as_ref().map(|s| s.enter());
                let mut eof = false;
                let packet = match fmt.next_packet() {
                    Ok(Some(p)) => Some(p),
                    Ok(None) => {
                        eof = true;
                        None
                    }
                    Err(e) => {
                        tracing::warn!("Symphonia decode error (skipping packet): {}", e);
                        continue; // Saltar paquetes corruptos en lugar de detener la canción
                    }
                };

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
                            .and(preload_decoder.as_ref())
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
                            // NOTA: la liberación forzada de capacidad de predecode_buffer
                            // fue eliminada — causaba un corte audible al desalocar hasta
                            // 64MB durante la transición DRAIN→DECODE, bloqueando el decoder
                            // thread (~7s después del inicio de la nueva canción). El buffer
                            // se reutiliza en la siguiente pre-carga y se libera cuando ya no
                            // se necesita.

                            {
                                let mut s = state.write();
                                // `eof_reached = true`: la GUI avanza el índice/playlist.
                                apply_promotion_metadata(
                                    &mut s,
                                    &mut preload_title,
                                    &mut preload_artist,
                                    &mut preload_album,
                                    &mut preload_cover,
                                    &mut preload_path,
                                    preload_rg,
                                    preload_total_duration_sec,
                                    preload_sr,
                                    true,
                                );
                                // is_playing se mantiene true: la nueva pista continúa.
                            }

                            // Promover el resampler de la pre-carga: ya está construido y resamplea la
                            // pista promovida a la tasa de salida (el predecode se generó
                            // con él). Si se descartara, al agotarse el drain se perdería
                            // el residuo de entrada aún sin procesar (~200ms a 384kHz →
                            // salto audible que escala con la diferencia de tasas) y se
                            // reconstruiría el resampler en vivo (micro-gap).
                            resampler = preload_resampler.take();
                            resampler_rates = preload_resampler_rates.take();
                            resampler_in_buf = std::mem::take(&mut preload_resampler_in_buf);
                            decode_plane_pool = std::mem::take(&mut preload_plane_pool);
                            resample_input_pool = std::mem::take(&mut preload_input_pool);
                            resample_output_pool = std::mem::take(&mut preload_output_pool);
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

                            // Limpiar el estado de pre-carga ya consumido (el resampler, su buffer
                            // de entrada y los pools ya fueron promovidos arriba).
                            preload_channel_map = ChannelMap::default();
                            preload_rg = (None, None);
                            preload_pending = None;

                            tracing::info!(
                                "Transición sin cortes: pista pre-cargada promovida ('{}').",
                                state.read().title
                            );
                            continue;
                        }
                    }

                    // Sin pre-carga: comportamiento original (purgar al llegar al EOF).
                    engine.set_playing(false);
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
                        decode_plane_pool.clear(); // Reset buffer layout
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

                let Some(packet) = packet else {
                    continue;
                };
                if tracing::enabled!(tracing::Level::TRACE) {
                    tracing::trace!(
                        "Packet next: ts={}, frames={}",
                        packet.pts.get(),
                        packet.dur.get()
                    );
                }

                if packet.track_id != track_id {
                    continue;
                }

                if let Ok(decoded) = dec.decode(&packet) {
                    let spec = decoded.spec().clone();
                    state.write().current_pos_sec = packet.pts.get() as f64 / spec.rate() as f64;

                    // Copiar a un pool planar f64 reutilizado: sin asignación por paquete.
                    decoded.copy_to_vecs_planar::<f64>(&mut decode_plane_pool);
                    record_batch(
                        &decode_span,
                        decode_start,
                        decoded.frames(),
                        spec.channels().count(),
                    );

                    let (out_rate, out_channels) = {
                        let s = state.read();
                        (s.device_sample_rate, s.channels)
                    };

                    // Resampleo adaptativo: calidad superior vs velocidad según recursos disponibles.
                    // En modo normal: SincInterpolation con oversampling masivo (calidad audiófila).
                    // En low-resource: Linear interpolation rápida con menor overhead de CPU.
                    if spec.rate() != out_rate {
                        let recreate = if let Some(stored) = resampler_rates {
                            stored != (spec.rate(), out_rate, spec.channels().count())
                        } else {
                            true
                        };

                        if recreate {
                            let is_low = crate::utils::is_low_resource();
                            let params = if is_low {
                                // Perfil rápido: Linear + Sinc corto, mínimo overhead de CPU
                                SincInterpolationParameters {
                                    sinc_len: 64,
                                    f_cutoff: Some(0.95),
                                    interpolation: SincInterpolationType::Linear,
                                    oversampling_factor: 64,
                                    window: WindowFunction::BlackmanHarris2,
                                }
                            } else {
                                // Perfil audiófilo: Cubic + Sinc largo, aliasing eliminado
                                SincInterpolationParameters {
                                    sinc_len: 256,
                                    f_cutoff: Some(0.99),
                                    interpolation: SincInterpolationType::Cubic,
                                    oversampling_factor: 256,
                                    window: WindowFunction::BlackmanHarris2,
                                }
                            };
                            let chunk_size = if is_low { 256 } else { 1024 };
                            let src_channels = spec.channels().count();
                            if let Some(ratio) = resample_ratio(spec.rate(), out_rate) {
                                match Async::<f64>::new_sinc(
                                    ratio,
                                    2.0,
                                    &params,
                                    chunk_size,
                                    src_channels,
                                    FixedAsync::Input,
                                ) {
                                    Ok(r) => {
                                        resampler = Some(r);
                                        resampler_rates =
                                            Some((spec.rate(), out_rate, src_channels));
                                        resampler_in_buf = (0..src_channels)
                                            .map(|_| {
                                                std::collections::VecDeque::with_capacity(
                                                    if is_low { 1024 } else { 4096 },
                                                )
                                            })
                                            .collect();
                                        tracing::info!(
                                            "Resampler initialized: {} -> {} ({} mode, 64-bit)",
                                            spec.rate(),
                                            out_rate,
                                            if is_low { "low-resource" } else { "audiophile" }
                                        );
                                    }
                                    Err(e) => {
                                        tracing::error!("Resampler init failed: {}", e);
                                        resampler = None;
                                    }
                                }
                            } else {
                                tracing::warn!("Sin resampleo: tasa de entrada inválida (0).");
                                resampler = None;
                                resampler_rates = None;
                            }
                        }
                    } else {
                        if resampler.is_some() {
                            resampler = None;
                            resampler_in_buf.clear();
                            resampler_rates = None;
                        }
                    }

                    // Durante la mezcla: mantener la cola de la canción anterior LLENA
                    // incluso en la ruta de decode normal (can_push && !process_preloaded).
                    // Esto asegura que la cola se rellene en TODAS las ramas del decoder,
                    // no solo en should_wait e idle.
                    if crossfade_active && tail_decoder.is_some() {
                        let tail_eof = !tail_decode_batch(
                            &mut tail_format,
                            &mut tail_decoder,
                            &mut tail_track_id,
                            &mut tail_plane_pool,
                            &mut tail_resampler,
                            &mut tail_resampler_rates,
                            &mut tail_resampler_in_buf,
                            &mut tail_input_pool,
                            &mut tail_output_pool,
                            &mut tail_channel_map,
                            &mut tail_buffer,
                            tail_buffer_cap,
                            out_rate,
                            out_channels as usize,
                        );
                        if tail_eof {
                            clear_tail_decoder_chain(
                                &mut tail_format,
                                &mut tail_decoder,
                                &mut tail_track_id,
                                &mut tail_channel_map,
                                &mut tail_resampler,
                                &mut tail_resampler_rates,
                                &mut tail_resampler_in_buf,
                                &mut tail_plane_pool,
                                &mut tail_input_pool,
                                &mut tail_output_pool,
                            );
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

                    if let Some(resampler) = resampler.as_mut() {
                        let resample_span = debug_batch_span!("batch_resample");
                        let resample_start = resample_span.as_ref().map(|_| Instant::now());
                        let _resample_enter = resample_span.as_ref().map(|s| s.enter());
                        let mut resampled_frames = 0usize;
                        let src_channels = spec.channels().count();

                        for c in 0..src_channels {
                            if c < resampler_in_buf.len()
                                && let Some(plane) = decode_plane_pool.get(c)
                            {
                                resampler_in_buf[c].extend(plane.iter().copied());
                            }
                        }

                        if resample_input_pool.len() < src_channels {
                            resample_input_pool.resize(src_channels, Vec::new());
                        }
                        if resample_output_pool.len() < src_channels {
                            resample_output_pool.resize(src_channels, Vec::new());
                        }

                        loop {
                            let needed = resampler.input_frames_next();
                            if resampler_in_buf.is_empty() || resampler_in_buf[0].len() < needed {
                                break;
                            }

                            let out_frames = resampler.output_frames_next();

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

                            if resampler
                                .process_into_buffer(&input_adapter, &mut output_adapter, None)
                                .is_ok()
                            {
                                resampled_frames += out_frames;
                                if tracing::enabled!(tracing::Level::TRACE) {
                                    tracing::trace!(
                                        "Resampled: {} -> {} frames",
                                        needed,
                                        out_frames
                                    );
                                }
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
                        record_batch(
                            &resample_span,
                            resample_start,
                            resampled_frames,
                            src_channels,
                        );
                    } else {
                        AudioEngine::mix_channels_planar(
                            &decode_plane_pool,
                            decoded.frames(),
                            spec.channels().count(),
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
            // The preload opener runs on its helper thread at the top of the loop.
            if preload_format.is_some() {
                // Decodificamos un lote por iteración; el loop controla la frecuencia.
                let _ = preload_decode_batch(
                    &mut preload_format,
                    &mut preload_decoder,
                    &mut preload_track_id,
                    &mut preload_plane_pool,
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
            // Durante la mezcla: recargar la COLA de la canción anterior (capa de la
            // mezcla) mientras la primaria no necesita el ringbuf — cada llamada
            // rellena el buffer completo (~200ms) para no interrumpir la mezcla.
            let tail_exhausted = if crossfade_active && tail_decoder.is_some() {
                !tail_decode_batch(
                    &mut tail_format,
                    &mut tail_decoder,
                    &mut tail_track_id,
                    &mut tail_plane_pool,
                    &mut tail_resampler,
                    &mut tail_resampler_rates,
                    &mut tail_resampler_in_buf,
                    &mut tail_input_pool,
                    &mut tail_output_pool,
                    &mut tail_channel_map,
                    &mut tail_buffer,
                    tail_buffer_cap,
                    out_rate,
                    out_channels,
                )
            } else {
                false
            };
            if tail_exhausted {
                // La canción anterior llegó a EOF: se libera la cadena de
                // decodificación pero se PRESERVA el tail_buffer para que el
                // audio ya decodificado se mezcle hasta agotarse naturalmente.
                clear_tail_decoder_chain(
                    &mut tail_format,
                    &mut tail_decoder,
                    &mut tail_track_id,
                    &mut tail_channel_map,
                    &mut tail_resampler,
                    &mut tail_resampler_rates,
                    &mut tail_resampler_in_buf,
                    &mut tail_plane_pool,
                    &mut tail_input_pool,
                    &mut tail_output_pool,
                );
            }
            if crossfade_trace_enabled && tracing::enabled!(tracing::Level::DEBUG) {
                tracing::debug!(
                    "[XFADE_TRACE]   → IDLE branch: tail_buf={}/{} sleep={}ms",
                    tail_buffer.len(),
                    tail_buffer_cap,
                    if crate::utils::is_low_resource() {
                        5
                    } else {
                        2
                    }
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

            // Crossfade automático: cuando la canción actual se acerca a su final y hay una
            // pista pre-cargada disponible, se inicia la mezcla "hacia adelante".
            if !crossfade_active {
                // Leer las condiciones en un bloque propio: el guard de lectura debe
                // SOLTARSE antes del swap (begin_forward_crossfade toma el write lock
                // del estado; un reader vivo aquí es un deadlock seguro — parking_lot
                // da preferencia al writer y la GUI también lee el estado).
                let (auto_enabled, auto_ms, total_duration, cur_pos) = {
                    let s = state.read();
                    (
                        s.crossfade_enabled
                            && s.crossfade_auto_enabled
                            && s.crossfade_auto_ms > 0.0,
                        s.crossfade_auto_ms as f64,
                        s.total_duration_sec,
                        s.current_pos_sec,
                    )
                };
                if auto_enabled
                    && total_duration > 0.0
                    && !predecode_buffer.is_empty()
                    && crossfade_auto_due(total_duration, cur_pos, auto_ms)
                {
                    // La primaria pasa a ser la canción pre-cargada (suena desde 0)
                    // y la actual pasa a la cola. El lote actual en el acumulador
                    // pertenece a la canción que sale: se descarta (el ringbuf aún
                    // contiene su cola audible) y la mezcla arranca en la próxima
                    // iteración con la primaria.
                    begin_forward_crossfade(
                        &mut current_format,
                        &mut current_decoder,
                        &mut track_id,
                        &mut channel_map,
                        &mut resampler,
                        &mut resampler_rates,
                        &mut resampler_in_buf,
                        &mut resample_input_pool,
                        &mut resample_output_pool,
                        &mut decode_plane_pool,
                        &mut tail_format,
                        &mut tail_decoder,
                        &mut tail_track_id,
                        &mut tail_channel_map,
                        &mut tail_resampler,
                        &mut tail_resampler_rates,
                        &mut tail_resampler_in_buf,
                        &mut tail_plane_pool,
                        &mut tail_input_pool,
                        &mut tail_output_pool,
                        &mut tail_buffer,
                        &mut tail_buffer_cap,
                        &mut preload_format,
                        &mut preload_decoder,
                        &mut preload_track_id,
                        &mut preload_sr,
                        &mut preload_total_duration_sec,
                        &mut preload_channel_map,
                        &mut preload_resampler,
                        &mut preload_resampler_rates,
                        &mut preload_resampler_in_buf,
                        &mut preload_plane_pool,
                        &mut preload_input_pool,
                        &mut preload_output_pool,
                        &mut predecode_buffer,
                        &mut preload_path,
                        &mut preload_title,
                        &mut preload_artist,
                        &mut preload_album,
                        &mut preload_cover,
                        &mut preload_rg,
                        &mut preload_pending,
                        &mut preloaded_pending,
                        &mut fade_state,
                        &mut silence_samples,
                        &mut in_silence,
                        &mut track_start_trimmed,
                        &engine,
                        &state,
                        out_rate,
                        out_channels,
                        true, // automático: la GUI avanza a la canción nueva
                    );
                    crossfade_duration_sec = auto_ms / 1000.0;
                    crossfade_elapsed_sec = 0.0;
                    crossfade_active = true;
                    // Inicializar traza de diagnóstico
                    crossfade_trace_enabled = true;
                    crossfade_start_time = Some(std::time::Instant::now());
                    last_trace_time = crossfade_start_time;
                    trace_iteration = 0;
                    if tracing::enabled!(tracing::Level::DEBUG) {
                        tracing::debug!(
                            "[XFADE_TRACE] Crossfade automático iniciado: {} ms",
                            auto_ms
                        );
                    }
                    continue; // Descartar el lote de la canción que sale.
                }
            }

            // Mezcla f64 "hacia adelante": superpone la COLA de la canción anterior
            // sobre la salida de la PRIMARIA (la canción nueva) SIN desvanecimiento
            // de volumen — ambas al nivel del reproductor (los fades de la columna
            // izquierda son independientes). La cola se decodifica en flujo en el
            // tiempo idle y solo se añade lo que haya en su buffer (~200ms); si la
            // canción anterior terminó antes, la mezcla se acorta naturalmente.
            //
            // Cuando los desvanecimientos de volumen están activos durante la mezcla:
            // - fade-in se aplica SOLO a la primaria (canción nueva) ANTES de mezclar
            //   la cola, para que la nueva canción suba de volumen desde 0.
            // - fade-out se aplica SOLO a la cola (canción anterior) DURANTE la mezcla,
            //   para que la canción vieja baje de volumen suavemente.
            // - No se aplica el fade normal al output final (ya se aplicó a cada canción
            //   por separado).

            // Pre-calculamos fade_coeff ANTES de la mezcla para poder aplicarlo
            // a la primaria y a la cola por separado.
            let fade_coeff = match fade_state {
                FadeState::Idle => 1.0,
                FadeState::FadingIn { coeff, .. } => {
                    if coeff >= 1.0 {
                        1.0
                    } else {
                        (std::f64::consts::PI / 2.0 * coeff).sin().powi(2)
                    }
                }
                FadeState::FadingOut { coeff, .. } => {
                    if coeff <= 0.0 {
                        0.0
                    } else {
                        (std::f64::consts::PI / 2.0 * coeff).sin().powi(2)
                    }
                }
            };

            if crossfade_active {
                let out_ch = out_channels;
                if out_ch > 0 {
                    let batch_dt_cf =
                        output_accumulator.len() as f64 / out_ch as f64 / out_rate as f64;

                    // 1) Aplicar fade-in a la PRIMARIA (canción nueva) antes de mezclar
                    //    la cola, para que solo la nueva canción suba de volumen desde 0.
                    //    El fade-out (si está activo) se aplica a la cola en el paso 2.
                    if matches!(fade_state, FadeState::FadingIn { .. }) {
                        for sample in output_accumulator.iter_mut() {
                            *sample *= fade_coeff;
                        }
                    }

                    // 2) Mezclar la cola sobre la primaria. Si el fade-out está activo
                    //    (la canción anterior está terminando), se aplica el fade-out
                    //    a cada sample de la cola para que baje de volumen suavemente.
                    let tail_before_mix = tail_buffer.len();
                    if !tail_buffer.is_empty() {
                        let fade_tail = if matches!(fade_state, FadeState::FadingOut { .. }) {
                            fade_coeff
                        } else {
                            1.0
                        };
                        for frame in output_accumulator.chunks_mut(out_ch) {
                            if tail_buffer.is_empty() {
                                break;
                            }
                            mix_tail_into_frame_faded(frame, &mut tail_buffer, fade_tail);
                        }
                    }
                    let tail_after_mix = tail_buffer.len();
                    let tail_consumed = tail_before_mix.saturating_sub(tail_after_mix);

                    if crossfade_trace_enabled && tracing::enabled!(tracing::Level::DEBUG) {
                        let peak = output_accumulator
                            .iter()
                            .map(|v| v.abs())
                            .fold(0.0_f64, f64::max);
                        tracing::debug!(
                            "[XFADE_TRACE]   MIX: output_acc={} samples ({:.1}ms) | tail {}→{} (consumed {}) | peak={:.3} | elapsed={:.1}/{:.1}s",
                            output_accumulator.len(),
                            batch_dt_cf * 1000.0,
                            tail_before_mix,
                            tail_after_mix,
                            tail_consumed,
                            peak,
                            crossfade_elapsed_sec,
                            crossfade_duration_sec
                        );
                    }

                    crossfade_elapsed_sec += batch_dt_cf;

                    // Log del nivel del ringbuf después de la mezcla.
                    if crossfade_trace_enabled && tracing::enabled!(tracing::Level::DEBUG) {
                        let rb_occ = engine
                            .buffer_producer
                            .lock()
                            .as_ref()
                            .map(|p| p.occupied_len())
                            .unwrap_or(0);
                        tracing::debug!(
                            "[XFADE_TRACE]   POST_MIX: ringbuf={}/{} ({:.1}ms)",
                            rb_occ,
                            engine
                                .buffer_producer
                                .lock()
                                .as_ref()
                                .map(|p| p.capacity().get())
                                .unwrap_or(0),
                            rb_occ as f64 / out_rate as f64 / out_ch as f64 * 1000.0
                        );
                    }

                    // La mezcla termina por ventana (tiempo) o naturalmente
                    // cuando la canción anterior agotó su decoder Y el buffer
                    // ya se mezcló completo (drenado natural, sin corte abrupto).
                    let window_done = crossfade_elapsed_sec >= crossfade_duration_sec;
                    let tail_naturally_done = tail_decoder.is_none() && tail_buffer.is_empty();
                    if window_done || tail_naturally_done {
                        if crossfade_trace_enabled && tracing::enabled!(tracing::Level::DEBUG) {
                            tracing::debug!(
                                "[XFADE_TRACE]   CROSSFADE ENDED: window_done={}, tail_naturally_done={}",
                                window_done,
                                tail_naturally_done
                            );
                        }
                        crossfade_active = false;
                        crossfade_trace_enabled = false;
                        clear_tail_state(
                            &mut tail_format,
                            &mut tail_decoder,
                            &mut tail_track_id,
                            &mut tail_channel_map,
                            &mut tail_resampler,
                            &mut tail_resampler_rates,
                            &mut tail_resampler_in_buf,
                            &mut tail_plane_pool,
                            &mut tail_input_pool,
                            &mut tail_output_pool,
                            &mut tail_buffer,
                            &mut tail_buffer_cap,
                        );
                    }
                }
            }

            // Single loudness gain point: the formula lives in
            // `audio::loudness`; this scope only hands it the snapshot.
            let gain_linear: f64;
            let vol: f64;
            let fades_enabled: bool;
            let smooth_volume_enabled: bool;
            {
                let s = state.read();
                let inputs = super::loudness::LoudnessGainInputs {
                    rg_track: s.replay_gain_track,
                    rg_album: s.replay_gain_album,
                    rg_track_enabled: s.replay_gain_track_enabled,
                    rg_album_enabled: s.replay_gain_album_enabled,
                    rg_master_enabled: s.rg_master_enabled,
                    offset_album_db: smoothed_rg_offset_album_db,
                    offset_track_db: smoothed_rg_offset_track_db,
                    offset_rt_db: smoothed_rg_offset_rt_db,
                    analyze_rt_enabled: s.rg_analyze_rt_enabled,
                    fixed_enabled: s.rg_fixed_enabled,
                    fixed_db: s.rg_fixed_db,
                };
                gain_linear = super::loudness::gain_linear(&inputs);
                vol = s.volume as f64;
                fades_enabled = s.fades_enabled;
                smooth_volume_enabled = s.smooth_volume_enabled;
            } // Release AudioState lock before DSP processing

            // ~100 ms EMA anti-click ramp for RG offsets.
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
            // Volumen y Mezcla: Silence detection + Edge trimming
            // Measure PRE-fade on output_accumulator before DSP/fades.
            // ============================================================
            let current_pos_sec: f64;
            {
                let s = state.read();
                current_pos_sec = s.current_pos_sec;

                // Edge trimming — start: fixed -50dB threshold, no minimum duration.
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

                // Silence detection — main body
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
                        // Silent frame: accumulate and potentially drop.
                        let frame_samples = output_accumulator.len() / out_channels;
                        silence_samples += frame_samples;
                        let silence_duration_ms =
                            (silence_samples as f64 / out_rate as f64) * 1000.0;
                        in_silence = true;

                        if silence_duration_ms >= s.silence_duration_ms as f64 {
                            // Drop this batch entirely (no seeks needed).
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

                // Fade-out trigger: at total_duration - fade_out_ms.
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

            // Volume smoothing: debounce + ramp.
            // While the user adjusts the volume, applied_vol stays frozen.
            // After 400ms of inactivity, a 1500ms linear ramp moves
            // applied_vol from its current level to the new target.
            // Gated by the group master (fades_enabled) AND the individual
            // smooth_volume_enabled flag — disabling the group disables
            // smoothing too.
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

            // Fade envelope coefficient: equal-power for fades,
            // linear for volume smoothing.
            // coeff advances by rate_per_sec * batch_dt — i.e. by REAL audio
            // time. This makes the configured ms exact regardless of sample
            // rate or batch size.
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
                        // Equal-power fade-in: sin²(π/2 * t)
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

            // 1. Aplicar gain → DSP → fades×volume al accumulator.
            let dsp_span = debug_batch_span!("batch_dsp");
            let dsp_start = dsp_span.as_ref().map(|_| Instant::now());
            let _dsp_enter = dsp_span.as_ref().map(|s| s.enter());
            let dsp_frames = output_accumulator.len() / out_channels.max(1);
            {
                let out_ch = out_channels;
                // combined = applied_vol (smoothed or immediate user volume)
                // × fade_coeff (equal-power musical fade, 1.0 when Idle).
                // Durante la mezcla cruzada, el fade ya se aplicó por separado
                // a la primaria (fade-in) y a la cola (fade-out) en el bloque
                // de mezcla anterior, así que no se aplica de nuevo aquí.
                let combined = if crossfade_active {
                    applied_vol
                } else {
                    applied_vol * fade_coeff
                };

                // Spin-wait for DSP lock up to 5ms before falling back.
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
                        // Meter: peak and RMS accumulation (post-DSP, pre-volume).
                        if out_ch >= 2 {
                            let pl = frame[0].abs();
                            let pr = frame[1].abs();
                            if pl > meter_peak_l {
                                meter_peak_l = pl;
                            }
                            if pr > meter_peak_r {
                                meter_peak_r = pr;
                            }
                            rms_sum_l += pl * pl;
                            rms_sum_r += pr * pr;
                        }
                        rms_count_l += 1;
                        rms_count_r += 1;
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
                        // Meter: peak and RMS accumulation even when DSP is bypassed.
                        if out_ch >= 2 {
                            let pl = frame[0].abs();
                            let pr = frame[1].abs();
                            if pl > meter_peak_l {
                                meter_peak_l = pl;
                            }
                            if pr > meter_peak_r {
                                meter_peak_r = pr;
                            }
                            rms_sum_l += pl * pl;
                            rms_sum_r += pr * pr;
                        }
                        rms_count_l += 1;
                        rms_count_r += 1;
                    }
                }

                // Meter: batch-level atomic writes — convert accumulated peak/RMS to dBFS
                // and store to atomics. Peak is reset every batch; RMS accumulates across
                // batches until the 300ms window expires (IEC 60268-17 VU standard).
                {
                    let peak_l_db = if meter_peak_l > 0.0 {
                        (20.0 * meter_peak_l.log10()).clamp(-60.0, 6.0)
                    } else {
                        -60.0
                    };
                    let peak_r_db = if meter_peak_r > 0.0 {
                        (20.0 * meter_peak_r.log10()).clamp(-60.0, 6.0)
                    } else {
                        -60.0
                    };
                    let rms_l_db = if rms_count_l > 0 {
                        let rms_val = (rms_sum_l / rms_count_l as f64).sqrt();
                        if rms_val > 0.0 {
                            (20.0 * rms_val.log10()).clamp(-60.0, 6.0)
                        } else {
                            -60.0
                        }
                    } else {
                        -60.0
                    };
                    let rms_r_db = if rms_count_r > 0 {
                        let rms_val = (rms_sum_r / rms_count_r as f64).sqrt();
                        if rms_val > 0.0 {
                            (20.0 * rms_val.log10()).clamp(-60.0, 6.0)
                        } else {
                            -60.0
                        }
                    } else {
                        -60.0
                    };

                    engine.meter.write_peak_rms(
                        peak_l_db as f32,
                        rms_l_db as f32,
                        peak_r_db as f32,
                        rms_r_db as f32,
                    );
                    meter_peak_l = 0.0;
                    meter_peak_r = 0.0;

                    // RMS 300ms window (IEC 60268-17): reset accumulator when window expires.
                    let now = std::time::Instant::now();
                    if now.duration_since(rms_last_update).as_millis() >= 300 {
                        rms_sum_l = 0.0;
                        rms_count_l = 0;
                        rms_sum_r = 0.0;
                        rms_count_r = 0;
                        rms_last_update = now;
                    }
                }
            }
            record_batch(&dsp_span, dsp_start, dsp_frames, out_channels);

            // Conversión limpia de f64 a f32 (Zero-Allocation pool)
            for &sample in output_accumulator.iter() {
                output_accumulator_f32.push(sample.clamp(-1.0, 1.0) as f32);
            }
            frames_since_metric += (output_accumulator_f32.len() / out_channels.max(1)) as u64;
            if frames_since_metric >= out_rate as u64 {
                tracing::info!(
                    target: METRICS_TARGET,
                    metric = "audio_frames_processed",
                    frames = frames_since_metric
                );
                frames_since_metric = 0;
            }
            if !output_accumulator_f32.is_empty() {
                let push_span = debug_batch_span!("batch_push");
                let push_start = push_span.as_ref().map(|_| Instant::now());
                let _push_enter = push_span.as_ref().map(|s| s.enter());
                let push_frames = output_accumulator_f32.len() / out_channels.max(1);
                // Push al RingBuffer (lock breve por iteración)
                // --- PUSHING ATÓMICO (FRAME ALIGNMENT) ---
                // Aseguramos que solo se envíen múltiplos exactos de `out_channels`.
                let mut pos = 0;
                let mut stalled = 0;
                while pos < output_accumulator_f32.len() {
                    let mut pushed = 0;
                    if let Some(producer) = producer_mutex.lock().as_mut() {
                        let out_ch_usize = out_channels;
                        let remaining = output_accumulator_f32.len() - pos;
                        let available = producer.capacity().get() - producer.occupied_len();

                        let max_frames = available / out_ch_usize;
                        let frames_to_push = (remaining / out_ch_usize).min(max_frames);

                        if frames_to_push > 0 {
                            pushed = producer.push_slice(
                                &output_accumulator_f32[pos..pos + (frames_to_push * out_ch_usize)],
                            );
                        }
                        if pushed > 0 {
                            // Latencia del buffer del ringbuf (µs) publicada como pico
                            // acumulado para que el camino no-RT la convierta en métrica.
                            let occupied = producer.occupied_len();
                            let latency_us = (occupied / out_channels.max(1)) as u64 * 1_000_000
                                / out_rate.max(1) as u64;
                            LATENCY_PEAK_US.fetch_max(latency_us, Ordering::Relaxed);
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
                record_batch(&push_span, push_start, push_frames, out_channels);
            }
        }
    }
}

#[cfg(test)]
#[path = "decoder_tests.rs"]
mod decoder_tests;

#[cfg(test)]
#[path = "decoder_preload_handoff_tests.rs"]
mod preload_handoff_tests;

#[cfg(test)]
#[path = "decoder_preload_buffer_tests.rs"]
mod preload_buffer_tests;

#[cfg(test)]
#[path = "decoder_preload_send_tests.rs"]
mod preload_send_tests;
