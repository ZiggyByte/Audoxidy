//! Metric-level tests for the album grid geometry.
//!
//! Every test here is pure arithmetic: no window, no renderer and no database are
//! involved, so the column rule, the card anatomy, the row heights and the
//! scrollbar rail exclusion can be pinned independently of the widget tree.

use crate::db::database::SongData;
use crate::gui::dimensions::{
    ALBUM_CARD_LINE_GAP, ALBUM_CARD_LINE_HEIGHT, ALBUM_CARD_MAX_WIDTH, ALBUM_CARD_MIN_WIDTH,
    ALBUM_CARD_REFERENCE_WIDTH, ALBUM_CARD_TEXT_BLOCK_HEIGHT, ALBUM_COVER_MAX, ALBUM_COVER_MIN,
    ALBUM_COVER_REFERENCE, ALBUM_EXPANDED_BOTTOM_INSET, ALBUM_EXPANDED_PADDING,
    ALBUM_SCROLLBAR_RAIL, LIST_ROW_HEADER,
};
use crate::gui::library::grid_metrics::*;
use crate::gui::library::{
    AlbumEntry, LibraryListItem, LibraryManager, LibraryNavDir, LibraryViewMode,
};
use std::sync::Arc;

/// Tolerance applied to every derived height and to every width sum.
const EPSILON: f32 = 0.001;

/// A manager holding `album_count` cached albums and the given snapshot.
fn manager_with_albums(album_count: usize, metrics: GridMetrics) -> LibraryManager {
    let mut manager = LibraryManager::default();
    manager.cached_albums = Some(
        (0..album_count)
            .map(|index| AlbumEntry {
                id: format!("id-{index}"),
                title: format!("Album {index}"),
                artist: "Artist".to_string(),
                genre: "Genre".to_string(),
                year: "2026".to_string(),
                cover_path: None,
            })
            .collect(),
    );
    manager.grid_metrics.set(metrics);
    manager
}

/// Composite identifier the grid uses for one album.
fn composite_id(album: &AlbumEntry) -> String {
    format!("{}|{}", album.artist, album.id)
}

/// A manager whose album at `expanded_index` is expanded over `song_count` songs.
///
/// Every song carries a distinct file path, which is the key the reveal actions
/// resolve, so a target can be followed from the path back to its rectangle.
fn manager_with_expansion(
    album_count: usize,
    expanded_index: usize,
    song_count: usize,
    metrics: GridMetrics,
) -> LibraryManager {
    let mut manager = manager_with_albums(album_count, metrics);
    let album = manager
        .cached_albums
        .as_ref()
        .expect("the fixture owns its albums")[expanded_index]
        .clone();

    let songs = (0..song_count)
        .map(|index| {
            Arc::new(SongData {
                id: index as i64,
                full_file_path: Arc::from(format!("/music/{index}.flac").into_boxed_str()),
                title: Some(Arc::from(format!("Song {index}").into_boxed_str())),
                album: Some(Arc::from(album.title.clone().into_boxed_str())),
                artist: Some(Arc::from(album.artist.clone().into_boxed_str())),
                ..SongData::default()
            })
        })
        .collect();

    manager.expanded_album = Some(composite_id(&album));
    manager.expanded_album_songs = Some(songs);
    manager
}

/// Rectangles of the rendered song rows, in renderer order.
fn song_rectangles(manager: &LibraryManager) -> Vec<iced::Rectangle> {
    manager
        .get_visible_items()
        .into_iter()
        .filter_map(|(item, rectangle)| {
            matches!(item, LibraryListItem::Song(_)).then_some(rectangle)
        })
        .collect()
}

#[test]
fn test_reference_layout_matches_the_locked_card_geometry() {
    let metrics = grid_metrics(2.0 * ALBUM_CARD_REFERENCE_WIDTH + ALBUM_SCROLLBAR_RAIL);

    assert_eq!(metrics.columns, 2);
    assert_eq!(metrics.card_width, 189.0);
    assert_eq!(metrics.cover_size, 159.0);
    assert_eq!(metrics.text_block_height, 62.0);
    assert_eq!(metrics.card_height, 253.0);
    assert_eq!(metrics.row_height, 253.0);
    assert_eq!(metrics.gap, 0.0);
}

