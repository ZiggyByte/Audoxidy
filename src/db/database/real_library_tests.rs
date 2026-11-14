//! Measurements over a copy of the real music library.
//!
//! The application database is never opened for writing: the source is opened
//! read-only and copied with `VACUUM INTO`, which reads a consistent snapshot
//! through the write-ahead log without leaving anything behind. The copy is
//! opened read-only for the measurement tests and wrapped directly in
//! [`Database`], bypassing the version gate: the versioned open path would
//! classify the copy and rebuild it, destroying the very rows the tests measure.
//! The full-text repair test opens the copy read-write, because rebuilding the
//! index writes to it; that is the only writable handle in this file, and the
//! source is never reopened.
//!
//! The copy is written to the temporary directory under a name composed only of
//! the temporary directory and the process id, and the destination is bound as a
//! statement parameter, so no user input reaches the `VACUUM INTO` statement.
//!
//! The query-plan test prepares copies of the statements in `library.rs` and
//! `playlists.rs`. The recorded plan is evidence, not an assertion: statistics
//! change plans without changing results, so the verdict it supports expires
//! when one of those statements changes and has to be re-derived.
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

    let rows: i64 = Connection::open_with_flags(&copy, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .and_then(|connection| {
            connection.query_row("SELECT COUNT(*) FROM SONGS", [], |row| row.get(0))
        })
        .unwrap_or(0);
    if rows == 0 {
        println!("real library copy is empty: measurements skipped");
        let _ = std::fs::remove_file(&copy);
        return None;
    }

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

/// Wraps the copy in a [`Database`] opened for writing.
///
/// The copy is a temporary file this process owns. The source was read-only when
/// it produced the copy and is never reopened, so this is the only writable
/// handle the tests hold.
fn copy_database_writable(copy: &Path) -> Database {
    let conn = Connection::open(copy).expect("the library copy must open read-write");
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

/// Returns whether the copy holds the full-text content table.
fn has_fts_content(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'SONGS_FTS_CONTENT'",
        [],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0)
        > 0
}

/// Returns whether the copy still stores `ALBUMS.cover_path`.
fn has_legacy_cover_path(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('ALBUMS') WHERE name = 'cover_path'",
        [],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0)
        > 0
}

/// Describes the shape of the copy: version, full-text content table, cover
/// column and the row counts the measurements are read against.
fn schema_shape(conn: &Connection) -> String {
    let user_version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap_or(-1);
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
        if has_fts_content(conn) {
            "present"
        } else {
            "absent"
        },
        if has_legacy_cover_path(conn) {
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

#[test]
fn test_real_library_read_paths_return_every_song() {
    let _guard = measurement_lock();
    let Some(copy) = library_copy() else {
        println!("real library absent: read paths not checked");
        return;
    };
    let db = copy_database(&copy);
    println!("measured library shape: {}", schema_shape(&db.conn));

    let library_count: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM SONGS WHERE is_external = 0",
            [],
            |row| row.get(0),
        )
        .expect("the library count must be read");
    let album_count: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(DISTINCT al.id) FROM ALBUMS al
             JOIN SONGS s ON s.album_id = al.id
             WHERE s.is_external = 0",
            [],
            |row| row.get(0),
        )
        .expect("the album count must be read");

    let songs = db.get_all_songs().expect("the library rows must be read");
    let albums = db.get_all_albums().expect("the album rows must be read");

    assert_eq!(
        songs.len() as i64,
        library_count,
        "the full read returns exactly the non-external rows"
    );
    assert_eq!(
        albums.len() as i64,
        album_count,
        "the album read returns one row per album the library shows"
    );

    let cache_dir = &crate::utils::paths::get().cover_cache_dir;
    for song in &songs {
        assert!(
            !song.full_file_path.is_empty(),
            "every library row names a file"
        );
        if let Some(path) = song.compressed_cached_cover_root.as_deref() {
            let relative = Path::new(path)
                .strip_prefix(cache_dir)
                .expect("a resolved cover path lives under the cover cache directory");
            assert_eq!(
                relative.components().count(),
                1,
                "a resolved cover path adds one file name inside the cache directory: {path}"
            );
        }
    }

    println!(
        "real library read paths: songs={} albums={}",
        songs.len(),
        albums.len()
    );
    remove_copy(&copy);
}

