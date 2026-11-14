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
    let index_count: i64 = db
        .conn
        .query_row("SELECT count(*) FROM SONGS_FTS", [], |row| row.get(0))
        .expect("count the index rows");
    assert_eq!(
        index_count, 2,
        "count(*) over the index reads its content table instead of a missing column"
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

#[test]
fn test_rebuild_repairs_a_missing_content_row() {
    let mut db = Database::new_memory().expect("create the in-memory database");
    let rowid = seed_song(
        &mut db,
        "/music/fts-repair.flac",
        "Zzq-Repair-Token",
        "Zzq-Repair-Artist",
        "Zzq-Repair-Album",
    );

    db.conn
        .execute("DELETE FROM SONGS_FTS_CONTENT WHERE song_id = ?1", [rowid])
        .expect("remove the content row without touching the song");
    assert!(
        !db.is_fts_consistent().unwrap(),
        "the missing row is detected before the repair"
    );

    db.rebuild_fts_index().expect("rebuild the index");

    assert!(
        db.is_fts_consistent().unwrap(),
        "the repair restores the missing content row"
    );
    assert_eq!(
        db.fts_content_count().unwrap(),
        1,
        "exactly one content row is reconstructed"
    );
    let found: i64 = db
        .conn
        .query_row(
            "SELECT count(*) FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1 AND rowid = ?2",
            params![token("Zzq-Repair-Token"), rowid],
            |row| row.get(0),
        )
        .expect("search the rebuilt index");
    assert_eq!(found, 1, "the rebuilt index finds the restored song");
}

#[test]
fn test_album_rename_reindexes_every_song_of_the_album() {
    let mut db = Database::new_memory().expect("create the in-memory database");
    let first = seed_song(
        &mut db,
        "/music/fts-album-a.flac",
        "Album Song A",
        "Album Artist",
        "Zzq-Album-Original",
    );
    let second = seed_song(
        &mut db,
        "/music/fts-album-b.flac",
        "Album Song B",
        "Album Artist",
        "Zzq-Album-Original",
    );

    let album_id: i64 = db
        .conn
        .query_row(
            "SELECT id FROM ALBUMS WHERE title = 'Zzq-Album-Original'",
            [],
            |row| row.get(0),
        )
        .expect("read the album the songs share");
    db.conn
        .execute(
            "UPDATE ALBUMS SET title = 'Zzq-Album-Renamed' WHERE id = ?1",
            [album_id],
        )
        .expect("rename the album");

    for rowid in [first, second] {
        let new_matches: i64 = db
            .conn
            .query_row(
                "SELECT count(*) FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1 AND rowid = ?2",
                params![token("Zzq-Album-Renamed"), rowid],
                |row| row.get(0),
            )
            .expect("count the new album token for one row");
        assert_eq!(
            new_matches, 1,
            "the renamed album indexes the song in row {rowid}"
        );

        let old_matches: i64 = db
            .conn
            .query_row(
                "SELECT count(*) FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1 AND rowid = ?2",
                params![token("Zzq-Album-Original"), rowid],
                |row| row.get(0),
            )
            .expect("count the old album token for one row");
        assert_eq!(
            old_matches, 0,
            "the old album title no longer indexes the song in row {rowid}"
        );
    }

    assert!(db.is_fts_consistent().unwrap(), "the counts still agree");
    db.verify_fts_integrity()
        .expect("the rename keeps index and content in step");
}

#[test]
fn test_artist_rename_reindexes_every_song_of_the_artist() {
    let mut db = Database::new_memory().expect("create the in-memory database");
    let first = seed_song(
        &mut db,
        "/music/fts-artist-a.flac",
        "Artist Song A",
        "Zzq-Artist-Original",
        "First Album",
    );
    let second = seed_song(
        &mut db,
        "/music/fts-artist-b.flac",
        "Artist Song B",
        "Zzq-Artist-Original",
        "Second Album",
    );

    let artist_id: i64 = db
        .conn
        .query_row(
            "SELECT id FROM ARTISTS WHERE name = 'Zzq-Artist-Original'",
            [],
            |row| row.get(0),
        )
        .expect("read the track artist both songs share");
    db.conn
        .execute(
            "UPDATE ARTISTS SET name = 'Zzq-Artist-Renamed' WHERE id = ?1",
            [artist_id],
        )
        .expect("rename the artist");

    for rowid in [first, second] {
        let new_matches: i64 = db
            .conn
            .query_row(
                "SELECT count(*) FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1 AND rowid = ?2",
                params![token("Zzq-Artist-Renamed"), rowid],
                |row| row.get(0),
            )
            .expect("count the new artist token for one row");
        assert_eq!(
            new_matches, 1,
            "the renamed artist indexes the song in row {rowid}"
        );

        let old_matches: i64 = db
            .conn
            .query_row(
                "SELECT count(*) FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1 AND rowid = ?2",
                params![token("Zzq-Artist-Original"), rowid],
                |row| row.get(0),
            )
            .expect("count the old artist token for one row");
        assert_eq!(
            old_matches, 0,
            "the old artist name no longer indexes the song in row {rowid}"
        );
    }

    assert!(db.is_fts_consistent().unwrap(), "the counts still agree");
    db.verify_fts_integrity()
        .expect("the rename keeps index and content in step");
}

#[test]
fn test_song_title_update_reindexes_only_that_row() {
    let mut db = Database::new_memory().expect("create the in-memory database");
    let edited = seed_song(
        &mut db,
        "/music/fts-title-edited.flac",
        "Zzq-Title-Original",
        "Title Artist",
        "Title Album",
    );
    let untouched = seed_song(
        &mut db,
        "/music/fts-title-kept.flac",
        "Zzq-Title-Untouched",
        "Title Artist",
        "Title Album",
    );

    db.conn
        .execute(
            "UPDATE SONGS SET title = 'Zzq-Title-Renamed' WHERE id = ?1",
            [edited],
        )
        .expect("rename one song title");

    let new_matches: i64 = db
        .conn
        .query_row(
            "SELECT count(*) FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1 AND rowid = ?2",
            params![token("Zzq-Title-Renamed"), edited],
            |row| row.get(0),
        )
        .expect("count the new title token for the edited row");
    assert_eq!(new_matches, 1, "the edited row carries the new title");

    let old_matches: i64 = db
        .conn
        .query_row(
            "SELECT count(*) FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1 AND rowid = ?2",
            params![token("Zzq-Title-Original"), edited],
            |row| row.get(0),
        )
        .expect("count the old title token for the edited row");
    assert_eq!(old_matches, 0, "the edited row dropped its old title");

    let kept_matches: i64 = db
        .conn
        .query_row(
            "SELECT count(*) FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1 AND rowid = ?2",
            params![token("Zzq-Title-Untouched"), untouched],
            |row| row.get(0),
        )
        .expect("count the title token of the untouched row");
    assert_eq!(kept_matches, 1, "the update did not touch the other row");

    db.verify_fts_integrity()
        .expect("one title update leaves both sides in step");
}

#[test]
fn test_song_delete_removes_its_tokens() {
    let mut db = Database::new_memory().expect("create the in-memory database");
    let deleted = seed_song(
        &mut db,
        "/music/fts-delete.flac",
        "Zzq-Delete-Token",
        "Delete Artist",
        "Delete Album",
    );
    let kept = seed_song(
        &mut db,
        "/music/fts-keep.flac",
        "Zzq-Keep-Token",
        "Keep Artist",
        "Keep Album",
    );
    assert_eq!(db.fts_content_count().unwrap(), 2, "two content rows");

    db.conn
        .execute("DELETE FROM SONGS WHERE id = ?1", [deleted])
        .expect("delete one song");

    let deleted_matches: i64 = db
        .conn
        .query_row(
            "SELECT count(*) FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1 AND rowid = ?2",
            params![token("Zzq-Delete-Token"), deleted],
            |row| row.get(0),
        )
        .expect("count the deleted row tokens");
    assert_eq!(deleted_matches, 0, "the deleted row left no token behind");

    let kept_matches: i64 = db
        .conn
        .query_row(
            "SELECT count(*) FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1 AND rowid = ?2",
            params![token("Zzq-Keep-Token"), kept],
            |row| row.get(0),
        )
        .expect("count the surviving row tokens");
    assert_eq!(kept_matches, 1, "the surviving row keeps its token");

    assert_eq!(db.fts_content_count().unwrap(), 1, "one content row left");
    assert!(db.is_fts_consistent().unwrap(), "the counts agree again");
    db.verify_fts_integrity()
        .expect("the delete keeps index and content in step");
}

#[test]
fn test_rescan_of_an_unchanged_song_keeps_the_index_consistent() {
    let mut db = Database::new_memory().expect("create the in-memory database");
    let path = "/music/fts-rescan.flac";
    seed_song(
        &mut db,
        path,
        "Zzq-Rescan-Token",
        "Zzq-Rescan-Artist",
        "Zzq-Rescan-Album",
    );
    seed_song(
        &mut db,
        path,
        "Zzq-Rescan-Token",
        "Zzq-Rescan-Artist",
        "Zzq-Rescan-Album",
    );

    assert_eq!(db.songs_count().unwrap(), 1, "the re-scan updated one row");
    assert_eq!(
        db.fts_content_count().unwrap(),
        1,
        "the value guards skipped the unchanged update"
    );
    assert!(db.is_fts_consistent().unwrap(), "the counts agree");
    db.verify_fts_integrity()
        .expect("the unchanged re-scan left the index untouched");
}

#[test]
fn test_album_delete_cascades_to_the_index() {
    let mut db = Database::new_memory().expect("create the in-memory database");
    let first = seed_song(
        &mut db,
        "/music/fts-cascade-a.flac",
        "Zzq-Cascade-Token-A",
        "Cascade Artist",
        "Zzq-Cascade-Album",
    );
    let second = seed_song(
        &mut db,
        "/music/fts-cascade-b.flac",
        "Zzq-Cascade-Token-B",
        "Cascade Artist",
        "Zzq-Cascade-Album",
    );

    let album_id: i64 = db
        .conn
        .query_row(
            "SELECT id FROM ALBUMS WHERE title = 'Zzq-Cascade-Album'",
            [],
            |row| row.get(0),
        )
        .expect("read the album both songs share");
    db.conn
        .execute("DELETE FROM ALBUMS WHERE id = ?1", [album_id])
        .expect("delete the album and let the cascade remove its songs");

    for (rowid, token_text) in [
        (first, "Zzq-Cascade-Token-A"),
        (second, "Zzq-Cascade-Token-B"),
    ] {
        let matched: i64 = db
            .conn
            .query_row(
                "SELECT count(*) FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1 AND rowid = ?2",
                params![token(token_text), rowid],
                |row| row.get(0),
            )
            .expect("count the cascaded row token");
        assert_eq!(
            matched, 0,
            "the song removed by the cascade left no token in row {rowid}"
        );
    }

    assert_eq!(
        db.songs_count().unwrap(),
        0,
        "the cascade removed both songs"
    );
    assert_eq!(
        db.fts_content_count().unwrap(),
        0,
        "their content rows followed the delete"
    );
    assert!(db.is_fts_consistent().unwrap(), "both counts are empty");
    db.verify_fts_integrity()
        .expect("the cascade keeps index and content in step");
}

#[test]
fn test_table_reshape_then_rebuild_restores_full_consistency() {
    let mut db = Database::new_memory().expect("create the in-memory database");
    let rowid = seed_song(
        &mut db,
        "/music/fts-reshape.flac",
        "Zzq-Reshape-Token",
        "Zzq-Reshape-Artist",
        "Zzq-Reshape-Album",
    );

    empty_index(&db);
    assert!(
        db.verify_fts_integrity().is_err(),
        "the emptied index no longer matches its content rows"
    );

    db.rebuild_fts_index().expect("rebuild the index");

    assert!(
        db.is_fts_consistent().unwrap(),
        "the counts agree after the rebuild"
    );
    db.verify_fts_integrity()
        .expect("the rank-1 check passes after the rebuild");

    let matched: i64 = db
        .conn
        .query_row(
            "SELECT count(*) FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1 AND rowid = ?2",
            params![token("Zzq-Reshape-Token"), rowid],
            |row| row.get(0),
        )
        .expect("count the rebuilt row token");
    assert_eq!(matched, 1, "the known token is found again for its rowid");
}
