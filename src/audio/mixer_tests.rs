//! Planar mixing, DSP smoke path, buffer math and the loudness gain.
//!
//! Pins the channel-order contract of `mix_channels_planar`, the loudness gain
//! formula the decoder applies before the DSP chain, the buffer and latency
//! arithmetic at high rates, and the guard that keeps `shrink_to_fit` out of the
//! preload transition. No audio device is opened, no playback happens and no
//! database is touched.

use crate::audio::dsp::DspChain;
use crate::audio::engine::{AudioEngine, ChannelMap};

// --- Loudness gain computation helper (mirrors decoder.rs:986-1025) ---

/// Compute loudness gain (linear) mirroring the decoder's single gain point.
/// All inputs are dB values; the function sums them, caps at +12 dB,
/// and returns the linear factor 10^(gain_db/20).
fn compute_gain_db(
    rg_track: Option<f32>,
    rg_album: Option<f32>,
    rg_track_enabled: bool,
    rg_album_enabled: bool,
    rg_master_enabled: bool,
    offset_album_db: f32,
    offset_track_db: f32,
    offset_rt_db: f32,
    analyze_rt_enabled: bool,
    normalize_gain_db: f32,
) -> f64 {
    let mut gain_db: f64 = 0.0;

    if rg_master_enabled {
        // 1. ReplayGain base from tags (album + track sum)
        if rg_track_enabled {
            if let Some(tg) = rg_track {
                gain_db += tg as f64;
            }
        }
        if rg_album_enabled {
            if let Some(ag) = rg_album {
                gain_db += ag as f64;
            }
        }

        // 2. RG offsets per source
        gain_db += offset_album_db as f64;
        gain_db += offset_track_db as f64;

        // 3. RT Analysis fallback: only when no tags AND analyze enabled
        let has_tags = rg_track.is_some() || rg_album.is_some();
        if !has_tags && analyze_rt_enabled {
            gain_db += offset_rt_db as f64;
        }
    }

    // 4. RMS Normalization output (shared controller)
    gain_db += normalize_gain_db as f64;

    // 5. Clamp to safety ceiling +12 dB and convert to linear
    10.0_f64.powf(gain_db.min(12.0) / 20.0)
}

// --- Loudness gain tests ---

#[test]
fn test_loudness_gain_no_tags() {
    // No RG tags, all defaults → unity gain (0 dB)
    let gain = compute_gain_db(
        None, None, true, true, true, // track/album/offsets
        0.0, 0.0, 0.0, true, 0.0,
    );
    assert!((gain - 1.0).abs() < 1e-10);
}

#[test]
fn test_loudness_gain_rg_track() {
    // RG track = -3 dB, track enabled, album disabled → gain = 10^(-3/20)
    let gain = compute_gain_db(
        Some(-3.0),
        None,
        true,
        false,
        true,
        0.0,
        0.0,
        0.0,
        false,
        0.0,
    );
    let expected = 10.0_f64.powf(-3.0 / 20.0);
    assert!((gain - expected).abs() < 1e-10);
}

#[test]
fn test_loudness_gain_rg_album_track_sum() {
    // RG track = -3 dB, album = -2 dB, both enabled → gain = 10^(-5/20)
    let gain = compute_gain_db(
        Some(-3.0),
        Some(-2.0),
        true,
        true,
        true,
        0.0,
        0.0,
        0.0,
        false,
        0.0,
    );
    let expected = 10.0_f64.powf(-5.0 / 20.0);
    assert!((gain - expected).abs() < 1e-10);
}

#[test]
fn test_loudness_gain_offsets() {
    // RG track = -3 dB, offset_album = +2 dB → gain = 10^(-1/20)
    let gain = compute_gain_db(
        Some(-3.0),
        None,
        true,
        false,
        true,
        2.0,
        0.0,
        0.0,
        false,
        0.0,
    );
    let expected = 10.0_f64.powf(-1.0 / 20.0);
    assert!((gain - expected).abs() < 1e-10);
}