#[test]
fn test_real_library_fts_index_is_consistent() {
    let _guard = measurement_lock();
    let Some(copy) = library_copy() else {
        println!("real library absent: full-text index not checked");
        return;
    };
    let db = copy_database_writable(&copy);
    println!("measured library shape: {}", schema_shape(&db.conn));

    if !has_fts_content(&db.conn) {
        println!("real library full-text check skipped: the copy has no content table");
        remove_copy(&copy);
        return;
    }

    let content_rows = db
        .fts_content_count()
        .expect("the content rows must be counted");
    let song_rows = db.songs_count().expect("the song rows must be counted");
    println!("real library full-text: content_rows={content_rows} song_rows={song_rows}");
    assert!(
        db.is_fts_consistent()
            .expect("the consistency check must run"),
        "every song must have exactly one content row"
    );

    db.rebuild_fts_index()
        .expect("the index must rebuild from the content table");
    db.verify_fts_integrity()
        .expect("the rebuilt index must match its content table");
    println!("real library full-text: rebuild and integrity check passed");

    remove_copy(&copy);
}

#[test]
fn test_real_library_cover_names_match_the_derivation() {
    let _guard = measurement_lock();
    let Some(copy) = library_copy() else {
        println!("real library absent: cover names not compared");
        return;
    };
    let db = copy_database(&copy);
    println!("measured library shape: {}", schema_shape(&db.conn));

    if has_legacy_cover_path(&db.conn) {
        let mut statement = db
            .conn
            .prepare(
                "SELECT hash_id, cover_hash, cover_path FROM ALBUMS WHERE cover_hash IS NOT NULL",
            )
            .expect("the cover rows must prepare");
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .expect("the cover rows must be read");

        let mut composed_rows = 0usize;
        let mut compared = 0usize;
        let mut mismatches = 0usize;
        for row in rows {
            let (album_hash, cover_hash, stored_path) = row.expect("the cover row must decode");
            let composed = crate::utils::covers::album_cover_file_name(&album_hash, &cover_hash)
                .expect("two hexadecimal hashes compose a file name");
            assert_eq!(
                Path::new(&composed).components().count(),
                1,
                "a composed cover name is a bare file name"
            );
            composed_rows += 1;

            let Some(stored_path) = stored_path else {
                continue;
            };
            compared += 1;
            let stored_name = Path::new(&stored_path)
                .file_name()
                .and_then(|name| name.to_str());
            if stored_name != Some(composed.as_str()) {
                mismatches += 1;
            }
        }

        println!(
            "real library cover names: legacy_path_column=present, composed={composed_rows}, compared={compared}, mismatches={mismatches}"
        );
        assert_eq!(
            mismatches, 0,
            "every stored cover path ends in the name its hashes compose"
        );
    } else {
        let cache_dir = &crate::utils::paths::get().cover_cache_dir;
        let mut statement = db
            .conn
            .prepare("SELECT hash_id, cover_hash FROM ALBUMS WHERE cover_hash IS NOT NULL")
            .expect("the cover rows must prepare");
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("the cover rows must be read");

        let mut composed_rows = 0usize;
        let mut present = 0usize;
        for row in rows {
            let (album_hash, cover_hash) = row.expect("the cover row must decode");
            let composed = crate::utils::covers::album_cover_file_name(&album_hash, &cover_hash)
                .expect("two hexadecimal hashes compose a file name");
            assert_eq!(
                Path::new(&composed).components().count(),
                1,
                "a composed cover name is a bare file name"
            );
            composed_rows += 1;
            if cache_dir.join(&composed).exists() {
                present += 1;
            }
        }

        println!(
            "real library cover names: legacy_path_column=absent, composed={composed_rows}, present_in_cache={present}"
        );
    }

    remove_copy(&copy);
}

/// One read-path statement, its label and the parameters it binds.
type PlanShape<'a> = (&'a str, &'a str, Vec<Box<dyn rusqlite::ToSql>>);

