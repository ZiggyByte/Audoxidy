//! Bookkeeping of the decoded-cover store and the decode request path.
//!
//! Every test builds byte handles and injects the enqueue closure, so no test
//! decodes a real image or writes into the cover cache directory. The single
//! decode request names a path that holds no file, so the worker reports a
//! missing file without ever decoding pixels. The counters and the in-flight
//! gauge are process-global, so each test holds the shared guard for its whole
//! body.

use super::*;
use crate::utils::covers::{GLOBAL_STATE_TEST_LOCK, cover_io_counters};
use crate::utils::test_support::TempTestDir;
use std::time::Duration;

/// Opaque stand-in for a worker-minted handle.
///
/// The store never inspects pixels, so the tests use byte handles: minting an
/// RGBA handle here would add a second `from_rgba` call site to this module.
fn fake_handle(label: &str) -> Handle {
    Handle::from_bytes(label.as_bytes().to_vec())
}

fn key(path: &str, bucket: u32) -> CoverKey {
    CoverKey {
        path: Arc::from(path),
        bucket: bucket as u16,
    }
}

fn job(path: &str, bucket: u32) -> DecodeJob {
    DecodeJob {
        key: key(path, bucket),
        path: PathBuf::from(path),
        bucket,
    }
}

/// The counters are process-global, so every test that looks a cover up or
/// enqueues a decode holds the shared guard for its whole body.
fn global_guard() -> std::sync::MutexGuard<'static, ()> {
    GLOBAL_STATE_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

#[test]
fn test_cover_cache_handle_ids_are_unique_and_stable() {
    let _guard = global_guard();

    let first = fake_handle("first");
    let second = fake_handle("second");
    assert!(first.id() != second.id());

    let mut store = CoverStore::default();
    store.insert_handle(key("/covers/a.avif", 160), first.clone(), 64);

    let stored = store
        .get_path("/covers/a.avif", 160)
        .expect("el handle insertado debe recuperarse");
    assert_eq!(stored.id(), first.id());

    let again = store
        .get_path("/covers/a.avif", 160)
        .expect("el handle insertado debe recuperarse de nuevo");
    assert!(again == first);
    assert!(store.get_path("/covers/a.avif", 220).is_none());
}

#[test]
fn test_cover_cache_records_hit_and_miss() {
    let _guard = global_guard();

    let mut store = CoverStore::default();
    store.insert_handle(key("/covers/hit.avif", 160), fake_handle("hit"), 16);

    let before = cover_io_counters();
    assert!(store.get_path("/covers/hit.avif", 160).is_some());
    assert!(store.get_path("/covers/miss.avif", 160).is_none());
    let after = cover_io_counters();

    assert_eq!(after.cache_hits - before.cache_hits, 1);
    assert_eq!(after.cache_misses - before.cache_misses, 1);
    assert_eq!(after.key_lookups - before.key_lookups, 2);
    assert_eq!(store.lookup_count(), 2);
}

#[test]
fn test_cover_cache_insert_accumulates_bytes_from_pixel_length() {
    let _guard = global_guard();

    let mut store = CoverStore::default();
    store.insert_handle(key("/covers/a.avif", 160), fake_handle("a160"), 4 * 4 * 4);
    store.insert_handle(key("/covers/a.avif", 220), fake_handle("a220"), 4 * 4 * 4);
    store.insert_handle(key("/covers/b.avif", 160), fake_handle("b160"), 2 * 2 * 4);

    assert_eq!(store.len(), 3);
    assert_eq!(store.bytes(), 4 * 4 * 4 + 4 * 4 * 4 + 2 * 2 * 4);

    // Re-inserting the same entry replaces its bytes instead of adding them.
    store.insert_handle(key("/covers/a.avif", 160), fake_handle("replaced"), 10);
    assert_eq!(store.len(), 3);
    assert_eq!(store.bytes(), 10 + 4 * 4 * 4 + 2 * 2 * 4);
    assert_eq!(cover_io_counters().cache_bytes, store.bytes() as u64);
}

#[test]
fn test_cover_cache_mark_in_flight_dedups() {
    let _guard = global_guard();

    let mut store = CoverStore::default();
    let request = job("/covers/a.avif", 160);
    let mut enqueued = 0;

    assert!(store.request_key(request.clone(), |_| {
        enqueued += 1;
        true
    }));
    assert!(!store.request_key(request.clone(), |_| {
        enqueued += 1;
        true
    }));
    assert_eq!(enqueued, 1);
    assert!(store.is_in_flight(&request.key));

    store.insert_handle(request.key.clone(), fake_handle("request"), 16);
    store.clear_in_flight(&request.key);
    assert!(!store.is_in_flight(&request.key));

    // A stored key is never requested again.
    assert!(!store.request_key(request, |_| {
        enqueued += 1;
        true
    }));
    assert_eq!(enqueued, 1);
}

#[test]
fn test_cover_request_retries_after_a_full_queue() {
    let _guard = global_guard();

    let mut store = CoverStore::default();
    let request = job("/covers/a.avif", 160);

    // The queue rejected the job, so the key must not be left in flight.
    assert!(!store.request_key(request.clone(), |_| false));
    assert!(!store.is_in_flight(&request.key));

    assert!(store.request_key(request.clone(), |_| true));
    assert!(store.is_in_flight(&request.key));

    store.clear_in_flight(&request.key);
    assert!(!store.is_in_flight(&request.key));
}

#[test]
fn test_cover_cache_order_tracks_recency_without_promoting_on_read() {
    let _guard = global_guard();

    let mut store = CoverStore::default();
    store.insert_handle(key("/covers/a.avif", 160), fake_handle("recency-a"), 16);
    store.insert_handle(key("/covers/b.avif", 160), fake_handle("recency-b"), 16);
    assert_eq!(
        store.order_mru_to_lru(),
        vec![
            "/covers/b.avif@160".to_string(),
            "/covers/a.avif@160".to_string()
        ]
    );

    // Reading through the render path does not change recency.
    assert!(store.get_path("/covers/a.avif", 160).is_some());
    assert_eq!(
        store.order_mru_to_lru(),
        vec![
            "/covers/b.avif@160".to_string(),
            "/covers/a.avif@160".to_string()
        ]
    );

    assert!(store.touch("/covers/a.avif", 160));
    assert_eq!(
        store.order_mru_to_lru(),
        vec![
            "/covers/a.avif@160".to_string(),
            "/covers/b.avif@160".to_string()
        ]
    );
}

#[test]
fn test_cover_decode_worker_reports_a_missing_file() {
    let _guard = global_guard();

    // A fresh directory guarantees the path names no file, and the guard removes
    // the directory when the test ends.
    let dir = TempTestDir::new("cover-cache-missing");
    let missing = dir.path().join("missing.cover.avif");
    let request = DecodeJob {
        key: CoverKey {
            path: Arc::from(missing.to_string_lossy().as_ref()),
            bucket: 160,
        },
        path: missing,
        bucket: 160,
    };

    assert!(request_cover_decode(request.clone()));

    let outcome = cover_decode_outcomes()
        .recv_timeout(Duration::from_secs(10))
        .expect("el worker no devolvió un resultado");

    assert_eq!(outcome.key, request.key);
    assert!(outcome.handle.is_none());
    assert!(outcome.error.is_some());
    assert_eq!(outcome.bytes, 0);
    assert_eq!(cover_work_in_flight(), 0);
}
