//! Area tests for the decoded-cover store, the grid window memo and the decode
//! request path.
//!
//! Every test builds prebuilt image handles and injects the enqueue closure, so
//! no test opens a file, decodes an image, starts a worker or needs a window.
//! The tests that drive the window request path read the process-global negative
//! set, so they hold the shared guard for their whole body.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use iced::widget::image::Handle;

use super::cover_cache::{
    CoverDecodeOutcome, CoverKey, CoverStore, GridCoverElement, prefetch_band,
};
use super::grid_metrics::{grid_metrics, row_count, total_content_height};
use super::state::LibraryManager;
use super::types::{AlbumEntry, ArtistGroup, LibraryViewMode, LibraryVirtualRow};
use crate::db::database::SongData;
use crate::utils::covers::{
    COVER_BUDGET_MAX_BYTES, COVER_BUDGET_MIN_BYTES, CoverDecodeError, GLOBAL_STATE_TEST_LOCK,
    clear_negative_cover_cache, cover_bucket, cover_budget_bytes, cover_io_counters,
    cover_native_size, record_negative_cover, viewport_capacity,
};

/// Whole-cache flush entry points a track change must never call again.
///
/// The two names are spelled in pieces so that no line of this file matches the
/// gate that keeps the production path free of them.
const WHOLE_CACHE_FLUSH_NAMES: [&str; 2] = [
    concat!("clear_", "raw_cache"),
    concat!("clear_all_", "cover_cache"),
];

/// Handle stand-in: the store never inspects pixels, only handle identity.
fn fake_handle(width: u32, height: u32) -> Handle {
    Handle::from_rgba(width, height, vec![0u8; (width * height * 4) as usize])
}

fn key(path: &str, bucket: u32) -> CoverKey {
    CoverKey {
        path: Arc::from(path),
        bucket: bucket as u16,
    }
}

/// Serializes the tests that touch process-global cover state.
fn global_guard() -> std::sync::MutexGuard<'static, ()> {
    GLOBAL_STATE_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

/// Manager with `count` synthetic albums, each with its own cover path.
fn manager_with_albums(count: usize) -> LibraryManager {
    let mut manager = LibraryManager::default();
    manager.cached_albums = Some(
        (0..count)
            .map(|index| AlbumEntry {
                id: format!("album-{index}"),
                title: format!("Album {index}"),
                artist: "Artist".to_string(),
                genre: "Genre".to_string(),
                year: "2024".to_string(),
                cover_path: Some(format!("covers/{index}.avif")),
            })
            .collect(),
    );

    manager
}

/// Manager with one artist group whose songs belong to a single album.
///
/// `cover` is the already-resolved path the read layer put on each song row, and
/// the album snapshot exposes the same path, so a test can compare what the grid
/// card and the detailed list receive.
fn manager_with_one_album(cover: &str, songs: usize) -> LibraryManager {
    let mut manager = LibraryManager::default();

    let songs: Vec<Arc<SongData>> = (0..songs)
        .map(|index| {
            let mut song = SongData::default();
            song.id = index as i64 + 1;
            song.title = Some(Arc::from(format!("Song {index}").into_boxed_str()));
            song.artist = Some(Arc::from("Artist"));
            song.album = Some(Arc::from("Album 0"));
            song.compressed_cached_cover_root = Some(Arc::from(cover));
            Arc::new(song)
        })
        .collect();

    manager.artist_groups = vec![ArtistGroup {
        name: "Artist".to_string(),
        albums_count: 1,
        songs_count: songs.len(),
        songs,
        duration_secs: 0.0,
    }];
    manager.cached_albums = Some(vec![AlbumEntry {
        id: "album-0".to_string(),
        title: "Album 0".to_string(),
        artist: "Artist".to_string(),
        genre: "Genre".to_string(),
        year: "2024".to_string(),
        cover_path: Some(cover.to_string()),
    }]);

    manager
}

