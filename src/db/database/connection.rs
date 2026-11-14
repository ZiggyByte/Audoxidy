//! SQLite connection lifecycle for the library database.

use rusqlite::{Connection, ErrorCode, Result};

use super::schema::SCHEMA_VERSION;

/// Outcome of inspecting the schema version recorded in an existing file.
enum VersionProbe {
    /// Nothing exists at the path, so the schema is created from scratch.
    Absent,
    /// The file was written by the current schema and opens as it is.
    Current,
    /// The file holds another version, or its bytes are not a database.
    Incompatible,
    /// The file could not be inspected for a reason unrelated to its contents.
    Failed(rusqlite::Error),
}

/// Builds the `-wal`/`-shm` sibling path of a database file.
fn sibling_path(path: &std::path::Path, suffix: &str) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    std::path::PathBuf::from(name)
}

/// Classifies a failure raised while reading `user_version`.
///
/// Only a failure that says the file itself is unusable — it is not a database,
/// or its header is corrupt — authorizes the destructive path. A busy, locked,
/// permission-denied or I/O failure says nothing about the schema, so it is kept
/// as a transient error.
fn classify_probe_error(error: rusqlite::Error) -> VersionProbe {
    let file_is_unusable = matches!(
        &error,
        rusqlite::Error::SqliteFailure(code, _)
            if matches!(code.code, ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt)
    );
    if file_is_unusable {
        VersionProbe::Incompatible
    } else {
        VersionProbe::Failed(error)
    }
}

/// Reads the schema version of an existing file through a throwaway connection.
fn probe_user_version(path: &std::path::Path) -> VersionProbe {
    if !path.exists() {
        return VersionProbe::Absent;
    }

    // The connection is closed when this function returns, so no handle survives
    // onto a file the caller may unlink.
    let conn = match Connection::open(path) {
        Ok(conn) => conn,
        Err(error) => return classify_probe_error(error),
    };

    match conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0)) {
        Ok(version) if version == SCHEMA_VERSION => VersionProbe::Current,
        Ok(_) => VersionProbe::Incompatible,
        Err(error) => classify_probe_error(error),
    }
}

/// SQLite database of the Audoxidy music library.
///
/// Holds the relational schema for folders, artists, albums, songs, extended
/// metadata, playlists and shuffle sessions. It runs in WAL mode, keeps an FTS5
/// index for full-text search and applies the connection pragmas the read paths
/// depend on.
pub struct Database {
    pub(in crate::db::database) conn: Connection,
    /// Whether this open replaced a stale or unreadable file.
    pub(in crate::db::database) rebuilt: bool,
}

impl Default for Database {
    fn default() -> Self {
        Self::new().expect("No se pudo iniciar la DB local SQLite")
    }
}

impl Database {
    /// Opens or creates the library database at the configured path.
    ///
    /// Tests must inject a path: the version gate rebuilds an incompatible file,
    /// so opening the default path from a test would delete the real library.
    pub fn new() -> Result<Self> {
        #[cfg(test)]
        {
            panic!(
                "tests must not open the default library path: the schema version gate would rebuild and delete it; use new_at or new_memory"
            );
        }
        #[cfg(not(test))]
        {
            Self::open_at(&crate::utils::paths::get().db_path)
        }
    }

    /// Opens the database at `path`, replacing the file when it is incompatible.
    ///
    /// The version is probed before the writable connection exists: a matching
    /// value is a pure open, a missing file is created, and a file from another
    /// schema version — or one that cannot be read as a database — has its main
    /// file and its `-wal`/`-shm` siblings removed so a fresh schema can be
    /// built. A failure that says nothing about the file, such as a lock held by
    /// another instance, is returned untouched.
    ///
    /// The version is stamped after the schema is created. A crash between the
    /// two leaves version 0, which the next open rebuilds instead of trusting a
    /// half-built file.
    fn open_at(path: &std::path::Path) -> Result<Self> {
        let rebuilt = match probe_user_version(path) {
            VersionProbe::Incompatible => {
                let _ = std::fs::remove_file(path);
                let _ = std::fs::remove_file(sibling_path(path, "-wal"));
                let _ = std::fs::remove_file(sibling_path(path, "-shm"));
                true
            }
            VersionProbe::Failed(error) => return Err(error),
            VersionProbe::Absent | VersionProbe::Current => false,
        };

        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        conn.set_prepared_statement_cache_capacity(32);
        Self::create_schema(&conn)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Self { conn, rebuilt })
    }

    /// Creates an on-disk database at an arbitrary path.
    ///
    /// The entry point injects the resolved library path, and tests use it to get
    /// their own file-backed database.
    pub fn new_at(path: &std::path::Path) -> Result<Self> {
        Self::open_at(path)
    }

    /// Reports whether opening this database replaced an incompatible file.
    pub fn was_rebuilt(&self) -> bool {
        self.rebuilt
    }

    /// Refreshes the query planner's statistics with `PRAGMA optimize`.
    ///
    /// The pragma has exactly three sanctioned call points: after a scan
    /// completes, after a bulk metadata re-read, and on application close. It
    /// never runs at startup — a fresh database has nothing to analyze, and the
    /// post-scan call already covers the tables that grew.
    ///
    /// A table is analyzed only while one of SQLite's conditions holds for it:
    /// an index with no `sqlite_stat1` entry, a planner that already used the
    /// statistics on this connection, or the `0x10000` bit in the mask. The
    /// scanner's connection never prepares the library queries, so after the
    /// first analyze its calls would become permanent no-ops; that caller passes
    /// `0x10002`, which sets `0x10000` and adds the default bit.
    ///
    /// The close path ends in `std::process::exit(0)`, which skips every
    /// destructor, so the call there must be explicit in the close handler and
    /// must precede the exit: a hook attached to `Drop` would never run.
    ///
    /// `analysis_limit` is deliberately not set. SQLite 3.46 and newer apply
    /// their own temporary limit inside `PRAGMA optimize`, so a value assigned
    /// here would be inert.
    pub fn optimize_query_planner(&self, mask: Option<i64>) -> Result<()> {
        match mask {
            Some(mask) => self.conn.pragma_update(None, "optimize", mask),
            // The unmasked pragma can report what it analyzed, so the batch form
            // is used to discard any result row instead of tripping `execute`.
            None => self.conn.execute_batch("PRAGMA optimize"),
        }
    }

    /// Creates an in-memory database for tests.
    ///
    /// It never probes and never deletes: an in-memory database reports version 0
    /// before the schema exists, so the file check would classify every
    /// construction as a rebuild.
    #[cfg(test)]
    pub fn new_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        conn.set_prepared_statement_cache_capacity(32);
        Self::create_schema(&conn)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Self {
            conn,
            rebuilt: false,
        })
    }
}
