//! SQLite music library database.
//!
//! Defines the connection type and the module tree that owns schema
//! creation, song and playlist persistence, settings, presets, and search.

mod connection;
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
mod eq_preset_tests {
    use super::*;

    /// Serializes a slice of f32 to a little-endian byte blob.
    fn serialize(bands: Option<&[f32]>) -> Option<Vec<u8>> {
        bands.map(|v| v.iter().flat_map(|f| f.to_le_bytes()).collect())
    }

    /// Deserializes a little-endian byte blob to a Vec<f32>.
    fn deserialize(blob: Option<Vec<u8>>) -> Option<Vec<f32>> {
        blob.and_then(|bytes| {
            if bytes.is_empty() || bytes.len() % 4 != 0 {
                return None;
            }
            Some(
                bytes
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                    .collect(),
            )
        })
    }

    #[test]
    fn test_save_and_load_eq_presets() {
        let db = Database::new_memory().unwrap();
        let b20 = serialize(Some(&[1.0; 20]));
        let b31 = serialize(Some(&[2.0; 31]));
        db.save_eq_preset_raw("Test Preset", 2.5, b20, b31).unwrap();

        let loaded = db.load_eq_presets_raw().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].0, "Test Preset");
        assert_eq!(loaded[0].1, 2.5);
        assert_eq!(deserialize(loaded[0].2.clone()).unwrap().len(), 20);
        assert_eq!(deserialize(loaded[0].3.clone()).unwrap().len(), 31);
        assert_eq!(deserialize(loaded[0].2.clone()).unwrap()[0], 1.0);
        assert_eq!(deserialize(loaded[0].3.clone()).unwrap()[0], 2.0);
    }

    #[test]
    fn test_save_and_load_eq_preset_with_none_bands() {
        let db = Database::new_memory().unwrap();
        db.save_eq_preset_raw("No Bands", 0.0, None, None).unwrap();

        let loaded = db.load_eq_presets_raw().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].0, "No Bands");
        assert!(loaded[0].2.is_none());
        assert!(loaded[0].3.is_none());
    }

    #[test]
    fn test_delete_eq_preset() {
        let db = Database::new_memory().unwrap();
        db.save_eq_preset_raw("To Delete", 0.0, None, None).unwrap();
        db.save_eq_preset_raw("Keep", 0.0, None, None).unwrap();
        assert_eq!(db.load_eq_presets_raw().unwrap().len(), 2);

        db.delete_eq_preset_by_name("To Delete").unwrap();
        let loaded = db.load_eq_presets_raw().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].0, "Keep");
    }

    #[test]
    fn test_clear_eq_presets() {
        let db = Database::new_memory().unwrap();
        db.save_eq_preset_raw("A", 0.0, None, None).unwrap();
        db.save_eq_preset_raw("B", 0.0, None, None).unwrap();
        assert_eq!(db.load_eq_presets_raw().unwrap().len(), 2);

        db.clear_all_eq_presets().unwrap();
        assert!(db.load_eq_presets_raw().unwrap().is_empty());
    }

    #[test]
    fn test_insert_or_replace_eq_preset() {
        let db = Database::new_memory().unwrap();
        db.save_eq_preset_raw("Same", 1.0, None, None).unwrap();
        db.save_eq_preset_raw("Same", 5.0, None, None).unwrap();

        let loaded = db.load_eq_presets_raw().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].1, 5.0);
    }
}

#[cfg(test)]
mod fts_tests {
    use super::*;
    use std::sync::Arc;

    fn make_song(path: &str, title: &str, artist: &str) -> SongData {
        SongData {
            full_file_path: Arc::from(path.to_string().into_boxed_str()),
            title: Some(Arc::from(title.to_string().into_boxed_str())),
            artist: Some(Arc::from(artist.to_string().into_boxed_str())),
            ..Default::default()
        }
    }

    fn seed_song(db: &mut Database, path: &str, title: &str, artist: &str) {
        let song = make_song(path, title, artist);
        db.insert_song_full(&song, &SongMetadataExtended::default(), None, vec![], false)
            .unwrap();
    }

    #[test]
    fn test_fts5_match_query_quotes_operators() {
        assert_eq!(
            fts5_match_query("foo OR bar").as_deref(),
            Some("\"foo\" \"OR\" \"bar\" *")
        );
        assert_eq!(fts5_match_query("a:b").as_deref(), Some("\"a:b\" *"));
        assert_eq!(fts5_match_query("\"").as_deref(), Some("\"\"\"\" *"));
        assert_eq!(fts5_match_query(""), None);
        assert_eq!(fts5_match_query("   "), None);
    }

    #[test]
    fn test_search_songs_operator_input_never_errors() {
        let mut db = Database::new_memory().unwrap();
        seed_song(&mut db, "/music/op.flac", "Operator Test", "Artist");

        let result = db.search_songs("* OR AND NOT NEAR { } ^ :");
        assert!(result.is_ok(), "la entrada con operadores no debe fallar");
    }

    #[test]
    fn test_search_songs_prefix_match() {
        let mut db = Database::new_memory().unwrap();
        seed_song(&mut db, "/music/another.flac", "Another Title", "Artist");

        let results = db.search_songs("Ano").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title.as_deref(), Some("Another Title"));
    }

    #[test]
    fn test_search_songs_empty_query() {
        let mut db = Database::new_memory().unwrap();
        seed_song(&mut db, "/music/empty.flac", "Empty Test", "Artist");

        assert!(db.search_songs("").unwrap().is_empty());
        assert!(db.search_songs("   ").unwrap().is_empty());
    }

    #[test]
    fn test_filtered_search_operator_input_never_errors() {
        let mut db = Database::new_memory().unwrap();
        seed_song(&mut db, "/music/filtered.flac", "Filtered Test", "Artist");

        let params = LibrarySearchParams {
            query: Some("* OR { } :".into()),
            ..Default::default()
        };
        assert!(db.get_library_songs_filtered(&params).is_ok());
    }

    #[test]
    fn test_artist_groups_operator_input_never_errors() {
        let mut db = Database::new_memory().unwrap();
        seed_song(
            &mut db,
            "/music/artist.flac",
            "Artist Group Test",
            "Grouped Artist",
        );

        assert!(db.get_artist_groups_sql(Some("* NEAR :")).is_ok());
    }

    #[test]
    fn test_playlist_like_search_untouched() {
        let mut db = Database::new_memory().unwrap();
        seed_song(
            &mut db,
            "/music/pl.flac",
            "Playlist Song",
            "Playlist Artist",
        );

        let playlist_id = db.create_playlist("Test Playlist", false).unwrap();
        let song_id = db.get_all_songs().unwrap()[0].id;
        db.add_song_to_playlist(playlist_id, song_id).unwrap();

        // La búsqueda de playlists usa LIKE, no FTS5: los operadores son texto literal.
        let results = db.search_playlist_songs(playlist_id, "* OR { } :").unwrap();
        assert!(results.is_empty());
    }
}

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
            db.insert_song_full(&song, &SongMetadataExtended::default(), None, vec![], false)
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