/// Cover path of the first album card in a grid row set.
fn first_grid_cover_path(rows: &[GridCoverElement]) -> Option<String> {
    rows.iter().find_map(|element| match element {
        GridCoverElement::AlbumRow { albums, .. } => {
            albums.first().and_then(|album| album.cover_path.clone())
        }
        GridCoverElement::ExpandedSongs { .. } => None,
    })
}

/// Cover path of the first album block in a row set.
fn first_block_cover_path(rows: &[LibraryVirtualRow]) -> Option<String> {
    rows.iter().find_map(|row| match row {
        LibraryVirtualRow::AlbumBlock { cover_path, .. } => cover_path.clone(),
        _ => None,
    })
}

#[test]
fn test_cover_cache_scroll_back_reuses_handle_without_decode() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let metrics = grid_metrics(800.0);
    let viewport_h = 800.0;
    let mut manager = manager_with_albums(4);
    manager.grid_metrics.set(metrics);

    let jobs = manager.window_jobs(0.0, viewport_h);
    assert_eq!(
        jobs.len(),
        4,
        "cada álbum visible debe solicitar una decodificación"
    );

    let mut requested_keys = Vec::new();
    let accepted = manager.request_jobs_with(jobs, |job| {
        requested_keys.push(key(&job.key.path, job.bucket));
        true
    });
    assert_eq!(accepted, 4);

    // The workers mint one handle per accepted key; store them the way the
    // decode-result handler does.
    for key in requested_keys {
        manager
            .cover_store
            .insert_handle(key.clone(), fake_handle(8, 8), 8 * 8 * 4);
        manager.cover_store.clear_in_flight(&key);
    }

    let mut requests = 0;
    let jobs = manager.window_jobs(0.0, viewport_h);
    let accepted = manager.request_jobs_with(jobs, |_| {
        requests += 1;
        true
    });

    assert_eq!(requests, 0, "una carátula retenida no se vuelve a pedir");
    assert_eq!(
        accepted, 0,
        "volver dentro de la ventana no debe encolar decodificaciones"
    );
}

#[test]
fn test_grid_rows_keep_the_expanded_song_block() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let metrics = grid_metrics(800.0);
    let mut manager = manager_with_albums(4);
    manager.grid_metrics.set(metrics);
    manager.expanded_album = Some("Artist|album-1".to_string());
    manager.expanded_album_songs = Some(vec![
        Arc::new(SongData::default()),
        Arc::new(SongData::default()),
    ]);

    let (_, _, rows) = manager.get_visible_grid_rows(&metrics, 0.0, 800.0);

    let albums = rows
        .iter()
        .find_map(|element| match element {
            GridCoverElement::AlbumRow { albums, .. } => Some(albums),
            GridCoverElement::ExpandedSongs { .. } => None,
        })
        .expect("la fila de álbumes debe estar presente");
    assert_eq!(albums.len(), 4);

    let songs = rows
        .iter()
        .find_map(|element| match element {
            GridCoverElement::ExpandedSongs { songs, .. } => Some(songs),
            GridCoverElement::AlbumRow { .. } => None,
        })
        .expect("el bloque de canciones del álbum expandido debe conservarse");
    assert_eq!(songs.len(), 2);
}

#[test]
fn test_grid_rows_memo_hit_performs_no_cover_lookup() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let metrics = grid_metrics(800.0);
    let mut manager = manager_with_albums(4);
    manager.grid_metrics.set(metrics);

    let (top_space, bottom_space, first) = manager.get_visible_grid_rows(&metrics, 0.0, 800.0);
    let lookups = manager.cover_store.lookup_count();
    assert_eq!(
        lookups, 4,
        "la primera ventana resuelve una carátula por tarjeta"
    );

    let (hit_top, hit_bottom, second) = manager.get_visible_grid_rows(&metrics, 0.0, 800.0);

    assert!(
        Rc::ptr_eq(&first, &second),
        "un acierto de memo debe compartir la misma fila"
    );
    assert_eq!(
        manager.cover_store.lookup_count(),
        lookups,
        "un acierto de memo no debe consultar el almacén"
    );
    assert_eq!((hit_top, hit_bottom), (top_space, bottom_space));
}

