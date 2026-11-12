//! Pure geometry of the album grid.
//!
//! One measured width produces one [`GridMetrics`] snapshot, and every consumer
//! of card geometry — the renderer, the row virtualization, the selection math
//! and the reveal paths — projects from that snapshot. The module holds no
//! widget, no global state and no interior mutability, so its arithmetic can be
//! exercised without a window, a renderer or a database.
//!
//! The width handed to [`grid_metrics`] is the one the layout pass reports for
//! the library content, and it still includes the visible scrollbar rail: that
//! rail is subtracted here so no caller can forget it. The transparent hit border
//! around the rail is not subtracted because it never covers a card.

use crate::gui::dimensions::{
    ALBUM_CARD_COVER_TEXT_GAP, ALBUM_CARD_MAX_WIDTH, ALBUM_CARD_MIN_WIDTH,
    ALBUM_CARD_PADDING_BOTTOM, ALBUM_CARD_PADDING_SIDE, ALBUM_CARD_PADDING_TOP,
    ALBUM_CARD_REFERENCE_WIDTH, ALBUM_CARD_TEXT_BLOCK_HEIGHT, ALBUM_EXPANDED_BOTTOM_INSET,
    ALBUM_EXPANDED_PADDING, ALBUM_PLACEHOLDER_ICON_RATIO, ALBUM_SCROLLBAR_RAIL, LIST_ROW_HEADER,
};

/// Geometry of the whole grid, derived once per layout from the measured width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridMetrics {
    /// Columns in a full row.
    pub columns: usize,
    /// Outer width of one card, padding included.
    pub card_width: f32,
    /// Outer height of one card, padding included.
    pub card_height: f32,
    /// Side of the square cover.
    pub cover_size: f32,
    /// Height of the four-line text block below the cover.
    pub text_block_height: f32,
    /// Vertical step between two rows.
    pub row_height: f32,
    /// Gap between cards on both axes.
    pub gap: f32,
}

impl GridMetrics {
    /// Builds the snapshot for a card of `card_width`.
    ///
    /// The cover takes the width left by the two fixed side paddings, and the
    /// card height stacks top padding, cover, cover-to-text gap, text block and
    /// bottom padding. Only the cover follows the card width. The two-column
    /// minimum is recorded as the column count; the width-driven column rule in
    /// [`grid_metrics`] overwrites it.
    pub fn from_card_width(card_width: f32) -> Self {
        let cover_size = card_width - 2.0 * ALBUM_CARD_PADDING_SIDE;
        let card_height = ALBUM_CARD_PADDING_TOP
            + cover_size
            + ALBUM_CARD_COVER_TEXT_GAP
            + ALBUM_CARD_TEXT_BLOCK_HEIGHT
            + ALBUM_CARD_PADDING_BOTTOM;

        Self {
            columns: 2,
            card_width,
            card_height,
            cover_size,
            text_block_height: ALBUM_CARD_TEXT_BLOCK_HEIGHT,
            row_height: card_height,
            gap: 0.0,
        }
    }

    /// The smallest supported layout: the shortest card, two columns.
    ///
    /// Used before the first layout pass and for any width the geometry cannot
    /// trust, so consumers never see zero columns, a zero card or a non-finite
    /// height.
    pub fn minimum() -> Self {
        Self::from_card_width(ALBUM_CARD_MIN_WIDTH)
    }
}

/// Derives the grid geometry from the width the layout pass reports.
///
/// The column count follows one rule: round the usable width to the reference
/// card width, clamp by the card bounds, and never go below two columns; the
/// card width then absorbs the whole usable width so a full row leaves no
/// remainder. The floating scrollbar rail is subtracted from `reported_width`
/// here, and the function is stateless — the same width always produces the
/// same snapshot whatever the resize history was.
pub fn grid_metrics(reported_width: f32) -> GridMetrics {
    if !reported_width.is_finite() || reported_width <= 0.0 {
        return GridMetrics::minimum();
    }

    let usable_width = reported_width - ALBUM_SCROLLBAR_RAIL;
    if !usable_width.is_finite() || usable_width <= 0.0 {
        return GridMetrics::minimum();
    }

    let mut columns = ((usable_width / ALBUM_CARD_REFERENCE_WIDTH).round() as usize).max(2);
    if usable_width / (columns as f32) > ALBUM_CARD_MAX_WIDTH {
        columns = (usable_width / ALBUM_CARD_MAX_WIDTH).ceil() as usize;
    }
    if usable_width / (columns as f32) < ALBUM_CARD_MIN_WIDTH {
        columns = (usable_width / ALBUM_CARD_MIN_WIDTH).floor() as usize;
    }
    columns = columns.max(2);

    let mut metrics = GridMetrics::from_card_width(usable_width / (columns as f32));
    metrics.columns = columns;
    metrics
}

