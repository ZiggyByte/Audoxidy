use iced::{
    widget::{container, column, row, text, button, slider, Stack, opaque, svg, mouse_area, Space},
    Element, Length, Alignment, Color, Theme
};
use crate::audio::AudioManager;
use crate::gui::app::Message;
use crate::gui::theme::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverZone {
    None,
    Previous,
    PlayPause,
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    Minimize,
    Maximize,
    Close,
}

pub struct PlayerUiState {
    pub hover_zone: HoverZone,
    pub showing_volume: Option<f32>,
    pub is_menu_open: bool,
    pub volume_tick_id: u64,
    pub tick_count: u64,
    pub current_art_len: usize,
    pub cached_art_handle: Option<iced::widget::image::Handle>,
}

impl Default for PlayerUiState {
    fn default() -> Self {
        Self {
            hover_zone: HoverZone::None,
            showing_volume: None,
            is_menu_open: false,
            volume_tick_id: 0,
            tick_count: 0,
            current_art_len: 0,
            cached_art_handle: None,
        }
    }
}

pub fn view<'a>(
    audio_manager: &'a AudioManager,
    ui_state: &'a PlayerUiState,
) -> Element<'a, Message> {
    let state = audio_manager.get_state();
    let bounds = Length::Fixed(400.0);

    // --- Capa 1 y 2: Fondo y Cover Art / Logo ---
    let background_layer: Element<'a, Message> = if let Some(handle) = ui_state.cached_art_handle.clone() {
        let img = iced::widget::image(handle)
            .width(bounds)
            .height(bounds)
            .content_fit(iced::ContentFit::Cover);
        container(img).width(bounds).height(bounds).into()
    } else {
        container(
            text("AuDoxiDY")
                .font(FONT_STAGE_WANDER)
                .size(40)
                .color(COLOR_TEXT_PRIMARY)
        )
        .width(bounds)
        .height(bounds)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .padding(20)
        .style(|_theme: &Theme| container::Style::default().background(COLOR_BG))
        .into()
    };

    // --- Capa 3: Oscurecimiento ---
    let blackout_layer: Element<'a, Message> = container(opaque(
        Space::new().width(Length::Fill).height(Length::Fill)
    ))
    .width(bounds)
    .height(bounds)
    .style(|_theme: &Theme| container::Style::default().background(Color::from_rgba(0.0, 0.0, 0.0, 0.3)))
    .into();

    // --- Capa 4: Información y Marquesinas (Top + Bottom) ---
    // Botones Top sin fondo usando styling transparente
    fn transparent_btn<'b>(icon: &str) -> iced::widget::Button<'b, Message> {
        button(svg(svg::Handle::from_path(format!("assets/icons/{}", icon))).width(32).height(32))
            .padding(0)
            .style(iced::widget::button::text)
    }

    let top_row = row![
        transparent_btn("menu.svg").on_press(Message::ToggleMenu),
        Space::new().width(Length::Fill),
        // Aquí irían los canales ej: text("5.1").color(Color::WHITE),
        Space::new().width(Length::Fill),
        transparent_btn("minimize.svg").on_press(Message::PlayerWindowAction(WindowAction::Minimize)),
        transparent_btn("maximize.svg").on_press(Message::PlayerWindowAction(WindowAction::Maximize)),
        transparent_btn("close.svg").on_press(Message::PlayerWindowAction(WindowAction::Close)),
    ]
    .height(Length::Fixed(40.0))
    .align_y(Alignment::Center)
    .padding([0, 10]); // padding horizontal

    let is_playing_or_paused = state.is_playing || state.current_pos_sec > 0.0;
    
    let apply_marquee = |text_str: &str, limit: usize| -> String {
        let chars: Vec<char> = text_str.chars().collect();
        if chars.len() <= limit { return text_str.to_string(); }
        let tick = ui_state.tick_count;
        let offset = (tick / 2) as usize % (chars.len() + 10);
        if offset < chars.len() {
            let end = (offset + limit).min(chars.len());
            let mut s: String = chars[offset..end].iter().collect();
            if offset + limit > chars.len() {
                s.push_str("   ");
                let needed = (offset + limit) - chars.len();
                if needed > 3 {
                    let rem = needed - 3;
                    s.push_str(&chars[0..rem.min(chars.len())].iter().collect::<String>());
                }
            }
            s
        } else { chars[0..limit.min(chars.len())].iter().collect() }
    };

    let info_col = if is_playing_or_paused {
        column![
            container(
                text(apply_marquee(&state.title, 32))
                    .size(18)
                    .color(Color::WHITE)
            )
            .height(Length::Fixed(35.0))
            .center_y(Length::Fill)
            .width(Length::Fill),
            row![
                container(
                    text(apply_marquee(&state.artist, 30))
                        .size(16)
                        .color(Color::WHITE)
                )
                .height(Length::Fixed(30.0))
                .center_y(Length::Fill)
                .width(Length::Fill),
                text(format!("{}:{:02}", state.current_pos_sec as u32 / 60, state.current_pos_sec as u32 % 60))
                    .size(16)
                    .color(Color::WHITE)
            ]
            .height(Length::Fixed(30.0))
            .align_y(Alignment::Center)
        ]
        .width(Length::Fill)
    } else {
        column![Space::new().height(Length::Fixed(65.0))]
        .width(Length::Fill)
    };

    let progress = if state.total_duration_sec > 0.0 {
        state.current_pos_sec as f32 / state.total_duration_sec as f32
    } else { 0.0 };
    
    let progress_bar = container(
        slider(0.0..=1.0, progress, move |v| Message::SeekTo(v * state.total_duration_sec as f32))
            .step(0.001)
            .style(move |theme: &Theme, status| {
                let mut st = iced::widget::slider::default(theme, status);
                st.handle.background = Color::TRANSPARENT.into();
                st.handle.border_color = Color::TRANSPARENT;
                if let iced::widget::slider::HandleShape::Circle { radius } = &mut st.handle.shape {
                    *radius = 0.0;
                }
                st.rail.width = 30.0;
                st.rail.backgrounds = (COLOR_ACCENT.into(), Color::from_rgba(1.0, 1.0, 1.0, 0.2).into());
                st
            })
    )
    .height(Length::Fixed(30.0))
    .center_y(Length::Fill)
    .padding([0, 0])
    .style(|_t: &Theme| container::Style::default().background(Color::from_rgba(0.0, 0.0, 0.0, 0.3)));

    let capa4: Element<'a, Message> = column![
        top_row,
        Space::new().height(Length::Fixed(255.0)),
        info_col.padding([0, 10]),
        progress_bar
    ]
    .width(bounds)
    .height(bounds)
    .into();

    // --- Capa 5: Controles de Audio ---
    let make_zone = |icon: &str, zone: HoverZone, action: Message, is_hovered: bool| {
        let content: Element<'a, Message> = if is_hovered {
            container(svg(svg::Handle::from_path(format!("assets/icons/{}", icon))).width(32).height(32))
                .width(Length::Fill).height(Length::Fill)
                .center_x(Length::Fill).center_y(Length::Fill)
                .style(|_t: &Theme| container::Style::default().background(Color::from_rgba(COLOR_CONTRAST.r, COLOR_CONTRAST.g, COLOR_CONTRAST.b, 0.5)))
                .into()
        } else {
            container(Space::new()).width(Length::Fill).height(Length::Fill).into()
        };
        
        let area: Element<'a, Message> = mouse_area(button(content).on_press(action).padding(0).style(iced::widget::button::text).width(Length::Fill).height(Length::Fill))
            .on_enter(Message::PlayerHoverZone(zone))
            .on_exit(Message::PlayerHoverZone(HoverZone::None))
            .into();
        area
    };

    let play_txt = if state.is_playing { "pause-straight-fill.svg" } else { "play-straight-fill.svg" };

    let controls_content: Element<'a, Message> = match ui_state.showing_volume {
        Some(vol) => {
            let icon = if vol == 0.0 { "volume-off-straight-fill.svg" }
                       else if vol < 50.0 { "volume-down-straight-fill.svg" }
                       else { "volume-up-straight-fill.svg" };
            
            container(
                column![
                    svg(svg::Handle::from_path(format!("assets/icons/{}", icon))).width(32).height(32),
                    text(format!("{:.0}", vol)).size(16).color(Color::WHITE)
                ]
                .align_x(Alignment::Center)
                .spacing(5)
            )
            .width(Length::Fill).height(Length::Fill)
            .center_x(Length::Fill).center_y(Length::Fill)
            // Cubrir toda el área con 40%
            .style(|_t: &Theme| container::Style::default().background(Color::from_rgba(COLOR_CONTRAST.r, COLOR_CONTRAST.g, COLOR_CONTRAST.b, 0.4)))
            .into()
        },
        None => {
            row![
                make_zone("skip-previous-straight-fill.svg", HoverZone::Previous, Message::PreviousTrack, ui_state.hover_zone == HoverZone::Previous),
                make_zone(play_txt, HoverZone::PlayPause, Message::PlayPause, ui_state.hover_zone == HoverZone::PlayPause),
                make_zone("skip-next-straight-fill.svg", HoverZone::Next, Message::NextTrack, ui_state.hover_zone == HoverZone::Next)
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        }
    };

    let capa5: Element<'a, Message> = container(VolumeScrollArea::new(controls_content, Message::PlayerScroll))
        .width(bounds)
        .height(Length::Fixed(320.0)) // 2/10 a 9/10
        .into();

    let capa5_positioned: Element<'a, Message> = column![
        Space::new().height(Length::Fixed(40.0)),
        capa5,
        Space::new().height(Length::Fixed(40.0))
    ]
    .width(bounds).height(bounds).into();

    // --- Menú Desplegable ---
    let dropdown: Element<'a, Message> = if ui_state.is_menu_open {
        let mk_menu_btn = |txt: &'a str, msg: Message| {
            button(text(txt.to_string()).color(COLOR_TEXT_PRIMARY).size(12))
                .width(Length::Fill)
                .padding([5, 15])
                .on_press(msg)
                .style(iced::widget::button::text)
        };
        
        let menu_box = container(
            column![
                mk_menu_btn("Acerca", Message::ToggleMenu),
                mk_menu_btn("Abrir Archivo", Message::ToggleMenu),
                mk_menu_btn("Abrir Carpeta", Message::OpenFolderPicker),
                mk_menu_btn("Biblioteca", Message::ToggleMenu),
                mk_menu_btn("Lista de Reproduccion", Message::ToggleMenu),
                mk_menu_btn("Lirycs", Message::ToggleMenu),
                mk_menu_btn("Apagado Automatico", Message::ToggleMenu),
                container(Space::new().height(1)).width(Length::Fill).style(|_t: &Theme| container::Style::default().background(COLOR_TEXT_SECONDARY)),
                mk_menu_btn("Configuración de Audio", Message::ToggleAudioCenter),
                mk_menu_btn("Ecualizador", Message::ToggleAudioCenter), 
                mk_menu_btn("Efectos de Audio", Message::ToggleAudioCenter), 
                container(Space::new().height(1)).width(Length::Fill).style(|_t: &Theme| container::Style::default().background(COLOR_TEXT_SECONDARY)),
                mk_menu_btn("Personalizacion", Message::ToggleMenu),
                mk_menu_btn("Preferencias", Message::ToggleMenu),
                mk_menu_btn("Complementos", Message::ToggleMenu),
                mk_menu_btn("Salir", Message::PlayerWindowAction(WindowAction::Close)),
            ]
        )
        .width(Length::Fixed(150.0))
        .style(|_t: &Theme| container::Style::default().background(COLOR_BG));

        let underlay = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
            .on_press(Message::ToggleMenu);

        let menu_wrapper = container(menu_box)
            .width(bounds) // el wrapper ocupa todo
            .height(bounds)
            .padding(iced::Padding { top: 40.0, right: 0.0, bottom: 0.0, left: 15.0 });

        container(
            Stack::new().push(underlay).push(menu_wrapper)
        )
        .width(bounds).height(bounds).into()
    } else {
        Space::new().into()
    };

    // Apilar capas en el Stack
    container(
        Stack::new()
            .push(background_layer)
            .push(blackout_layer)
            .push(capa4)
            .push(capa5_positioned)
            .push(dropdown)
    )
    .width(bounds)
    .height(bounds)
    .into()
}

