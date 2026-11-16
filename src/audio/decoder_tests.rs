//! Decoder tests for the crossfade helpers, the preload promotion, the golden sample
//! images and the memory-mapped source.
//!
//! Every fixture is a 16-bit or 24-bit PCM WAV written by the shared writer into a
//! private temporary directory, so no test opens an output device, touches the
//! application database, or leaves a file behind. The tests reach decoder internals
//! through the parent module rather than through a public seam, because the decoder
//! keeps those items private on purpose.

use super::{
    AudioDecoder, ManualPreloadSource, SymphoniaDecoder, apply_promotion_metadata,
    crossfade_auto_due, derive_duration_sec, manual_preload_source, mix_tail_into_frame,
    predecode_cap_frames_for, tail_buffer_cap_for, tail_decode_batch,
};
use crate::audio::engine::{AudioEngine, AudioState, ChannelMap};
use crate::utils::test_support::{TempTestDir, write_pcm_wav};
use std::collections::VecDeque;
use symphonia::core::codecs::audio::{AudioDecoder as SymphoniaAudioDecoder, AudioDecoderOptions};
use symphonia::core::common::Limit;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, Track, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Duration, TimeBase};

/// Escribe un WAV PCM de DC constante con 16 o 24 bits por muestra.
///
/// La muestra codificada se conoce de antemano, así que sirve de fixture
/// golden: el decodificador debe devolver su imagen f64 exacta.
fn write_pcm_wav_code(path: &str, rate: u32, channels: u16, seconds: f64, code: i32, bits: u16) {
    let bytes_per_sample = (bits / 8) as usize;
    let frames = (rate as f64 * seconds) as u32;
    let block_align = channels as u32 * bytes_per_sample as u32;
    let data_len = frames * block_align;
    let mut wav: Vec<u8> = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * block_align).to_le_bytes());
    wav.extend_from_slice(&(block_align as u16).to_le_bytes());
    wav.extend_from_slice(&bits.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    let b = code.to_le_bytes();
    for _ in 0..(frames * channels as u32) {
        wav.extend_from_slice(&b[..bytes_per_sample]);
    }
    std::fs::write(path, wav).unwrap();
}

/// Abre un WAV con symphonia y devuelve la cadena completa (formato,
/// decoder, id de pista y mapa de canales) lista para `tail_decode_batch`.
fn open_wav_chain(
    path: &str,
) -> (
    Option<Box<dyn FormatReader>>,
    Option<Box<dyn SymphoniaAudioDecoder>>,
    u32,
    ChannelMap,
) {
    let source = super::open_audio_source(path).unwrap();
    let mss = MediaSourceStream::new(source, Default::default());
    let hint = Hint::new();
    let metadata_opts = MetadataOptions::default()
        .limit_tag_bytes(Limit::Maximum(0))
        .limit_visual_bytes(Limit::Maximum(0));
    let format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), metadata_opts)
        .unwrap();
    let track = format.default_track(TrackType::Audio).unwrap();
    let id = track.id;
    let audio_params = track.codec_params.as_ref().and_then(|p| p.audio()).unwrap();
    let decoder = symphonia::default::get_codecs()
        .make_audio_decoder(audio_params, &AudioDecoderOptions::default())
        .unwrap();
    let ch_map = if let Some(channels) = audio_params.channels.as_ref() {
        AudioEngine::get_channel_map(channels.clone())
    } else {
        ChannelMap::default()
    };
    (Some(format), Some(decoder), id, ch_map)
}

#[test]
fn crossfade_auto_due_fires_in_time() {
    // 9s antes del final con mezcla de 9000ms → dispara.
    assert!(crossfade_auto_due(180.0, 171.0, 9000.0));
    // Justo en el umbral → dispara.
    assert!(crossfade_auto_due(180.0, 171.0, 9000.0));
    // Aún lejos → no dispara.
    assert!(!crossfade_auto_due(180.0, 100.0, 9000.0));
}

#[test]
fn crossfade_auto_due_ignores_disabled_conditions() {
    // Duración desconocida o mezcla de 0ms → nunca dispara.
    assert!(!crossfade_auto_due(0.0, 0.0, 9000.0));
    assert!(!crossfade_auto_due(180.0, 171.0, 0.0));
}

