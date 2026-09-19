//! Cover service: resolve paths, purge, and clear cache.
//!
//! All methods return plain data — paths, not iced handles. The GUI adapter
//! wraps a resolved path into an `iced::Handle::from_path` at render time,
//! preserving the iced-free invariant of `src/core/`.
//!
//! Lock order compliance: the cover cache uses its own internal mutex and
//! does not interact with the database or audio manager locks.

use std::path::PathBuf;

use super::Core;

impl Core {
    /// Resolves the cover art path for an album hash.
    ///
    /// Checks whether the AVIF file exists on disk under the cover cache
    /// directory and returns its path. Returns `None` when the file does
    /// not exist or the album has no cached cover.
    pub fn resolve_cover_path(&self, album_hash: &str) -> Option<PathBuf> {
        let path = self
            .paths
            .cover_cache_dir
            .join(format!("{}.avif", album_hash));
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    /// Purges the oldest entries from the in-memory LRU cover cache.
    ///
    /// The `count` parameter controls how many entries to evict. In
    /// low-resource mode the effective count is doubled.
    pub fn purge_old_covers(&self, count: usize) {
        crate::utils::covers::purge_old_covers(count);
    }

    /// Clears the cover art negative cache (failed-path lookups).
    ///
    /// Useful after a re-scan to allow retrying paths that previously
    /// failed to load.
    pub fn clear_cover_cache(&self) {
        crate::utils::covers::clear_all_cover_cache();
    }

    /// Enqueues a cover art processing job.
    ///
    /// The job is processed asynchronously on the cover thread pool.
    /// `song_path` is the raw image data source path; `album_hash` is
    /// the deterministic cache key.
    pub fn enqueue_cover_job(&self, data: Vec<u8>, album_hash: String) {
        crate::utils::covers::enqueue_cover_job(data, album_hash);
    }

    /// Clears the entire in-memory LRU cover cache.
    ///
    /// Called on song change to release stale cover handles.
    pub fn clear_cover_lru_cache(&self) {
        crate::utils::covers::clear_raw_cache();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Core;
    use crate::core::types::CorePaths;
    use crate::db::Database;
    use crate::db::scanner::Scanner;
    use std::sync::{Arc, Mutex};

    fn make_core_for_test() -> Core {
        let paths = CorePaths::desktop_default();
        let db = Arc::new(Mutex::new(
            Database::new_memory().expect("in-memory db"),
        ));
        let scanner = Arc::new(Scanner::new(db.clone()));
        let audio_manager =
            Arc::new(crate::audio::AudioManager::new().expect("audio manager init"));
        Core::new(audio_manager, db, scanner, paths)
    }

    #[test]
    fn test_resolve_cover_path_missing() {
        let core = make_core_for_test();
        // A random hash should not exist on disk.
        let result = core.resolve_cover_path("nonexistent_hash_abc123");
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_cover_path_existing() {
        let core = make_core_for_test();
        // Create a temporary cover file to test resolution.
        let cache_dir = &core.paths().cover_cache_dir;
        std::fs::create_dir_all(cache_dir).ok();
        let test_hash = "test_cover_resolve_exists";
        let path = cache_dir.join(format!("{}.avif", test_hash));
        std::fs::write(&path, b"fake avif data").ok();

        let resolved = core.resolve_cover_path(test_hash);
        assert_eq!(resolved, Some(path.clone()));

        // Cleanup.
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_purge_old_covers_does_not_panic() {
        let core = make_core_for_test();
        core.purge_old_covers(5);
    }

    #[test]
    fn test_clear_cover_cache_does_not_panic() {
        let core = make_core_for_test();
        core.clear_cover_cache();
    }

    #[test]
    fn test_clear_cover_lru_cache_does_not_panic() {
        let core = make_core_for_test();
        core.clear_cover_lru_cache();
    }
}
