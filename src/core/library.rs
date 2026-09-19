//! Library service: scan, query, stats, delete, and refresh.
//!
//! All methods follow the acquire→copy→release pattern: they acquire the
//! database lock, read or mutate data, release the lock, and return plain
//! data. No lock is held across audio manager or scanner calls.

use std::sync::Arc;

use crate::db::database::{LibrarySearchParams, SongData};

use super::Core;
use super::error::CoreError;
use super::types::LibraryStats;

/// One album row from the grid query: `(hash_id, title, artist, genre, year, cover_path)`.
pub type AlbumGridRow = (String, String, String, String, String, Option<String>);

/// Raw aggregate statistics: `(songs, albums, duration, size, artists)`.
pub type LibraryStatsRaw = (usize, usize, f64, f64, usize);

/// A full library refresh payload: songs, album grid, and aggregate stats.
pub type LibrarySnapshot = (Vec<Arc<SongData>>, Vec<AlbumGridRow>, LibraryStatsRaw);

impl Core {
    /// Initiates a background scan of the given folder.
    ///
    /// Delegates to the scanner's async scan method. The scanner owns its
    /// own database connection and manages its own lock lifecycle.
    /// Emits [`CoreEvent::ScanStarted`] before returning.
    pub fn scan_folder(&self, path: String) {
        self.emit(super::events::CoreEvent::ScanStarted);
        self.scanner.scan_folder_async(path);
    }

