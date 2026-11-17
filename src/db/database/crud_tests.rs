//! Area tests for the persistence surface: albums, artists, folders, songs,
//! playlists, playlist items, shuffle sessions and settings.
//!
//! Every fixture row is written through the product's own entry points, so a
//! change that breaks the insert path breaks the fixture instead of slipping
//! past a hand-written statement. Assertions read back through the product's
//! read API; the only direct statements are the two updates the API does not
//! expose (an album title and an artist rename), which are the surface a
//! metadata editor will use.

use super::{Database, LibrarySearchParams, ShuffleSession, SongData, SongMetadataExtended};
use rusqlite::params;
use std::sync::Arc;

/// Folder every seeded album lives in.
const FOLDER: &str = "/music/crud";
/// First seeded album; its year starts null.
const FIRST_ALBUM: &str = "Crud Album";
/// Second seeded album; it belongs to the other artist.
const SECOND_ALBUM: &str = "Drift Album";
/// Track and album artist of the first album.
const ARTIST: &str = "Crud Artist";
/// Track and album artist of the second album.
const OTHER_ARTIST: &str = "Drift Artist";
/// Playlist the fixture fills with every seeded song.
const PLAYLIST: &str = "Crud List";
/// Path of the first seeded song.
const FIRST_SONG_PATH: &str = "/music/crud/one.flac";
/// Path of the second seeded song.
const SECOND_SONG_PATH: &str = "/music/crud/two.flac";
/// Path of the third seeded song.
const THIRD_SONG_PATH: &str = "/music/crud/three.flac";

/// One album row as the album read path returns it.
type AlbumRow = (String, String, String, String, String, Option<String>);

/// Builds one song row for the write funnel.
fn song(path: &str, title: &str, artist: &str, album: &str) -> SongData {
    SongData {
        full_file_path: Arc::from(path.to_string().into_boxed_str()),
        title: Some(Arc::from(title.to_string().into_boxed_str())),
        artist: Some(Arc::from(artist.to_string().into_boxed_str())),
        album_artist: Some(Arc::from(artist.to_string().into_boxed_str())),
        album: Some(Arc::from(album.to_string().into_boxed_str())),
        track_number: Some(Arc::from("1".to_string().into_boxed_str())),
        duration_secs: Some(90.0),
        format: Some(Arc::from("FLAC".to_string().into_boxed_str())),
        bit_depth: Some(24),
        sample_rate: Some(96_000),
        channels: Some(2),
        size: Some(1_000),
        import_order: 1,
        ..Default::default()
    }
}

/// Builds the deterministic fixture every test starts from.
///
/// Two artists, two albums in one folder and three songs, all written through
/// `insert_song_full`. The second song carries a picture that differs from the
/// album's, so the per-song override is exercised. The first album is seeded
/// without a year, which is the state a later insert fills. One playlist holds
/// all three songs and one shuffle session records a played and two pending
/// songs.
fn seeded() -> Database {
    let mut db = Database::new_memory().expect("the in-memory database must open");

    let mut first = song(FIRST_SONG_PATH, "Crud One", ARTIST, FIRST_ALBUM);
    first.genre = Some(Arc::from("Jazz".to_string().into_boxed_str()));
    db.insert_song_full(
        &first,
        &SongMetadataExtended::default(),
        Some("aaaa1111".to_string()),
        false,
    )
    .expect("the first song must insert");

    let mut second = song(SECOND_SONG_PATH, "Crud Two", ARTIST, FIRST_ALBUM);
    second.track_number = Some(Arc::from("2".to_string().into_boxed_str()));
    second.duration_secs = Some(120.0);
    second.size = Some(2_000);
    second.genre = Some(Arc::from("Jazz".to_string().into_boxed_str()));
    db.insert_song_full(
        &second,
        &SongMetadataExtended::default(),
        Some("bbbb2222".to_string()),
        false,
    )
    .expect("the second song must insert");

    let mut third = song(THIRD_SONG_PATH, "Crud Three", OTHER_ARTIST, SECOND_ALBUM);
    third.genre = Some(Arc::from("Rock".to_string().into_boxed_str()));
    third.release_year = Some(Arc::from("2001".to_string().into_boxed_str()));
    db.insert_song_full(&third, &SongMetadataExtended::default(), None, false)
        .expect("the third song must insert");

    let ids = [FIRST_SONG_PATH, SECOND_SONG_PATH, THIRD_SONG_PATH].map(|path| {
        db.get_song_id_by_path(path)
            .expect("the song id must be read")
            .unwrap_or_else(|| panic!("the seeded song {path:?} must exist"))
    });

    let playlist_id = db
        .create_playlist(PLAYLIST, false)
        .expect("the playlist must be created");
    db.add_songs_to_playlist(playlist_id, &ids)
        .expect("the seeded songs must enter the playlist");

    db.save_shuffle_session(
        playlist_id,
        &ShuffleSession {
            session_id: "crud-session".to_string(),
            shuffle_order: vec![ids[1], ids[2]],
            current_position: 1,
            history: vec![ids[0]],
        },
    )
    .expect("the shuffle session must be saved");

    db
}