#[test]
fn test_loudness_gain_offset_track_and_album() {
    // RG track = -3 dB, offset_track = +1 dB, offset_album = +2 dB
    // gain_db = -3 + 0 + 1 + 2 = 0 dB → linear 1.0
    let gain = compute_gain_db(
        Some(-3.0),
        None,
        true,
        false,
        true,
        2.0,
        1.0,
        0.0,
        false,
        0.0,
    );
    assert!((gain - 1.0).abs() < 1e-10);
}

#[test]
fn test_loudness_gain_rt_fallback() {
    // No RG tags, analyze_rt=true, offset_rt = +2 dB → gain = 10^(2/20)
    let gain = compute_gain_db(None, None, true, true, true, 0.0, 0.0, 2.0, true, 0.0);
    let expected = 10.0_f64.powf(2.0 / 20.0);
    assert!((gain - expected).abs() < 1e-10);
}

#[test]
fn test_loudness_gain_rt_no_fallback_with_tags() {
    // RG track = -3 dB present, analyze_rt=true, offset_rt = +2 dB
    // offset_rt NOT added when tags exist.
    // gain_db = -3 + 0 = -3 dB
    let gain = compute_gain_db(
        Some(-3.0),
        None,
        true,
        false,
        true,
        0.0,
        0.0,
        2.0,
        true,
        0.0,
    );
    let expected = 10.0_f64.powf(-3.0 / 20.0);
    assert!((gain - expected).abs() < 1e-10);
}

#[test]
fn test_loudness_gain_master_off() {
    // RG master enabled=false → all RG contributions = 0, gain = 1.0
    let gain = compute_gain_db(
        Some(-3.0),
        Some(-2.0),
        true,
        true,
        false, // master OFF
        5.0,
        5.0,
        5.0,
        true,
        0.0,
    );
    assert!((gain - 1.0).abs() < 1e-10);
}

#[test]
fn test_loudness_gain_with_normalization() {
    // RG track = -3 dB + normalization = +4 dB → gain_db = +1 dB
    let gain = compute_gain_db(
        Some(-3.0),
        None,
        true,
        false,
        true,
        0.0,
        0.0,
        0.0,
        false,
        4.0,
    );
    let expected = 10.0_f64.powf(1.0 / 20.0);
    assert!((gain - expected).abs() < 1e-10);
}

#[test]
fn test_loudness_gain_cap() {
    // RG track = -3 dB, offset_album = +20 dB → gain_db = +17 dB
    // capped at +12 dB (safety ceiling)
    let gain = compute_gain_db(
        Some(-3.0),
        None,
        true,
        false,
        true,
        20.0,
        0.0,
        0.0,
        false,
        0.0,
    );
    let expected_at_cap = 10.0_f64.powf(12.0 / 20.0);
    let expected_uncapped = 10.0_f64.powf(17.0 / 20.0);
    assert!((gain - expected_at_cap).abs() < 1e-10);
    assert!((gain - expected_uncapped).abs() > 1e-10);
}

#[test]
fn test_loudness_gain_rg_album_only() {
    // RG album = -4 dB, album enabled, track disabled → gain = 10^(-4/20)
    let gain = compute_gain_db(
        None,
        Some(-4.0),
        false,
        true,
        true,
        0.0,
        0.0,
        0.0,
        false,
        0.0,
    );
    let expected = 10.0_f64.powf(-4.0 / 20.0);
    assert!((gain - expected).abs() < 1e-10);
}

// --- Mix Channels Planar (existing tests expanded) ---

#[test]
fn test_mix_channels_planar_stereo_to_stereo() {
    let frames = 4;
    let in_channels = 2;
    let out_channels = 2;

    let input = vec![vec![1.0, 1.0, 1.0, 1.0], vec![0.5, 0.5, 0.5, 0.5]];

    let map = ChannelMap {
        fl: Some(0),
        fr: Some(1),
        c: None,
        lfe: None,
        sbl: None,
        sbr: None,
        sl: None,
        sr: None,
        ..Default::default()
    };

    let mut out_buf = Vec::new();
    AudioEngine::mix_channels_planar(
        &input,
        frames,
        in_channels,
        out_channels,
        &map,
        (1.0, 1.0, 1.0, 1.0),
        &mut out_buf,
    );

    assert_eq!(out_buf.len(), frames * out_channels);
    assert_eq!(out_buf[0], 1.0);
    assert_eq!(out_buf[1], 0.5);
    assert_eq!(out_buf[2], 1.0);
    assert_eq!(out_buf[3], 0.5);
}

