//! Pinned values of the dimension tokens and their derived helpers.
//!
//! Every assertion restates the number a caller would see, so a silent
//! renumbering of the token module fails the suite instead of moving pixels.
//! The file imports no rendering code: it pins the numeric authority alone.

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
    assert_eq!(SPACE_SM, 8.0);
    assert_eq!(SPACE_LG, 16.0);
    assert_eq!(SPACE_XL, 24.0);
}

#[test]
fn test_radius_scale_is_fixed() {
    assert_eq!(RADIUS_S, 4.0);
    assert_eq!(RADIUS_M, 8.0);
    assert_eq!(RADIUS_PILL, 999.0);
}

#[test]
fn test_control_heights_are_fixed() {
    assert_eq!(CONTROL_SM, 24.0);
    assert_eq!(CONTROL_MD, 32.0);
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
    assert_eq!(CONTEXT_MENU_PADDING, 5.0);
    assert_eq!(MODAL_MIN_WIDTH, 200.0);
    assert_eq!(FILTER_DROPDOWN_WIDTH, 172.0);
    assert_eq!(FILTER_MENU_ICON, 18.0);
    assert_eq!(FILTER_DROPDOWN_ICON, 28.0);
    assert_eq!(FILTER_OVERLAY_TOP, 70.0);
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

#[test]
fn test_album_card_tokens_are_fixed() {
    assert_eq!(ALBUM_CARD_REFERENCE_WIDTH, 189.0);
    assert_eq!(ALBUM_COVER_REFERENCE, 159.0);
    assert_eq!(ALBUM_CARD_PADDING_TOP, 15.0);
    assert_eq!(ALBUM_CARD_PADDING_SIDE, 15.0);
    assert_eq!(ALBUM_CARD_PADDING_BOTTOM, 12.0);
    assert_eq!(ALBUM_CARD_COVER_TEXT_GAP, 5.0);
    assert_eq!(ALBUM_CARD_TEXT_SIZE, 12.0);
    assert_eq!(ALBUM_CARD_LINE_HEIGHT, 14.0);
    assert_eq!(ALBUM_CARD_LINE_GAP, 2.0);
    assert_eq!(ALBUM_CARD_TEXT_BLOCK_HEIGHT, 62.0);
    assert_eq!(ALBUM_CARD_SELECTION_RADIUS, 10.0);
    assert_eq!(ALBUM_EXPANDED_PADDING, 32.0);
    assert_eq!(ALBUM_EXPANDED_BOTTOM_INSET, 5.0);

    // The cover bounds are literals: the reference cover times 0.7 and 1.3
    // lands one f32 ulp away from the decimal value, so they are compared
    // with a tolerance instead of bit equality.
    assert!((ALBUM_COVER_MIN - ALBUM_COVER_REFERENCE * 0.7).abs() < 0.001);
    assert!((ALBUM_COVER_MAX - ALBUM_COVER_REFERENCE * 1.3).abs() < 0.001);

    // The card bounds derive from those literals and stay bit-exact.
    assert_eq!(ALBUM_CARD_MIN_WIDTH, 141.3);
    assert_eq!(ALBUM_CARD_MAX_WIDTH, 236.7);
}

#[test]
fn test_library_content_origin_derives_from_panels() {
    assert_eq!(FILTER_PANEL_WIDTH, 187.0);
    assert_eq!(ALBUM_SCROLLBAR_RAIL, 4.0);
    assert_eq!(LIBRARY_CONTENT_X_ORIGIN, 591.0);
    assert_eq!(
        LIBRARY_CONTENT_X_ORIGIN,
        PLAYER_PANEL_SIZE + FILTER_PANEL_WIDTH + SPACE_XS
    );
}

#[test]
fn test_layout_tokens_hold_their_documented_values() {
    assert_eq!(SPACE_2XS, 2.0);
    assert_eq!(ALBUM_CARD_LINE_GAP, SPACE_2XS);
    assert_eq!(PLAYLIST_GROUP_HEADER_INSET, 10.0);
    assert_eq!(SCROLLBAR_WIDTH, 8.0);
}

#[test]
fn test_icon_button_box_derives_from_the_icon_and_the_smallest_gap() {
    assert_eq!(icon_button_box(IconSize::Xl), IconSize::Xl.px() + SPACE_XS);
    assert_eq!(icon_button_box(IconSize::Xl), 44.0);
}
