//! Cover ledger, bucket and decode-funnel tests.
//!
//! The ledger assertions pin the retained-byte accounting against the budget the
//! cover cache clamps to: insertion promotes, a peek does not, eviction takes the
//! least recently used entry and a window protects what the viewport shows. The
//! decode tests write their own PNG into a private directory and read it back
//! through the funnel that would otherwise open a stored AVIF, so no test decodes
//! a real cover from the cache.

use super::*;
use crate::utils::test_support::TempTestDir;

/// Bytes one ledger entry retains in these tests, so an assertion reads as a
/// multiple of a single entry.
const ENTRY_BYTES: usize = 100;

/// Bucket every ledger entry in these tests is recorded under.
const BUCKET: u16 = 160;

/// Inserts `count` entries `k0..k{count-1}` into the ledger, oldest first.
fn fill_ledger(budget: &mut CoverBudget, count: usize) {
    for index in 0..count {
        budget.insert(&format!("k{index}"), BUCKET, ENTRY_BYTES);
    }
}

/// Keys the ledger holds, most recently used first.
fn held_keys(budget: &CoverBudget) -> Vec<String> {
    budget.order_mru_to_lru()
}

/// Explicit contract for the ledger's recency: insertion promotes, a peek does
/// not, and eviction takes the least recently used entry.
#[test]
fn covercache_golden_lru_order() {
    let mut budget = CoverBudget::default();
    budget.insert("a", BUCKET, ENTRY_BYTES);
    budget.insert("b", BUCKET, ENTRY_BYTES);
    budget.insert("c", BUCKET, ENTRY_BYTES);
    assert_eq!(held_keys(&budget).join(","), "c@160,b@160,a@160");

    assert!(budget.touch("a", BUCKET));
    assert_eq!(held_keys(&budget).join(","), "a@160,c@160,b@160");

    // A peek reports the retained bytes without altering recency.
    assert_eq!(budget.get("b", BUCKET), Some(ENTRY_BYTES));
    assert_eq!(held_keys(&budget).join(","), "a@160,c@160,b@160");

    // One entry short of the retained bytes: the least recent one goes, not
    // the one just read.
    budget.set_budget(2 * ENTRY_BYTES);
    assert_eq!(
        budget.evict_to_budget(),
        vec![("b".to_string(), BUCKET)],
        "el desalojo toma la entrada menos reciente"
    );
    assert_eq!(held_keys(&budget).join(","), "a@160,c@160");
    assert_eq!(budget.get("b", BUCKET), None);
    assert_eq!(budget.bytes(), 2 * ENTRY_BYTES);
}

#[test]
fn covercache_capacity_bound_parity() {
    for target in [4 * ENTRY_BYTES, 16 * ENTRY_BYTES, 64 * ENTRY_BYTES] {
        let mut budget = CoverBudget::default();
        fill_ledger(&mut budget, target / ENTRY_BYTES + 5);

        assert!(
            budget.bytes() > target,
            "el escenario debe empezar por encima del presupuesto"
        );

        budget.set_budget(target);
        let removed = budget.evict_to_budget();

        assert!(
            !removed.is_empty(),
            "un presupuesto excedido debe desalojar"
        );
        assert!(
            budget.bytes() <= budget.budget(),
            "los bytes retenidos no pueden superar el presupuesto"
        );
    }
}

#[test]
fn covercache_recency_eviction_order_parity() {
    for count in [3usize, 4, 16, 64] {
        let mut budget = CoverBudget::default();
        fill_ledger(&mut budget, count);
        let oldest = "k0".to_string();
        let next_oldest = "k1".to_string();

        // Touching the oldest entry promotes it; the next-oldest is then the
        // one an insert has to evict.
        assert!(budget.touch(&oldest, BUCKET));
        budget.insert("nueva", BUCKET, ENTRY_BYTES);

        budget.set_budget(count * ENTRY_BYTES);
        let removed = budget.evict_to_budget();

        assert_eq!(
            removed,
            vec![(next_oldest.clone(), BUCKET)],
            "la entrada recién tocada no es la candidata al desalojo"
        );
        assert!(budget.get(&oldest, BUCKET).is_some());
        assert!(budget.get(&next_oldest, BUCKET).is_none());
        assert!(budget.get("nueva", BUCKET).is_some());
    }
}

