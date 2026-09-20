use super::*;

use crate::gui::dimensions::{IconSize, RADIUS_S, SPACE_MD, SPACE_XS, icon_button_box};

// ==========================================
// 2. Funciones Libres y Componentes UI Globales
// ==========================================

/// Interaction state shared by every icon button.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconState {
    /// Resting state: the icon is drawn in the secondary text color.
    Default,
    /// Toggled on: the icon is drawn in the accent color.
    Active,
    /// Non-interactive state: secondary icon, no press and no hover feedback.
    Disabled,
}

/// Visual variant shared by every icon button.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconVariant {
    /// Reacts to hover with an accent background and a primary icon.
    Default,
    /// Never reacts to hover; used for window controls.
    Borderless,
}

/// Resolves the `(icon color, background color)` pair of an icon button.
///
/// `Disabled` and `Borderless` never react to hover; an actionable `Default`
/// button turns its background into the accent and its icon into the primary
/// text color.
pub fn icon_palette(state: IconState, variant: IconVariant, is_hovered: bool) -> (Color, Color) {
    let base_color = match state {
        IconState::Active => COLOR_ACCENT,
        IconState::Default | IconState::Disabled => COLOR_TEXT_SECONDARY,
    };

    let is_hovering = is_hovered && variant == IconVariant::Default && state != IconState::Disabled;

    if is_hovering {
        (COLOR_TEXT_PRIMARY, COLOR_ACCENT)
    } else {
        (base_color, Color::TRANSPARENT)
    }
}

/// Botón transparente que muestra un ícono SVG y emite un mensaje al presionarse.
/// Variante sin borde ni realce al pasar el puntero; delega en `icon_button`.
pub fn action_icon_button<'a, Message: Clone + 'a>(
    icon_filename: &str,
    size: IconSize,
    action: Message,
) -> Element<'a, Message> {
    icon_button(
        icon_filename,
        size,
        IconState::Default,
        Some(action),
        IconVariant::Borderless,
        None,
    )
}

/// Botón de ícono SVG reutilizable con estados compartidos por toda la interfaz.
///
/// - `state`: descanso (`Default`), activado (`Active`) o deshabilitado (`Disabled`).
/// - `action`: `Some(msg)` habilita el botón; `None` o `Disabled` lo dejan sin interacción.
/// - `variant`: `Default` realza al pasar el puntero, `Borderless` no.
/// - `label`: nombre accesible opcional; cuando existe y el botón es accionable se
///   envuelve en un tooltip.
pub fn icon_button<'a, Message: Clone + 'a>(
    icon_filename: &str,
    size: IconSize,
    state: IconState,
    action: Option<Message>,
    variant: IconVariant,
    label: Option<&'static str>,
) -> Element<'a, Message> {
    let is_actionable = action.is_some() && state != IconState::Disabled;

    let icon = svg(svg::Handle::from_path(format!(
        "assets/icons/{}",
        icon_filename
    )))
    .width(iced::Length::Fill)
    .height(iced::Length::Fill)
    .style(move |_t: &Theme, status| {
        let is_hovered = matches!(status, svg::Status::Hovered);
        let (icon_color, _) = icon_palette(state, variant, is_hovered);
        svg::Style {
            color: Some(icon_color),
        }
    });

    let btn = button(icon)
        .width(icon_button_box(size))
        .height(icon_button_box(size))
        .padding(0)
        .style(move |_t: &Theme, status| {
            let is_hovered = matches!(status, button::Status::Hovered);
            let (_, background) = icon_palette(state, variant, is_hovered);
            button::Style {
                background: Some(iced::Background::Color(if is_actionable {
                    background
                } else {
                    Color::TRANSPARENT
                })),
                text_color: COLOR_TEXT_PRIMARY,
                border: iced::Border {
                    radius: RADIUS_S.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..Default::default()
            }
        });

    let element: Element<'a, Message> = match action {
        Some(msg) if is_actionable => btn.on_press(msg).into(),
        _ => btn.into(),
    };

    match label {
        Some(name) if is_actionable => iced::widget::tooltip(
            element,
            iced::widget::text(name),
            iced::widget::tooltip::Position::FollowCursor,
        )
        .into(),
        _ => element,
    }
}

/// Interruptor (Toggler) global de diseño premium personalizado.
/// - Permite personalizar el tamaño, colores de fondo y círculo para estados activo e inactivo.
pub fn standard_toggler<'a, Message>(
    is_active: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
    size: f32,
    active_color: Color,
    inactive_color: Color,
    thumb_active_color: Color,
    thumb_inactive_color: Color,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    standard_toggler_full(
        is_active,
        on_toggle,
        size,
        active_color,
        inactive_color,
        thumb_active_color,
        thumb_inactive_color,
        Color::TRANSPARENT,
    )
}

/// Toggler con control total de colores, incluyendo fondo del estado inactivo.
/// - `inactive_bg_color`: Color de fondo cuando está desactivado (por defecto TRANSPARENT).
pub fn standard_toggler_full<'a, Message>(
    is_active: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
    size: f32,
    active_color: Color,
    inactive_color: Color,
    thumb_active_color: Color,
    thumb_inactive_color: Color,
    inactive_bg_color: Color,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    toggler(is_active)
        .size(size)
        .on_toggle(on_toggle)
        .style(move |_theme: &Theme, _status| {
            let (bg, border_color, border_width, fg) = if is_active {
                (
                    active_color.into(),
                    active_color,
                    1.0,
                    thumb_active_color.into(),
                )
            } else {
                (
                    inactive_bg_color.into(),
                    inactive_color,
                    1.0,
                    thumb_inactive_color.into(),
                )
            };

            iced::widget::toggler::Style {
                background: bg,
                background_border_width: border_width,
                background_border_color: border_color,
                foreground: fg,
                foreground_border_width: 0.0,
                foreground_border_color: Color::TRANSPARENT,
                text_color: None,
                border_radius: None,
                padding_ratio: 0.2,
            }
        })
        .into()
}

