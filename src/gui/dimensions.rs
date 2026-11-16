//! Dimension tokens for the graphical interface.
//!
//! This module is the only source of numeric layout constants in the GUI. Every
//! gap, outer inset, corner radius, interactive control height, horizontal bar
//! height, icon box size and recurring fixed box size is named here once, so two
//! areas that are meant to look equivalent consume the same token instead of
//! repeating the same number. All values are logical pixels; physical-pixel
//! conversion, if it is ever needed, happens at the token boundary and never at a
//! call site.
//!
//! # Allow-list
//!
//! Numeric literals are permitted outside this module only in derived
//! computations that must match runtime geometry rather than express a design
//! choice:
//!
//! - canvas offset math — the VU meter and the equalizer gain guides;
//! - scrollbar track and handle geometry — the shared scrollable widgets;
//! - virtualization margins and row-offset arithmetic — the library and playlist
//!   list views;
//! - measurement-derived clamping and truncation math — the marquee clamp, the
//!   text measurement helpers and the sort-bar column-width heuristic;
//! - custom-widget internal geometry — the number-stepper bands, the slider bands
//!   and the volume scroll area;
//! - pointer hit-testing regions — the player transport and progress-bar mouse
//!   bounds and the panel-alignment offsets, which approximate runtime geometry
//!   rather than express a design size.
//!
//! Even inside those computations a literal that corresponds to a token must be
//! derived from that token instead of repeated. The allow-list is for derived
//! math, never for a design value in disguise; introducing a dimension constant
//! outside this module is not permitted.

/// Gap between two stacked lines of secondary text.
pub const SPACE_2XS: f32 = 2.0;
/// Smallest gap between adjacent elements.
pub const SPACE_XS: f32 = 4.0;
/// Compact gap between adjacent elements.
pub const SPACE_SM: f32 = 8.0;
/// Default gap between adjacent elements inside a group.
pub const SPACE_MD: f32 = 12.0;
/// Gap separating two groups of elements.
pub const SPACE_LG: f32 = 16.0;
/// Section-level gap.
pub const SPACE_XL: f32 = 24.0;
/// Gap between major sections.
pub const SPACE_2XL: f32 = 32.0;

/// Compact outer inset between a region and its container.
pub const MARGIN_SM: f32 = 8.0;
/// Content inset of a bar, panel or dialog.
pub const MARGIN_MD: f32 = 16.0;
/// Outer inset of a major content region.
pub const MARGIN_LG: f32 = 24.0;

/// Small corner rounding for inputs, cells and chips.
pub const RADIUS_S: f32 = 4.0;
/// Medium corner rounding for panels, modals, menus and tooltips.
pub const RADIUS_M: f32 = 8.0;
/// Large corner rounding, reserved for large surfaces.
pub const RADIUS_L: f32 = 12.0;
/// Fully rounded corner, used by pills and toggle thumbs.
pub const RADIUS_PILL: f32 = 999.0;

/// Small interactive control height (dropdowns, small selectors).
pub const CONTROL_SM: f32 = 24.0;
/// Medium interactive control height (list rows, context-menu rows, buttons).
pub const CONTROL_MD: f32 = 32.0;
/// Large interactive control height (bar action buttons, primary controls).
pub const CONTROL_LG: f32 = 40.0;

/// Height of the library and filter top bars.
pub const BAR_TOP: f32 = 40.0;
/// Height of the playlist, library and filter bottom bars.
pub const BAR_BOTTOM: f32 = 40.0;
/// Height of the audio-center tab strip and header bar.
pub const BAR_TAB: f32 = 40.0;
/// Height of the sort bar above its divider.
pub const BAR_SUB: f32 = 28.0;

/// Hairline border or divider thickness.
pub const STROKE_HAIRLINE: f32 = 1.0;
/// Thicker underline below a bar and the audio-center divider.
pub const DIVIDER_THIN: f32 = 2.0;