#[test]
fn test_row_fills_the_usable_width_without_remainder() {
    let reported_widths = [
        309.0_f32, 433.0, 769.0, 1009.0, 1329.0, 1969.0, 3249.0, 8008.0,
    ];

    for reported_width in reported_widths {
        let metrics = grid_metrics(reported_width);
        let usable_width = reported_width - ALBUM_SCROLLBAR_RAIL;

        assert!(
            (metrics.columns as f32 * metrics.card_width - usable_width).abs() < EPSILON,
            "a full row must cover the usable width at {reported_width}"
        );
        assert_eq!(metrics.gap, 0.0);
    }
}

#[test]
fn test_bounds_hold_across_the_supported_width_range() {
    let mut reported_width = 2.0 * ALBUM_CARD_MIN_WIDTH + ALBUM_SCROLLBAR_RAIL;

    while reported_width <= 8008.0 {
        let metrics = grid_metrics(reported_width);
        let usable_width = reported_width - ALBUM_SCROLLBAR_RAIL;

        assert!(
            metrics.columns >= 2,
            "the grid never drops below two columns at {reported_width}"
        );
        assert!(
            metrics.card_width >= ALBUM_CARD_MIN_WIDTH - EPSILON
                && metrics.card_width <= ALBUM_CARD_MAX_WIDTH + EPSILON,
            "card width left the bounds at {reported_width}: {}",
            metrics.card_width
        );
        assert!(
            metrics.cover_size >= ALBUM_COVER_MIN - EPSILON
                && metrics.cover_size <= ALBUM_COVER_MAX + EPSILON,
            "cover left the bounds at {reported_width}: {}",
            metrics.cover_size
        );
        assert!(
            (metrics.columns as f32 * metrics.card_width - usable_width).abs() < EPSILON,
            "a full row must cover the usable width at {reported_width}"
        );
        assert!(metrics.card_height.is_finite());

        reported_width += 0.5;
    }
}

#[test]
fn test_column_count_at_the_reference_widths() {
    let cases = [
        (1329.0_f32, 7_usize),
        (1969.0, 10),
        (3249.0, 17),
        (769.0, 4),
        (1009.0, 5),
        (433.0, 2),
        (309.0, 2),
    ];

    for (reported_width, expected_columns) in cases {
        assert_eq!(
            grid_metrics(reported_width).columns,
            expected_columns,
            "unexpected column count at {reported_width}"
        );
    }
}

#[test]
fn test_rounding_is_half_away_from_zero() {
    let three = grid_metrics(480.5);
    assert_eq!(three.columns, 3);
    assert!(
        (three.columns as f32 * three.card_width - (480.5 - ALBUM_SCROLLBAR_RAIL)).abs() < EPSILON
    );

    let two = grid_metrics(291.5);
    assert_eq!(two.columns, 2);
    assert!((two.columns as f32 * two.card_width - (291.5 - ALBUM_SCROLLBAR_RAIL)).abs() < EPSILON);
}

#[test]
fn test_column_bound_guards_are_idempotent() {
    // Rounding to the reference width already lands the card inside the bounds,
    // so neither size guard can fire. The guards stay because the column rule
    // states them; this asserts they leave the rounded count untouched.
    let reported_widths = [
        2.0 * ALBUM_CARD_MIN_WIDTH + ALBUM_SCROLLBAR_RAIL,
        472.5,
        1009.0,
        1969.0,
        3249.0,
        8008.0,
    ];

    for reported_width in reported_widths {
        let usable_width = reported_width - ALBUM_SCROLLBAR_RAIL;
        let rounded = ((usable_width / ALBUM_CARD_REFERENCE_WIDTH).round() as usize).max(2);
        let card_width = usable_width / rounded as f32;

        assert!(card_width <= ALBUM_CARD_MAX_WIDTH);
        assert!(card_width >= ALBUM_CARD_MIN_WIDTH);
        assert_eq!(grid_metrics(reported_width).columns, rounded);
    }
}

#[test]
fn test_minimum_layout_guards_non_positive_and_non_finite_width() {
    let widths = [
        0.0_f32,
        -1.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        ALBUM_SCROLLBAR_RAIL,
        ALBUM_SCROLLBAR_RAIL / 2.0,
    ];

    for reported_width in widths {
        let metrics = grid_metrics(reported_width);

        assert_eq!(metrics.columns, 2, "width {reported_width}");
        assert_eq!(metrics.card_width, ALBUM_CARD_MIN_WIDTH);
        assert_eq!(metrics.cover_size, ALBUM_COVER_MIN);
        assert_eq!(metrics.text_block_height, 62.0);
        assert!(metrics.card_height.is_finite());
        assert!((metrics.card_height - 205.3).abs() < EPSILON);
    }
}

