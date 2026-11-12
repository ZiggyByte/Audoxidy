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
//!
//! The module also owns the front-end-free decode funnel and the pure cover
//! policy: [`decode_cover_file`] turns a stored cover file into RGBA pixels once,
//! while [`cover_bucket`] and [`cover_budget_bytes`] decide the decode size and
//! the retained-byte target. Keeping both here lets `utils/` stay free of iced
//! types while the GUI store only holds the handles they produce.

use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::codecs::avif::AvifEncoder;
use image::{ExtendedColorType, ImageEncoder};
use lru::LruCache;
use parking_lot::Mutex;
use rayon::ThreadPoolBuilder;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

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

/// Clears the negative cache (failed file paths) only. Useful after a re-scan.
pub fn clear_negative_cover_cache() {
    if let Some(neg_cache_mtx) = NEGATIVE_CACHE.get() {
        clear_negative_cache(&mut neg_cache_mtx.lock());
    }
}

/// Clears both the negative cache and the LRU cover cache. Use when a full
/// reset is needed (e.g. after deleting cover files on disk).
pub fn clear_all_cover_cache() {
    clear_negative_cover_cache();
    clear_raw_cache();
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
    let cache_dir = crate::utils::paths::get().cover_cache_dir.clone();
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

    let target_size = cover_native_size();

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

/// Returns whether a cover path is known to fail loading.
///
/// Consulted while decode requests are built, never while rows are rendered, so
/// the negative set stays off the layout path.
pub fn is_cover_negative(path: &str) -> bool {
    NEGATIVE_CACHE
        .get()
        .is_some_and(|negative| negative.lock().contains(path))
}

/// Failure to compose a cover file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverNameError {
    /// One of the hash arguments was empty.
    EmptyHash,
    /// One of the hash arguments contains a character that is not ASCII hex.
    NonHexHash,
}

impl std::fmt::Display for CoverNameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyHash => write!(formatter, "a cover hash cannot be empty"),
            Self::NonHexHash => write!(formatter, "a cover hash must contain only hex digits"),
        }
    }
}

impl std::error::Error for CoverNameError {}

/// Rejects a hash that would escape or distort a cover file name.
///
/// Every hash reaching a name is a SHA-256 hex digest, so anything else — an
/// empty string, a path separator, a traversal segment — is refused before it
/// becomes a file name.
fn validate_cover_hash(hash: &str) -> Result<(), CoverNameError> {
    if hash.is_empty() {
        return Err(CoverNameError::EmptyHash);
    }
    if !hash.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(CoverNameError::NonHexHash);
    }
    Ok(())
}

/// Complete file name for an album's own cover.
///
/// The album owner and the image content both appear in the name, so two albums
/// holding byte-identical pictures never share a file and deleting one album can
/// never remove the other's cover. The result includes the `.avif` extension.
pub fn album_cover_file_name(album_hash: &str, cover_hash: &str) -> Result<String, CoverNameError> {
    validate_cover_hash(album_hash)?;
    validate_cover_hash(cover_hash)?;
    Ok(format!("{album_hash}-{cover_hash}.avif"))
}

/// Complete file name for a song's own cover.
///
/// The song id in the name scopes the file to the song that owns the override,
/// so two songs with different art never share a file. The result includes the
/// `.avif` extension.
pub fn song_cover_file_name(song_id: i64, cover_hash: &str) -> Result<String, CoverNameError> {
    validate_cover_hash(cover_hash)?;
    Ok(format!("song-{song_id}-{cover_hash}.avif"))
}

/// Decoded RGBA8 pixels ready to become a front-end image handle.
///
/// `pixels` is always `width * height * 4` bytes. [`decode_cover_file`] refuses
/// to return a buffer that does not match its declared dimensions, because a
/// front-end handle built from a mismatched buffer stays blank for the whole
/// lifetime of its id.
#[derive(Debug)]
pub struct DecodedCover {
    /// Decoded width in pixels.
    pub width: u32,
    /// Decoded height in pixels.
    pub height: u32,
    /// RGBA8 pixels, `width * height * 4` bytes.
    pub pixels: Vec<u8>,
}

