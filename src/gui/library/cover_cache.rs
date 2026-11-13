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
//!
//! The grid window builder memoizes its row elements in [`GridRowsMemo`], so a
//! layout pass that changes no geometry clones one `Rc` and the stored spacers
//! instead of rebuilding the rows and resolving a handle per card.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;

use iced::widget::image::Handle;

use super::grid_metrics::{self, GridMetrics};
use super::state::LibraryManager;
use super::types::LibraryViewMode;
use crate::gui::dimensions::LIST_ROW_HEADER;
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

    /// Sets the retained-byte target the next eviction enforces.
    pub fn set_budget(&mut self, bytes: usize) {
        self.budget.set_budget(bytes);
        cover_metrics().set_cache_bytes(self.budget.bytes());
    }

    /// Retained-byte target currently enforced.
    pub fn budget(&self) -> usize {
        self.budget.budget()
    }

    /// Replaces the protected window with the keys the viewport needs.
    ///
    /// The ledger promotes the keys it already holds and only records the rest,
    /// so a look-ahead request becomes protected as soon as its decode lands.
    pub fn set_window(&mut self, keys: impl IntoIterator<Item = (String, u16)>) {
        self.budget.set_window(keys);
    }

    /// Returns whether the current window needs a path and bucket.
    pub fn is_window(&self, path: &str, bucket: u16) -> bool {
        self.budget.is_window(path, bucket)
    }

    /// Drops least-recently-used handles outside the window until the retained
    /// bytes fit the budget; returns how many handles were dropped.
    pub fn evict_to_budget(&mut self) -> usize {
        let removed = self.budget.evict_to_budget();
        self.drop_handles(&removed);
        removed.len()
    }

    /// Drops every handle outside the window; returns how many were dropped.
    ///
    /// The collection pass does not call this: retention is the byte budget's
    /// job, so the covers just scrolled past stay available for a scroll back
    /// until the budget evicts them. This is the hard trim, kept for tests.
    pub fn trim_outside_window(&mut self) -> usize {
        let removed = self.budget.trim_outside_window();
        self.drop_handles(&removed);
        removed.len()
    }

    /// Removes the handles of keys the ledger already dropped and republishes
    /// the retained bytes, so the gauge never lags behind an eviction.
    fn drop_handles(&mut self, keys: &[(String, u16)]) {
        for (path, bucket) in keys {
            let emptied = match self.handles.get_mut(bucket) {
                Some(by_path) => {
                    by_path.remove(path.as_str());
                    by_path.is_empty()
                }
                None => false,
            };

            if emptied {
                self.handles.remove(bucket);
            }

            cover_metrics().record_eviction();
        }

        cover_metrics().set_cache_bytes(self.budget.bytes());
    }
}

/// Milliseconds of scroll idle after which the look-ahead band is requested.
///
/// A burst of wheel or trackpad events never requests a decode: only a pause of
/// this length does.
pub const SCROLL_IDLE_MS: u64 = 250;

