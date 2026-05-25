// Benchmark de utilidades — formatting, interner, covers.
// Ejecutar: cargo bench --bench utils_bench

use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[path = "../src/utils/mod.rs"]
mod utils;

use utils::*;

fn bench_truncate_text(c: &mut Criterion) {
    let long = "Una canción con un título extremadamente largo que debería ser truncado correctamente";

    c.bench_function("truncate_text_long", |b| {
        b.iter(|| {
            truncate_text(black_box(long), black_box(30));
        });
    });

    c.bench_function("truncate_text_short", |b| {
        b.iter(|| {
            truncate_text(black_box("Corta"), black_box(30));
        });
    });
}

fn bench_format_duration(c: &mut Criterion) {
    c.bench_function("format_duration_various", |b| {
        b.iter(|| {
            format_duration(black_box(0.0));
            format_duration(black_box(65.0));
            format_duration(black_box(3661.0));
            format_duration(black_box(90061.0));
        });
    });
}

fn bench_format_size(c: &mut Criterion) {
    c.bench_function("format_size_various", |b| {
        b.iter(|| {
            format_size(black_box(0));
            format_size(black_box(1_048_576));
            format_size(black_box(1_073_741_824));
            format_size(black_box(536_870_912));
        });
    });
}

fn bench_compare_track_numbers(c: &mut Criterion) {
    c.bench_function("compare_track_numbers_various", |b| {
        b.iter(|| {
            compare_track_numbers(Some("1"), Some("10"));
            compare_track_numbers(Some("A1"), Some("B2"));
            compare_track_numbers(Some("1/10"), Some("2/10"));
            compare_track_numbers(Some(""), None);
        });
    });
}

fn bench_intelligent_path(c: &mut Criterion) {
    let roots = vec!["/music".to_string(), "/downloads".to_string()];
    let mounts = vec![("/".to_string(), "SSD".to_string())];

    c.bench_function("intelligent_path_deep", |b| {
        b.iter(|| {
            format_intelligent_path(
                black_box("/music/Artists/Q/Queen/A Night at the Opera/01 Bohemian Rhapsody.flac"),
                black_box(&roots),
                black_box(&mounts),
            );
        });
    });
}

criterion_group!(
    name = utils_grp;
    config = Criterion::default().sample_size(100);
    targets = bench_truncate_text, bench_format_duration, bench_format_size,
              bench_compare_track_numbers, bench_intelligent_path
);
criterion_main!(utils_grp);
