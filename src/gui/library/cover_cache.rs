//! Decoded cover index and its decode workers.
//!
//! The store maps a cover path and decode bucket to the one `iced` image handle
//! a worker minted for it, so a layout pass only clones a handle and never opens
//! a file, decodes an image or takes a lock. It is a plain field of the library
//! manager: `update()` mutates it, and the grid and list row builders read it
//! through `&self`.
//!
//! Requests travel over a bounded job queue consumed by dedicated
//! `cover-decode-worker-*` threads. Those threads are deliberately not the cover
//! write pool, whose workers park on its own queue for the life of the process
//! and never run a spawned job. Results return as [`CoverDecodeOutcome`] values
//! that the app forwards as the `CoverDecoded` message.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use iced::widget::image::Handle;

use crate::utils::covers::{CoverBudget, cover_metrics, decode_cover_file};

/// Job queue capacity: a scroll burst waits in the queue instead of allocating
/// one decode per card.
const DECODE_QUEUE_CAPACITY: usize = 64;

/// Decode worker stack size, matching the cover write pool for AVIF decoding.
const DECODE_WORKER_STACK_BYTES: usize = 8 * 1024 * 1024;

/// Identity of one decoded cover: the stored path and the decode bucket.
///
/// The bucket is part of the key because the same file is decoded at different
/// edges for the grid, the detailed list and the thumbnail row.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CoverKey {
    /// Stored cover path.
    pub path: Arc<str>,
    /// Decode bucket, in pixels of the cover edge.
    pub bucket: u16,
}

/// Handles minted by the decode workers, indexed by bucket and path.
///
/// `handles` is nested on purpose: `HashMap<Arc<str>, _>::get(&str)` is a
/// borrowed lookup, so a list row resolves its handle without allocating a key
/// and without taking a lock. Accounting and recency live in [`CoverBudget`].
#[derive(Debug, Default)]
pub struct CoverStore {
    handles: HashMap<u16, HashMap<Arc<str>, Handle>>,
    in_flight: HashSet<CoverKey>,
    budget: CoverBudget,
    lookups: Cell<u64>,
}

impl CoverStore {
    /// Returns the handle stored for a path and bucket.
    ///
    /// Counts the lookup and records a hit or a miss on the process-global
    /// counters, but neither promotes recency nor allocates a key.
    pub fn get_path(&self, path: &str, bucket: u16) -> Option<Handle> {
        self.lookups.set(self.lookups.get() + 1);

        let metrics = cover_metrics();
        metrics.record_key_lookup();

        let handle = self
            .handles
            .get(&bucket)
            .and_then(|by_path| by_path.get(path))
            .cloned();

        match handle {
            Some(handle) => {
                metrics.record_cache_hit();
                Some(handle)
            }
            None => {
                metrics.record_cache_miss();
                None
            }
        }
    }

    /// Returns the handle for a key, delegating to [`Self::get_path`].
    pub fn get(&self, key: &CoverKey) -> Option<Handle> {
        self.get_path(&key.path, key.bucket)
    }

    /// Returns whether a handle is already stored for a path and bucket.
    pub fn contains(&self, path: &str, bucket: u16) -> bool {
        self.handles
            .get(&bucket)
            .is_some_and(|by_path| by_path.contains_key(path))
    }

    /// Number of stored handles across every bucket.
    pub fn len(&self) -> usize {
        self.handles.values().map(HashMap::len).sum()
    }

    /// Returns whether the store holds no handle.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Decoded bytes the store currently retains, from the budget ledger.
    pub fn bytes(&self) -> usize {
        self.budget.bytes()
    }

    /// Number of lookups performed through this store.
    pub fn lookup_count(&self) -> u64 {
        self.lookups.get()
    }

    /// Returns whether a decode for this key is already queued or running.
    pub fn is_in_flight(&self, key: &CoverKey) -> bool {
        self.in_flight.contains(key)
    }

