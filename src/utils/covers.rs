//! Cover art cache and image processing.
//!
//! Scales images to 400x400, compresses them as AVIF at 90% quality,
//! and caches them on disk under `cache/covers/`. An in-memory LRU
//! cache retains cover handles  for the GUI without importing any
// !front-end types.
//!
//! The [`CoverHandle`] type replaces iced handles in this module so
//! that `utils/` has zero front-end imports. The GUI adapter wraps a
//! `CoverHandle` into an iced handle at render time.

use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::codecs::avif::AvifEncoder;
use image::{ExtendedColorType, ImageEncoder};
use lru::LruCache;
use parking_lot::Mutex;
use rayon::ThreadPoolBuilder;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Front-end-agnostic handle to a cover art image.
///
/// Stores either a file path (AVIF on disk) or raw image bytes
/// (embedded fallback). The `stable_id` is a SHA-256 digest used
/// as the LRU cache key for identity comparison.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CoverHandle {
    /// File-backed cover: path to the AVIF on disk.
    File { path: PathBuf, stable_id: [u8; 32] },
    /// Byte-backed cover: raw image data held in memory.
    Bytes { data: Vec<u8>, stable_id: [u8; 32] },
}

impl CoverHandle {
    /// Computes the SHA-256 stable ID from a byte slice.
    pub fn stable_id_from_bytes(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        id
    }

    /// Returns the stable ID for cache keying.
    pub fn stable_id(&self) -> [u8; 32] {
        match self {
            CoverHandle::File { stable_id, .. } => *stable_id,
            CoverHandle::Bytes { stable_id, .. } => *stable_id,
        }
    }

    /// Returns the file path if this is a file-backed handle.
    pub fn path(&self) -> Option<&Path> {
        match self {
            CoverHandle::File { path, .. } => Some(path),
            CoverHandle::Bytes { .. } => None,
        }
    }

    /// Returns the raw bytes if this is a byte-backed handle.
    pub fn bytes(&self) -> Option<&[u8]> {
        match self {
            CoverHandle::File { .. } => None,
            CoverHandle::Bytes { data, .. } => Some(data),
        }
    }
}

/// Static instance of the thread pool for background cover processing.
pub static COVER_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

/// Bounded channel to control memory pressure during scanning.
static COVER_GATEWAY: OnceLock<crossbeam::channel::Sender<(Vec<u8>, String)>> = OnceLock::new();

/// Stores file paths that previously failed to load (avoids retries).
pub static NEGATIVE_CACHE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

/// LRU cache for cover handles, preventing excessive memory use.
pub struct CoverCache {
    /// Entries ordered by recency (most recent at the front).
    entries: LruCache<String, CoverHandle>,
}

impl CoverCache {
    /// Crea la caché con la capacidad indicada. La capacidad debe ser > 0.
    fn new(cap: usize) -> Self {
        Self {
            entries: LruCache::new(
                NonZeroUsize::new(cap).expect("la capacidad debe ser mayor que cero"),
            ),
        }
    }

    /// Returns the handle and promotes the entry to most-recent position.
    fn get(&mut self, key: &str) -> Option<CoverHandle> {
        self.entries.get(key).cloned()
    }

    /// Peeks at an entry without altering recency order.
    #[cfg(test)]
    fn peek(&self, key: &str) -> Option<CoverHandle> {
        self.entries.peek(key).cloned()
    }

    /// Inserts or updates an entry, promoting it to most-recent position.
    fn put(&mut self, key: String, value: CoverHandle) {
        self.entries.put(key, value);
    }

    /// Promueve una clave existente sin insertarla si no está presente.
    fn promote(&mut self, key: &str) -> bool {
        self.entries.promote(key)
    }

    /// Ajusta la capacidad; al reducir desaloja primero las menos recientes.
    fn resize(&mut self, cap: usize) {
        if self.entries.cap().get() != cap {
            self.entries
                .resize(NonZeroUsize::new(cap).expect("la capacidad debe ser mayor que cero"));
        }
    }

