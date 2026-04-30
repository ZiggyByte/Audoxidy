use iced::{
    widget::{button, column, container, mouse_area, row, scrollable, svg, text, text_input, Space, Responsive},
    Alignment, Color, Element, Length, Theme, Padding,
};
use iced::advanced::{layout, mouse, overlay, renderer, widget::{Operation, Tree}, Clipboard, Layout, Shell, Widget};
use iced::{Event, Rectangle, Size, Vector};

use crate::gui::theme::{COLOR_TEXT_PRIMARY, COLOR_CONTRAST, COLOR_TEXT_SECONDARY, COLOR_ACCENT, COLOR_BG, FONT_INTER_SANS_NORMAL,FONT_INTER_SANS_MEDIUM};
use crate::utils::{truncate_text, SortColumn, format_duration, format_metadata};
use std::collections::HashMap;

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
    content: String,
    size: f32,
    font: iced::Font,
    color: Color,
) -> Element<'a, Message> {
    smart_truncate_text_advanced(
        content, size, font, color, 
        iced::widget::text::Wrapping::None, 
        None,
        Alignment::Start,
        Alignment::Center
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
        content, size, font, color, 
        iced::widget::text::Wrapping::None, 
        Some(suffix),
        Alignment::Start,
        Alignment::Center
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
        let suffix_w = if suffix_s.is_empty() { 0.0 } else { (suffix_s.chars().count() as f32) * char_w + 2.0 };
        
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
                .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(size + 2.0)))
        ]
        .align_y(align_y)
        .spacing(0);

        if !suffix_s.is_empty() {
            r = r.push(text(suffix_s).size(size).font(font).color(suffix_c).wrapping(wrapping)
                .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(size + 2.0))));
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

// ==========================================
// 3. Widgets de Cabecera y Filas
// ==========================================

/// Envuelve un contenido y captura específicamente la rueda del scroll del mouse
/// devolviendo un valor de tipo delta que puede transformarse al Mensaje deseado.
pub struct VolumeScrollArea<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    on_scroll: Box<dyn Fn(f32) -> Message + 'a>,
}

impl<'a, Message, Theme, Renderer> VolumeScrollArea<'a, Message, Theme, Renderer> {
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        on_scroll: impl Fn(f32) -> Message + 'a,
    ) -> Self {
        Self {
            content: content.into(),
            on_scroll: Box::new(on_scroll),
        }
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for VolumeScrollArea<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport)
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content))
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content.as_widget_mut().operate(&mut tree.children[0], layout, renderer, operation)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
            if cursor.is_over(layout.bounds()) {
                let d = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 10.0,
                };
                shell.publish((self.on_scroll)(d));
                return;
            }
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        )
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}

impl<'a, Message, Theme, Renderer> From<VolumeScrollArea<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(area: VolumeScrollArea<'a, Message, Theme, Renderer>) -> Self {
        Element::new(area)
    }
}

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
    let content = svg(svg::Handle::from_path(format!("assets/icons/{}", icon_filename)))
        .width(size)
        .height(size);

    mouse_area(content)
        .on_press(action)
        .interaction(iced::mouse::Interaction::Pointer)
        .into()
}

/// Estilos de Placeholder para el widget de carátulas
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaceholderStyle {
    /// Versión grande (Grid, Lista Detallada): Icono 96px + Texto "AuDoxiDY"
    Large,
    /// Versión pequeña (Thumbnail List): Solo icono
    Small,
    /// Versión específica del reproductor (Solo texto)
    Player,
}