/// Number of rows needed for `album_count` albums; an empty library has none.
pub fn row_count(metrics: &GridMetrics, album_count: usize) -> usize {
    if album_count == 0 {
        return 0;
    }
    album_count.div_ceil(metrics.columns)
}

/// Vertical extent of one grid row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowSpan {
    /// Top edge of the row in content coordinates.
    pub top: f32,
    /// Full height of the row, expanded song block included.
    pub height: f32,
    /// Whether the expanded album sits in this row.
    pub expanded: bool,
}

/// Height of a row that hosts an expanded album.
///
/// The chrome is the two fixed paddings of the song block plus the bottom inset
/// of the container around it; every rendered song adds one list row. A block
/// with no rendered song keeps exactly the chrome, so nothing is reserved for a
/// song that does not exist.
pub fn expanded_row_height(metrics: &GridMetrics, rendered_songs: usize) -> f32 {
    metrics.card_height
        + 2.0 * ALBUM_EXPANDED_PADDING
        + ALBUM_EXPANDED_BOTTOM_INSET
        + rendered_songs as f32 * LIST_ROW_HEADER
}

/// Top edge of the `rendered_index`-th rendered song inside an expanded row.
///
/// The index counts only the songs the renderer receives, so a filtered-out song
/// never shifts the ones below it.
pub fn song_y_in_row(metrics: &GridMetrics, row_top: f32, rendered_index: usize) -> f32 {
    row_top + metrics.card_height + ALBUM_EXPANDED_PADDING + rendered_index as f32 * LIST_ROW_HEADER
}

/// Top edge of `row_index`, folding the heights of every row above it.
///
/// The fold is allocation-free and stops at the total row count, so an
/// `expanded_row` past the last row is ignored.
pub fn row_top_of(
    metrics: &GridMetrics,
    album_count: usize,
    expanded_row: Option<usize>,
    rendered_songs: usize,
    row_index: usize,
) -> f32 {
    let rows = row_count(metrics, album_count);
    let limit = row_index.min(rows);
    let expanded_height = expanded_row_height(metrics, rendered_songs);

    let mut top = 0.0;
    for row in 0..limit {
        top += if expanded_row == Some(row) {
            expanded_height
        } else {
            metrics.row_height
        };
    }
    top
}

/// Total height of the grid content: every row plus the expanded block.
pub fn total_content_height(
    metrics: &GridMetrics,
    album_count: usize,
    expanded_row: Option<usize>,
    rendered_songs: usize,
) -> f32 {
    row_top_of(
        metrics,
        album_count,
        expanded_row,
        rendered_songs,
        row_count(metrics, album_count),
    )
}

/// Builds one [`RowSpan`] per grid row in a single pass over the row count.
pub fn grid_rows(
    metrics: &GridMetrics,
    album_count: usize,
    expanded_row: Option<usize>,
    rendered_songs: usize,
) -> Vec<RowSpan> {
    let rows = row_count(metrics, album_count);
    let expanded_height = expanded_row_height(metrics, rendered_songs);

    let mut spans = Vec::with_capacity(rows);
    let mut top = 0.0;
    for row in 0..rows {
        let expanded = expanded_row == Some(row);
        let height = if expanded {
            expanded_height
        } else {
            metrics.row_height
        };
        spans.push(RowSpan {
            top,
            height,
            expanded,
        });
        top += height;
    }
    spans
}

/// Scroll offset that keeps a row at the same viewport position after a relayout.
///
/// `old_row_top` and `new_row_top` are the row's top edge before and after the
/// geometry changed, so their difference is the height change of every row above
/// it. The returned offset moves the old offset by that delta and clamps it to
/// the scrollable range, so a correction can never land above the content or
/// past its last page.
pub fn anchor_offset(old_offset: f32, old_row_top: f32, new_row_top: f32, max_scroll: f32) -> f32 {
    (old_offset + (new_row_top - old_row_top)).clamp(0.0, max_scroll.max(0.0))
}

/// Scroll offset that keeps the first visible album at the same viewport
/// position after the grid geometry changes.
///
/// With no expanded album every row has the same height, so the row at the top
/// of the viewport is the anchor: its first album is located in the new layout,
/// the offset inside the row is scaled by the row-height ratio, and the result
/// is clamped to the scrollable range. Callers invoke this only when the
/// geometry generation actually changed.
pub fn anchor_row_offset(
    previous: &GridMetrics,
    next: &GridMetrics,
    offset_y: f32,
    album_count: usize,
    viewport_height: f32,
) -> f32 {
    anchor_viewport_offset(
        previous,
        next,
        offset_y,
        album_count,
        None,
        0,
        viewport_height,
    )
}

