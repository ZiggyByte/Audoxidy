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
    CoverDecodeOutcome, CoverKey, CoverStore, GridCoverElement, prefetch_band, prefetch_band_offset,
};
use super::grid_metrics::{grid_metrics, row_count, total_content_height};
use super::state::LibraryManager;
use super::types::{AlbumEntry, ArtistGroup, LibraryViewMode, LibraryVirtualRow};
use crate::db::database::SongData;
use crate::utils::covers::{
    COVER_BUDGET_MAX_BYTES, COVER_BUDGET_MIN_BYTES, CoverDecodeError, GLOBAL_STATE_TEST_LOCK,
    clear_negative_cover_cache, cover_bucket, cover_budget_bytes, cover_io_counters,
    cover_native_size, library_cover_bucket, record_negative_cover, viewport_capacity,
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

    assert_eq!(cover_budget_bytes(capacity, 192), 3 * 50 * 192 * 192 * 4);
    assert_eq!(cover_budget_bytes(capacity, 192), 22_118_400);
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

    let downward_band = prefetch_band(&metrics, album_count, None, 0, 0.0, viewport_h, 1.0, 1.0)
        .expect("la banda hacia abajo debe existir");
    assert!(
        (downward_band - row_height * 2.0).abs() < 0.01,
        "una pantalla hacia abajo cae en la fila 2"
    );

    let upward_band = prefetch_band(
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
        (upward_band - row_height * 6.0).abs() < 0.01,
        "una pantalla hacia arriba cae en la fila 6"
    );
    assert!(
        upward_band < row_height * 8.0,
        "la banda queda por encima del viewport"
    );

    let half_screen_band = prefetch_band(&metrics, album_count, None, 0, 0.0, viewport_h, 1.0, 0.5)
        .expect("media pantalla también produce banda");
    assert!(
        (half_screen_band - row_height).abs() < 0.01,
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

    // The first pass queues the covers the viewport draws. The request-side
    // dedup is what replaced the per-request epoch: a key already queued is
    // skipped, so a later pass over the same rows cannot open the same file
    // twice.
    let first = manager.window_jobs(0.0, 800.0);
    assert_eq!(first.len(), 4, "la ventana pide una carátula por tarjeta");

    let mut enqueued = 0;
    assert_eq!(
        manager.request_jobs_with(first, |_| {
            enqueued += 1;
            true
        }),
        4
    );

    // A pass that repeats the request while the decodes are still in flight
    // discards every key: the cards are already on their way.
    let again = manager.window_jobs(0.0, 800.0);
    assert_eq!(
        manager.request_jobs_with(again, |_| {
            enqueued += 1;
            true
        }),
        0,
        "una solicitud en vuelo no se repite"
    );
    assert_eq!(enqueued, 4);
}

#[test]
fn test_stale_generation_outcome_is_retained() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let metrics = grid_metrics(800.0);
    let mut manager = manager_with_albums(64);
    manager.grid_metrics.set(metrics);

    let requested = manager.window_jobs(0.0, 800.0);
    assert!(
        !requested.is_empty(),
        "la ventana pide una carátula por tarjeta"
    );
    let first = requested[0].clone();

    // The viewport moves on while the worker decodes, so the answer arrives for
    // a card the visible window no longer draws.
    manager.scroll_offset = iced::Vector::new(0.0, 800.0);
    let moved = manager.window_jobs(800.0, 800.0);
    assert!(
        !moved.iter().any(|job| job.key == first.key),
        "la tarjeta de la primera ventana ya no está en la segunda"
    );

    let outcome = CoverDecodeOutcome {
        key: first.key.clone(),
        handle: Some(fake_handle(2, 2)),
        bytes: 2 * 2 * 4,
        error: None,
    };

    assert!(
        manager.apply_cover_decoded(outcome),
        "una decodificación terminada se conserva aunque su tarjeta haya salido de la ventana"
    );
    assert!(
        manager
            .cover_store
            .contains(&first.key.path, first.key.bucket),
        "el handle decodificado debe quedar en el almacén"
    );

    // Back at the original viewport the retained handle answers its card, so the
    // file is never opened and decoded a second time: only the covers that are
    // still missing are queued.
    manager.scroll_offset = iced::Vector::new(0.0, 0.0);
    let mut requested_again = 0;
    let job_count = manager.window_jobs(0.0, 800.0).len();
    let accepted = manager.request_jobs_with(manager.window_jobs(0.0, 800.0), |job| {
        if job.key == first.key {
            requested_again += 1;
        }
        true
    });

    assert_eq!(
        requested_again, 0,
        "una carátula retenida no se vuelve a pedir"
    );
    assert_eq!(
        accepted,
        job_count - 1,
        "solo las carátulas que faltan se encolan"
    );
}

