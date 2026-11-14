//! Equivalence tests for the filtered grid construction.
//!
//! The grid used to look each song's album up with three chained linear scans.
//! It now builds three first-wins maps over the same album slice. These tests
//! keep the old algorithm written out as the reference and require the same
//! album ids in the same order, including the fallbacks that make the chain
//! load-bearing.

use crate::db::database::SongData;
use crate::gui::library::{AlbumEntry, LibraryManager};
use std::collections::HashSet;
use std::sync::Arc;

/// One album row as the snapshot carries it.
fn album(id: &str, title: &str, artist: &str) -> AlbumEntry {
    AlbumEntry {
        id: id.into(),
        title: title.into(),
        artist: artist.into(),
        genre: String::new(),
        year: String::new(),
        cover_path: None,
    }
}

/// One song row; the title, artist and hash are the only fields the grid reads.
fn song(album_hash: Option<&str>, album_title: Option<&str>, artist: &str) -> Arc<SongData> {
    Arc::new(SongData {
        album_hash: album_hash.map(Arc::from),
        album: album_title.map(Arc::from),
        artist: Some(Arc::from(artist)),
        title: Some(Arc::from("Tema")),
        ..Default::default()
    })
}

/// The album lookup the grid performed before the maps existed: the first match
/// by hash, then by title plus artist, then by title alone.
fn linear_lookup<'a>(
    albums: &'a [AlbumEntry],
    album_hash: Option<&str>,
    album_title: &str,
    artist: &str,
) -> Option<&'a AlbumEntry> {
    album_hash
        .and_then(|hash| albums.iter().find(|a| a.id == hash))
        .or_else(|| {
            albums
                .iter()
                .find(|a| a.title.as_str() == album_title && a.artist == artist)
        })
        .or_else(|| albums.iter().find(|a| a.title.as_str() == album_title))
}

/// A manager whose album list holds two versions of the same title plus a
/// distinct one, and whose songs exercise every branch of the lookup chain.
fn fixture() -> (LibraryManager, Vec<Arc<SongData>>) {
    let albums = vec![
        album("hash-a", "Greatest Hits", "Artista"),
        album("hash-b", "Greatest Hits", "Artista"),
        album("hash-c", "Solo Uno", "Otro"),
    ];

    let songs = vec![
        // The hash matches the first version.
        song(Some("hash-a"), Some("Greatest Hits"), "Artista"),
        // The hash matches the second version, which only the hash can tell apart.
        song(Some("hash-b"), Some("Greatest Hits"), "Artista"),
        // A hash that matches nothing must fall back to title plus artist.
        song(Some("hash-zzz"), Some("Greatest Hits"), "Artista"),
        // No hash at all, and a unique title.
        song(None, Some("Solo Uno"), "Otro"),
        // No album title: the grid skips the song entirely.
        song(Some("hash-a"), None, "Artista"),
    ];

    let manager = LibraryManager {
        cached_albums: Some(albums),
        // Any active filter selects the filtered-grid branch.
        search_query: "greatest".into(),
        // The canonical album order is the one both sides must agree on.
        sort_column: None,
        ..LibraryManager::default()
    };

    (manager, songs)
}

/// The ids the filtered grid currently holds.
fn grid_ids(manager: &LibraryManager) -> Vec<String> {
    manager
        .filtered_albums
        .as_ref()
        .expect("the filtered branch always stores the grid")
        .iter()
        .map(|a| a.id.clone())
        .collect()
}

#[test]
fn test_filtered_grid_matches_the_linear_scan() {
    let (mut manager, songs) = fixture();
    manager.update_processed_data(songs);

    let albums = manager
        .cached_albums
        .as_ref()
        .expect("the fixture owns the album list");

    let mut expected = Vec::new();
    let mut seen = HashSet::new();
    for group in &manager.artist_groups {
        for song in &group.songs {
            let Some(album_title) = &song.album else {
                continue;
            };

            let identity = song
                .album_hash
                .as_deref()
                .map(str::to_owned)
                .unwrap_or_else(|| album_title.trim().to_lowercase());
            if !seen.insert((group.name.to_lowercase(), identity)) {
                continue;
            }

            if let Some(entry) =
                linear_lookup(albums, song.album_hash.as_deref(), album_title, &group.name)
            {
                let mut unified = entry.clone();
                unified.artist = group.name.clone();
                expected.push(unified);
            }
        }
    }
    LibraryManager::sort_albums_static(&mut expected, None, Some(true));

    let expected_ids: Vec<String> = expected.iter().map(|a| a.id.clone()).collect();
    assert_eq!(
        grid_ids(&manager),
        expected_ids,
        "the maps must reproduce the linear chain, fallbacks included"
    );
    assert_eq!(
        expected_ids,
        vec!["hash-a", "hash-b", "hash-a", "hash-c"],
        "the fixture keeps every branch distinct"
    );
}

#[test]
fn test_filtered_grid_order_is_stable() {
    let (mut manager, songs) = fixture();

    manager.update_processed_data(songs.clone());
    let first = grid_ids(&manager);

    manager.update_processed_data(songs);
    let second = grid_ids(&manager);

    assert_eq!(first, second, "the same input keeps the same order");
}