/// Widget global para mostrar carátulas de álbumes de forma eficiente y consistente.
/// Soporta: Caché AVIF (Prioridad 1) -> Datos binarios crudos (Prioridad 2) -> Placeholder Automático.
pub fn album_art_widget<'a, Message: 'a>(
    avif_path: Option<&String>,
    raw_data: Option<&Vec<u8>>,
    preloaded_handle: Option<iced::widget::image::Handle>,
    style: PlaceholderStyle,
    bounds: Length,
    radius: f32,
) -> Element<'a, Message> {
    // 1. Usar handle precargado si existe (Prioridad 0 - Máximo rendimiento)
    let mut handle_opt = preloaded_handle;

    // 2. Intentar cargar desde archivo AVIF en disco (Prioridad 1) — sin caché RAM
    if handle_opt.is_none() {
        handle_opt = avif_path.and_then(|p| crate::utils::covers::load_cover_handle(p));
    }
    
    // 3. Fallback: Cargar desde bytes crudos (Prioridad 2)
    if handle_opt.is_none() {
        if let Some(data) = raw_data {
            handle_opt = crate::utils::covers::load_raw_image_for_iced(data);
        }
    }

    if let Some(handle) = handle_opt {
        container(
            iced::widget::image(handle)
                .width(bounds)
                .height(bounds)
                .content_fit(iced::ContentFit::Cover)
                .border_radius(radius)
        )
        .width(bounds)
        .height(bounds)
        .style(move |_t: &Theme| {
            container::Style::default()
                .border(iced::Border { radius: radius.into(), ..Default::default() })
        })
        .clip(true)
        .into()
    } else {
        // 4. Fallback: Placeholder Automático basado en el estilo solicitado
        match style {
            PlaceholderStyle::Large => {
                container(
                    column![
                        svg(svg::Handle::from_path("assets/icons/album.svg"))
                            .width(Length::Fixed(96.0))
                            .height(Length::Fixed(96.0))
                            .style(|_t: &Theme, _s| svg::Style { color: Some(Color::from(COLOR_TEXT_SECONDARY)) }),
                        text("AuDoxiDY")
                            .font(crate::gui::theme::FONT_STAGE_WANDER)
                            .size(11)
                            .color(COLOR_TEXT_SECONDARY)
                    ]
                    .align_x(Alignment::Center)
                    .spacing(5)
                )
                .width(bounds)
                .height(bounds)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_t: &Theme| {
                    container::Style::default()
                        .background(COLOR_BG)
                        .border(iced::Border { radius: radius.into(), ..Default::default() })
                })
                .into()
            }
            PlaceholderStyle::Small => {
                container(
                    svg(svg::Handle::from_path("assets/icons/album.svg"))
                        .width(Length::Fixed(20.0))
                        .height(Length::Fixed(20.0))
                        .style(|_t: &Theme, _s| svg::Style { color: Some(Color::from(COLOR_TEXT_SECONDARY)) })
                )
                .width(bounds)
                .height(bounds)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_t: &Theme| {
                    container::Style::default()
                        .background(COLOR_BG)
                        .border(iced::Border { radius: radius.into(), ..Default::default() })
                })
                .into()
            }
            PlaceholderStyle::Player => {
                container(
                    text("AuDoxiDY")
                        .font(crate::gui::theme::FONT_STAGE_WANDER)
                        .size(40)
                        .color(COLOR_TEXT_PRIMARY)
                )
                .width(bounds)
                .height(bounds)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .padding(20)
                .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
                .into()
            }
        }
    }
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
                Color::from(COLOR_TEXT_SECONDARY)
            }
        }
        iced::widget::scrollable::Status::Dragged { .. } => Color::from(COLOR_ACCENT),
        _ => Color::TRANSPARENT,
    };
    
    iced::widget::scrollable::Style {
        container: iced::widget::container::Style::default(),
        vertical_rail: iced::widget::scrollable::Rail {
            background: None,
            border: iced::Border::default(),
            scroller: iced::widget::scrollable::Scroller {
                background: if color != Color::TRANSPARENT {
                    iced::Background::Gradient(iced::Gradient::Linear(iced::gradient::Linear {
                        angle: 1.5707964.into(), // 90 grados (Horizontal)
                        stops: [
                            Some(iced::gradient::ColorStop { offset: 0.49, color: Color::TRANSPARENT }),
                            Some(iced::gradient::ColorStop { offset: 0.5, color: color }),
                            Some(iced::gradient::ColorStop { offset: 1.0, color: color }),
                            None, None, None, None, None
                        ],
                    }))
                } else {
                    Color::TRANSPARENT.into()
                },
                border: iced::Border { 
                    radius: 0.0.into(), 
                    width: 0.0, 
                    color: Color::TRANSPARENT 
                },
            },
        },
        horizontal_rail: iced::widget::scrollable::Rail {
            background: None,
            border: iced::Border::default(),
            scroller: iced::widget::scrollable::Scroller {
                background: if color != Color::TRANSPARENT {
                    iced::Background::Gradient(iced::Gradient::Linear(iced::gradient::Linear {
                        angle: 3.1415927.into(), // 180 grados (Vertical)
                        stops: [
                            Some(iced::gradient::ColorStop { offset: 0.49, color: Color::TRANSPARENT }),
                            Some(iced::gradient::ColorStop { offset: 0.5, color: color }),
                            Some(iced::gradient::ColorStop { offset: 1.0, color: color }),
                            None, None, None, None, None
                        ],
                    }))
                } else {
                    Color::TRANSPARENT.into()
                },
                border: iced::Border { 
                    radius: 0.0.into(), 
                    width: 0.0, 
                    color: Color::TRANSPARENT 
                },
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

/// Widget scrollable estándar con barra de desplazamiento de 12px (4px visibles + 4px de margen interactivo transparente a cada lado).
pub fn standard_scrollable<'a, Message: 'a>(
    id: iced::widget::Id,
    content: impl Into<Element<'a, Message>>,
    direction: iced::widget::scrollable::Direction,
) -> iced::widget::scrollable::Scrollable<'a, Message> {
    scrollable(content)
        .id(id)
        .direction(direction)
        .style(custom_scrollbar_style)
}

/// Configuración estándar de la barra de desplazamiento para uso global.
pub fn standard_scrollbar() -> iced::widget::scrollable::Scrollbar {
    iced::widget::scrollable::Scrollbar::new()
        .width(8)
        .scroller_width(8)
        .margin(0)
}

/// Aplica un efecto de rotación tipo "Marquesina" sobre una cadena de texto dada una longitud límite.
/// Devuelve el string recortado o rotado en función de un "tick" temporal global.
pub fn apply_marquee(text_str: &str, limit: usize, tick: u64) -> String {
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

/// Construye una barra de ordenamiento interactiva, reciclable para cualquier tabla.
pub fn build_sort_bar<'a, Message: Clone + 'a>(
    columns: &[SortColumn],
    current_sort: Option<SortColumn>,
    sort_ascending: Option<bool>,
    column_widths: &HashMap<SortColumn, u16>,
    resizing_column: Option<SortColumn>,
    hovered_column: Option<SortColumn>,
    left_padding: f32,
    on_hover: impl Fn(Option<SortColumn>) -> Message + 'a,
    on_resize: impl Fn(SortColumn) -> Message + 'a,
    on_sort: impl Fn(SortColumn) -> Message + 'a,
) -> Element<'a, Message> {

    let mut sort_bar_content = row![].align_y(Alignment::Center).height(Length::Fill).padding(iced::Padding { top: 0.0, right: 5.0, bottom: 0.0, left: left_padding });

    for &sort in columns {
        let is_active = current_sort == Some(sort);
        let width = *column_widths.get(&sort).unwrap_or(&100) as f32;
        
        let available_w = width - 15.0; // Espacio reservado para icono/separador
        let max_chars = (available_w / 7.0).max(1.0) as usize;
        let t_str = truncate_text(sort.as_str(), max_chars);
        
        let t = text(t_str)
            .size(12)
            .font(FONT_INTER_SANS_MEDIUM)
            .color(COLOR_TEXT_SECONDARY)
            .wrapping(iced::widget::text::Wrapping::None);

        let icon_el = if is_active {
             let handle = match sort_ascending {
                 Some(true) => Some(svg::Handle::from_path("assets/icons/arrow-up-chevron.svg")),
                 Some(false) => Some(svg::Handle::from_path("assets/icons/arrow-down-chevron.svg")),
                 _ => None,
             };
             
             if let Some(h) = handle {
                 Some(svg(h)
                     .width(20)
                     .height(20)
                     .style(move |_t: &Theme, _s: svg::Status| svg::Style { color: Some(COLOR_TEXT_SECONDARY) }))
             } else {
                 None
             }
        } else { None };
        let is_hovered = resizing_column == Some(sort) || hovered_column == Some(sort);
        
        let separator_visual = container(Space::new())
            .width(Length::Fixed(9.0))
            .height(Length::Fixed(16.0))
            .style(move |_t: &Theme| {
                let bg_color = if is_hovered { COLOR_ACCENT } else { COLOR_TEXT_SECONDARY };
                container::Style::default()
                    .background(bg_color)
                    .border(iced::Border { 
                        radius: 4.0.into(), 
                        width: 4.0, 
                        color: Color::TRANSPARENT
                    })
            });

        let on_h = on_hover(Some(sort));
        let on_h_exit = on_hover(None);
        let separator_area: Element<Message> = if sort == SortColumn::AlbumCard || sort == SortColumn::AlbumThumbnail {
            Space::new().width(4.0).into()
        } else {
            mouse_area(separator_visual)
                .on_enter(on_h)
                .on_exit(on_h_exit)
                .on_press(on_resize(sort))
                .interaction(iced::mouse::Interaction::ResizingHorizontally)
                .into()
        };

        let sort_btn_content = if sort == SortColumn::TrackNumber {
            if let Some(ic) = icon_el {
                row![Space::new().width(Length::Fill), t, ic, Space::new().width(3.0)].align_y(Alignment::Center)
            } else {
                row![Space::new().width(Length::Fill), t, Space::new().width(3.0)].align_y(Alignment::Center)
            }
        } else {
            if let Some(ic) = icon_el {
                row![t, Space::new().width(Length::Fill), ic].align_y(Alignment::Center)
            } else {
                row![t, Space::new().width(Length::Fill)].align_y(Alignment::Center)
            }
        };

        let sort_btn = if sort == SortColumn::AlbumCard || sort == SortColumn::AlbumThumbnail {
            button(sort_btn_content)
                .width(Length::Fill)
                .padding(iced::Padding { left: 5.0, right: 0.0, top: 0.0, bottom: 0.0 })
                .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
        } else {
            button(sort_btn_content)
                .width(Length::Fill)
                .padding(iced::Padding { left: 5.0, right: 0.0, top: 0.0, bottom: 0.0 })
                .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
                .on_press(on_sort(sort))
        };

        let content = row![
            sort_btn,
            Space::new().width(3.0),
            separator_area,
            Space::new().width(3.0)
        ].align_y(Alignment::Center).width(Length::Fixed(width));
            
        let col_container = container(content)
            .width(Length::Fixed(width))
            .clip(true);
            
        sort_bar_content = sort_bar_content.push(col_container);
    }

    column![
        container(
            scrollable(sort_bar_content)
                .direction(iced::widget::scrollable::Direction::Horizontal(
                    iced::widget::scrollable::Scrollbar::new().width(0).scroller_width(0)
                ))
        )
            .width(Length::Fill)
            .height(Length::Fixed(28.0))
            .style(|_t: &Theme| container::Style::default().background(COLOR_BG)),
        container(Space::new().width(Length::Fill).height(2.0))
            .style(|_t: &Theme| container::Style::default().background(Color::from(COLOR_CONTRAST)))
    ].into()
}

// ==========================================
// 4. Widgets de Entrada y Búsqueda
// ==========================================

/// Widget de búsqueda estandarizado para toda la aplicación.
pub fn standard_search_input<'a, Message: Clone + 'a>(
    placeholder: &'a str,
    value: &'a str,
    on_change: impl Fn(String) -> Message + 'a,
    on_clear: Message,
    width: Length,
) -> Element<'a, Message> {
    let has_content = !value.is_empty();

    let input = text_input(placeholder, value)
        .on_input(on_change)
        .padding(iced::Padding { right: 25.0, ..Default::default() }) // Espacio a la derecha para la 'x'
        .size(14)
        .font(FONT_INTER_SANS_MEDIUM)
        .width(Length::Fill)
        .style(move |_t: &Theme, status: iced::widget::text_input::Status| {
            let is_focused = matches!(status, iced::widget::text_input::Status::Focused { .. });
            
            let (bg, txt) = if is_focused {
                (COLOR_CONTRAST, COLOR_TEXT_PRIMARY)
            } else {
                (COLOR_CONTRAST, COLOR_TEXT_SECONDARY)
            };

            iced::widget::text_input::Style {
                background: bg.into(),
                border: iced::Border { 
                    radius: 0.0.into(), 
                    width: 0.0, 
                    color: Color::TRANSPARENT 
                },
                icon: COLOR_TEXT_SECONDARY,
                placeholder: COLOR_TEXT_SECONDARY,
                value: txt,
                selection: COLOR_ACCENT,
            }
        });

    let mut content = iced::widget::stack![input];

    if has_content {
        let clear_btn = button(
            container(text("x").size(14).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM))
                .width(Length::Fixed(20.0))
                .height(Length::Fixed(20.0))
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
        )
        .padding(0)
        .style(|_t, _s| button::Style::default().with_background(Color::TRANSPARENT))
        .on_press(on_clear);

        content = content.push(
            container(clear_btn)
                .width(Length::Fill)
                .height(Length::Fixed(24.0))
                .align_x(iced::alignment::Horizontal::Right)
                .padding(iced::Padding { right: 5.0, ..Default::default() })
        );
    }

    container(content)
        .width(width)
        .height(Length::Fixed(24.0))
        .center_y(Length::Fill)
        .into()
}