/// Fixed width of a search field.
pub const WIDTH_SEARCH: f32 = 180.0;
/// Minimum width of a sort-bar column.
pub const SORT_COLUMN_MIN: f32 = 100.0;
/// Minimum width of a context menu.
pub const CONTEXT_MENU_MIN_WIDTH: f32 = 80.0;
/// Fixed width of the equalizer preset list.
pub const PRESET_LIST_WIDTH: f32 = 140.0;
/// Fixed width of the playlist module panel.
pub const PLAYLIST_PANEL_WIDTH: f32 = 400.0;
/// Fixed width of the square player panel.
pub const PLAYER_PANEL_SIZE: f32 = 400.0;
/// Fixed width of the library filters panel.
pub const FILTER_PANEL_WIDTH: f32 = 187.0;
/// Horizontal reclaim on the album card's right edge.
///
/// The expansion chevron is a 32 px box whose visible glyph is inset a few
/// pixels, so the bottom row reserves this much less on the right: the chevron
/// moves that many pixels towards the card edge and the text block gains the
/// same width. The cover keeps its fixed size and its left alignment.
pub const ALBUM_CARD_CHEVRON_INSET: f32 = 5.0;
/// Left edge of the library content, derived from the player column, the filters
/// panel and the content inset instead of restating the window-to-content offset.
pub const LIBRARY_CONTENT_X_ORIGIN: f32 = PLAYER_PANEL_SIZE + FILTER_PANEL_WIDTH + SPACE_XS;
/// Width of the visible rail of the shared vertical scrollbar. This is the width
/// the grid reserves: the scrollbar's pointer target is the full 8 px box, so its
/// transparent left half overlays the last 4 px of the row and can capture clicks
/// there. The geometry is owner-approved; do not widen the reserve.
pub const ALBUM_SCROLLBAR_RAIL: f32 = 4.0;
/// Side of the shared scrollbar's box, shared by its track and its scroller.
///
/// It is not twice the visible rail: the rail is the width the grid reserves,
/// while this is the whole box the scrollbar occupies.
pub const SCROLLBAR_WIDTH: f32 = 8.0;
/// Fixed width of the VU meter numeric readout.
pub const METER_NUMERIC_WIDTH: f32 = 110.0;
/// Fixed width of the audio-center window.
pub const AUDIO_CENTER_WIDTH: f32 = 940.0;
/// Fixed height of the audio-center window.
pub const AUDIO_CENTER_HEIGHT: f32 = 480.0;
/// Height of a library list row.
pub const LIST_ROW: f32 = 42.0;
/// Height of a list header or tree row.
pub const LIST_ROW_HEADER: f32 = 32.0;
/// Height of a playlist song or folder row.
pub const PLAYLIST_ROW: f32 = 46.0;
/// Space under the divider of a playlist group header.
pub const PLAYLIST_GROUP_HEADER_INSET: f32 = 10.0;

// ── Residual design values ─────────────────────────────────────────
//
// The values below are the remaining design choices of the audio-center
// tabs, the dialogs and the player chrome. They have no place on the shared
// spacing scale (a stack gap that must stay a hairline apart, a panel width
// driven by its content, a placement offset), so each one is named here
// instead of being repeated at its call site.

/// Width of an audio-configuration dropdown.
pub const CONFIG_DROPDOWN_WIDTH: f32 = 125.0;
/// Width of the wider output-device dropdown in the configuration tab.
pub const CONFIG_DROPDOWN_WIDE: f32 = 260.0;
/// Width of a configuration-tab field label column.
pub const CONFIG_LABEL_WIDTH: f32 = 180.0;
/// Width of the leftmost label column of the audio information table.
pub const CONFIG_TABLE_LABEL_WIDTH: f32 = 90.0;
/// Width of the divider column of the audio information table.
pub const CONFIG_TABLE_DIVIDER_COLUMN: f32 = 20.0;
/// Width of a field label in the right column of the configuration tab.
pub const CONFIG_FIELD_LABEL_WIDTH: f32 = 80.0;
/// Width of the column that holds the tall separator in the configuration tab.
pub const CONFIG_SEPARATOR_COLUMN: f32 = 80.0;
/// Width of an information label in the right column of the configuration tab.
pub const CONFIG_INFO_LABEL_WIDTH: f32 = 170.0;
/// Height of the vertical divider inside the audio information table.
pub const CONFIG_TABLE_DIVIDER_HEIGHT: f32 = 90.0;
/// Gap between two labels stacked in one configuration-tab cell.
pub const CONFIG_LABEL_STACK_SPACING: f32 = 2.0;
/// Height of the tall vertical separator between two panel columns.
pub const PANEL_SEPARATOR_HEIGHT: f32 = 220.0;
/// Width of the panel column that holds a vertical separator.
pub const PANEL_SEPARATOR_COLUMN: f32 = 30.0;
/// Height of an effect parameter slider.
pub const EFFECT_SLIDER_HEIGHT: f32 = 18.0;
/// Width of the numeric input attached to an effect slider.
pub const EFFECT_INPUT_WIDTH: f32 = 40.0;

