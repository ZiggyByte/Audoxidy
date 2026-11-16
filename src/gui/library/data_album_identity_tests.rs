//! Album identity across versions and folders.
//!
//! A song belongs to an album by hash when its row carries one, and by title
//! plus artist otherwise. The tests build plain values, so no widget tree and no
//! database take part; they pin that two versions sharing a title and an artist
//! stay separate blocks and that the fallback still finds the right one.

use super::*;
use crate::db::database::SongData;
use std::sync::Arc;

fn album(id: &str) -> AlbumEntry {
    AlbumEntry {
        id: id.to_string(),
        title: "Hits".to_string(),
        artist: "The Artist".to_string(),
        genre: "Rock".to_string(),
        year: "1999".to_string(),
        cover_path: None,
    }
}

fn song(album_hash: Option<&str>) -> Arc<SongData> {
    Arc::new(SongData {
        album_hash: album_hash.map(Arc::from),
        album: Some(Arc::from("Hits")),
        artist: Some(Arc::from("The Artist")),
        album_artist: Some(Arc::from("The Artist")),
        title: Some(Arc::from("A Song")),
        ..SongData::default()
    })
}

#[test]
fn test_two_versions_with_the_same_title_and_artist_keep_their_songs_separate() {
    let first = album("hash-a");
    let second = album("hash-b");
    let from_first = song(Some("hash-a"));
    let from_second = song(Some("hash-b"));

    assert!(song_belongs_to_album(&from_first, &first));
    assert!(!song_belongs_to_album(&from_first, &second));
    assert!(song_belongs_to_album(&from_second, &second));
    assert!(!song_belongs_to_album(&from_second, &first));
}

#[test]
fn test_a_song_without_an_album_hash_falls_back_to_title_and_artist() {
    let first = album("hash-a");
    let mut other = first.clone();
    other.title = "Another Album".to_string();

    assert!(song_belongs_to_album(&song(None), &first));
    assert!(!song_belongs_to_album(&song(None), &other));
}
