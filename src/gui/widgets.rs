use iced::{
    widget::{button, column, container, mouse_area, row, scrollable, svg, text, text_input, Space, Responsive, pick_list},
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
    content: impl Into<String>,
    font_size: f32,
    font: iced::Font,
    color: Color,
) -> Element<'a, Message> {
    smart_truncate_text_advanced(
        content.into(), font_size, font, color, 
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
                iced::widget::pick_list::Status::Hovered | iced::widget::pick_list::Status::Opened { .. } => true,
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
        .menu_style(|_theme: &Theme| {
            iced::overlay::menu::Style {
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
            }
        });

    pick.into()
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
    avif_path: Option<&str>,
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
    id: Option<iced::widget::Id>,
    placeholder: &'a str,
    value: &'a str,
    on_change: impl Fn(String) -> Message + 'a,
    on_clear: Message,
    width: Length,
) -> Element<'a, Message> {
    let has_content = !value.is_empty();

    let mut input = text_input(placeholder, value)
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

    if let Some(actual_id) = id {
        input = input.id(actual_id);
    }

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
    let is_playing = song.full_file_path.as_ref() == playing_path;

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
            song.compressed_cached_cover_root.as_deref(),
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
/// gestiona el scroll, la virtualización, y las cabeceras pegajosas de forma global.
pub fn universal_song_list<'a, F>(
    manager: &'a crate::gui::library::LibraryManager,
    row_builder: F,
    _row_height: f32, 
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

    let header_h = if manager.view_mode == crate::gui::library::LibraryViewMode::ThumbnailList { 42.0 } else { 32.0 }; 
    let album_header_h = if manager.view_mode == crate::gui::library::LibraryViewMode::ThumbnailList { 42.0 } else { 32.0 };

    let (top_space, bottom_space, _total_h, visible_elements, sticky_artist_info) = manager.get_visible_elements();

    let mut list_col = column![].spacing(0);
    if top_space > 0.0 {
        list_col = list_col.push(
            mouse_area(Space::new().height(Length::Fixed(top_space)))
                .on_press(crate::gui::app::Message::LibraryDeselect)
        );
    }

    for element in visible_elements {
        match element {
            crate::gui::library::LibraryVirtualRow::ArtistHeader { name, is_collapsed, albums_count, songs_count, duration_secs } => {
                let is_header_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Artist(name.clone()));
                
                let is_artist_playing = manager.artist_groups.iter()
                    .find(|g| g.name == *name)
                    .map(|g| g.songs.iter().any(|s| s.full_file_path.as_ref() == playing_path))
                    .unwrap_or(false);
                
                list_col = list_col.push(artist_header_widget(
                    name.clone(), is_collapsed, is_header_selected, is_artist_playing,
                    albums_count, songs_count, duration_secs, header_h,
                    crate::gui::app::Message::SelectArtistHeader(name.clone()),
                    crate::gui::app::Message::ToggleArtistExpansion(name.clone()),
                ));
            }
            crate::gui::library::LibraryVirtualRow::AlbumBlock { album_name, album_hash, artist_name, is_expanded, songs_count, duration_secs, .. } => {
                let composite_id = format!("{}|{}", artist_name, album_hash);
                let is_album_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Album(composite_id.clone()));
                
                // En este modo, el header del álbum se renderiza como una fila
                list_col = list_col.push(album_header_widget(
                    album_name.clone(), songs_count, duration_secs, is_expanded, is_album_selected, false,
                    album_header_h,
                    crate::gui::app::Message::SelectAlbum(composite_id.clone()),
                    crate::gui::app::Message::ToggleAlbumExpansion(composite_id),
                ));
            }
            crate::gui::library::LibraryVirtualRow::SimpleSong { song, global_idx } => {
                let is_song_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Song(song.id));
                list_col = list_col.push(row_builder(&song, global_idx, is_song_selected, playing_path));
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

    let content: Element<'a, crate::gui::app::Message> = if let Some((ref st_name, st_collapsed, st_albums, st_songs, st_duration)) = sticky_artist_info {
        let is_header_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Artist(st_name.clone()));
        
        let is_artist_playing = manager.artist_groups.iter()
            .find(|g| g.name == *st_name)
            .map(|g| g.songs.iter().any(|s| s.full_file_path .as_ref() == playing_path))
            .unwrap_or(false);

        let sticky_overlay = container(
            artist_header_widget(
                st_name.clone(), st_collapsed, is_header_selected, is_artist_playing,
                st_albums, st_songs, st_duration, header_h,
                crate::gui::app::Message::SelectArtistHeader(st_name.clone()),
                crate::gui::app::Message::ToggleArtistExpansion(st_name.clone()),
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
    _row_height: f32, 
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

    let _header_h = if manager.view_mode == crate::gui::library::LibraryViewMode::ThumbnailList { 42.0 } else { 32.0 };
    let album_header_h = 32.0;
    let card_w = 250.0;

    let (top_space, bottom_space, _total_h, visible_elements, sticky_artist_info) = manager.get_visible_elements();

    let mut list_col = column![].spacing(0);
    if top_space > 0.0 {
        list_col = list_col.push(
            mouse_area(Space::new().height(Length::Fixed(top_space)))
                .on_press(crate::gui::app::Message::LibraryDeselect)
        );
    }

    for element in visible_elements {
        match element {
            crate::gui::library::LibraryVirtualRow::ArtistHeader { name, is_collapsed, albums_count, songs_count, duration_secs } => {
                let is_artist_explicitly_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Artist(name.clone()));
                
                let is_artist_playing = manager.artist_groups.iter()
                    .find(|g| g.name == *name)
                    .map(|g| g.songs.iter().any(|s| s.full_file_path.as_ref() == playing_path))
                    .unwrap_or(false);

                let header = artist_header_widget(
                    name.clone(), is_collapsed, is_artist_explicitly_selected, is_artist_playing,
                    albums_count, songs_count, duration_secs, 32.0,
                    crate::gui::app::Message::SelectArtistHeader(name.clone()),
                    crate::gui::app::Message::ToggleArtistExpansion(name.clone()),
                );
                list_col = list_col.push(header);
            }
            crate::gui::library::LibraryVirtualRow::AlbumBlock { album_name, album_hash, artist_name, genre, year, is_expanded, songs, songs_count, duration_secs } => {
                let cover_path = if let Some(albums) = &manager.cached_albums {
                    albums.iter().find(|a| a.title == *album_name && a.artist == *artist_name).and_then(|a| a.cover_path.clone())
                } else {
                    None
                };

                let card_wrapper = album_art_widget(
                    cover_path.as_deref(), None, None, PlaceholderStyle::Large, Length::Fixed(card_w - 30.0), 8.0,
                );

                let info_col = column![
                    smart_truncate_text(artist_name.clone(), 13.0, FONT_INTER_SANS_MEDIUM, COLOR_TEXT_PRIMARY),
                    smart_truncate_text(album_name.clone(), 13.0, FONT_INTER_SANS_MEDIUM, COLOR_TEXT_PRIMARY),
                    smart_truncate_text(genre.clone(), 13.0, FONT_INTER_SANS_MEDIUM, COLOR_TEXT_PRIMARY),
                    text(year.clone()).size(13).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
                ].spacing(2).width(Length::Fill);

                let composite_id = format!("{}|{}", artist_name, album_hash);
                let is_album_explicitly_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Album(composite_id.clone()));
                let is_song_selected_in_album = songs.iter().any(|(s, _)| manager.selected_items.contains(&crate::gui::library::LibraryListItem::Song(s.id)));
                let is_album_card_highlighted = is_album_explicitly_selected || is_song_selected_in_album;

                let card_col = column![card_wrapper, info_col].spacing(5).width(Length::Fixed(card_w - 30.0));
                
                let card_container = mouse_area(
                    container(card_col)
                        .width(Length::Fixed(card_w)).height(Length::Fixed(323.0))
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

                let is_album_playing = songs.iter().any(|(s, _)| s.full_file_path.as_ref() == playing_path);
                
                let alb_header = album_header_widget(
                    album_name.clone(),
                    songs_count,
                    duration_secs,
                    is_expanded,
                    is_album_explicitly_selected,
                    is_album_playing,
                    album_header_h,
                    crate::gui::app::Message::SelectAlbum(composite_id.clone()),
                    crate::gui::app::Message::ToggleAlbumExpansion(composite_id),
                );

                let mut right_col = column![alb_header].spacing(0).width(Length::Fill);

                if is_expanded {
                    for (song, song_i) in &songs {
                        let is_song_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Song(song.id));
                        let song_row = row_builder(song, *song_i, is_song_selected, playing_path);
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
            _ => {}
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

    let content: Element<'a, crate::gui::app::Message> = if let Some((ref st_name, st_collapsed, st_albums, st_songs, st_duration)) = sticky_artist_info {
        let is_header_selected = manager.selected_items.contains(&crate::gui::library::LibraryListItem::Artist(st_name.clone()));
        
        let is_artist_playing = manager.artist_groups.iter()
            .find(|g| g.name == *st_name)
            .map(|g| g.songs.iter().any(|s| s.full_file_path.as_ref() == playing_path))
            .unwrap_or(false);
        
        let sticky_overlay = container(
            artist_header_widget(
                st_name.clone(), st_collapsed, is_header_selected, is_artist_playing,
                st_albums, st_songs, st_duration, 32.0,
                crate::gui::app::Message::SelectArtistHeader(st_name.clone()),
                crate::gui::app::Message::ToggleArtistExpansion(st_name.clone()),
            )
        )
        .width(Length::Fill).height(Length::Fixed(32.0))
        .padding(Padding { left: 10.0, right: 15.0, top: 0.0, bottom: 0.0 })
        .align_y(iced::alignment::Vertical::Top);

        iced::widget::stack![main_scroll, sticky_overlay].into()
    } else {
        main_scroll.into()
    };

    content
}