// ── Album card ─────────────────────────────────────────────────────
//
// The card mixes a scaling cover with fixed internal chrome: the paddings, the
// text block and the radii never follow the cover, so they are named here and
// consumed by the geometry model instead of being repeated at the render sites.

/// Reference outer width of an album card; it is the divisor of the column rule.
pub const ALBUM_CARD_REFERENCE_WIDTH: f32 = 189.0;
/// Reference side of the square album cover at the reference card width.
pub const ALBUM_COVER_REFERENCE: f32 = 159.0;
/// Smallest cover side: the reference cover at −30 %.
///
/// Kept as a literal instead of a product of the reference cover because the f32
/// product lands one ulp away from the decimal value the design states.
pub const ALBUM_COVER_MIN: f32 = 111.3;
/// Largest cover side: the reference cover at +30 %.
///
/// Kept as a literal for the same reason as the lower bound.
pub const ALBUM_COVER_MAX: f32 = 206.7;
/// Fixed top padding of an album card.
pub const ALBUM_CARD_PADDING_TOP: f32 = 15.0;
/// Fixed left and right padding of an album card.
pub const ALBUM_CARD_PADDING_SIDE: f32 = 15.0;
/// Fixed bottom padding of an album card.
pub const ALBUM_CARD_PADDING_BOTTOM: f32 = 12.0;
/// Fixed gap between the cover and the text block.
pub const ALBUM_CARD_COVER_TEXT_GAP: f32 = 5.0;
/// Font size of the four card meta lines.
pub const ALBUM_CARD_TEXT_SIZE: f32 = 12.0;
/// Absolute line height of the four card meta lines.
pub const ALBUM_CARD_LINE_HEIGHT: f32 = 14.0;
/// Gap between two stacked card meta lines.
pub const ALBUM_CARD_LINE_GAP: f32 = SPACE_2XS;
/// Height of the four-line card text block, derived from its lines so a line
/// height change cannot desynchronise the virtualized row height.
pub const ALBUM_CARD_TEXT_BLOCK_HEIGHT: f32 =
    4.0 * ALBUM_CARD_LINE_HEIGHT + 3.0 * ALBUM_CARD_LINE_GAP;
/// Corner radius of the selection and expansion fill of an album card.
pub const ALBUM_CARD_SELECTION_RADIUS: f32 = 10.0;
/// Top and bottom padding of an expanded album song block.
pub const ALBUM_EXPANDED_PADDING: f32 = SPACE_2XL;
/// Bottom inset of the container that wraps an expanded album song block.
pub const ALBUM_EXPANDED_BOTTOM_INSET: f32 = 5.0;
/// Placeholder album icon side divided by the cover side.
pub const ALBUM_PLACEHOLDER_ICON_RATIO: f32 = 96.0 / ALBUM_COVER_REFERENCE;
/// Narrowest album card: the smallest cover plus both fixed side paddings.
pub const ALBUM_CARD_MIN_WIDTH: f32 = ALBUM_COVER_MIN + 2.0 * ALBUM_CARD_PADDING_SIDE;
/// Widest album card: the largest cover plus both fixed side paddings.
pub const ALBUM_CARD_MAX_WIDTH: f32 = ALBUM_COVER_MAX + 2.0 * ALBUM_CARD_PADDING_SIDE;