#[test]
fn covercache_get_promotes_but_peek_does_not() {
    let mut budget = CoverBudget::default();
    fill_ledger(&mut budget, 3);
    let oldest = "k0".to_string();
    let before = held_keys(&budget);

    // The read path peeks: it reports the retained bytes and keeps the order.
    assert_eq!(budget.get(&oldest, BUCKET), Some(ENTRY_BYTES));
    assert_eq!(held_keys(&budget), before, "una consulta no promueve");

    // An explicit touch moves the entry to most recently used.
    assert!(budget.touch(&oldest, BUCKET));
    assert_eq!(
        held_keys(&budget).first(),
        Some(&format!("{oldest}@{BUCKET}")),
        "un toque promueve la entrada"
    );
    assert_ne!(held_keys(&budget), before);
}

/// Concurrency stress test under a single `Mutex`: eight threads insert,
/// touch and evict, and the ledger never exceeds its budget or panics.
fn concurrency_stress(ledger: &Mutex<CoverBudget>, budget_bytes: usize) {
    std::thread::scope(|scope| {
        for thread in 0..8usize {
            scope.spawn(move || {
                for index in 0..200usize {
                    let path = format!("/music/album/{thread}/{index}");
                    let mut guard = ledger.lock();
                    guard.insert(&path, BUCKET, ENTRY_BYTES);
                    guard.touch(&path, BUCKET);
                    let _ = guard.get("/music/album/ausente", BUCKET);
                    guard.evict_to_budget();
                    assert!(
                        guard.bytes() <= budget_bytes,
                        "el registro excedió su presupuesto"
                    );
                }
            });
        }
    });

    assert!(ledger.lock().bytes() <= budget_bytes);
}

#[test]
fn covercache_concurrency_parity() {
    let budget_bytes = 8 * ENTRY_BYTES;
    let mut ledger = CoverBudget::default();
    ledger.set_budget(budget_bytes);

    concurrency_stress(&Mutex::new(ledger), budget_bytes);
}

#[test]
fn covercache_clear_parity() {
    let mut budget = CoverBudget::default();
    fill_ledger(&mut budget, 5);
    assert!(!budget.is_empty());
    assert_eq!(budget.bytes(), 5 * ENTRY_BYTES);

    budget.clear();

    assert!(budget.is_empty());
    assert_eq!(budget.len(), 0);
    assert_eq!(budget.bytes(), 0);
    assert!(held_keys(&budget).is_empty());
}

#[test]
fn covercache_negative_cache_clear_is_isolated() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let corrupt = "cache/covers/undecodable.avif";
    record_negative_cover(
        Path::new(corrupt),
        &CoverDecodeError::Decode(PathBuf::from(corrupt)),
    );
    assert!(
        is_cover_negative(corrupt),
        "el archivo indescodificable debe quedar registrado"
    );

    let mut budget = CoverBudget::default();
    fill_ledger(&mut budget, 3);
    let bytes_before = budget.bytes();
    let keys_before = held_keys(&budget);

    clear_negative_cover_cache();

    assert!(
        !is_cover_negative(corrupt),
        "el conjunto negativo debe quedar vacío"
    );
    // Clearing the negative set is independent of the byte ledger.
    assert_eq!(budget.bytes(), bytes_before);
    assert_eq!(held_keys(&budget), keys_before);

    clear_negative_cover_cache();
}

#[test]
fn covercache_purge_parity() {
    for protected in [0usize, 2] {
        let mut budget = CoverBudget::default();
        fill_ledger(&mut budget, 6);

        let window: Vec<(String, u16)> = (0..protected)
            .map(|index| (format!("k{index}"), BUCKET))
            .collect();
        budget.set_window(window);

        let removed = budget.trim_outside_window();

        assert_eq!(
            removed.len(),
            6 - protected,
            "se descarta toda entrada fuera de la ventana"
        );
        for index in 0..protected {
            assert!(
                budget.get(&format!("k{index}"), BUCKET).is_some(),
                "una entrada de la ventana se conserva"
            );
        }
        for index in protected..6 {
            assert!(
                budget.get(&format!("k{index}"), BUCKET).is_none(),
                "una entrada fuera de la ventana no sobrevive"
            );
        }
        assert_eq!(budget.bytes(), protected * ENTRY_BYTES);
    }
}