    /// Number of entries retained.
    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    /// Checks whether the key is in the cache without altering recency.
    fn contains(&self, key: &str) -> bool {
        self.entries.contains(key)
    }

    /// Most-recent to least-recent order.
    #[cfg(test)]
    fn order_mru_to_lru(&self) -> Vec<String> {
        self.entries.iter().map(|(key, _)| key.clone()).collect()
    }
}

/// Dynamic cover cache limit based on mode:
/// - Normal: 64 (standard, sufficient for large displays)
/// - Low-resource: 16 (aggressive eviction to save RAM)
fn get_max_covers() -> usize {
    if crate::utils::is_low_resource() {
        16
    } else {
        64
    }
}

pub static LRU_COVER_CACHE: OnceLock<Mutex<CoverCache>> = OnceLock::new();

/// Returns or initializes the global LRU cover cache.
pub fn get_lru_cache() -> &'static Mutex<CoverCache> {
    LRU_COVER_CACHE.get_or_init(|| Mutex::new(CoverCache::new(get_max_covers())))
}

/// Returns or initializes the global thread pool for cover processing.
///
/// Uses 1 thread in low-resource mode; 1/3 of cores in normal mode.
pub fn get_cover_pool() -> &'static rayon::ThreadPool {
    COVER_POOL.get_or_init(|| {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        // Low-resource: max 1 thread. Normal: 1/3 of processors (to prevent overheating).
        let pool_size = if crate::utils::is_low_resource() {
            1
        } else {
            (cores / 3).max(1)
        };

        ThreadPoolBuilder::new()
            .num_threads(pool_size)
            .stack_size(8 * 1024 * 1024) // 8MB per thread — stability with AVIF/ravif
            .thread_name(|idx| format!("cover-cache-worker-{}", idx))
            .build()
            .unwrap()
    })
}

/// Enqueues a cover processing job safely.
/// When too many jobs are pending (16), the scanner thread waits,
/// preventing thousands of image buffers from being cloned into RAM simultaneously.
pub fn enqueue_cover_job(data: Vec<u8>, hash: String) {
    let tx = COVER_GATEWAY.get_or_init(|| {
        let (tx, rx) = crossbeam::channel::bounded::<(Vec<u8>, String)>(16);

        let pool = get_cover_pool();

        // Spawn persistent worker threads in the pool that consume from the channel.
        for _ in 0..pool.current_num_threads() {
            let rx_worker = rx.clone();
            pool.spawn(move || {
                while let Ok((pic, name)) = rx_worker.recv() {
                    let _ = std::panic::catch_unwind(move || {
                        if let Err(e) = process_and_save_cover(&pic, &name) {
                            tracing::warn!(
                                "No se pudo procesar/guardar la carátula '{}': {}",
                                name,
                                e
                            );
                        }
                    });
                }
            });
        }

        tx
    });

    let _ = tx.send((data, hash));
}

/// Clears the set of file paths that previously failed to load.
fn clear_negative_cache(set: &mut HashSet<String>) {
    set.clear();
}

/// Clears the negative cache (failed file paths). Useful after a re-scan.
pub fn clear_all_cover_cache() {
    if let Some(neg_cache_mtx) = NEGATIVE_CACHE.get() {
        clear_negative_cache(&mut neg_cache_mtx.lock());
    }
}

/// Generates a unique hash for an image from raw bytes.
pub fn generate_pic_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Returns the CPU extension to force, or `None` to keep the library default
/// (which already selects the best available extension at runtime).
///
/// Only forces AVX2 when the processor supports it; otherwise nothing is touched.
fn forced_cpu_extension() -> Option<fast_image_resize::CpuExtensions> {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx2") {
            return Some(fast_image_resize::CpuExtensions::Avx2);
        }
    }
    None
}