// ==========================================
// 5. Menús Contextuales
// ==========================================

/// Representa una entrada en un menú contextual.
#[derive(Debug, Clone)]
pub struct ContextMenuEntry<Message> {
    pub label: String,
    pub icon: Option<String>,    // Nombre del icono en assets/icons/
    pub action: Option<Message>, // None representa un divisor
}

/// Construye el contenido visual de un menú contextual basado en una lista de entradas.
pub fn build_context_menu_content<'a, Message: Clone + 'a>(
    entries: Vec<ContextMenuEntry<Message>>,
) -> Element<'a, Message> {
    let max_chars = entries.iter().map(|e| e.label.chars().count()).max().unwrap_or(0);
    let calculated_width = (max_chars as f32 * 7.5) + 52.0;
    let final_width = calculated_width.max(80.0);

    let mut content = column![].spacing(0).width(Length::Fixed(final_width));

    for entry in entries {
        if let Some(action) = entry.action {
            let label = entry.label.clone();
            let icon_name = entry.icon.clone();
            
            let is_rename = label == "Renombrar lista";
            let left_padding = if is_rename { 9.0 } else { 10.0 };
            let row_spacing = if is_rename { 16.0 } else { 15.0 };

            let btn = button(
                container(
                    row![
                        // Icono
                        if let Some(icon) = icon_name {
                            Element::from(
                                container(
                                    svg(svg::Handle::from_path(format!("assets/icons/{}", icon)))
                                        .width(Length::Fixed(24.0))
                                        .height(Length::Fixed(24.0))
                                        .style(|_t, _s| iced::widget::svg::Style {
                                            color: Some(COLOR_TEXT_PRIMARY),
                                        })
                                )
                                .width(Length::Fixed(24.0))
                            )
                        } else {
                            Element::from(Space::new().width(Length::Fixed(24.0)))
                        },
                        Space::new().width(Length::Fixed(row_spacing)),
                        // Texto
                        text(label)
                            .size(13)
                            .font(FONT_INTER_SANS_NORMAL)
                            .wrapping(iced::widget::text::Wrapping::None)
                    ]
                    .align_y(Alignment::Center)
                )
                .width(Length::Fill)
                .height(Length::Fixed(32.0))
                .padding(iced::Padding { left: left_padding, right: 0.0, ..Default::default() })
                .align_y(iced::alignment::Vertical::Center)
            )
            .on_press(action)
            .padding(0)
            .style(move |_t: &Theme, status: iced::widget::button::Status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                
                button::Style {
                    background: if is_hovered { Some(COLOR_CONTRAST.into()) } else { None },
                    text_color: COLOR_TEXT_PRIMARY,
                    border: iced::Border { radius: 0.0.into(), width: 0.0, color: Color::TRANSPARENT },
                    ..Default::default()
                }
            });

            content = content.push(btn);
        } else {
            // Divisor
            content = content.push(
                container(Space::new().height(Length::Fixed(1.0)))
                    .width(Length::Fill)
                    .padding(iced::Padding { top: 0.0, bottom: 0.0, ..Default::default() })
                    .style(|_t: &Theme| container::Style::default().background(COLOR_TEXT_SECONDARY))
            );
        }
    }

    container(
        column![
            content
        ].spacing(0)
    )
    .width(Length::Fixed(final_width))
        .style(|_t: &Theme| {
            container::Style::default()
                .background(COLOR_BG)
                .border(iced::Border {
                    color: COLOR_TEXT_SECONDARY,
                    width: 1.0,
                    radius: 8.0.into(),
                })
                .shadow(iced::Shadow {
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 10.0,
                    color: Color::from_rgba8(0, 0, 0, 0.5),
                })
        })
        .padding(iced::Padding { top: 5.0, bottom: 5.0, left: 1.0, right: 1.0 })
        .into()
}