#[test]
fn covercache_resize_parity() {
    for start in [16usize, 64] {
        let mut budget = CoverBudget::default();
        fill_ledger(&mut budget, start);
        let newest = format!("k{}", start - 1);

        // Shrinking the budget evicts the least recently used entries first.
        let target = start / 4;
        budget.set_budget(target * ENTRY_BYTES);
        let removed = budget.evict_to_budget();

        assert_eq!(removed.len(), start - target);
        assert!(
            budget.get(&newest, BUCKET).is_some(),
            "la entrada más reciente sobrevive"
        );
        assert!(
            budget.get("k0", BUCKET).is_none(),
            "la entrada menos reciente se desaloja primero"
        );
        assert_eq!(budget.bytes(), target * ENTRY_BYTES);
        let retained = budget.len();

        // Growing it evicts nothing.
        budget.set_budget(start * ENTRY_BYTES * 2);
        assert!(budget.evict_to_budget().is_empty(), "ampliar no desaloja");
        assert_eq!(budget.len(), retained);
        assert_eq!(budget.bytes(), target * ENTRY_BYTES);
    }
}

#[test]
fn avx2_selection_matches_detection() {
    let forced = forced_cpu_extension();
    // En x86_64: forzamos Avx2 solo si el host lo soporta.
    #[cfg(target_arch = "x86_64")]
    assert_eq!(forced.is_some(), std::is_x86_feature_detected!("avx2"));
    // Fuera de x86_64 nunca forzamos nada.
    #[cfg(not(target_arch = "x86_64"))]
    assert!(forced.is_none());
}

/// Encodes a solid PNG in memory so the decode funnel reads a real image file.
fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 30, 255]));
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("no se pudo codificar el PNG de prueba");
    bytes
}

#[test]
fn test_cover_bucket_rounds_up_to_bucket_set() {
    assert_eq!(cover_bucket(96.0, 1.0, 400), 96);
    assert_eq!(cover_bucket(120.0, 1.0, 400), 128);
    assert_eq!(cover_bucket(159.0, 1.0, 400), 160);
    assert_eq!(cover_bucket(206.7, 1.0, 400), 220);
}

#[test]
fn test_cover_bucket_includes_the_detailed_list_size() {
    assert!(COVER_BUCKETS.contains(&220));
    assert_eq!(cover_bucket(220.0, 1.0, 400), 220);
}

#[test]
fn test_library_cover_bucket_never_below_220() {
    // The album cards measure well under 220 px at 1x; the library floor
    // wins so a card is not drawn from a soft 160 px texture.
    assert_eq!(library_cover_bucket(111.3, 1.0, 400), 220);
    assert_eq!(library_cover_bucket(159.0, 1.0, 400), 220);
    assert_eq!(library_cover_bucket(206.7, 1.0, 400), 220);
    assert_eq!(library_cover_bucket(32.0, 1.0, 400), 220);

    // Above the floor the display bucket is kept, and the native writer
    // edge still caps it.
    assert_eq!(library_cover_bucket(300.0, 1.0, 400), 320);
    assert_eq!(library_cover_bucket(500.0, 1.0, 400), 400);
    assert_eq!(library_cover_bucket(500.0, 1.0, 200), 200);

    // The general rule is unchanged, and it is what a thumbnail uses.
    assert_eq!(cover_bucket(159.0, 1.0, 400), 160);
    assert_eq!(cover_bucket(32.0, 1.0, 400), 32);
}

#[test]
fn test_cover_bucket_scales_with_display_scale() {
    assert_eq!(cover_bucket(159.0, 1.5, 400), 256);
    assert_eq!(cover_bucket(159.0, 2.0, 400), 320);
}

#[test]
fn test_cover_bucket_caps_at_native_size() {
    assert_eq!(cover_bucket(500.0, 1.0, 400), 400);
    assert_eq!(cover_bucket(500.0, 1.0, 200), 200);
    assert_eq!(cover_bucket(159.0, 1.0, 96), 96);
}

