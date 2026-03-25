use iced::{
    widget::{button, column, container, mouse_area, row, scrollable, svg, text, Space},
    Alignment, Color, Element, Length, Theme,
};
use iced::advanced::{layout, mouse, overlay, renderer, widget::{Operation, Tree}, Clipboard, Layout, Shell, Widget};
use iced::{Event, Rectangle, Size, Vector, Padding};

use crate::gui::theme::{COLOR_TEXT_PRIMARY, COLOR_CONTRAST, COLOR_TEXT_SECONDARY, COLOR_ACCENT, COLOR_BG, FONT_INTER_SANS_MEDIUM};
use crate::utils::{truncate_text, SortColumn, format_duration, format_metadata};
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

        let sort_btn_content = if let Some(ic) = icon_el {
            row![t, Space::new().width(Length::Fill), ic].align_y(Alignment::Center)
        } else {
            row![t, Space::new().width(Length::Fill)].align_y(Alignment::Center)
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

// ==========================================
// 3. Componentes Específicos de Biblioteca
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
    albums_count: usize,
    songs_count: usize,
    duration_secs: f64,
    row_h: f32,       // Altura de la fila: 32px para SimpleList/DetailedList, 42px para ThumbnailList
    on_select: Message,
    on_toggle: Message,
) -> Element<'a, Message> {
    let chevron = if is_collapsed { "arrow-down-chevron.svg" } else { "arrow-up-chevron.svg" };
    let time_str = format_duration(duration_secs);
    
    let artist_name_row = row![
        text(name).size(15).font(FONT_INTER_SANS_MEDIUM).color(COLOR_TEXT_PRIMARY),
    ].align_y(Alignment::Center);

    let artist_name_row = if is_selected {
        artist_name_row.push(text(" •").size(15).font(FONT_INTER_SANS_MEDIUM).color(COLOR_TEXT_PRIMARY))
    } else {
        artist_name_row
    };

    let header_content = row![
        artist_name_row,
        Space::new().width(Length::Fill),
        text(format!("{} Canciones | {} Álbumes | {}",  songs_count, albums_count, time_str))
            .size(14).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM),
        Space::new().width(15),
        chevron_btn(chevron, on_toggle, row_h, row_h - 4.0),
    ].align_y(Alignment::Center).padding([0, 15]);

    container(
        mouse_area(header_content)
            .on_press(on_select)
    )
    .width(Length::Fill)
    .height(Length::Fixed(row_h))
    .center_y(Length::Fill)
    .style(|_t| container::Style::default().background(COLOR_CONTRAST))
    .into()
}

/// Renderiza una fila de canción en modo lista (32px).
pub fn library_song_row_widget<'a, Message: Clone + 'a>(
    song: &crate::db::database::SongData,
    _song_idx: usize,
    is_selected: bool,
    columns: &[SortColumn],
    column_widths: &HashMap<SortColumn, u16>,
    on_select: Message,
    on_add_playlist: Message,
) -> Element<'a, Message> {
    let txt_color = if is_selected { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };
    
    let get_col = |col: SortColumn| -> Element<'a, Message> {
        let w = *column_widths.get(&col).unwrap_or(&100) as f32;
        let max_chars = ((w - 10.0) / 7.0).max(1.0) as usize;
        let val = format_metadata(song, &col); // Reutiliza lógica de utils/mod.rs
        let truncated = truncate_text(&val, max_chars);

        container(text(truncated).size(13).color(Color::from(txt_color)).font(FONT_INTER_SANS_MEDIUM))
            .width(Length::Fixed(w)).height(Length::Fixed(15.0)).center_y(Length::Fill)
            .padding(Padding { left: 5.0, right: 5.0, top: 0.0, bottom: 0.0 }).clip(true).into()
    };

    let mut elements: Vec<Element<'a, Message>> = Vec::new();
    for col in columns {
        if *col != SortColumn::AlbumCard {
            elements.push(get_col(*col));
        }
    }
    elements.push(
        button(text("►").size(11).color(Color::from(txt_color))).on_press(on_add_playlist)
            .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
            .into()
    );

    let song_row_inner = iced::widget::Row::with_children(elements)
        .align_y(Alignment::Center).padding([0, 15]).height(Length::Fixed(15.0));

    mouse_area(
        container(song_row_inner).width(Length::Fill).height(Length::Fixed(32.0)).align_y(Alignment::Center)
            .style(move |_t: &Theme| {
                if is_selected { container::Style::default().background(Color::from(COLOR_CONTRAST)) } 
                else { container::Style::default() }
            })
    ).on_press(on_select).interaction(iced::mouse::Interaction::Pointer).into()
}