/// Widget para diálogos modales (sub-ventanas).
pub fn standard_modal<'a, Message: Clone + 'a>(
    title: String,
    content: Element<'a, Message>,
    cancel_msg: Option<Message>,
    confirm_msg: Option<Message>,
    confirm_label: String,
) -> Element<'a, Message> {

    let mut footer = row![].spacing(10).padding(iced::Padding { top: 5.0,  ..Default::default() }).align_y(Alignment::Center);

    if let Some(cancel) = cancel_msg {
        footer = footer.push(
            button(text("Cancelar").size(14).font(FONT_INTER_SANS_MEDIUM))
                .padding([5, 10])
                .on_press(cancel)
                .style(|_t: &Theme, status: iced::widget::button::Status| {
                    let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                    button::Style {
                        background: if is_hovered { Some(COLOR_ACCENT.into()) } else { Some(COLOR_CONTRAST.into()) },
                        text_color: if is_hovered { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY },
                        border: iced::Border { radius: 6.0.into(), width: 0.0, color: Color::TRANSPARENT },
                        ..Default::default()
                    }
                })
        );
    }

    if let Some(confirm) = confirm_msg {
        footer = footer.push(
            button(text(confirm_label).size(14).font(FONT_INTER_SANS_MEDIUM))
                .padding([5, 10])
                .on_press(confirm)
                .style(|_t: &Theme, status: iced::widget::button::Status| {
                    let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                    button::Style {
                        background: if is_hovered { Some(COLOR_ACCENT.into()) } else { Some(COLOR_CONTRAST.into()) },
                        text_color: if is_hovered { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY },
                        border: iced::Border { radius: 6.0.into(), width: 0.0, color: Color::TRANSPARENT },
                        ..Default::default()
                    }
                })
        );
    }

    container(
        column![
            Space::new().width(Length::Fixed(200.0)).height(Length::Fixed(0.0)),
            text(title).size(16).font(FONT_INTER_SANS_MEDIUM).color(COLOR_TEXT_PRIMARY).align_x(iced::alignment::Horizontal::Center),
            container(content).padding([10, 0]).width(Length::Shrink),
            footer,
        ]
        .spacing(0)
        .align_x(Alignment::Center)
        .width(Length::Shrink)
    )
    .padding(15)
    .width(Length::Shrink)
    .style(|_t: &Theme| container::Style {
        background: Some(COLOR_BG.into()),
        border: iced::Border {
            color: COLOR_ACCENT,
            width: 2.0,
            radius: 8.0.into(),
        },
        shadow: iced::Shadow {
            offset: iced::Vector::new(0.0, 10.0),
            blur_radius: 30.0,
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.8),
        },
        ..container::Style::default()
    })
    .into()
}

/// Helper para texto dentro de modales con estilo global.
pub fn modal_text<'a, Message: Clone + 'a>(content: String) -> Element<'a, Message> {
    text(content)
        .size(14)
        .color(COLOR_TEXT_PRIMARY)
        .font(FONT_INTER_SANS_NORMAL)
        .align_x(iced::alignment::Horizontal::Center)
        .into()
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
            svg(svg::Handle::from_path(format!("assets/icons/{}", icon_filename)))
                .width(icon_size)
                .height(icon_size)
                .style(move |_t: &Theme, _s: svg::Status| svg::Style { color: Some(COLOR_TEXT_SECONDARY) })
        )
        .width(btn_size)
        .height(btn_size)
        .center_x(btn_size)
        .center_y(btn_size)
    )
    .on_press(action)
    .padding(0)
    .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
    .into()
}

/// Renderiza la cabecera de un grupo de artistas (Gris oscuro, 32px).
pub fn artist_header_widget<'a, Message: Clone + 'a>(
    name: String,
    is_collapsed: bool,
    is_selected: bool,
    is_playing: bool,
    albums_count: usize,
    songs_count: usize,
    duration_secs: f64,
    row_h: f32,       // Altura de la fila: 32px para SimpleList/DetailedList, 42px para ThumbnailList
    on_select: Message,
    on_toggle: Message,
) -> Element<'a, Message> {
    let chevron = if is_collapsed { "arrow-down-chevron.svg" } else { "arrow-up-chevron.svg" };
    let time_str = format_duration(duration_secs);
    
    let (name_color, dot_color, show_dot) = if is_selected {
        (COLOR_TEXT_PRIMARY, COLOR_TEXT_PRIMARY, true)
    } else if is_playing {
        (COLOR_ACCENT, COLOR_ACCENT, true)
    } else {
        (COLOR_TEXT_PRIMARY, COLOR_TEXT_PRIMARY, false)
    };

    let on_select_clone = on_select.clone();
    let on_toggle_clone = on_toggle.clone();

    let font_size = if row_h > 40.0 { 16.0 } else { 15.0 };

    let name_widget = if show_dot {
        smart_truncate_text_with_suffix(
            name,
            font_size,
            FONT_INTER_SANS_MEDIUM,
            name_color,
            (" •".to_string(), dot_color),
        )
    } else {
        smart_truncate_text(name, font_size, FONT_INTER_SANS_MEDIUM, name_color)
    };

    let header_content = row![
        name_widget,
        container(
            text(format!("{} Canciones | {} Álbumes | {}",  songs_count, albums_count, time_str))
                .size(14).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM)
                .wrapping(iced::widget::text::Wrapping::None)
        ).padding(Padding { left: 10.0, right: 0.0, top: 0.0, bottom: 0.0 }).center_y(Length::Fill),
        Space::new().width(15),
        chevron_btn(chevron, on_toggle_clone, row_h, row_h - 4.0),
    ].align_y(Alignment::Center).padding(Padding { left: 15.0, right: 6.0, top: 0.0, bottom: 0.0 })
    .height(Length::Fill);

    container(
        mouse_area(header_content)
            .on_press(on_select_clone)
    )
    .width(Length::Fill)
    .height(Length::Fixed(row_h))
    .center_y(Length::Fill)
    .style(|_t| container::Style::default().background(COLOR_CONTRAST))
    .into()
}

/// Renderiza la cabecera de un grupo de álbumes dentro de un artista.
pub fn album_header_widget<'a, Message: Clone + 'a>(
    album_name: String,
    songs_count: usize,
    duration_secs: f64,
    is_expanded: bool,
    is_selected: bool,
    is_playing: bool,
    row_h: f32,
    on_select: Message,
    on_toggle: Message,
) -> Element<'a, Message> {
    let chevron = if is_expanded { "arrow-up-chevron.svg" } else { "arrow-down-chevron.svg" };
    let time_str = format_duration(duration_secs);
    
    let (txt_color, dot_color, show_dot) = if is_selected {
        (COLOR_TEXT_PRIMARY, COLOR_TEXT_PRIMARY, true)
    } else if is_playing {
        (COLOR_ACCENT, COLOR_ACCENT, true)
    } else {
        (COLOR_TEXT_PRIMARY, COLOR_TEXT_PRIMARY, false)
    };

    let on_select_clone = on_select.clone();
    let on_toggle_clone = on_toggle.clone();

    let name_widget = if show_dot {
        smart_truncate_text_with_suffix(
            album_name,
            15.0,
            FONT_INTER_SANS_MEDIUM,
            txt_color,
            (" •".to_string(), dot_color),
        )
    } else {
        smart_truncate_text(album_name, 15.0, FONT_INTER_SANS_MEDIUM, txt_color)
    };

    let header_content = row![
        name_widget,
        container(
            text(format!("{} Canciones | {}", songs_count, time_str))
                .size(14).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM)
                .wrapping(iced::widget::text::Wrapping::None)
        ).padding(Padding { left: 10.0, right: 0.0, top: 0.0, bottom: 0.0 }).center_y(Length::Fill),
        Space::new().width(15),
        chevron_btn(chevron, on_toggle_clone, row_h, row_h - 4.0),
    ].align_y(Alignment::Center).padding(Padding { left: 15.0, right: 6.0, top: 0.0, bottom: 0.0 })
    .height(Length::Fill);

    container(
        mouse_area(header_content)
            .on_press(on_select_clone)
    )
    .width(Length::Fill)
    .height(Length::Fixed(row_h))
    .center_y(Length::Fill)
    .style(|_t| container::Style::default().background(COLOR_CONTRAST))
    .into()
}