// ==========================================
// Custom Widget Area
// ==========================================

use iced::advanced::{widget::Tree, Widget, Layout, renderer, mouse, Clipboard, Shell};
use iced::{Event, Rectangle, Size};

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

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer> for VolumeScrollArea<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &iced::advanced::layout::Limits) -> iced::advanced::layout::Node {
        self.content.as_widget_mut().layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(&self, tree: &Tree, renderer: &mut Renderer, theme: &Theme, style: &renderer::Style, layout: Layout<'_>, cursor: mouse::Cursor, viewport: &Rectangle) {
        self.content.as_widget().draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport)
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content))
    }

    fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn iced::advanced::widget::Operation) {
        self.content.as_widget_mut().operate(&mut tree.children[0], layout, renderer, operation)
    }

    fn update(&mut self, tree: &mut Tree, event: &Event, layout: Layout<'_>, cursor: mouse::Cursor, renderer: &Renderer, clipboard: &mut dyn Clipboard, shell: &mut Shell<'_, Message>, viewport: &Rectangle) {
        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
            if cursor.is_over(layout.bounds()) {
                let d = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 10.0,
                };
                shell.publish((self.on_scroll)(d));
                return; // Evita propagar al contenido hijo si ya lo procesamos
            }
        }
        self.content.as_widget_mut().update(&mut tree.children[0], event, layout, cursor, renderer, clipboard, shell, viewport)
    }

    fn mouse_interaction(&self, tree: &Tree, layout: Layout<'_>, cursor: mouse::Cursor, viewport: &Rectangle, renderer: &Renderer) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(&'b mut self, tree: &'b mut Tree, layout: Layout<'b>, renderer: &Renderer, viewport: &Rectangle, translation: iced::Vector) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}

impl<'a, Message, Theme, Renderer> From<VolumeScrollArea<'a, Message, Theme, Renderer>> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(area: VolumeScrollArea<'a, Message, Theme, Renderer>) -> Self {
        Element::new(area)
    }
}