/// Height of the scroll area of the preset list in the load-preset dialog.
pub const EQ_PRESET_SCROLL_HEIGHT: f32 = 287.0;
/// Height of the preset list panel in the load-preset dialog.
pub const EQ_PRESET_PANEL_HEIGHT: f32 = 289.0;
/// Width of a dialog action button.
pub const DIALOG_BUTTON_WIDTH: f32 = 150.0;
/// Width of the load-preset dialog panel.
pub const EQ_DIALOG_WIDTH: f32 = 330.0;
/// Height of the load-preset dialog panel.
pub const EQ_DIALOG_HEIGHT: f32 = 352.0;
/// Inset of the load-preset dialog panel.
pub const EQ_DIALOG_PADDING: f32 = 17.0;
/// Vertical offset applied when centering the load-preset dialog.
pub const EQ_DIALOG_VERTICAL_OFFSET: f32 = 15.0;
/// Width of an EQ save-dialog button.
pub const EQ_SAVE_BUTTON_WIDTH: f32 = 74.0;
/// Width of the save-preset dialog panel.
pub const EQ_SAVE_DIALOG_WIDTH: f32 = 190.0;
/// Height of the save-preset dialog panel.
pub const EQ_SAVE_DIALOG_HEIGHT: f32 = 162.0;
/// Top offset of the save-preset dialog inside the audio-center window.
pub const EQ_SAVE_DIALOG_TOP: f32 = 120.0;
/// Assumed width of an anchored dialog, used to keep it inside the window.
pub const DIALOG_DEFAULT_WIDTH: f32 = 350.0;
/// Assumed height of an anchored dialog, used to keep it inside the window.
pub const DIALOG_DEFAULT_HEIGHT: f32 = 240.0;
/// Outer margin kept between a dialog and the window edge.
pub const DIALOG_EDGE_MARGIN: f32 = 10.0;
/// Anchor offset applied to place a dialog near its trigger.
pub const DIALOG_ANCHOR_OFFSET: f32 = 20.0;
/// Fallback position of a dialog that has no anchor.
pub const DIALOG_FALLBACK_POS: f32 = 200.0;
/// Extra margin reserved when clamping a dialog inside the window.
pub const DIALOG_CLAMP_MARGIN: f32 = 40.0;

/// Height of the player title line.
pub const PLAYER_TITLE_HEIGHT: f32 = 35.0;
/// Height of the player artist line.
pub const PLAYER_ARTIST_HEIGHT: f32 = 30.0;
/// Height of the player progress line.
pub const PLAYER_PROGRESS_ROW_HEIGHT: f32 = 36.0;
/// Vertical padding of the player progress bar.
pub const PLAYER_PROGRESS_PADDING_V: f32 = 15.0;
/// Horizontal padding of the player progress bar.
pub const PLAYER_PROGRESS_PADDING_H: f32 = 13.0;
/// Padding inside a player tooltip box.
pub const PLAYER_TOOLTIP_PADDING: f32 = 6.0;
/// Inset of the player dropdown menu.
pub const PLAYER_MENU_PADDING: f32 = 15.0;
/// Width of the player dropdown menu.
pub const PLAYER_MENU_WIDTH: f32 = 180.0;
/// Hover padding of the wide player transport buttons.
pub const PLAYER_ZONE_PAD_WIDE: f32 = 7.0;
/// Hover padding of the tight player transport buttons.
pub const PLAYER_ZONE_PAD_TIGHT: f32 = 3.0;

/// Width of the native context menu.
pub const CONTEXT_MENU_WIDTH: f32 = 260.0;
/// Vertical padding of the native context menu.
pub const CONTEXT_MENU_PADDING: f32 = 5.0;
/// Minimum width of a standard modal dialog.
pub const MODAL_MIN_WIDTH: f32 = 200.0;
/// Width of the library filter dropdown.
pub const FILTER_DROPDOWN_WIDTH: f32 = 172.0;
/// Icon box of a row in the library filter dropdown.
pub const FILTER_MENU_ICON: f32 = 18.0;
/// Icon box of the library filter dropdown header chevron.
pub const FILTER_DROPDOWN_ICON: f32 = 28.0;
/// Top offset of the library filter menu overlay.
pub const FILTER_OVERLAY_TOP: f32 = 70.0;
/// Margin kept between an overlay and the window edge.
pub const OVERLAY_EDGE_MARGIN: f32 = 5.0;
/// Width of the VU-meter configuration popup.
pub const METER_POPUP_WIDTH: f32 = 280.0;
/// Height of the VU-meter configuration popup.
pub const METER_POPUP_HEIGHT: f32 = 200.0;

/// Icon box sizes for SVG icons.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconSize {
    /// Small inline icon.
    S,
    /// Medium icon for bars, menus and window controls.
    M,
    /// Large icon for top-bar window controls and bar actions.
    L,
    /// Extra-large icon for player transport controls.
    Xl,
}

impl IconSize {
    /// The icon box side in logical pixels.
    pub const fn px(self) -> f32 {
        match self {
            Self::S => 16.0,
            Self::M => 24.0,
            Self::L => 32.0,
            Self::Xl => 40.0,
        }
    }
}

/// The hit box of every icon button, derived from the icon box plus the smallest
/// gap so it is never written as a literal.
pub const fn icon_button_box(icon: IconSize) -> f32 {
    icon.px() + SPACE_XS
}

#[cfg(test)]
#[path = "dimensions_tests.rs"]
mod tests;
