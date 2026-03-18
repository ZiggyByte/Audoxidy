use iced::{
    widget::{button, column, container, mouse_area, row, scrollable, svg, text, Space},
    Alignment, Color, Element, Length, Theme,
};
use iced::advanced::{layout, mouse, overlay, renderer, widget::{Operation, Tree}, Clipboard, Layout, Shell, Widget};
use iced::{Event, Rectangle, Size, Vector};

use crate::gui::theme::{COLOR_TEXT_PRIMARY, COLOR_CONTRAST, COLOR_TEXT_SECONDARY, COLOR_ACCENT, COLOR_BG, FONT_INTER_SANS_MEDIUM};
use crate::utils::{truncate_text, SortColumn};
use std::collections::HashMap;

// ==========================================
// 1. Modelos de Datos y Widgets Avanzados
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
        .interaction(iced::mouse::Interaction::Idle)
        .into()
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
    on_hover: impl Fn(Option<SortColumn>) -> Message + 'a,
    on_resize: impl Fn(SortColumn) -> Message + 'a,
    on_sort: impl Fn(SortColumn) -> Message + 'a,
) -> Element<'a, Message> {

    let mut sort_bar_content = row![].align_y(Alignment::Center).height(Length::Fill).padding(iced::Padding { top: 0.0, right: 5.0, bottom: 0.0, left: 35.0 });

    for &sort in columns {
        let is_active = current_sort == Some(sort);
        let width = *column_widths.get(&sort).unwrap_or(&100) as f32;
        
        let available_w = width - 25.0; // Espacio reservado para icono/separador
        let max_chars = (available_w / 7.0).max(1.0) as usize;
        let t_str = truncate_text(sort.as_str(), max_chars);
        
        let t = text(t_str)
            .size(12)
            .font(FONT_INTER_SANS_MEDIUM)
            .color(COLOR_TEXT_SECONDARY);

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
            .width(Length::Fixed(3.0))
            .height(Length::Fixed(16.0))
            .style(move |_t: &Theme| {
                let bg_color = if is_hovered { COLOR_ACCENT } else { Color::from_rgba(COLOR_TEXT_SECONDARY.r, COLOR_TEXT_SECONDARY.g, COLOR_TEXT_SECONDARY.b, 0.3)};
                container::Style::default()
                    .background(bg_color)
                    .border(iced::Border { radius: 4.0.into(), ..Default::default() })
            });

        let on_h = on_hover(Some(sort));
        let on_h_exit = on_hover(None);
        let separator_area = mouse_area(separator_visual)
            .on_enter(on_h)
            .on_exit(on_h_exit)
            .on_press(on_resize(sort))
            .interaction(iced::mouse::Interaction::ResizingHorizontally);

        let sort_btn_content = if let Some(ic) = icon_el {
            row![t, Space::new().width(Length::Fill), ic].align_y(Alignment::Center)
        } else {
            row![t, Space::new().width(Length::Fill)].align_y(Alignment::Center)
        };

        let sort_btn = button(sort_btn_content)
            .width(Length::Fill)
            .padding(iced::Padding { left: 5.0, right: 0.0, top: 0.0, bottom: 0.0 })
            .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
            .on_press(on_sort(sort));

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
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::new().width(0).scroller_width(0)
                ))
        )
            .width(Length::Fill)
            .height(Length::Fixed(28.0))
            .style(|_t: &Theme| container::Style::default().background(COLOR_BG)),
        container(Space::new().width(Length::Fill).height(2.0))
            .style(|_t: &Theme| container::Style::default().background(Color::from(COLOR_CONTRAST)))
    ].into()
}