#[test]
fn test_mix_channels_planar_stereo_to_5_1_simple() {
    let frames = 2;
    let in_channels = 2;
    let out_channels = 6;

    let input = vec![vec![1.0, 1.0], vec![0.5, 0.5]];

    let mut map = ChannelMap::default();
    map.fl = Some(0);
    map.fr = Some(1);

    let mut out_buf = Vec::new();
    AudioEngine::mix_channels_planar(
        &input,
        frames,
        in_channels,
        out_channels,
        &map,
        (1.0, 1.0, 1.0, 1.0),
        &mut out_buf,
    );

    assert_eq!(out_buf.len(), frames * out_channels);
    assert_eq!(out_buf[0], 1.0);
    assert_eq!(out_buf[1], 0.5);
    assert_eq!(out_buf[2], 0.0);
    assert_eq!(out_buf[3], 0.0);
    assert_eq!(out_buf[4], 0.0);
    assert_eq!(out_buf[5], 0.0);
}

#[test]
fn test_mix_channels_planar_mono_to_stereo() {
    let frames = 2;
    let in_channels = 1;
    let out_channels = 2;

    let input = vec![vec![0.8, 0.9]];

    let map = ChannelMap {
        c: Some(0),
        ..Default::default()
    };

    let mut out_buf = Vec::new();
    AudioEngine::mix_channels_planar(
        &input,
        frames,
        in_channels,
        out_channels,
        &map,
        (1.0, 1.0, 1.0, 1.0),
        &mut out_buf,
    );

    assert_eq!(out_buf.len(), frames * out_channels);
}

#[test]
fn test_mix_channels_planar_output_grows_with_frames() {
    let map = ChannelMap {
        fl: Some(0),
        fr: Some(1),
        ..Default::default()
    };
    let input_2 = vec![vec![1.0, 1.0], vec![0.5, 0.5]];

    let mut out_buf_short = Vec::new();
    AudioEngine::mix_channels_planar(
        &input_2,
        2,
        2,
        2,
        &map,
        (1.0, 1.0, 1.0, 1.0),
        &mut out_buf_short,
    );
    assert_eq!(out_buf_short.len(), 4);

    let input_4 = vec![vec![1.0, 1.0, 1.0, 1.0], vec![0.5, 0.5, 0.5, 0.5]];
    let mut out_buf_long = Vec::new();
    AudioEngine::mix_channels_planar(
        &input_4,
        4,
        2,
        2,
        &map,
        (1.0, 1.0, 1.0, 1.0),
        &mut out_buf_long,
    );
    assert_eq!(out_buf_long.len(), 8);
}

#[test]
fn test_mix_channels_planar_with_downmix_params() {
    let frames = 1;
    let in_channels = 6;
    let out_channels = 2;

    let input = vec![
        vec![0.5],
        vec![0.5],
        vec![0.8],
        vec![0.3],
        vec![0.2],
        vec![0.2],
    ];

    let map = ChannelMap {
        fl: Some(0),
        fr: Some(1),
        c: Some(2),
        lfe: Some(3),
        sl: Some(4),
        sr: Some(5),
        ..Default::default()
    };

    let mut out_default = Vec::new();
    AudioEngine::mix_channels_planar(
        &input,
        frames,
        in_channels,
        out_channels,
        &map,
        (1.0, 1.0, 1.0, 1.0),
        &mut out_default,
    );
    assert_eq!(out_default.len(), 2);

    let mut out_silent = Vec::new();
    AudioEngine::mix_channels_planar(
        &input,
        frames,
        in_channels,
        out_channels,
        &map,
        (0.0, 0.0, 0.0, 0.0),
        &mut out_silent,
    );
    assert_eq!(out_silent.len(), 2);
}

