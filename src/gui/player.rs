use iced::{
    widget::{container, column, row, text, button, slider, Stack, opaque, svg, mouse_area, Space},
    Element, Length, Alignment, Color, Theme
};
use crate::audio::AudioManager;
use crate::gui::app::Message;
use crate::gui::theme::*;
use crate::gui::widgets::{VolumeScrollArea, action_icon_button};
use crate::utils::format_duration;

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
    pub current_art_id: String,
    pub current_cover_path: String,
    pub cached_art_handle: Option<iced::widget::image::Handle>,
    pub prefetched_next_handle: Option<iced::widget::image::Handle>,
    pub mouse_pos: Option<iced::Point>,
    pub active_until_tick: u64,
    pub is_active: bool,
    pub volume_clearing: bool,

    // Caché para marquesinas optimizadas
    pub last_title: String,
    pub last_artist: String,
    pub title_chars: Vec<char>,
    pub artist_chars: Vec<char>,
    pub display_title: String,
    pub display_artist: String,
}

impl Default for PlayerUiState {
    fn default() -> Self {
        Self {
            hover_zone: HoverZone::None,
            showing_volume: None,
            is_menu_open: false,
            volume_tick_id: 0,
            tick_count: 0,
            current_art_id: String::new(),
            current_cover_path: String::new(),
            cached_art_handle: None,
            prefetched_next_handle: None,
            mouse_pos: None,
            active_until_tick: 0,
            is_active: false,
            volume_clearing: false,
            last_title: String::new(),
            last_artist: String::new(),
            title_chars: Vec::new(),
            artist_chars: Vec::new(),
            display_title: String::new(),
            display_artist: String::new(),
        }
    }
}

