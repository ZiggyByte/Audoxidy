//! Consistency tests for the full-text index over its content table.
//!
//! Every scenario starts from its own in-memory database so a failure names one
//! route: the count comparison, the rank-1 integrity check, or the rebuild.

use super::*;
use rusqlite::params;
use std::sync::Arc;

/// Inserts one song through the regular write path and returns its row id.
fn seed_song(db: &mut Database, path: &str, title: &str, artist: &str, album: &str) -> i64 {
    let song = SongData {
        full_file_path: Arc::from(path.to_string().into_boxed_str()),
        title: Some(Arc::from(title.to_string().into_boxed_str())),
        artist: Some(Arc::from(artist.to_string().into_boxed_str())),
        album: Some(Arc::from(album.to_string().into_boxed_str())),
        ..Default::default()
    };
    db.insert_song_full(&song, &SongMetadataExtended::default(), None, false)
        .expect("insert the song");
    db.conn
        .query_row("SELECT id FROM SONGS WHERE file_path = ?1", [path], |row| {
            row.get(0)
        })
        .expect("read the assigned song id")
}

/// Empties the index while leaving the content table intact.
fn empty_index(db: &Database) {
    db.conn
        .execute("INSERT INTO SONGS_FTS(SONGS_FTS) VALUES('delete-all')", [])
        .expect("empty the index");
}

/// Builds the FTS expression for one literal token.
fn token(text: &str) -> String {
    fts5_match_query(text).expect("a token produces a MATCH expression")
}

#[test]
fn test_fts_count_matches_songs_after_insert() {
    let mut db = Database::new_memory().expect("create the in-memory database");
    seed_song(
        &mut db,
        "/music/fts-count-a.flac",
        "Count A",
        "Artist",
        "Album",
    );
    seed_song(
        &mut db,
        "/music/fts-count-b.flac",
        "Count B",
        "Artist",
        "Album",
    );

    assert_eq!(db.songs_count().unwrap(), 2, "both songs are stored");
    assert_eq!(
        db.fts_content_count().unwrap(),
        2,
        "the insert trigger mirrors both songs"
    );
    assert!(
        db.is_fts_consistent().unwrap(),
        "the content count equals the song count"
    );
}

#[test]
fn test_rebuild_rebuilds_the_index() {
    let mut db = Database::new_memory().expect("create the in-memory database");
    let rowid = seed_song(
        &mut db,
        "/music/fts-rebuild.flac",
        "Zzq-Rebuild-Token",
        "Artist",
        "Album",
    );

    empty_index(&db);
    let matched: i64 = db
        .conn
        .query_row(
            "SELECT count(*) FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1 AND rowid = ?2",
            params![token("Zzq-Rebuild-Token"), rowid],
            |row| row.get(0),
        )
        .expect("count the emptied index rows");
    assert_eq!(matched, 0, "the emptied index finds nothing");

    db.rebuild_fts_index().expect("rebuild the index");

    let matched: i64 = db
        .conn
        .query_row(
            "SELECT count(*) FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1 AND rowid = ?2",
            params![token("Zzq-Rebuild-Token"), rowid],
            |row| row.get(0),
        )
        .expect("count the rebuilt index rows");
    assert_eq!(matched, 1, "the rebuild restores the token for its rowid");
    db.verify_fts_integrity()
        .expect("the rebuilt index matches its content rows");
    assert!(
        db.is_fts_consistent().unwrap(),
        "the rebuild keeps both counts equal"
    );
}

#[test]
fn test_integrity_check_with_rank_reports_a_clean_index() {
    let mut db = Database::new_memory().expect("create the in-memory database");
    seed_song(
        &mut db,
        "/music/fts-clean.flac",
        "Zzq-Clean-Token",
        "Zzq-Clean-Artist",
        "Zzq-Clean-Album",
    );

    assert!(
        db.verify_fts_integrity().is_ok(),
        "a freshly populated index matches its content rows"
    );
}

#[test]
fn test_integrity_check_with_rank_detects_drift() {
    let mut db = Database::new_memory().expect("create the in-memory database");
    seed_song(
        &mut db,
        "/music/fts-drift.flac",
        "Zzq-Drift-Token",
        "Zzq-Drift-Artist",
        "Zzq-Drift-Album",
    );

    empty_index(&db);

    assert!(
        db.verify_fts_integrity().is_err(),
        "the rank-1 form compares the index with the content table"
    );
    assert!(
        db.is_fts_consistent().unwrap(),
        "the count comparison cannot see an emptied index"
    );
}

#[test]
fn test_plain_integrity_check_passes_on_drift() {
    let mut db = Database::new_memory().expect("create the in-memory database");
    seed_song(
        &mut db,
        "/music/fts-plain.flac",
        "Zzq-Plain-Token",
        "Zzq-Plain-Artist",
        "Zzq-Plain-Album",
    );

    empty_index(&db);

    db.conn
        .execute(
            "INSERT INTO SONGS_FTS(SONGS_FTS) VALUES('integrity-check')",
            [],
        )
        .expect("the single-value form only inspects the index interior");
    assert!(
        db.verify_fts_integrity().is_err(),
        "the two-value form is the check that rejects this index"
    );
}

#[test]
fn test_count_comparison_detects_a_missing_content_row() {
    let mut db = Database::new_memory().expect("create the in-memory database");
    let rowid = seed_song(
        &mut db,
        "/music/fts-missing.flac",
        "Zzq-Missing-Token",
        "Zzq-Missing-Artist",
        "Zzq-Missing-Album",
    );
    assert!(
        db.is_fts_consistent().unwrap(),
        "the fresh content row agrees"
    );

    db.conn
        .execute("DELETE FROM SONGS_FTS_CONTENT WHERE song_id = ?1", [rowid])
        .expect("remove the content row without touching the song");

    assert_eq!(db.songs_count().unwrap(), 1, "the song is still stored");
    assert_eq!(
        db.fts_content_count().unwrap(),
        0,
        "its content row is gone"
    );
    assert!(
        !db.is_fts_consistent().unwrap(),
        "the count comparison reports the missing content row"
    );
}
