//! Cover art naming, image processing and the front-end-free cover policy.
//!
//! The write path scales an embedded picture to a square of the native cover
//! edge, encodes it as AVIF at 90% quality and stores it under `cache/covers/`
//! under the name its owner composes: `{album_hash}-{cover_hash}.avif` for an
//! album and `song-{song_id}-{cover_hash}.avif` for a per-song override. The
//! owner and the content hash both appear in the name, so no two owners share a
//! file and a deletion can only ever reach the file of the row it removes.
//!
//! The read path is the decode funnel: [`decode_cover_file`] turns one stored
//! file into RGBA pixels exactly once, on a cover worker. The GUI store holds the
//! resulting handles and delegates its accounting to [`CoverBudget`], the
//! byte-and-recency ledger that bounds retained pixels and protects the keys the
//! viewport shows. [`cover_bucket`], [`library_cover_bucket`],
//! [`viewport_capacity`] and [`cover_budget_bytes`] decide the decode size and the
//! retained-byte target; the library rule raises an album cover to its minimum
//! edge while a thumbnail keeps the bucket its row draws at.
//!
//! `utils/` stays free of front-end types: this module names, decodes, measures
//! and accounts for covers, while the layer that knows what a screen shows owns
//! the iced handles.

use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::codecs::avif::AvifEncoder;
use image::{ExtendedColorType, ImageEncoder};
use parking_lot::Mutex;
use rayon::ThreadPoolBuilder;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Static instance of the thread pool for background cover processing.
pub static COVER_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

/// Bounded channel to control memory pressure during scanning.
static COVER_GATEWAY: OnceLock<crossbeam::channel::Sender<(Vec<u8>, String)>> = OnceLock::new();

/// Stores file paths a decode already found undecodable (avoids retries).
pub static NEGATIVE_CACHE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

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

/// Cover files this process has queued for, or is currently writing.
///
/// A queued or running write means a cover file may appear on disk that no
/// album or song row references yet, so the reference-based cleanup paths read
/// this counter before they treat an unreferenced file as an orphan.
pub static COVER_WRITES_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

/// Number of cover writes queued or running.
pub fn cover_writes_in_flight() -> usize {
    COVER_WRITES_IN_FLIGHT.load(Ordering::Relaxed)
}