#[test]
fn test_real_library_query_plans_are_recorded() {
    let _guard = measurement_lock();
    let Some(copy) = library_copy() else {
        println!("real library absent: query plans not recorded");
        return;
    };
    let db = copy_database(&copy);
    println!("measured library shape: {}", schema_shape(&db.conn));

    let genre: String = db
        .conn
        .query_row(
            "SELECT al.genre FROM ALBUMS al WHERE al.genre IS NOT NULL AND al.genre <> '' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_default();
    let album_hash: String = db
        .conn
        .query_row("SELECT hash_id FROM ALBUMS LIMIT 1", [], |row| row.get(0))
        .unwrap_or_default();
    let playlist_id: i64 = db
        .conn
        .query_row(
            "SELECT id FROM PLAYLISTS ORDER BY sort_order ASC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(1);

    let mut shapes: Vec<PlanShape> = vec![
        ("all songs", PLAN_ALL_SONGS, Vec::new()),
        ("filtered songs", PLAN_FILTERED_SONGS, vec![Box::new(genre)]),
        ("grid by artist", PLAN_GRID_BY_ARTIST, Vec::new()),
        ("all albums", PLAN_ALL_ALBUMS, Vec::new()),
        ("album stats", PLAN_ALBUM_STATS, vec![Box::new(album_hash)]),
        (
            "playlist songs",
            PLAN_PLAYLIST_SONGS,
            vec![Box::new(playlist_id)],
        ),
        (
            "playlist songs paginated",
            PLAN_PLAYLIST_PAGINATED,
            vec![Box::new(playlist_id), Box::new(10_i64), Box::new(0_i64)],
        ),
        ("library stats", PLAN_LIBRARY_STATS, Vec::new()),
    ];
    if has_fts_content(&db.conn) {
        shapes.insert(
            2,
            (
                "filtered songs with the full-text join",
                PLAN_FILTERED_SONGS_FTS,
                vec![Box::new("\"a\" *".to_string())],
            ),
        );
    } else {
        println!("query plan skipped for the full-text join: the copy has no content table");
    }

    let mut recorded = 0usize;
    for (label, sql, parameters) in &shapes {
        let plan_sql = format!("EXPLAIN QUERY PLAN {sql}");
        let mut statement = db
            .conn
            .prepare(&plan_sql)
            .unwrap_or_else(|error| panic!("the plan of {label:?} must prepare: {error}"));
        let params: Vec<&dyn rusqlite::ToSql> =
            parameters.iter().map(|value| value.as_ref()).collect();
        let mut rows = statement
            .query(params.as_slice())
            .unwrap_or_else(|error| panic!("the plan of {label:?} must run: {error}"));

        let mut details = Vec::new();
        while let Some(row) = rows
            .next()
            .unwrap_or_else(|error| panic!("the plan of {label:?} must yield rows: {error}"))
        {
            details.push(
                row.get::<_, String>(3)
                    .expect("the plan detail must decode"),
            );
        }
        assert!(
            !details.is_empty(),
            "the plan of {label:?} yields at least one row"
        );
        recorded += 1;
        println!("query plan [{label}]: {}", details.join(" | "));
    }

    println!("real library query plans recorded: {recorded}");
    remove_copy(&copy);
}

/// The statement `get_all_songs` issues.
const PLAN_ALL_SONGS: &str = "
    SELECT s.id, s.file_path, s.title, ar.name, al.title, al.cover_hash,
           s.duration, s.format, s.size, s.track_num, s.bit_depth, s.sample_rate,
           s.channels, s.cover_override,
           al.genre, al.year, al_ar.name, al.hash_id
    FROM SONGS s
    JOIN ARTISTS ar ON s.artist_id = ar.id
    JOIN ALBUMS al ON s.album_id = al.id
    JOIN ARTISTS al_ar ON al.artist_id = al_ar.id
    WHERE s.is_external = 0
    ORDER BY ar.name COLLATE NOCASE ASC, al.year COLLATE NOCASE ASC, al.title COLLATE NOCASE ASC, s.track_num ASC
";

/// The statement `get_library_songs_filtered` issues with a genre filter.
const PLAN_FILTERED_SONGS: &str = "
    SELECT s.id, s.file_path, s.title,
           ar.name as artist_name, al.title as album_title, al.year, al.genre,
           s.track_num, s.duration, s.format, s.bit_depth, s.sample_rate,
           s.size, s.channels, s.cover_override,
           al.cover_hash, aar.name as album_artist_name, al.hash_id
    FROM SONGS s
    JOIN ARTISTS ar ON s.artist_id = ar.id
    JOIN ALBUMS al ON s.album_id = al.id
    LEFT JOIN ARTISTS aar ON al.artist_id = aar.id
    WHERE al.genre = ?1
    ORDER BY COALESCE(aar.name, ar.name) COLLATE NOCASE, al.year, al.title, CAST(s.track_num AS INTEGER), s.track_num
";

/// The same statement with the full-text join and match.
const PLAN_FILTERED_SONGS_FTS: &str = "
    SELECT s.id, s.file_path, s.title,
           ar.name as artist_name, al.title as album_title, al.year, al.genre,
           s.track_num, s.duration, s.format, s.bit_depth, s.sample_rate,
           s.size, s.channels, s.cover_override,
           al.cover_hash, aar.name as album_artist_name, al.hash_id
    FROM SONGS s
    JOIN ARTISTS ar ON s.artist_id = ar.id
    JOIN ALBUMS al ON s.album_id = al.id
    LEFT JOIN ARTISTS aar ON al.artist_id = aar.id
    JOIN SONGS_FTS fts ON fts.rowid = s.id
    WHERE SONGS_FTS MATCH ?1
    ORDER BY COALESCE(aar.name, ar.name) COLLATE NOCASE, al.year, al.title, CAST(s.track_num AS INTEGER), s.track_num
";

/// The statement `get_grid_items_by_artist` issues.
const PLAN_GRID_BY_ARTIST: &str = "
    SELECT al.hash_id, al.title, ar.name, al.genre, al.year, al.cover_hash
    FROM ALBUMS al
    JOIN ARTISTS ar ON al.artist_id = ar.id
    JOIN SONGS s ON s.album_id = al.id
    WHERE s.is_external = 0
    GROUP BY al.id
    ORDER BY ar.name COLLATE NOCASE ASC, al.year COLLATE NOCASE ASC, al.title COLLATE NOCASE ASC
";

/// The statement `get_all_albums` issues.
const PLAN_ALL_ALBUMS: &str = "
    SELECT DISTINCT al.hash_id, al.title, ar.name, al.genre, al.year, al.cover_hash
    FROM ALBUMS al
    JOIN ARTISTS ar ON al.artist_id = ar.id
    JOIN SONGS s ON s.album_id = al.id
    WHERE s.is_external = 0
    ORDER BY ar.name COLLATE NOCASE ASC, al.year COLLATE NOCASE ASC, al.title COLLATE NOCASE ASC
";

/// The statement `get_album_stats_by_hash` issues.
const PLAN_ALBUM_STATS: &str = "
    SELECT COUNT(*), SUM(duration), SUM(size)
    FROM SONGS s
    JOIN ALBUMS al ON s.album_id = al.id
    WHERE al.hash_id = ?1
";

/// The statement `get_playlist_songs` issues.
const PLAN_PLAYLIST_SONGS: &str = "
    SELECT pi.id, pi.song_id, pi.sequence_order, pi.enabled = 1,
           s.title, ar.name, al.title, al_ar.name,
           al.year, al.genre, s.track_num, s.duration, s.file_path,
           al.cover_hash, s.cover_override,
           f.path, f.name, al.hash_id
    FROM PLAYLIST_ITEMS pi
    JOIN SONGS s ON s.id = pi.song_id
    JOIN ALBUMS al ON al.id = s.album_id
    JOIN ARTISTS ar ON ar.id = s.artist_id
    JOIN ARTISTS al_ar ON al_ar.id = al.artist_id
    JOIN FOLDERS f ON f.id = s.folder_id
    WHERE pi.playlist_id = ?1
    ORDER BY pi.sequence_order ASC
";

/// The statement `get_playlist_songs_paginated` issues.
const PLAN_PLAYLIST_PAGINATED: &str = "
    SELECT pi.id, pi.song_id, pi.sequence_order, pi.enabled = 1,
           s.title, ar.name, al.title, al_ar.name,
           al.year, al.genre, s.track_num, s.duration, s.file_path,
           al.cover_hash, s.cover_override,
           f.path, f.name, al.hash_id
    FROM PLAYLIST_ITEMS pi
    JOIN SONGS s ON s.id = pi.song_id
    JOIN ALBUMS al ON al.id = s.album_id
    JOIN ARTISTS ar ON ar.id = s.artist_id
    JOIN ARTISTS al_ar ON al_ar.id = al.artist_id
    JOIN FOLDERS f ON f.id = s.folder_id
    WHERE pi.playlist_id = ?1
    ORDER BY pi.sequence_order ASC
    LIMIT ?2 OFFSET ?3
";

/// The album count `get_library_stats` issues, the statement of that read whose
/// subquery is worth recording.
const PLAN_LIBRARY_STATS: &str = "
    SELECT COUNT(*) FROM ALBUMS WHERE id IN (SELECT album_id FROM SONGS WHERE is_external = 0)
";