#[test]
fn test_grid_rows_memo_hit_preserves_spacers() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let metrics = grid_metrics(1200.0);
    let mut manager = manager_with_albums(64);
    manager.grid_metrics.set(metrics);

    let viewport_y = 2000.0;
    let viewport_h = 300.0;

    let (top_space, bottom_space, first) =
        manager.get_visible_grid_rows(&metrics, viewport_y, viewport_h);
    assert!(top_space > 0.0, "debe haber filas por encima de la ventana");
    assert!(
        bottom_space > 0.0,
        "debe haber filas por debajo de la ventana"
    );

    let (hit_top, hit_bottom, second) =
        manager.get_visible_grid_rows(&metrics, viewport_y, viewport_h);

    assert!(Rc::ptr_eq(&first, &second));
    assert_eq!(
        hit_top, top_space,
        "un acierto de memo no debe colapsar el espacio superior"
    );
    assert_eq!(
        hit_bottom, bottom_space,
        "un acierto de memo no debe colapsar el espacio inferior"
    );
}

#[test]
fn test_cover_budget_matches_viewport_capacity() {
    let capacity = viewport_capacity(10, 1167.0, 233.4);
    assert_eq!(
        capacity, 50,
        "diez columnas de cinco filas llenan el viewport medido"
    );

    assert_eq!(cover_budget_bytes(capacity, 192), 2 * 50 * 192 * 192 * 4);
    assert_eq!(cover_budget_bytes(capacity, 192), 14_745_600);
}

#[test]
fn test_cover_budget_clamped_to_floor_and_ceiling() {
    assert_eq!(
        cover_budget_bytes(1, 32),
        COVER_BUDGET_MIN_BYTES,
        "una capacidad pequeña no baja del piso"
    );
    assert_eq!(
        cover_budget_bytes(400, 400),
        COVER_BUDGET_MAX_BYTES,
        "una capacidad grande no supera el techo"
    );
    assert_eq!(
        cover_budget_bytes(10_000, 400),
        COVER_BUDGET_MAX_BYTES,
        "el techo no depende del número de álbumes"
    );
}

#[test]
fn test_cover_cache_evicts_least_recent_outside_window() {
    let _guard = global_guard();

    let mut store = CoverStore::default();
    store.insert_handle(key("covers/a.avif", 160), fake_handle(1, 1), 100);
    store.insert_handle(key("covers/b.avif", 160), fake_handle(1, 1), 100);
    store.insert_handle(key("covers/c.avif", 160), fake_handle(1, 1), 100);

    let evictions_before = cover_io_counters().evictions;
    store.set_budget(250);
    assert_eq!(store.budget(), 250);

    assert_eq!(store.evict_to_budget(), 1);
    assert_eq!(store.bytes(), 200);
    assert!(
        !store.contains("covers/a.avif", 160),
        "el menos reciente se descarta primero"
    );
    assert!(store.contains("covers/b.avif", 160));
    assert!(store.contains("covers/c.avif", 160));
    assert_eq!(cover_io_counters().evictions - evictions_before, 1);
    assert_eq!(cover_io_counters().cache_bytes, 200);
}

#[test]
fn test_cover_cache_never_evicts_window_entries() {
    let _guard = global_guard();

    let mut store = CoverStore::default();
    store.insert_handle(key("covers/a.avif", 160), fake_handle(1, 1), 100);
    store.insert_handle(key("covers/b.avif", 160), fake_handle(1, 1), 100);
    store.insert_handle(key("covers/c.avif", 160), fake_handle(1, 1), 100);
    store.set_window(vec![
        ("covers/a.avif".to_string(), 160),
        ("covers/b.avif".to_string(), 160),
    ]);

    assert!(store.is_window("covers/a.avif", 160));
    assert!(!store.is_window("covers/c.avif", 160));

    // A budget below the window alone cannot blank a visible cover.
    store.set_budget(1);
    assert_eq!(store.evict_to_budget(), 1);
    assert_eq!(store.bytes(), 200);
    assert!(store.contains("covers/a.avif", 160));
    assert!(store.contains("covers/b.avif", 160));
    assert!(!store.contains("covers/c.avif", 160));

    // With only window entries left, the eviction stops instead of emptying.
    assert_eq!(store.evict_to_budget(), 0);
    assert_eq!(store.bytes(), 200);
}