/// Row index and top edge of the row that contains `offset`.
///
/// The expanded row, when present, contributes its own height; every other row
/// contributes `metrics.row_height`. The walk stops at the first row whose span
/// contains the offset, so the cost is bounded by the number of rows above the
/// viewport.
pub fn row_at_offset(
    metrics: &GridMetrics,
    album_count: usize,
    expanded_row: Option<usize>,
    rendered_songs: usize,
    offset: f32,
) -> (usize, f32) {
    let rows = row_count(metrics, album_count);
    if rows == 0 {
        return (0, 0.0);
    }

    let offset = offset.max(0.0);
    let expanded_height = expanded_row_height(metrics, rendered_songs);
    let mut top = 0.0;
    for row in 0..rows {
        let height = if expanded_row == Some(row) {
            expanded_height
        } else {
            metrics.row_height
        };
        if offset < top + height || row + 1 == rows {
            return (row, top);
        }
        top += height;
    }
    (rows - 1, top)
}

/// Scroll offset that keeps the first visible album at the same viewport
/// position after the geometry changes, with or without an expanded album.
///
/// The anchor is always the first album of the first visible row: the expanded
/// album only contributes its height to the rows above the viewport, never the
/// anchor itself. This keeps the rule independent of whether an expansion is
/// present or currently visible.
pub fn anchor_viewport_offset(
    previous: &GridMetrics,
    next: &GridMetrics,
    offset_y: f32,
    album_count: usize,
    expanded_album_index: Option<usize>,
    rendered_songs: usize,
    viewport_height: f32,
) -> f32 {
    if album_count == 0 || previous.row_height <= 0.0 || next.row_height <= 0.0 {
        return 0.0;
    }

    let old_expanded_row = expanded_album_index.map(|index| index / previous.columns.max(1));
    let new_expanded_row = expanded_album_index.map(|index| index / next.columns.max(1));
    let (old_row, old_top) = row_at_offset(
        previous,
        album_count,
        old_expanded_row,
        rendered_songs,
        offset_y,
    );
    let album_index = old_row * previous.columns.max(1);
    let new_rows = row_count(next, album_count);
    let new_row = (album_index / next.columns.max(1)).min(new_rows.saturating_sub(1));
    let new_top = row_top_of(next, album_count, new_expanded_row, rendered_songs, new_row);

    let old_height = if old_expanded_row == Some(old_row) {
        expanded_row_height(previous, rendered_songs)
    } else {
        previous.row_height
    };
    let new_height = if new_expanded_row == Some(new_row) {
        expanded_row_height(next, rendered_songs)
    } else {
        next.row_height
    };
    if old_height <= 0.0 || new_height <= 0.0 {
        return new_top;
    }

    let intra = (offset_y.max(0.0) - old_top).max(0.0);
    let scaled = (intra * (new_height / old_height)).min(new_height);
    let max_scroll = (total_content_height(next, album_count, new_expanded_row, rendered_songs)
        - viewport_height.max(0.0))
    .max(0.0);

    (new_top + scaled).clamp(0.0, max_scroll)
}

/// Scroll offset that shows `row_y..row_y + row_h` completely inside the viewport.
///
/// The offset must be at least the row's bottom edge minus the viewport height
/// and at most the row's top edge. Both bounds are clamped to the scrollable
/// range and then ordered, because a row taller than the viewport cannot satisfy
/// them at once. The input offset is kept whenever it already lies inside that
/// range, so a visible row never moves and a correction scrolls the minimum
/// amount that makes the row visible.
pub fn keep_row_visible(
    offset: f32,
    row_y: f32,
    row_h: f32,
    viewport_h: f32,
    max_scroll: f32,
) -> f32 {
    let limit = max_scroll.max(0.0);
    let lower = (row_y + row_h - viewport_h).clamp(0.0, limit);
    let upper = row_y.clamp(0.0, limit);

    offset.clamp(lower.min(upper), lower.max(upper))
}

/// Side of the placeholder album icon drawn inside a cover of `cover_size`.
///
/// The ratio reproduces the reference icon exactly and keeps the icon plus the
/// fixed wordmark inside the smallest supported cover.
pub fn album_placeholder_icon_side(cover_size: f32) -> f32 {
    (cover_size * ALBUM_PLACEHOLDER_ICON_RATIO).round()
}
