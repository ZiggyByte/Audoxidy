use iced::{
    widget::{button, column, container, row, scrollable, text, text_input, Space},
    Alignment, Color, Element, Length, Theme,
};
use crate::audio::AudioManager;
use crate::gui::app::Message;

// TODO: Consolidar colores globales en theme.rs
use crate::gui::theme::*;

#[derive(Clone, PartialEq, Debug)]
pub struct PlaylistItem {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_sec: f32,
    pub year: String,
    pub path: String,
}

pub struct PlaylistManager {
    pub lists: Vec<(String, Vec<PlaylistItem>)>, // (Nombre Lista, Canciones)
    pub active_list_idx: usize,
    pub selected_song_idx: Option<usize>,
    pub playing_song_idx: Option<usize>,
    pub shuffle_active: bool,
    pub repeat_mode: u8,
    pub search_query: String,
}

impl Default for PlaylistManager {
    fn default() -> Self {
        Self {
            lists: vec![
                ("Default".into(), Vec::new()), 
                ("Favoritos".into(), Vec::new()),
            ],
            active_list_idx: 0,
            selected_song_idx: None,
            playing_song_idx: None,
            shuffle_active: false,
            repeat_mode: 0,
            search_query: String::new(),
        }
    }
}

impl PlaylistManager {
    pub fn play_next(&mut self, audio_manager: &AudioManager) {
        if self.lists.is_empty() || self.lists[self.active_list_idx].1.is_empty() { return; }
        let list = &self.lists[self.active_list_idx].1;
        let len = list.len();
        if len == 0 { return; }

        if self.repeat_mode == 2 {
            if let Some(idx) = self.playing_song_idx {
                if let Err(e) = audio_manager.load_file(&list[idx].path) { tracing::error!("Error: {}", e); } else { audio_manager.play(); }
                return;
            }
        }

        let mut next_idx = 0;
        if self.shuffle_active {
            next_idx = fastrand::usize(0..len);
        } else if let Some(idx) = self.playing_song_idx {
            next_idx = idx + 1;
            if next_idx >= len {
                if self.repeat_mode == 1 { next_idx = 0; } else { return; }
            }
        }

        self.playing_song_idx = Some(next_idx);
        if let Err(e) = audio_manager.load_file(&list[next_idx].path) { tracing::error!("Error: {}", e); } else { audio_manager.play(); }
    }

    pub fn play_prev(&mut self, audio_manager: &AudioManager) {
        if self.lists.is_empty() || self.lists[self.active_list_idx].1.is_empty() { return; }
        let list = &self.lists[self.active_list_idx].1;
        let len = list.len();
        if len == 0 { return; }

        let mut next_idx = 0;
        if self.shuffle_active {
            next_idx = fastrand::usize(0..len);
        } else if let Some(idx) = self.playing_song_idx {
            if idx > 0 {
                next_idx = idx - 1;
            } else {
                if self.repeat_mode == 1 { next_idx = len - 1; } else { next_idx = 0; }
            }
        }

        self.playing_song_idx = Some(next_idx);
        if let Err(e) = audio_manager.load_file(&list[next_idx].path) { tracing::error!("Error: {}", e); } else { audio_manager.play(); }
    }
}

pub fn view<'a>(manager: &'a PlaylistManager, _audio_manager: &AudioManager) -> Element<'a, Message> {
    
    // Barra superior: Pestañas (mock simple por ahora)
    let tabs = row![
        text("Default").color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
        text("Favoritos").color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM)
    ].spacing(20).padding(15);
    
    let tabs_container = container(tabs)
        .width(Length::Fill)
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    // Stats de lista actual
    let active_list = &manager.lists[manager.active_list_idx].1;
    let total_secs: f32 = active_list.iter().map(|s| s.duration_sec).sum();
    let mins = (total_secs / 60.0).floor() as u32;
    let secs = (total_secs % 60.0).floor() as u32;
    
    let stats_header = row![
        text("Todas Las Canciones").size(14).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
        Space::new().width(Length::Fill),
        text(format!("{} pistas • {}:{:02}", active_list.len(), mins, secs)).size(13).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM)
    ]
    .padding(10)
    .align_y(Alignment::Center);

    let header_container = container(stats_header)
        .width(Length::Fill)
        .style(|_t: &Theme| container::Style::default().background(COLOR_BG));

    // Lista de canciones desplazable
    let mut songs_col = column![].spacing(5).padding(5);
    
    for (i, song) in active_list.iter().enumerate() {
        let is_playing = manager.playing_song_idx == Some(i);
        let color = if is_playing { COLOR_ACCENT } else { COLOR_TEXT_SECONDARY };
        
        // Formatear duración
        let s_mins = (song.duration_sec / 60.0).floor() as u32;
        let s_secs = (song.duration_sec % 60.0).floor() as u32;
        
        let song_row = row![
            text(format!("{:02}", i + 1)).color(COLOR_TEXT_SECONDARY).width(30).size(14).font(FONT_INTER_SANS_MEDIUM),
            column![
                text(song.title.clone()).color(color).size(14).font(FONT_INTER_SANS_MEDIUM),
                text(format!("{} • {} • {}", song.artist, song.album, song.year)).color(COLOR_TEXT_SECONDARY).size(13).font(FONT_INTER_SANS_MEDIUM)
            ].width(Length::Fill),
            text(format!("{}:{:02}", s_mins, s_secs)).color(color).size(13).font(FONT_INTER_SANS_MEDIUM)
        ].align_y(Alignment::Center);

        // Hacemos cada canción un botón transparente que emite el Message al hacer clic
        let btn = button(song_row)
            .width(Length::Fill)
            .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
            .on_press(Message::PlaySongIndex(i));
            
        songs_col = songs_col.push(btn);
    }
    
    let playlist_scroll = scrollable(songs_col).height(Length::Fill);

    let shuffle_txt = text("Shuffle").color(if manager.shuffle_active { COLOR_ACCENT } else { COLOR_TEXT_PRIMARY }).font(FONT_INTER_SANS_MEDIUM);
    let repeat_texts = ["Repeat (Off)", "Repeat (All)", "Repeat (One)"];
    let repeat_color = if manager.repeat_mode > 0 { COLOR_ACCENT } else { COLOR_TEXT_PRIMARY };
    let repeat_txt = text(repeat_texts[manager.repeat_mode as usize]).color(repeat_color).font(FONT_INTER_SANS_MEDIUM);

    let bottom_bar = row![
        text_input("Buscar...", &manager.search_query)
            .on_input(Message::SearchQueryChanged)
            .width(160)
            .font(FONT_INTER_SANS_MEDIUM),
        Space::new().width(Length::Fill),
        button(shuffle_txt).on_press(Message::ToggleShuffle).style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT)),
        button(repeat_txt).on_press(Message::ToggleRepeat).style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
    ]
    .padding([0, 10])
    .spacing(10)
    .align_y(Alignment::Center);

    let bottom_container = container(bottom_bar)
        .width(Length::Fill)
        .height(Length::Fixed(40.0))
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    // Ensamblar todo
    container(
        column![
            tabs_container,
            header_container,
            playlist_scroll,
            bottom_container
        ]
    )
    .width(Length::Fixed(400.0))
    .height(Length::Fill)
    .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
    .into()
}