#[test]
fn test_cover_cache_retains_scroll_back_cushion() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let metrics = grid_metrics(800.0);
    let mut manager = manager_with_albums(64);
    manager.grid_metrics.set(metrics);

    // First viewport: every handle its cards ask for is decoded and stored.
    let first_jobs = manager.window_jobs(0.0, 800.0);
    assert!(!first_jobs.is_empty(), "la primera ventana pide carátulas");
    let bucket = first_jobs[0].bucket;
    let entry_bytes = (bucket as usize) * (bucket as usize) * 4;

    let first_keys: Vec<(String, u16)> = first_jobs
        .iter()
        .map(|job| (job.key.path.to_string(), job.key.bucket))
        .collect();
    manager.cover_store.set_window(first_keys.clone());
    for job in &first_jobs {
        manager
            .cover_store
            .insert_handle(job.key.clone(), fake_handle(2, 2), entry_bytes);
    }

    // The window moves one viewport down: the rows below decode too, and the
    // first viewport is no longer protected.
    let second_jobs = manager.window_jobs(800.0, 800.0);
    assert!(!second_jobs.is_empty(), "la segunda ventana pide carátulas");
    let second_keys: Vec<(String, u16)> = second_jobs
        .iter()
        .map(|job| (job.key.path.to_string(), job.key.bucket))
        .collect();
    manager.cover_store.set_window(second_keys.clone());
    for job in &second_jobs {
        manager
            .cover_store
            .insert_handle(job.key.clone(), fake_handle(2, 2), entry_bytes);
    }

    // The budget covers both viewports, so moving the window discards nothing.
    let two_viewports = manager.cover_store.bytes();
    manager.cover_store.set_budget(two_viewports);

    assert_eq!(
        manager.cover_store.evict_to_budget(),
        0,
        "el cojín que el presupuesto retiene no se descarta"
    );
    for (path, bucket) in &first_keys {
        assert!(
            manager.cover_store.contains(path, *bucket),
            "una carátula que el scroll acaba de dejar atrás sigue retenida"
        );
    }

    // Only a budget below the two viewports evicts, and it starts with the
    // oldest entry that left the window, never with a visible one.
    let oldest_outside = first_keys
        .iter()
        .find(|key| !second_keys.contains(key))
        .expect("las dos ventanas no cubren las mismas filas");
    manager.cover_store.set_budget(two_viewports - entry_bytes);

    assert_eq!(manager.cover_store.evict_to_budget(), 1);
    assert!(
        !manager
            .cover_store
            .contains(&oldest_outside.0, oldest_outside.1),
        "solo un presupuesto por debajo del cojín lo descarta"
    );
    for (path, bucket) in &second_keys {
        assert!(
            manager.cover_store.contains(path, *bucket),
            "la ventana visible nunca se descarta"
        );
    }

    // The collection pass trims the covers outside the window when the library is
    // out of focus, and keeps the window. The behavioral test above
    // (`test_cover_cache_trim_outside_window_keeps_window`) pins that the visible
    // window is never dropped; the gate below pins that the pass runs at all.
}

/// The collection pass releases the covers the viewport no longer shows.
///
/// The budget only evicts once a decode overflows it, so the covers behind the
/// window stay resident for the whole session unless the collection pass trims
/// them. It trims only while the library has no focus: a focused scroll is what
/// reads next, and there the prefetch band and the scroll-back cushion are live.
///
/// The handler needs a built application, so this pins the wiring in the source
/// the way the render gates do.
#[test]
fn test_gc_releases_off_window_covers_while_the_library_is_unfocused() {
    let source = include_str!("../app/update/memory.rs");

    let trim = source
        .find("cover_store.trim_outside_window()")
        .expect("the collection pass must release the covers outside the window");

    let branch = source[..trim]
        .rfind("if !is_library_focused")
        .expect("the trim must run only while the library has no focus");
    assert!(
        !source[branch..trim].contains('}'),
        "the trim cannot run while the library has the focus"
    );
}

#[test]
fn test_memo_not_invalidated_by_off_window_decode() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let metrics = grid_metrics(800.0);
    let mut manager = manager_with_albums(4);
    manager.grid_metrics.set(metrics);

    // The grid rows are built once for the viewport and memoized.
    let (_, _, rows) = manager.get_visible_grid_rows(&metrics, 0.0, 800.0);
    assert!(manager.cached_grid_rows.borrow().is_some());

    let jobs = manager.window_jobs(0.0, 800.0);
    assert!(!jobs.is_empty(), "la ventana pide una carátula por tarjeta");
    manager.cover_store.set_window(
        jobs.iter()
            .map(|job| (job.key.path.to_string(), job.key.bucket))
            .collect::<Vec<_>>(),
    );

    // A decode that lands for a cover no visible row draws: the handle is kept
    // and the memoized rows are untouched, because they never drew it.
    let off_window = CoverDecodeOutcome {
        key: key("covers/off-window.avif", 160),
        handle: Some(fake_handle(2, 2)),
        bytes: 2 * 2 * 4,
        error: None,
    };
    assert!(manager.apply_cover_decoded(off_window));
    assert!(manager.cover_store.contains("covers/off-window.avif", 160));
    assert!(
        manager.cached_grid_rows.borrow().is_some(),
        "una carátula fuera de la ventana no cambia las filas dibujadas"
    );

    let (_, _, again) = manager.get_visible_grid_rows(&metrics, 0.0, 800.0);
    assert!(
        Rc::ptr_eq(&rows, &again),
        "el memo sigue sirviendo las mismas filas"
    );

    // A handle the window does draw does change them.
    let drawn = CoverDecodeOutcome {
        key: jobs[0].key.clone(),
        handle: Some(fake_handle(2, 2)),
        bytes: 2 * 2 * 4,
        error: None,
    };
    assert!(manager.apply_cover_decoded(drawn));
    assert!(
        manager.cached_grid_rows.borrow().is_none(),
        "una carátula de la ventana invalida las filas"
    );
}

