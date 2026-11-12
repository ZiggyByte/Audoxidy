//! Area tests for the decoded-cover store, the grid window memo and the decode
//! request path.
//!
//! Every test builds prebuilt image handles and injects the enqueue closure, so
//! no test opens a file, decodes an image, starts a worker or needs a window.
//! The tests that drive the window request path read the process-global negative
//! set, so they hold the shared guard for their whole body.

use std::rc::Rc;
use std::sync::Arc;

use iced::widget::image::Handle;

use super::cover_cache::{
    CoverDecodeOutcome, CoverKey, CoverStore, GridCoverElement, prefetch_band,
};
use super::grid_metrics::{grid_metrics, row_count, total_content_height};
use super::state::LibraryManager;
use super::types::AlbumEntry;
use crate::db::database::SongData;
use crate::utils::covers::{
    COVER_BUDGET_MAX_BYTES, COVER_BUDGET_MIN_BYTES, GLOBAL_STATE_TEST_LOCK,
    clear_negative_cover_cache, cover_budget_bytes, cover_io_counters, viewport_capacity,
};

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

    let mut manager = LibraryManager::default();
    manager.scroll_generation = 4;
    manager
        .cover_store
        .set_window(vec![("covers/window.avif".to_string(), 160)]);

    let outcome = |path: &str, generation: u64| CoverDecodeOutcome {
        key: key(path, 160),
        generation,
        handle: Some(fake_handle(2, 2)),
        bytes: 16,
        error: None,
    };

    // The current generation is kept even outside the window.
    assert!(manager.apply_cover_decoded(outcome("covers/current.avif", 4)));
    assert!(manager.cover_store.contains("covers/current.avif", 160));

    // One generation older and outside the window is superseded and dropped.
    assert!(!manager.apply_cover_decoded(outcome("covers/stale.avif", 3)));
    assert!(!manager.cover_store.contains("covers/stale.avif", 160));
    assert!(
        !manager
            .cover_store
            .is_in_flight(&key("covers/stale.avif", 160)),
        "una clave descartada no queda en vuelo"
    );

    // One generation older but still needed by the window is kept.
    assert!(manager.apply_cover_decoded(outcome("covers/window.avif", 3)));
    assert!(manager.cover_store.contains("covers/window.avif", 160));
}