/// Album row for one title, read through the product's album read path.
fn album_row(db: &Database, title: &str) -> AlbumRow {
    db.get_all_albums()
        .expect("the albums must be read")
        .into_iter()
        .find(|row| row.1 == title)
        .unwrap_or_else(|| panic!("the album {title:?} must exist"))
}

/// Album identity hash as the album read path reports it.
fn album_hash(db: &Database, title: &str) -> String {
    album_row(db, title).0
}

/// Song id by path, through the product's lookup.
fn song_id(db: &Database, path: &str) -> i64 {
    db.get_song_id_by_path(path)
        .expect("the song id must be read")
        .unwrap_or_else(|| panic!("the song {path:?} must exist"))
}

#[test]
fn test_album_create_read_update_delete() {
    let mut db = seeded();
    let hash = album_hash(&db, FIRST_ALBUM);

    // Read: the song insert created the album, and the album read path reports
    // its artist, genre and derived cover path.
    let created = album_row(&db, FIRST_ALBUM);
    assert_eq!(created.2, ARTIST, "the album carries its album artist");
    assert_eq!(
        created.3, "Jazz",
        "the album carries the genre of its songs"
    );
    assert!(
        created.5.is_some(),
        "the seeded picture gives the album a cover path"
    );

    let (songs, duration, size) = db
        .get_album_stats_by_hash(&hash)
        .expect("the album stats must be read");
    assert_eq!(songs, 2, "both songs of the first album are counted");
    assert_eq!(duration, 210.0);
    assert_eq!(size, 3_000.0);

    // Update by re-insert: the album upsert fills a year the first insert left
    // null. It never replaces a year it already holds, so the later insert keeps
    // the first value instead of overwriting it.
    let mut filled = song(FIRST_SONG_PATH, "Crud One", ARTIST, FIRST_ALBUM);
    filled.release_year = Some(Arc::from("1999".to_string().into_boxed_str()));
    filled.genre = Some(Arc::from("Jazz".to_string().into_boxed_str()));
    db.insert_song_full(&filled, &SongMetadataExtended::default(), None, false)
        .expect("the year must be filled through the write funnel");
    assert_eq!(
        album_row(&db, FIRST_ALBUM).4,
        "1999",
        "a null year is filled by the next insert"
    );

    let mut late = song(SECOND_SONG_PATH, "Crud Two", ARTIST, FIRST_ALBUM);
    late.track_number = Some(Arc::from("2".to_string().into_boxed_str()));
    late.duration_secs = Some(120.0);
    late.size = Some(2_000);
    late.release_year = Some(Arc::from("2020".to_string().into_boxed_str()));
    db.insert_song_full(&late, &SongMetadataExtended::default(), None, false)
        .expect("the later insert must succeed");
    assert_eq!(
        album_row(&db, FIRST_ALBUM).4,
        "1999",
        "the upsert keeps the first non-null year"
    );

    // Update by direct statement: no product API renames an album, and this is
    // the statement a later metadata editor will issue against the same row.
    let renamed = "Crud Album Retitled";
    db.conn
        .execute(
            "UPDATE ALBUMS SET title = ?1 WHERE hash_id = ?2",
            params![renamed, &hash],
        )
        .expect("the album title must be updated");
    assert_eq!(album_row(&db, renamed).0, hash);
    assert!(
        db.get_all_albums()
            .expect("the albums must be read")
            .iter()
            .all(|row| row.1 != FIRST_ALBUM),
        "the old title is gone from the read path"
    );

    // Delete: the whole group goes, and no song of it survives.
    db.delete_album_group(&hash)
        .expect("the album group must be deleted");
    assert!(
        db.get_all_albums()
            .expect("the albums must be read")
            .iter()
            .all(|row| row.0 != hash),
        "the deleted album is gone from the read path"
    );
    let remaining = db.get_all_songs().expect("the songs must be read");
    assert_eq!(remaining.len(), 1, "only the other album's song survives");
    assert_eq!(remaining[0].album.as_deref(), Some(SECOND_ALBUM));
}