    /// Clears the in-flight mark once a decode result has been handled.
    pub fn clear_in_flight(&mut self, key: &CoverKey) {
        self.in_flight.remove(key);
    }

    /// Queues a decode for a key that is neither stored nor already in flight.
    ///
    /// `enqueue` runs before the key is marked, so a rejected enqueue (a full
    /// queue) leaves the key retryable instead of permanently in flight.
    pub fn request_key(
        &mut self,
        job: DecodeJob,
        enqueue: impl FnOnce(&DecodeJob) -> bool,
    ) -> bool {
        if self.contains(&job.key.path, job.key.bucket) || self.in_flight.contains(&job.key) {
            return false;
        }

        if !enqueue(&job) {
            return false;
        }

        self.in_flight.insert(job.key.clone());
        true
    }

    /// Stores the handle a worker minted and records the bytes it retains.
    ///
    /// The store never builds a handle: the decode worker mints one RGBA handle
    /// per entry, which keeps the image id stable for the life of the entry.
    pub fn insert_handle(&mut self, key: CoverKey, handle: Handle, bytes: usize) {
        self.budget.insert(&key.path, key.bucket, bytes);

        let CoverKey { path, bucket } = key;
        self.handles.entry(bucket).or_default().insert(path, handle);

        cover_metrics().set_cache_bytes(self.budget.bytes());
    }

    /// Promotes a stored entry to most recently used; returns whether it exists.
    pub fn touch(&mut self, path: &str, bucket: u16) -> bool {
        self.budget.touch(path, bucket)
    }

    /// Stored entries as `path@bucket`, most recently used first.
    pub fn order_mru_to_lru(&self) -> Vec<String> {
        self.budget.order_mru_to_lru()
    }
}

/// One decode request handed to the worker queue.
#[derive(Debug, Clone)]
pub struct DecodeJob {
    /// Key the result is stored under.
    pub key: CoverKey,
    /// File to decode.
    pub path: PathBuf,
    /// Decode bucket, in pixels of the cover edge.
    pub bucket: u32,
    /// Scroll generation that requested the decode; a result stamped with an
    /// older generation is stale.
    pub generation: u64,
}

/// Result of one decode job.
#[derive(Clone)]
pub struct CoverDecodeOutcome {
    /// Key the handle belongs to.
    pub key: CoverKey,
    /// Scroll generation of the request, echoed for staleness checks.
    pub generation: u64,
    /// Minted handle, or `None` when the decode failed.
    pub handle: Option<Handle>,
    /// Bytes the handle retains when present.
    pub bytes: usize,
    /// Failure description when the decode failed.
    pub error: Option<String>,
}

// `iced` image handles do not implement `Debug`; the outcome reports whether a
// handle is present instead of trying to print it, so the message can derive it.
impl std::fmt::Debug for CoverDecodeOutcome {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CoverDecodeOutcome")
            .field("key", &self.key)
            .field("generation", &self.generation)
            .field("has_handle", &self.handle.is_some())
            .field("bytes", &self.bytes)
            .field("error", &self.error)
            .finish()
    }
}

/// Bounded job queue and unbounded result channel behind the decode workers.
struct DecodePipeline {
    jobs: crossbeam::channel::Sender<DecodeJob>,
    outcomes: crossbeam::channel::Receiver<CoverDecodeOutcome>,
}

static DECODE_PIPELINE: OnceLock<DecodePipeline> = OnceLock::new();

