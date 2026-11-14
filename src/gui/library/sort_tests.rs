//! Sortability contract for the library columns.
//!
//! The technical columns are still rendered by `format_metadata` in the list
//! views and in the expanded album song list, but they are display-only: no
//! click may reorder the library by them. These tests are pure comparison
//! contracts — no window, no renderer and no database are involved.

use crate::db::database::SongData;
use crate::gui::library::LibraryManager;
use crate::utils::SortColumn;
use std::sync::Arc;

/// Columns that only render a value: the four technical fields plus the two
/// layout pseudo-columns.
const DISPLAY_ONLY: [SortColumn; 6] = [
    SortColumn::Format,
    SortColumn::SampleRate,
    SortColumn::BitDepth,
    SortColumn::Channels,
    SortColumn::AlbumCard,
    SortColumn::AlbumThumbnail,
];

/// Columns whose ordering the library still offers.
const SORTABLE: [SortColumn; 10] = [
    SortColumn::TrackNumber,
    SortColumn::Title,
    SortColumn::Artist,
    SortColumn::AlbumArtist,
    SortColumn::Album,
    SortColumn::Genre,
    SortColumn::Year,
    SortColumn::Duration,
    SortColumn::Bitrate,
    SortColumn::Size,
];

#[test]
fn test_display_only_columns_are_not_sortable() {
    for column in DISPLAY_ONLY {
        assert!(!column.is_sortable(), "{column:?} must stay display-only");
    }
}

#[test]
fn test_remaining_columns_are_sortable() {
    for column in SORTABLE {
        assert!(column.is_sortable(), "{column:?} must stay sortable");
    }
}

/// A stable sort over an all-equal comparator keeps the input order, which is
/// the observable consequence of a display-only column having no ordering.
#[test]
fn test_sorting_by_a_display_only_column_keeps_the_order() {
    let song = |id: i64, channels: i64| {
        Arc::new(SongData {
            id,
            channels: Some(channels),
            ..SongData::default()
        })
    };
    // Deliberately not in channel order: a real comparison would reorder it.
    let mut songs = vec![song(7, 6), song(3, 2), song(9, 8)];
    let before: Vec<i64> = songs.iter().map(|s| s.id).collect();

    LibraryManager::sort_songs_static(&mut songs, Some(SortColumn::Channels), Some(true));

    let after: Vec<i64> = songs.iter().map(|s| s.id).collect();
    assert_eq!(
        before, after,
        "a display-only column must not reorder the list"
    );
}