pub fn view<'a>(
    audio_manager: &'a AudioManager,
    ui_state: &'a PlayerUiState,
) -> Element<'a, Message> {
    let state = audio_manager.get_state();
    let bounds = Length::Fixed(400.0);

    let background_layer = crate::gui::widgets::album_art_widget(
        None, // No pasamos el path aquí ya que el player gestiona su propio handle precargado
        None, // No pasamos los bytes aquí ya que el player gestiona su propio handle precargado
        ui_state.cached_art_handle.clone(),
        crate::gui::widgets::PlaceholderStyle::Player,
        bounds,
        0.0, // El reproductor principal es cuadrado (sin redondeo)
    );

    // --- Capa 3: Oscurecimiento ---
    let blackout_layer: Element<'a, Message> = container(opaque(
        Space::new().width(Length::Fill).height(Length::Fill)
    ))
    .width(bounds)
    .height(bounds)
    .style(|_theme: &Theme| container::Style::default().background(Color::from_rgba(0.0, 0.0, 0.0, 0.30)))
    .into();

    // --- Capa 4: Información y Marquesinas (Top + Bottom) ---
    let top_row = row![
        action_icon_button("menu.svg", 31, Message::ToggleMenu),
        Space::new().width(Length::Fill),
        // Aquí irían los canales ej: text("5.1").color(Color::WHITE),
    ]
    .height(Length::Fixed(40.0))
    .align_y(Alignment::Center)
    .padding([0, 8]); // padding horizontal

    let is_playing_or_paused = state.is_playing || state.current_pos_sec > 0.0;

    let title_el = container(
        text(&ui_state.display_title).size(18).color(Color::WHITE).font(FONT_INTER_SANS_NORMAL)
        .shaping(iced::widget::text::Shaping::Advanced).wrapping(iced::widget::text::Wrapping::None)
    ).padding([0, 2]).height(Length::Fixed(35.0)).center_y(Length::Fill).width(Length::Fill);

    let title_widget: Element<'a, Message> = if state.title.len() > 35 {
        iced::widget::tooltip(title_el, container(text(state.title.clone()).size(13).color(Color::WHITE)).padding(6).style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST).border(iced::Border::default().width(1.0).color(COLOR_TEXT_SECONDARY))), iced::widget::tooltip::Position::FollowCursor).into()
    } else { title_el.into() };

    let artist_el = container(
        text(&ui_state.display_artist).size(15).color(Color::WHITE).font(FONT_INTER_SANS_NORMAL)
        .shaping(iced::widget::text::Shaping::Advanced).wrapping(iced::widget::text::Wrapping::None)
    ).padding([0, 2]).height(Length::Fixed(30.0)).center_y(Length::Fill).width(Length::Fill);

    let artist_widget: Element<'a, Message> = if state.artist.len() > 35 {
        iced::widget::tooltip(artist_el, container(text(state.artist.clone()).size(13).color(Color::WHITE)).padding(6).style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST).border(iced::Border::default().width(1.0).color(COLOR_TEXT_SECONDARY))), iced::widget::tooltip::Position::FollowCursor).into()
    } else { artist_el.into() };

    let info_col = if is_playing_or_paused {
        column![
            Space::new().height(Length::Fixed(262.0)),
            container(title_widget).height(Length::Fixed(40.0)).center_y(Length::Fill).padding([0, 10]),
            container(
                row![
                    artist_widget,
                    Space::new().width(Length::Fill),
                    container(
                        text(format_duration(state.current_pos_sec))
                            .size(14).color(Color::WHITE).font(FONT_INTER_SANS_NORMAL)
                    ).padding([0, 2])
                ].align_y(Alignment::Center)
            ).height(Length::Fixed(36.0)).padding([0, 10])
        ]
        .width(Length::Fill)
        .height(Length::Fixed(320.0))
    } else {
        column![Space::new().height(Length::Fixed(320.0))]
        .width(Length::Fill)
    };

    let progress = if state.total_duration_sec > 0.0 {
        state.current_pos_sec as f32 / state.total_duration_sec as f32
    } else { 0.0 };
    
    let slider_el = slider(0.0..=1.0, progress, move |v| Message::SeekTo(v * state.total_duration_sec as f32))
        .step(0.001)
        .style(move |theme: &Theme, status| {
            let mut st = iced::widget::slider::default(theme, status);
            st.handle.background = Color::TRANSPARENT.into();
            st.handle.border_color = Color::TRANSPARENT;
            if let iced::widget::slider::HandleShape::Circle { radius } = &mut st.handle.shape {
                *radius = 0.0;
            }
            st.rail.width = 5.0;
            st.rail.backgrounds = (Color::WHITE.into(), COLOR_CONTRAST.into());
            st
        });

    let progress_bar = mouse_area(
        container(slider_el).padding([15, 13]).height(Length::Fill).center_y(Length::Fill)
    )
    .interaction(iced::mouse::Interaction::Idle);

    let progress_container = container(progress_bar)
        .height(Length::Fixed(40.0))
        .center_y(Length::Fill)
        .style(|_t: &Theme| container::Style::default().background(Color::from_rgba(0.0, 0.0, 0.0, 0.25)));

    let capa4: Element<'a, Message> = column![
        top_row,
        info_col,
        progress_container
    ]
    .width(bounds)
    .height(bounds)
    .into();

    let make_zone = |icon: &str, action: Message, is_hovered: bool| -> Element<'a, Message> {
        let size = if icon == "pause-straight-fill.svg" { 34 } else { 42 };
        
        let visual_content = if is_hovered {
            let pad = if icon == "pause-straight-fill.svg" { 7 } else { 3 };
            container(svg(svg::Handle::from_path(format!("assets/icons/{}", icon))).width(size).height(size))
                .padding(pad)
                .style(|_t: &Theme| container::Style::default()
                    .background(Color::from_rgba(COLOR_CONTRAST.r, COLOR_CONTRAST.g, COLOR_CONTRAST.b, 0.5))
                    .border(iced::Border::default().rounded(30.0))
                )
        } else {
            container(Space::new().width(size).height(size))
        };
        
        let clickable_area = mouse_area(
            container(visual_content)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
        )
        .on_press(action)
        .interaction(iced::mouse::Interaction::Idle);

        clickable_area.into()
    };

    let play_txt = if state.is_playing { "pause-straight-fill.svg" } else { "play-straight-fill.svg" };

    let computed_zone = if let Some(pos) = ui_state.mouse_pos {
        if ui_state.is_active && ui_state.showing_volume.is_none() && pos.x <= 400.0 && pos.y <= 400.0 && pos.y >= 40.0 && pos.y <= 360.0 && !ui_state.is_menu_open {
            if pos.x < 133.3 { HoverZone::Previous }
            else if pos.x < 266.6 { HoverZone::PlayPause }
            else { HoverZone::Next }
        } else { HoverZone::None }
    } else { HoverZone::None };

    let base_controls = row![
        make_zone("skip-previous-straight-fill.svg", Message::PreviousTrack, computed_zone == HoverZone::Previous),
        make_zone(play_txt, Message::PlayPause, computed_zone == HoverZone::PlayPause),
        make_zone("skip-next-straight-fill.svg", Message::NextTrack, computed_zone == HoverZone::Next)
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    let controls_content: Element<'a, Message> = match ui_state.showing_volume {
        Some(vol) => {
            let icon = if vol == 0.0 { "volume-off-straight-fill.svg" }
                       else if vol < 50.0 { "volume-down-straight-fill.svg" }
                       else { "volume-up-straight-fill.svg" };
            
            let vol_ui = container(
                container(
                    column![
                        svg(svg::Handle::from_path(format!("assets/icons/{}", icon))).width(44).height(44),
                        text(format!("{:.0}", vol)).size(20).color(Color::WHITE).font(FONT_INTER_SANS_MEDIUM)
                    ]
                    .align_x(Alignment::Center)
                    .spacing(0)
                )
                .padding(iced::Padding { top: 8.0, right: 22.0, bottom: 13.0, left: 22.0 })
                .style(|_t: &Theme| container::Style::default()
                    .background(Color::from_rgba(COLOR_CONTRAST.r, COLOR_CONTRAST.g, COLOR_CONTRAST.b, 0.5))
                    .border(iced::Border::default().rounded(50.0))
                )
            )
            .width(Length::Fill).height(Length::Fill)
            .center_x(Length::Fill).center_y(Length::Fill);

            Stack::new().push(base_controls).push(vol_ui).into()
        },
        None => base_controls.into(),
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
            button(text(txt.to_string()).color(COLOR_TEXT_PRIMARY).size(13).font(FONT_INTER_SANS_MEDIUM))
                .width(Length::Fill)
                .padding([4, 0])
                .on_press(msg)
                .style(iced::widget::button::text)
        };
        
        let menu_box = container(
            column![
                mk_menu_btn("Abrir Archivo", Message::ToggleMenu), // Cierran y delegan si lo implemento luego
                mk_menu_btn("Abrir Carpeta", Message::OpenFolderPicker),
                mk_menu_btn("Biblioteca", Message::ToggleMenu),
                mk_menu_btn("Lista de Reproduccion", Message::ToggleMenu),
                mk_menu_btn("Lirycs", Message::ToggleMenu),
                Space::new().height(Length::Fixed(2.0)),
                container(Space::new().height(1)).width(Length::Fill).style(|_t: &Theme| container::Style::default().background(COLOR_TEXT_SECONDARY)),
                Space::new().height(Length::Fixed(2.0)),
                mk_menu_btn("Configuración de Audio", Message::ToggleAudioCenter),
                mk_menu_btn("Ecualizador", Message::ToggleAudioCenter), 
                mk_menu_btn("Efectos de Audio", Message::ToggleAudioCenter), 
                Space::new().height(Length::Fixed(2.0)),
                container(Space::new().height(1)).width(Length::Fill).style(|_t: &Theme| container::Style::default().background(COLOR_TEXT_SECONDARY)),
                Space::new().height(Length::Fixed(2.0)),
                mk_menu_btn("Apagado Automatico", Message::ToggleMenu),
                mk_menu_btn("Personalizacion", Message::ToggleMenu),
                mk_menu_btn("Preferencias", Message::ToggleMenu),
                mk_menu_btn("Complementos", Message::ToggleMenu),
                mk_menu_btn("Acerca", Message::ToggleMenu),
                mk_menu_btn("Salir", Message::PlayerWindowAction(WindowAction::Close)),
            ]
        )
        .width(Length::Fixed(180.0))
        .padding(15)
        .style(|_t: &Theme| container::Style::default()
            .background(COLOR_BG)
            .border(iced::Border::default().rounded(8.0).width(2.0).color(COLOR_ACCENT))
        );

        let underlay = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
            .interaction(iced::mouse::Interaction::Idle)
            .on_press(Message::ToggleMenu);

        let menu_wrapper = container(menu_box)
            .width(bounds) // el wrapper ocupa todo
            .height(bounds)
            .padding(iced::Padding { top: 35.0, right: 0.0, bottom: 0.0, left: 15.0 });

        container(
            Stack::new().push(underlay).push(menu_wrapper)
        )
        .width(bounds).height(bounds).into()
    } else {
        Space::new().into()
    };

    let mut main_stack = Stack::new()
            .push(background_layer)
            .push(blackout_layer)
            .push(capa4)
            .push(capa5_positioned)
            .push(dropdown);

    // Dynamic Tooltip Cursor exacto para barra de progreso
    if let Some(pos) = ui_state.mouse_pos {
        if ui_state.is_active && pos.y >=360.0 && pos.y <= 400.0 && pos.x >= 13.0 && pos.x <= 387.0 && state.total_duration_sec > 0.0 {
            let frac = ((pos.x - 13.0) / 376.0).clamp(0.0, 1.0);
            let tooltip_txt = format_duration(frac as f64 * state.total_duration_sec);
            
            let tooltip_box = container(text(tooltip_txt).size(12).color(Color::WHITE))
                .padding(4)
                .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST).border(iced::Border::default().width(1.0).color(COLOR_TEXT_SECONDARY)));
            
            let hover_tooltip = container(tooltip_box)
                .width(Length::Fill).height(Length::Fill)
                .padding(iced::Padding { top: pos.y - 30.0, right: 0.0, bottom: 0.0, left: pos.x - 15.0 });
            
            main_stack = main_stack.push(hover_tooltip);
        }
    }

    // Apilar capas en el contenedor final
    container(main_stack)
    .width(bounds)
    .height(bounds)
    .into()
}