#[test]
fn test_text_block_height_is_the_derived_constant() {
    assert_eq!(ALBUM_CARD_TEXT_BLOCK_HEIGHT, 62.0);
    assert_eq!(
        ALBUM_CARD_LINE_HEIGHT * 4.0 + ALBUM_CARD_LINE_GAP * 3.0,
        62.0
    );

    for reported_width in [
        2.0 * ALBUM_CARD_MIN_WIDTH + ALBUM_SCROLLBAR_RAIL,
        2.0 * ALBUM_CARD_REFERENCE_WIDTH + ALBUM_SCROLLBAR_RAIL,
        1329.0,
        3249.0,
    ] {
        assert_eq!(grid_metrics(reported_width).text_block_height, 62.0);
    }
}

#[test]
fn test_card_height_derives_from_the_cover() {
    let reported_widths = [
        2.0 * ALBUM_CARD_MIN_WIDTH + ALBUM_SCROLLBAR_RAIL,
        2.0 * ALBUM_CARD_REFERENCE_WIDTH + ALBUM_SCROLLBAR_RAIL,
        1329.0,
        1969.0,
        3249.0,
        8008.0,
    ];

    for reported_width in reported_widths {
        let metrics = grid_metrics(reported_width);

        assert!((metrics.card_height - (metrics.cover_size + 94.0)).abs() < EPSILON);
        assert!((metrics.card_height - (metrics.card_width + 64.0)).abs() < EPSILON);
        assert!((metrics.row_height - metrics.card_height).abs() < EPSILON);
        assert!((metrics.cover_size - (metrics.card_width - 30.0)).abs() < EPSILON);
    }
}

#[test]
fn test_expanded_row_height_matches_the_rendered_chrome() {
    let layouts = [
        GridMetrics::from_card_width(ALBUM_CARD_MIN_WIDTH),
        GridMetrics::from_card_width(ALBUM_CARD_REFERENCE_WIDTH),
        GridMetrics::from_card_width(ALBUM_CARD_MAX_WIDTH),
    ];

    for metrics in layouts {
        for rendered_songs in [0_usize, 1, 30] {
            let expected = metrics.card_height + 69.0 + rendered_songs as f32 * LIST_ROW_HEADER;

            assert!(
                (expanded_row_height(&metrics, rendered_songs) - expected).abs() < EPSILON,
                "chrome mismatch for {} songs at card height {}",
                rendered_songs,
                metrics.card_height
            );
        }
    }
}

#[test]
fn test_expanded_row_has_no_phantom_song() {
    let layouts = [
        GridMetrics::from_card_width(ALBUM_CARD_MIN_WIDTH),
        GridMetrics::from_card_width(ALBUM_CARD_REFERENCE_WIDTH),
        GridMetrics::from_card_width(ALBUM_CARD_MAX_WIDTH),
    ];

    for metrics in layouts {
        let height = expanded_row_height(&metrics, 0);

        assert!((height - (metrics.card_height + 69.0)).abs() < EPSILON);
        assert!((height - (metrics.card_height + 101.0)).abs() > EPSILON);
    }
}

#[test]
fn test_index_to_row_and_top_mapping() {
    let album_count = 53_usize;
    let cases = [
        (
            2.0 * ALBUM_CARD_REFERENCE_WIDTH + ALBUM_SCROLLBAR_RAIL,
            2_usize,
        ),
        (1329.0, 7),
        (1969.0, 10),
        (3249.0, 17),
    ];

    for (reported_width, expected_columns) in cases {
        let metrics = grid_metrics(reported_width);
        assert_eq!(metrics.columns, expected_columns);

        for album_index in [0_usize, 1, expected_columns - 1, expected_columns, 52] {
            let row_index = album_index / metrics.columns;
            let top = row_top_of(&metrics, album_count, None, 0, row_index);

            assert!((top - row_index as f32 * metrics.row_height).abs() < EPSILON);
        }

        let rows = row_count(&metrics, album_count);
        assert!(
            (total_content_height(&metrics, album_count, None, 0)
                - rows as f32 * metrics.row_height)
                .abs()
                < EPSILON
        );

        // An expanded row replaces its flat height with the chrome plus one row
        // per rendered song, and every row below shifts by exactly that delta.
        let expanded_row = 1_usize;
        let rendered_songs = 12_usize;
        let expanded_height = metrics.card_height + 69.0 + rendered_songs as f32 * LIST_ROW_HEADER;
        let spans = grid_rows(&metrics, album_count, Some(expanded_row), rendered_songs);

        assert_eq!(spans.len(), rows);
        assert!(spans[expanded_row].expanded);
        assert!((spans[expanded_row].height - expanded_height).abs() < EPSILON);
        assert!(
            (spans[expanded_row + 1].top - (spans[expanded_row].top + expanded_height)).abs()
                < EPSILON
        );
    }
}