#[test]
fn predecode_cap_covers_crossfade_at_normal_configs() {
    // Mezcla de 10s (máx permitido) + margen, acotada a 8s de audio.
    let cap = predecode_cap_frames_for(10000.0, 44100, 2);
    let secs = cap as f64 / 44100.0 / 2.0;
    assert!(
        (secs - 8.0).abs() < 0.01,
        "cap por tiempo acotado a ~8s: {secs}"
    );
    let cap = predecode_cap_frames_for(1000.0, 44100, 2);
    let secs = cap as f64 / 44100.0 / 2.0;
    assert!(
        (secs - 3.0).abs() < 0.01,
        "mezcla 1s + 2s de margen: {secs}"
    );
}

#[test]
fn predecode_cap_bounded_by_memory_at_extreme_configs() {
    // 384kHz × 8 ch con mezcla máxima: el tope de ~8M muestras (64MB f64) acota
    // el adelanto y el decoder cubre el resto en vivo.
    let cap = predecode_cap_frames_for(10000.0, 384000, 8);
    assert_eq!(cap, 8_000_000);
    let cap = predecode_cap_frames_for(10000.0, 384000, 8);
    let secs = cap as f64 / 384000.0 / 8.0;
    assert!(secs < 4.0, "memoria acotada: {secs}s");
    // Nunca por debajo de un frame de salida.
    assert!(predecode_cap_frames_for(0.0, 384000, 8) >= 8);
}

#[test]
fn tail_buffer_cap_is_200ms() {
    let cap = tail_buffer_cap_for(44100, 2);
    let secs = cap as f64 / 44100.0 / 2.0;
    assert!((secs - 0.2).abs() < 0.01, "cola de ~200ms: {secs}");
    let cap = tail_buffer_cap_for(384000, 8);
    let secs = cap as f64 / 384000.0 / 8.0;
    assert!((secs - 0.2).abs() < 0.01, "cola de ~200ms a 384k×8: {secs}");
}

#[test]
fn mix_tail_is_additive_without_volume_loss() {
    // Ambas canciones a pleno nivel: la suma simple no baja el volumen.
    let mut frame = [0.5, 0.5];
    let mut tail = VecDeque::from([0.5, 0.5]);
    mix_tail_into_frame(&mut frame, &mut tail);
    assert!((frame[0] - 1.0).abs() < 1e-9);
    assert!((frame[1] - 1.0).abs() < 1e-9);
    assert!(tail.is_empty());
}

#[test]
fn mix_tail_scales_peak_only_when_needed() {
    // Suma que excede 1.0: la mezcla es puramente aditiva (sin escalado).
    // El escalado de pico se eliminó para no reducir el volumen de la
    // primaria — el DSP (limitador) y el clamp f32 se encargan.
    let mut frame = [0.9, 0.9];
    let mut tail = VecDeque::from([0.8, 0.5]);
    mix_tail_into_frame(&mut frame, &mut tail);
    assert!(
        (frame[0] - 1.7).abs() < 1e-9,
        "mezcla aditiva: {}",
        frame[0]
    );
    assert!(
        (frame[1] - 1.4).abs() < 1e-9,
        "mezcla aditiva: {}",
        frame[1]
    );
    let peak = frame.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
    assert!(peak > 1.0, "el pico puede exceder 1.0: {peak}");
}

#[test]
fn mix_tail_stops_when_tail_exhausted() {
    // La canción anterior terminó antes de la ventana: la mezcla se acorta y el
    // resto del frame se conserva sin modificar.
    let mut frame = [0.3, 0.4, 0.5];
    let mut tail = VecDeque::from([0.1]);
    mix_tail_into_frame(&mut frame, &mut tail);
    assert!((frame[0] - 0.4).abs() < 1e-9);
    assert!((frame[1] - 0.4).abs() < 1e-9);
    assert!((frame[2] - 0.5).abs() < 1e-9);
}