#[test]
fn test_cover_bucket_rejects_invalid_input() {
    assert_eq!(cover_bucket(f32::NAN, 1.0, 400), COVER_BUCKETS[0]);
    assert_eq!(cover_bucket(0.0, 1.0, 400), COVER_BUCKETS[0]);
    assert_eq!(cover_bucket(-1.0, 1.0, 400), COVER_BUCKETS[0]);
    assert_eq!(cover_bucket(100.0, f32::INFINITY, 400), COVER_BUCKETS[0]);
    assert_eq!(cover_bucket(100.0, 0.0, 400), COVER_BUCKETS[0]);
}

#[test]
fn test_viewport_capacity_counts_a_grid_viewport() {
    assert_eq!(viewport_capacity(7, 800.0, 200.0), 28);
    assert_eq!(viewport_capacity(7, 800.0, 250.0), 28);
    assert_eq!(viewport_capacity(1, 800.0, 200.0), 4);
    assert_eq!(viewport_capacity(0, 800.0, 200.0), 0);
    assert_eq!(viewport_capacity(7, 0.0, 200.0), 0);
    assert_eq!(viewport_capacity(7, 800.0, 0.0), 0);
}

#[test]
fn test_cover_budget_bytes_clamps_to_its_bounds() {
    assert_eq!(cover_budget_bytes(1, 32), COVER_BUDGET_MIN_BYTES);
    assert_eq!(cover_budget_bytes(10_000, 400), COVER_BUDGET_MAX_BYTES);
    assert_eq!(cover_budget_bytes(50, 192), 22_118_400);
}

#[test]
fn test_cover_budget_ledger_tracks_bytes_and_recency() {
    let mut budget = CoverBudget::default();
    assert!(budget.is_empty());

    budget.insert("a.avif", 160, 100);
    budget.insert("b.avif", 160, 40);
    assert_eq!(budget.bytes(), 140);
    assert_eq!(budget.len(), 2);
    assert_eq!(
        budget.order_mru_to_lru(),
        vec!["b.avif@160".to_string(), "a.avif@160".to_string()]
    );

    // A peek reports the retained bytes without changing recency.
    assert_eq!(budget.get("a.avif", 160), Some(100));
    assert_eq!(
        budget.order_mru_to_lru(),
        vec!["b.avif@160".to_string(), "a.avif@160".to_string()]
    );

    assert!(budget.touch("a.avif", 160));
    assert_eq!(
        budget.order_mru_to_lru(),
        vec!["a.avif@160".to_string(), "b.avif@160".to_string()]
    );
    assert!(!budget.touch("missing.avif", 160));

    assert_eq!(budget.remove("a.avif", 160), Some(100));
    assert_eq!(budget.bytes(), 40);
    assert_eq!(budget.len(), 1);
    assert_eq!(budget.get("a.avif", 160), None);

    budget.clear();
    assert_eq!(budget.bytes(), 0);
    assert!(budget.is_empty());
}

#[test]
fn test_cover_budget_ledger_replaces_bytes_on_reinsert() {
    let mut budget = CoverBudget::default();
    budget.insert("a.avif", 160, 100);
    budget.insert("a.avif", 160, 25);

    assert_eq!(budget.bytes(), 25);
    assert_eq!(budget.len(), 1);
    assert_eq!(budget.order_mru_to_lru(), vec!["a.avif@160".to_string()]);

    budget.insert("a.avif", 220, 25);
    assert_eq!(budget.len(), 2);
    assert_eq!(budget.bytes(), 50);
}

/// Holds the guard the process-global cover counters are asserted against.
fn global_guard() -> std::sync::MutexGuard<'static, ()> {
    GLOBAL_STATE_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

#[test]
fn test_negative_cache_only_records_undecodable_files() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    // A file that is not there yet may still be written: it stays retryable.
    let absent = "cache/covers/absent.avif";
    record_negative_cover(
        Path::new(absent),
        &CoverDecodeError::NotFound(PathBuf::from(absent)),
    );
    assert!(
        !is_cover_negative(absent),
        "an absent cover must not be recorded"
    );

    // A read error is not a decode failure either.
    let unreadable = "cache/covers/unreadable.avif";
    record_negative_cover(
        Path::new(unreadable),
        &CoverDecodeError::Io(
            PathBuf::from(unreadable),
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        ),
    );
    assert!(
        !is_cover_negative(unreadable),
        "an unreadable cover must not be recorded"
    );

    // A present file that cannot be decoded is the only recorded case.
    let corrupt = "cache/covers/corrupt.avif";
    record_negative_cover(
        Path::new(corrupt),
        &CoverDecodeError::Decode(PathBuf::from(corrupt)),
    );
    assert!(
        is_cover_negative(corrupt),
        "an undecodable cover must be recorded"
    );
    assert!(
        !is_cover_negative(absent),
        "only the corrupt path was recorded"
    );

    clear_negative_cover_cache();
}