#[test]
fn test_visible_items_tile_the_usable_width() {
    for reported_width in [
        2.0 * ALBUM_CARD_REFERENCE_WIDTH + ALBUM_SCROLLBAR_RAIL,
        1329.0,
        1969.0,
        3249.0,
    ] {
        let metrics = grid_metrics(reported_width);
        let usable_width = reported_width - ALBUM_SCROLLBAR_RAIL;

        // The first card starts at the content origin and every card starts
        // where the previous one ends, so a full row leaves no gap and no
        // overlap.
        let mut previous_right = 0.0_f32;
        for column in 0..metrics.columns {
            let left = column as f32 * metrics.card_width;
            let right = left + metrics.card_width;

            assert!((left - previous_right).abs() < EPSILON);
            assert!((right - (column + 1) as f32 * metrics.card_width).abs() < EPSILON);
            previous_right = right;
        }

        assert!((previous_right - usable_width).abs() < EPSILON);
    }

    // The hit rectangles project from the same snapshot: the first card starts
    // at the content origin, every card is the full card box, and rows step by
    // exactly one row height.
    for columns in [2_usize, 7, 10, 17] {
        let mut metrics = GridMetrics::from_card_width(ALBUM_CARD_REFERENCE_WIDTH);
        metrics.columns = columns;

        let album_count = columns * 2 + 3;
        let manager = manager_with_albums(album_count, metrics);
        let rectangles: Vec<iced::Rectangle> = manager
            .get_visible_items()
            .into_iter()
            .filter_map(|(item, rectangle)| {
                matches!(item, LibraryListItem::Album(_)).then_some(rectangle)
            })
            .collect();

        assert_eq!(rectangles.len(), album_count);
        assert_eq!(rectangles[0].x, 0.0);

        for (index, rectangle) in rectangles.iter().enumerate() {
            let row = index / columns;
            let column = index % columns;

            assert!((rectangle.x - column as f32 * metrics.card_width).abs() < EPSILON);
            assert!((rectangle.y - row as f32 * metrics.row_height).abs() < EPSILON);
            assert!((rectangle.width - metrics.card_width).abs() < EPSILON);
            assert!((rectangle.height - metrics.card_height).abs() < EPSILON);
        }

        // The last card of a full row ends exactly at the usable width, so the
        // row leaves no gap and no overlap.
        let last_full_row_card = rectangles[columns - 1];
        let row_width = columns as f32 * metrics.card_width;
        assert!((last_full_row_card.x + last_full_row_card.width - row_width).abs() < EPSILON);
    }
}

#[test]
fn test_scrollbar_rail_is_excluded_from_the_row() {
    for reported_width in [1329.0_f32, 1969.0, 3249.0, 769.0] {
        let metrics = grid_metrics(reported_width);

        assert!(
            (metrics.columns as f32 * metrics.card_width + ALBUM_SCROLLBAR_RAIL - reported_width)
                .abs()
                < EPSILON
        );
        assert!(
            (metrics.columns as f32 * metrics.card_width - (reported_width - ALBUM_SCROLLBAR_RAIL))
                .abs()
                < EPSILON
        );
    }
}

