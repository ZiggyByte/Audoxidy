//! Area tests for the schema version gate, the rebuild and the connection pragmas.
//!
//! Every file-backed test owns a directory under the system temporary directory
//! and never touches the application database.

use super::schema::SCHEMA_VERSION;
use super::*;
use rusqlite::{Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Adds a suffix to a path, matching how SQLite names the WAL siblings.
fn sibling_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// Temporary directory owned by one test and removed when it is dropped.
struct TempDbDir {
    path: PathBuf,
}

impl TempDbDir {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "audoxidy-db-schema-test-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create the temporary database directory");
        Self { path }
    }

    /// The database file this directory owns.
    fn db_path(&self) -> PathBuf {
        self.path.join("library.db")
    }

    /// The directory itself, for paths the test builds relative to it.
    fn dir(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDbDir {
    fn drop(&mut self) {
        let db_path = self.db_path();
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(sibling_path(&db_path, "-wal"));
        let _ = std::fs::remove_file(sibling_path(&db_path, "-shm"));
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Inserts one song so the file holds rows a rebuild must discard.
fn insert_song(db: &mut Database) {
    let song = SongData {
        full_file_path: Arc::from("/music/schema_gate.flac".to_string().into_boxed_str()),
        title: Some(Arc::from("Schema Gate".to_string().into_boxed_str())),
        artist: Some(Arc::from("Schema Artist".to_string().into_boxed_str())),
        album: Some(Arc::from("Schema Album".to_string().into_boxed_str())),
        ..Default::default()
    };
    db.insert_song_full(&song, &SongMetadataExtended::default(), None, false)
        .expect("insert the song");
}

/// Reads the version stamped in the database header.
fn user_version(db: &Database) -> i64 {
    db.conn
        .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
        .expect("read user_version")
}

/// Counts the rows of a table the schema tests inspect.
fn row_count(db: &Database, table: &str) -> i64 {
    db.conn
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap_or_else(|error| panic!("count the {table} rows: {error}"))
}

/// Rewrites the version of an existing file, simulating a database written by a
/// previous schema.
fn stamp_version(path: &Path, version: i64) {
    let conn = Connection::open(path).expect("open the database to stamp its version");
    conn.pragma_update(None, "user_version", version)
        .expect("write user_version");
}

/// Reads a numeric pragma from an open connection.
fn pragma_i64(db: &Database, name: &str) -> i64 {
    db.conn
        .pragma_query_value(None, name, |row| row.get::<_, i64>(0))
        .unwrap_or_else(|error| panic!("read the {name} pragma: {error}"))
}

/// Reads a text pragma from an open connection.
fn pragma_text(db: &Database, name: &str) -> String {
    db.conn
        .pragma_query_value(None, name, |row| row.get::<_, String>(0))
        .unwrap_or_else(|error| panic!("read the {name} pragma: {error}"))
}

#[test]
fn test_new_file_is_created_at_the_current_version() {
    let temp = TempDbDir::new();
    let db = Database::new_at(&temp.db_path()).expect("create the database");

    assert!(!db.was_rebuilt(), "a new file is created, not rebuilt");
    assert_eq!(user_version(&db), SCHEMA_VERSION);
    assert_eq!(row_count(&db, "SONGS"), 0);
}

#[test]
fn test_matching_version_keeps_the_rows() {
    let temp = TempDbDir::new();
    let path = temp.db_path();

    {
        let mut db = Database::new_at(&path).expect("create the database");
        insert_song(&mut db);
    }

    let db = Database::new_at(&path).expect("reopen the database");

    assert!(!db.was_rebuilt(), "a matching version is a pure open");
    assert_eq!(user_version(&db), SCHEMA_VERSION);
    assert_eq!(
        row_count(&db, "SONGS"),
        1,
        "the row written by the first open survives"
    );
}

#[test]
fn test_mismatched_version_deletes_the_three_files_and_recreates() {
    let temp = TempDbDir::new();
    let path = temp.db_path();

    {
        let mut db = Database::new_at(&path).expect("create the database");
        insert_song(&mut db);
    }
    stamp_version(&path, 0);

    // A WAL database leaves its `-wal`/`-shm` sidecars on disk between runs, so
    // the rebuild is exercised with all three files present: the fresh database
    // must still open clean and hold no row from the discarded file.
    std::fs::write(sibling_path(&path, "-wal"), b"stale wal").expect("write the wal sidecar");
    std::fs::write(sibling_path(&path, "-shm"), b"stale shm").expect("write the shm sidecar");

    let db = Database::new_at(&path).expect("rebuild the database");

    assert!(db.was_rebuilt(), "an older version is rebuilt");
    assert_eq!(user_version(&db), SCHEMA_VERSION);
    assert_eq!(
        row_count(&db, "SONGS"),
        0,
        "the rebuilt file holds no songs"
    );
    assert_eq!(
        row_count(&db, "ALBUMS"),
        0,
        "the rebuilt file holds no albums"
    );
}

#[test]
fn test_unreadable_file_is_treated_as_incompatible() {
    let temp = TempDbDir::new();
    let path = temp.db_path();
    std::fs::write(&path, b"these bytes are not a SQLite database").expect("write the fake file");

    let db = Database::new_at(&path).expect("a file that is not a database is replaced");

    assert!(
        db.was_rebuilt(),
        "an unreadable file is rebuilt instead of trusted"
    );
    assert_eq!(user_version(&db), SCHEMA_VERSION);
    assert_eq!(row_count(&db, "SONGS"), 0);
}

#[test]
fn test_unopenable_path_is_reported_not_deleted() {
    let temp = TempDbDir::new();
    let path = temp.db_path();
    std::fs::create_dir(&path).expect("create a directory where the database should be");

    let result = Database::new_at(&path);

    assert!(result.is_err(), "a path that cannot be opened is an error");
    assert!(
        path.is_dir(),
        "a path that is not an incompatible file is never deleted"
    );
}

#[test]
fn test_in_memory_database_sets_the_version_without_a_probe() {
    let db = Database::new_memory().expect("create the in-memory database");

    assert!(
        !db.was_rebuilt(),
        "an in-memory database has no file to rebuild"
    );
    assert_eq!(user_version(&db), SCHEMA_VERSION);
}

#[test]
fn test_pragmas_are_applied_on_both_connection_kinds() {
    let temp = TempDbDir::new();
    let file_db = Database::new_at(&temp.db_path()).expect("create the database");
    let memory_db = Database::new_memory().expect("create the in-memory database");

    for db in [&file_db, &memory_db] {
        assert_eq!(
            pragma_i64(db, "synchronous"),
            1,
            "synchronous must stay NORMAL"
        );
        assert_eq!(
            pragma_i64(db, "cache_size"),
            -8000,
            "the page cache must stay at 8 MiB"
        );
        assert_eq!(
            pragma_i64(db, "foreign_keys"),
            1,
            "foreign keys must stay enforced"
        );
    }

    assert_eq!(pragma_text(&file_db, "journal_mode"), "wal");
    assert_eq!(pragma_i64(&file_db, "mmap_size"), 16_777_216);

    // An in-memory database has no file to journal or map, so SQLite reports the
    // journal as `memory` and answers the mapping pragma with no row at all.
    assert_eq!(pragma_text(&memory_db, "journal_mode"), "memory");
    assert_eq!(
        memory_db
            .conn
            .query_row("PRAGMA mmap_size", [], |row| row.get::<_, i64>(0))
            .optional()
            .expect("query the in-memory mapping size"),
        None,
        "an in-memory database has no mapping size to report"
    );
}

#[test]
fn test_second_connection_on_a_rebuilt_file_does_not_rebuild_again() {
    let temp = TempDbDir::new();
    let path = temp.db_path();
    std::fs::write(&path, b"a file written by another schema").expect("write the stale file");

    drop(Database::new_at(&path).expect("rebuild the database"));
    let second = Database::new_at(&path).expect("open the rebuilt database");

    assert!(
        !second.was_rebuilt(),
        "the scanner connection must not rebuild the file the GUI keeps open"
    );
    assert_eq!(user_version(&second), SCHEMA_VERSION);
}

#[test]
fn test_matching_version_deletes_nothing() {
    let temp = TempDbDir::new();
    let path = temp.db_path();

    {
        let mut db = Database::new_at(&path).expect("create the database");
        insert_song(&mut db);
    }
    let length_before = std::fs::metadata(&path).expect("stat the database").len();

    let db = Database::new_at(&path).expect("reopen the database");
    let length_after = std::fs::metadata(&path)
        .expect("stat the reopened database")
        .len();

    assert!(!db.was_rebuilt(), "a matching version is a pure open");
    assert_eq!(row_count(&db, "SONGS"), 1, "the song survives the reopen");
    assert!(
        length_after >= length_before,
        "the main file is never truncated: {length_after} < {length_before}"
    );
}

#[test]
fn test_absent_path_creates_the_parentless_file() {
    let temp = TempDbDir::new();
    let path = temp.dir().join("missing").join("library.db");

    let result = Database::new_at(&path);

    assert!(
        result.is_err(),
        "a path whose parent does not exist is a typed failure, not a panic"
    );
    assert!(!path.exists(), "no database is created somewhere else");
}
