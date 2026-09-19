//! Library service: scan, query, stats, delete, and refresh.
//!
//! All methods follow the acquire→copy→release pattern: they acquire the
//! database lock, read or mutate data, release the lock, and return plain
//! data. No lock is held across audio manager or scanner calls.

use crate::db::database::SongData;

use super::Core;
use super::error::CoreError;
use super::types::LibraryStats;

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