/// Failure of [`decode_cover_file`].
#[derive(Debug)]
pub enum CoverDecodeError {
    /// The cover file does not exist.
    NotFound(PathBuf),
    /// The cover file exists but could not be read.
    Io(PathBuf, std::io::Error),
    /// The bytes are not a decodable image, or the image has a zero dimension.
    Decode(PathBuf),
    /// The image could not be resized; the payload is the library message.
    Resize(PathBuf, String),
    /// The produced pixel buffer does not match its declared dimensions.
    PixelLength {
        /// Width the buffer was decoded or resized to.
        width: u32,
        /// Height the buffer was decoded or resized to.
        height: u32,
        /// Actual buffer length in bytes.
        len: usize,
    },
}

impl std::fmt::Display for CoverDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(path) => {
                write!(formatter, "cover file not found: {}", path.display())
            }
            Self::Io(path, error) => {
                write!(
                    formatter,
                    "cover file could not be read ({}): {error}",
                    path.display()
                )
            }
            Self::Decode(path) => {
                write!(
                    formatter,
                    "cover image could not be decoded: {}",
                    path.display()
                )
            }
            Self::Resize(path, error) => {
                write!(
                    formatter,
                    "cover image could not be resized ({}): {error}",
                    path.display()
                )
            }
            Self::PixelLength { width, height, len } => write!(
                formatter,
                "cover pixel buffer has {len} bytes, expected {width}x{height}x4"
            ),
        }
    }
}

impl std::error::Error for CoverDecodeError {}

/// Native cover edge produced by the writer and accepted by the decode cap.
///
/// The writer emits this edge, so decoding must never scale beyond it: 200 px in
/// low-resource mode, 400 px otherwise.
pub fn cover_native_size() -> u32 {
    if crate::utils::is_low_resource() {
        200
    } else {
        400
    }
}

/// Decodes a cover file into RGBA8 pixels sized for the rendered bucket.
///
/// The file is read exactly once and resized only downwards: a target at or
/// above the native width returns the native pixels untouched. This is the
/// library decode funnel; the player's per-track path handle and its next-cover
/// prefetch stay outside it.
pub fn decode_cover_file(path: &Path, target_px: u32) -> Result<DecodedCover, CoverDecodeError> {
    if target_px == 0 {
        return Err(CoverDecodeError::Resize(
            path.to_path_buf(),
            "the target size must be greater than zero".to_string(),
        ));
    }

    cover_metrics().record_file_open();

    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(CoverDecodeError::NotFound(path.to_path_buf()));
        }
        Err(error) => return Err(CoverDecodeError::Io(path.to_path_buf(), error)),
    };

    let image = image::load_from_memory(&bytes)
        .map_err(|_| CoverDecodeError::Decode(path.to_path_buf()))?
        .to_rgba8();
    cover_metrics().record_decode();

    let width = image.width();
    let height = image.height();

    if width == 0 || height == 0 {
        return Err(CoverDecodeError::Decode(path.to_path_buf()));
    }

    let (out_width, out_height, pixels) = if target_px >= width {
        // Never upscale: the native pixels are already the largest usable image.
        (width, height, image.into_raw())
    } else {
        // Cover files are written square, so the target is a square edge.
        let pixels = resize_cover_pixels(path, width, height, image.into_raw(), target_px)?;
        (target_px, target_px, pixels)
    };

    let expected = (out_width as usize) * (out_height as usize) * 4;
    if pixels.len() != expected {
        return Err(CoverDecodeError::PixelLength {
            width: out_width,
            height: out_height,
            len: pixels.len(),
        });
    }

    Ok(DecodedCover {
        width: out_width,
        height: out_height,
        pixels,
    })
}

