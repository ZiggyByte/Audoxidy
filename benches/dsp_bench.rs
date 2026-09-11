// Benchmark de la cadena DSP de Audoxidy.
// Enlaza la librería `audoxidy` para medir el DSP que se publica.
// Ejecutar: cargo bench --bench dsp_bench

use audoxidy::audio::dsp::*;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

fn bench_dsp_chain_full(c: &mut Criterion) {
    let mut chain = DspChain::default();
    chain.equalizer.enabled = true;
    chain.reverb.enabled = true;
    chain.compressor.enabled = true;
    chain.limiter.enabled = true;
    chain.stereo_expander.enabled = true;
    chain.noise_gate.enabled = true;
    chain.preamp_gain = 1.2;

    // Frame 5.1 (6 canales) para medir carga real
    let mut frame = [0.5f64, -0.3f64, 0.1f64, -0.7f64, 0.2f64, 0.0f64];

    // Elemento = una muestra del frame (6 canales) procesada por iteración.
    let mut group = c.benchmark_group("dsp_chain_51_full");
    group.throughput(Throughput::Elements(6));
    group.bench_function("full", |b| {
        b.iter(|| {
            chain.process_frame(black_box(&mut frame));
        });
    });
    group.finish();
}

fn bench_dsp_chain_disabled(c: &mut Criterion) {
    let mut chain = DspChain::default();
    chain.enabled = false;

    let mut frame = [0.5f64, -0.3f64];

    // Elemento = una muestra del frame (estéreo) procesada por iteración.
    let mut group = c.benchmark_group("dsp_chain_disabled_bypass");
    group.throughput(Throughput::Elements(2));
    group.bench_function("bypass", |b| {
        b.iter(|| {
            chain.process_frame(black_box(&mut frame));
        });
    });
    group.finish();
}

fn bench_equalizer_31band_stereo(c: &mut Criterion) {
    let mut eq = Equalizer::new(31);
    eq.enabled = true;
    // Activar algunas bandas para evitar bypass completo
    eq.bands[0].set_gain(2.0);
    eq.bands[10].set_gain(-3.0);
    eq.bands[20].set_gain(1.5);
    eq.bands[30].set_gain(-1.0);

    let mut frame = [0.5f64, -0.3f64];

    // Elemento = una muestra del frame (estéreo) procesada por iteración.
    let mut group = c.benchmark_group("eq_31band_stereo");
    group.throughput(Throughput::Elements(2));
    group.bench_function("31band", |b| {
        b.iter(|| {
            eq.process_frame(black_box(&mut frame));
        });
    });
    group.finish();
}

fn bench_biquad_simd_vs_scalar(c: &mut Criterion) {
    let mut filter = BiquadFilter::new(BiquadFilterType::Peak, 1000.0, 3.0, 1.41);

    let mut frame_2ch = [0.5f64, -0.3f64];
    let mut frame_4ch = [0.5f64, -0.3f64, 0.1f64, -0.7f64];

    // Elemento = una muestra del frame procesada por iteración.
    let mut group = c.benchmark_group("biquad_peak");
    group.throughput(Throughput::Elements(2));
    group.bench_function("stereo", |b| {
        b.iter(|| {
            filter.process_frame(black_box(&mut frame_2ch));
        });
    });
    group.throughput(Throughput::Elements(4));
    group.bench_function("4ch_simd_candidate", |b| {
        b.iter(|| {
            filter.process_frame(black_box(&mut frame_4ch));
        });
    });
    group.finish();
}

fn bench_reverb_processing(c: &mut Criterion) {
    let mut reverb = Reverb::new();
    reverb.enabled = true;
    let mut frame = [0.5f64, -0.3f64];

    // Elemento = una muestra del frame (estéreo) procesada por iteración.
    let mut group = c.benchmark_group("reverb_stereo_frame");
    group.throughput(Throughput::Elements(2));
    group.bench_function("stereo", |b| {
        b.iter(|| {
            reverb.process(black_box(&mut frame));
        });
    });
    group.finish();
}

fn bench_compressor_processing(c: &mut Criterion) {
    let mut comp = Compressor::new();
    comp.enabled = true;
    // Señal por encima del threshold para activar reducción
    let mut frame = [0.5f64, -0.3f64];

    // Elemento = una muestra del frame (estéreo) procesada por iteración.
    let mut group = c.benchmark_group("compressor_above_threshold");
    group.throughput(Throughput::Elements(2));
    group.bench_function("above_threshold", |b| {
        b.iter(|| {
            comp.process(black_box(&mut frame));
        });
    });
    group.finish();
}

fn bench_stereo_expander(c: &mut Criterion) {
    let mut expander = MultiBandStereoExpander::default();
    expander.enabled = true;
    expander.width = 1.5;
    let mut frame = [0.5f64, -0.3f64];

    // Elemento = una muestra del frame (estéreo) procesada por iteración.
    let mut group = c.benchmark_group("stereo_expander_width_150");
    group.throughput(Throughput::Elements(2));
    group.bench_function("width_150", |b| {
        b.iter(|| {
            expander.process(black_box(&mut frame));
        });
    });
    group.finish();
}

criterion_group!(
    name = dsp;
    config = Criterion::default().sample_size(100).warm_up_time(std::time::Duration::from_millis(500));
    targets = bench_dsp_chain_full, bench_dsp_chain_disabled, bench_equalizer_31band_stereo,
              bench_biquad_simd_vs_scalar, bench_reverb_processing,
              bench_compressor_processing, bench_stereo_expander
);
criterion_main!(dsp);
