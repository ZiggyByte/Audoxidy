//! Button palette and hit-box rules for the icon buttons.
//!
//! The tests read the pure helpers the buttons are built from: an interaction
//! state and a variant resolve to a color pair, and the hit box follows the
//! icon size. No window, renderer or message loop takes part.

use super::{IconState, IconVariant, icon_palette};
use crate::gui::dimensions::{IconSize, icon_button_box};
use crate::gui::theme::{COLOR_ACCENT, COLOR_TEXT_PRIMARY, COLOR_TEXT_SECONDARY};
use iced::Color;

#[test]
fn test_icon_palette_default_resting_uses_secondary() {
    assert_eq!(
        icon_palette(IconState::Default, IconVariant::Default, false),
        (COLOR_TEXT_SECONDARY, Color::TRANSPARENT)
    );
}

#[test]
fn test_icon_palette_default_hover_uses_accent_background() {
    assert_eq!(
        icon_palette(IconState::Default, IconVariant::Default, true),
        (COLOR_TEXT_PRIMARY, COLOR_ACCENT)
    );
}

#[test]
fn test_icon_palette_active_resting_uses_accent_icon() {
    assert_eq!(
        icon_palette(IconState::Active, IconVariant::Default, false),
        (COLOR_ACCENT, Color::TRANSPARENT)
    );
}

#[test]
fn test_icon_palette_active_hover_still_uses_accent_background() {
    assert_eq!(
        icon_palette(IconState::Active, IconVariant::Default, true),
        (COLOR_TEXT_PRIMARY, COLOR_ACCENT)
    );
}

#[test]
fn test_icon_palette_disabled_resting_uses_secondary() {
    assert_eq!(
        icon_palette(IconState::Disabled, IconVariant::Default, false),
        (COLOR_TEXT_SECONDARY, Color::TRANSPARENT)
    );
}

#[test]
fn test_icon_palette_disabled_ignores_hover() {
    assert_eq!(
        icon_palette(IconState::Disabled, IconVariant::Default, true),
        (COLOR_TEXT_SECONDARY, Color::TRANSPARENT)
    );
}

#[test]
fn test_icon_palette_borderless_ignores_hover() {
    assert_eq!(
        icon_palette(IconState::Default, IconVariant::Borderless, true),
        (COLOR_TEXT_SECONDARY, Color::TRANSPARENT)
    );
}

#[test]
fn test_icon_palette_borderless_shares_the_active_color() {
    assert_eq!(
        icon_palette(IconState::Active, IconVariant::Borderless, false),
        (COLOR_ACCENT, Color::TRANSPARENT)
    );
}

#[test]
fn test_icon_button_box_small_preserves_the_eq_hit_box() {
    assert_eq!(icon_button_box(IconSize::S), 20.0);
}