#[test]
fn test_placeholder_icon_scales_with_the_cover() {
    assert_eq!(album_placeholder_icon_side(ALBUM_COVER_REFERENCE), 96.0);
    assert_eq!(album_placeholder_icon_side(ALBUM_COVER_MIN), 67.0);
    assert_eq!(album_placeholder_icon_side(ALBUM_COVER_MAX), 125.0);

    for reported_width in [
        2.0 * ALBUM_CARD_MIN_WIDTH + ALBUM_SCROLLBAR_RAIL,
        2.0 * ALBUM_CARD_REFERENCE_WIDTH + ALBUM_SCROLLBAR_RAIL,
        1329.0,
        1969.0,
        3249.0,
        8008.0,
    ] {
        let metrics = grid_metrics(reported_width);

        assert!(album_placeholder_icon_side(metrics.cover_size) < metrics.cover_size);
    }
}

#[test]
fn test_reveal_target_equals_virtualization_target() {
    // The reveal actions project their scroll target from the same row producer
    // the virtualizer uses, so a revealed song must land on its own rendered
    // rectangle at every supported column count.
    let cases = [
        (
            2.0 * ALBUM_CARD_REFERENCE_WIDTH + ALBUM_SCROLLBAR_RAIL,
            2_usize,
        ),
        (1329.0, 7),
        (1969.0, 10),
        (3249.0, 17),
    ];

    for (reported_width, expected_columns) in cases {
        let metrics = grid_metrics(reported_width);
        assert_eq!(metrics.columns, expected_columns, "width {reported_width}");

        let album_count = expected_columns * 3 + 2;
        let expanded_index = expected_columns * 2 + 1;
        let song_count = 9;
        let manager = manager_with_expansion(album_count, expanded_index, song_count, metrics);

        let expanded_row = expanded_index / metrics.columns;
        let rendered_songs = manager.rendered_expanded_song_count();
        assert_eq!(rendered_songs, song_count);

        let row_top = row_top_of(
            &metrics,
            album_count,
            Some(expanded_row),
            rendered_songs,
            expanded_row,
        );
        let rectangles = song_rectangles(&manager);
        assert_eq!(rectangles.len(), song_count);

        for (rendered_index, rectangle) in rectangles.iter().enumerate() {
            let reveal_y = song_y_in_row(&metrics, row_top, rendered_index);

            assert!(
                (reveal_y - rectangle.y).abs() < EPSILON,
                "reveal target {} differs from the rendered top {} at {expected_columns} columns",
                reveal_y,
                rectangle.y
            );
            assert!((rectangle.height - LIST_ROW_HEADER).abs() < EPSILON);
        }

        // A search filter changes the rendered indices; the reveal counts the same
        // subset the renderer keeps, so the surviving song stays on its rectangle.
        let mut filtered = manager_with_expansion(album_count, expanded_index, song_count, metrics);
        filtered.search_query = "song 1".to_string();

        let filtered_songs = filtered.rendered_expanded_song_count();
        assert_eq!(filtered_songs, 1);

        let filtered_top = row_top_of(
            &metrics,
            album_count,
            Some(expanded_row),
            filtered_songs,
            expanded_row,
        );
        let filtered_rectangles = song_rectangles(&filtered);
        assert_eq!(filtered_rectangles.len(), filtered_songs);
        assert!(
            (song_y_in_row(&metrics, filtered_top, 0) - filtered_rectangles[0].y).abs() < EPSILON
        );
    }
}

#[test]
fn test_keyboard_hint_matches_row_layout() {
    // Keyboard autoscroll recomputes the hint from the same expression, so the
    // focused row can never sit under an edge of the expanded block.
    let metrics = grid_metrics(1969.0);
    let album_count = metrics.columns * 2 + 3;
    let expanded_index = metrics.columns + 2;
    let song_count = 30;
    let manager = manager_with_expansion(album_count, expanded_index, song_count, metrics);

    let expanded_row = expanded_index / metrics.columns;
    let row_top = row_top_of(
        &metrics,
        album_count,
        Some(expanded_row),
        song_count,
        expanded_row,
    );
    let rectangles = song_rectangles(&manager);
    assert_eq!(rectangles.len(), song_count);

    let first = song_y_in_row(&metrics, row_top, 0);
    let last = song_y_in_row(&metrics, row_top, song_count - 1);
    assert!((first - rectangles[0].y).abs() < EPSILON);
    assert!((last - rectangles[song_count - 1].y).abs() < EPSILON);

    for (index, rectangle) in rectangles.iter().enumerate().skip(1) {
        let previous = song_y_in_row(&metrics, row_top, index - 1);
        let current = song_y_in_row(&metrics, row_top, index);

        assert!((current - previous - LIST_ROW_HEADER).abs() < EPSILON);
        assert!((current - rectangle.y).abs() < EPSILON);
    }
}