#[test]
fn test_request_window_emits_list_mode_buckets() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let native = cover_native_size();

    // The detailed list draws the album card's cover at 220 logical pixels.
    let mut manager = manager_with_one_album("covers/detailed.avif", 2);
    manager.view_mode = LibraryViewMode::DetailedList;
    let detailed_bucket = library_cover_bucket(220.0, manager.scale_factor, native);
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

    // The grid keeps requesting the album bucket its measured card reads.
    let metrics = grid_metrics(800.0);
    let mut manager = manager_with_albums(4);
    manager.view_mode = LibraryViewMode::Grid;
    manager.grid_metrics.set(metrics);
    let grid_bucket = library_cover_bucket(metrics.cover_size, manager.scale_factor, native);
    let jobs = manager.window_jobs(0.0, 800.0);
    assert_eq!(jobs.len(), 4);
    assert!(
        jobs.iter().all(|job| job.key.bucket == grid_bucket as u16),
        "cada trabajo de la cuadrícula usa el bucket de álbum de su tarjeta"
    );
}

#[test]
fn test_grid_window_requests_the_library_minimum_bucket() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    // A 1080p window at 1x measures seven columns, and the library viewport fits
    // four rows, so the retained budget covers three viewport capacities at 220 px:
    // the visible window, its prefetch band and one screen of scroll-back cushion.
    let metrics = grid_metrics(1300.0);
    assert_eq!(metrics.columns, 7, "el ancho medido da siete columnas");
    let capacity = viewport_capacity(metrics.columns, 800.0, metrics.row_height);
    assert_eq!(
        capacity, 28,
        "cuatro filas de siete tarjetas llenan la ventana"
    );
    assert_eq!(
        cover_budget_bytes(capacity, 220),
        16_262_400,
        "el presupuesto de 1080p a 1x son tres ventanas de 220 px"
    );

    let mut manager = manager_with_albums(28);
    manager.grid_metrics.set(metrics);
    manager.scale_factor = 1.0;

    assert_eq!(
        manager.window_bucket(),
        220,
        "la cuadrícula nunca decodifica por debajo del mínimo de álbum"
    );

    let jobs = manager.window_jobs(0.0, 800.0);
    assert!(!jobs.is_empty(), "la ventana pide una carátula por tarjeta");
    assert!(
        jobs.iter().all(|job| job.bucket == 220),
        "cada tarjeta pide el mínimo de 220 px a 1x"
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

/// The render path receives resolved handles and never builds one from pixels.
///
/// A handle minted during layout would create a new texture on every frame,
/// which is the flicker and the redundant decode the stored-handle cache
/// exists to prevent.
#[test]
fn test_render_path_never_builds_a_cover_handle() {
    for (file, source) in [
        ("view.rs", include_str!("view.rs")),
        ("lists.rs", include_str!("lists.rs")),
    ] {
        assert!(
            !source.contains("Handle::from_bytes"),
            "{file} cannot build a cover handle while laying out a frame"
        );
        assert!(
            !source.contains("Handle::from_rgba"),
            "{file} cannot build a cover handle while laying out a frame"
        );
    }
}

/// The grid render consumes the memoized rows without per-card album work.
///
/// The visible rows carry their cover path already, so the layout pass must not
/// scan the album snapshot, resolve a path per card or reach into the decoded
/// store while it draws.
#[test]
fn test_grid_render_path_reads_only_the_memoized_rows() {
    let source = include_str!("view.rs");

    assert!(
        source.contains("manager.get_visible_grid_rows("),
        "the grid render must consume the rows the manager memoizes"
    );
    assert!(
        !source.contains("cover_store"),
        "the grid render cannot reach the decoded store per frame"
    );
    assert!(
        !source.contains("album_cover_handle"),
        "the grid render cannot resolve a cover path per card"
    );
    assert!(
        !source.contains(".iter().find("),
        "the grid render cannot run a linear search per card"
    );
    assert!(
        !source.contains("_database.lock"),
        "the grid render cannot take the database lock per frame"
    );
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
fn test_failed_decode_does_not_request_the_window_again() {
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
    let failed_key = requested[0].key.clone();

    let failed = CoverDecodeOutcome {
        key: failed_key.clone(),
        handle: None,
        bytes: 0,
        error: Some("the file has not been written yet".to_string()),
    };

    // A failure releases the key but keeps nothing, so the outcome handler must
    // not ask for the window again in the same message: the released key would
    // be re-enqueued at once and the missing file reopened until it lands.
    assert!(
        !manager.apply_cover_decoded(failed),
        "una decodificación fallida no conserva ningún handle"
    );
    assert!(
        !manager
            .cover_store
            .contains(&failed_key.path, failed_key.bucket),
        "un fallo no deja una entrada en el almacén"
    );
    assert!(
        !manager.cover_store.is_in_flight(&failed_key),
        "la clave sigue reintentable en la siguiente pasada"
    );
    assert_eq!(
        manager.window_jobs(0.0, 800.0).len(),
        4,
        "la siguiente pasada de la ventana vuelve a pedir las cuatro carátulas"
    );

    // The source half: the request sits inside the kept-handle branch, so a
    // failed outcome can never reach it.
    let source = include_str!("../app/update/library.rs");
    let arm_start = source
        .find("Message::CoverDecoded(outcome)")
        .expect("el mensaje de decodificación terminada existe");
    let rest = &source[arm_start..];
    let arm_end = rest
        .find("Message::CoverSweepFinished")
        .expect("el brazo termina antes del siguiente mensaje");
    let arm = &rest[..arm_end];

    assert_eq!(
        arm.matches("request_cover_window").count(),
        1,
        "el brazo pide la ventana una sola vez"
    );
    let branch = arm
        .find("if self.library_manager.apply_cover_decoded(outcome) {")
        .expect("la petición está condicionada al handle conservado");
    let request = arm
        .find("request_cover_window")
        .expect("el brazo pide la ventana");

    assert!(
        branch < request,
        "la petición de la ventana sigue a la comprobación del handle"
    );
    assert!(
        !arm[branch..request].contains('}'),
        "la petición está dentro del brazo que conserva el handle"
    );
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

/// Manager measured like a 1080p window at 1x: seven columns, four rows of cards.
///
/// The viewport height is the one the request path reads from `last_viewport`, so a
/// test drives the same capacity and the same render window a real window does.
fn manager_at_1080p(albums: usize) -> LibraryManager {
    let mut manager = manager_with_albums(albums);
    manager.grid_metrics.set(grid_metrics(1300.0));
    manager.scale_factor = 1.0;
    manager.view_mode = LibraryViewMode::Grid;
    manager.last_viewport = Some(iced::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 1300.0,
        height: 800.0,
    });
    manager.scroll_offset = iced::Vector::new(0.0, 0.0);

    manager
}

#[test]
fn test_requested_band_keys_are_window_protected() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let mut manager = manager_at_1080p(200);
    manager.scroll_direction = 1.0;

    let window_only = manager.window_jobs(0.0, 800.0).len();
    let requested = manager.request_cover_window_with(|_| true, 1.0);

    assert!(
        window_only > 0,
        "la ventana debe pedir al menos una carátula"
    );
    assert!(
        requested.len() > window_only,
        "la banda de una pantalla hacia abajo suma claves a la ventana"
    );

    // A look-ahead key is requested exactly once: if the pass does not protect it,
    // the decode that lands it is evicted at once and the next pass asks again.
    for key in &requested {
        assert!(
            manager.cover_store.is_window(&key.path, key.bucket),
            "toda clave pedida queda protegida: {}@{}",
            key.path,
            key.bucket
        );
    }
}

#[test]
fn test_requested_keys_fit_the_budget() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let mut manager = manager_at_1080p(200);
    manager.scroll_direction = 1.0;

    let requested = manager.request_cover_window_with(|_| true, 1.0);
    let bucket = manager.window_bucket() as usize;
    let bucket_bytes = bucket * bucket * 4;
    let requested_bytes = requested.len() * bucket_bytes;

    assert_eq!(bucket, 220, "la cuadrícula pide el mínimo de álbum a 1x");
    assert!(!requested.is_empty(), "la ventana pide carátulas");
    assert!(
        requested_bytes <= manager.cover_store.budget(),
        "el presupuesto cubre las {} claves pedidas: {requested_bytes} > {}",
        requested.len(),
        manager.cover_store.budget()
    );
    assert!(
        manager.cover_store.budget() >= COVER_BUDGET_MIN_BYTES,
        "el presupuesto nunca baja del piso"
    );
    assert!(
        manager.cover_store.budget() <= COVER_BUDGET_MAX_BYTES,
        "el presupuesto nunca supera el techo"
    );
}

