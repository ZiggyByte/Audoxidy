//! Formatting, search and sort contracts of the `utils` module root.
//!
//! Every test here asserts on values alone: the formatting helpers turn a number
//! into its display string, the search predicate matches one field of a song row,
//! and the comparison functions define the canonical listing order. No window,
//! no database and no filesystem is involved, so each contract is pinned from a
//! value the test builds.

use super::*;
use std::sync::Arc;

fn make_test_song() -> SongData {
    SongData {
        id: 1,
        full_file_path: Arc::from("/music/test.flac"),
        title: Some(Arc::from("Test Title")),
        artist: Some(Arc::from("Test Artist")),
        album: Some(Arc::from("Test Album")),
        album_artist: Some(Arc::from("Test Album Artist")),
        genre: Some(Arc::from("Rock")),
        release_year: Some(Arc::from("2024")),
        track_number: Some(Arc::from("1")),
        duration_secs: Some(180.0),
        format: Some(Arc::from("FLAC")),
        bit_depth: Some(24),
        sample_rate: Some(96000),
        size: Some(50_000_000),
        channels: Some(2),
        ..Default::default()
    }
}

fn make_test_song2() -> SongData {
    SongData {
        id: 2,
        full_file_path: Arc::from("/music/test2.mp3"),
        title: Some(Arc::from("Another Title")),
        artist: Some(Arc::from("Another Artist")),
        album: Some(Arc::from("Another Album")),
        album_artist: Some(Arc::from("Another Album Artist")),
        release_year: Some(Arc::from("2023")),
        track_number: Some(Arc::from("2")),
        duration_secs: Some(240.0),
        ..Default::default()
    }
}

// ── format_duration ──

#[test]
fn test_format_duration_zero() {
    assert_eq!(format_duration(0.0), "00:00");
}

#[test]
fn test_format_duration_one_min_five_sec() {
    assert_eq!(format_duration(65.0), "01:05");
}

#[test]
fn test_format_duration_one_hour_one_min_one_sec() {
    assert_eq!(format_duration(3661.0), "01:01:01");
}

#[test]
fn test_format_duration_one_day_one_hour_one_min_one_sec() {
    assert_eq!(format_duration(90061.0), "01:01:01:01");
}

#[test]
fn test_format_duration_negative_clamps_to_zero() {
    assert_eq!(format_duration(-5.0), "00:00");
}

#[test]
fn test_format_duration_one_hour_one_min() {
    assert_eq!(format_duration(3660.0), "01:01:00");
}

// ── format_size ──

#[test]
fn test_format_size_zero() {
    assert_eq!(format_size(0), "0.00 MB");
}

#[test]
fn test_format_size_negative() {
    assert_eq!(format_size(-1), "0.00 MB");
}

#[test]
fn test_format_size_one_mb() {
    assert_eq!(format_size(1_048_576), "1.00 MB");
}

#[test]
fn test_format_size_one_gb() {
    assert_eq!(format_size(1_073_741_824), "1.00 GB");
}

#[test]
fn test_format_size_512_mb() {
    assert_eq!(format_size(536_870_912), "512.00 MB");
}

// ── truncate_text ──

#[test]
fn test_truncate_text_within_limit() {
    assert_eq!(truncate_text("hello", 10), "hello");
}

#[test]
fn test_truncate_text_exceeds_limit() {
    assert_eq!(truncate_text("hello world", 8), "hello...");
}

#[test]
fn test_truncate_text_unicode_below_limit() {
    assert_eq!(truncate_text("café", 10), "café");
}

#[test]
fn test_truncate_text_unicode_exceeds_limit() {
    assert_eq!(truncate_text("café", 3), "...");
}

#[test]
fn test_truncate_text_empty() {
    assert_eq!(truncate_text("", 5), "");
}

#[test]
fn test_truncate_text_at_exact_limit() {
    assert_eq!(truncate_text("abc", 3), "abc");
}

// ── compare_track_numbers ──

#[test]
fn test_compare_track_numbers_both_none() {
    assert_eq!(compare_track_numbers(None, None), std::cmp::Ordering::Equal);
}

#[test]
fn test_compare_track_numbers_numeric_less() {
    assert_eq!(
        compare_track_numbers(Some("1"), Some("2")),
        std::cmp::Ordering::Less
    );
}

#[test]
fn test_compare_track_numbers_numeric_greater() {
    assert_eq!(
        compare_track_numbers(Some("10"), Some("2")),
        std::cmp::Ordering::Greater
    );
}

#[test]
fn test_compare_track_numbers_alphanumeric_fallback() {
    assert_eq!(
        compare_track_numbers(Some("A1"), Some("A2")),
        std::cmp::Ordering::Less
    );
}