#[test]
fn test_expanded_album_anchor_is_identity_based() {
    // The anchor resolves the expanded album by its composite id, so a geometry
    // change re-finds the same album instead of trusting a pixel position.
    let metrics = grid_metrics(1329.0);
    let album_count = metrics.columns * 3;
    let expanded_index = metrics.columns + 2;
    let song_count = 9;

    let mut manager = manager_with_expansion(album_count, expanded_index, song_count, metrics);
    assert_eq!(
        manager.expanded_album_anchor(),
        Some((expanded_index, song_count))
    );

    // The count follows the same search predicate the renderer applies, so a
    // filtered-out song never occupies a row the anchor would reserve.
    manager.search_query = "song 1".to_string();
    assert_eq!(manager.expanded_album_anchor(), Some((expanded_index, 1)));

    // An album outside the rendered list has no anchor to correct.
    manager.filtered_albums = Some(vec![
        manager.cached_albums.as_ref().expect("fixture albums")[0].clone(),
    ]);
    assert_eq!(manager.expanded_album_anchor(), None);

    // Without an expansion there is nothing to anchor.
    let plain = manager_with_albums(album_count, metrics);
    assert_eq!(plain.expanded_album_anchor(), None);
}

#[test]
fn test_expanded_chrome_matches_the_rendered_padding() {
    // The renderer wraps the song rows in one expanded padding above and below
    // and adds the container's bottom inset; the row height must charge exactly
    // that chrome and one list row per rendered song, and nothing else.
    let chrome = 2.0 * ALBUM_EXPANDED_PADDING + ALBUM_EXPANDED_BOTTOM_INSET;
    assert_eq!(chrome, 69.0);

    let layouts = [
        GridMetrics::from_card_width(ALBUM_CARD_MIN_WIDTH),
        GridMetrics::from_card_width(ALBUM_CARD_REFERENCE_WIDTH),
        GridMetrics::from_card_width(ALBUM_CARD_MAX_WIDTH),
    ];

    for metrics in layouts {
        for rendered_songs in [0_usize, 1, 30] {
            let expanded_height = expanded_row_height(&metrics, rendered_songs);
            let expected = metrics.card_height + chrome + rendered_songs as f32 * LIST_ROW_HEADER;

            assert!(
                (expanded_height - expected).abs() < EPSILON,
                "row height {} differs from the rendered block {} for {} songs",
                expanded_height,
                expected,
                rendered_songs
            );
        }

        // Without a rendered song the block is exactly the chrome, so no phantom
        // row is reserved for a song that does not exist.
        assert!((expanded_row_height(&metrics, 0) - metrics.card_height - chrome).abs() < EPSILON);

        // The first song starts one expanded padding below the card's bottom edge
        // and each following song advances by one list row.
        let row_top = 100.0_f32;
        let first = song_y_in_row(&metrics, row_top, 0);
        assert!((first - (row_top + metrics.card_height) - ALBUM_EXPANDED_PADDING).abs() < EPSILON);

        for index in 1..30 {
            let previous = song_y_in_row(&metrics, row_top, index - 1);
            let current = song_y_in_row(&metrics, row_top, index);

            assert!((current - previous - LIST_ROW_HEADER).abs() < EPSILON);
        }
    }
}

#[test]
fn test_view_mode_setting_round_trip() {
    // Every variant has one stable stored form and comes back unchanged.
    let modes = [
        (LibraryViewMode::Grid, "grid"),
        (LibraryViewMode::DetailedList, "detailed_list"),
        (LibraryViewMode::ThumbnailList, "thumbnail_list"),
        (LibraryViewMode::SimpleList, "simple_list"),
    ];

    for (mode, expected) in modes {
        assert_eq!(mode.as_setting_value(), expected);
        assert_eq!(
            LibraryViewMode::from_setting_value(Some(mode.as_setting_value())),
            mode,
            "a stored mode must resolve to the variant that wrote it"
        );
    }
}

#[test]
fn test_view_mode_setting_falls_back_to_grid() {
    // Missing, empty, differently cased and unknown values all open the grid.
    let invalid_values: [Option<&str>; 5] =
        [None, Some(""), Some("GRID"), Some("bogus"), Some("grid ")];

    for value in invalid_values {
        assert_eq!(
            LibraryViewMode::from_setting_value(value),
            LibraryViewMode::Grid,
            "an unrecognised stored value must fall back to the grid"
        );
    }

    assert_eq!(
        LibraryViewMode::from_setting_value(Some("grid")),
        LibraryViewMode::Grid
    );
}

