//! Area tests for full-text search: the operator quoting and the read paths
//! that build a MATCH query.
//!
//! Every test runs against an in-memory database seeded through the shared song
//! factory, so no test touches the application database.

use super::*;
use crate::utils::test_support::make_song;

fn seed_song(db: &mut Database, path: &str, title: &str, artist: &str) {
    let song = make_song(path, title, artist);
    db.insert_song_full(&song, &SongMetadataExtended::default(), None, false)
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

/// Every song read path carries the album row identity a view groups by.
#[test]
fn test_song_rows_carry_the_album_identity() {
    let mut db = Database::new_memory().unwrap();
    seed_song(&mut db, "/music/identity.flac", "Identity", "Artist");

    let expected = db
        .get_all_albums()
        .unwrap()
        .first()
        .map(|album| album.0.clone())
        .expect("la canción sembrada tiene un álbum");

    for songs in [
        db.get_all_songs().unwrap(),
        db.search_songs("Ident").unwrap(),
        db.get_library_songs_filtered(&LibrarySearchParams::default())
            .unwrap(),
        db.get_songs_by_album(&expected).unwrap(),
        db.get_songs_by_album_and_artist(&expected, "Artist")
            .unwrap(),
    ] {
        assert_eq!(songs.len(), 1, "cada consulta devuelve la canción sembrada");
        assert_eq!(
            songs[0].album_hash.as_deref(),
            Some(expected.as_str()),
            "la fila lleva el hash del álbum, no su título visible"
        );
        assert_ne!(
            songs[0].album_hash.as_deref(),
            songs[0].album.as_deref(),
            "la identidad del álbum no es el título"
        );
    }
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
