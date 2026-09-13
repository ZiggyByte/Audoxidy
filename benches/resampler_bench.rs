//! Comparación de caudal de CPU entre el resampler sinc actual y el candidato FFT
//! de rubato. Cada resampler se construye una vez y procesa chunks de un seno
//! estéreo de 1 kHz, reportando el caudal en frames de entrada por segundo.
//!
//! Ejecutar: `cargo bench --bench resampler_bench -j 2`

use audioadapter_buffers::direct::SequentialSliceOfVecs;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use rubato::{
    Async, Fft, FixedAsync, FixedSync, Resampler, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
};
use std::hint::black_box;

/// Tamaño de chunk en frames, idéntico para ambos resamplers.
const CHUNK: usize = 1024;
/// Canales del frame medido.
const STEREO: usize = 2;
/// Capacidad máxima del buffer de salida reutilizado entre iteraciones.
const OUT_CAP: usize = 4096;
/// Tono del seno de entrada, en Hz.
const TONE_HZ: f64 = 1000.0;

/// Pares de tasas medidos: `(entrada, salida, etiqueta)`.
const RATE_PAIRS: [(u32, u32, &str); 3] = [
    (48000, 44100, "48k_to_44k1"),
    (44100, 48000, "44k1_to_48k"),
    (96000, 48000, "96k_to_48k"),
];

/// Parámetros del perfil audiófilo, los mismos que usa el decodificador.
fn audiophile_sinc_params() -> SincInterpolationParameters {
    SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: Some(0.99),
        interpolation: SincInterpolationType::Cubic,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    }
}

fn make_sinc(in_rate: u32, out_rate: u32) -> Box<dyn Resampler<f64>> {
    Box::new(
        Async::<f64>::new_sinc(
            out_rate as f64 / in_rate as f64,
            2.0,
            &audiophile_sinc_params(),
            CHUNK,
            STEREO,
            FixedAsync::Input,
        )
        .expect("new_sinc debe construirse"),
    )
}

fn make_fft(in_rate: u32, out_rate: u32) -> Box<dyn Resampler<f64>> {
    Box::new(
        Fft::<f64>::new(
            in_rate as usize,
            out_rate as usize,
            CHUNK,
            STEREO,
            FixedSync::Input,
        )
        .expect("Fft debe construirse"),
    )
}

/// Seno continuo de `CHUNK` frames a la tasa de entrada, precalculado una vez para
/// que el llenado del buffer no entre en la región medida.
fn sine_input(in_rate: u32) -> Vec<Vec<f64>> {
    let channel: Vec<f64> = (0..CHUNK)
        .map(|frame| {
            0.5 * (2.0 * std::f64::consts::PI * TONE_HZ * frame as f64 / in_rate as f64).sin()
        })
        .collect();
    vec![channel; STEREO]
}

/// Mide un mismo `make` a lo largo de todos los pares de tasas bajo un nombre de
/// grupo que distingue el tipo de resampler en la salida de criterion.
fn bench_kind(c: &mut Criterion, group_name: &str, make: fn(u32, u32) -> Box<dyn Resampler<f64>>) {
    let mut group = c.benchmark_group(group_name);
    group.throughput(Throughput::Elements(CHUNK as u64));
    for &(in_rate, out_rate, label) in &RATE_PAIRS {
        let mut resampler = make(in_rate, out_rate);
        let input = sine_input(in_rate);
        let mut output = vec![vec![0.0_f64; OUT_CAP]; STEREO];
        group.bench_function(label, |b| {
            b.iter(|| {
                let needed = resampler.input_frames_next();
                let out_frames = resampler.output_frames_next();
                let in_adapt = SequentialSliceOfVecs::new(&input, STEREO, needed).unwrap();
                let mut out_adapt =
                    SequentialSliceOfVecs::new_mut(&mut output, STEREO, out_frames).unwrap();
                resampler
                    .process_into_buffer(&in_adapt, &mut out_adapt, None)
                    .expect("process_into_buffer debe tener éxito");
                black_box(&output);
            });
        });
    }
    group.finish();
}

fn bench_resampler_sinc(c: &mut Criterion) {
    bench_kind(c, "resampler_sinc", make_sinc);
}

fn bench_resampler_fft(c: &mut Criterion) {
    bench_kind(c, "resampler_fft", make_fft);
}

criterion_group!(
    name = resampler;
    config = Criterion::default();
    targets = bench_resampler_sinc, bench_resampler_fft
);
criterion_main!(resampler);