/// The negative set is cleared once before the walk and once when the scan
/// has settled, never per batch.
///
/// A clear inside the walk would re-enable a retry for every cover the grid
/// asked for before the scanner wrote it, once per committed batch. The walk
/// reads the filesystem and its batch boundaries are not injectable, so the
/// source is the seam: `scan_folder` holds exactly one clear, before the
/// directory walk, and the post-scan cleanup tick holds the other one. The
/// scan therefore has one start boundary and one finish boundary, and the
/// finish boundary is the moment the covers of a settled scan are retried.
#[test]
fn test_scan_clears_negative_cache_only_at_boundaries() {
    let scan = include_str!("../db/scanner.rs");
    let clear = "clear_negative_cover_cache";

    assert_eq!(
        scan.matches(clear).count(),
        1,
        "el escaneo limpia el conjunto negativo una sola vez, al empezar"
    );

    let walk = scan
        .find("WalkDir::new(root)")
        .expect("the scan walks the folder it was given");
    let start = scan.find(clear).expect("start boundary");

    assert!(
        start < walk,
        "the set is cleared before the directory walk starts"
    );

    // The finish boundary is the post-scan cleanup, which also re-requests
    // the window so the covers the scan wrote stop looking absent.
    let ticks = include_str!("../gui/app/update/ticks.rs");
    let settled = ticks
        .find("now.saturating_sub(finished_at) >= 40")
        .expect("the post-scan cleanup runs once the scan has settled");
    let finish = ticks
        .find(clear)
        .expect("the post-scan cleanup clears the negative set");
    let retry = ticks
        .find("request_cover_window")
        .expect("the post-scan cleanup re-requests the window");

    assert!(
        finish > settled,
        "the finish boundary sits in the post-scan cleanup path"
    );
    assert!(
        retry > settled,
        "the settled scan retries the covers it may have written late"
    );
}

#[test]
fn test_decode_cover_file_opens_the_file_once() {
    let _guard = global_guard();

    let dir = TempTestDir::new("covers-decode-open");
    let path = dir.path().join("test-cover.png");
    std::fs::write(&path, png_bytes(4, 4)).expect("no se pudo escribir el PNG de prueba");

    let before = cover_io_counters().file_opens;
    let decoded = decode_cover_file(&path, 400).expect("la carátula de prueba no se decodificó");
    let after = cover_io_counters().file_opens;

    assert_eq!(
        after - before,
        1,
        "una decodificación abre el archivo una vez"
    );
    assert_eq!(
        decoded.pixels.len(),
        (decoded.width * decoded.height * 4) as usize
    );
}

#[test]
fn test_decode_cover_file_does_not_upscale() {
    let _guard = global_guard();
    let dir = TempTestDir::new("covers-decode-native");
    let path = dir.path().join("native-cover.png");
    std::fs::write(&path, png_bytes(8, 8)).expect("no se pudo escribir el PNG de prueba");

    let decoded = decode_cover_file(&path, 10_000).expect("la carátula de prueba no se decodificó");

    assert_eq!((decoded.width, decoded.height), (8, 8));
}

#[test]
fn test_decode_cover_file_downscales_to_the_target() {
    let _guard = global_guard();
    let dir = TempTestDir::new("covers-decode-downscale");
    let path = dir.path().join("large-cover.png");
    std::fs::write(&path, png_bytes(8, 8)).expect("no se pudo escribir el PNG de prueba");

    let decoded = decode_cover_file(&path, 4).expect("la carátula de prueba no se decodificó");

    assert_eq!((decoded.width, decoded.height), (4, 4));
    assert_eq!(decoded.pixels.len(), 4 * 4 * 4);
}

