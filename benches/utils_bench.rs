// Benchmark de utilidades — formatting, interner, covers.
// Enlaza la librería `audoxidy`.
// Ejecutar: cargo bench --bench utils_bench

use audoxidy::utils::*;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

fn bench_truncate_text(c: &mut Criterion) {
    let long =
        "Una canción con un título extremadamente largo que debería ser truncado correctamente";

    // Elemento = una llamada de truncado por iteración.
    let mut group = c.benchmark_group("truncate_text");
    group.throughput(Throughput::Elements(1));
    group.bench_function("long", |b| {
        b.iter(|| {
            truncate_text(black_box(long), black_box(30));
        });
    });

    group.bench_function("short", |b| {
        b.iter(|| {
            truncate_text(black_box("Corta"), black_box(30));
        });
    });
    group.finish();
}

fn bench_format_duration(c: &mut Criterion) {
    // Elemento = una llamada de formato por iteración.
    let mut group = c.benchmark_group("format_duration");
    group.throughput(Throughput::Elements(4));
    group.bench_function("various", |b| {
        b.iter(|| {
            format_duration(black_box(0.0));
            format_duration(black_box(65.0));
            format_duration(black_box(3661.0));
            format_duration(black_box(90061.0));
        });
    });
    group.finish();
}

fn bench_format_size(c: &mut Criterion) {
    // Elemento = una llamada de formato por iteración.
    let mut group = c.benchmark_group("format_size");
    group.throughput(Throughput::Elements(4));
    group.bench_function("various", |b| {
        b.iter(|| {
            format_size(black_box(0));
            format_size(black_box(1_048_576));
            format_size(black_box(1_073_741_824));
            format_size(black_box(536_870_912));
        });
    });
    group.finish();
}

fn bench_compare_track_numbers(c: &mut Criterion) {
    // Elemento = una comparación por iteración.
    let mut group = c.benchmark_group("compare_track_numbers");
    group.throughput(Throughput::Elements(4));
    group.bench_function("various", |b| {
        b.iter(|| {
            compare_track_numbers(Some("1"), Some("10"));
            compare_track_numbers(Some("A1"), Some("B2"));
            compare_track_numbers(Some("1/10"), Some("2/10"));
            compare_track_numbers(Some(""), None);
        });
    });
    group.finish();
}

fn bench_intelligent_path(c: &mut Criterion) {
    let roots = vec!["/music".to_string(), "/downloads".to_string()];
    let mounts = vec![("/".to_string(), "SSD".to_string())];

    // Elemento = un formateo de ruta inteligente por iteración.
    let mut group = c.benchmark_group("intelligent_path");
    group.throughput(Throughput::Elements(1));
    group.bench_function("deep", |b| {
        b.iter(|| {
            format_intelligent_path(
                black_box("/music/Artists/Q/Queen/A Night at the Opera/01 Bohemian Rhapsody.flac"),
                black_box(&roots),
                black_box(&mounts),
            );
        });
    });
    group.finish();
}

criterion_group!(
    name = utils_grp;
    config = Criterion::default().sample_size(100);
    targets = bench_truncate_text, bench_format_duration, bench_format_size,
              bench_compare_track_numbers, bench_intelligent_path
);
criterion_main!(utils_grp);