/// Resizes decoded RGBA pixels to a square target edge with Lanczos3.
fn resize_cover_pixels(
    path: &Path,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    target_px: u32,
) -> Result<Vec<u8>, CoverDecodeError> {
    let source = Image::from_vec_u8(width, height, pixels, fast_image_resize::PixelType::U8x4)
        .map_err(|error| CoverDecodeError::Resize(path.to_path_buf(), error.to_string()))?;

    let mut destination = Image::new(target_px, target_px, source.pixel_type());
    let mut resizer = Resizer::new();

    if let Some(ext) = forced_cpu_extension() {
        // SAFETY: `forced_cpu_extension` only returns `Avx2` after checking at
        // runtime with `is_x86_feature_detected!("avx2")` that the processor
        // supports it.
        unsafe {
            resizer.set_cpu_extensions(ext);
        }
    }

    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));

    resizer
        .resize(&source, &mut destination, &options)
        .map_err(|error| CoverDecodeError::Resize(path.to_path_buf(), error.to_string()))?;

    Ok(destination.buffer().to_vec())
}

/// Process-global counters for the cover read path.
///
/// Every field is a relaxed atomic because the counters describe a trend, not an
/// invariant: decode workers update them without a lock and the GUI reads a
/// snapshot when it logs or measures.
#[derive(Debug, Default)]
pub struct CoverMetrics {
    file_opens: AtomicU64,
    decodes: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    key_lookups: AtomicU64,
    evictions: AtomicU64,
    cache_bytes: AtomicU64,
    in_flight: AtomicU64,
}

impl CoverMetrics {
    /// Creates a zeroed counter set; the process-global instance uses this.
    const fn new() -> Self {
        Self {
            file_opens: AtomicU64::new(0),
            decodes: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            key_lookups: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            cache_bytes: AtomicU64::new(0),
            in_flight: AtomicU64::new(0),
        }
    }

    fn increment(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    /// Counts one cover file open performed by the decode funnel.
    pub fn record_file_open(&self) {
        Self::increment(&self.file_opens);
    }

    /// Counts one image decoded from a cover file.
    pub fn record_decode(&self) {
        Self::increment(&self.decodes);
    }

    /// Counts one decoded-cover lookup that found a handle.
    pub fn record_cache_hit(&self) {
        Self::increment(&self.cache_hits);
    }

    /// Counts one decoded-cover lookup that found no handle.
    pub fn record_cache_miss(&self) {
        Self::increment(&self.cache_misses);
    }

    /// Counts one decoded-cover store lookup, hit or miss.
    pub fn record_key_lookup(&self) {
        Self::increment(&self.key_lookups);
    }

    /// Counts one decoded cover removed from the store.
    pub fn record_eviction(&self) {
        Self::increment(&self.evictions);
    }

    /// Publishes the decoded bytes the store currently retains.
    pub fn set_cache_bytes(&self, bytes: usize) {
        self.cache_bytes.store(bytes as u64, Ordering::Relaxed);
    }

    /// Counts a decode that entered a worker.
    pub fn increment_in_flight(&self) {
        Self::increment(&self.in_flight);
    }

    /// Counts a decode that left a worker.
    pub fn decrement_in_flight(&self) {
        self.in_flight.fetch_sub(1, Ordering::Relaxed);
    }

    /// Reads every counter once, for logging or a test assertion.
    pub fn snapshot(&self) -> CoverIoCounters {
        CoverIoCounters {
            file_opens: self.file_opens.load(Ordering::Relaxed),
            decodes: self.decodes.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            key_lookups: self.key_lookups.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            cache_bytes: self.cache_bytes.load(Ordering::Relaxed),
            in_flight: self.in_flight.load(Ordering::Relaxed),
        }
    }
}

static COVER_METRICS: CoverMetrics = CoverMetrics::new();

/// Returns the process-global cover counters.
pub fn cover_metrics() -> &'static CoverMetrics {
    &COVER_METRICS
}

/// Point-in-time copy of every cover counter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CoverIoCounters {
    /// Cover file opens performed by the decode funnel.
    pub file_opens: u64,
    /// Covers decoded from a file.
    pub decodes: u64,
    /// Decoded-cover lookups that found a handle.
    pub cache_hits: u64,
    /// Decoded-cover lookups that found no handle.
    pub cache_misses: u64,
    /// Decoded-cover store lookups, hit or miss.
    pub key_lookups: u64,
    /// Decoded covers removed from the store.
    pub evictions: u64,
    /// Decoded bytes the store currently retains.
    pub cache_bytes: u64,
    /// Decodes currently running on a worker.
    pub in_flight: u64,
}