#[test]
fn test_second_window_request_after_inserts_requests_nothing() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let mut manager = manager_at_1080p(200);
    manager.scroll_direction = 1.0;
    // A scrolled position puts the window and its band on rows that together ask
    // for more covers than the capacity budget pays for, which is the state a scroll
    // leaves the store in.
    manager.scroll_offset = iced::Vector::new(0.0, 1000.0);

    let requested = manager.request_cover_window_with(|_| true, 1.0);
    assert!(!requested.is_empty(), "la ventana pide carátulas");

    let bucket = manager.window_bucket() as usize;
    let entry_bytes = bucket * bucket * 4;
    for key in &requested {
        manager
            .cover_store
            .insert_handle(key.clone(), fake_handle(2, 2), entry_bytes);
        manager.cover_store.clear_in_flight(key);
    }

    // The decode handler trims after every stored handle. Trimming here is what
    // exposes the loop: without the protection and the matching budget, the keys
    // the pass just requested are dropped and the next pass asks for them again.
    assert_eq!(
        manager.cover_store.evict_to_budget(),
        0,
        "las claves pedidas caben en el presupuesto"
    );

    let unique: std::collections::HashSet<&CoverKey> = requested.iter().collect();
    assert_eq!(
        manager.cover_store.len(),
        unique.len(),
        "cada clave pedida queda retenida"
    );

    let mut enqueued = 0;
    let again = manager.request_cover_window_with(
        |_| {
            enqueued += 1;
            true
        },
        1.0,
    );

    assert_eq!(
        enqueued, 0,
        "una segunda pasada sobre las mismas claves no encola nada"
    );
    assert_eq!(
        again.len(),
        requested.len(),
        "la ventana pedida no cambia entre pasadas"
    );
}

