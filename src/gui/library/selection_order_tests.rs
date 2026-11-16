//! Selection order for the album selection.
//!
//! The tests drive a manager with synthetic albums and songs and read the
//! resolved selection back, so no widget tree is built and no row is rendered.
//! Adding several album versions keeps each version's songs contiguous, in grid
//! order and then in track order.

use super::*;
use crate::db::database::SongData;
use crate::gui::library::types::AlbumEntry;

fn album(id: &str) -> AlbumEntry {
    AlbumEntry {
        id: id.to_string(),
        title: "Hits".to_string(),
        artist: "Artist".to_string(),
        genre: "Rock".to_string(),
        year: "1999".to_string(),
        cover_path: None,
    }
}

fn song(id: i64, album_hash: &str, track: &str) -> Arc<SongData> {
    Arc::new(SongData {
        id,
        album_hash: Some(Arc::from(album_hash)),
        album: Some(Arc::from("Hits")),
        artist: Some(Arc::from("Artist")),
        album_artist: Some(Arc::from("Artist")),
        track_number: Some(Arc::from(track)),
        title: Some(Arc::from(format!("Track {track}"))),
        ..SongData::default()
    })
}

#[test]
fn test_album_versions_are_kept_together_when_adding_to_a_playlist() {
    let mut manager = LibraryManager::default();
    manager.cached_albums = Some(vec![album("hash-b"), album("hash-a")]);
    manager.cached_all_songs = Some(vec![
        song(1, "hash-a", "1"),
        song(2, "hash-b", "1"),
        song(3, "hash-a", "2"),
        song(4, "hash-b", "2"),
    ]);
    manager
        .selected_items
        .insert(LibraryListItem::Album("Artist|hash-b".to_string()));
    manager
        .selected_items
        .insert(LibraryListItem::Album("Artist|hash-a".to_string()));

    let selected = manager.get_selected_songs();
    let ids: Vec<i64> = selected.iter().map(|song| song.id).collect();

    assert_eq!(
        ids,
        vec![2, 4, 1, 3],
        "cada versión va contigua, en orden de rejilla y de pista"
    );
}