/// Widget universal para renderizar una fila de canción en cualquier vista de lista.
/// Soporta: Diferentes alturas, miniaturas opcionales, resaltado de reproducción y selección.
pub fn universal_song_row_widget<'a, Message: Clone + 'a>(
    song: &crate::db::database::SongData,
    _song_idx: usize,
    is_selected: bool,
    columns: &[SortColumn],
    column_widths: &HashMap<SortColumn, u16>,
    on_select: Message,
    playing_path: &str,
    row_height: f32,
    show_thumbnail: bool,
) -> Element<'a, Message> {
    let txt_color = if is_selected { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };
    let is_playing = song.full_file_path == playing_path;

    let get_col = |col: SortColumn| -> Element<'a, Message> {
        let w = *column_widths.get(&col).unwrap_or(&100) as f32;
        let max_chars = ((w - 10.0) / 7.0).max(1.0) as usize;
        let val = format_metadata(song, &col);
        let truncated = truncate_text(&val, max_chars);

        let content: Element<'a, Message> = if col == SortColumn::TrackNumber {
            row![
                container(if is_playing { text("•").size(13).color(COLOR_ACCENT).font(FONT_INTER_SANS_MEDIUM).wrapping(iced::widget::text::Wrapping::None) } else { text("").size(13) })
                    .width(Length::Fixed(26.0)).align_x(iced::alignment::Horizontal::Center).align_y(iced::alignment::Vertical::Center),
                container(text(truncated).size(13).color(Color::from(txt_color)).font(FONT_INTER_SANS_MEDIUM).wrapping(iced::widget::text::Wrapping::None))
                    .width(Length::Fixed(26.0)).align_x(iced::alignment::Horizontal::Right).align_y(iced::alignment::Vertical::Center)
            ].spacing(0).align_y(Alignment::Center).into()
        } else {
            text(truncated).size(13).color(Color::from(txt_color)).font(FONT_INTER_SANS_MEDIUM).wrapping(iced::widget::text::Wrapping::None).into()
        };

        let pad_left = if col == SortColumn::TrackNumber { 0.0 } else { 15.0 };
        container(content)
            .width(Length::Fixed(w)).height(Length::Fill).center_y(Length::Fill)
            .padding(Padding { left: pad_left, right: 5.0, top: 0.0, bottom: 0.0 }).clip(true).into()
    };

    let mut elements: Vec<Element<'a, Message>> = Vec::new();

    // 1. Agregar Miniatura si se solicita
    if show_thumbnail {
        let thumb_img = album_art_widget(
            song.compressed_cached_cover_root.as_ref(),
            None,
            None,
            PlaceholderStyle::Small,
            Length::Fixed(32.0),
            4.0,
        );

        elements.push(
            container(thumb_img)
                .width(Length::Fixed(42.0))
                .height(Length::Fixed(row_height))
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .into()
        );
    }

    // 2. Agregar Columnas de metadatos (evitando duplicar el espacio de Thumbnail)
    for col in columns {
        if *col != SortColumn::AlbumCard && *col != SortColumn::AlbumThumbnail {
            elements.push(get_col(*col));
        }
    }



    let song_row_inner = iced::widget::Row::with_children(elements)
        .align_y(Alignment::Center).padding([0, 5]).height(Length::Fill);

    mouse_area(
        container(song_row_inner).width(Length::Fill).height(Length::Fixed(row_height)).align_y(Alignment::Center)
            .style(move |_t: &Theme| {
                if is_selected { container::Style::default().background(Color::from(COLOR_CONTRAST)) } 
                else { container::Style::default() }
            })
    ).on_press(on_select).interaction(iced::mouse::Interaction::Pointer).into()
}

pub fn library_song_row_widget<'a, Message: Clone + 'a>(
    song: &crate::db::database::SongData,
    song_idx: usize,
    is_selected: bool,
    columns: &[SortColumn],
    column_widths: &HashMap<SortColumn, u16>,
    on_select: Message,
    playing_path: &str,
) -> Element<'a, Message> {
    universal_song_row_widget(
        song, song_idx, is_selected, columns, column_widths, 
        on_select, playing_path, 32.0, false
    )
}

/// Renderiza una fila de canción en modo ThumbnailList (42px) con thumbnail 32x32 redondeado al inicio.
/// Renderiza una fila de canción en modo ThumbnailList (42px) con thumbnail 32x32 redondeado al inicio.
pub fn thumbnail_song_row_widget<'a, Message: Clone + 'a>(
    song: &crate::db::database::SongData,
    song_idx: usize,
    is_selected: bool,
    columns: &[SortColumn],
    column_widths: &HashMap<SortColumn, u16>,
    on_select: Message,
    playing_path: &str,
) -> Element<'a, Message> {
    universal_song_row_widget(
        song, song_idx, is_selected, columns, column_widths, 
        on_select, playing_path, 42.0, true
    )
}