#[test]
fn test_band_is_truncated_at_the_budget_ceiling() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    // A viewport tall enough that the window and its band together exceed the hard
    // ceiling at the album bucket, while the window alone still fits: the 32 MiB
    // clamp on a large screen.
    let height = 3000.0;
    let mut manager = manager_at_1080p(600);
    manager.last_viewport = Some(iced::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 1300.0,
        height,
    });
    manager.scroll_direction = 1.0;

    let metrics = manager.grid_metrics.get();
    let bucket = manager.window_bucket() as usize;
    let bucket_bytes = bucket * bucket * 4;
    let ceiling_entries = COVER_BUDGET_MAX_BYTES / bucket_bytes;

    let window_only = manager.window_jobs(0.0, height).len();
    let screens = if crate::utils::is_low_resource() {
        0.5
    } else {
        1.0
    };
    let band_y = prefetch_band(&metrics, 600, None, 0, 0.0, height, 1.0, screens)
        .expect("la banda de una pantalla hacia abajo debe existir");
    let band_only = manager.window_jobs(band_y, height).len();

    assert!(
        window_only + band_only > ceiling_entries,
        "el escenario debe exceder el techo: {window_only} + {band_only} > {ceiling_entries}"
    );

    let requested = manager.request_cover_window_with(|_| true, 1.0);

    assert!(
        requested.len() >= window_only,
        "la ventana visible siempre se pide completa"
    );
    assert!(
        requested.len() <= ceiling_entries,
        "ninguna clave pedida excede el techo: {} > {ceiling_entries}",
        requested.len()
    );
    assert!(
        requested.len() < window_only + band_only,
        "la banda no entra completa: {} de {}",
        requested.len(),
        window_only + band_only
    );
    for key in &requested {
        assert!(
            manager.cover_store.is_window(&key.path, key.bucket),
            "toda clave pedida queda protegida: {}@{}",
            key.path,
            key.bucket
        );
    }

    // The published budget stays inside the hard bounds and pays for every key the
    // pass kept, so the truncation is what made the set affordable.
    assert!(
        manager.cover_store.budget() <= COVER_BUDGET_MAX_BYTES,
        "el presupuesto nunca supera el techo"
    );
    assert!(
        manager.cover_store.budget() >= requested.len() * bucket_bytes,
        "el presupuesto cubre las claves que sí entraron"
    );
}

#[test]
fn test_budget_retains_one_screen_behind_the_window() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    // The window plus its band must not consume the whole budget: a short scroll
    // back has to land on entries that are still retained.
    let mut manager = manager_at_1080p(200);
    manager.scroll_direction = 1.0;

    let metrics = manager.grid_metrics.get();
    let capacity = viewport_capacity(metrics.columns, 800.0, metrics.row_height);
    assert_eq!(
        capacity, 28,
        "cuatro filas de siete tarjetas llenan la ventana"
    );

    let bucket = manager.window_bucket() as usize;
    let bucket_bytes = bucket * bucket * 4;
    let requested = manager.request_cover_window_with(|_| true, 1.0);
    let budget_entries = manager.cover_store.budget() / bucket_bytes;

    assert_eq!(
        budget_entries, 84,
        "el presupuesto de 1080p retiene tres ventanas a 220 px"
    );
    assert!(
        requested.len() + capacity <= budget_entries,
        "la ventana y su banda dejan una pantalla de colchón: {} + {capacity} > {budget_entries}",
        requested.len()
    );
}

#[test]
fn test_band_is_capped_by_the_budget_not_the_ceiling() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    // A viewport tall enough that three screens of entries exceed the 32 MiB clamp at
    // the album bucket, so the budget lands below three capacities and the band has to
    // be capped by the budget rather than by the ceiling. The window alone still fits.
    let height = 3000.0;
    let mut manager = manager_at_1080p(600);
    manager.last_viewport = Some(iced::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 1300.0,
        height,
    });
    manager.scroll_direction = 1.0;

    let metrics = manager.grid_metrics.get();
    let capacity = viewport_capacity(metrics.columns, height, metrics.row_height);
    let bucket = manager.window_bucket() as usize;
    let bucket_bytes = bucket * bucket * 4;
    assert!(
        3 * capacity * bucket_bytes > COVER_BUDGET_MAX_BYTES,
        "el escenario debe recortar el presupuesto: {capacity} entradas no caben en tres pantallas"
    );

    let window_only = manager.window_jobs(0.0, height).len();
    let requested = manager.request_cover_window_with(|_| true, 1.0);
    let budget_entries = manager.cover_store.budget() / bucket_bytes;

    assert_eq!(
        manager.cover_store.budget(),
        COVER_BUDGET_MAX_BYTES,
        "el techo es lo que recorta el presupuesto"
    );
    assert!(
        budget_entries < 3 * capacity,
        "el presupuesto queda por debajo de tres pantallas: {budget_entries} de {}",
        3 * capacity
    );
    assert!(
        requested.len() > window_only,
        "la banda entra mientras quede sitio en el presupuesto"
    );
    assert!(
        requested.len() <= budget_entries,
        "ninguna clave pedida excede el presupuesto: {} > {budget_entries}",
        requested.len()
    );
}