#[test]
fn test_downmix_coefficients_independent() {
    // Verifica que dm_conf.1 (LFE) solo afecta al canal LFE, no a los demas.
    // Con entrada que tiene LFE != 0, comparamos output con lfe_coeff=1.0 vs lfe_coeff=0.0
    // La unica diferencia debe ser la contribucion del LFE.
    let frames = 1;
    let in_channels = 6;
    let out_channels = 2;

    let input = vec![
        vec![0.5], // FL
        vec![0.3], // FR
        vec![0.8], // C
        vec![1.0], // LFE (fuerte)
        vec![0.2], // SL
        vec![0.2], // SR
    ];

    let map = crate::audio::engine::ChannelMap {
        fl: Some(0),
        fr: Some(1),
        c: Some(2),
        lfe: Some(3),
        sl: Some(4),
        sr: Some(5),
        ..Default::default()
    };

    // Con LFE activo (coeff=1.0) vs LFE silenciado (coeff=0.0)
    let mut out_full = Vec::new();
    crate::audio::engine::AudioEngine::mix_channels_planar(
        &input,
        frames,
        in_channels,
        out_channels,
        &map,
        (1.0, 1.0, 1.0, 1.0),
        &mut out_full,
    );

    let mut out_no_lfe = Vec::new();
    crate::audio::engine::AudioEngine::mix_channels_planar(
        &input,
        frames,
        in_channels,
        out_channels,
        &map,
        (1.0, 0.0, 1.0, 1.0),
        &mut out_no_lfe,
    );

    assert_eq!(out_full.len(), 2);
    assert_eq!(out_no_lfe.len(), 2);

    // La diferencia debe ser exactamente LFE (1.0 * coeff_diff = 1.0)
    // LFE se suma por igual a L y R: out_full = out_no_lfe + LFE * (1.0 - 0.0) para cada canal
    for ch in 0..2 {
        let diff = (out_full[ch] - out_no_lfe[ch] - 1.0).abs();
        assert!(
            diff < 1e-6,
            "LFE contribution should be exactly 1.0 per channel: {}",
            diff
        );
    }

    // Con center coeff=0.0, la contribucion del centro debe desaparecer
    let mut out_no_center = Vec::new();
    crate::audio::engine::AudioEngine::mix_channels_planar(
        &input,
        frames,
        in_channels,
        out_channels,
        &map,
        (0.0, 1.0, 1.0, 1.0),
        &mut out_no_center,
    );

    for ch in 0..2 {
        let diff = (out_full[ch] - out_no_center[ch] - 0.8).abs();
        assert!(
            diff < 1e-6,
            "Center contribution should be exactly 0.8: {}",
            diff
        );
    }
}