#[test]
fn test_artist_create_read_rename_and_cleanup() {
    let db = seeded();
    let artist_id = db
        .upsert_artist(ARTIST)
        .expect("the seeded artist must resolve to its row");

    // Read: the insert created the artist and the album join resolves its name.
    assert_eq!(album_row(&db, FIRST_ALBUM).2, ARTIST);
    let groups = db
        .get_artist_groups_sql(None)
        .expect("the artist groups must be read");
    let group = groups
        .iter()
        .find(|row| row.0 == ARTIST)
        .expect("the seeded artist must appear in the groups");
    assert_eq!(group.1, 2, "both songs count toward the artist");
    assert_eq!(group.2, 1, "both songs belong to one album");
    assert_eq!(group.3, 210.0);

    // Update: no product API renames an artist. This is the statement a later
    // metadata editor will issue, and the one the full-text triggers cover.
    let renamed = "Crud Artist Renamed";
    db.conn
        .execute(
            "UPDATE ARTISTS SET name = ?1 WHERE id = ?2",
            params![renamed, artist_id],
        )
        .expect("the artist name must be updated");

    assert_eq!(
        album_row(&db, FIRST_ALBUM).2,
        renamed,
        "the album join reports the renamed artist"
    );
    let groups = db
        .get_artist_groups_sql(None)
        .expect("the artist groups must be read");
    assert!(
        groups.iter().any(|row| row.0 == renamed),
        "the renamed artist is listed"
    );
    assert!(
        groups.iter().all(|row| row.0 != ARTIST),
        "the old name is gone from the groups"
    );

    // Delete: cleanup removes an artist nothing references and keeps the one the
    // album and its songs still point to. A fresh insert of the orphan after the
    // cleanup receives a new identity, which proves the row was removed.
    let orphan_before = db
        .upsert_artist("Crud Orphan")
        .expect("the unreferenced artist must insert");
    db.cleanup_empty_metadata()
        .expect("the empty metadata must be cleaned up");
    let orphan_after = db
        .upsert_artist("Crud Orphan")
        .expect("the unreferenced artist must insert again");
    assert_ne!(
        orphan_before, orphan_after,
        "the unreferenced artist is removed by the cleanup"
    );
    assert_eq!(
        album_row(&db, FIRST_ALBUM).2,
        renamed,
        "the artist the album still references survives the cleanup"
    );
}

#[test]
fn test_folder_upsert_is_idempotent() {
    let db = seeded();

    let first = db
        .upsert_folder(FOLDER)
        .expect("the seeded folder must resolve");
    let second = db
        .upsert_folder(FOLDER)
        .expect("the seeded folder must resolve again");
    assert_eq!(first, second, "an existing path returns the same id");

    let folders = db.get_all_folders().expect("the folders must be read");
    let stored = folders
        .iter()
        .find(|row| row.1 == FOLDER)
        .expect("the seeded folder must be listed");
    assert_eq!(stored.0, first);
    assert_eq!(stored.2, "crud", "the name is the last path component");
    assert_eq!(
        folders.iter().filter(|row| row.1 == FOLDER).count(),
        1,
        "the folder is stored once"
    );

    // A path no song names is created once as well.
    let created = db
        .upsert_folder("/music/empty")
        .expect("the new folder must insert");
    assert_eq!(
        created,
        db.upsert_folder("/music/empty")
            .expect("the new folder must resolve"),
        "a repeated upsert adds no second row"
    );
    assert_eq!(
        db.get_all_folders()
            .expect("the folders must be read")
            .iter()
            .filter(|row| row.1 == "/music/empty")
            .count(),
        1
    );
}