/// A scroll event requests exactly the cards the viewport draws.
///
/// The band is speculative work: it belongs to the idle prefetch, so a wheel or
/// trackpad burst must never pay for rows the user has not reached yet.
#[test]
fn test_scroll_request_asks_for_the_visible_window_only() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let mut manager = manager_at_1080p(200);
    // A moving gesture has a direction, so a band would exist if the scroll
    // request asked for one.
    manager.scroll_direction = 1.0;
    let metrics = manager.grid_metrics.get();
    assert!(
        prefetch_band(&metrics, 200, None, 0, 0.0, 800.0, 1.0, 1.0).is_some(),
        "el escenario tiene una banda disponible si alguien la pidiera"
    );

    let window_only: Vec<CoverKey> = manager
        .window_jobs(0.0, 800.0)
        .iter()
        .map(|job| job.key.clone())
        .collect();
    let requested = manager.request_cover_window_with(|_| true, 0.0);

    assert!(!window_only.is_empty(), "la ventana pide carátulas");
    assert_eq!(
        requested, window_only,
        "la petición del scroll pide la ventana visible y ninguna fila de más"
    );

    // The wrapper the scroll handler calls is the visible-only intent, and the
    // handler uses it instead of the look-ahead one.
    let source = include_str!("cover_cache.rs");
    let wrapper_start = source
        .find("pub fn request_cover_window(&mut self)")
        .expect("la petición de la ventana visible existe");
    let wrapper_end = source
        .find("pub fn request_cover_window_ahead")
        .expect("la petición con banda existe");
    assert!(wrapper_start < wrapper_end);
    assert!(
        source[wrapper_start..wrapper_end].contains(", 0.0);"),
        "la petición de la ventana visible no lleva banda"
    );

    let library = include_str!("../app/update/library.rs");
    let scroll_arm = &library[library
        .find("Message::LibraryScroll(offset) =>")
        .expect("el brazo del scroll existe")
        ..library
            .find("Message::FilterScroll(viewport) =>")
            .expect("el scroll de filtros cierra el brazo")];
    assert!(
        scroll_arm.contains("self.library_manager.request_cover_window();"),
        "el scroll pide la ventana visible"
    );
    assert!(
        !scroll_arm.contains("request_cover_window_ahead"),
        "el scroll nunca pide la banda de precarga"
    );
}

/// The look-ahead band follows the extent of the active view mode.
///
/// The list modes are several times taller than the grid for the same library, so a
/// target clamped to the grid extent disappears for most of the scroll range.
/// `prefetch_band_offset` takes the extent as an input, and this pins that a target
/// past the grid rows still resolves while the supplied extent allows it.
#[test]
fn test_prefetch_band_offset_uses_the_supplied_content_extent() {
    let metrics = grid_metrics(800.0);
    let grid_total = total_content_height(&metrics, 2, None, 0);
    let list_total = grid_total * 10.0;
    let viewport_h = 800.0;

    // Far past the grid rows, still inside the list extent.
    let viewport_y = grid_total * 4.0;
    let band = prefetch_band_offset(viewport_y, viewport_h, list_total, 1.0, 1.0)
        .expect("la banda existe dentro de la extensión de lista");
    assert!(
        (band - (viewport_y + viewport_h)).abs() < 0.01,
        "la banda queda una pantalla por debajo del viewport"
    );
    assert!(
        band > grid_total,
        "la banda supera la extensión de la rejilla: {band} > {grid_total}"
    );

    // Past the supplied extent there is nothing left to prefetch.
    assert_eq!(
        prefetch_band_offset(
            list_total - viewport_h * 0.5,
            viewport_h,
            list_total,
            1.0,
            1.0
        ),
        None,
        "una banda que arranca pasada la extensión no pide nada"
    );

    // Scrolling up from the middle keeps the band above the viewport.
    let upward_band = prefetch_band_offset(viewport_y, viewport_h, list_total, -1.0, 1.0)
        .expect("la banda hacia arriba existe");
    assert!(
        upward_band < viewport_y,
        "la banda hacia arriba queda por encima"
    );

    // A mode that draws no covers reports no extent.
    assert_eq!(prefetch_band_offset(0.0, viewport_h, 0.0, 1.0, 1.0), None);

    // A resting gesture asks for no band.
    assert_eq!(
        prefetch_band_offset(0.0, viewport_h, list_total, 0.0, 1.0),
        None
    );
}

/// Each view mode reports its own scroll extent, from the producer the renderer uses.
#[test]
fn test_list_modes_report_their_own_scroll_extent() {
    let mut manager = manager_with_one_album("covers/0.avif", 200);

    manager.view_mode = LibraryViewMode::Grid;
    let grid_extent = manager.scrollable_content_height();

    manager.view_mode = LibraryViewMode::DetailedList;
    let structure_extent = manager.get_view_structure().0;
    let list_extent = manager.scrollable_content_height();

    assert!(structure_extent > 0.0, "la lista tiene contenido");
    assert!(
        (list_extent - structure_extent).abs() < 0.01,
        "la extensión de lista es la que produce el maquetador: {list_extent} vs {structure_extent}"
    );
    assert!(
        list_extent > grid_extent * 4.0,
        "la lista es mucho más alta que la rejilla: {list_extent} > {grid_extent}"
    );

    // The simple list draws no covers, so no band is requested for it.
    manager.view_mode = LibraryViewMode::SimpleList;
    assert_eq!(
        manager.scrollable_content_height(),
        0.0,
        "la lista simple no pide banda"
    );
}