/// Returns a snapshot of the process-global cover counters.
pub fn cover_io_counters() -> CoverIoCounters {
    cover_metrics().snapshot()
}

/// Decode buckets, in pixels of the cover edge.
///
/// The set covers the thumbnail row, the album grid's responsive range and the
/// detailed-list card (`220`), and ends at the native writer size so the decoder
/// never upscales.
pub const COVER_BUCKETS: [u32; 10] = [32, 64, 96, 128, 160, 192, 220, 256, 320, 400];

/// Picks the smallest decode bucket that covers a rendered cover.
///
/// `logical_px` is the cover edge the layout asks for and `scale` the display
/// scale factor; the product is rounded to the nearest pixel, raised to the next
/// bucket and clamped to `native_px`, the largest image the writer produces. A
/// target above the whole set falls back to the largest bucket before the clamp.
/// Non-finite or non-positive input falls back to the smallest bucket.
pub fn cover_bucket(logical_px: f32, scale: f32, native_px: u32) -> u32 {
    if !logical_px.is_finite() || !scale.is_finite() || logical_px <= 0.0 || scale <= 0.0 {
        return COVER_BUCKETS[0];
    }

    let target = (logical_px * scale).round();
    let bucket = COVER_BUCKETS
        .iter()
        .copied()
        .find(|bucket| *bucket as f32 >= target)
        .unwrap_or(COVER_BUCKETS[COVER_BUCKETS.len() - 1]);

    bucket.min(native_px)
}

/// Number of covers a viewport shows, used to size the decoded-cover budget.
///
/// `columns` is the grid column count, or 1 for the list views where a row shows
/// one cover. A non-positive or non-finite measurement yields zero, so a budget
/// cannot be derived from an unmeasured viewport.
pub fn viewport_capacity(columns: usize, viewport_h: f32, row_height: f32) -> usize {
    if columns == 0
        || !viewport_h.is_finite()
        || !row_height.is_finite()
        || viewport_h <= 0.0
        || row_height <= 0.0
    {
        return 0;
    }

    let rows = (viewport_h / row_height).ceil() as usize;
    columns.saturating_mul(rows)
}

/// Floor for the decoded-cover budget, in bytes.
pub const COVER_BUDGET_MIN_BYTES: usize = 8 * 1024 * 1024;

/// Ceiling for the decoded-cover budget, in bytes.
pub const COVER_BUDGET_MAX_BYTES: usize = 48 * 1024 * 1024;

/// Budget for decoded covers: two viewports of one bucket, clamped.
///
/// The cushion is one extra viewport, so returning inside the retained window
/// finds its covers still decoded. The clamp keeps a large window from retaining
/// an unreasonable amount of host RGBA.
pub fn cover_budget_bytes(capacity: usize, bucket: u32) -> usize {
    let bucket_bytes = (bucket as usize)
        .saturating_mul(bucket as usize)
        .saturating_mul(4);
    let target = capacity.saturating_mul(2).saturating_mul(bucket_bytes);

    target.clamp(COVER_BUDGET_MIN_BYTES, COVER_BUDGET_MAX_BYTES)
}

/// Byte and recency ledger for the decoded-cover store.
///
/// Entries are keyed by cover path and decode bucket, and `bytes` is the exact
/// RGBA size retained for them. The ledger owns no lock and no front-end type,
/// so the store holds it as a plain field and delegates accounting here.
#[derive(Debug, Default)]
pub struct CoverBudget {
    /// Bucket → cover path → retained bytes.
    entries: HashMap<u16, HashMap<String, usize>>,
    /// Recency order, most recently used first.
    order: VecDeque<(String, u16)>,
    bytes: usize,
}

impl CoverBudget {
    /// Records the retained bytes for one path and bucket, promoting the entry to
    /// most recently used and replacing any previous accounting for it.
    pub fn insert(&mut self, path: &str, bucket: u16, bytes: usize) {
        if let Some(previous) = self
            .entries
            .entry(bucket)
            .or_default()
            .insert(path.to_string(), bytes)
        {
            self.bytes = self.bytes.saturating_sub(previous);
            if let Some(index) = self.position(path, bucket) {
                self.order.remove(index);
            }
        }

        self.bytes += bytes;
        self.order.push_front((path.to_string(), bucket));
    }

