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

use super::cover_cache::{CoverKey, GridCoverElement};
use super::grid_metrics::grid_metrics;
use super::state::LibraryManager;
use super::types::AlbumEntry;
use crate::db::database::SongData;
use crate::utils::covers::{GLOBAL_STATE_TEST_LOCK, clear_negative_cover_cache};

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