/// Permite construir una lista universalizada que agrupa canciones por artistas,
/// gestiona el scroll, la virtualización, y las cabeceras pegajosas de forma global,
/// delegando la representación visual de la "fila" a un renderizador externo.
pub fn universal_song_list<'a, F>(
    manager: &'a crate::gui::library::LibraryManager,
    row_builder: F,
    row_height: f32, // Altura estimada para virtualización
    playing_path: &'a str,
) -> Element<'a, crate::gui::app::Message>
where
    F: Fn(&std::sync::Arc<crate::db::database::SongData>, usize, bool, &str) -> Element<'a, crate::gui::app::Message> + 'a,
{
    let groups = &manager.artist_groups;
    if groups.is_empty() {
        return container(text("La biblioteca está vacía o cargando...").color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM))
            .width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill).into();
    }

    let header_h: f32 = if manager.view_mode == crate::gui::library::LibraryViewMode::ThumbnailList { 42.0 } else { 32.0 };
    let album_header_h: f32 = if manager.view_mode == crate::gui::library::LibraryViewMode::ThumbnailList { 42.0 } else { 32.0 };

    let view_min_raw = manager.scroll_offset.y;
    let viewport_h = manager.last_viewport.as_ref().map(|v| v.height).unwrap_or(800.0);

    struct AlbumGroup<'a> {
        album_name: String,
        album_hash: String,
        songs: Vec<(&'a std::sync::Arc<crate::db::database::SongData>, usize)>,
        duration_secs: f64,
    }

    struct ArtistBlock<'a> {
        group: &'a crate::gui::library::ArtistGroup,
        albums: Vec<AlbumGroup<'a>>,
        is_collapsed: bool,
        top_y: f32,
        height: f32,
    }

    let mut artist_blocks = Vec::new();
    let mut total_content_h = 0.0;
    let mut global_song_idx = 0;

    for group in groups {
        let is_artist_collapsed = manager.collapsed_artists.contains(&group.name);
        let mut albums: Vec<AlbumGroup> = Vec::new();

        // Agrupar canciones por álbum preservando el orden
        for song in &group.songs {
            let alb_name = song.album.clone().unwrap_or_else(|| "Desconocido".to_string());
            
            if let Some(last_alb) = albums.last_mut() {
                if last_alb.album_name == alb_name {
                    last_alb.duration_secs += song.duration_secs.unwrap_or(0.0);
                    last_alb.songs.push((song, global_song_idx));
                    global_song_idx += 1;
                    continue;
                }
            }

            // Buscar hash en caché si existe
            let album_hash = if let Some(albums_cache) = &manager.cached_albums {
                albums_cache.iter()
                    .find(|a| a.title == alb_name && a.artist == group.name)
                    .map(|a| a.id.clone())
                    .unwrap_or_else(|| alb_name.clone())
            } else {
                alb_name.clone()
            };

            albums.push(AlbumGroup {
                album_name: alb_name,
                album_hash,
                songs: vec![(song, global_song_idx)],
                duration_secs: song.duration_secs.unwrap_or(0.0),
            });
            global_song_idx += 1;
        }

        let mut block_h = header_h;
        if !is_artist_collapsed {
            for alb in &albums {
                let composite_id = format!("{}|{}", group.name, alb.album_hash);
                let is_album_collapsed = manager.collapsed_albums.contains(composite_id.as_str());
                block_h += album_header_h;
                if !is_album_collapsed {
                    block_h += alb.songs.len() as f32 * row_height;
                }
            }
        }

        artist_blocks.push(ArtistBlock {
            group,
            albums,
            is_collapsed: is_artist_collapsed,
            top_y: total_content_h,
            height: block_h,
        });
        total_content_h += block_h;
    }

    // 2. Virtualización
    let max_scroll = (total_content_h - viewport_h).max(0.0);
    let view_min = view_min_raw.min(max_scroll);
    let view_max = view_min + viewport_h;
    
    let render_min = view_min - 300.0;
    let render_max = view_max + 300.0;

    let mut top_space = 0.0;
    let mut bottom_space = 0.0;
    
    enum VirtualRow<'a> {
        ArtistHeader(&'a crate::gui::library::ArtistGroup, bool),
        AlbumHeader(String, String, String, usize, f64, bool, bool), // name, hash, artist, count, duration, is_expanded, is_playing
        SongRow(&'a std::sync::Arc<crate::db::database::SongData>, usize),
    }

    let mut visible_elements = Vec::new();
    let mut sticky_artist_info = None;

    for ab in artist_blocks {
        let block_end = ab.top_y + ab.height;

        // Sticky Header: umbral +31px
        if ab.top_y <= view_min + 31.0 {
            sticky_artist_info = Some((ab.group, ab.is_collapsed));
        }

        if block_end < render_min {
            top_space += ab.height;
        } else if ab.top_y > render_max {
            bottom_space += ab.height;
        } else {
            // Artist Header
            if ab.top_y + header_h >= render_min && ab.top_y <= render_max {
                visible_elements.push(VirtualRow::ArtistHeader(ab.group, ab.is_collapsed));
            } else if ab.top_y + header_h < render_min {
                top_space += header_h;
            } else {
                bottom_space += header_h;
            }

            if !ab.is_collapsed {
                let mut current_y = ab.top_y + header_h;
                for alb in ab.albums {
                    let composite_id = format!("{}|{}", ab.group.name, alb.album_hash);
                    let is_album_expanded = !manager.collapsed_albums.contains(&composite_id);
                    let alb_songs_h = if is_album_expanded { alb.songs.len() as f32 * row_height } else { 0.0 };
                    let alb_total_h = album_header_h + alb_songs_h;
                    let alb_end = current_y + alb_total_h;

                    if alb_end >= render_min && current_y <= render_max {
                        // Album Header
                        if current_y + album_header_h >= render_min && current_y <= render_max {
                            let is_album_playing = alb.songs.iter().any(|(s, _)| s.full_file_path == playing_path);
                            visible_elements.push(VirtualRow::AlbumHeader(
                                alb.album_name.clone(), 
                                alb.album_hash.clone(), 
                                ab.group.name.clone(),
                                alb.songs.len(), 
                                alb.duration_secs, 
                                is_album_expanded,
                                is_album_playing
                            ));
                        } else if current_y + album_header_h < render_min {
                            top_space += album_header_h;
                        } else {
                            bottom_space += album_header_h;
                        }

                        // Songs
                        if is_album_expanded {
                            let mut song_y = current_y + album_header_h;
                            for (song, song_i) in alb.songs {
                                let s_end = song_y + row_height;
                                if s_end >= render_min && song_y <= render_max {
                                    visible_elements.push(VirtualRow::SongRow(song, song_i));
                                } else if s_end < render_min {
                                    top_space += row_height;
                                } else {
                                    bottom_space += row_height;
                                }
                                song_y += row_height;
                            }
                        }
                    } else if alb_end < render_min {
                        top_space += alb_total_h;
                    } else {
                        bottom_space += alb_total_h;
                    }
                    current_y += alb_total_h;
                }
            }
        }
    }

    let mut list_col = column![].spacing(0);
    if top_space > 0.0 {
        list_col = list_col.push(
            mouse_area(Space::new().height(Length::Fixed(top_space)))
                .on_press(crate::gui::app::Message::LibraryDeselect)
        );
    }

    for element in visible_elements {
        match element {
            VirtualRow::ArtistHeader(group, is_collapsed) => {
                let is_header_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Artist(group.name.clone()));
                let is_artist_playing = group.songs.iter().any(|s| s.full_file_path == playing_path);
                
                list_col = list_col.push(artist_header_widget(
                    group.name.clone(), is_collapsed, is_header_selected, is_artist_playing,
                    group.albums_count, group.songs_count, group.duration_secs, header_h,
                    crate::gui::app::Message::SelectArtistHeader(group.name.clone()),
                    crate::gui::app::Message::ToggleArtistExpansion(group.name.clone()),
                ));
            }
            VirtualRow::AlbumHeader(name, hash, artist, count, duration, is_expanded, is_playing) => {
                let composite_id = format!("{}|{}", artist, hash);
                let is_album_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Album(composite_id.clone()));

                list_col = list_col.push(album_header_widget(
                    name, count, duration, is_expanded, is_album_selected, is_playing,
                    album_header_h,
                    crate::gui::app::Message::SelectAlbum(composite_id.clone()),
                    crate::gui::app::Message::ToggleAlbumExpansion(composite_id),
                ));
            }
            VirtualRow::SongRow(song, song_i) => {
                let is_song_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Song(song.id));
                list_col = list_col.push(row_builder(song, song_i, is_song_selected, playing_path));
            }
        }
    }

    if bottom_space > 0.0 {
        list_col = list_col.push(
            mouse_area(Space::new().height(Length::Fixed(bottom_space)))
                .on_press(crate::gui::app::Message::LibraryDeselect)
        );
    }

    let list_container = mouse_area(container(list_col).width(Length::Fill).padding(Padding { left: 10.0, right: 15.0, top: 0.0, bottom: 0.0 }))
        .on_press(crate::gui::app::Message::LibraryDeselect);

    let main_scroll = standard_scrollable(
        crate::gui::library::LIBRARY_SCROLL_ID.clone(),
        list_container,
        iced::widget::scrollable::Direction::Vertical(standard_scrollbar())
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .on_scroll(crate::gui::app::Message::LibraryScroll);

    let content: Element<'a, crate::gui::app::Message> = if let Some((st_group, is_collapsed)) = sticky_artist_info {
        let is_header_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Artist(st_group.name.clone()));
        
        let is_artist_playing = st_group.songs.iter().any(|s| s.full_file_path == playing_path);

        let sticky_overlay = container(
            artist_header_widget(
                st_group.name.clone(),
                is_collapsed,
                is_header_selected,
                is_artist_playing,
                st_group.albums_count,
                st_group.songs_count,
                st_group.duration_secs,
                header_h,
                crate::gui::app::Message::SelectArtistHeader(st_group.name.clone()),
                crate::gui::app::Message::ToggleArtistExpansion(st_group.name.clone()),
            )
        )
        .width(Length::Fill)
        .height(Length::Fixed(header_h))
        .padding(Padding { left: 10.0, right: 15.0, top: 0.0, bottom: 0.0 })
        .align_y(iced::alignment::Vertical::Top);

        iced::widget::stack![
            main_scroll,
            sticky_overlay
        ].into()
    } else {
        main_scroll.into()
    };

    content
}