/// Enqueues a cover processing job safely.
/// When too many jobs are pending (16), the scanner thread waits,
/// preventing thousands of image buffers from being cloned into RAM simultaneously.
///
/// `file_name` is the complete name the writer stores the cover under; it is
/// forwarded untouched, so the gateway channel carries no other representation.
pub fn enqueue_cover_job(data: Vec<u8>, file_name: String) {
    let sender = COVER_GATEWAY.get_or_init(|| {
        let (sender, receiver) = crossbeam::channel::bounded::<(Vec<u8>, String)>(16);

        let pool = get_cover_pool();

        // Spawn persistent worker threads in the pool that consume from the channel.
        for _ in 0..pool.current_num_threads() {
            let rx_worker = receiver.clone();
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
                    COVER_WRITES_IN_FLIGHT.fetch_sub(1, Ordering::Relaxed);
                }
            });
        }

        sender
    });

    COVER_WRITES_IN_FLIGHT.fetch_add(1, Ordering::Relaxed);
    if sender.send((data, file_name)).is_err() {
        // Nothing will be written, so the queue depth must not keep the sweep away.
        COVER_WRITES_IN_FLIGHT.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Clears the set of file paths that previously failed to load.
fn clear_negative_cache(set: &mut HashSet<String>) {
    set.clear();
}

/// Clears the negative cache (undecodable cover files) only. Useful after a re-scan.
pub fn clear_negative_cover_cache() {
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
///
/// `file_name` is the complete name the naming helpers composed; it is joined
/// verbatim, so no layer appends a second extension.
pub fn process_and_save_cover(data: &[u8], file_name: &str) -> std::io::Result<PathBuf> {
    let cache_dir = &crate::utils::paths::get().cover_cache_dir;
    process_and_save_cover_in(cache_dir, data, file_name)
}

/// Writes a cover into `dir` under its complete file name.
///
/// `file_name` is joined verbatim: it already carries the `.avif` extension and
/// the owner and content hashes the naming helpers composed, so no layer appends
/// a second extension. The image is scaled to a square of the native cover edge
/// and encoded as AVIF at 90% quality; an existing file is left untouched.
pub fn process_and_save_cover_in(
    dir: &Path,
    data: &[u8],
    file_name: &str,
) -> std::io::Result<PathBuf> {
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
    }

    let dst_path = dir.join(file_name);
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

/// Returns whether a cover path is already known to be undecodable.
///
/// Consulted while decode requests are built, never while rows are rendered, so
/// the negative set stays off the layout path.
pub fn is_cover_negative(path: &str) -> bool {
    NEGATIVE_CACHE
        .get()
        .is_some_and(|negative| negative.lock().contains(path))
}

/// Records a cover file the decode funnel could not turn into an image.
///
/// Only a file that is present and still undecodable is recorded. `NotFound`, and
/// a read that failed for any other I/O reason, stay retryable: the write path is
/// fire-and-forget, so a decode can legitimately race a file that has not landed
/// yet, and recording it would pin the cover blank for the session. A recorded
/// path is skipped when the request window is built, so it is never decoded twice.
pub fn record_negative_cover(path: &Path, reason: &CoverDecodeError) {
    let undecodable = matches!(
        reason,
        CoverDecodeError::Decode(_)
            | CoverDecodeError::Resize(..)
            | CoverDecodeError::PixelLength { .. }
    );

    if !undecodable {
        tracing::debug!(
            path = %path.display(),
            error = %reason,
            "The cover file may still appear; leaving it retryable"
        );
        return;
    }

    let negative = NEGATIVE_CACHE.get_or_init(|| Mutex::new(HashSet::new()));
    negative.lock().insert(path.to_string_lossy().into_owned());
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

/// Owner-scoped stem for an album's own cover.
///
/// The album owner and the image content both appear in the stem, so two albums
/// holding byte-identical pictures never share a file and deleting one album can
/// never remove the other's cover.
pub fn album_cover_stem(album_hash: &str, cover_hash: &str) -> Result<String, CoverNameError> {
    validate_cover_hash(album_hash)?;
    validate_cover_hash(cover_hash)?;
    Ok(format!("{album_hash}-{cover_hash}"))
}

/// Complete file name for an album's own cover, `.avif` included.
pub fn album_cover_file_name(album_hash: &str, cover_hash: &str) -> Result<String, CoverNameError> {
    album_cover_stem(album_hash, cover_hash).map(|stem| format!("{stem}.avif"))
}

/// Owner-scoped stem for a song's own cover.
///
/// The song id scopes the stem to the song that owns the override, so two songs
/// with different art never share a file.
pub fn song_cover_stem(song_id: i64, cover_hash: &str) -> Result<String, CoverNameError> {
    validate_cover_hash(cover_hash)?;
    Ok(format!("song-{song_id}-{cover_hash}"))
}

/// Complete file name for a song's own cover, `.avif` included.
pub fn song_cover_file_name(song_id: i64, cover_hash: &str) -> Result<String, CoverNameError> {
    song_cover_stem(song_id, cover_hash).map(|stem| format!("{stem}.avif"))
}

/// Complete cached path of an album's own cover, or `None` when it has none.
///
/// Both arguments are content hashes — the album identity and the picture digest
/// — never paths. The file name is composed by [`album_cover_file_name`] and
/// joined to the injected cache directory, so a row can only ever name a file
/// inside that directory. An absent or empty hash yields `None`, and a hash the
/// naming helper refuses yields `None` instead of a partial path.
pub fn album_cover_path(album_hash: Option<&str>, cover_hash: Option<&str>) -> Option<String> {
    let album_hash = album_hash.filter(|hash| !hash.is_empty())?;
    let cover_hash = cover_hash.filter(|hash| !hash.is_empty())?;

    match album_cover_file_name(album_hash, cover_hash) {
        Ok(file_name) => Some(
            crate::utils::paths::get()
                .cover_cache_dir
                .join(file_name)
                .to_string_lossy()
                .into_owned(),
        ),
        Err(error) => {
            tracing::warn!(
                album_hash,
                error = %error,
                "An album cover cannot compose a file name from its hashes"
            );
            None
        }
    }
}

/// Resolves the one cover path a song row shows.
///
/// A per-song override wins over the album's cover; without one the album path is
/// returned unchanged. Every argument is a content hash — the album identity, the
/// album's picture digest and the raw hash kept in `SONGS.cover_override` — never
/// a path, so the composed file is `cover_cache_dir/song-{song_id}-{hash}.avif`.
/// A hash the naming helper refuses falls back to the album path instead of
/// hiding the album's art.
pub fn effective_cover_path(
    album_hash: Option<&str>,
    album_cover_hash: Option<&str>,
    override_hash: Option<&str>,
    song_id: i64,
) -> Option<String> {
    let album_path = album_cover_path(album_hash, album_cover_hash);

    let Some(override_hash) = override_hash.filter(|hash| !hash.is_empty()) else {
        return album_path;
    };

    match song_cover_file_name(song_id, override_hash) {
        Ok(file_name) => Some(
            crate::utils::paths::get()
                .cover_cache_dir
                .join(file_name)
                .to_string_lossy()
                .into_owned(),
        ),
        Err(error) => {
            tracing::warn!(
                song_id,
                error = %error,
                "A song cover override cannot compose a file name; using the album cover"
            );
            album_path
        }
    }
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

/// Smallest decode edge an album cover in the library may use.
///
/// The album cards are narrow enough that their display bucket alone looks soft
/// on a 1x display, so a library album cover always decodes at least this large.
/// The thumbnail rows are excluded: they draw at their own small edge, and
/// raising them would multiply their pixels for no visible gain.
pub const MIN_LIBRARY_COVER_BUCKET: u32 = 220;

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

/// Picks the decode bucket of an album cover the library draws.
///
/// The display bucket is a floor only for the album cards: the result is raised
/// to [`MIN_LIBRARY_COVER_BUCKET`] and then clamped to `native_px`, so a library
/// cover is never decoded below that edge and never upscaled past the image the
/// writer produced. A thumbnail keeps [`cover_bucket`] untouched, because it
/// draws at its own small edge.
pub fn library_cover_bucket(logical_px: f32, scale: f32, native_px: u32) -> u32 {
    cover_bucket(logical_px, scale, native_px)
        .max(MIN_LIBRARY_COVER_BUCKET)
        .min(native_px)
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
///
/// Three viewports of one bucket fit in 32 MiB at the measured 220 px bucket;
/// the ceiling only binds at high DPI, where the same entry count costs more.
pub const COVER_BUDGET_MAX_BYTES: usize = 32 * 1024 * 1024;

/// Budget for decoded covers: the window, its prefetch band and a scroll-back
/// cushion of one viewport, clamped.
///
/// The three viewports of one bucket are the visible window, the band the request
/// pass looks ahead and a cushion behind the viewport that the band does not cover;
/// without that third screen a short scroll back lands on entries the request pass
/// already spent the budget on. The clamp keeps a large window from retaining an
/// unreasonable amount of host RGBA, and it is a hard cap: the request pass reads
/// the resulting byte count as the retention it may protect, so the ceiling can
/// never be used as spare room.
pub fn cover_budget_bytes(capacity: usize, bucket: u32) -> usize {
    let bucket_bytes = (bucket as usize)
        .saturating_mul(bucket as usize)
        .saturating_mul(4);
    let target = capacity.saturating_mul(3).saturating_mul(bucket_bytes);

    target.clamp(COVER_BUDGET_MIN_BYTES, COVER_BUDGET_MAX_BYTES)
}

/// Byte and recency ledger for the decoded-cover store.
///
/// Entries are keyed by cover path and decode bucket, and `bytes` is the exact
/// RGBA size retained for them. `window` holds the keys the current viewport
/// needs; they are never evicted, so a visible cover cannot blank. The ledger
/// owns no lock and no front-end type, so the store holds it as a plain field
/// and delegates accounting here.
#[derive(Debug)]
pub struct CoverBudget {
    /// Bucket → cover path → retained bytes.
    entries: HashMap<u16, HashMap<String, usize>>,
    /// Recency order, most recently used first.
    order: VecDeque<(String, u16)>,
    /// Bytes the ledger is allowed to retain.
    budget: usize,
    /// Path → buckets the current viewport shows.
    window: HashMap<String, HashSet<u16>>,
    bytes: usize,
}

impl Default for CoverBudget {
    /// Starts at the ceiling, so a ledger that has not been given a viewport
    /// budget yet retains everything it stores.
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            budget: COVER_BUDGET_MAX_BYTES,
            window: HashMap::new(),
            bytes: 0,
        }
    }
}

impl CoverBudget {
    /// Sets the retained-byte target the next eviction enforces.
    pub fn set_budget(&mut self, bytes: usize) {
        self.budget = bytes;
    }

    /// Retained-byte target currently enforced.
    pub fn budget(&self) -> usize {
        self.budget
    }

    /// Replaces the protected window with `keys` and promotes the ones the
    /// ledger already holds.
    ///
    /// It neither inserts nor evicts: a key the store does not hold yet is only
    /// recorded as needed, and the promotion keeps a scroll inside the window
    /// from making its own entries the next eviction candidates.
    pub fn set_window(&mut self, keys: impl IntoIterator<Item = (String, u16)>) {
        self.window.clear();

        for (path, bucket) in keys {
            self.window.entry(path.clone()).or_default().insert(bucket);
            self.touch(&path, bucket);
        }
    }

    /// Returns whether the window needs a path and bucket.
    ///
    /// The lookup is borrowed and does not promote, so the decode-result handler
    /// can ask without changing the recency order.
    pub fn is_window(&self, path: &str, bucket: u16) -> bool {
        self.window
            .get(path)
            .is_some_and(|buckets| buckets.contains(&bucket))
    }

    /// Number of paths the current viewport window holds.
    ///
    /// Zero means no layout has run yet. Trimming in that state would find every
    /// entry outside an empty window and drop the lot, so callers gate on it.
    pub fn window_len(&self) -> usize {
        self.window.len()
    }

    /// Drops least-recently-used entries outside the window until the retained
    /// bytes fit the budget.
    ///
    /// A window entry is never removed, so an eviction stops when only window
    /// entries remain: the budget is a target for the retained entries, not an
    /// invariant that may blank a visible cover. Returns the removed keys in
    /// removal order.
    pub fn evict_to_budget(&mut self) -> Vec<(String, u16)> {
        let mut removed = Vec::new();

        while self.bytes > self.budget {
            let Some(index) = self
                .order
                .iter()
                .rposition(|(path, bucket)| !self.is_window(path, *bucket))
            else {
                break;
            };
            let (path, bucket) = self.order[index].clone();

            if self.remove(&path, bucket).is_some() {
                removed.push((path, bucket));
            }
        }

        removed
    }

    /// Drops every entry outside the window and returns the removed keys.
    ///
    /// The entries the window shows keep their bytes untouched, so a collection
    /// during playback cannot blank a cover the user is looking at.
    pub fn trim_outside_window(&mut self) -> Vec<(String, u16)> {
        let outside: Vec<(String, u16)> = self
            .order
            .iter()
            .filter(|(path, bucket)| !self.is_window(path, *bucket))
            .cloned()
            .collect();

        let mut removed = Vec::with_capacity(outside.len());
        for (path, bucket) in outside {
            if self.remove(&path, bucket).is_some() {
                removed.push((path, bucket));
            }
        }

        removed
    }

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
    ///
    /// The recency entry is dropped even when the byte accounting was already
    /// gone, so a caller that walks the order always makes progress.
    pub fn remove(&mut self, path: &str, bucket: u16) -> Option<usize> {
        let removed = self
            .entries
            .get_mut(&bucket)
            .and_then(|by_path| by_path.remove(path));

        if let Some(bytes) = removed {
            self.bytes = self.bytes.saturating_sub(bytes);
        }

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

        removed
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
#[path = "covers_tests.rs"]
mod tests;
