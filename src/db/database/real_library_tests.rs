//! Measurements over a copy of the real music library.
//!
//! The application database is never opened for writing: the source is opened
//! read-only and copied with `VACUUM INTO`, which reads a consistent snapshot
//! through the write-ahead log without leaving anything behind. The copy is
//! opened read-only as well and wrapped directly in [`Database`], bypassing the
//! version gate: the versioned open path would classify the copy and rebuild it,
//! destroying the very rows the tests measure.
//!
//! The copy is written to the temporary directory under a name composed only of
//! the temporary directory and the process id, and the destination is bound as a
//! statement parameter, so no user input reaches the `VACUUM INTO` statement.
//!
//! Every measurement line is printed with `println!`. The test process installs
//! no tracing subscriber, so a tracing event would be discarded and `--nocapture`
//! would show nothing.

use super::{Database, LibrarySearchParams, PlaylistSongRef, SongData};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Path of the application database, relative to the crate root tests run in.
fn library_source() -> PathBuf {
    PathBuf::from("library.db")
}

/// Copies the real library with `VACUUM INTO`, or `None` when it is absent.
///
/// The source and the copy are both opened read-only. The destination is
/// interpolated as a bound parameter supported by SQLite, and it comes entirely
/// from the temporary directory plus the process id, never from user input.
fn library_copy() -> Option<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    let source = library_source();
    if !source.exists() {
        return None;
    }

    let copy = std::env::temp_dir().join(format!(
        "audoxidy-real-library-{}-{}.db",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_file(&copy);

    let reader = Connection::open_with_flags(&source, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("the library copy source must open read-only");
    reader
        .execute("VACUUM INTO ?1", [copy.to_string_lossy().as_ref()])
        .expect("VACUUM INTO must copy the library into the temporary file");
    drop(reader);

    Some(copy)
}

/// Wraps the copy in a [`Database`] without running the version gate.
fn copy_database(copy: &Path) -> Database {
    let conn = Connection::open_with_flags(copy, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("the library copy must open read-only");
    Database {
        conn,
        rebuilt: false,
    }
}

/// Removes the copy and any sibling SQLite may have created next to it.
fn remove_copy(copy: &Path) {
    let _ = std::fs::remove_file(copy);
    let _ = std::fs::remove_file(format!("{}-wal", copy.display()));
    let _ = std::fs::remove_file(format!("{}-shm", copy.display()));
}

/// Describes the shape of the copy: version, full-text content table, cover
/// column and the row counts the measurements are read against.
fn schema_shape(conn: &Connection) -> String {
    let user_version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap_or(-1);
    let fts_content: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'SONGS_FTS_CONTENT'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let cover_path_column: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('ALBUMS') WHERE name = 'cover_path'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let songs: i64 = conn
        .query_row("SELECT COUNT(*) FROM SONGS", [], |row| row.get(0))
        .unwrap_or(0);
    let library_songs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM SONGS WHERE is_external = 0",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let albums: i64 = conn
        .query_row("SELECT COUNT(*) FROM ALBUMS", [], |row| row.get(0))
        .unwrap_or(0);
    let artists: i64 = conn
        .query_row("SELECT COUNT(*) FROM ARTISTS", [], |row| row.get(0))
        .unwrap_or(0);
    let playlists: i64 = conn
        .query_row("SELECT COUNT(*) FROM PLAYLISTS", [], |row| row.get(0))
        .unwrap_or(0);
    let playlist_items: i64 = conn
        .query_row("SELECT COUNT(*) FROM PLAYLIST_ITEMS", [], |row| row.get(0))
        .unwrap_or(0);

    format!(
        "user_version={user_version}, SONGS_FTS_CONTENT={}, ALBUMS.cover_path={}, SONGS={songs} (library {library_songs}), ALBUMS={albums}, ARTISTS={artists}, PLAYLISTS={playlists}, PLAYLIST_ITEMS={playlist_items}",
        if fts_content > 0 { "present" } else { "absent" },
        if cover_path_column > 0 {
            "present"
        } else {
            "absent"
        }
    )
}

/// Reads the private anonymous residency of this process in KiB.
///
/// `Pss_Anon` is the metric the memory audit reports. `Rss` is used only when
/// the kernel does not expose the anonymous line, and `None` means the kernel
/// does not expose `/proc/self/smaps_rollup` at all.
fn anon_residency_kib() -> Option<u64> {
    let rollup = std::fs::read_to_string("/proc/self/smaps_rollup").ok()?;
    parse_anon_residency_kib(&rollup)
}

/// Parses the anonymous residency out of a `smaps_rollup` body.
fn parse_anon_residency_kib(rollup: &str) -> Option<u64> {
    let mut rss_kib = None;
    for line in rollup.lines() {
        if let Some(value) = line.strip_prefix("Pss_Anon:")
            && let Some(kib) = value.split_whitespace().next().and_then(|v| v.parse().ok())
        {
            return Some(kib);
        }
        if rss_kib.is_none()
            && let Some(value) = line.strip_prefix("Rss:")
        {
            rss_kib = value.split_whitespace().next().and_then(|v| v.parse().ok());
        }
    }
    rss_kib
}

/// Returns the median of three or more samples.
fn median_ms(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(|a, b| a.total_cmp(b));
    samples[samples.len() / 2]
}

/// Serializes the measurement tests: the residency reading is process-wide, so
/// a concurrently running test would be attributed to the library.
fn measurement_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn test_song_row_size_is_recorded() {
    let size = std::mem::size_of::<SongData>();
    println!("SongData struct size: {size} bytes");

    // The row draws nothing from `folder_id`, `artist_id`, `album_id` or
    // `cover_override`; their removal is 3 × i64 plus one Option<Arc<str>>.
    // The ceiling is the measured row plus one pointer pair of margin, so a
    // field that returns to the row is caught instead of absorbed.
    assert!(
        size <= 288,
        "the library row must stay under its tightened ceiling, got {size} bytes"
    );
}

#[test]
fn test_real_library_anon_residency_is_measured() {
    let _guard = measurement_lock();
    let Some(copy) = library_copy() else {
        println!("real library absent: anonymous residency not measured");
        return;
    };
    let db = copy_database(&copy);
    println!("measured library shape: {}", schema_shape(&db.conn));

    let before = anon_residency_kib();
    let songs = db.get_all_songs().expect("the library rows must be read");
    std::hint::black_box(&songs);
    let held = anon_residency_kib();

    let rows = songs.len();
    match (before, held) {
        (Some(before), Some(held)) => {
            let delta = held as i64 - before as i64;
            let per_row_bytes = if rows > 0 {
                delta * 1024 / rows as i64
            } else {
                0
            };
            println!(
                "Pss_Anon before materializing: {before} KiB; with {rows} rows held: {held} KiB; delta: {delta} KiB; per row: {per_row_bytes} B"
            );
        }
        _ => println!(
            "Pss_Anon unavailable on this platform: residency not measured for {rows} rows"
        ),
    }

    drop(songs);
    remove_copy(&copy);
}

#[test]
fn test_real_library_read_paths_are_measured() {
    let _guard = measurement_lock();
    let Some(copy) = library_copy() else {
        println!("real library absent: read paths not measured");
        return;
    };
    let db = copy_database(&copy);
    let shape = schema_shape(&db.conn);
    println!("measured library shape: {shape}");

    let library_count: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM SONGS WHERE is_external = 0",
            [],
            |row| row.get(0),
        )
        .expect("the library count must be read");

    let genre: Option<String> = db
        .conn
        .query_row(
            "SELECT al.genre FROM ALBUMS al WHERE al.genre IS NOT NULL AND al.genre <> '' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();
    let params = LibrarySearchParams {
        genre: genre.clone(),
        ..Default::default()
    };

    let mut all_samples = Vec::with_capacity(3);
    let mut all_songs = Vec::new();
    for _ in 0..3 {
        let started = Instant::now();
        all_songs = db
            .get_all_songs()
            .expect("the full library read must succeed");
        all_samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }

    let mut filtered_samples = Vec::with_capacity(3);
    let mut filtered_songs = Vec::new();
    for _ in 0..3 {
        let started = Instant::now();
        filtered_songs = db
            .get_library_songs_filtered(&params)
            .expect("the filtered library read must succeed");
        filtered_samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }

    let filtered_label = match genre.as_deref() {
        Some(genre) => format!("genre={genre:?}"),
        None => "no filter".to_string(),
    };
    println!(
        "read path get_all_songs median: {:.3} ms; get_library_songs_filtered ({filtered_label}) median: {:.3} ms; songs: {}",
        median_ms(all_samples),
        median_ms(filtered_samples),
        all_songs.len()
    );

    let expected_filtered: i64 = if let Some(genre) = genre.as_deref() {
        db.conn
            .query_row(
                "SELECT COUNT(*) FROM SONGS s
                 JOIN ARTISTS ar ON s.artist_id = ar.id
                 JOIN ALBUMS al ON s.album_id = al.id
                 LEFT JOIN ARTISTS aar ON al.artist_id = aar.id
                 WHERE al.genre = ?1",
                [genre],
                |row| row.get(0),
            )
            .expect("the filtered count must be read")
    } else {
        db.conn
            .query_row(
                "SELECT COUNT(*) FROM SONGS s
                 JOIN ARTISTS ar ON s.artist_id = ar.id
                 JOIN ALBUMS al ON s.album_id = al.id
                 LEFT JOIN ARTISTS aar ON al.artist_id = aar.id",
                [],
                |row| row.get(0),
            )
            .expect("the unfiltered count must be read")
    };

    assert_eq!(
        all_songs.len() as i64,
        library_count,
        "the full read returns exactly the non-external rows"
    );
    assert_eq!(
        filtered_songs.len() as i64,
        expected_filtered,
        "the filtered read returns exactly the rows its join shape selects"
    );

    remove_copy(&copy);
}

#[test]
fn test_playlist_read_paths_are_measured() {
    let _guard = measurement_lock();
    let Some(copy) = library_copy() else {
        println!("real library absent: playlist read paths not measured");
        return;
    };
    let db = copy_database(&copy);
    println!("measured library shape: {}", schema_shape(&db.conn));
    println!(
        "PlaylistSongRef struct size: {} bytes",
        std::mem::size_of::<PlaylistSongRef>()
    );

    let playlists = db.get_all_playlists().expect("the playlists must be read");

    for playlist in &playlists {
        let mut samples = Vec::with_capacity(3);
        let mut items = 0;
        for _ in 0..3 {
            let started = Instant::now();
            let songs = db
                .get_playlist_songs(playlist.id)
                .expect("the playlist read must succeed");
            samples.push(started.elapsed().as_secs_f64() * 1000.0);
            items = songs.len();
        }
        println!(
            "playlist {:?} (id {}, system {}): {items} items, median {:.3} ms",
            playlist.name,
            playlist.id,
            playlist.is_system,
            median_ms(samples)
        );
    }

    let before = anon_residency_kib();
    let held: Vec<Vec<PlaylistSongRef>> = playlists
        .iter()
        .map(|playlist| {
            db.get_playlist_songs(playlist.id)
                .expect("the playlist rows must be read")
        })
        .collect();
    std::hint::black_box(&held);
    let after = anon_residency_kib();

    let total_items: usize = held.iter().map(Vec::len).sum();
    match (before, after) {
        (Some(before), Some(after)) => {
            let delta = after as i64 - before as i64;
            let per_item = if total_items > 0 {
                delta * 1024 / total_items as i64
            } else {
                0
            };
            println!(
                "Pss_Anon before playlists: {before} KiB; with {total_items} items held: {after} KiB; delta: {delta} KiB; per item: {per_item} B"
            );
        }
        _ => println!(
            "Pss_Anon unavailable on this platform: playlist residency not measured for {total_items} items"
        ),
    }

    drop(held);
    remove_copy(&copy);
}
