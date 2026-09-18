use super::*;

// ==========================================
// 2. Funciones Libres y Componentes UI Globales
// ==========================================

/// Crea un botón transparente que muestra un ícono SVG y emite un mensaje al presionarse.
/// Útil para la barra superior, cabeceras, o cualquier elemento "sin bordes".
pub fn action_icon_button<'a, Message: Clone + 'a>(
    icon_filename: &str,
    size: u32,
    action: Message,
) -> Element<'a, Message> {
    let content = svg(svg::Handle::from_path(format!(
        "assets/icons/{}",
        icon_filename
    )))
    .width(size)
    .height(size);

    mouse_area(content)
        .on_press(action)
        .interaction(iced::mouse::Interaction::Pointer)
        .into()
}

/// Botón icono SVG reutilizable con hover y estado deshabilitado.
/// - action: Some(msg) = botón activo con on_press. None = deshabilitado, sin interacción.
pub fn icon_button<'a, Message: Clone + 'a>(
    icon_filename: &str,
    size: u32,
    action: Option<Message>,
) -> Element<'a, Message> {
    let has_action = action.is_some();
    let btn_size = (size + 4).max(20);
    let icon = svg(svg::Handle::from_path(format!(
        "assets/icons/{}",
        icon_filename
    )))
    .width(iced::Length::Fill)
    .height(iced::Length::Fill)
    .style(|_t: &Theme, status| svg::Style {
        color: Some(match status {
            svg::Status::Hovered => COLOR_TEXT_PRIMARY,
            _ => COLOR_TEXT_SECONDARY,
        }),
    });

    let btn = button(icon)
        .width(btn_size)
        .height(btn_size)
        .padding(0)
        .style(move |_t: &Theme, status| {
            let is_hovered = matches!(status, button::Status::Hovered);
            button::Style {
                background: if has_action && is_hovered {
                    Some(COLOR_ACCENT.into())
                } else {
                    Some(Color::TRANSPARENT.into())
                },
                text_color: COLOR_TEXT_PRIMARY,
                border: iced::Border {
                    radius: 4.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..Default::default()
            }
        });

    if let Some(msg) = action {
        btn.on_press(msg).into()
    } else {
        btn.into()
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
        .padding([4, 10]) // Padding vertical mínimo para que tenga 24px de alto con fuente de 14px
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