/// Botón de radio (Radio Button) global de diseño premium personalizado.
/// - Desactivados: Borde en COLOR_TEXT_PRIMARY y fondo transparente.
/// - Activados: Borde en COLOR_ACCENT y el círculo (dot) en COLOR_TEXT_SECONDARY.
pub fn standard_radio<'a, Message, V>(
    label: impl Into<String>,
    value: V,
    selected: Option<V>,
    on_click: impl Fn(V) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
    V: Copy + Eq + 'a,
{
    let is_selected = selected == Some(value);

    radio(label, value, selected, on_click)
        .size(16)
        .text_size(14)
        .font(FONT_INTER_SANS_MEDIUM)
        .style(move |_theme: &Theme, _status| {
            let (border_color, dot_color) = if is_selected {
                (COLOR_ACCENT, COLOR_ACCENT)
            } else {
                (COLOR_TEXT_PRIMARY, Color::TRANSPARENT)
            };

            iced::widget::radio::Style {
                background: Color::TRANSPARENT.into(),
                dot_color,
                border_width: 1.3,
                border_color,
                text_color: Some(COLOR_TEXT_PRIMARY),
            }
        })
        .into()
}

/// Desplegable (PickList) global de diseño premium personalizado.
/// - 24px de alto.
/// - Fondo en COLOR_CONTRAST.
/// - Tipografía de 14px y fuente Inter Medium.
/// - Sin bordes.
/// - Hover/Focus en COLOR_ACCENT con texto COLOR_TEXT_PRIMARY.
pub fn standard_pick_list<'a, T, Message>(
    options: Vec<T>,
    selected: Option<T>,
    on_selected: impl Fn(T) -> Message + 'a,
    width: Length,
) -> Element<'a, Message>
where
    T: Clone + PartialEq + std::fmt::Display + 'a,
    Message: Clone + 'a,
{
    let pick = pick_list(options, selected, on_selected)
        .width(width)
        .padding([SPACE_XS, SPACE_MD]) // Vertical padding that yields a 24 px control with a 14 px font
        .font(FONT_INTER_SANS_MEDIUM)
        .text_size(14)
        .style(|_theme: &Theme, status| {
            let is_active_hover = match status {
                iced::widget::pick_list::Status::Hovered
                | iced::widget::pick_list::Status::Opened { .. } => true,
                _ => false,
            };

            let bg_color = if is_active_hover {
                COLOR_ACCENT
            } else {
                COLOR_CONTRAST
            };

            let txt_color = if is_active_hover {
                COLOR_TEXT_PRIMARY
            } else {
                COLOR_TEXT_PRIMARY
            };

            iced::widget::pick_list::Style {
                text_color: txt_color,
                placeholder_color: COLOR_TEXT_PRIMARY,
                handle_color: txt_color,
                background: bg_color.into(),
                border: iced::Border {
                    width: 0.0,
                    color: Color::TRANSPARENT,
                    radius: 0.0.into(),
                },
            }
        })
        .menu_style(|_theme: &Theme| iced::overlay::menu::Style {
            background: COLOR_CONTRAST.into(),
            border: iced::Border {
                width: 0.0,
                color: Color::TRANSPARENT,
                radius: 0.0.into(),
            },
            text_color: COLOR_TEXT_PRIMARY,
            selected_text_color: COLOR_TEXT_PRIMARY,
            selected_background: COLOR_ACCENT.into(),
            shadow: iced::Shadow::default(),
        });

    pick.into()
}

// ==========================================
// 6. Lógica de Virtualización y Scroll
// ==========================================

/// Botón circular pequeño para cheurones (flechas) de expansión.
pub fn chevron_btn<'a, Message: Clone + 'a>(
    icon_filename: &str,
    action: Message,
    btn_size: f32,
    icon_size: f32,
) -> Element<'a, Message> {
    button(
        container(
            svg(svg::Handle::from_path(format!(
                "assets/icons/{}",
                icon_filename
            )))
            .width(icon_size)
            .height(icon_size)
            .style(move |_t: &Theme, _s: svg::Status| svg::Style {
                color: Some(COLOR_TEXT_SECONDARY),
            }),
        )
        .width(btn_size)
        .height(btn_size)
        .center_x(btn_size)
        .center_y(btn_size),
    )
    .on_press(action)
    .padding(0)
    .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
    .into()
}

/// Botón chevron con el tamaño de caja y de ícono tomados de un token.
///
/// Delega en `chevron_btn`; los llamadores de tamaño fijo usan este punto de
/// entrada para no repetir el número en el sitio de llamada.
pub fn chevron_btn_sized<'a, Message: Clone + 'a>(
    icon_filename: &str,
    action: Message,
    size: IconSize,
) -> Element<'a, Message> {
    chevron_btn(icon_filename, action, size.px(), size.px())
}

#[cfg(test)]
mod tests {
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
}
