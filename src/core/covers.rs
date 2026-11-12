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
        if path.exists() { Some(path) } else { None }
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
        crate::utils::covers::clear_negative_cover_cache();
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
    use crate::db::database::{SongData, SongMetadataExtended};
    use crate::db::scanner::Scanner;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    fn make_core_for_test() -> Core {
        let paths = CorePaths::desktop_default();
        let db = Arc::new(Mutex::new(Database::new_memory().expect("in-memory db")));
        let scanner = Arc::new(Scanner::new(db.clone()));
        let audio_manager =
            Arc::new(crate::audio::AudioManager::new().expect("audio manager init"));
        Core::new(audio_manager, db, scanner, paths)
    }

    /// Builds a Core whose database and cover cache live in a private temporary
    /// directory, so a delete can be observed on disk without touching the
    /// process-global cover cache.
    fn make_core_with_temp_cover_dir() -> (Core, PathBuf) {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "audoxidy-core-cover-test-{}-{sequence}",
            std::process::id()
        ));
        let cover_cache_dir = dir.join("covers");
        std::fs::create_dir_all(&cover_cache_dir)
            .expect("no se pudo crear el directorio temporal de carátulas");

        let paths = CorePaths {
            db_path: dir.join("library.db"),
            cover_cache_dir,
            config_path: dir.join("config.toml"),
        };
        let db = Arc::new(Mutex::new(
            Database::new_at(&paths.db_path).expect("no se pudo abrir la base de datos temporal"),
        ));
        let scanner = Arc::new(Scanner::new(db.clone()));
        let audio_manager =
            Arc::new(crate::audio::AudioManager::new().expect("audio manager init"));

        (Core::new(audio_manager, db, scanner, paths), dir)
    }

    /// Inserts one non-external song and returns its row id.
    fn insert_song(core: &Core, path: &str, album: &str, picture_hash: Option<&str>) -> i64 {
        let song = SongData {
            full_file_path: Arc::from(path.to_string().into_boxed_str()),
            title: Some(Arc::from("Track".to_string().into_boxed_str())),
            artist: Some(Arc::from("The Artist".to_string().into_boxed_str())),
            album: Some(Arc::from(album.to_string().into_boxed_str())),
            ..Default::default()
        };

        let mut db = core.database().lock().expect("database lock");
        db.insert_song_full(
            &song,
            &SongMetadataExtended::default(),
            picture_hash.map(str::to_owned),
            Vec::new(),
            false,
        )
        .expect("no se pudo insertar la canción de prueba");

        db.get_song_id_by_path(path)
            .expect("no se pudo leer el id de la canción")
            .expect("la canción insertada debe existir")
    }

    /// Stored cover path of the album, or `None` when the row is gone.
    fn album_cover_path(core: &Core, album: &str) -> Option<String> {
        let db = core.database().lock().expect("database lock");
        db.get_all_albums()
            .expect("no se pudieron leer los álbumes")
            .into_iter()
            .find(|row| row.1 == album)
            .and_then(|row| row.5)
    }

    /// File name the album row stores, without its directory.
    fn album_cover_file_name(core: &Core, album: &str) -> String {
        album_cover_path(core, album)
            .as_deref()
            .and_then(|path| std::path::Path::new(path).file_name())
            .and_then(|name| name.to_str())
            .map(str::to_owned)
            .expect("el álbum debe tener una carátula almacenada")
    }

    /// Writes a stand-in cover file into the injected cache directory.
    fn write_cover(core: &Core, file_name: &str) -> PathBuf {
        let path = core.paths().cover_cache_dir.join(file_name);
        std::fs::write(&path, b"cover bytes").expect("no se pudo escribir la carátula de prueba");
        path
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

    #[test]
    fn test_delete_album_removes_only_its_own_cover() {
        let (core, dir) = make_core_with_temp_cover_dir();
        let shared_picture = "ab12cd34";

        let alpha_song = insert_song(
            &core,
            "/music/alpha/track.flac",
            "Album Alpha",
            Some(shared_picture),
        );
        insert_song(
            &core,
            "/music/beta/track.flac",
            "Album Beta",
            Some(shared_picture),
        );

        let alpha_name = album_cover_file_name(&core, "Album Alpha");
        let beta_name = album_cover_file_name(&core, "Album Beta");
        assert_ne!(
            alpha_name, beta_name,
            "two albums must never share a cover file name"
        );

        let alpha_file = write_cover(&core, &alpha_name);
        let beta_file = write_cover(&core, &beta_name);

        core.delete_songs_with_transaction(&[alpha_song])
            .expect("el borrado del álbum debe completarse");

        assert!(
            !alpha_file.exists(),
            "la carátula del álbum borrado debe desaparecer"
        );
        assert!(
            beta_file.exists(),
            "la carátula de otro álbum no puede tocarse, aunque la imagen sea idéntica"
        );
        assert_eq!(
            album_cover_file_name(&core, "Album Beta"),
            beta_name,
            "la ruta almacenada del álbum que sobrevive no cambia"
        );
        assert!(
            album_cover_path(&core, "Album Alpha").is_none(),
            "el álbum sin canciones no debe sobrevivir a la limpieza"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_delete_keeps_cover_still_referenced() {
        let (core, dir) = make_core_with_temp_cover_dir();

        let first_song = insert_song(
            &core,
            "/music/shared/one.flac",
            "Shared Album",
            Some("00ff00ff"),
        );
        insert_song(&core, "/music/shared/two.flac", "Shared Album", None);

        let cover_name = album_cover_file_name(&core, "Shared Album");
        let cover_file = write_cover(&core, &cover_name);
        let stored_path = album_cover_path(&core, "Shared Album");

        core.delete_songs_with_transaction(&[first_song])
            .expect("el borrado de una canción debe completarse");

        assert!(
            cover_file.exists(),
            "la carátula sigue referenciada por la canción restante"
        );
        assert_eq!(
            album_cover_path(&core, "Shared Album"),
            stored_path,
            "la ruta almacenada del álbum que sobrevive no cambia"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_delete_reads_cover_paths_before_commit() {
        let (core, dir) = make_core_with_temp_cover_dir();

        let song = insert_song(
            &core,
            "/music/solo/track.flac",
            "Solo Album",
            Some("0badf00d"),
        );
        let cover_name = album_cover_file_name(&core, "Solo Album");
        let cover_file = write_cover(&core, &cover_name);

        core.delete_songs_with_transaction(&[song])
            .expect("el borrado debe completarse");

        assert!(
            !cover_file.exists(),
            "la ruta leída antes del commit debe seguir siendo un candidato"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