#[test]
fn test_song_create_read_update_delete() {
    let mut db = seeded();
    let song_id = song_id(&db, FIRST_SONG_PATH);
    let hash = album_hash(&db, FIRST_ALBUM);

    // Read: the five song read paths return the seeded row with the same
    // identity the views group by.
    let reads = [
        db.get_all_songs().expect("the songs must be read"),
        db.get_library_songs_filtered(&LibrarySearchParams::default())
            .expect("the filtered songs must be read"),
        db.search_songs("Crud One")
            .expect("the full-text search must run"),
        db.get_songs_by_album(&hash)
            .expect("the album songs must be read"),
        db.get_songs_by_album_and_artist(&hash, ARTIST)
            .expect("the album songs of the artist must be read"),
    ];
    for songs in &reads {
        let row = songs
            .iter()
            .find(|song| song.id == song_id)
            .expect("the seeded song appears in every read path");
        assert_eq!(row.title.as_deref(), Some("Crud One"));
        assert_eq!(row.duration_secs, Some(90.0));
        assert_eq!(
            row.album_hash.as_deref(),
            Some(hash.as_str()),
            "every read path carries the album identity"
        );
    }

    // Update: re-inserting the same path is the supported update and adds no row.
    let mut updated = song(FIRST_SONG_PATH, "Crud One Updated", ARTIST, FIRST_ALBUM);
    updated.duration_secs = Some(95.5);
    updated.genre = Some(Arc::from("Jazz".to_string().into_boxed_str()));
    db.insert_song_full(&updated, &SongMetadataExtended::default(), None, false)
        .expect("the re-insert must update the song");

    let songs = db.get_all_songs().expect("the songs must be read");
    assert_eq!(songs.len(), 3, "the update did not add a row");
    let row = songs
        .iter()
        .find(|song| song.id == song_id)
        .expect("the updated song must exist");
    assert_eq!(row.title.as_deref(), Some("Crud One Updated"));
    assert_eq!(row.duration_secs, Some(95.5));

    // Delete: the row disappears from the read path and from the path lookup.
    db.batch_delete_songs(&[song_id])
        .expect("the song must be deleted");
    assert!(
        db.get_song_id_by_path(FIRST_SONG_PATH)
            .expect("the lookup must run")
            .is_none(),
        "the deleted path no longer resolves"
    );
    let remaining = db.get_all_songs().expect("the songs must be read");
    assert_eq!(remaining.len(), 2);
    assert!(remaining.iter().all(|song| song.id != song_id));
}

#[test]
fn test_playlist_create_read_update_delete() {
    let db = seeded();
    let playlist_id = db
        .create_playlist("Crud Second List", false)
        .expect("the playlist must be created");

    // Read by id and by name.
    let by_id = db
        .get_playlist_by_id(playlist_id)
        .expect("the playlist must be read")
        .expect("the created playlist must exist");
    assert_eq!(by_id.name, "Crud Second List");
    assert!(!by_id.is_system, "a created playlist is a user playlist");
    let by_name = db
        .get_playlist_by_name("Crud Second List")
        .expect("the playlist must be read by name")
        .expect("the created playlist must exist");
    assert_eq!(by_name.id, playlist_id);
    assert!(
        db.get_all_playlists()
            .expect("the playlists must be read")
            .iter()
            .any(|playlist| playlist.id == playlist_id)
    );

    // Update: rename, reorder and persist the playback state.
    assert!(
        db.rename_playlist(playlist_id, "Crud Renamed")
            .expect("the playlist must be renamed")
    );
    assert_eq!(
        db.get_playlist_by_id(playlist_id)
            .expect("the playlist must be read")
            .expect("the renamed playlist must exist")
            .name,
        "Crud Renamed"
    );

    db.update_playlist_order(playlist_id, 5)
        .expect("the playlist order must change");
    let song_id = song_id(&db, FIRST_SONG_PATH);
    db.update_playlist_persistence(
        playlist_id,
        Some(song_id),
        12.5,
        true,
        true,
        2,
        3,
        Some("crud-shuffle".to_string()),
    )
    .expect("the playback state must persist");
    let persisted = db
        .get_playlist_by_id(playlist_id)
        .expect("the playlist must be read")
        .expect("the persisted playlist must exist");
    assert_eq!(persisted.sort_order, 5);
    assert_eq!(persisted.last_song_id, Some(song_id));
    assert!((persisted.last_pos_sec - 12.5).abs() < f64::EPSILON);
    assert!(persisted.is_playing);
    assert!(persisted.shuffle_active);
    assert_eq!(persisted.repeat_mode, 2);
    assert_eq!(persisted.shuffle_pos, 3);
    assert_eq!(persisted.shuffle_id.as_deref(), Some("crud-shuffle"));

    // Delete: a user playlist goes, a system playlist refuses.
    assert!(
        db.delete_playlist(playlist_id)
            .expect("the playlist must be deleted")
    );
    assert!(
        db.get_playlist_by_id(playlist_id)
            .expect("the lookup must run")
            .is_none(),
        "the deleted playlist no longer reads"
    );
    assert!(
        db.get_playlist_by_name("Crud Renamed")
            .expect("the lookup must run")
            .is_none()
    );

    let system_id = db
        .get_playlist_by_name("Audoxidy")
        .expect("the system playlist must be read")
        .expect("the system playlist must exist")
        .id;
    assert!(
        !db.delete_playlist(system_id)
            .expect("the system playlist delete must answer"),
        "a system playlist is never deleted"
    );
    assert!(
        db.get_playlist_by_id(system_id)
            .expect("the lookup must run")
            .is_some()
    );
}

