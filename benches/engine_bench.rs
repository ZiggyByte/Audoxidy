// Benchmark del motor de audio — mix_channels_planar, ChannelMap, state ops.
// Enlaza la librería `audoxidy`.
// Ejecutar: cargo bench --bench engine_bench

use audoxidy::audio::engine::{AudioEngine, AudioState, ChannelMap};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

fn bench_mix_channels_planar_stereo(c: &mut Criterion) {
    let frames = 512;
    let in_ch = 2;
    let out_ch = 2;
    let input = vec![vec![1.0f64; frames], vec![0.5f64; frames]];
    let map = ChannelMap {
        fl: Some(0),
        fr: Some(1),
        ..Default::default()
    };

    // Elemento = una muestra de salida (frames * canales de salida) por iteración.
    let mut group = c.benchmark_group("mix_planar");
    group.throughput(Throughput::Elements((frames * out_ch) as u64));
    group.bench_function("stereo_to_stereo_512frames", |b| {
        b.iter(|| {
            let mut out = Vec::with_capacity(frames * out_ch);
            AudioEngine::mix_channels_planar(
                black_box(&input),
                black_box(frames),
                black_box(in_ch),
                black_box(out_ch),
                black_box(&map),
                black_box((1.0, 1.0, 1.0, 1.0)),
                &mut out,
            );
        });
    });
    group.finish();
}

fn bench_mix_channels_planar_51_to_stereo(c: &mut Criterion) {
    let frames = 512;
    let in_ch = 6;
    let out_ch = 2;
    let input = vec![
        vec![1.0f64; frames],
        vec![0.8f64; frames],
        vec![0.5f64; frames],
        vec![0.3f64; frames],
        vec![0.2f64; frames],
        vec![0.2f64; frames],
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

    // Elemento = una muestra de salida (frames * canales de salida) por iteración.
    let mut group = c.benchmark_group("mix_planar");
    group.throughput(Throughput::Elements((frames * out_ch) as u64));
    group.bench_function("51_to_stereo_512frames", |b| {
        b.iter(|| {
            let mut out = Vec::with_capacity(frames * out_ch);
            AudioEngine::mix_channels_planar(
                black_box(&input),
                black_box(frames),
                black_box(in_ch),
                black_box(out_ch),
                black_box(&map),
                black_box((0.7071, 0.6666, 0.7671, 0.8071)),
                &mut out,
            );
        });
    });
    group.finish();
}

fn bench_channel_map_construction(c: &mut Criterion) {
    use symphonia::core::audio::Channels;

    // Simular varios formatos multicanal
    let configs = [
        Channels::FRONT_LEFT | Channels::FRONT_RIGHT,
        Channels::FRONT_LEFT | Channels::FRONT_RIGHT | Channels::FRONT_CENTRE | Channels::LFE1,
        Channels::FRONT_LEFT
            | Channels::FRONT_RIGHT
            | Channels::FRONT_CENTRE
            | Channels::LFE1
            | Channels::REAR_LEFT
            | Channels::REAR_RIGHT,
        Channels::FRONT_LEFT
            | Channels::FRONT_RIGHT
            | Channels::FRONT_CENTRE
            | Channels::LFE1
            | Channels::REAR_LEFT
            | Channels::REAR_RIGHT
            | Channels::SIDE_LEFT
            | Channels::SIDE_RIGHT,
    ];

    // Elemento = una configuración de canales mapeada por iteración.
    let mut group = c.benchmark_group("channel_map_construction");
    group.throughput(Throughput::Elements(4));
    group.bench_function("4configs", |b| {
        b.iter(|| {
            for cfg in &configs {
                let _ = AudioEngine::get_channel_map(*cfg);
            }
        });
    });
    group.finish();
}

fn bench_audio_state_read(c: &mut Criterion) {
    use parking_lot::RwLock;
    use std::sync::Arc;

    let state = Arc::new(RwLock::new(AudioState::default()));
    state.write().volume = 0.8;
    state.write().is_playing = true;

    c.bench_function("audio_state_read", |b| {
        b.iter(|| {
            let s = state.read();
            black_box(s.is_playing);
            black_box(s.volume);
            black_box(s.sample_rate);
        });
    });
}

criterion_group!(
    name = engine;
    config = Criterion::default().sample_size(100);
    targets = bench_mix_channels_planar_stereo, bench_mix_channels_planar_51_to_stereo,
              bench_channel_map_construction, bench_audio_state_read
);
criterion_main!(engine);