/// Extracts from a raw image buffer, crops/scales to 400x400, and saves as .avif at 90% quality.
pub fn process_and_save_cover(data: &[u8], safe_album_name: &str) -> std::io::Result<PathBuf> {
    let cache_dir = PathBuf::from("cache/covers");
    if !cache_dir.exists() {
        std::fs::create_dir_all(&cache_dir)?;
    }

    let dst_path = cache_dir.join(format!("{}.avif", safe_album_name));
    if dst_path.exists() {
        return Ok(dst_path);
    }

    // 1. Load image into memory and convert generically to RGBA 8-bit.
    let img = match image::load_from_memory(data) {
        Ok(i) => i.to_rgba8(),
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Failed to decode image",
            ));
        }
    };

    let width = img.width();
    let height = img.height();

    if width == 0 || height == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid dimensions: 0",
        ));
    }

    let target_size = if crate::utils::is_low_resource() {
        200
    } else {
        400
    };

    let src_image = Image::from_vec_u8(
        width,
        height,
        img.into_raw(),
        fast_image_resize::PixelType::U8x4,
    )
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    let dst_width = target_size;
    let dst_height = target_size;
    let mut dst_image = Image::new(dst_width, dst_height, src_image.pixel_type());

    let mut resizer = Resizer::new();

    if let Some(ext) = forced_cpu_extension() {
        // SAFETY: `forced_cpu_extension` only returns `Avx2` after checking at
        // runtime with `is_x86_feature_detected!("avx2")` that the processor
        // supports it, so the extension is guaranteed at this point.
        unsafe {
            resizer.set_cpu_extensions(ext);
        }
    }

    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));

    resizer
        .resize(&src_image, &mut dst_image, &options)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    // Save as AVIF with quality 90 and fast speed (8) to save RAM/CPU.
    let file = std::fs::File::create(&dst_path)?;
    let encoder = AvifEncoder::new_with_speed_quality(file, 8, 90);

    encoder
        .write_image(
            dst_image.buffer(),
            dst_width,
            dst_height,
            ExtendedColorType::Rgba8,
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    Ok(dst_path)
}

/// Loads a cover art handle from a file path on disk.
///
/// Checks the negative cache first, then the LRU cache, and finally
/// constructs a new file-backed handle. The AVIF decoding is deferred
/// to the GUI adapter that wraps the handle into an iced type.
pub fn load_cover_handle(path: &str) -> Option<CoverHandle> {
    let neg_cache_mtx = NEGATIVE_CACHE.get_or_init(|| Mutex::new(HashSet::new()));

    // 1. Check if this path previously failed to load.
    if neg_cache_mtx.lock().contains(path) {
        return None;
    }

    // 2. Check the in-memory LRU cache.
    let cache_mtx = get_lru_cache();
    let mut cache = cache_mtx.lock();

    // Recalculate dynamic limit on each load (mode may have changed);
    // reducing evicts least-recent entries first.
    cache.resize(get_max_covers());

    if let Some(handle) = cache.get(path) {
        return Some(handle);
    }

    // 3. Create a new file-backed handle and store in the cache;
    // `put` evicts if capacity is exceeded.
    let stable_id = CoverHandle::stable_id_from_bytes(path.as_bytes());
    let handle = CoverHandle::File {
        path: PathBuf::from(path),
        stable_id,
    };
    cache.put(path.to_string(), handle.clone());

    Some(handle)
}

/// Evicts the least-recent entries from the cache.
/// In low-resource mode the effective eviction count is doubled.
fn purge_oldest_covers(cache: &mut CoverCache, count: usize, low_resource: bool) {
    let effective_count = if low_resource { count * 2 } else { count };
    let to_remove = effective_count.min(cache.entries.len());

    for _ in 0..to_remove {
        cache.entries.pop_lru();
    }
}