#[test]
fn test_anchor_viewport_offset_uses_the_visible_row_not_the_expanded_album() {
    let previous = grid_metrics(1329.0);
    let next = grid_metrics(769.0);
    let songs = 10;
    let old_top = row_top_of(&previous, 60, Some(0), songs, 5);
    let offset = old_top + 10.0;

    let target = anchor_viewport_offset(&previous, &next, offset, 60, Some(0), songs, 800.0);

    let new_top = row_top_of(&next, 60, Some(0), songs, 8);
    let expected = new_top + 10.0 * (next.row_height / previous.row_height);
    assert!((target - expected).abs() < EPSILON);
    assert!(
        target > 1000.0,
        "the anchor must follow the visible row, not album 0"
    );
}

#[test]
fn test_anchor_viewport_offset_keeps_a_visible_expanded_row() {
    let previous = grid_metrics(1329.0);
    let next = grid_metrics(769.0);
    let songs = 10;
    let offset = 20.0;

    let target = anchor_viewport_offset(&previous, &next, offset, 60, Some(0), songs, 800.0);

    let expected =
        20.0 * (expanded_row_height(&next, songs) / expanded_row_height(&previous, songs));
    assert!((target - expected).abs() < EPSILON);
}

#[test]
fn test_cover_stays_inside_the_reference_thirty_percent_band() {
    // The card anatomy fixes the cover at ±30 % of the 159 px reference. The band
    // is derived from that reference and not from the clamp constants, so a clamp
    // that drifted away from the locked ratio would still be caught here.
    let lower = ALBUM_COVER_REFERENCE * 0.7;
    let upper = ALBUM_COVER_REFERENCE * 1.3;
    let mut reported_width = 2.0 * ALBUM_CARD_MIN_WIDTH + ALBUM_SCROLLBAR_RAIL;

    while reported_width <= 8008.0 {
        let metrics = grid_metrics(reported_width);

        assert!(
            metrics.cover_size >= lower - EPSILON && metrics.cover_size <= upper + EPSILON,
            "cover left the ±30 % band at {reported_width}: {}",
            metrics.cover_size
        );
        assert!(
            metrics.cover_size < metrics.card_width,
            "the cover must stay inside the card at {reported_width}"
        );
        assert!(
            metrics.text_block_height == ALBUM_CARD_TEXT_BLOCK_HEIGHT,
            "the text block must not follow the width at {reported_width}"
        );

        reported_width += 2.0;
    }
}

#[test]
fn test_metrics_are_stateless_across_the_resize_history() {
    // The snapshot depends on the width alone: replaying the same widths in any
    // order, after any other width, returns the same values, and a guarded width
    // always resolves to the documented minimum layout.
    let widths = [1329.0_f32, 769.0, 3249.0, 1009.0, 1969.0];

    let ascending: Vec<GridMetrics> = widths.iter().copied().map(grid_metrics).collect();
    let descending: Vec<GridMetrics> = widths
        .iter()
        .rev()
        .copied()
        .map(grid_metrics)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    assert_eq!(ascending, descending);
    assert_eq!(ascending, widths.map(grid_metrics).to_vec());

    let minimum = GridMetrics::minimum();
    for guarded_width in [0.0_f32, -1.0, f32::NAN, ALBUM_SCROLLBAR_RAIL] {
        assert_eq!(grid_metrics(guarded_width), minimum);
    }
}

