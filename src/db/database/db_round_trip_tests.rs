//! Area test for persistence across a connection close: a song inserted through
//! one connection is still there when the same file is reopened.
//!
//! The test owns a `TempTestDir`, so the database file and the WAL/SHM siblings
//! SQLite creates beside it are removed when the guard drops, including after a
//! failed assertion.

use super::*;
use std::sync::Arc;

use crate::utils::test_support::TempTestDir;

#[test]
fn test_on_disk_round_trip_persists_song() {
    // Persistence has to survive closing and reopening the connection. The file
    // lives in a private directory, so the test neither touches the application
    // database nor leaves a file in the crate root.
    let dir = TempTestDir::new("db-round-trip");
    let path = dir.path().join("round-trip.db");

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
}
