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
//!   and the volume scroll area.
//!
//! Even inside those computations a literal that corresponds to a token must be
//! derived from that token instead of repeated. The allow-list is for derived
//! math, never for a design value in disguise; introducing a dimension constant
//! outside this module is not permitted.

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
mod tests {
    use super::*;

    #[test]
    fn test_spacing_scale_matches_the_base_unit() {
        assert_eq!(SPACE_XS, 4.0);
        assert_eq!(SPACE_SM, 8.0);
        assert_eq!(SPACE_MD, 12.0);
        assert_eq!(SPACE_LG, 16.0);
        assert_eq!(SPACE_XL, 24.0);
        assert_eq!(SPACE_2XL, 32.0);
    }

    #[test]
    fn test_margin_scale_is_fixed() {
        assert_eq!(MARGIN_SM, 8.0);
        assert_eq!(MARGIN_MD, 16.0);
        assert_eq!(MARGIN_LG, 24.0);
    }

    #[test]
    fn test_radius_scale_is_fixed() {
        assert_eq!(RADIUS_S, 4.0);
        assert_eq!(RADIUS_M, 8.0);
        assert_eq!(RADIUS_L, 12.0);
        assert_eq!(RADIUS_PILL, 999.0);
    }

    #[test]
    fn test_control_heights_are_fixed() {
        assert_eq!(CONTROL_SM, 24.0);
        assert_eq!(CONTROL_MD, 32.0);
        assert_eq!(CONTROL_LG, 40.0);
    }

    #[test]
    fn test_bar_heights_are_fixed() {
        assert_eq!(BAR_TOP, 40.0);
        assert_eq!(BAR_BOTTOM, 40.0);
        assert_eq!(BAR_TAB, 40.0);
        assert_eq!(BAR_SUB, 28.0);
    }

    #[test]
    fn test_stroke_thicknesses_are_fixed() {
        assert_eq!(STROKE_HAIRLINE, 1.0);
        assert_eq!(DIVIDER_THIN, 2.0);
    }

    #[test]
    fn test_recurring_box_sizes_are_fixed() {
        assert_eq!(WIDTH_SEARCH, 180.0);
        assert_eq!(SORT_COLUMN_MIN, 100.0);
        assert_eq!(CONTEXT_MENU_MIN_WIDTH, 80.0);
        assert_eq!(PRESET_LIST_WIDTH, 140.0);
        assert_eq!(PLAYLIST_PANEL_WIDTH, 400.0);
        assert_eq!(PLAYER_PANEL_SIZE, 400.0);
        assert_eq!(METER_NUMERIC_WIDTH, 110.0);
        assert_eq!(AUDIO_CENTER_WIDTH, 940.0);
        assert_eq!(AUDIO_CENTER_HEIGHT, 480.0);
        assert_eq!(LIST_ROW, 42.0);
        assert_eq!(LIST_ROW_HEADER, 32.0);
        assert_eq!(PLAYLIST_ROW, 46.0);
    }

    #[test]
    fn test_residual_design_values_are_fixed() {
        assert_eq!(CONFIG_DROPDOWN_WIDTH, 125.0);
        assert_eq!(CONFIG_DROPDOWN_WIDE, 260.0);
        assert_eq!(CONFIG_LABEL_WIDTH, 180.0);
        assert_eq!(CONFIG_TABLE_LABEL_WIDTH, 90.0);
        assert_eq!(CONFIG_TABLE_DIVIDER_COLUMN, 20.0);
        assert_eq!(CONFIG_FIELD_LABEL_WIDTH, 80.0);
        assert_eq!(CONFIG_SEPARATOR_COLUMN, 80.0);
        assert_eq!(CONFIG_INFO_LABEL_WIDTH, 170.0);
        assert_eq!(CONFIG_TABLE_DIVIDER_HEIGHT, 90.0);
        assert_eq!(CONFIG_LABEL_STACK_SPACING, 2.0);
        assert_eq!(PANEL_SEPARATOR_HEIGHT, 220.0);
        assert_eq!(PANEL_SEPARATOR_COLUMN, 30.0);
        assert_eq!(EFFECT_SLIDER_HEIGHT, 18.0);
        assert_eq!(EFFECT_INPUT_WIDTH, 40.0);
        assert_eq!(EQ_PRESET_SCROLL_HEIGHT, 287.0);
        assert_eq!(EQ_PRESET_PANEL_HEIGHT, 289.0);
        assert_eq!(DIALOG_BUTTON_WIDTH, 150.0);
        assert_eq!(EQ_DIALOG_WIDTH, 330.0);
        assert_eq!(EQ_DIALOG_HEIGHT, 352.0);
        assert_eq!(EQ_DIALOG_PADDING, 17.0);
        assert_eq!(EQ_DIALOG_VERTICAL_OFFSET, 15.0);
        assert_eq!(EQ_SAVE_BUTTON_WIDTH, 74.0);
        assert_eq!(EQ_SAVE_DIALOG_WIDTH, 190.0);
        assert_eq!(EQ_SAVE_DIALOG_HEIGHT, 162.0);
        assert_eq!(EQ_SAVE_DIALOG_TOP, 120.0);
        assert_eq!(DIALOG_DEFAULT_WIDTH, 350.0);
        assert_eq!(DIALOG_DEFAULT_HEIGHT, 240.0);
        assert_eq!(DIALOG_EDGE_MARGIN, 10.0);
        assert_eq!(DIALOG_ANCHOR_OFFSET, 20.0);
        assert_eq!(DIALOG_FALLBACK_POS, 200.0);
        assert_eq!(DIALOG_CLAMP_MARGIN, 40.0);
        assert_eq!(PLAYER_TITLE_HEIGHT, 35.0);
        assert_eq!(PLAYER_ARTIST_HEIGHT, 30.0);
        assert_eq!(PLAYER_PROGRESS_ROW_HEIGHT, 36.0);
        assert_eq!(PLAYER_PROGRESS_PADDING_V, 15.0);
        assert_eq!(PLAYER_PROGRESS_PADDING_H, 13.0);
        assert_eq!(PLAYER_TOOLTIP_PADDING, 6.0);
        assert_eq!(PLAYER_MENU_PADDING, 15.0);
        assert_eq!(PLAYER_MENU_WIDTH, 180.0);
        assert_eq!(PLAYER_ZONE_PAD_WIDE, 7.0);
        assert_eq!(PLAYER_ZONE_PAD_TIGHT, 3.0);
        assert_eq!(CONTEXT_MENU_WIDTH, 260.0);
        assert_eq!(OVERLAY_EDGE_MARGIN, 5.0);
        assert_eq!(METER_POPUP_WIDTH, 280.0);
        assert_eq!(METER_POPUP_HEIGHT, 200.0);
    }

    #[test]
    fn test_icon_size_pixels_are_fixed() {
        assert_eq!(IconSize::S.px(), 16.0);
        assert_eq!(IconSize::M.px(), 24.0);
        assert_eq!(IconSize::L.px(), 32.0);
        assert_eq!(IconSize::Xl.px(), 40.0);
    }

    #[test]
    fn test_icon_button_box_derives_from_the_icon_size() {
        assert_eq!(icon_button_box(IconSize::S), 20.0);
        assert_eq!(icon_button_box(IconSize::M), 28.0);
        assert_eq!(icon_button_box(IconSize::L), 36.0);
        assert_eq!(icon_button_box(IconSize::Xl), 44.0);
    }
}