    /// Returns whether a library scan is currently in progress.
    ///
    /// Reads the scanner's atomic flag directly — no lock required.
    pub fn is_scanning(&self) -> bool {
        self.scanner
            .is_scanning
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Reads and clears the scanner's dirty flag.
    ///
    /// The flag is set by the scanner when it commits new rows, so the
    /// front-end can refresh the library only when there is new data.
    pub fn check_db_dirty(&self) -> bool {
        self.scanner
            .db_dirty
            .swap(false, std::sync::atomic::Ordering::Relaxed)
    }

    /// Imports an M3U/M3U8 file as a new playlist; returns its id when created.
    pub fn import_m3u(&self, m3u_path: &str) -> Option<i64> {
        crate::db::scanner::Scanner::import_m3u(&self.database, m3u_path)
    }

    /// Returns all library songs (non-external) sorted by artist, year, album, track.
    ///
    /// Acquires the DB lock, reads all songs, releases the lock, and returns
    /// the cloned result.
    pub fn get_all_songs(&self) -> Vec<SongData> {
        let db = self.database.lock().expect("database lock poisoned");
        let songs = db.get_all_songs().unwrap_or_default();
        drop(db);
        // Arc<SongData> → SongData via clone
        songs.into_iter().map(|s| (*s).clone()).collect()
    }

    /// Returns high-level library statistics.
    ///
    /// Acquires the DB lock, reads aggregate counts, releases the lock,
    /// and returns a plain-data statistics snapshot.
    pub fn get_library_stats(&self) -> Result<LibraryStats, CoreError> {
        let db = self.database.lock().expect("database lock poisoned");
        let (songs, albums, duration, size, artists) = db.get_library_stats()?;
        drop(db);
        Ok(LibraryStats {
            total_songs: songs,
            total_albums: albums,
            total_artists: artists,
            total_duration_secs: duration,
            total_size_bytes: size as u64,
        })
    }

    /// Returns album grid items grouped by artist.
    ///
    /// Acquires the DB lock, queries the grid view, releases the lock,
    /// and returns the cloned tuples. Each tuple is
    /// `(hash_id, title, artist, genre, year, cover_path)`.
    pub fn get_albums(&self) -> Vec<(String, String, String, String, String, Option<String>)> {
        let db = self.database.lock().expect("database lock poisoned");
        let albums = db.get_grid_items_by_artist().unwrap_or_default();
        drop(db);
        albums
    }

    /// Returns all registered library folders.
    ///
    /// Acquires the DB lock, reads folder entries, releases the lock,
    /// and returns the cloned result. Each tuple is
    /// `(id, path, name)`.
    pub fn get_all_folders(&self) -> Result<Vec<(i64, String, String)>, CoreError> {
        let db = self.database.lock().expect("database lock poisoned");
        let folders = db.get_all_folders()?;
        drop(db);
        Ok(folders)
    }

    /// Deletes songs by their IDs and cleans up orphaned metadata.
    ///
    /// Acquires the DB lock, performs the batch delete and cleanup,
    /// releases the lock, and emits [`CoreEvent::LibraryDirty`].
    pub fn delete_songs(&self, ids: &[i64]) -> Result<(), CoreError> {
        {
            let db = self.database.lock().expect("database lock poisoned");
            db.batch_delete_songs(ids)?;
            db.cleanup_empty_metadata()?;
        }
        self.emit(super::events::CoreEvent::LibraryDirty);
        Ok(())
    }

    /// Imports a batch of external files into a playlist.
    ///
    /// Acquires the DB lock via the scanner's batch processor, which
    /// manages its own transaction lifecycle. Emits
    /// [`CoreEvent::LibraryDirty`] on completion.
    pub fn import_external_batch(
        &self,
        paths: Vec<std::path::PathBuf>,
        playlist_id: i64,
    ) -> Result<(), CoreError> {
        crate::db::scanner::Scanner::process_external_batch(&self.database, paths, playlist_id);
        self.emit(super::events::CoreEvent::LibraryDirty);
        Ok(())
    }

    /// Returns all library songs as shared references for the GUI list caches.
    ///
    /// Acquires the DB lock, reads all songs, releases the lock, and returns
    /// the shared handles so the front-end does not clone the payload.
    pub fn get_all_songs_shared(&self) -> Vec<Arc<SongData>> {
        let db = self.database.lock().expect("database lock poisoned");
        db.get_all_songs().unwrap_or_default()
    }

    /// Returns the songs of an album identified by its hash id.
    pub fn get_songs_by_album(&self, album_hash_id: &str) -> Vec<Arc<SongData>> {
        let db = self.database.lock().expect("database lock poisoned");
        db.get_songs_by_album(album_hash_id).unwrap_or_default()
    }

    /// Returns the songs of an album identified by artist and hash id.
    pub fn get_songs_by_album_and_artist(
        &self,
        album_hash_id: &str,
        artist_name: &str,
    ) -> Vec<Arc<SongData>> {
        let db = self.database.lock().expect("database lock poisoned");
        db.get_songs_by_album_and_artist(album_hash_id, artist_name)
            .unwrap_or_default()
    }

    /// Returns the songs matching the current library search parameters.
    pub fn get_library_songs_filtered(&self, params: &LibrarySearchParams) -> Vec<Arc<SongData>> {
        let db = self.database.lock().expect("database lock poisoned");
        db.get_library_songs_filtered(params).unwrap_or_default()
    }

    /// Returns raw aggregate library statistics as a plain tuple.
    pub fn get_library_stats_raw(&self) -> LibraryStatsRaw {
        let db = self.database.lock().expect("database lock poisoned");
        db.get_library_stats().unwrap_or((0, 0, 0.0, 0.0, 0))
    }

    /// Reads the full library payload (all songs, album grid, stats) under one lock.
    ///
    /// Used by the periodic refresh path when the scanner reports new data.
    pub fn refresh_library_snapshot(&self) -> LibrarySnapshot {
        let Ok(db) = self.database.lock() else {
            return (Vec::new(), Vec::new(), (0, 0, 0.0, 0.0, 0));
        };
        let songs = db.get_all_songs().unwrap_or_default();
        let albums = db.get_grid_items_by_artist().unwrap_or_default();
        let stats = db.get_library_stats().unwrap_or((0, 0, 0.0, 0.0, 0));
        (songs, albums, stats)
    }

    /// Reads the filtered library payload (matching songs, album grid, stats).
    ///
    /// Used by the explicit library refresh path.
    pub fn refresh_library_filtered(&self, params: &LibrarySearchParams) -> LibrarySnapshot {
        let Ok(db) = self.database.lock() else {
            return (Vec::new(), Vec::new(), (0, 0, 0.0, 0.0, 0));
        };
        let songs = db.get_library_songs_filtered(params).unwrap_or_default();
        let albums = db.get_grid_items_by_artist().unwrap_or_default();
        let stats = db.get_library_stats().unwrap_or((0, 0, 0.0, 0.0, 0));
        (songs, albums, stats)
    }

    /// Deletes songs in a transaction and removes cover files that became unused.
    ///
    /// The database transaction and the cover-usage checks run under the lock;
    /// the cover files are removed after the lock is released.
    pub fn delete_songs_with_transaction(
        &self,
        ids: &[i64],
        cover_hashes: &[String],
    ) -> Result<(), CoreError> {
        let removable = {
            let db = self.lock_db()?;

            if let Err(e) = db.begin_transaction() {
                tracing::warn!(
                    target: "audoxidy::persistence",
                    error = %e,
                    "No se pudo iniciar la transacción de borrado de canciones"
                );
            }
            if let Err(e) = db.batch_delete_songs(ids) {
                tracing::warn!(
                    target: "audoxidy::persistence",
                    count = ids.len(),
                    error = %e,
                    "No se pudieron borrar las canciones en lote"
                );
            }
            if let Err(e) = db.cleanup_empty_metadata() {
                tracing::warn!(
                    target: "audoxidy::persistence",
                    error = %e,
                    "No se pudieron limpiar los metadatos vacíos"
                );
            }
            if let Err(e) = db.commit_transaction() {
                tracing::warn!(
                    target: "audoxidy::persistence",
                    error = %e,
                    "No se pudo confirmar la transacción de borrado de canciones"
                );
            }

            let mut removable = Vec::new();
            for hash in cover_hashes {
                if let Ok(false) = db.is_cover_hash_in_use(hash) {
                    removable.push(hash.clone());
                }
            }
            removable
        };

        for hash in removable {
            let path = self.paths.cover_cache_dir.join(format!("{}.avif", hash));
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn make_core_for_test() -> Core {
        let paths = super::super::types::CorePaths::desktop_default();
        let db = Arc::new(Mutex::new(
            crate::db::Database::new_memory().expect("in-memory db"),
        ));
        let scanner = Arc::new(crate::db::scanner::Scanner::new(db.clone()));
        let audio_manager =
            Arc::new(crate::audio::AudioManager::new().expect("audio manager init"));
        Core::new(audio_manager, db, scanner, paths)
    }

    #[test]
    fn test_get_all_songs_returns_empty_for_fresh_db() {
        let core = make_core_for_test();
        let songs = core.get_all_songs();
        assert!(songs.is_empty());
    }

    #[test]
    fn test_get_library_stats_returns_zeros_for_fresh_db() {
        let core = make_core_for_test();
        let stats = core.get_library_stats().unwrap();
        assert_eq!(stats.total_songs, 0);
        assert_eq!(stats.total_albums, 0);
        assert_eq!(stats.total_artists, 0);
    }

    #[test]
    fn test_get_albums_returns_empty_for_fresh_db() {
        let core = make_core_for_test();
        let albums = core.get_albums();
        assert!(albums.is_empty());
    }

    #[test]
    fn test_get_all_folders_returns_empty_for_fresh_db() {
        let core = make_core_for_test();
        let folders = core.get_all_folders().unwrap();
        assert!(folders.is_empty());
    }

    #[test]
    fn test_delete_songs_on_empty_db_succeeds() {
        let core = make_core_for_test();
        core.delete_songs(&[1, 2, 3]).unwrap();
    }

    #[test]
    fn test_is_scanning_returns_false_initially() {
        let core = make_core_for_test();
        assert!(!core.is_scanning());
    }
}