/// Monta la cadena de cola con un WAV temporal y devuelve los parámetros
/// junto con el buffer de cola, listos para `tail_decode_batch`.
fn setup_tail_chain(
    path: &str,
) -> (
    Option<Box<dyn FormatReader>>,
    Option<Box<dyn SymphoniaAudioDecoder>>,
    u32,
    Vec<Vec<f64>>,
    Option<rubato::Async<f64>>,
    Option<(u32, u32, usize)>,
    Vec<VecDeque<f64>>,
    Vec<Vec<f64>>,
    Vec<Vec<f64>>,
    ChannelMap,
    VecDeque<f64>,
) {
    let (fmt, dec, track_id, ch_map) = open_wav_chain(path);
    (
        fmt,
        dec,
        track_id,
        Vec::new(),
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        ch_map,
        VecDeque::new(),
    )
}

#[test]
fn tail_decode_batch_fills_buffer_in_single_call() {
    // Un WAV de 2s produce paquetes de ~1152 frames (~26ms): UNA llamada a
    // tail_decode_batch debe llenar la capacidad completa (~200ms). Antes
    // solo se decodificaba un paquete por llamada → la cola se agotaba en
    // 2-4 iteraciones del decoder (sonido entrecortado, peor a 384 kHz).
    let dir = TempTestDir::new("tail-fill");
    let path = dir.path().join("audoxidy_tail_fill.wav");
    write_pcm_wav(&path, 44100, 2, 2.0);
    let (
        mut fmt,
        mut dec,
        mut track_id,
        mut decode_plane_pool,
        mut resampler,
        mut resampler_rates,
        mut resampler_in_buf,
        mut input_pool,
        mut output_pool,
        mut ch_map,
        mut tail,
    ) = setup_tail_chain(path.to_str().unwrap());
    let cap = tail_buffer_cap_for(44100, 2);

    let ok = tail_decode_batch(
        &mut fmt,
        &mut dec,
        &mut track_id,
        &mut decode_plane_pool,
        &mut resampler,
        &mut resampler_rates,
        &mut resampler_in_buf,
        &mut input_pool,
        &mut output_pool,
        &mut ch_map,
        &mut tail,
        cap,
        44100,
        2,
    );

    assert!(ok, "quedan ~1.8s de audio por decodificar");
    assert!(
        tail.len() >= cap,
        "una sola llamada debe llenar al menos la capacidad (~200ms): got {}",
        tail.len()
    );

    // Consumo simulado de la mezcla (mitad del buffer) y recarga: la
    // siguiente llamada rellena SOLO el déficit hasta la capacidad.
    tail.drain(..(cap / 2));
    let ok2 = tail_decode_batch(
        &mut fmt,
        &mut dec,
        &mut track_id,
        &mut decode_plane_pool,
        &mut resampler,
        &mut resampler_rates,
        &mut resampler_in_buf,
        &mut input_pool,
        &mut output_pool,
        &mut ch_map,
        &mut tail,
        cap,
        44100,
        2,
    );
    assert!(ok2);
    assert!(
        tail.len() >= cap,
        "recarga al menos hasta la capacidad tras el consumo: got {}",
        tail.len()
    );
}

#[test]
fn tail_decode_batch_stops_at_eof_before_cap() {
    // WAV más corto que la capacidad (~200ms): el EOF llega antes de llenar
    // el buffer → la función reporta false y la mezcla se acorta
    // naturalmente (la primaria continúa sola con lo ya mezclado).
    let dir = TempTestDir::new("tail-eof");
    let path = dir.path().join("audoxidy_tail_eof.wav");
    write_pcm_wav(&path, 44100, 2, 0.05); // 50ms
    let (
        mut fmt,
        mut dec,
        mut track_id,
        mut decode_plane_pool,
        mut resampler,
        mut resampler_rates,
        mut resampler_in_buf,
        mut input_pool,
        mut output_pool,
        mut ch_map,
        mut tail,
    ) = setup_tail_chain(path.to_str().unwrap());
    let cap = tail_buffer_cap_for(44100, 2);

    let ok = tail_decode_batch(
        &mut fmt,
        &mut dec,
        &mut track_id,
        &mut decode_plane_pool,
        &mut resampler,
        &mut resampler_rates,
        &mut resampler_in_buf,
        &mut input_pool,
        &mut output_pool,
        &mut ch_map,
        &mut tail,
        cap,
        44100,
        2,
    );

    assert!(!ok, "el EOF de la canción anterior acorta la mezcla");
    let expected = (0.05 * 44100.0) as usize * 2;
    assert_eq!(tail.len(), expected);
    assert!(tail.len() < cap);

    // El EOF persiste: una llamada posterior también lo reporta.
    let ok2 = tail_decode_batch(
        &mut fmt,
        &mut dec,
        &mut track_id,
        &mut decode_plane_pool,
        &mut resampler,
        &mut resampler_rates,
        &mut resampler_in_buf,
        &mut input_pool,
        &mut output_pool,
        &mut ch_map,
        &mut tail,
        cap,
        44100,
        2,
    );
    assert!(!ok2);
}