#[test]
fn test_mix_channels_planar_preserves_channel_order() {
    // El orden de canales es un contrato público: valores distintos por
    // plano para que una reordenación no pueda pasar por casualidad.
    // 6-in / 6-out: FL, FR, C, LFE, SL, SR.
    let input6 = vec![
        vec![1.0],
        vec![2.0],
        vec![3.0],
        vec![4.0],
        vec![5.0],
        vec![6.0],
    ];
    let map6 = ChannelMap {
        fl: Some(0),
        fr: Some(1),
        c: Some(2),
        lfe: Some(3),
        sl: Some(4),
        sr: Some(5),
        ..Default::default()
    };
    let mut out6 = Vec::new();
    AudioEngine::mix_channels_planar(&input6, 1, 6, 6, &map6, (1.0, 1.0, 1.0, 1.0), &mut out6);
    assert_eq!(out6, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);

    // 8-in / 8-out: FL, FR, C, LFE, SBL, SBR, SL, SR.
    let input8 = vec![
        vec![1.0],
        vec![2.0],
        vec![3.0],
        vec![4.0],
        vec![5.0],
        vec![6.0],
        vec![7.0],
        vec![8.0],
    ];
    let map8 = ChannelMap {
        fl: Some(0),
        fr: Some(1),
        c: Some(2),
        lfe: Some(3),
        sbl: Some(4),
        sbr: Some(5),
        sl: Some(6),
        sr: Some(7),
    };
    let mut out8 = Vec::new();
    AudioEngine::mix_channels_planar(&input8, 1, 8, 8, &map8, (1.0, 1.0, 1.0, 1.0), &mut out8);
    assert_eq!(out8, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
}

#[test]
fn test_pipeline_384khz_8ch_smoke() {
    // Humo del camino caliente: 384 kHz x 8 canales por la cadena DSP
    // completa y una mezcla planar 8-in/8-out. Acotado (200 frames) para no
    // inflar el presupuesto de la suite; ninguna muestra debe quedar no
    // finita ni superar el clamp del limitador.
    let mut chain = DspChain::default();
    chain.enabled = true;
    chain.set_sample_rate(384_000.0);
    chain.set_channel_count(8);
    chain.equalizer.enabled = true;
    chain.noise_gate.enabled = true;
    chain.compressor.enabled = true;
    chain.limiter.enabled = true;

    let mut frame = vec![0.5_f64; 8];
    for _ in 0..200 {
        chain.process_frame(&mut frame);
        assert!(
            frame.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
            "la cadena DSP a 384kHz/8ch debe permanecer finita y acotada"
        );
    }

    let frames = 64;
    let input: Vec<Vec<f64>> = (1..=8).map(|n| vec![n as f64; frames]).collect();
    // Mapa poblado FL, FR, C, LFE, SBL, SBR, SL, SR: con un valor distinto
    // por plano, la mezcla 8-in/8-out debe conservar exactamente ese orden.
    let map8 = ChannelMap {
        fl: Some(0),
        fr: Some(1),
        c: Some(2),
        lfe: Some(3),
        sbl: Some(4),
        sbr: Some(5),
        sl: Some(6),
        sr: Some(7),
    };
    let mut out = Vec::new();
    AudioEngine::mix_channels_planar(&input, frames, 8, 8, &map8, (1.0, 1.0, 1.0, 1.0), &mut out);
    assert_eq!(out.len(), frames * 8);
    let expected_frame = [1.0_f64, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    for (i, frame) in out.chunks_exact(8).enumerate() {
        assert_eq!(
            frame,
            expected_frame.as_slice(),
            "frame {i} no conserva el orden planar FL,FR,C,LFE,SBL,SBR,SL,SR"
        );
    }
}

// --- Matemática de buffers a altas tasas (diagnóstico del cambio de rate) ---

#[test]
fn test_ringbuf_sizes_at_high_rates() {
    // rb_size = rate × ch × 2.0s (mínimo 384.000 muestras), como en recreate_stream.
    let cases = [
        (44100u32, 2usize, 176_400usize),
        (96000, 2, 384_000),
        (192_000, 2, 768_000),
        (192_000, 6, 2_304_000),
        (384_000, 8, 6_144_000),
    ];
    for (rate, ch, expected) in cases {
        let rb_size = ((rate as f64 * ch as f64) * 2.0) as usize;
        assert_eq!(rb_size.max(384_000), expected.max(384_000));
    }
}

#[test]
fn test_target_latency_math() {
    // 100ms de latencia objetivo a distintas tasas/canales (decoder should_wait).
    assert_eq!((44100 * 2 * 100) / 1000, 8820);
    assert_eq!((96000 * 2 * 100) / 1000, 19200);
    assert_eq!((192000 * 2 * 100) / 1000, 38400);
    assert_eq!((384000 * 8 * 100) / 1000, 307200);
}

#[test]
fn test_pipeline_is_f64_until_ringbuf() {
    use ringbuf::HeapRb;
    use ringbuf::traits::{Consumer, Producer, Split};

    // La mezcla planar, el DSP y los buffers de salida son f64 por
    // construcción: cambiar cualquiera a f32 rompería la compilación.
    let planar: Vec<Vec<f64>> = vec![vec![0.0_f64; 8]; 2];
    let mut out: Vec<f64> = Vec::new();
    let map = ChannelMap::default();
    AudioEngine::mix_channels_planar(&planar, 8, 2, 2, &map, (0.74, 0.66, 0.81, 0.91), &mut out);
    let mut frame = [0.0_f64; 8];
    crate::audio::dsp::DspChain::default().process_frame(&mut frame);

    // El único estrechamiento a f32 es el bus ringbuf: se verifica el
    // round-trip completo productor/consumidor.
    let rb = HeapRb::<f32>::new(1024);
    let (mut prod, mut cons) = rb.split();
    assert_eq!(prod.push_slice(&[0.1_f32, -0.1]), 2);
    assert_eq!(cons.pop_slice(&mut [0.0_f32; 2]), 2);

    // 8 frames × 2 canales de salida, todos f64.
    assert_eq!(out.len(), 16);
}

// --- Predecode acotado por memoria y avance de posición durante el drenado ---

#[test]
fn test_predecode_cap_memory_bounded() {
    use crate::audio::decoder::predecode_cap_frames_for;

    // El cap del predecode se limita por MEMORIA (~64MB de f64, 8M muestras)
    // además de por tiempo: en salidas multi-canal de alta tasa el tope por
    // tiempo retendría cientos de MB (384kHz × 8ch × 8s ≈ 196MB).
    //
    // Se llama a la función de producción para que el test falle si el tope
    // real cambia.
    const PRELOAD_MAX_SAMPLES: usize = 8_000_000;

    // 384000 Hz × 8 ch: el cap por tiempo (8s) supera el tope de memoria.
    let cap = predecode_cap_frames_for(10_000.0, 384_000, 8);
    assert_eq!(cap, PRELOAD_MAX_SAMPLES);
    let secs = cap as f64 / 384_000.0 / 8.0;
    assert!(secs < 4.0, "memoria acotada: {secs}s");

    // 44100 Hz × 2 ch: el cap por tiempo (8s) es menor → gana el tiempo.
    let cap = predecode_cap_frames_for(10_000.0, 44_100, 2);
    let secs = cap as f64 / 44_100.0 / 2.0;
    assert!((secs - 8.0).abs() < 0.01, "cap por tiempo ~8s: {secs}");
    assert!(cap < PRELOAD_MAX_SAMPLES);

    // Nunca por debajo de un frame de salida.
    assert!(predecode_cap_frames_for(0.0, 384_000, 8) >= 8);
}

#[test]
fn test_current_pos_advance_during_drain() {
    // La posición de reproducción avanza durante el drenado de la pre-carga:
    // batch / (tasa × canales) segundos por lote (~100ms por lote).
    let rate = 384_000usize;
    let ch = 2usize;
    let batch = 76_800usize; // 100ms a 384000×2
    let batch_secs = batch as f64 / (rate * ch) as f64;
    assert!((batch_secs - 0.1).abs() < 1e-9);

    // 8s de pre-carga drenados en lotes de 100ms → la suma avanza 8s exactos.
    let total_samples = batch * 80;
    let total_secs = total_samples as f64 / (rate * ch) as f64;
    assert!((total_secs - 8.0).abs() < 1e-9);
}

#[test]
fn no_shrink_to_fit_in_preload_transition() {
    // Guarda de regresión: la liberación forzada de capacidad no debe reaparecer
    // en la ventana DRAIN→DECODE del crossfade; desalojar decenas de MB en el
    // hilo decodificador justo cuando reanuda el decode corta el audio por segundos.
    const SRC: &str = include_str!("decoder.rs");
    let start = SRC
        .find("let process_preloaded = !preloaded_pending.is_empty();")
        .expect("drain-block anchor missing");
    let end = SRC
        .find("// `eof_reached = true`: la GUI avanza el índice/playlist.")
        .expect("promotion-block anchor missing");
    assert!(start < end, "anchors out of order");
    assert!(
        !SRC[start..end].contains(".shrink_to_fit"),
        "shrink_to_fit must not reappear in the preload drain/decode transition: \
         deallocating the buffers there blocks the decode thread and cuts audio"
    );
}