#[test]
fn test_cover_cache_trim_outside_window_keeps_window() {
    let _guard = global_guard();

    let mut store = CoverStore::default();
    store.insert_handle(key("covers/a.avif", 220), fake_handle(1, 1), 120);
    store.insert_handle(key("covers/b.avif", 220), fake_handle(1, 1), 130);
    store.insert_handle(key("covers/c.avif", 160), fake_handle(1, 1), 140);
    store.set_window(vec![
        ("covers/a.avif".to_string(), 220),
        ("covers/b.avif".to_string(), 220),
    ]);

    assert_eq!(store.trim_outside_window(), 1);
    assert!(store.contains("covers/a.avif", 220));
    assert!(store.contains("covers/b.avif", 220));
    assert!(!store.contains("covers/c.avif", 160));
    assert_eq!(store.bytes(), 250, "las claves de la ventana no cambian");
    assert_eq!(cover_io_counters().cache_bytes, 250);
}

#[test]
fn test_cover_prefetch_band_targets_one_screen_in_scroll_direction() {
    let metrics = grid_metrics(800.0);
    let album_count = 120;
    let row_height = metrics.row_height;
    let viewport_h = row_height * 2.0;

    let down = prefetch_band(&metrics, album_count, None, 0, 0.0, viewport_h, 1.0, 1.0)
        .expect("la banda hacia abajo debe existir");
    assert!(
        (down - row_height * 2.0).abs() < 0.01,
        "una pantalla hacia abajo cae en la fila 2"
    );

    let up = prefetch_band(
        &metrics,
        album_count,
        None,
        0,
        row_height * 8.0,
        viewport_h,
        -1.0,
        1.0,
    )
    .expect("la banda hacia arriba debe existir");
    assert!(
        (up - row_height * 6.0).abs() < 0.01,
        "una pantalla hacia arriba cae en la fila 6"
    );
    assert!(
        up < row_height * 8.0,
        "la banda queda por encima del viewport"
    );

    let half = prefetch_band(&metrics, album_count, None, 0, 0.0, viewport_h, 1.0, 0.5)
        .expect("media pantalla también produce banda");
    assert!(
        (half - row_height).abs() < 0.01,
        "media pantalla hacia abajo cae en la fila 1"
    );

    assert_eq!(
        prefetch_band(&metrics, album_count, None, 0, 0.0, viewport_h, 0.0, 1.0),
        None,
        "en reposo no hay banda que pedir"
    );
}

#[test]
fn test_cover_prefetch_band_clamps_at_the_last_row() {
    let metrics = grid_metrics(800.0);
    let album_count = 6;
    assert!(row_count(&metrics, album_count) >= 2);

    let total = total_content_height(&metrics, album_count, None, 0);
    let last_top = total - metrics.row_height;

    // The band lands inside the last row, so it resolves to that row and never
    // to an offset past the content.
    let band = prefetch_band(
        &metrics,
        album_count,
        None,
        0,
        total - metrics.row_height * 0.6,
        metrics.row_height * 0.4,
        1.0,
        1.0,
    )
    .expect("la banda dentro del contenido debe existir");
    assert!(
        (band - last_top).abs() < 0.01,
        "la banda se recorta en la última fila"
    );

    // Past the last row there is nothing left to prefetch.
    assert_eq!(
        prefetch_band(
            &metrics,
            album_count,
            None,
            0,
            total,
            metrics.row_height,
            1.0,
            1.0
        ),
        None
    );
    assert_eq!(
        prefetch_band(&metrics, 0, None, 0, 0.0, metrics.row_height, 1.0, 1.0),
        None,
        "una biblioteca vacía no tiene banda"
    );
}