#[test]
fn promotion_writes_all_metadata() {
    // Al promover la pre-carga, TODOS los metadatos deben volcarse en el
    // AudioState y las opciones pendientes quedar consumidas: así los datos
    // de la canción anterior no pueden resurgir en una promoción posterior.
    let mut state = AudioState::default();
    let mut title = Some("Nueva canción".to_string());
    let mut artist = Some("Nueva artista".to_string());
    let mut album = Some("Nuevo álbum".to_string());
    let mut cover = Some(Some("/tmp/nueva.avif".to_string()));
    let mut path = Some("/tmp/nueva.flac".to_string());

    apply_promotion_metadata(
        &mut state,
        &mut title,
        &mut artist,
        &mut album,
        &mut cover,
        &mut path,
        (Some(-3.0), Some(-2.0)),
        180.0,
        48000,
        true,
    );

    assert_eq!(state.title, "Nueva canción");
    assert_eq!(state.artist, "Nueva artista");
    assert_eq!(state.album, "Nuevo álbum");
    assert_eq!(state.cover_path.as_deref(), Some("/tmp/nueva.avif"));
    assert_eq!(state.path, "/tmp/nueva.flac");
    assert_eq!(state.replay_gain_track, Some(-3.0));
    assert_eq!(state.replay_gain_album, Some(-2.0));
    assert!((state.total_duration_sec - 180.0).abs() < 1e-9);
    assert_eq!(state.sample_rate, 48000);
    assert!(state.current_pos_sec.abs() < 1e-9);
    assert!(state.eof_reached);

    assert!(title.is_none(), "title pendiente sin consumir");
    assert!(artist.is_none(), "artist pendiente sin consumir");
    assert!(album.is_none(), "album pendiente sin consumir");
    assert!(cover.is_none(), "cover pendiente sin consumir");
    assert!(path.is_none(), "path pendiente sin consumir");
}

#[test]
fn promotion_eof_flag_is_preserved() {
    // El crossfade promueve con `signal_eof = false` (la GUI ya avanzó en el
    // cambio manual) y el fin de pista natural con `true`; el helper debe
    // escribir exactamente el valor recibido.
    let mut crossfade_state = AudioState::default();
    let (mut t, mut ar, mut al, mut cv, mut p) = (None, None, None, None, None);
    apply_promotion_metadata(
        &mut crossfade_state,
        &mut t,
        &mut ar,
        &mut al,
        &mut cv,
        &mut p,
        (None, None),
        0.0,
        0,
        false,
    );
    assert!(!crossfade_state.eof_reached);

    let mut eof_state = AudioState::default();
    let (mut t2, mut ar2, mut al2, mut cv2, mut p2) = (None, None, None, None, None);
    apply_promotion_metadata(
        &mut eof_state,
        &mut t2,
        &mut ar2,
        &mut al2,
        &mut cv2,
        &mut p2,
        (None, None),
        0.0,
        0,
        true,
    );
    assert!(eof_state.eof_reached);
}

#[test]
fn symphonia_open_succeeds_on_generated_wav() {
    // Ruta de éxito del probe: un WAV PCM generado abre por el decodificador
    // real y expone la tasa y los canales del stream.
    let dir = TempTestDir::new("open-ok");
    let path = dir.path().join("audoxidy_open_ok.wav");
    write_pcm_wav(&path, 44100, 2, 0.25);

    let mut decoder = SymphoniaDecoder::new();
    let info = decoder
        .open(path.to_str().unwrap())
        .expect("un WAV PCM generado debe abrirse");

    assert_eq!(info.sample_rate, 44100);
    assert_eq!(info.channels, 2);
    assert_eq!(info.channel_count, 2);
}