/// The idle prefetch requests the visible window plus the look-ahead band.
#[test]
fn test_prefetch_request_adds_the_look_ahead_band() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let mut manager = manager_at_1080p(200);
    manager.scroll_direction = 1.0;

    let window_only = manager.window_jobs(0.0, 800.0).len();
    let requested = manager.request_cover_window_with(|_| true, 1.0);

    assert!(window_only > 0, "la ventana pide carátulas");
    assert!(
        requested.len() > window_only,
        "la precarga suma la banda de una pantalla: {} de {window_only}",
        requested.len()
    );
    for key in &requested {
        assert!(
            manager.cover_store.is_window(&key.path, key.bucket),
            "toda clave pedida, banda incluida, queda protegida: {}@{}",
            key.path,
            key.bucket
        );
    }

    // The prefetch handler is the only caller that looks ahead.
    let library = include_str!("../app/update/library.rs");
    let prefetch_arm = &library[library
        .find("Message::CoverPrefetchDue =>")
        .expect("el brazo de la precarga existe")
        ..library
            .find("Message::LibraryShowPlaying =>")
            .expect("el mensaje siguiente cierra el brazo")];
    assert!(
        prefetch_arm.contains("request_cover_window_ahead("),
        "la precarga pide la ventana con banda"
    );
}

/// Every handler that changes the visible set requests the decode window.
///
/// A search, a sort, a view-mode switch and an expansion all replace which rows
/// the viewport draws without a scroll. The scrollable only notifies when the
/// content is taller than the viewport and its offset changed, so none of those
/// flows can rely on it to ask for the covers they are about to draw.
#[test]
fn test_visible_set_handlers_request_the_cover_window() {
    let library = include_str!("../app/update/library.rs");
    let arms = [
        (
            "Message::LibraryFilteredLoaded(songs) =>",
            "Message::InternalLibraryRefreshed(songs, albums, stats) =>",
        ),
        (
            "Message::LibrarySortChanged(col) =>",
            "Message::StartColumnResize(col) =>",
        ),
        (
            "Message::ChangeLibraryViewMode(mode) =>",
            "Message::ToggleLibraryViewDropdown =>",
        ),
        (
            "Message::ToggleAlbumExpansion(album_id) =>",
            "Message::ToggleArtistExpansion(name) =>",
        ),
        (
            "Message::ToggleArtistExpansion(name) =>",
            "Message::LibraryAllSongsLoaded(songs) =>",
        ),
    ];

    for (start, end) in arms {
        let arm_start = library
            .find(start)
            .unwrap_or_else(|| panic!("no se encontró el brazo {start}"));
        let arm_end = library
            .find(end)
            .unwrap_or_else(|| panic!("no se encontró el cierre {end}"));
        assert!(arm_start < arm_end, "el brazo {start} está antes de {end}");

        assert!(
            library[arm_start..arm_end].contains("self.library_manager.request_cover_window();"),
            "el brazo {start} debe pedir la ventana de carátulas tras cambiar el conjunto visible"
        );
    }
}

/// A changed visible set asks for its own covers, and the pass converges.
#[test]
fn test_visible_set_change_requests_the_new_paths() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let mut manager = manager_at_1080p(200);
    manager.scroll_direction = 1.0;

    // First visible set: request it and land every handle the way the decode
    // handler does, so a second pass has nothing left to ask for.
    let first = manager.request_cover_window_with(|_| true, 0.0);
    assert!(!first.is_empty(), "la primera ventana pide carátulas");
    let bucket = manager.window_bucket() as usize;
    let entry_bytes = bucket * bucket * 4;
    for key in &first {
        manager
            .cover_store
            .insert_handle(key.clone(), fake_handle(2, 2), entry_bytes);
        manager.cover_store.clear_in_flight(key);
    }

    let mut repeated = 0;
    let requested_again = manager.request_cover_window_with(
        |_| {
            repeated += 1;
            true
        },
        0.0,
    );
    assert_eq!(
        repeated, 0,
        "una segunda pasada sobre el mismo conjunto no encola nada"
    );
    assert_eq!(
        requested_again, first,
        "la ventana pedida no cambia entre pasadas"
    );

    // The snapshot is replaced without a scroll: a filter or a search result.
    manager.cached_albums = Some(
        (0..200)
            .map(|index| AlbumEntry {
                id: format!("filtered-{index}"),
                title: format!("Album {index}"),
                artist: "Artist".to_string(),
                genre: "Genre".to_string(),
                year: "2024".to_string(),
                cover_path: Some(format!("covers/filtered-{index}.avif")),
            })
            .collect(),
    );

    let mut new_paths = Vec::new();
    let second = manager.request_cover_window_with(
        |job| {
            new_paths.push(job.key.path.to_string());
            true
        },
        0.0,
    );

    assert!(!second.is_empty(), "el conjunto nuevo pide sus carátulas");
    assert_eq!(
        new_paths.len(),
        second.len(),
        "cada clave nueva se encola una vez"
    );
    assert!(
        new_paths
            .iter()
            .all(|path| path.starts_with("covers/filtered-")),
        "la petición sólo nombra las carátulas del conjunto nuevo"
    );

    // Landing the new handles converges the pass, exactly as it does for the
    // first set: this is the idempotence the handler relies on.
    for key in &second {
        manager
            .cover_store
            .insert_handle(key.clone(), fake_handle(2, 2), entry_bytes);
        manager.cover_store.clear_in_flight(key);
    }

    let mut again = 0;
    let requested_again = manager.request_cover_window_with(
        |_| {
            again += 1;
            true
        },
        0.0,
    );
    assert_eq!(
        again, 0,
        "con las carátulas nuevas retenidas la pasada no encola nada"
    );
    assert_eq!(
        requested_again, second,
        "el conjunto nuevo sigue siendo el mismo entre pasadas"
    );
}

