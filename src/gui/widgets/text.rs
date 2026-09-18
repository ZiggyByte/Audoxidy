use super::*;

// ==========================================
// 2. Elementos de Interfaz Auxiliares
// ==========================================

/// ======================================================================================
/// SISTEMA DE TRUNCAMIENTO DE TEXTO INTELIGENTE (RESPONSIVE)
/// ======================================================================================
///
/// Estas funciones proporcionan un widget de texto que se adapta dinámicamente al ancho real
/// de su contenedor, truncando con elipsis (...) cuando el espacio es insuficiente.
///
/// El sistema utiliza el componente `Responsive` de Iced para detectar el tamaño en tiempo real,
/// eliminando la necesidad de conteos de caracteres fijos que fallan al cambiar de fuente o ventana.

/// [NIVEL 1] Versión simplificada para el 90% de los casos.
/// - Sin saltos de línea (Wrapping::None)
/// - Sin sufijo adicional
/// - Alineación: Izquierda (Horizontal) y Centro (Vertical) por defecto.
///
/// Ejemplo: smart_truncate_text(song_title, 14.0, FONT, COLOR)
pub fn smart_truncate_text<'a, Message: Clone + 'a>(
    content: impl Into<String>,
    font_size: f32,
    font: iced::Font,
    color: Color,
) -> Element<'a, Message> {
    smart_truncate_text_advanced(
        content.into(),
        font_size,
        font,
        color,
        iced::widget::text::Wrapping::None,
        None,
        Alignment::Start,
        Alignment::Center,
    )
}

/// [NIVEL 2] Versión con soporte para sufijo pegado al texto.
/// - Ideal para cabeceras que llevan un punto indicador o icono pegado al nombre.
/// - El sufijo siempre permanecerá junto al texto, cortando el texto si es necesario.
/// - Sin saltos de línea (Wrapping::None)
/// - Alineación: Izquierda (Horizontal) y Centro (Vertical) por defecto.
///
/// Ejemplo: smart_truncate_text_with_suffix(name, 15.0, FONT, COLOR, (" •".to_string(), DOT_COLOR))
pub fn smart_truncate_text_with_suffix<'a, Message: Clone + 'a>(
    content: String,
    size: f32,
    font: iced::Font,
    color: Color,
    suffix: (String, Color),
) -> Element<'a, Message> {
    smart_truncate_text_advanced(
        content,
        size,
        font,
        color,
        iced::widget::text::Wrapping::None,
        Some(suffix),
        Alignment::Start,
        Alignment::Center,
    )
}

/// [NIVEL 3] Versión avanzada para control total.
/// Permite definir manualmente el comportamiento de wrapping, sufijos y alineación.
///
/// Alineaciones (Usando el enum `Alignment` de Iced):
/// - `align_x`: Alineación HORIZONTAL (Start = Izquierda, Center = Centro, End = Derecha).
/// - `align_y`: Alineación VERTICAL (Start = Arriba, Center = Centro, End = Abajo).
pub fn smart_truncate_text_advanced<'a, Message: Clone + 'a>(
    content: String,
    size: f32,
    font: iced::Font,
    color: Color,
    wrapping: iced::widget::text::Wrapping,
    suffix: Option<(String, Color)>,
    align_x: Alignment,
    align_y: Alignment,
) -> Element<'a, Message> {
    Responsive::new(move |size_info| {
        // Estimación: ~0.55 el tamaño de la fuente por carácter.
        let char_w = size * 0.55;
        let suffix_s = suffix.as_ref().map(|(s, _)| s.clone()).unwrap_or_default();
        let suffix_c = suffix.as_ref().map(|(_, c)| *c).unwrap_or(color);

        // Calculamos ancho de sufijo (incluyendo un margen de 2px)
        let suffix_w = if suffix_s.is_empty() {
            0.0
        } else {
            (suffix_s.chars().count() as f32) * char_w + 2.0
        };

        let available_w = size_info.width;
        let text_w_needed = (content.chars().count() as f32) * char_w;

        // Decidimos si truncar basándonos en el espacio real asignado por Iced
        let display_text = if (text_w_needed + suffix_w) <= available_w {
            content.clone()
        } else {
            let limit_w = (available_w - suffix_w).max(0.0);
            let max_chars = (limit_w / char_w).floor() as usize;
            truncate_text(&content, max_chars)
        };

        let mut r = row![
            text(display_text)
                .size(size)
                .font(font)
                .color(color)
                .wrapping(wrapping)
                .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(
                    size + 2.0
                )))
        ]
        .align_y(align_y)
        .spacing(0);

        if !suffix_s.is_empty() {
            r = r.push(
                text(suffix_s)
                    .size(size)
                    .font(font)
                    .color(suffix_c)
                    .wrapping(wrapping)
                    .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(
                        size + 2.0,
                    ))),
            );
        }

        container(r)
            .width(Length::Fill)
            .height(Length::Shrink)
            .align_x(align_x)
            .align_y(align_y)
            .into()
    })
    .width(Length::Fill)
    .height(Length::Shrink)
    .into()
}

/// Aplica un efecto de rotación tipo "Marquesina" sobre una cadena de texto dada una longitud límite.
/// Devuelve el string recortado o rotado en función de un "tick" temporal global.
pub fn apply_marquee(text_str: &str, limit: usize, tick: u64) -> String {
    // Optimización rápida: si es corto, no procesar nada
    if text_str.len() <= limit && text_str.is_ascii() {
        return text_str.to_string();
    }

    let chars: Vec<char> = text_str.chars().collect();
    if chars.len() <= limit {
        return text_str.to_string();
    }

    // Suavizamos el movimiento dividiendo el tick
    let offset = (tick / 3) as usize % (chars.len() + 10);
    if offset < chars.len() {
        let end = (offset + limit).min(chars.len());
        let mut s: String = chars[offset..end].iter().collect();
        // Si llegamos al final, mostramos la separación invisible y empalmamos el inicio
        if offset + limit > chars.len() {
            s.push_str("   ");
            let needed = (offset + limit) - chars.len();
            if needed > 3 {
                let rem = needed - 3;
                s.push_str(&chars[0..rem.min(chars.len())].iter().collect::<String>());
            }
        }
        s
    } else {
        chars[0..limit.min(chars.len())].iter().collect()
    }
}