#[test]
fn test_cover_prefetch_dedups_in_flight() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let metrics = grid_metrics(1200.0);
    let mut manager = manager_with_albums(12);
    manager.grid_metrics.set(metrics);

    // A viewport shorter than one row places the look-ahead band on the row that
    // is already visible, so the prefetch asks for keys the window requested.
    let viewport_h = metrics.row_height * 0.25;
    let band_y = prefetch_band(&metrics, 12, None, 0, 0.0, viewport_h, 1.0, 1.0)
        .expect("la banda debe existir");

    let mut jobs = manager.window_jobs(0.0, viewport_h);
    jobs.extend(manager.window_jobs(band_y, viewport_h));

    let mut enqueued = 0;
    let accepted = manager.request_jobs_with(jobs, |_| {
        enqueued += 1;
        true
    });

    let window_only = manager.window_jobs(0.0, viewport_h).len();
    assert!(
        window_only > 0,
        "la ventana debe pedir al menos una tarjeta"
    );
    assert_eq!(
        accepted, window_only,
        "una clave en vuelo no se vuelve a encolar"
    );
    assert_eq!(enqueued, window_only);
    assert_eq!(manager.cover_store.len(), 0);

    // A second identical pass finds every key already queued.
    let jobs = manager.window_jobs(0.0, viewport_h);
    assert_eq!(
        manager.request_jobs_with(jobs, |_| {
            enqueued += 1;
            true
        }),
        0
    );
    assert_eq!(enqueued, window_only);
}

#[test]
fn test_cover_prefetch_discards_stale_generation() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let metrics = grid_metrics(800.0);
    let mut manager = manager_with_albums(4);
    manager.grid_metrics.set(metrics);
    manager.scroll_generation = 4;

    // A request pass stamps every job with the generation current at that
    // moment, so no request can go out carrying an outdated one.
    let stale = manager.window_jobs(0.0, 800.0);
    assert_eq!(stale.len(), 4, "la ventana pide una carátula por tarjeta");
    assert!(stale.iter().all(|job| job.generation == 4));

    let mut enqueued = 0;
    assert_eq!(
        manager.request_jobs_with(stale, |_| {
            enqueued += 1;
            true
        }),
        4
    );

    // The scroll moves on before the workers answer. The keys are still in
    // flight, so the newer pass discards them instead of requesting them under
    // the new generation.
    manager.scroll_generation = 5;
    let fresh = manager.window_jobs(0.0, 800.0);
    assert!(fresh.iter().all(|job| job.generation == 5));

    assert_eq!(
        manager.request_jobs_with(fresh, |_| {
            enqueued += 1;
            true
        }),
        0,
        "una solicitud en vuelo no se repite con la generación nueva"
    );
    assert_eq!(enqueued, 4);
}

#[test]
fn test_stale_generation_outcome_is_retained() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let metrics = grid_metrics(800.0);
    let mut manager = manager_with_albums(4);
    manager.grid_metrics.set(metrics);

    let requested = manager.window_jobs(0.0, 800.0);
    assert_eq!(
        requested.len(),
        4,
        "la ventana pide una carátula por tarjeta"
    );
    let first = requested[0].clone();
    let generation_at_request = manager.scroll_generation;

    // The scroll keeps moving while the worker decodes, so the answer comes
    // back stamped with a generation the manager has already left behind.
    manager.scroll_generation += 3;

    let outcome = CoverDecodeOutcome {
        key: first.key.clone(),
        generation: generation_at_request,
        handle: Some(fake_handle(2, 2)),
        bytes: 2 * 2 * 4,
        error: None,
    };

    assert!(
        manager.apply_cover_decoded(outcome),
        "una decodificación terminada se conserva aunque la generación haya avanzado"
    );
    assert!(
        manager
            .cover_store
            .contains(&first.key.path, first.key.bucket),
        "el handle decodificado debe quedar en el almacén"
    );

    // The retained handle answers every later pass for that card, so the file
    // is never opened and decoded a second time.
    let mut requested_again = 0;
    let jobs = manager.window_jobs(0.0, 800.0);
    let accepted = manager.request_jobs_with(jobs, |job| {
        if job.key == first.key {
            requested_again += 1;
        }
        true
    });

    assert_eq!(
        requested_again, 0,
        "una carátula retenida no se vuelve a pedir"
    );
    assert_eq!(accepted, 3, "solo las tres carátulas que faltan se encolan");
}