#[test]
fn test_decode_cover_file_reports_a_missing_file() {
    let _guard = global_guard();
    let dir = TempTestDir::new("covers-decode-missing");
    let path = dir.path().join("absent.avif");

    let error = decode_cover_file(&path, 160).expect_err("un archivo ausente no puede decodificar");
    assert!(matches!(error, CoverDecodeError::NotFound(_)));
}

#[test]
fn test_album_cover_path_composes_from_hashes() {
    let album_hash = "aa11bb22";
    let cover_hash = "cc33dd44";

    let expected = crate::utils::paths::get()
        .cover_cache_dir
        .join(format!("{album_hash}-{cover_hash}.avif"))
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        album_cover_path(Some(album_hash), Some(cover_hash)),
        Some(expected),
        "la ruta del álbum se compone del hash del álbum y el del contenido"
    );

    assert_eq!(album_cover_path(None, Some(cover_hash)), None);
    assert_eq!(album_cover_path(Some(album_hash), None), None);
    assert_eq!(
        album_cover_path(Some(""), Some(cover_hash)),
        None,
        "un hash vacío no compone una ruta"
    );
    assert_eq!(
        album_cover_path(Some(album_hash), Some("../evil")),
        None,
        "un hash no hexadecimal no puede escapar del directorio de carátulas"
    );
}

#[test]
fn test_effective_cover_path_override_wins() {
    let album_hash = "aa11bb22";
    let cover_hash = "cc33dd44";

    let resolved = effective_cover_path(Some(album_hash), Some(cover_hash), Some("ab12cd34"), 42)
        .expect("un override siempre resuelve una ruta");

    let file_name = Path::new(&resolved)
        .file_name()
        .and_then(|name| name.to_str())
        .expect("la ruta resuelta debe tener un nombre de archivo");
    assert_eq!(
        file_name, "song-42-ab12cd34.avif",
        "el override nombra el archivo por su dueño y su contenido"
    );
    assert!(
        resolved.starts_with(
            crate::utils::paths::get()
                .cover_cache_dir
                .to_string_lossy()
                .as_ref()
        ),
        "la ruta del override vive en el directorio de carátulas"
    );
}

#[test]
fn test_effective_cover_path_falls_back_to_album() {
    let album_path = album_cover_path(Some("aa11bb22"), Some("cc33dd44"))
        .expect("dos hashes hexadecimales componen la ruta del álbum");

    assert_eq!(
        effective_cover_path(Some("aa11bb22"), Some("cc33dd44"), None, 7),
        Some(album_path.clone()),
        "sin override se muestra la carátula del álbum tal cual"
    );
    assert_eq!(
        effective_cover_path(Some("aa11bb22"), Some("cc33dd44"), Some(""), 7),
        Some(album_path),
        "un override vacío no es una carátula propia"
    );
}

#[test]
fn test_effective_cover_path_none_when_no_cover() {
    assert_eq!(
        effective_cover_path(None, None, None, 1),
        None,
        "sin carátula de álbum ni override no hay ruta que mostrar"
    );
    assert_eq!(effective_cover_path(Some(""), None, None, 1), None);
    assert!(
        effective_cover_path(None, None, Some("ab12cd34"), 1).is_some(),
        "un override compone su ruta aunque el álbum no tenga carátula"
    );
}

#[test]
fn test_cover_file_name_rejects_non_hex_hash() {
    assert_eq!(
        album_cover_file_name("aa", "bb"),
        Ok("aa-bb.avif".to_string()),
        "un nombre de álbum válido lleva ambos hashes"
    );
    assert_eq!(
        song_cover_file_name(3, "bb"),
        Ok("song-3-bb.avif".to_string()),
        "un nombre propio válido lleva el dueño y el contenido"
    );

    assert_eq!(
        album_cover_file_name("", "bb"),
        Err(CoverNameError::EmptyHash)
    );
    assert_eq!(song_cover_file_name(3, ""), Err(CoverNameError::EmptyHash));
    assert_eq!(
        album_cover_file_name("aa", "../evil"),
        Err(CoverNameError::NonHexHash),
        "un hash no hexadecimal no puede convertirse en nombre de archivo"
    );
    assert_eq!(
        song_cover_file_name(3, "zz"),
        Err(CoverNameError::NonHexHash)
    );
}
