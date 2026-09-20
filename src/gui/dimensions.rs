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