#[test]
fn test_request_window_emits_list_mode_buckets() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let native = cover_native_size();

    // The detailed list draws the album card's cover at 220 logical pixels.
    let mut manager = manager_with_one_album("covers/detailed.avif", 2);
    manager.view_mode = LibraryViewMode::DetailedList;
    let detailed_bucket = cover_bucket(220.0, manager.scale_factor, native);
    let jobs = manager.window_jobs(0.0, 800.0);
    assert!(
        !jobs.is_empty(),
        "el modo detallado debe pedir la carátula de su tarjeta"
    );
    assert!(
        jobs.iter()
            .all(|job| job.key.bucket == detailed_bucket as u16),
        "cada trabajo del modo detallado usa el bucket de 220"
    );

    // The thumbnail list draws one 32 px thumbnail per visible song row.
    let mut manager = manager_with_one_album("covers/thumb.avif", 3);
    manager.view_mode = LibraryViewMode::ThumbnailList;
    let thumbnail_bucket = cover_bucket(32.0, manager.scale_factor, native);
    let jobs = manager.window_jobs(0.0, 800.0);
    assert!(
        !jobs.is_empty(),
        "el modo miniatura debe pedir una carátula por fila"
    );
    assert!(
        jobs.iter()
            .all(|job| job.key.bucket == thumbnail_bucket as u16),
        "cada trabajo del modo miniatura usa el bucket de 32"
    );
    assert_ne!(
        detailed_bucket, thumbnail_bucket,
        "los dos modos de lista no comparten bucket"
    );

    // The grid keeps requesting the bucket its measured card reads.
    let metrics = grid_metrics(800.0);
    let mut manager = manager_with_albums(4);
    manager.view_mode = LibraryViewMode::Grid;
    manager.grid_metrics.set(metrics);
    let grid_bucket = cover_bucket(metrics.cover_size, manager.scale_factor, native);
    let jobs = manager.window_jobs(0.0, 800.0);
    assert_eq!(jobs.len(), 4);
    assert!(
        jobs.iter().all(|job| job.key.bucket == grid_bucket as u16),
        "cada trabajo de la cuadrícula usa el bucket de su tarjeta"
    );
}

#[test]
fn test_album_block_rows_carry_cover_path() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let cover = "covers/one-album.avif";
    let metrics = grid_metrics(800.0);

    let mut manager = manager_with_one_album(cover, 2);
    manager.grid_metrics.set(metrics);
    let (_, _, grid_rows) = manager.get_visible_grid_rows(&metrics, 0.0, 800.0);
    let grid_path = first_grid_cover_path(&grid_rows);

    manager.view_mode = LibraryViewMode::DetailedList;
    manager.invalidate_cache();
    let (_, _, _, rows, _) = manager.visible_rows_at(0.0, 800.0);
    let block_path = first_block_cover_path(&rows);

    assert_eq!(
        grid_path.as_deref(),
        Some(cover),
        "la tarjeta de la cuadrícula toma la ruta de su entrada de álbum"
    );
    assert_eq!(
        block_path.as_deref(),
        Some(cover),
        "el bloque detallado toma la misma ruta desde el snapshot"
    );
    assert_eq!(grid_path, block_path);
}