/// Purges the oldest entries from the cover cache.
/// Called by the Garbage Collector when the app is inactive.
/// In low-resource mode, purges double the entries per cycle.
pub fn purge_old_covers(count: usize) {
    let cache_mtx = get_lru_cache();
    let mut cache = cache_mtx.lock();
    purge_oldest_covers(&mut cache, count, crate::utils::is_low_resource());
}

/// Preloads cover art for a list of visible album paths.
///
/// Useful to avoid blank frames when scrolling through the library:
/// covers are loaded into the LRU cache before the user sees them.
pub fn preload_visible_covers(paths: &[String]) {
    if paths.is_empty() {
        return;
    }
    let cache_mtx = get_lru_cache();
    let mut cache = cache_mtx.lock();
    for path in paths {
        if cache.contains(path) {
            // Already cached: promote to MRU without reinserting.
            cache.promote(path);
        } else {
            // Not cached: load from disk and store in LRU.
            let stable_id = CoverHandle::stable_id_from_bytes(path.as_bytes());
            let handle = CoverHandle::File {
                path: PathBuf::from(path),
                stable_id,
            };
            cache.put(path.clone(), handle);
        }
    }
}

/// Loads a cover from raw image bytes (fallback for embedded album art).
///
/// Returns a byte-backed [`CoverHandle`]. The GUI adapter wraps the
/// bytes into an iced handle at render time.
pub fn load_cover_from_bytes(data: &[u8]) -> Option<CoverHandle> {
    if data.is_empty() {
        return None;
    }
    let stable_id = CoverHandle::stable_id_from_bytes(data);
    Some(CoverHandle::Bytes {
        data: data.to_vec(),
        stable_id,
    })
}

/// Vacía por completo la caché LRU de carátulas.
fn clear_cover_cache(cache: &mut CoverCache) {
    cache.entries.clear();
}