/// Vista Detallada: Combina agrupación por Artista, y dentro por Álbum.
/// Muestra a la izquierda la tarjeta del Álbum y a la derecha el Header de Álbum + la lista de canciones.
pub fn detailed_song_list<'a, F>(
    manager: &'a crate::gui::library::LibraryManager,
    row_builder: F,
    row_height: f32, // 32.0
    playing_path: &'a str,
) -> Element<'a, crate::gui::app::Message>
where
    F: Fn(&std::sync::Arc<crate::db::database::SongData>, usize, bool, &str) -> Element<'a, crate::gui::app::Message> + 'a,
{
    let groups = &manager.artist_groups;
    if groups.is_empty() {
        return container(text("La biblioteca está vacía o cargando...").color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM))
            .width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill).into();
    }

    let header_h = 32.0; // Artist header
    let album_header_h = 32.0;

    // La tarjeta es estricta a 250px como lo definió la columna
    let card_w = 250.0;

    let view_min_raw = manager.scroll_offset.y;
    let viewport_h = manager.last_viewport.as_ref().map(|v| v.height).unwrap_or(800.0);

    let mut total_content_h = 0.0;
    
    // Estructura para agrupar canciones por álbum mantieniendo el orden de las canciones
    struct AlbumGroup<'a> {
        album_name: String,
        album_hash: String,
        songs: Vec<(&'a std::sync::Arc<crate::db::database::SongData>, usize)>, // (song, global_idx)
        duration_secs: f64,
    }

    struct VirtualArtist<'a> {
        group: &'a crate::gui::library::ArtistGroup,
        is_collapsed: bool,
        top_y: f32,
        height: f32,
        albums: Vec<AlbumGroup<'a>>,
    }

    let mut v_artists = Vec::new();
    let mut global_song_idx = 0;

    for group in groups {
        let is_artist_collapsed = manager.collapsed_artists.contains(&group.name);
        
        let mut albums_map: Vec<AlbumGroup> = Vec::new();
        // Agrupar preserving order en O(N)
        for song in &group.songs {
            let alb_name = song.album.clone().unwrap_or_else(|| "Desconocido".to_string());
            
            if let Some(last_alb) = albums_map.last_mut() {
                if last_alb.album_name == alb_name {
                    last_alb.duration_secs += song.duration_secs.unwrap_or(0.0);
                    last_alb.songs.push((song, global_song_idx));
                    global_song_idx += 1;
                    continue;
                }
            }
            
            // Buscar hash en caché si existe
            let album_hash = if let Some(albums_cache) = &manager.cached_albums {
                albums_cache.iter()
                    .find(|a| a.title == alb_name && a.artist == group.name)
                    .map(|a| a.id.clone())
                    .unwrap_or_else(|| alb_name.clone())
            } else {
                alb_name.clone()
            };

            albums_map.push(AlbumGroup {
                album_name: alb_name,
                album_hash,
                songs: vec![(song, global_song_idx)],
                duration_secs: song.duration_secs.unwrap_or(0.0),
            });
            global_song_idx += 1;
        }

        let mut artist_h = header_h;
        if !is_artist_collapsed {
            for alb in &albums_map {
                let composite_id = format!("{}|{}", group.name, alb.album_hash);
                let is_album_expanded = !manager.collapsed_albums.contains(&composite_id);
                let card_h: f32 = if is_album_expanded { 323.0 } else { 0.0 };
                let right_h = if is_album_expanded {
                    album_header_h + alb.songs.len() as f32 * row_height
                } else {
                    album_header_h
                };
                let block_h = card_h.max(right_h) + 10.0; // padding inferior
                artist_h += block_h;
            }
        }

        v_artists.push(VirtualArtist {
            group,
            is_collapsed: is_artist_collapsed,
            top_y: total_content_h,
            height: artist_h,
            albums: albums_map,
        });
        
        total_content_h += artist_h;
    }

    let max_scroll = (total_content_h - viewport_h).max(0.0);
    let view_min = view_min_raw.min(max_scroll);
    let view_max = view_min + viewport_h;
    
    let render_min = view_min - 300.0;
    let render_max = view_max + 300.0;

    let mut top_space = 0.0;
    let mut bottom_space = 0.0;
    
    if top_space > 0.0 {
        // No añadimos espacio clicable aquí por ahora ya que el Sticky Header lo cubre
    }

    enum VirtualRow<'a> {
        ArtistHeader(&'a crate::gui::library::ArtistGroup, bool),
        AlbumBlock(AlbumGroup<'a>, bool, String, String, String), // (group, is_expanded, artist, genre, year)
    }

    let mut visible_elements = Vec::new();
    let mut sticky_artist_info = None;

    for va in v_artists {
        let a_end = va.top_y + va.height;

        // Sticky Header: umbral +31px
        if va.top_y <= view_min + 31.0 {
            sticky_artist_info = Some((va.group, va.is_collapsed));
        }

        if a_end < render_min {
            top_space += va.height;
        } else if va.top_y > render_max {
            bottom_space += va.height;
        } else {
            // Header
            let ah_end = va.top_y + header_h;
            if ah_end >= render_min && va.top_y <= render_max {
                visible_elements.push(VirtualRow::ArtistHeader(va.group, va.is_collapsed));
            } else if ah_end < render_min {
                top_space += header_h;
            } else {
                bottom_space += header_h;
            }

            if !va.is_collapsed {
                let mut current_y = va.top_y + header_h;
                for alb in va.albums {
                    let composite_id = format!("{}|{}", va.group.name, alb.album_hash);
                    let is_album_expanded = !manager.collapsed_albums.contains(&composite_id);
                    let card_h: f32 = if is_album_expanded { 323.0 } else { 0.0 }; 
                    let right_h = if is_album_expanded {
                        album_header_h + alb.songs.len() as f32 * row_height
                    } else {
                        album_header_h
                    };
                    let block_h = card_h.max(right_h) + 10.0; // padding inferior - must match pre-accumulator
                    let block_end = current_y + block_h;

                    if block_end >= render_min && current_y <= render_max {
                        let artist_name = &va.group.name;
                        let genre = alb.songs.first().and_then(|(s,_)| s.genre.clone()).unwrap_or_default();
                        let year = alb.songs.first().and_then(|(s,_)| s.release_year.clone()).unwrap_or_default();
                        visible_elements.push(VirtualRow::AlbumBlock(alb, is_album_expanded, artist_name.clone(), genre, year));
                    } else if block_end < render_min {
                        top_space += block_h;
                    } else {
                        bottom_space += block_h;
                    }
                    current_y += block_h;
                }
            }
        }
    }

    let mut list_col = column![].spacing(0);
    if top_space > 0.0 {
        list_col = list_col.push(
            mouse_area(Space::new().height(Length::Fixed(top_space)))
                .on_press(crate::gui::app::Message::LibraryDeselect)
        );
    }

    for element in visible_elements {
        match element {
            VirtualRow::ArtistHeader(group, is_collapsed) => {
                let is_artist_explicitly_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Artist(group.name.clone()));
                let is_artist_playing = group.songs.iter().any(|s| s.full_file_path == playing_path);
                let header = artist_header_widget(
                    group.name.clone(),
                    is_collapsed,
                    is_artist_explicitly_selected,
                    is_artist_playing,
                    group.albums_count,
                    group.songs_count,
                    group.duration_secs,
                    32.0, // DetailedList always uses 32px headers
                    crate::gui::app::Message::SelectArtistHeader(group.name.clone()),
                    crate::gui::app::Message::ToggleArtistExpansion(group.name.clone()),
                );
                list_col = list_col.push(header);
            }
            VirtualRow::AlbumBlock(alb, is_expanded, artist_name, genre, year) => {
                // Buscamos el álbum en cached_albums usando el título para obtener su hash_id y cover_path
                let (album_hash, cover_path) = if let Some(albums) = &manager.cached_albums {
                    let match_alb = albums.iter().find(|a| a.title == alb.album_name && a.artist == artist_name);
                    (
                        alb.album_hash.clone(),
                        match_alb.and_then(|a| a.cover_path.clone())
                    )
                } else {
                    (alb.album_hash.clone(), None)
                };

                let card_wrapper = album_art_widget(
                    cover_path.as_ref(), // AVIF Path
                    None,                // RAW Data
                    None,                // Preloaded Handle
                    PlaceholderStyle::Large,
                    Length::Fixed(card_w - 30.0),
                    8.0,
                );

                let info_col = column![
                    smart_truncate_text(artist_name.clone(), 13.0, FONT_INTER_SANS_MEDIUM, COLOR_TEXT_PRIMARY),
                    smart_truncate_text(alb.album_name.clone(), 13.0, FONT_INTER_SANS_MEDIUM, COLOR_TEXT_PRIMARY),
                    smart_truncate_text(genre.clone(), 13.0, FONT_INTER_SANS_MEDIUM, COLOR_TEXT_PRIMARY),
                    text(year.clone()).size(13).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
                ].spacing(2).width(Length::Fill);

                let composite_id = format!("{}|{}", artist_name, album_hash);
                let is_album_explicitly_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Album(composite_id.clone()));
                let is_song_selected_in_album = alb.songs.iter().any(|(s, _)| manager.selected_items.contains(&crate::gui::library::LibraryListItem::Song(s.id)));
                let is_album_card_highlighted = is_album_explicitly_selected || is_song_selected_in_album;

                let card_col = column![card_wrapper, info_col].spacing(5).width(Length::Fixed(card_w - 30.0));
                
                let card_container = mouse_area(
                    container(card_col)
                        .width(Length::Fixed(card_w))
                        .height(Length::Fixed(323.0))
                        .padding(iced::Padding { top: 15.0, bottom: 15.0, left: 15.0, right: 15.0 })
                        .style(move |_t: &Theme| {
                            if is_album_card_highlighted {
                                let rad = iced::border::Radius { top_left: 0.0, top_right: 0.0, bottom_right: 10.0, bottom_left: 10.0 };
                                container::Style::default().background(COLOR_CONTRAST).border(iced::Border { radius: rad, ..Default::default() })
                            } else {
                                container::Style::default()
                            }
                        })
                )
                .on_press(crate::gui::app::Message::SelectAlbum(composite_id.clone()))
                .interaction(iced::mouse::Interaction::Pointer);

                // Lado Derecho (Album Header + Canciones)
                let right_h = if is_expanded {
                    album_header_h + alb.songs.len() as f32 * row_height
                } else {
                    album_header_h
                };
                let _block_h = (card_w + 30.0).max(right_h);
                
                let is_album_playing = alb.songs.iter().any(|(s, _)| s.full_file_path == playing_path);
                
                let alb_header = album_header_widget(
                    alb.album_name.clone(),
                    alb.songs.len(),
                    alb.duration_secs,
                    is_expanded,
                    is_album_explicitly_selected,
                    is_album_playing,
                    album_header_h,
                    crate::gui::app::Message::SelectAlbum(composite_id.clone()),
                    crate::gui::app::Message::ToggleAlbumExpansion(composite_id),
                );

                let mut right_col = column![alb_header].spacing(0).width(Length::Fill);

                if is_expanded {
                    for (song, song_i) in alb.songs {
                        let is_song_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Song(song.id));
                        let song_row = row_builder(song, song_i, is_song_selected, playing_path);
                        right_col = right_col.push(song_row);
                    }
                }

                let content_row = if is_expanded {
                    row![
                        container(card_container).height(Length::Shrink).align_y(iced::alignment::Vertical::Top),
                        container(right_col).height(Length::Shrink).align_y(iced::alignment::Vertical::Top)
                    ].spacing(0).width(Length::Fill).align_y(Alignment::Start)
                } else {
                    row![
                        // If collapsed, we omit the card (it's hidden) and just show the header block spanning
                        container(Space::new().width(card_w)).height(Length::Shrink),
                        container(right_col).height(Length::Shrink).align_y(iced::alignment::Vertical::Top)
                    ].spacing(0).width(Length::Fill).align_y(Alignment::Start)
                };

                list_col = list_col.push(
                    column![content_row, Space::new().height(Length::Fixed(10.0))]
                );
            }
        }
    }

    if bottom_space > 0.0 {
        list_col = list_col.push(
            mouse_area(Space::new().height(Length::Fixed(bottom_space)))
                .on_press(crate::gui::app::Message::LibraryDeselect)
        );
    }

    let list_container = mouse_area(container(list_col).width(Length::Fill).padding(Padding { left: 10.0, right: 15.0, top: 0.0, bottom: 0.0 }))
        .on_press(crate::gui::app::Message::LibraryDeselect);

    let main_scroll = standard_scrollable(
        crate::gui::library::LIBRARY_SCROLL_ID.clone(),
        list_container,
        iced::widget::scrollable::Direction::Vertical(standard_scrollbar())
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .on_scroll(crate::gui::app::Message::LibraryScroll);

    let content: Element<'a, crate::gui::app::Message> = if let Some((st_group, is_collapsed)) = sticky_artist_info {
        let is_header_explicitly_selected = manager.selected_header.as_ref() == Some(&st_group.name)
            && manager.selected_album.is_none()
            && manager.selected_song_idx.is_none();
        
        let is_artist_playing = st_group.songs.iter().any(|s| s.full_file_path == playing_path);
        
        let sticky_overlay = container(
            artist_header_widget(
                st_group.name.clone(),
                is_collapsed,
                is_header_explicitly_selected,
                is_artist_playing,
                st_group.albums_count,
                st_group.songs_count,
                st_group.duration_secs,
                32.0, // DetailedList always uses 32px headers
                crate::gui::app::Message::SelectArtistHeader(st_group.name.clone()),
                crate::gui::app::Message::ToggleArtistExpansion(st_group.name.clone()),
            )
        )
        .width(Length::Fill)
        .height(Length::Fixed(32.0))
        .padding(Padding { left: 10.0, right: 15.0, top: 0.0, bottom: 0.0 })
        .align_y(iced::alignment::Vertical::Top);

        iced::widget::stack![
            main_scroll,
            sticky_overlay
        ].into()
    } else {
        main_scroll.into()
    };

    content
}