/// Renderiza una fila de canción en modo ThumbnailList (42px) con thumbnail 32x32 redondeado al inicio.
pub fn thumbnail_song_row_widget<'a, Message: Clone + 'a>(
    song: &crate::db::database::SongData,
    _song_idx: usize,
    is_selected: bool,
    columns: &[SortColumn],
    column_widths: &HashMap<SortColumn, u16>,
    on_select: Message,
    on_add_playlist: Message,
) -> Element<'a, Message> {
    let txt_color = if is_selected { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };

    // --- Thumbnail del álbum — ancho FIJO 42px, no se ve afectado por column_widths ---
    // Usar compressed_cached_cover_root preferentemente antes que original_cover_root
    let cover_path = song.compressed_cached_cover_root.as_ref()
        .or(song.original_cover_root.as_ref());

    let thumb_img: Element<'a, Message> = if let Some(path) = cover_path {
        container(
            iced::widget::image::Image::new(iced::widget::image::Handle::from_path(path.clone()))
                .width(Length::Fixed(32.0))
                .height(Length::Fixed(32.0))
                .content_fit(iced::ContentFit::Cover)
                .border_radius(4.0)
        )
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(32.0))
        .style(|_t: &Theme| {
            container::Style::default()
                .border(iced::Border { radius: 4.0.into(), ..Default::default() })
        })
        .clip(true)
        .into()
    } else {
        // Placeholder: mismo estilo que las tarjetas del Grid sin portada (COLOR_BG + bordes redondeados)
        container(
            iced::widget::svg(iced::widget::svg::Handle::from_path("assets/icons/album.svg"))
                .width(Length::Fixed(20.0))
                .height(Length::Fixed(20.0))
                .style(|_t: &Theme, _s| iced::widget::svg::Style {
                    color: Some(Color::from(COLOR_TEXT_SECONDARY)),
                })
        )
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(32.0))
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(|_t: &Theme| {
            container::Style::default()
                .background(Color::from(COLOR_BG))
                .border(iced::Border { radius: 4.0.into(), ..Default::default() })
        })
        .into()
    };

    // Columna del thumbnail: siempre 42px fija + 10px padding a cada lado
    let thumb_col: Element<'a, Message> = container(thumb_img)
        .width(Length::Fixed(42.0))
        .height(Length::Fixed(42.0))
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .padding(Padding { left: 0.0, right: 10.0, top: 0.0, bottom: 0.0 })
        .into();

    // --- Columnas de texto (misma lógica de SimpleList, excluye AlbumCard y AlbumThumbnail) ---
    let get_col = |col: SortColumn| -> Element<'a, Message> {
        let w = *column_widths.get(&col).unwrap_or(&100) as f32;
        let max_chars = ((w - 10.0) / 7.0).max(1.0) as usize;
        let val = format_metadata(song, &col);
        let truncated = truncate_text(&val, max_chars);

        container(text(truncated).size(13).color(Color::from(txt_color)).font(FONT_INTER_SANS_MEDIUM))
            .width(Length::Fixed(w)).height(Length::Fixed(15.0)).center_y(Length::Fill)
            .padding(Padding { left: 5.0, right: 5.0, top: 0.0, bottom: 0.0 }).clip(true).into()
    };

    let mut elements: Vec<Element<'a, Message>> = vec![thumb_col];
    for col in columns {
        if *col != SortColumn::AlbumCard && *col != SortColumn::AlbumThumbnail {
            elements.push(get_col(*col));
        }
    }
    elements.push(
        button(text("►").size(11).color(Color::from(txt_color))).on_press(on_add_playlist)
            .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
            .into()
    );

    // Fila interna: 10px izq. para alinear metadatos con sort bar, 10px der.
    let song_row_inner = iced::widget::Row::with_children(elements)
        .align_y(Alignment::Center)
        .padding(Padding { left: 15.0, right: 10.0, top: 0.0, bottom: 0.0 })
        .height(Length::Fixed(42.0));

    mouse_area(
        container(song_row_inner)
            .width(Length::Fill)
            .height(Length::Fixed(42.0))
            .align_y(iced::alignment::Vertical::Center)
            .style(move |_t: &Theme| {
                if is_selected { container::Style::default().background(Color::from(COLOR_CONTRAST)) } 
                else { container::Style::default() }
            })
    ).on_press(on_select).interaction(iced::mouse::Interaction::Pointer).into()
}