/// Y offset of the look-ahead band one screen ahead in the scroll direction.
///
/// `direction` is the sign of the last scroll delta (`1.0` downward, `-1.0`
/// upward) and `screens` how far ahead to look, both fixed by policy. The offset
/// is resolved to the row that contains it, so a band landing inside a row still
/// requests that row. An empty library, a resting or non-finite direction and a
/// band starting past the last row yield `None`.
// The inputs are the exact geometry the row math already needs; grouping them
// into a struct would only move the same list to another module.
#[allow(clippy::too_many_arguments)]
pub fn prefetch_band(
    metrics: &GridMetrics,
    album_count: usize,
    expanded_row: Option<usize>,
    rendered_songs: usize,
    viewport_y: f32,
    viewport_h: f32,
    direction: f32,
    screens: f32,
) -> Option<f32> {
    if !viewport_y.is_finite()
        || !viewport_h.is_finite()
        || viewport_h <= 0.0
        || !direction.is_finite()
        || direction == 0.0
        || !screens.is_finite()
        || screens <= 0.0
    {
        return None;
    }

    let band_y = viewport_y
        + if direction < 0.0 {
            -viewport_h * screens
        } else {
            viewport_h * screens
        };

    let total =
        grid_metrics::total_content_height(metrics, album_count, expanded_row, rendered_songs);
    if total <= 0.0 || band_y >= total {
        return None;
    }

    let (_, top) =
        grid_metrics::row_at_offset(metrics, album_count, expanded_row, rendered_songs, band_y);

    Some(top)
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

/// One element of the memoized grid window.
///
/// The set carries everything the grid draws: the album cards with a handle
/// already resolved for each one, and the expanded album's song block, which must
/// survive every memo hit unchanged.
#[derive(Debug, Clone)]
pub enum GridCoverElement {
    /// One row of album cards.
    AlbumRow {
        /// Albums of the row.
        albums: Vec<crate::gui::library::AlbumEntry>,
        /// Handle per album, aligned with `albums`; `None` until its decode lands.
        handles: Vec<Option<Handle>>,
        /// Composite id of the expanded album when it sits in this row.
        active_expansion_id: Option<String>,
    },
    /// The song block of the expanded album.
    ExpandedSongs {
        /// Composite id of the expanded album.
        album_id: String,
        /// Rendered songs with their index inside the snapshot list.
        songs: Vec<(Arc<crate::db::database::SongData>, usize)>,
        /// Height of the songs above the render window.
        top_space: f32,
        /// Height of the songs below the render window.
        bottom_space: f32,
    },
}

/// Memoized grid window: the geometry it was built for and every value it returns.
///
/// Storing the spacers beside the row slice is deliberate. The grid renders the
/// accumulated heights of the rows outside the render window, so a hit that
/// returned zero for either spacer would collapse the content height and shift
/// the scroll position.
#[derive(Debug)]
pub struct GridRowsMemo {
    /// Geometry the rows were built with.
    pub metrics: GridMetrics,
    /// Viewport top the rows were built for.
    pub viewport_y: f32,
    /// Viewport height the rows were built for.
    pub viewport_h: f32,
    /// Decode bucket of the resolved handles.
    pub bucket: u32,
    /// Height of the rows above the render window.
    pub top_space: f32,
    /// Height of the rows below the render window.
    pub bottom_space: f32,
    /// Row elements shared by every layout pass with the same key.
    pub elements: Rc<[GridCoverElement]>,
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
            Ok(Err(error)) => {
                // A file that is present and still undecodable is recorded, so the
                // request window stops asking for it; a missing or unreadable file
                // stays retryable because a write may still land.
                crate::utils::covers::record_negative_cover(&job.path, &error);

                CoverDecodeOutcome {
                    key: job.key,
                    generation: job.generation,
                    handle: None,
                    bytes: 0,
                    error: Some(error.to_string()),
                }
            }
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

impl LibraryManager {
    /// Requests a decode for every job whose key is neither stored nor in flight.
    ///
    /// `enqueue` is the queue seam: production passes the global decode queue and
    /// a test passes a counting closure, so the request policy runs without a
    /// worker. Returns the number of accepted jobs.
    pub fn request_jobs_with(
        &mut self,
        jobs: Vec<DecodeJob>,
        mut enqueue: impl FnMut(&DecodeJob) -> bool,
    ) -> usize {
        let mut accepted = 0;

        for job in jobs {
            if self.cover_store.request_key(job, |job| enqueue(job)) {
                accepted += 1;
            }
        }

        accepted
    }

    /// Stores a decode outcome and returns whether its handle was kept.
    ///
    /// The in-flight mark is always released, so a failed key stays retryable for
    /// the next request. A successful handle is always stored: the decode already
    /// paid for its file open and its pixels, and the retained-byte budget decides
    /// how long the entry survives, so a scroll that outran the decoder can never
    /// force the same cover to be decoded again. The generation stamped on the
    /// request therefore gates requests, never retention.
    ///
    /// The memoized grid rows are invalidated only when the stored key belongs to
    /// the requested window: an entry outside it cannot change a single card, and
    /// invalidating for it would rebuild every row once per finished decode. The
    /// requested window includes the look-ahead band, so a band entry rebuilds the
    /// rows once even when no card draws it; that is the price of keeping it
    /// protected, and it ends with the band.
    pub fn apply_cover_decoded(&mut self, outcome: CoverDecodeOutcome) -> bool {
        self.cover_store.clear_in_flight(&outcome.key);

        let Some(handle) = outcome.handle else {
            if let Some(error) = outcome.error.as_deref() {
                tracing::debug!(
                    path = %outcome.key.path,
                    error,
                    "No se pudo decodificar una carátula"
                );
            }
            return false;
        };

        let in_window = self
            .cover_store
            .is_window(&outcome.key.path, outcome.key.bucket);

        self.cover_store
            .insert_handle(outcome.key, handle, outcome.bytes);

        if in_window {
            self.cached_grid_rows.replace(None);
        }

        self.cover_store.evict_to_budget();

        true
    }

    /// Requests the decode of every cover the current viewport shows.
    ///
    /// The visible rows already resolve to a handle; this only asks for the paths
    /// whose handle is still missing. A full queue drops the request and leaves the
    /// key retryable for the next scroll or decode completion.
    pub fn request_cover_window(&mut self) {
        self.request_cover_window_with(|job| request_cover_decode(job.clone()));
    }

    /// Requests the current window and its look-ahead band through `enqueue`.
    ///
    /// The visible window is built first and always keeps its room: the band one
    /// screen ahead in the scroll direction is capped to the entries the retained
    /// budget can still hold, and skipped when the window fills it. A band key the
    /// window already requests is left out, so the room is spent on rows the
    /// viewport does not cover yet.
    ///
    /// The budget bounds the band, never the clamp alone: the ceiling is a hard cap
    /// on the budget, while the budget is the bytes the store may actually keep. A
    /// band sized against the ceiling would protect more keys than the store can
    /// retain, and the surplus would be evicted and requested again.
    ///
    /// The protected set is the final job set, band included, and the retained-byte
    /// budget is raised to the bytes that set occupies. Both are required for the
    /// request to converge: a look-ahead key that is not protected is evicted by the
    /// decode that just landed it, and the next request asks for it again, opening
    /// and decoding the same file forever. `enqueue` is the queue seam: production
    /// passes the global decode queue and a test a counting closure. Returns the
    /// keys the pass asked the store to retain, in request order.
    pub fn request_cover_window_with(
        &mut self,
        enqueue: impl FnMut(&DecodeJob) -> bool,
    ) -> Vec<CoverKey> {
        let height = self
            .last_viewport
            .map(|viewport| viewport.height)
            .unwrap_or(800.0);
        let metrics = self.grid_metrics.get();
        // The budget follows the bucket the current mode renders, so a list mode
        // sizes its retained bytes by the covers its rows actually use.
        let bucket = self.window_bucket();
        let bucket_bytes = (bucket as usize)
            .saturating_mul(bucket as usize)
            .saturating_mul(4);
        let capacity = match self.view_mode {
            LibraryViewMode::Grid => {
                crate::utils::covers::viewport_capacity(metrics.columns, height, metrics.row_height)
            }
            _ => crate::utils::covers::viewport_capacity(1, height, LIST_ROW_HEADER),
        };
        // The budget is the retention this pass can protect, so it is computed before
        // the jobs: the window and its band together must fit inside it. The hard
        // ceiling only clamps the budget; it is not spare room the band may spend.
        let budget_target = crate::utils::covers::cover_budget_bytes(capacity, bucket);
        let budget_entries = budget_target / bucket_bytes.max(1);

        let mut jobs = self.window_jobs(self.scroll_offset.y, height);
        let window_len = jobs.len();

        let screens = if crate::utils::is_low_resource() {
            0.5
        } else {
            1.0
        };
        let albums = self
            .filtered_albums
            .as_ref()
            .or(self.cached_albums.as_ref());
        let album_count = albums.map_or(0, Vec::len);
        let expanded_row =
            albums.and_then(|albums| self.expanded_album_row(albums, metrics.columns));
        let rendered_songs = self.rendered_expanded_song_count();

        if let Some(band_y) = prefetch_band(
            &metrics,
            album_count,
            expanded_row,
            rendered_songs,
            self.scroll_offset.y,
            height,
            self.scroll_direction.signum(),
            screens,
        ) {
            // The band renders with its own margin, so it repeats rows the window
            // already reaches past the viewport. Those keys are already requested
            // and must not spend the band's room.
            let window_keys: HashSet<CoverKey> = jobs.iter().map(|job| job.key.clone()).collect();
            let room = budget_entries.saturating_sub(window_len);

            jobs.extend(
                self.window_jobs(band_y, height)
                    .into_iter()
                    .filter(|job| !window_keys.contains(&job.key))
                    .take(room),
            );
        }

        let requested: Vec<CoverKey> = jobs.iter().map(|job| job.key.clone()).collect();

        // Every key this pass requests is protected, band included: a requested
        // entry must survive the decode that lands it, or the next pass asks for it
        // again.
        self.cover_store.set_window(
            requested
                .iter()
                .map(|key| (key.path.to_string(), key.bucket)),
        );

        // The budget is at least the bytes the requested set retains, clamped to the
        // ceiling so a huge viewport cannot retain an unbounded amount of host RGBA.
        // The visible window wins: the band is what gets truncated, so a set inside
        // the ceiling always fits its own budget.
        let budget = budget_target
            .max(jobs.len().saturating_mul(bucket_bytes))
            .clamp(
                crate::utils::covers::COVER_BUDGET_MIN_BYTES,
                crate::utils::covers::COVER_BUDGET_MAX_BYTES,
            );
        self.cover_store.set_budget(budget);

        let accepted = self.request_jobs_with(jobs, enqueue);
        self.cover_store.evict_to_budget();

        tracing::trace!(
            target: "audoxidy::covers",
            cover_requested_keys = requested.len(),
            cover_accepted_jobs = accepted,
            cover_retained_budget_bytes = budget,
            "cover window request"
        );

        requested
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
