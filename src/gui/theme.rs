use iced::{color, Color, Theme, Font};
use iced::font::{Family, Weight, Stretch, Style};
use iced::theme::Palette;

pub const FONT_STAGE_WANDER: Font = Font::with_name("Stage Wander");

pub const FONT_INTER_SANS_NORMAL: Font = Font {
    family: Family::Name("Inter"),
    weight: Weight::Normal,
    stretch: Stretch::SemiExpanded,
    style: Style::Normal,
};
pub const FONT_INTER_SANS_MEDIUM: Font = Font {
    family: Family::Name("Inter"),
    weight: Weight::Medium,
    stretch: Stretch::ExtraExpanded,
    style: Style::Normal,
};

pub const COLOR_ACCENT: Color = color!(0xFF003D);
pub const COLOR_BG: Color = color!(0x000000);
pub const COLOR_CONTRAST: Color = color!(0x111111);
pub const COLOR_TEXT_PRIMARY: Color = color!(0xAFAFAF);
pub const COLOR_TEXT_SECONDARY: Color = color!(0x5B5B5B);
pub const COLOR_SUCCESS: Color = color!(0x10B981);
pub const COLOR_WARNING: Color = color!(0xFBBF24);

pub fn custom_theme() -> Theme {
    Theme::custom(
        "Audoxidy".to_string(),
        Palette {
            background: COLOR_BG,
            text: COLOR_TEXT_PRIMARY,
            primary: COLOR_ACCENT,
            success: COLOR_SUCCESS,
            warning: COLOR_WARNING,
            danger: COLOR_ACCENT,
        }
    )
}