#[test]
fn test_negative_cover_is_not_requested_again() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let manager = manager_with_albums(4);
    manager.grid_metrics.set(grid_metrics(800.0));

    let path = "covers/0.avif";
    let before = manager.window_jobs(0.0, 800.0);
    assert_eq!(before.len(), 4, "la ventana pide una carátula por tarjeta");
    assert!(
        before.iter().any(|job| job.key.path.as_ref() == path),
        "la primera pasada sí pide la carátula"
    );

    record_negative_cover(
        Path::new(path),
        &CoverDecodeError::Decode(PathBuf::from(path)),
    );

    let after = manager.window_jobs(0.0, 800.0);
    assert_eq!(
        after.len(),
        3,
        "una carátula registrada como indescodificable no se vuelve a pedir"
    );
    assert!(
        !after.iter().any(|job| job.key.path.as_ref() == path),
        "ningún trabajo debe apuntar al archivo registrado"
    );

    clear_negative_cover_cache();
}

#[test]
fn test_track_change_keeps_cover_cache() {
    let _guard = global_guard();

    // No line of the playback path may flush the store: that removed flush is what
    // makes the decoded entries survive a track change.
    let playback_source = include_str!("../app/update/playback.rs");
    for flush in WHOLE_CACHE_FLUSH_NAMES {
        assert!(
            !playback_source.contains(flush),
            "el cambio de pista no puede vaciar la caché de carátulas"
        );
    }

    // A track change only reads the playing cover, so the entry the previous track
    // stored is still retained, byte for byte, afterwards.
    let mut store = CoverStore::default();
    store.insert_handle(key("/covers/album.avif", 160), fake_handle(2, 2), 2 * 2 * 4);
    let stored_id = store
        .get_path("/covers/album.avif", 160)
        .expect("la entrada debe existir antes del cambio de pista")
        .id();

    let played = store.get_path("/covers/album.avif", 160);
    assert_eq!(played.map(|handle| handle.id()), Some(stored_id));

    assert!(store.contains("/covers/album.avif", 160));
    assert_eq!(store.len(), 1);
    assert_eq!(store.bytes(), 2 * 2 * 4);
}

#[test]
fn test_album_block_ambiguous_title_falls_back_to_the_row_path() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let row_path = "covers/row-version.avif";
    let mut manager = manager_with_one_album(row_path, 2);

    // Two album identities share one (artist, title) key, so the album map alone
    // cannot say which version the group belongs to.
    manager.cached_albums = Some(vec![
        AlbumEntry {
            id: "album-first".to_string(),
            title: "Album 0".to_string(),
            artist: "Artist".to_string(),
            genre: "Genre".to_string(),
            year: "2024".to_string(),
            cover_path: Some("covers/first-version.avif".to_string()),
        },
        AlbumEntry {
            id: "album-last".to_string(),
            title: "Album 0".to_string(),
            artist: "Artist".to_string(),
            genre: "Genre".to_string(),
            year: "2025".to_string(),
            cover_path: Some("covers/last-version.avif".to_string()),
        },
    ]);

    manager.view_mode = LibraryViewMode::DetailedList;
    let (_, structure) = manager.get_view_structure();
    let group_path = structure
        .first()
        .and_then(|artist| artist.albums.first())
        .and_then(|album| album.cover_path.clone());

    assert_eq!(
        group_path.as_deref(),
        Some(row_path),
        "un título ambiguo usa la ruta de la primera canción, nunca la del último álbum insertado"
    );

    let (_, _, _, rows, _) = manager.visible_rows_at(0.0, 800.0);
    assert_eq!(
        first_block_cover_path(&rows).as_deref(),
        Some(row_path),
        "el bloque detallado expone la ruta de la canción"
    );

    // The non-detailed construction site must expose the same path.
    manager.view_mode = LibraryViewMode::ThumbnailList;
    manager.invalidate_cache();
    let (_, _, _, rows, _) = manager.visible_rows_at(0.0, 800.0);
    let thumbnail_path = first_block_cover_path(&rows);
    assert_eq!(
        thumbnail_path.as_deref(),
        Some(row_path),
        "el otro brazo de la fila expone la misma ruta"
    );
    assert_ne!(
        thumbnail_path.as_deref(),
        Some("covers/last-version.avif"),
        "la ruta adivinada por el mapa de álbumes nunca llega a la tarjeta"
    );
}
