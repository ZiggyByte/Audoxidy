//! Area tests for the library walk: the directory traversal, its transaction
//! bracketing and its import-order bookkeeping.
//!
//! Every test runs against `Database::new_memory()`, every fixture is an art-free
//! WAV written into a `TempTestDir`, and no test opens the application database or
//! writes into the cover cache: a fixture without embedded art leaves the picture
//! bytes empty, so the insert reports no cover to process and the enqueue chain
//! stops before it reaches a path.
//!
//! The walk clears the process-global negative cover set before it starts, so each
//! test holds `crate::utils::covers::GLOBAL_STATE_TEST_LOCK` for its whole body.
//! The cover suites assert on that same set mid-body; without the shared lock a
//! walk can clear it between their record and their assertion.

use super::*;
use crate::db::database::{Database, SongMetadataExtended};
use crate::utils::test_support::{TempTestDir, make_song, write_pcm_wav};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// Opens the in-memory database the walk writes through.
fn scanner_db() -> Arc<Mutex<Database>> {
    Arc::new(Mutex::new(
        Database::new_memory().expect("the in-memory database must open"),
    ))
}

/// Walks `root` and returns how many files the scanner processed.
fn walk(db: &Arc<Mutex<Database>>, root: &str) -> usize {
    let dirty = Arc::new(AtomicBool::new(false));
    Scanner::scan_folder(db, root, &dirty)
}

#[test]
fn scanner_rescan_does_not_duplicate_songs_or_albums() {
    let _covers_guard = crate::utils::covers::GLOBAL_STATE_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());

    let dir = TempTestDir::new("scanner-rescan");
    write_pcm_wav(&dir.path().join("one.wav"), 44_100, 2, 0.05);
    write_pcm_wav(&dir.path().join("two.wav"), 44_100, 2, 0.05);

    let db = scanner_db();
    let root = dir.path().to_string_lossy().to_string();

    // Two tag-free files in one folder: the album title defaults to a single
    // value and the folder is the same, so both songs share one album row.
    assert_eq!(walk(&db, &root), 2, "the walk visits both fixtures");
    let first = db
        .lock()
        .expect("the scanner database must not be poisoned")
        .get_library_stats()
        .expect("the library stats must be readable");
    assert_eq!(
        (first.0, first.1),
        (2, 1),
        "two files, one album, after the first walk"
    );

    // The rescan upserts by file path, so neither the song rows nor the album
    // row may grow.
    assert_eq!(walk(&db, &root), 2, "the rescan visits both fixtures");
    let second = db
        .lock()
        .expect("the scanner database must not be poisoned")
        .get_library_stats()
        .expect("the library stats must be readable");
    assert_eq!(
        (second.0, second.1),
        (2, 1),
        "two files, one album, after the rescan"
    );
}

#[test]
fn scanner_import_order_advances_from_the_maximum() {
    let _covers_guard = crate::utils::covers::GLOBAL_STATE_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());

    let dir = TempTestDir::new("scanner-import-order");
    write_pcm_wav(&dir.path().join("first.wav"), 44_100, 2, 0.05);
    write_pcm_wav(&dir.path().join("second.wav"), 44_100, 2, 0.05);

    let db = scanner_db();
    {
        let mut database = db
            .lock()
            .expect("the scanner database must not be poisoned");
        let mut seeded = make_song("/seeded/elsewhere.flac", "Seeded", "Seeder");
        seeded.album = Some(Arc::from("Seeded Album"));
        seeded.import_order = 5;
        database
            .insert_song_full(&seeded, &SongMetadataExtended::default(), None, false)
            .expect("the seeded row must insert");
        // The stored maximum is the row's own order, so this reads the seed back
        // through the query the walk uses to choose its starting value.
        assert_eq!(
            database
                .get_max_import_order()
                .expect("the import order must be readable"),
            5,
            "the seed fixes the floor the walk has to start above"
        );
    }

    let root = dir.path().to_string_lossy().to_string();
    walk(&db, &root);

    let database = db
        .lock()
        .expect("the scanner database must not be poisoned");
    // The walk starts at max + 1 and advances by one per file, so the two new
    // rows carry 6 and 7 and the seeded row keeps its 5.
    assert_eq!(
        database
            .get_max_import_order()
            .expect("the import order must be readable"),
        7,
        "the walk continues above the seeded maximum"
    );
    assert_eq!(
        database
            .get_library_stats()
            .expect("the library stats must be readable")
            .0,
        3,
        "the seeded row is still there beside the two new ones"
    );
    drop(database);

    // A rescan must not renumber anything: the upsert leaves the column alone,
    // so a walk that started at 8 cannot raise the maximum.
    walk(&db, &root);
    assert_eq!(
        db.lock()
            .expect("the scanner database must not be poisoned")
            .get_max_import_order()
            .expect("the import order must be readable"),
        7,
        "the rescan preserves the order the two rows already carry"
    );
}

#[test]
fn scanner_ignores_an_unsupported_extension() {
    let _covers_guard = crate::utils::covers::GLOBAL_STATE_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());

    let dir = TempTestDir::new("scanner-extension");
    write_pcm_wav(&dir.path().join("track.wav"), 44_100, 2, 0.05);
    std::fs::write(dir.path().join("notes.txt"), b"not audio")
        .expect("the text fixture must be writable");

    let db = scanner_db();
    let root = dir.path().to_string_lossy().to_string();
    walk(&db, &root);

    let songs = db
        .lock()
        .expect("the scanner database must not be poisoned")
        .get_all_songs()
        .expect("the library rows must be readable");
    assert_eq!(songs.len(), 1, "only the supported extension becomes a row");
    assert!(
        songs[0].full_file_path.ends_with("track.wav"),
        "the imported row is the audio file, not the text one: {:?}",
        songs[0].full_file_path
    );
}

#[test]
fn scanner_walk_survives_a_corrupt_file() {
    let _covers_guard = crate::utils::covers::GLOBAL_STATE_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());

    let dir = TempTestDir::new("scanner-corrupt");
    // The walk sorts by file name, so the corrupt file is parsed first and the
    // good one is only reached if the failure did not abort the walk.
    std::fs::write(dir.path().join("broken.flac"), b"not audio at all")
        .expect("the corrupt fixture must be writable");
    write_pcm_wav(&dir.path().join("good.wav"), 44_100, 2, 0.05);

    let db = scanner_db();
    let root = dir.path().to_string_lossy().to_string();
    let processed = walk(&db, &root);

    assert_eq!(
        processed, 2,
        "the walk keeps going after the file the parser rejects"
    );
    let songs = db
        .lock()
        .expect("the scanner database must not be poisoned")
        .get_all_songs()
        .expect("the library rows must be readable");
    assert!(
        songs
            .iter()
            .any(|song| song.full_file_path.ends_with("good.wav")),
        "the file after the corrupt one is imported: {:?}",
        songs
            .iter()
            .map(|song| song.full_file_path.as_ref())
            .collect::<Vec<_>>()
    );
}
