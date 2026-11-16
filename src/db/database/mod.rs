//! SQLite music library database.
//!
//! Defines the connection type and the module tree that owns schema
//! creation, song and playlist persistence, settings, presets, and search.

mod connection;
mod fts;
mod library;
mod models;
mod playlists;
mod presets;
mod schema;
mod search;
mod settings;
mod songs;

pub use connection::Database;
pub use models::LibrarySearchParams;
pub use models::PlaylistData;
pub use models::PlaylistFolderGroup;
pub use models::PlaylistSongRef;
pub use models::ShuffleSession;
pub use models::SongData;
pub use models::SongMetadataExtended;
pub(in crate::db::database) use search::fts5_match_query;

#[cfg(test)]
mod eq_preset_tests;

#[cfg(test)]
mod fts_tests;

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[test]
    fn test_prepared_statement_cache_flush() {
        let db = Database::new_memory().unwrap();

        // Primer acceso: puebla la caché de sentencias preparadas.
        let first = db.get_all_songs().unwrap();
        assert!(first.is_empty());

        // Vaciar la caché no debe afectar a las consultas posteriores:
        // las sentencias se vuelven a preparar de forma transparente.
        db.flush_prepared_statements();

        let second = db.get_all_songs().unwrap();
        assert!(second.is_empty());
    }
}

#[cfg(test)]
mod db_round_trip_tests {
    use super::*;
    use std::sync::Arc;

    /// Elimina la base de datos temporal y sus hermanos WAL/SHM. Solo toca la
    /// ruta única de este test; nunca la `library.db` de la aplicación.
    fn remove_db_files(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }

    #[test]
    fn test_on_disk_round_trip_persists_song() {
        // La persistencia en disco debe sobrevivir al cierre y reapertura de la
        // conexión. Se usa una ruta temporal única por proceso para no ensuciar
        // la raíz del crate ni chocar con la base de datos de la aplicación.
        let path = std::env::temp_dir().join(format!("audoxidy_db_{}.db", std::process::id()));
        remove_db_files(&path);

        {
            let mut db = Database::new_at(&path).expect("abrir la DB temporal");
            let song = SongData {
                full_file_path: Arc::from("/music/round_trip.flac".to_string().into_boxed_str()),
                title: Some(Arc::from("Round Trip".to_string().into_boxed_str())),
                artist: Some(Arc::from("Tester".to_string().into_boxed_str())),
                ..Default::default()
            };
            db.insert_song_full(&song, &SongMetadataExtended::default(), None, false)
                .expect("insertar la canción");
        } // La conexión se cierra antes de reabrir.

        let db = Database::new_at(&path).expect("reabrir la DB temporal");
        let songs = db.get_all_songs().expect("leer las canciones");
        assert_eq!(songs.len(), 1, "la canción debe sobrevivir la reapertura");
        assert_eq!(songs[0].title.as_deref(), Some("Round Trip"));
        drop(db);

        remove_db_files(&path);
    }
}

#[cfg(test)]
mod cover_tests;

#[cfg(test)]
mod crud_tests;

#[cfg(test)]
mod fts_consistency_tests;

#[cfg(test)]
mod real_library_tests;

#[cfg(test)]
mod schema_tests;

#[cfg(test)]
mod settings_tests;