#[test]
fn test_column_count_follows_the_width_without_a_cap() {
    // One rule decides the columns: the usable width rounded to the reference
    // card width, never below two. Stating it as a closed form over the whole
    // range rejects any cap, any floor at a historical maximum, and any guard
    // that would silently override the rounded count.
    let mut reported_width = 2.0 * ALBUM_CARD_MIN_WIDTH + ALBUM_SCROLLBAR_RAIL;
    let mut previous_columns = 0;

    while reported_width <= 8008.0 {
        let usable_width = reported_width - ALBUM_SCROLLBAR_RAIL;
        let expected = ((usable_width / ALBUM_CARD_REFERENCE_WIDTH).round() as usize).max(2);
        let metrics = grid_metrics(reported_width);

        assert_eq!(
            metrics.columns, expected,
            "the column rule changed at {reported_width}"
        );
        assert!(
            metrics.columns >= previous_columns,
            "the column count shrank while the width grew at {reported_width}"
        );

        previous_columns = metrics.columns;
        reported_width += 4.0;
    }

    // A wide panel keeps adding columns instead of stopping at a cap, and the
    // card never leaves its bounds to pay for them.
    let wide = grid_metrics(3600.0);
    assert_eq!(wide.columns, 19);
    assert!(wide.card_width >= ALBUM_CARD_MIN_WIDTH && wide.card_width <= ALBUM_CARD_MAX_WIDTH);
    assert!(wide.cover_size >= ALBUM_COVER_MIN && wide.cover_size <= ALBUM_COVER_MAX);
}

#[test]
fn test_keyboard_navigation_steps_by_the_measured_columns() {
    // Keyboard navigation asks the snapshot for the number of items per row and
    // stores the rectangle the virtualizer produced. After a relayout the next
    // step must follow the new column count and land on the new rectangle, not on
    // the geometry the previous layout had.
    let wide = grid_metrics(1969.0);
    assert_eq!(wide.columns, 10);

    let album_count = 60;
    let mut manager = manager_with_albums(album_count, wide);
    let albums = manager.cached_albums.clone().expect("fixture albums");
    manager.focused_item = Some(LibraryListItem::Album(composite_id(&albums[0])));

    let (item, y, height) = manager
        .handle_key_nav(LibraryNavDir::Down, iced::keyboard::Modifiers::NONE)
        .expect("the focused album has a neighbour below");

    assert_eq!(item, LibraryListItem::Album(composite_id(&albums[10])));
    assert!((y - wide.row_height).abs() < EPSILON);
    assert!((height - wide.card_height).abs() < EPSILON);
    assert_eq!(manager.selected_item_hint, Some((y, height)));

    // A narrower layout halves the step: the same focus now moves four albums.
    let narrow = grid_metrics(769.0);
    assert_eq!(narrow.columns, 4);
    manager.grid_metrics.set(narrow);
    manager.invalidate_cache();

    let (item, y, height) = manager
        .handle_key_nav(LibraryNavDir::Down, iced::keyboard::Modifiers::NONE)
        .expect("the focused album still has a neighbour below");

    assert_eq!(item, LibraryListItem::Album(composite_id(&albums[14])));
    assert!((y - 3.0 * narrow.row_height).abs() < EPSILON);
    assert!((height - narrow.card_height).abs() < EPSILON);
    assert_eq!(manager.selected_item_hint, Some((y, height)));

    // The hinted rectangle is the one the renderer will draw for that album.
    let rendered = manager
        .get_visible_items()
        .into_iter()
        .find(|(candidate, _)| *candidate == item)
        .expect("the focused album is part of the rendered grid");
    assert!((rendered.1.y - y).abs() < EPSILON);
    assert!((rendered.1.height - height).abs() < EPSILON);
}

#[test]
fn test_anchor_never_scrolls_past_the_last_page() {
    // A correction is clamped to the scrollable range: an offset past the end of
    // the content lands exactly on the last page, and a negative offset on the
    // first one.
    let previous = grid_metrics(1969.0);
    let next = grid_metrics(1329.0);
    let album_count = 60;
    let viewport_height = 800.0;

    let max_scroll = (total_content_height(&next, album_count, None, 0) - viewport_height).max(0.0);

    let last_page = anchor_viewport_offset(
        &previous,
        &next,
        1.0e6,
        album_count,
        None,
        0,
        viewport_height,
    );
    assert!((last_page - max_scroll).abs() < EPSILON);

    let first_page = anchor_viewport_offset(
        &previous,
        &next,
        -50.0,
        album_count,
        None,
        0,
        viewport_height,
    );
    assert!(first_page.abs() < EPSILON);

    // Inside the content the anchor tracks the row instead of clamping.
    let old_top = row_top_of(&previous, album_count, None, 0, 1);
    let inner = anchor_viewport_offset(
        &previous,
        &next,
        old_top,
        album_count,
        None,
        0,
        viewport_height,
    );
    assert!(inner > 0.0 && inner < max_scroll);
    assert!((inner - row_top_of(&next, album_count, None, 0, 1)).abs() < EPSILON);
}