#[test]
fn symphonia_open_rejects_unprobeable_input() {
    // Ruta de fallo del probe: bytes que no son audio devuelven el error
    // nombrado en vez de silencio o un pánico.
    let dir = TempTestDir::new("open-bad");
    let path = dir.path().join("audoxidy_open_bad.bin");
    std::fs::write(&path, b"bytes que no son un archivo de audio reconocible").unwrap();

    let mut decoder = SymphoniaDecoder::new();
    let err = decoder
        .open(path.to_str().unwrap())
        .expect_err("bytes no-audio deben rechazarse");
    assert!(
        matches!(err, crate::audio::AudioError::UnsupportedFormat(_)),
        "se esperaba UnsupportedFormat, se obtuvo {err:?}"
    );
}

#[test]
fn test_golden_decode_wav_16bit_matches_samples() {
    // Decodificación golden: un WAV 16-bit de DC constante 16384 se decodifica
    // frame-for-frame a su imagen f64 exacta (16384 / 2^15) y expone la tasa,
    // los canales y la duración declarados por el contenedor.
    let dir = TempTestDir::new("golden16");
    let path = dir.path().join("audoxidy_golden16.wav");
    let path_str = path.to_str().unwrap();
    write_pcm_wav_code(path_str, 44100, 2, 0.25, 16384, 16);

    let mut decoder = SymphoniaDecoder::new();
    let info = decoder
        .open(path_str)
        .expect("un WAV 16-bit de DC constante debe abrirse");
    assert_eq!(info.sample_rate, 44100);
    assert_eq!(info.channels, 2);
    assert!(
        (info.total_duration_sec - 0.25).abs() < 1.0 / 44100.0,
        "duración declarada: {}",
        info.total_duration_sec
    );

    let packet = decoder
        .decode_next()
        .expect("decode_next no debe fallar")
        .expect("el WAV contiene al menos un paquete");
    let expected = 16384.0 / 32_768.0;
    assert!(!packet.data.is_empty(), "el paquete no debe estar vacío");
    for (i, sample) in packet.data.iter().enumerate() {
        assert_eq!(*sample, expected, "muestra {i} fuera de la imagen exacta");
    }
}

#[test]
fn test_golden_decode_wav_24bit_precision() {
    // Decodificación golden 24-bit: el código 1 se conserva como 1 / 2^23.
    // El divisor es una potencia de dos, así que la igualdad es exacta; que
    // ese valor quede por debajo del LSB de 16 bits prueba que no hubo
    // conversión descendente.
    let dir = TempTestDir::new("golden24");
    let path = dir.path().join("audoxidy_golden24.wav");
    let path_str = path.to_str().unwrap();
    write_pcm_wav_code(path_str, 44100, 2, 0.05, 1, 24);

    let mut decoder = SymphoniaDecoder::new();
    let info = decoder
        .open(path_str)
        .expect("un WAV 24-bit de DC constante debe abrirse");
    assert_eq!(info.sample_rate, 44100);
    assert_eq!(info.channels, 2);

    let packet = decoder
        .decode_next()
        .expect("decode_next no debe fallar")
        .expect("el WAV contiene al menos un paquete");
    let lsb24 = 1.0 / 8_388_608.0;
    let lsb16 = 1.0 / 32_768.0;
    assert!(!packet.data.is_empty(), "el paquete no debe estar vacío");
    for (i, sample) in packet.data.iter().enumerate() {
        assert_eq!(*sample, lsb24, "muestra {i} no conserva los 24 bits");
    }
    // La evidencia se deriva de la señal decodificada: su magnitud debe ser
    // estrictamente positiva y más fina que el LSB de 16 bits. Una conversión
    // descendente a 16 bits aplanaría este código a 0, y ningún paso de 16
    // bits puede representar un valor tan pequeño.
    let decoded_min = packet
        .data
        .iter()
        .fold(f64::INFINITY, |min, s| min.min(s.abs()));
    assert!(
        decoded_min > 0.0,
        "el código 1 de 24 bits no debe aplanarse a cero"
    );
    assert!(
        decoded_min < lsb16,
        "la resolución de 24 bits ({decoded_min}) debe quedar por debajo del LSB de 16 bits ({lsb16})"
    );
}