#[test]
fn test_playlist_items_add_remove_and_clear() {
    let db = seeded();
    let playlist_id = db
        .create_playlist("Crud Items", false)
        .expect("the playlist must be created");
    let ids = [
        song_id(&db, FIRST_SONG_PATH),
        song_id(&db, SECOND_SONG_PATH),
        song_id(&db, THIRD_SONG_PATH),
    ];

    for id in ids {
        assert!(
            db.add_song_to_playlist(playlist_id, id)
                .expect("the song must enter the playlist"),
            "a first add reports the insertion"
        );
    }
    assert!(
        !db.add_song_to_playlist(playlist_id, ids[0])
            .expect("the repeated add must answer"),
        "adding the same song twice is a no-op"
    );
    assert_eq!(
        db.get_playlist_song_count(playlist_id)
            .expect("the playlist count must be read"),
        3
    );

    // Read back in insertion order.
    let items = db
        .get_playlist_songs(playlist_id)
        .expect("the playlist items must be read");
    let stored: Vec<i64> = items.iter().map(|item| item.song_id).collect();
    assert_eq!(stored, ids, "the items read back in insertion order");
    assert!(
        items
            .windows(2)
            .all(|pair| pair[0].sequence_order < pair[1].sequence_order),
        "the sequence order is strictly increasing"
    );

    // Update: toggling reports the new state and the enabled read agrees.
    assert!(
        !db.toggle_song_enabled_in_playlist(playlist_id, ids[1])
            .expect("the toggle must answer"),
        "the toggle reports the song as disabled"
    );
    let toggled = db
        .get_playlist_songs(playlist_id)
        .expect("the playlist items must be read")
        .into_iter()
        .find(|item| item.song_id == ids[1])
        .expect("the toggled item must exist");
    assert!(!toggled.enabled);
    assert_eq!(
        db.get_playlist_enabled_songs(playlist_id)
            .expect("the enabled songs must be read"),
        vec![ids[0], ids[2]],
        "a disabled song leaves the enabled list"
    );
    assert!(
        db.toggle_song_enabled_in_playlist(playlist_id, ids[1])
            .expect("the toggle must answer"),
        "the second toggle re-enables the song"
    );

    // Delete one item, then all of them.
    assert!(
        db.remove_song_from_playlist(playlist_id, ids[1])
            .expect("the item must be removed")
    );
    assert_eq!(
        db.get_playlist_song_count(playlist_id)
            .expect("the playlist count must be read"),
        2
    );
    assert!(
        !db.is_song_in_playlist(playlist_id, ids[1])
            .expect("the membership must be checked")
    );
    assert!(
        !db.remove_song_from_playlist(playlist_id, ids[1])
            .expect("the repeated removal must answer"),
        "removing an absent item changes nothing"
    );

    db.clear_playlist(playlist_id)
        .expect("the playlist must be cleared");
    assert!(
        db.get_playlist_songs(playlist_id)
            .expect("the playlist items must be read")
            .is_empty(),
        "clearing removes every item"
    );

    let seeded_playlist = db
        .get_playlist_by_name(PLAYLIST)
        .expect("the seeded playlist must be read")
        .expect("the seeded playlist must exist");
    assert_eq!(
        db.get_playlist_song_count(seeded_playlist.id)
            .expect("the seeded playlist count must be read"),
        3,
        "clearing one playlist leaves the others untouched"
    );
}