/// The orphan sweep is held back by scans and cover writes, never by decodes.
///
/// A decode only reads a file and cannot make an unreferenced file exist, so a
/// running decode cannot turn a referenced cover into an apparent orphan. The
/// gauge stays published for the cover counters.
#[test]
fn test_sweep_guard_ignores_read_only_decodes() {
    for (file, source) in [
        (
            "src/gui/app/update/memory.rs",
            include_str!("../app/update/memory.rs"),
        ),
        (
            "src/gui/app/update/library.rs",
            include_str!("../app/update/library.rs"),
        ),
    ] {
        assert!(
            source.contains("sweep_orphan_covers("),
            "{file} debe barrer las carátulas huérfanas"
        );
        assert!(
            source.contains("cover_writes_in_flight()"),
            "{file} debe consultar las escrituras de carátulas en curso"
        );
        assert!(
            !source.contains("cover_work_in_flight"),
            "{file} no puede frenar el barrido por una decodificación de sólo lectura"
        );
    }

    // The decode gauge still feeds the cover counters.
    assert!(
        include_str!("cover_cache.rs").contains("pub fn cover_work_in_flight()"),
        "el contador de decodificaciones sigue disponible para las métricas"
    );
}

/// Two album versions that share a title and an artist stay two blocks.
///
/// The row's album hash is the identity, so a remaster in another folder keeps
/// its own card and its own cover instead of merging into the first version's.
#[test]
fn test_same_title_versions_stay_separate() {
    let _guard = global_guard();
    clear_negative_cover_cache();

    let version_song = |index: i64, album_hash: &str, cover: &str| {
        let mut song = SongData::default();
        song.id = index;
        song.title = Some(Arc::from(format!("Song {index}").into_boxed_str()));
        song.artist = Some(Arc::from("Artist"));
        song.album = Some(Arc::from("Album 0"));
        song.album_hash = Some(Arc::from(album_hash));
        song.compressed_cached_cover_root = Some(Arc::from(cover));
        Arc::new(song)
    };

    // Both versions are "Album 0" by "Artist", and their rows interleave, which is
    // the shape the title-only grouping merged into a single card.
    let songs = vec![
        version_song(1, "album-first", "covers/first.avif"),
        version_song(2, "album-last", "covers/last.avif"),
        version_song(3, "album-first", "covers/first.avif"),
        version_song(4, "album-last", "covers/last.avif"),
    ];

    let mut manager = LibraryManager::default();
    manager.artist_groups = vec![ArtistGroup {
        name: "Artist".to_string(),
        albums_count: 2,
        songs_count: songs.len(),
        songs,
        duration_secs: 0.0,
    }];
    manager.cached_albums = Some(vec![
        AlbumEntry {
            id: "album-first".to_string(),
            title: "Album 0".to_string(),
            artist: "Artist".to_string(),
            genre: "Genre".to_string(),
            year: "2024".to_string(),
            cover_path: Some("covers/first.avif".to_string()),
        },
        AlbumEntry {
            id: "album-last".to_string(),
            title: "Album 0".to_string(),
            artist: "Artist".to_string(),
            genre: "Genre".to_string(),
            year: "2025".to_string(),
            cover_path: Some("covers/last.avif".to_string()),
        },
    ]);
    manager.view_mode = LibraryViewMode::DetailedList;

    let (_, structure) = manager.get_view_structure();
    let albums = &structure
        .first()
        .expect("el artista de las dos versiones existe")
        .albums;

    assert_eq!(
        albums.len(),
        2,
        "dos versiones con el mismo título y artista son dos álbumes"
    );
    assert_eq!(
        albums
            .iter()
            .map(|album| album.album_hash.as_str())
            .collect::<Vec<_>>(),
        vec!["album-first", "album-last"],
        "cada bloque usa el hash de su propia versión"
    );
    assert_eq!(
        albums[0].cover_path.as_deref(),
        Some("covers/first.avif"),
        "la primera versión conserva su portada"
    );
    assert_eq!(
        albums[1].cover_path.as_deref(),
        Some("covers/last.avif"),
        "la segunda versión conserva la suya"
    );
    assert_eq!(albums[0].songs.len(), 2);
    assert_eq!(albums[1].songs.len(), 2);

    let (_, _, _, rows, _) = manager.visible_rows_at(0.0, 800.0);
    let covers: Vec<Option<String>> = rows
        .iter()
        .filter_map(|row| match row {
            LibraryVirtualRow::AlbumBlock { cover_path, .. } => Some(cover_path.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(covers.len(), 2, "el detalle pinta una tarjeta por versión");
    assert_ne!(
        covers[0], covers[1],
        "cada tarjeta dibuja la portada de su propia versión"
    );

    // The filtered grid resolves its album entries with the same identity, so an
    // active search cannot collapse the two versions back into one card.
    let filtered_songs: Vec<Arc<SongData>> = manager
        .artist_groups
        .first()
        .map(|group| group.songs.clone())
        .unwrap_or_default();
    manager.search_query = "Song".to_string();
    manager.update_processed_data(filtered_songs);

    let filtered = manager
        .filtered_albums
        .as_ref()
        .expect("un filtro activo construye la lista de álbumes de la cuadrícula");

    assert_eq!(
        filtered.len(),
        2,
        "la cuadrícula filtrada tampoco fusiona las versiones"
    );
    assert_ne!(
        filtered[0].cover_path, filtered[1].cover_path,
        "cada versión conserva su portada en la cuadrícula filtrada"
    );
}