/// Clears the entire in-memory LRU cover cache (called on song change).
pub fn clear_raw_cache() {
    // Clear the full LRU on cleanup.
    let cache_mtx = get_lru_cache();
    let mut cache = cache_mtx.lock();
    clear_cover_cache(&mut cache);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, VecDeque};

    /// Deterministic file-backed cover handle for testing.
    fn test_handle(i: usize) -> CoverHandle {
        let path = PathBuf::from(format!("cache/covers/{i}.avif"));
        let stable_id = CoverHandle::stable_id_from_bytes(path.to_str().unwrap().as_bytes());
        CoverHandle::File { path, stable_id }
    }

    /// Minimal operations the parity harness needs from each implementation.
    trait ParityCache {
        fn get(&mut self, key: &str) -> Option<CoverHandle>;
        fn peek(&self, key: &str) -> Option<CoverHandle>;
        fn insert(&mut self, key: String, value: CoverHandle);
        fn clear(&mut self);
        fn purge_oldest(&mut self, count: usize, low_resource: bool);
        fn resize(&mut self, cap: usize);
        fn len(&self) -> usize;
        fn contains(&self, key: &str) -> bool;
        /// Orden de más reciente a menos reciente.
        fn order_mru_to_lru(&self) -> Vec<String>;
    }

    /// Adaptador de paridad sobre la caché de producción (respaldada por `lru`).
    struct ProductionCache {
        cache: CoverCache,
    }

    impl ProductionCache {
        fn new(cap: usize) -> Self {
            Self {
                cache: CoverCache::new(cap),
            }
        }
    }

    impl ParityCache for ProductionCache {
        fn get(&mut self, key: &str) -> Option<CoverHandle> {
            self.cache.get(key)
        }

        fn peek(&self, key: &str) -> Option<CoverHandle> {
            self.cache.peek(key)
        }

        fn insert(&mut self, key: String, value: CoverHandle) {
            self.cache.put(key, value);
        }

        fn clear(&mut self) {
            clear_cover_cache(&mut self.cache);
        }

        fn purge_oldest(&mut self, count: usize, low_resource: bool) {
            purge_oldest_covers(&mut self.cache, count, low_resource);
        }

        fn resize(&mut self, cap: usize) {
            self.cache.resize(cap);
        }

        fn len(&self) -> usize {
            self.cache.len()
        }

        fn contains(&self, key: &str) -> bool {
            self.cache.contains(key)
        }

        fn order_mru_to_lru(&self) -> Vec<String> {
            self.cache.order_mru_to_lru()
        }
    }

    /// Freezes the semantics of the manual cache that preceded the
    /// adoption of `lru`. Independent reference: a real divergence in
    /// capacity, recency, eviction, purge, or resize will fail the test.
    struct ReferenceCache {
        map: HashMap<String, CoverHandle>,
        order: VecDeque<String>,
        cap: usize,
    }

    impl ReferenceCache {
        fn new(cap: usize) -> Self {
            Self {
                map: HashMap::with_capacity(cap),
                order: VecDeque::with_capacity(cap),
                cap,
            }
        }
    }

    impl ParityCache for ReferenceCache {
        fn get(&mut self, key: &str) -> Option<CoverHandle> {
            let handle = self.map.get(key).cloned()?;
            // A hit promotes: the key moves to the tail of the queue.
            if let Some(idx) = self.order.iter().position(|x| x == key) {
                self.order.remove(idx);
                self.order.push_back(key.to_string());
            }
            Some(handle)
        }

        fn peek(&self, key: &str) -> Option<CoverHandle> {
            self.map.get(key).cloned()
        }

        fn insert(&mut self, key: String, value: CoverHandle) {
            if self.map.insert(key.clone(), value).is_some() {
                // Reinsertar una clave existente también la promueve.
                if let Some(idx) = self.order.iter().position(|x| x == &key) {
                    self.order.remove(idx);
                }
            }
            self.order.push_back(key);
            while self.order.len() > self.cap {
                if let Some(oldest) = self.order.pop_front() {
                    self.map.remove(&oldest);
                }
            }
        }

        fn clear(&mut self) {
            self.map.clear();
            self.order.clear();
        }

        fn purge_oldest(&mut self, count: usize, low_resource: bool) {
            let effective_count = if low_resource { count * 2 } else { count };
            let to_remove = effective_count.min(self.order.len());
            for _ in 0..to_remove {
                if let Some(oldest) = self.order.pop_front() {
                    self.map.remove(&oldest);
                }
            }
        }

        fn resize(&mut self, cap: usize) {
            self.cap = cap;
            while self.order.len() > self.cap {
                if let Some(oldest) = self.order.pop_front() {
                    self.map.remove(&oldest);
                }
            }
        }

        fn len(&self) -> usize {
            self.order.len()
        }

        fn contains(&self, key: &str) -> bool {
            self.map.contains_key(key)
        }

        fn order_mru_to_lru(&self) -> Vec<String> {
            self.order.iter().rev().cloned().collect()
        }
    }

    /// Runs the same scenario against the production cache and the
    /// independent reference, and asserts that the observable trace and
    /// final order match. The reference does not use `lru`, so a real
    /// divergence in the production implementation will fail the test.
    fn assert_parity(cap: usize, scenario: impl Fn(&mut dyn ParityCache) -> Vec<String>) {
        let mut production = ProductionCache::new(cap);
        let mut reference = ReferenceCache::new(cap);

        let production_trace = scenario(&mut production);
        let reference_trace = scenario(&mut reference);

        assert_eq!(
            production_trace, reference_trace,
            "rastro divergente en cap={cap}"
        );
        assert_eq!(
            production.order_mru_to_lru(),
            reference.order_mru_to_lru(),
            "orden divergente en cap={cap}"
        );
    }

    /// Explicit contract for recency, hit promotion, and eviction without
    /// comparing against another implementation: fixes the expected
    /// MRU-to-LRU order.
    #[test]
    fn covercache_golden_lru_order() {
        let mut cache = CoverCache::new(3);
        cache.put("a".into(), test_handle(0));
        cache.put("b".into(), test_handle(1));
        cache.put("c".into(), test_handle(2));
        assert_eq!(cache.order_mru_to_lru().join(","), "c,b,a");

        let _ = cache.get("a");
        assert_eq!(cache.order_mru_to_lru().join(","), "a,c,b");

        // `peek` does not alter recency order.
        assert!(cache.peek("b").is_some());
        assert_eq!(cache.order_mru_to_lru().join(","), "a,c,b");

        // Inserting evicts the least-recent ("b"), not the just-read entry.
        cache.put("d".into(), test_handle(3));
        assert_eq!(cache.order_mru_to_lru().join(","), "d,a,c");
        assert!(!cache.contains("b"));
    }

    /// Inserts `cap` numbered keys and returns their names in insertion order.
    fn fill(cache: &mut dyn ParityCache, cap: usize) -> Vec<String> {
        let keys: Vec<String> = (0..cap).map(|i| format!("k{i}")).collect();
        for (i, key) in keys.iter().enumerate() {
            cache.insert(key.clone(), test_handle(i));
        }
        keys
    }

    #[test]
    fn covercache_capacity_bound_parity() {
        for cap in [4usize, 16, 64] {
            assert_parity(cap, |cache| {
                for i in 0..cap + 5 {
                    cache.insert(format!("k{i}"), test_handle(i));
                }
                vec![format!("len={}", cache.len())]
            });
        }
    }

    #[test]
    fn covercache_recency_eviction_order_parity() {
        for cap in [3usize, 4, 16, 64] {
            assert_parity(cap, |cache| {
                let keys = fill(cache, cap);
                let first = keys[0].clone();
                let second = keys[1].clone();
                // Reading the oldest entry promotes it; inserting a new entry
                // evicts the next-oldest, not the just-read one.
                let _ = cache.get(&first);
                cache.insert("nueva".into(), test_handle(999));
                vec![
                    format!("len={}", cache.len()),
                    format!("first_present={}", cache.contains(&first)),
                    format!("second_present={}", cache.contains(&second)),
                    format!("order={:?}", cache.order_mru_to_lru()),
                ]
            });
        }
    }

    #[test]
    fn covercache_get_promotes_but_peek_does_not() {
        for cap in [4usize, 16, 64] {
            assert_parity(cap, |cache| {
                let keys = fill(cache, cap.min(3));
                let before = cache.order_mru_to_lru();

                let _ = cache.peek(&keys[0]);
                let after_peek = cache.order_mru_to_lru();

                let _ = cache.get(&keys[0]);
                let after_get = cache.order_mru_to_lru();

                vec![
                    format!("peek_preserves_order={}", before == after_peek),
                    format!(
                        "get_promotes_to_mru={}",
                        after_get.first() == Some(&keys[0])
                    ),
                ]
            });
        }
    }

    /// Concurrency stress test under a single `Mutex`: no thread may leave
    /// the cache above capacity or trigger a panic.
    fn concurrency_stress<C: ParityCache + Send>(mutex: &Mutex<C>, cap: usize) {
        std::thread::scope(|scope| {
            for thread in 0..8usize {
                scope.spawn(move || {
                    for i in 0..200usize {
                        let key = format!("/music/album/{thread}/{i}");
                        let mut guard = mutex.lock();
                        guard.insert(key.clone(), test_handle(i));
                        let _ = guard.get(&key);
                        let _ = guard.get("/music/album/ausente");
                        assert!(guard.len() <= cap, "la caché excedió su capacidad");
                    }
                });
            }
        });
        assert!(mutex.lock().len() <= cap);
    }

    #[test]
    fn covercache_concurrency_parity() {
        let cap = 8;
        concurrency_stress(&Mutex::new(ProductionCache::new(cap)), cap);
        concurrency_stress(&Mutex::new(ReferenceCache::new(cap)), cap);
    }

    #[test]
    fn covercache_clear_parity() {
        for cap in [4usize, 16, 64] {
            assert_parity(cap, |cache| {
                fill(cache, cap);
                cache.clear();
                vec![
                    format!("len={}", cache.len()),
                    format!("order={:?}", cache.order_mru_to_lru()),
                ]
            });
        }
    }

    #[test]
    fn covercache_negative_cache_clear_is_isolated() {
        let mut negative: HashSet<String> = HashSet::new();
        for i in 0..3 {
            negative.insert(format!("missing-{i}"));
        }

        // Both implementations start with entries that must not be affected.
        let mut production = ProductionCache::new(4);
        production.insert("keep".into(), test_handle(1));
        let mut reference = ReferenceCache::new(4);
        reference.insert("keep".into(), test_handle(1));

        clear_negative_cache(&mut negative);

        assert!(
            negative.is_empty(),
            "el conjunto negativo debe quedar vacío"
        );
        // Limpiar el conjunto negativo es independiente de la caché LRU en ambos casos.
        assert_eq!(production.len(), 1);
        assert_eq!(reference.len(), 1);
    }

    #[test]
    fn covercache_purge_parity() {
        for cap in [4usize, 16] {
            for (count, low_resource) in [(1usize, false), (2usize, true)] {
                assert_parity(cap, |cache| {
                    fill(cache, cap);
                    let newest = format!("k{}", cap - 1);
                    cache.purge_oldest(count, low_resource);

                    let removed = if low_resource { count * 2 } else { count }.min(cap);
                    // La purga quita las menos recientes; la más nueva sobrevive
                    // mientras no se haya purgado la caché entera.
                    assert_eq!(cache.len(), cap - removed);
                    if removed < cap {
                        assert!(cache.contains(&newest));
                        assert!(!cache.contains("k0"));
                    } else {
                        assert!(cache.order_mru_to_lru().is_empty());
                    }

                    vec![
                        format!("len={}", cache.len()),
                        format!("order={:?}", cache.order_mru_to_lru()),
                    ]
                });
            }
        }
    }

    #[test]
    fn covercache_resize_parity() {
        for start in [16usize, 64] {
            for target in [4usize, 8] {
                assert_parity(start, |cache| {
                    fill(cache, start);
                    let newest = format!("k{}", start - 1);

                    // Reducir la capacidad desaloja primero las menos recientes.
                    cache.resize(target);
                    assert_eq!(
                        cache.len(),
                        target,
                        "reducir debe respetar la nueva capacidad"
                    );
                    assert!(cache.contains(&newest), "la más reciente debe sobrevivir");
                    assert!(!cache.contains("k0"), "la menos reciente debe desalojarse");
                    let shrink_order = cache.order_mru_to_lru();

                    // Ampliar la capacidad no desaloja ninguna entrada.
                    cache.resize(start + 16);
                    assert_eq!(cache.len(), target, "ampliar no debe desalojar");

                    vec![
                        format!("shrink_len={target}"),
                        format!("order={shrink_order:?}"),
                        format!("grow_len={}", cache.len()),
                    ]
                });
            }
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

    #[test]
    fn process_and_save_cover_smoke() {
        // Imagen RGBA en memoria → PNG en bytes, para recorrer el camino real
        // de decodificación, redimensionado y codificación AVIF.
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("no se pudo codificar el PNG de prueba");

        let path = process_and_save_cover(&bytes, "audoxidy_smoke_test_cover")
            .expect("process_and_save_cover falló en el smoke test");

        assert!(path.exists(), "la carátula AVIF no se creó en disco");
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("avif"));

        // Limpia solo el archivo generado; el directorio cache/covers/ es un
        // artefacto de runtime ignorado por git y se deja en su sitio.
        let _ = std::fs::remove_file(&path);
    }
}
