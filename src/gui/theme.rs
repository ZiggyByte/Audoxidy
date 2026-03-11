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

pub fn custom_scrollbar_style(
    _theme: &Theme,
    status: iced::widget::scrollable::Status,
) -> iced::widget::scrollable::Style {
    let color = match status {
        iced::widget::scrollable::Status::Hovered { is_vertical_scrollbar_hovered, is_horizontal_scrollbar_hovered, .. } => {
            if is_vertical_scrollbar_hovered || is_horizontal_scrollbar_hovered {
                Color::from(COLOR_TEXT_PRIMARY)
            } else {
                Color::from(COLOR_CONTRAST)
            }
        }
        iced::widget::scrollable::Status::Dragged { .. } => Color::from(COLOR_TEXT_PRIMARY),
        _ => Color::TRANSPARENT,
    };
    
    iced::widget::scrollable::Style {
        container: iced::widget::container::Style::default(),
        vertical_rail: iced::widget::scrollable::Rail {
            background: None,
            border: iced::Border::default(),
            scroller: iced::widget::scrollable::Scroller {
                background: color.into(),
                border: iced::Border { radius: 2.0.into(), ..Default::default() },
            },
        },
        horizontal_rail: iced::widget::scrollable::Rail {
            background: None,
            border: iced::Border::default(),
            scroller: iced::widget::scrollable::Scroller {
                background: Color::TRANSPARENT.into(),
                border: iced::Border::default(),
            },
        },
        gap: None,
        auto_scroll: iced::widget::scrollable::AutoScroll {
            background: Color::TRANSPARENT.into(),
            border: iced::Border::default(),
            shadow: iced::Shadow::default(),
            icon: Color::TRANSPARENT,
        },
    }
}