#[test]
fn test_compare_track_numbers_empty_vs_none() {
    // Both unwrap to "", a_str == b_str returns Equal (code behaviour)
    assert_eq!(
        compare_track_numbers(Some(""), None),
        std::cmp::Ordering::Equal
    );
}

#[test]
fn test_compare_track_numbers_fractional() {
    assert_eq!(
        compare_track_numbers(Some("1/10"), Some("2/10")),
        std::cmp::Ordering::Less
    );
}

#[test]
fn test_compare_track_numbers_some_vs_none() {
    // Some("1") vs None: None unwraps to "", b_str is empty -> Less (tracks with numbers sort before empties)
    assert_eq!(
        compare_track_numbers(Some("1"), None),
        std::cmp::Ordering::Less
    );
}

// ── song_matches_search ──

#[test]
fn test_song_matches_search_on_title() {
    let song = make_test_song();
    assert!(song_matches_search(&song, "test title"));
}

#[test]
fn test_song_matches_search_on_artist() {
    let song = make_test_song();
    assert!(song_matches_search(&song, "test artist"));
}

#[test]
fn test_song_matches_search_no_match() {
    let song = make_test_song();
    assert!(!song_matches_search(&song, "zzzznotfound"));
}

#[test]
fn test_song_matches_search_case_insensitive() {
    // query_lowercase must already be lowercased by the caller;
    // song fields (which may be mixed-case) get lowercased inside the function
    let song = make_test_song();
    assert!(song_matches_search(&song, "test title"));
    assert!(song_matches_search(&song, "test artist"));
    assert!(song_matches_search(&song, "test album"));
    assert!(song_matches_search(&song, "test album artist"));
}

#[test]
fn test_song_matches_search_on_album() {
    let song = make_test_song();
    assert!(song_matches_search(&song, "test album"));
}

#[test]
fn test_song_matches_search_on_album_artist() {
    let song = make_test_song();
    assert!(song_matches_search(&song, "test album artist"));
}

// ── compare_strings_ignore_case ──

#[test]
fn test_compare_strings_ignore_case_less() {
    assert_eq!(
        compare_strings_ignore_case("a", "b"),
        std::cmp::Ordering::Less
    );
}

#[test]
fn test_compare_strings_ignore_case_greater() {
    assert_eq!(
        compare_strings_ignore_case("B", "a"),
        std::cmp::Ordering::Greater
    );
}

#[test]
fn test_compare_strings_ignore_case_equal() {
    assert_eq!(
        compare_strings_ignore_case("foo", "FOO"),
        std::cmp::Ordering::Equal
    );
}

// ── SortColumn::as_str ──

#[test]
fn test_sort_column_as_str_non_empty() {
    use SortColumn::*;
    let columns = [
        TrackNumber,
        Title,
        Artist,
        AlbumArtist,
        Album,
        Genre,
        Year,
        Duration,
        Format,
        SampleRate,
        Channels,
        BitDepth,
        Bitrate,
        Size,
        AlbumCard,
    ];
    for col in &columns {
        assert!(
            !col.as_str().is_empty(),
            "SortColumn::{:?} as_str() should not be empty",
            col
        );
    }
}

#[test]
fn test_sort_column_album_thumbnail_as_str_is_empty() {
    assert_eq!(SortColumn::AlbumThumbnail.as_str(), "");
}

// ── compare_songs_for_listing ──

#[test]
fn test_compare_songs_for_listing_same() {
    let a = make_test_song();
    let b = make_test_song();
    assert_eq!(compare_songs_for_listing(&a, &b), std::cmp::Ordering::Equal);
}

#[test]
fn test_compare_songs_for_listing_different_artist() {
    let a = make_test_song();
    let b = make_test_song2();
    // "Test Album Artist" vs "Another Album Artist" -> "test album artist" vs "another album artist"
    // "test album artist" > "another album artist" lexicographically (t > a)
    assert_eq!(
        compare_songs_for_listing(&a, &b),
        std::cmp::Ordering::Greater
    );
}

#[test]
fn test_compare_songs_for_listing_by_track_number() {
    let mut a = make_test_song();
    let mut b = make_test_song();
    // Same artist/album/year but different track numbers
    a.album_artist = Some(Arc::from("Same Artist"));
    b.album_artist = Some(Arc::from("Same Artist"));
    a.album = Some(Arc::from("Same Album"));
    b.album = Some(Arc::from("Same Album"));
    a.release_year = Some(Arc::from("2024"));
    b.release_year = Some(Arc::from("2024"));
    a.track_number = Some(Arc::from("1"));
    b.track_number = Some(Arc::from("5"));
    assert_eq!(compare_songs_for_listing(&a, &b), std::cmp::Ordering::Less);
}
