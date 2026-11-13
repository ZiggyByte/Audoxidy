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
    /// Emits [`CoreEvent::ScanStarted`] before returning and
    /// [`CoreEvent::ScanCompleted`] from the scanner thread when the walk
    /// finishes. The completion event reports the number of audio files the
    /// scan processed, which is the count the scanner exposes; a separate
    /// inserted-row delta would require changing the insert path.
    pub fn scan_folder(&self, path: String) {
        self.emit(super::events::CoreEvent::ScanStarted);
        let event_tx = self.event_tx.clone();
        self.scanner
            .scan_folder_async_with(path, move |files_processed| {
                if let Err(e) = event_tx.send(super::events::CoreEvent::ScanCompleted {
                    songs_added: files_processed,
                }) {
                    tracing::warn!("core event channel disconnected; event discarded: {e}");
                }
            });
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

    /// Reports whether the full-text index still mirrors the song table.
    ///
    /// Acquires the database lock, compares the content-table row count with the
    /// song count, and releases the lock. The comparison never walks the index,
    /// so it is cheap enough to run once at startup; the deeper index check
    /// belongs to the test suite.
    pub fn fts_is_consistent(&self) -> Result<bool, CoreError> {
        let db = self.lock_db()?;
        Ok(db.is_fts_consistent()?)
    }

    /// Rebuilds the full-text index from its content table.
    ///
    /// Acquires the database lock for the rebuild and releases it before
    /// returning, so a caller can schedule the repair on a background task
    /// without holding the lock across the wait.
    pub fn rebuild_fts_index(&self) -> Result<(), CoreError> {
        let db = self.lock_db()?;
        db.rebuild_fts_index()?;
        Ok(())
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
    /// The cover references of the rows being deleted are read **before** the
    /// transaction commits, so a file reachable only through a disappearing row
    /// is still a candidate. After the commit the database lock is released,
    /// every candidate is checked against the surviving rows, and only the
    /// unclaimed files are removed — from the injected cover cache directory,
    /// by file name, never from a stored path.
    pub fn delete_songs_with_transaction(&self, ids: &[i64]) -> Result<(), CoreError> {
        /// One cover file a deleted row may have left without a reference.
        enum CoverCandidate {
            /// An album cover named by the stored `ALBUMS.cover_path`.
            AlbumPath { file_name: String },
            /// A per-song override named from its owner id and content hash.
            SongOverride {
                song_id: i64,
                cover_hash: String,
                file_name: String,
            },
        }

        let candidates = {
            let db = self.lock_db()?;

            let mut candidates = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for (album_path, override_hash, song_id) in db.cover_refs_for_songs(ids)? {
                if let Some(file_name) = album_path
                    .as_deref()
                    .and_then(|path| std::path::Path::new(path).file_name())
                    .and_then(|name| name.to_str())
                    .map(str::to_owned)
                    && seen.insert(file_name.clone())
                {
                    candidates.push(CoverCandidate::AlbumPath { file_name });
                }

                if let Some(cover_hash) = override_hash {
                    match crate::utils::covers::song_cover_file_name(song_id, &cover_hash) {
                        Ok(file_name) => {
                            if seen.insert(file_name.clone()) {
                                candidates.push(CoverCandidate::SongOverride {
                                    song_id,
                                    cover_hash,
                                    file_name,
                                });
                            }
                        }
                        Err(error) => tracing::warn!(
                            target: "audoxidy::persistence",
                            song_id,
                            error = %error,
                            "No se pudo componer el nombre de la carátula propia de una canción"
                        ),
                    }
                }
            }

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

            candidates
        };

        let unreferenced = {
            let db = self.lock_db()?;
            let mut unreferenced = Vec::new();

            for candidate in candidates {
                match candidate {
                    CoverCandidate::AlbumPath { file_name } => {
                        match db.is_cover_path_referenced(&file_name) {
                            Ok(false) => unreferenced.push(file_name),
                            Ok(true) => {}
                            Err(error) => tracing::warn!(
                                target: "audoxidy::persistence",
                                file = %file_name,
                                error = %error,
                                "No se pudo comprobar si una carátula de álbum sigue referenciada"
                            ),
                        }
                    }
                    CoverCandidate::SongOverride {
                        song_id,
                        cover_hash,
                        file_name,
                    } => match db.is_cover_override_referenced(song_id, &cover_hash) {
                        Ok(false) => unreferenced.push(file_name),
                        Ok(true) => {}
                        Err(error) => tracing::warn!(
                            target: "audoxidy::persistence",
                            song_id,
                            error = %error,
                            "No se pudo comprobar si una carátula propia sigue referenciada"
                        ),
                    },
                }
            }

            unreferenced
        };

        for file_name in unreferenced {
            let path = self.paths.cover_cache_dir.join(&file_name);
            if let Err(error) = std::fs::remove_file(&path) {
                tracing::warn!(
                    target: "audoxidy::persistence",
                    file = %path.display(),
                    error = %error,
                    "No se pudo eliminar una carátula que ya no está referenciada"
                );
            }
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

    #[test]
    fn test_scan_folder_emits_started_and_completed() {
        let core = make_core_for_test();
        let events = core.event_receiver();
        core.scan_folder("/nonexistent/audoxidy-scan-path".to_string());

        assert!(matches!(
            events
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("ScanStarted must be emitted"),
            super::super::events::CoreEvent::ScanStarted
        ));
        match events
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("ScanCompleted must be emitted")
        {
            super::super::events::CoreEvent::ScanCompleted { songs_added } => {
                assert_eq!(songs_added, 0);
            }
            other => panic!("expected ScanCompleted, got {other:?}"),
        }
    }
}