#[test]
fn test_song_delete_cascades_to_playlist_items_and_history() {
    let db = seeded();
    let playlist_id = db
        .get_playlist_by_name(PLAYLIST)
        .expect("the seeded playlist must be read")
        .expect("the seeded playlist must exist")
        .id;
    let removed = song_id(&db, FIRST_SONG_PATH);

    let session = db
        .load_shuffle_session(playlist_id)
        .expect("the seeded session must load")
        .expect("the seeded session must exist");
    assert!(
        session.history.contains(&removed),
        "the played song is recorded in the history"
    );
    let items_before = db
        .get_playlist_song_count(playlist_id)
        .expect("the playlist count must be read");

    db.batch_delete_songs(&[removed])
        .expect("the song must be deleted");

    assert_eq!(
        db.get_playlist_song_count(playlist_id)
            .expect("the playlist count must be read"),
        items_before - 1,
        "the playlist item of the deleted song is gone"
    );
    assert!(
        !db.is_song_in_playlist(playlist_id, removed)
            .expect("the membership must be checked")
    );

    let session = db
        .load_shuffle_session(playlist_id)
        .expect("the session must load")
        .expect("the remaining session must exist");
    assert!(
        !session.history.contains(&removed),
        "the history row of the deleted song is gone"
    );
    assert!(
        !session.shuffle_order.contains(&removed),
        "the pending order no longer reaches the deleted song"
    );
    assert!(
        session.shuffle_order.len() + session.history.len() == 2,
        "the session keeps exactly the two surviving songs"
    );
}

#[test]
fn test_shuffle_session_round_trip_and_clear() {
    let mut db = seeded();
    let playlist_id = db
        .get_playlist_by_name(PLAYLIST)
        .expect("the seeded playlist must be read")
        .expect("the seeded playlist must exist")
        .id;
    let ids = [
        song_id(&db, FIRST_SONG_PATH),
        song_id(&db, SECOND_SONG_PATH),
        song_id(&db, THIRD_SONG_PATH),
    ];

    let session = ShuffleSession {
        session_id: "round-trip-session".to_string(),
        shuffle_order: vec![ids[2], ids[0], ids[1]],
        current_position: 2,
        history: vec![ids[1]],
    };
    db.save_shuffle_session(playlist_id, &session)
        .expect("the session must be saved");

    let loaded = db
        .load_shuffle_session(playlist_id)
        .expect("the session must load")
        .expect("the saved session must exist");
    assert_eq!(loaded.session_id, session.session_id);
    assert_eq!(
        loaded.shuffle_order, session.shuffle_order,
        "the pending order survives the round trip in the same order"
    );
    assert_eq!(loaded.current_position, session.current_position);
    assert_eq!(loaded.history, session.history);

    let playlist = db
        .get_playlist_by_id(playlist_id)
        .expect("the playlist must be read")
        .expect("the seeded playlist must exist");
    assert_eq!(playlist.shuffle_id.as_deref(), Some("round-trip-session"));
    assert_eq!(playlist.shuffle_pos, 2);

    db.clear_shuffle_session(playlist_id)
        .expect("the session must be cleared");
    assert!(
        db.load_shuffle_session(playlist_id)
            .expect("the lookup must run")
            .is_none(),
        "a cleared session no longer loads"
    );
}

#[test]
fn test_settings_add_update_delete_and_batch_read() {
    let db = seeded();

    db.set_setting("crud.key", "first")
        .expect("the setting must be added");
    assert_eq!(db.get_setting("crud.key").as_deref(), Some("first"));
    assert_eq!(
        db.load_all_settings()
            .expect("the settings must be read")
            .get("crud.key")
            .map(String::as_str),
        Some("first")
    );

    db.set_setting("crud.key", "second")
        .expect("the setting must be updated");
    assert_eq!(
        db.get_setting("crud.key").as_deref(),
        Some("second"),
        "the single-key read sees the update"
    );
    assert_eq!(
        db.load_all_settings()
            .expect("the settings must be read")
            .get("crud.key")
            .map(String::as_str),
        Some("second"),
        "the batch read sees the same update"
    );

    db.delete_setting("crud.key")
        .expect("the setting must be deleted");
    assert!(db.get_setting("crud.key").is_none());
    assert!(
        !db.load_all_settings()
            .expect("the settings must be read")
            .contains_key("crud.key"),
        "the deleted key is gone from the batch read"
    );
    db.delete_setting("crud.key")
        .expect("deleting an absent setting is a no-op");
}