/// Permite construir una lista universalizada que agrupa canciones por artistas,
/// gestiona el scroll, la virtualización, y las cabeceras pegajosas de forma global,
/// delegando la representación visual de la "fila" a un renderizador externo.
pub fn universal_song_list<'a, F>(
    manager: &'a crate::gui::library::LibraryManager,
    row_builder: F,
    row_height: f32, // Altura estimada para virtualización
) -> Element<'a, crate::gui::app::Message>
where
    F: Fn(&crate::db::database::SongData, usize, bool) -> Element<'a, crate::gui::app::Message> + 'a,
{
    let groups = &manager.artist_groups;
    if groups.is_empty() {
        return container(text("La biblioteca está vacía o cargando...").color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM))
            .width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill).into();
    }

    // Header height must match get_visible_items and artist_header_widget heights
    let header_h: f32 = if manager.view_mode == crate::gui::library::LibraryViewMode::ThumbnailList { 42.0 } else { 32.0 };

    // 1. Calcular alturas acumuladas para virtualización
    let view_min_raw = manager.last_viewport.as_ref().map(|v| v.absolute_offset().y).unwrap_or(0.0);
    let viewport_h = manager.last_viewport.as_ref().map(|v| v.bounds().height).unwrap_or(800.0);

    let mut total_content_h = 0.0;
    let mut artist_tops = Vec::new();

    for group in groups {
        let is_collapsed = manager.collapsed_artists.contains(&group.name);
        let group_h = header_h + if is_collapsed { 0.0 } else { group.songs.len() as f32 * row_height };
        
        artist_tops.push((total_content_h, group, is_collapsed));
        total_content_h += group_h;
    }

    // Clampear view_min
    let max_scroll = (total_content_h - viewport_h).max(0.0);
    let view_min = view_min_raw.min(max_scroll);
    let view_max = view_min + viewport_h;
    
    // Margen de seguridad para scroll suave
    let render_min = view_min - 200.0;
    let render_max = view_max + 200.0;

    let mut top_space = 0.0;
    let mut bottom_space = 0.0;
    
    // Almacenamos si es Header o Fila
    enum VirtualRow<'a> {
        Header(&'a crate::gui::library::ArtistGroup, bool), // (group, is_collapsed)
        Row(&'a crate::db::database::SongData, usize), // (song, global_idx)
    }

    let mut visible_elements = Vec::new();
    let mut global_song_idx = 0;
    let mut sticky_artist_info = None;

    for (h_start, group, is_collapsed) in artist_tops {
        let group_songs = group.songs.len();
        let group_h = header_h + if is_collapsed { 0.0 } else { group_songs as f32 * row_height };
        let h_end = h_start + group_h;

        // Sticky Header: umbral +31px
        if h_start <= view_min + 31.0 {
            sticky_artist_info = Some((group, is_collapsed));
        }

        // Virtualización
        if h_end < render_min {
            top_space += group_h;
            global_song_idx += group_songs;
        } else if h_start > render_max {
            bottom_space += group_h;
            global_song_idx += group_songs;
        } else {
            // El header es visible?
            let header_end = h_start + header_h;
            if header_end >= render_min && h_start <= render_max {
                visible_elements.push(VirtualRow::Header(group, is_collapsed));
            }
            
            // Canciones visibles?
            if !is_collapsed {
                let mut song_y = h_start + header_h;
                for song in &group.songs {
                    let s_end = song_y + row_height;
                    // OJO: solo incluimos una instancia de la fila si toca la pantalla
                    if s_end >= render_min && song_y <= render_max {
                        visible_elements.push(VirtualRow::Row(song, global_song_idx));
                    } else if s_end < render_min {
                        top_space += row_height;
                    } else {
                        bottom_space += row_height;
                    }
                    song_y += row_height;
                    global_song_idx += 1;
                }
            } else {
                global_song_idx += group_songs;
            }
        }
    }

    let mut list_col = column![].spacing(0);
    if top_space > 0.0 {
        list_col = list_col.push(Space::new().height(Length::Fixed(top_space)));
    }

    for element in visible_elements {
        match element {
            VirtualRow::Header(group, is_collapsed) => {
                let is_header_selected = manager.selected_header.as_ref() == Some(&group.name);
                
                let header = artist_header_widget(
                    group.name.clone(),
                    is_collapsed,
                    is_header_selected,
                    group.albums.len(),
                    group.songs.len(),
                    group.duration_secs,
                    header_h, // Dynamic: 42px for ThumbnailList, 32px for others
                    crate::gui::app::Message::SelectArtistHeader(group.name.clone()),
                    crate::gui::app::Message::ToggleArtistExpansion(group.name.clone()),
                );
                
                list_col = list_col.push(header);
            }
            VirtualRow::Row(song, song_i) => {
                let is_song_selected = manager.selected_song_idx == Some(song_i);
                
                // Usamos el constructor inyectado
                let song_row = row_builder(song, song_i, is_song_selected);

                list_col = list_col.push(song_row);
            }
        }
    }

    if bottom_space > 0.0 {
        list_col = list_col.push(Space::new().height(Length::Fixed(bottom_space)));
    }

    let main_scroll = scrollable(container(list_col).width(Length::Fill).padding([0, 15]))
        .width(Length::Fill).height(Length::Fill)
        .direction(iced::widget::scrollable::Direction::Vertical(
            iced::widget::scrollable::Scrollbar::new().width(4).margin(0).scroller_width(4)
        ))
        .id(crate::gui::library::LIBRARY_SCROLL_ID.clone())
        .on_scroll(crate::gui::app::Message::LibraryScroll)
        .style(crate::gui::widgets::custom_scrollbar_style);

    let content: Element<'a, crate::gui::app::Message> = if let Some((st_group, is_collapsed)) = sticky_artist_info {
        let is_header_selected = manager.selected_header.as_ref() == Some(&st_group.name);
        
        let sticky_overlay = container(
            artist_header_widget(
                st_group.name.clone(),
                is_collapsed,
                is_header_selected,
                st_group.albums.len(),
                st_group.songs.len(),
                st_group.duration_secs,
                header_h, // Dynamic: 42px for ThumbnailList, 32px for others
                crate::gui::app::Message::SelectArtistHeader(st_group.name.clone()),
                crate::gui::app::Message::ToggleArtistExpansion(st_group.name.clone()),
            )
        )
        .width(Length::Fill)
        .height(Length::Fixed(header_h))
        .padding([0, 15])
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
) -> Element<'a, crate::gui::app::Message>
where
    F: Fn(&crate::db::database::SongData, usize, bool) -> Element<'a, crate::gui::app::Message> + 'a,
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

    let view_min_raw = manager.last_viewport.as_ref().map(|v| v.absolute_offset().y).unwrap_or(0.0);
    let viewport_h = manager.last_viewport.as_ref().map(|v| v.bounds().height).unwrap_or(800.0);

    let mut total_content_h = 0.0;
    
    // Estructura para agrupar canciones por álbum mantieniendo el orden de las canciones
    struct AlbumGroup<'a> {
        album_name: String,
        songs: Vec<(&'a crate::db::database::SongData, usize)>, // (song, global_idx)
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
            
            albums_map.push(AlbumGroup {
                album_name: alb_name,
                songs: vec![(song, global_song_idx)],
                duration_secs: song.duration_secs.unwrap_or(0.0),
            });
            global_song_idx += 1;
        }

        let mut artist_h = header_h;
        if !is_artist_collapsed {
            for alb in &albums_map {
                let is_album_expanded = !manager.collapsed_albums.contains(alb.album_name.as_str());
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
                    let is_album_expanded = !manager.collapsed_albums.contains(alb.album_name.as_str());
                    let card_h: f32 = if is_album_expanded { 323.0 } else { 0.0 }; 
                    let right_h = if is_album_expanded {
                        album_header_h + alb.songs.len() as f32 * row_height
                    } else {
                        album_header_h
                    };
                    let block_h = card_h.max(right_h) + 10.0; // padding inferior - must match pre-accumulator
                    let block_end = current_y + block_h;

                    if block_end >= render_min && current_y <= render_max {
                        let artist_name = alb.songs.first().and_then(|(s,_)| s.artist.clone()).unwrap_or_default();
                        let genre = alb.songs.first().and_then(|(s,_)| s.genre.clone()).unwrap_or_default();
                        let year = alb.songs.first().and_then(|(s,_)| s.release_year.clone()).unwrap_or_default();
                        visible_elements.push(VirtualRow::AlbumBlock(alb, is_album_expanded, artist_name, genre, year));
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
        list_col = list_col.push(Space::new().height(Length::Fixed(top_space)));
    }

    for element in visible_elements {
        match element {
            VirtualRow::ArtistHeader(group, is_collapsed) => {
                let is_header_explicitly_selected = manager.selected_header.as_ref() == Some(&group.name) 
                    && manager.selected_album.is_none() 
                    && manager.selected_song_idx.is_none();
                let header = artist_header_widget(
                    group.name.clone(),
                    is_collapsed,
                    is_header_explicitly_selected,
                    group.albums.len(),
                    group.songs.len(),
                    group.duration_secs,
                    32.0, // DetailedList always uses 32px headers
                    crate::gui::app::Message::SelectArtistHeader(group.name.clone()),
                    crate::gui::app::Message::ToggleArtistExpansion(group.name.clone()),
                );
                list_col = list_col.push(header);
            }
            VirtualRow::AlbumBlock(alb, is_expanded, artist_name, genre, year) => {
                // Buscamos el cover en cached_albums usando el nombre del álbum (a.1)
                let album_id = alb.album_name.clone();
                let cover_path = manager.cached_albums.as_ref()
                    .and_then(|albums| albums.iter().find(|a| a.1 == album_id).and_then(|a| a.5.clone()));

                let card_wrapper: Element<'a, crate::gui::app::Message> = if let Some(path) = cover_path {
                    container(
                        iced::widget::image(iced::widget::image::Handle::from_path(path))
                            .width(Length::Fixed(card_w - 30.0))
                            .height(Length::Fixed(card_w - 30.0))
                            .content_fit(iced::ContentFit::Cover)
                            .border_radius(8.0)
                    ).width(Length::Fixed(card_w - 30.0))
                     .height(Length::Fixed(card_w - 30.0))
                     .style(|_t| container::Style::default().border(iced::Border { radius: 8.0.into(), ..Default::default() }))
                     .clip(true)
                     .into()
                } else {
                    let icon = iced::widget::svg(iced::widget::svg::Handle::from_path("assets/icons/album.svg"))
                        .width(Length::Fixed(64.0)).height(Length::Fixed(64.0))
                        .style(|_t: &Theme, _s| iced::widget::svg::Style { color: Some(COLOR_TEXT_SECONDARY) });
                    let title_text = text("AuDoxiDY").font(crate::gui::theme::FONT_STAGE_WANDER).size(11).color(COLOR_TEXT_SECONDARY);
                    container(column![icon, title_text].align_x(Alignment::Center).spacing(5))
                        .width(Length::Fixed(card_w - 30.0)).height(Length::Fixed(card_w - 30.0)).center_x(Length::Fill).center_y(Length::Fill)
                        .style(|_t: &Theme| container::Style::default().background(Color::from(COLOR_BG)).border(iced::Border { radius: 8.0.into(), ..Default::default() }))
                        .clip(true)
                        .into()
                };

                let info_col = column![
                    text(truncate_text(&artist_name, 24)).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
                    text(truncate_text(&alb.album_name, 24)).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
                    text(truncate_text(&genre, 24)).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
                    text(year.clone()).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
                ].spacing(2).width(Length::Fill);
                let is_album_explicitly_selected = manager.selected_album.as_deref() == Some(alb.album_name.as_str()) && manager.selected_song_idx.is_none();
                let is_song_selected_in_album = manager.selected_song_idx.map(|idx| alb.songs.iter().any(|(_, i)| *i == idx)).unwrap_or(false);
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
                .on_press(crate::gui::app::Message::SelectAlbum(album_id.clone()))
                .interaction(iced::mouse::Interaction::Pointer);

                // Lado Derecho (Album Header + Canciones)
                let right_h = if is_expanded {
                    album_header_h + alb.songs.len() as f32 * row_height
                } else {
                    album_header_h
                };
                let _block_h = (card_w + 30.0).max(right_h);
                let chevron = if !is_expanded { "arrow-down-chevron.svg" } else { "arrow-up-chevron.svg" };
                let time_str = format_duration(alb.duration_secs);
                
                let album_title_row = if is_album_explicitly_selected {
                    row![text(alb.album_name.clone()).size(15).font(FONT_INTER_SANS_MEDIUM).color(COLOR_TEXT_PRIMARY),
                         text(" •").size(15).font(FONT_INTER_SANS_MEDIUM).color(COLOR_TEXT_PRIMARY)]
                } else {
                    row![text(alb.album_name.clone()).size(15).font(FONT_INTER_SANS_MEDIUM).color(COLOR_TEXT_PRIMARY)]
                }.align_y(Alignment::Center);

                let alb_header_content = row![
                    album_title_row,
                    Space::new().width(Length::Fill),
                    text(format!("{} Canciones | {}", alb.songs.len(), time_str))
                        .size(13).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM),
                    Space::new().width(15),
                    chevron_btn(chevron, crate::gui::app::Message::ToggleAlbumExpansion(album_id.clone()), 32.0, 28.0),
                ].align_y(Alignment::Center).padding([0, 15]);

                let alb_header = container(mouse_area(alb_header_content).on_press(crate::gui::app::Message::SelectAlbum(album_id.clone())))
                    .width(Length::Fill)
                    .height(Length::Fixed(album_header_h))
                    .center_y(Length::Fill)
                    .style(|_t| container::Style::default().background(Color::from(COLOR_CONTRAST)));

                let mut right_col = column![alb_header].spacing(0).width(Length::Fill);

                if is_expanded {
                    for (song, song_i) in alb.songs {
                        let is_song_selected = manager.selected_song_idx == Some(song_i);
                        let song_row = row_builder(song, song_i, is_song_selected);
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
        list_col = list_col.push(Space::new().height(Length::Fixed(bottom_space)));
    }

    let main_scroll = scrollable(container(list_col).width(Length::Fill).padding([0, 15]))
        .width(Length::Fill).height(Length::Fill)
        .direction(iced::widget::scrollable::Direction::Vertical(
            iced::widget::scrollable::Scrollbar::new().width(4).margin(0).scroller_width(4)
        ))
        .id(crate::gui::library::LIBRARY_SCROLL_ID.clone())
        .on_scroll(crate::gui::app::Message::LibraryScroll)
        .style(crate::gui::widgets::custom_scrollbar_style);

    let content: Element<'a, crate::gui::app::Message> = if let Some((st_group, is_collapsed)) = sticky_artist_info {
        let is_header_explicitly_selected = manager.selected_header.as_ref() == Some(&st_group.name)
            && manager.selected_album.is_none()
            && manager.selected_song_idx.is_none();
        
        let sticky_overlay = container(
            artist_header_widget(
                st_group.name.clone(),
                is_collapsed,
                is_header_explicitly_selected,
                st_group.albums.len(),
                st_group.songs.len(),
                st_group.duration_secs,
                32.0, // DetailedList always uses 32px headers
                crate::gui::app::Message::SelectArtistHeader(st_group.name.clone()),
                crate::gui::app::Message::ToggleArtistExpansion(st_group.name.clone()),
            )
        )
        .width(Length::Fill)
        .height(Length::Fixed(32.0))
        .padding([0, 15])
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