    /// Promotes an entry to most recently used; returns whether it exists.
    pub fn touch(&mut self, path: &str, bucket: u16) -> bool {
        let Some(index) = self.position(path, bucket) else {
            return false;
        };

        if let Some(entry) = self.order.remove(index) {
            self.order.push_front(entry);
        }

        true
    }

    /// Returns the retained bytes for an entry without changing its recency.
    pub fn get(&self, path: &str, bucket: u16) -> Option<usize> {
        self.entries
            .get(&bucket)
            .and_then(|by_path| by_path.get(path))
            .copied()
    }

    /// Drops an entry and returns the bytes it retained.
    pub fn remove(&mut self, path: &str, bucket: u16) -> Option<usize> {
        let removed = self.entries.get_mut(&bucket)?.remove(path)?;
        self.bytes = self.bytes.saturating_sub(removed);

        if let Some(index) = self.position(path, bucket) {
            self.order.remove(index);
        }

        if self
            .entries
            .get(&bucket)
            .is_some_and(|by_path| by_path.is_empty())
        {
            self.entries.remove(&bucket);
        }

        Some(removed)
    }

    /// Drops every entry.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.bytes = 0;
    }

    /// Retained bytes across every entry.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Number of retained entries.
    pub fn len(&self) -> usize {
        self.entries.values().map(HashMap::len).sum()
    }

    /// Returns whether the ledger holds no entry.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Entry keys as `path@bucket`, most recently used first.
    pub fn order_mru_to_lru(&self) -> Vec<String> {
        self.order
            .iter()
            .map(|(path, bucket)| format!("{path}@{bucket}"))
            .collect()
    }

    fn position(&self, path: &str, bucket: u16) -> Option<usize> {
        self.order
            .iter()
            .position(|(entry_path, entry_bucket)| entry_path == path && *entry_bucket == bucket)
    }
}

/// Serializes the tests that read or mutate process-global cover state.
///
/// The counters and the negative set are shared by every test in the binary, so
/// tests asserting deltas on them take this guard for their whole body.
#[cfg(test)]
pub(crate) static GLOBAL_STATE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

    /// Creates a unique temporary directory for a test that needs a real file.
    fn temp_cover_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "audoxidy-cover-test-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("no se pudo crear el directorio temporal de prueba");
        dir
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
        assert_eq!(cover_budget_bytes(50, 192), 14_745_600);
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
    fn test_decode_cover_file_opens_the_file_once() {
        let _guard = global_guard();

        let dir = temp_cover_dir();
        let path = dir.join("test-cover.png");
        std::fs::write(&path, png_bytes(4, 4)).expect("no se pudo escribir el PNG de prueba");

        let before = cover_io_counters().file_opens;
        let decoded =
            decode_cover_file(&path, 400).expect("la carátula de prueba no se decodificó");
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

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_decode_cover_file_does_not_upscale() {
        let _guard = global_guard();
        let dir = temp_cover_dir();
        let path = dir.join("native-cover.png");
        std::fs::write(&path, png_bytes(8, 8)).expect("no se pudo escribir el PNG de prueba");

        let decoded =
            decode_cover_file(&path, 10_000).expect("la carátula de prueba no se decodificó");

        assert_eq!((decoded.width, decoded.height), (8, 8));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_decode_cover_file_downscales_to_the_target() {
        let _guard = global_guard();
        let dir = temp_cover_dir();
        let path = dir.join("large-cover.png");
        std::fs::write(&path, png_bytes(8, 8)).expect("no se pudo escribir el PNG de prueba");

        let decoded = decode_cover_file(&path, 4).expect("la carátula de prueba no se decodificó");

        assert_eq!((decoded.width, decoded.height), (4, 4));
        assert_eq!(decoded.pixels.len(), 4 * 4 * 4);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_decode_cover_file_reports_a_missing_file() {
        let _guard = global_guard();
        let dir = temp_cover_dir();
        let path = dir.join("absent.avif");

        let error =
            decode_cover_file(&path, 160).expect_err("un archivo ausente no puede decodificar");
        assert!(matches!(error, CoverDecodeError::NotFound(_)));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