#[test]
fn test_derive_duration_sec_prefers_time_base() {
    // La duración declarada por el contenedor (time_base + duration) gana
    // sobre el conteo de frames.
    let mut track = Track::new(0);
    track.time_base = TimeBase::try_from_recip(48_000);
    track.duration = Some(Duration::new(96_000));

    let dur = derive_duration_sec(&track, 48_000).expect("time_base utilizable");
    assert!((dur - 2.0).abs() < 1e-9, "duración del contenedor: {dur}");
}

#[test]
fn test_derive_duration_sec_falls_back_to_num_frames() {
    // Sin contenedor que declare duración, se usa num_frames / sample_rate.
    let mut track = Track::new(0);
    track.num_frames = Some(96_000);

    assert_eq!(derive_duration_sec(&track, 48_000), Some(2.0));
}

#[test]
fn test_derive_duration_sec_returns_none_when_unusable() {
    // Sin time_base, sin duración y sin frames no hay fuente utilizable.
    let track = Track::new(0);
    assert_eq!(derive_duration_sec(&track, 48_000), None);
}

/// La fuente mmap libera las páginas ya leídas (`MADV_DONTNEED`) sin perder
/// los datos: una lectura completa y una relectura tras `seek` deben devolver
/// exactamente el contenido del archivo aunque las páginas se hayan descartado.
#[cfg(unix)]
#[test]
fn mmap_source_releases_pages_and_stays_readable() {
    use std::io::{Read, Seek, SeekFrom};

    let dir = TempTestDir::new("mmap-release");
    let path = dir.path().join("audoxidy_mmap_release.wav");
    // 60s estéreo 44.1k a 16 bits ≈ 10 MB: supera `MEMMAP_THRESHOLD`.
    write_pcm_wav(&path, 44100, 2, 60.0);
    let original = std::fs::read(&path).unwrap();

    let mut source = super::open_audio_source(path.to_str().unwrap()).unwrap();
    assert!(
        original.len() as u64 > super::MEMMAP_THRESHOLD,
        "el fixture debe superar el umbral de mmap"
    );

    // Lectura completa en trozos pequeños (cada uno descarta sus páginas).
    let mut read_back = Vec::with_capacity(original.len());
    let mut buf = vec![0u8; 4096];
    loop {
        let n = source.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        read_back.extend_from_slice(&buf[..n]);
    }
    assert_eq!(read_back, original, "la lectura completa debe ser exacta");

    // Relectura tras un seek hacia atrás: las páginas descartadas se
    // repueblan desde el archivo y el contenido debe seguir siendo correcto.
    source.seek(SeekFrom::Start(0)).unwrap();
    let mut head = vec![0u8; 8192];
    source.read_exact(&mut head).unwrap();
    assert_eq!(
        head,
        original[..8192],
        "relectura tras seek debe ser exacta"
    );
}

/// A manual crossfade must wait for an in-flight preload open instead of
/// dropping the mix. The queued and in-flight states are distinct: the idle
/// branch takes a queued request and starts the helper thread, so by the time
/// the crossfade command arrives the request can be in either state.
#[test]
fn test_manual_crossfade_waits_for_an_in_flight_preload() {
    assert_eq!(
        manual_preload_source(false, 0, false, true),
        ManualPreloadSource::InFlight
    );
    assert_eq!(
        manual_preload_source(false, 0, true, false),
        ManualPreloadSource::Queued
    );
    assert_eq!(
        manual_preload_source(false, 0, true, true),
        ManualPreloadSource::Queued,
        "a queued request is opened synchronously and wins over an older open"
    );
    assert_eq!(
        manual_preload_source(true, 0, false, false),
        ManualPreloadSource::Ready
    );
    assert_eq!(
        manual_preload_source(false, 1024, false, false),
        ManualPreloadSource::Ready,
        "a decoded head is enough to start the mix"
    );
    assert_eq!(
        manual_preload_source(false, 0, false, false),
        ManualPreloadSource::Unavailable
    );
}