/// Returns the decode pipeline, starting its workers on first use.
fn decode_pipeline() -> &'static DecodePipeline {
    DECODE_PIPELINE.get_or_init(|| {
        let (job_sender, job_receiver) =
            crossbeam::channel::bounded::<DecodeJob>(DECODE_QUEUE_CAPACITY);
        let (outcome_sender, outcome_receiver) =
            crossbeam::channel::unbounded::<CoverDecodeOutcome>();

        let worker_count = if crate::utils::is_low_resource() {
            1
        } else {
            2
        };

        for index in 0..worker_count {
            let jobs = job_receiver.clone();
            let outcomes = outcome_sender.clone();

            std::thread::Builder::new()
                .name(format!("cover-decode-worker-{index}"))
                .stack_size(DECODE_WORKER_STACK_BYTES)
                .spawn(move || decode_worker_loop(jobs, outcomes))
                .expect("failed to spawn a cover decode worker");
        }

        drop(outcome_sender);

        DecodePipeline {
            jobs: job_sender,
            outcomes: outcome_receiver,
        }
    })
}

/// Queues a decode job without blocking the caller.
///
/// Uses `try_send` only: the update thread must never wait on a decode backlog.
/// Returns `false` when the queue is full or the workers are gone.
pub fn request_cover_decode(job: DecodeJob) -> bool {
    decode_pipeline().jobs.try_send(job).is_ok()
}

/// Returns the channel the app bridges into its message loop.
pub fn cover_decode_outcomes() -> &'static crossbeam::channel::Receiver<CoverDecodeOutcome> {
    &decode_pipeline().outcomes
}

/// Number of decodes currently running on a worker.
pub fn cover_work_in_flight() -> u64 {
    cover_metrics().snapshot().in_flight
}

/// Drains the job queue for the life of the process.
///
/// Each job decodes off the UI thread and produces the only handle minted for its
/// key. The in-flight gauge is bumped before the decode and released on both the
/// success and the failure path.
fn decode_worker_loop(
    jobs: crossbeam::channel::Receiver<DecodeJob>,
    outcomes: crossbeam::channel::Sender<CoverDecodeOutcome>,
) {
    while let Ok(job) = jobs.recv() {
        let metrics = cover_metrics();
        metrics.increment_in_flight();

        // A decoder panic must neither take the worker down nor leave the gauge
        // raised, so the decode is caught and reported as a failed outcome.
        let decoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            decode_cover_file(&job.path, job.bucket)
        }));

        metrics.decrement_in_flight();

        let outcome = match decoded {
            Ok(Ok(cover)) => {
                let bytes = cover.pixels.len();
                // The single handle mint for this entry: the worker owns it, so a
                // clone across the channel is an `Arc` bump.
                let handle = Handle::from_rgba(cover.width, cover.height, cover.pixels);

                CoverDecodeOutcome {
                    key: job.key,
                    generation: job.generation,
                    handle: Some(handle),
                    bytes,
                    error: None,
                }
            }
            Ok(Err(error)) => CoverDecodeOutcome {
                key: job.key,
                generation: job.generation,
                handle: None,
                bytes: 0,
                error: Some(error.to_string()),
            },
            Err(_) => CoverDecodeOutcome {
                key: job.key,
                generation: job.generation,
                handle: None,
                bytes: 0,
                error: Some("the cover decode panicked".to_string()),
            },
        };

        if outcomes.send(outcome).is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::covers::{GLOBAL_STATE_TEST_LOCK, cover_io_counters};
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
            generation: 0,
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

        let missing = std::env::temp_dir().join(format!(
            "audoxidy-missing-cover-{}-{}.avif",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        ));
        let request = DecodeJob {
            key: CoverKey {
                path: Arc::from(missing.to_string_lossy().as_ref()),
                bucket: 160,
            },
            path: missing,
            bucket: 160,
            generation: 7,
        };

        assert!(request_cover_decode(request.clone()));

        let outcome = cover_decode_outcomes()
            .recv_timeout(Duration::from_secs(10))
            .expect("el worker no devolvió un resultado");

        assert_eq!(outcome.key, request.key);
        assert_eq!(outcome.generation, request.generation);
        assert!(outcome.handle.is_none());
        assert!(outcome.error.is_some());
        assert_eq!(outcome.bytes, 0);
        assert_eq!(cover_work_in_flight(), 0);
    }
}
