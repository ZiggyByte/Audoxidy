use iced::{
    widget::{button, column, container, row, scrollable, text, text_input, Space, image},
    Alignment, Color, Element, Length, Theme,
};
use std::sync::{Arc, Mutex};
use crate::db::Database;
use crate::gui::app::Message;
use crate::gui::theme::*;

/// ID estático para el scrollable de la biblioteca — garantiza que view y update usan EXACTAMENTE el mismo ID
pub static LIBRARY_SCROLL_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::unique());

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LibraryViewMode {
    Grid,
    DetailedList,
    ThumbnailList,
    SimpleList,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LibraryNavDir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LibrarySource {
    Local,
    Spotify,
    YouTube,
    Qobuz,
    Deezer,
    Tidal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortColumn {
    TrackNumber,
    Title,
    Artist,
    Album,
    Genre,
    Year,
    Duration,
    Format,
    SampleRate,
    Channels,
    BitDepth,
    Bitrate,
    Size,
}

impl SortColumn {
    pub fn as_str(&self) -> &'static str {
        match self {
            SortColumn::TrackNumber => "#",
            SortColumn::Title => "Título",
            SortColumn::Artist => "Artista",
            SortColumn::Album => "Álbum",
            SortColumn::Genre => "Género",
            SortColumn::Year => "Año",
            SortColumn::Duration => "Duración",
            SortColumn::Format => "Formato",
            SortColumn::SampleRate => "Muestreo",
            SortColumn::Channels => "Canales",
            SortColumn::BitDepth => "Profundidad",
            SortColumn::Bitrate => "Bits",
            SortColumn::Size => "Tamaño",
        }
    }
}

pub struct LibraryManager {
    pub view_mode: LibraryViewMode,
    pub source: LibrarySource,
    pub sort_column: Option<SortColumn>,
    pub sort_ascending: Option<bool>,
    pub search_query: String,
    pub expanded_album: Option<String>,
    pub expanded_album_songs: Option<Vec<crate::db::database::SongRecord>>,
    pub cached_albums: Option<Vec<(String, String, String, String, Option<String>)>>,
    pub view_menu_open: bool,
    pub add_menu_open: bool,
    pub total_songs: usize,
    pub total_albums: usize,
    pub total_duration_secs: f64,
    pub total_size_bytes: f64,
    
    pub column_widths: std::collections::HashMap<SortColumn, u16>,
    pub resizing_column: Option<SortColumn>,
    pub resizing_start_x: f32,
    pub resizing_start_w: u16,
    pub hovered_column: Option<SortColumn>,
    
    // Selection & keyboard navigation
    pub selected_album: Option<String>,
    pub selected_song_idx: Option<usize>,
    pub albums_per_row: std::cell::Cell<usize>,
    pub library_area_width: f32,
    pub last_viewport: Option<iced::widget::scrollable::Viewport>,
}

impl Default for LibraryManager {
    fn default() -> Self {
        let mut column_widths = std::collections::HashMap::new();
        column_widths.insert(SortColumn::TrackNumber, 30);
        column_widths.insert(SortColumn::Title, 285);
        column_widths.insert(SortColumn::Artist, 155);
        column_widths.insert(SortColumn::Album, 165);
        column_widths.insert(SortColumn::Genre, 135);
        column_widths.insert(SortColumn::Year, 50);
        column_widths.insert(SortColumn::Duration, 53);
        column_widths.insert(SortColumn::Format, 53);
        column_widths.insert(SortColumn::SampleRate, 80);
        column_widths.insert(SortColumn::Channels, 55);
        column_widths.insert(SortColumn::BitDepth, 60);
        column_widths.insert(SortColumn::Bitrate, 90);
        column_widths.insert(SortColumn::Size, 85);
        
        Self {
            view_mode: LibraryViewMode::Grid,
            source: LibrarySource::Local,
            sort_column: None,
            sort_ascending: None,
            search_query: String::new(),
            expanded_album: None,
            expanded_album_songs: None,
            cached_albums: None,
            view_menu_open: false,
            add_menu_open: false,
            total_songs: 0,
            total_albums: 0,
            total_duration_secs: 0.0,
            total_size_bytes: 0.0,
            
            column_widths,
            resizing_column: None,
            resizing_start_x: 0.0,
            resizing_start_w: 0,
            hovered_column: None,
            selected_album: None,
            selected_song_idx: None,
            albums_per_row: std::cell::Cell::new(6),
            library_area_width: 900.0,
            last_viewport: None,
        }
    }
}

impl LibraryManager {
    pub fn sort_songs(&self, songs: &mut [crate::db::database::SongRecord]) {
        if let Some(col_ref) = self.sort_column {
            let is_asc = self.sort_ascending.unwrap_or(true);
            songs.sort_by(|a, b| {
                let res = match col_ref {
                    SortColumn::TrackNumber => {
                        a.track_number.unwrap_or(0).cmp(&b.track_number.unwrap_or(0))
                    },
                    SortColumn::Title => {
                        a.title.cmp(&b.title).then(a.track_number.unwrap_or(0).cmp(&b.track_number.unwrap_or(0)))
                    },
                    SortColumn::Artist => {
                        a.artist.cmp(&b.artist).then(a.track_number.unwrap_or(0).cmp(&b.track_number.unwrap_or(0)))
                    },
                    SortColumn::Album => {
                        a.album.cmp(&b.album).then(a.track_number.unwrap_or(0).cmp(&b.track_number.unwrap_or(0)))
                    },
                    SortColumn::Format => {
                        a.format.cmp(&b.format)
                    },
                    SortColumn::Size => {
                        a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0))
                    },
                    SortColumn::SampleRate => {
                        a.sample_rate.unwrap_or(0).cmp(&b.sample_rate.unwrap_or(0))
                    },
                    SortColumn::Channels => {
                        a.channels.unwrap_or(0).cmp(&b.channels.unwrap_or(0))
                    },
                    SortColumn::Bitrate => {
                        let bit_a = if a.duration_secs.unwrap_or(0.0) > 0.0 { (a.size.unwrap_or(0) as f64 * 8.0) / (a.duration_secs.unwrap() * 1000.0) } else { 0.0 };
                        let bit_b = if b.duration_secs.unwrap_or(0.0) > 0.0 { (b.size.unwrap_or(0) as f64 * 8.0) / (b.duration_secs.unwrap() * 1000.0) } else { 0.0 };
                        bit_a.partial_cmp(&bit_b).unwrap_or(std::cmp::Ordering::Equal)
                    },
                    SortColumn::Genre => {
                        a.genre.cmp(&b.genre).then(a.track_number.unwrap_or(0).cmp(&b.track_number.unwrap_or(0)))
                    },
                    SortColumn::Year => {
                        a.release_year.cmp(&b.release_year).then(a.track_number.unwrap_or(0).cmp(&b.track_number.unwrap_or(0)))
                    },
                    SortColumn::Duration => {
                        let dur_a = a.duration_secs.unwrap_or(0.0);
                        let dur_b = b.duration_secs.unwrap_or(0.0);
                        dur_a.partial_cmp(&dur_b).unwrap_or(std::cmp::Ordering::Equal)
                    },
                    SortColumn::BitDepth => {
                        a.bit_depth.unwrap_or(0).cmp(&b.bit_depth.unwrap_or(0))
                    },
                };
                if is_asc { res } else { res.reverse() }
            });
        }
    }
}

pub fn view<'a>(
    manager: &'a LibraryManager,
    _database: &'a Arc<Mutex<Database>>,
) -> Element<'a, Message> {
    
    // Función de acortado re-utilizable en varias vistas
    fn truncate(s: &str, limit: usize) -> String {
        let len = s.chars().count();
        if len > limit {
            format!("{}...", s.chars().take(limit.saturating_sub(3)).collect::<String>())
        } else {
            s.to_string()
        }
    }
    
    // Función auxiliar para iconos sin fondo (top y bottom bar) interactivos
    let icon_btn_size = |icon: &str, action: Message, size: f32| -> Element<'a, Message> {
        let content = iced::widget::svg(iced::widget::svg::Handle::from_path(format!("assets/icons/{}", icon)))
            .width(Length::Fixed(size))
            .height(Length::Fixed(size));
        iced::widget::mouse_area(content)
            .on_press(action)
            .interaction(iced::mouse::Interaction::Pointer)
            .into()
    };

    let _icon_btn = |icon: &str, action: Message| -> Element<'a, Message> {
        icon_btn_size(icon, action, 20.0)
    };
    
    // Controles de ventana
    let win_action_btn = |icon: &str, action: Message, size: f32| -> Element<'a, Message> {
        let content = iced::widget::svg(iced::widget::svg::Handle::from_path(format!("assets/icons/{}", icon)))
            .width(Length::Fixed(size))
            .height(Length::Fixed(size));
        
        iced::widget::mouse_area(content)
            .on_press(action)
            .interaction(iced::mouse::Interaction::Pointer)
            .into()
    };

    // --- BARRA SUPERIOR (40px) ---
    let create_source_tab = |label: &'a str, source: LibrarySource, current: LibrarySource| -> Element<'a, Message> {
        let is_active = source == current;
        let text_el = text(label)
            .size(15)
            .font(FONT_INTER_SANS_MEDIUM)
            .color(if is_active { Color::WHITE } else { COLOR_TEXT_SECONDARY });
        
        let source_color = match source {
            LibrarySource::Spotify => Color::from_rgb8(29, 185, 84),
            LibrarySource::YouTube => Color::from_rgb8(221, 36, 37),
            LibrarySource::Qobuz => Color::from_rgb8(48, 48, 48),
            LibrarySource::Deezer => Color::from_rgb8(162, 56, 255),
            LibrarySource::Tidal => Color::from_rgb8(68, 68, 68),
            LibrarySource::Local => COLOR_ACCENT,
        };

        let btn_container = container(text_el)
            .padding([2, 10]) // 3px arriba/abajo, 10px lados
            .style(move |_t: &Theme| {
                if is_active {
                    container::Style::default()
                        .background(source_color)
                        .border(iced::Border {
                            radius: 14.0.into(),
                            ..Default::default()
                        })
                } else {
                    container::Style::default()
                }
            });

        iced::widget::mouse_area(btn_container)
            .on_press(Message::LibrarySourceSelected(source))
            .interaction(iced::mouse::Interaction::Pointer)
            .into()
    };

    let source_tabs = row![
        create_source_tab("Biblioteca", LibrarySource::Local, manager.source),
        create_source_tab("Spotify", LibrarySource::Spotify, manager.source),
        create_source_tab("YouTube", LibrarySource::YouTube, manager.source),
        create_source_tab("Qobuz", LibrarySource::Qobuz, manager.source),
        create_source_tab("Deezer", LibrarySource::Deezer, manager.source),
        create_source_tab("Tidal", LibrarySource::Tidal, manager.source),
    ].spacing(10).align_y(Alignment::Center).height(Length::Fill);

    let top_bar = row![
        source_tabs,
        Space::new().width(Length::Fill),
        row![
            container(win_action_btn("minimize.svg", Message::PlayerWindowAction(crate::gui::player::WindowAction::Minimize), 30.0))
                .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 8.0, left: 0.0 }),
            win_action_btn("maximize.svg", Message::PlayerWindowAction(crate::gui::player::WindowAction::Maximize), 30.0),
            win_action_btn("close.svg", Message::PlayerWindowAction(crate::gui::player::WindowAction::Close), 34.0),
        ].spacing(5).align_y(Alignment::Center)
    ]
    .align_y(Alignment::Center)
    .height(Length::Fill)
    .padding(iced::Padding { top: 0.0, right: 5.0, bottom: 0.0, left: 15.0 });

    let top_container = container(top_bar)
        .width(Length::Fill)
        .height(Length::Fixed(40.0))
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    // --- BARRA DE ORDENAMIENTO (30px) ---
    let create_sort_col = |sort: SortColumn, current: Option<SortColumn>, asc: Option<bool>| -> Element<'a, Message> {
        let is_active = current == Some(sort);
        let width = *manager.column_widths.get(&sort).unwrap_or(&100) as f32;
        
        let available_w = width - 25.0; // Espacio reservado para icono/separador
        let max_chars = (available_w / 7.0).max(1.0) as usize;
        let t_str = truncate(sort.as_str(), max_chars);
        
        let t = text(t_str)
            .size(12)
            .font(FONT_INTER_SANS_MEDIUM)
            .color(COLOR_TEXT_SECONDARY);

        let icon_el = if is_active {
             let handle = match asc {
                 Some(true) => Some(iced::widget::svg::Handle::from_path("assets/icons/arrow-up-chevron.svg")),
                 Some(false) => Some(iced::widget::svg::Handle::from_path("assets/icons/arrow-down-chevron.svg")),
                 _ => None,
             };
             
             if let Some(h) = handle {
                 Some(iced::widget::svg(h)
                     .width(20)
                     .height(20)
                     .style(move |_t: &Theme, _s: iced::widget::svg::Status| iced::widget::svg::Style { color: Some(COLOR_TEXT_SECONDARY) }))
             } else {
                 None
             }
        } else { None };
        let is_hovered = manager.resizing_column == Some(sort) || manager.hovered_column == Some(sort);
        
        let separator_visual = container(Space::new())
            .width(Length::Fixed(3.0))
            .height(Length::Fixed(16.0))
            .style(move |_t: &Theme| {
                let bg_color = if is_hovered { COLOR_ACCENT } else { Color::from_rgba(COLOR_TEXT_SECONDARY.r, COLOR_TEXT_SECONDARY.g, COLOR_TEXT_SECONDARY.b, 0.3)};
                container::Style::default()
                    .background(bg_color)
                    .border(iced::Border { radius: 4.0.into(), ..Default::default() })
            });

        let separator_area = iced::widget::mouse_area(separator_visual)
            .on_enter(Message::ColumnHover(Some(sort)))
            .on_exit(Message::ColumnHover(None))
            .on_press(Message::StartColumnResize(sort))
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
            .on_press(Message::LibrarySortChanged(sort));

        let content = row![
            sort_btn,
            Space::new().width(3.0),
            separator_area,
            Space::new().width(3.0)
        ].align_y(Alignment::Center).width(Length::Fixed(width));
            
        container(content)
            .width(Length::Fixed(width))
            .clip(true)
            .into()
    };

    let sort_bar_content = row![
        create_sort_col(SortColumn::TrackNumber, manager.sort_column, manager.sort_ascending),
        create_sort_col(SortColumn::Title, manager.sort_column, manager.sort_ascending),
        create_sort_col(SortColumn::Artist, manager.sort_column, manager.sort_ascending),
        create_sort_col(SortColumn::Album, manager.sort_column, manager.sort_ascending),
        create_sort_col(SortColumn::Genre, manager.sort_column, manager.sort_ascending),
        create_sort_col(SortColumn::Year, manager.sort_column, manager.sort_ascending),
        create_sort_col(SortColumn::Duration, manager.sort_column, manager.sort_ascending),
        create_sort_col(SortColumn::Format, manager.sort_column, manager.sort_ascending),
        create_sort_col(SortColumn::SampleRate, manager.sort_column, manager.sort_ascending),
        create_sort_col(SortColumn::Channels, manager.sort_column, manager.sort_ascending),
        create_sort_col(SortColumn::BitDepth, manager.sort_column, manager.sort_ascending),
        create_sort_col(SortColumn::Bitrate, manager.sort_column, manager.sort_ascending),
        create_sort_col(SortColumn::Size, manager.sort_column, manager.sort_ascending),
    ].align_y(Alignment::Center).height(Length::Fill).padding(iced::Padding { top: 0.0, right: 5.0, bottom: 0.0, left: 35.0 });

    let sort_container = column![
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
    ];

    // --- CONTENIDO GRID / LISTA ---
    let content: Element<'a, Message> = match manager.view_mode {
        LibraryViewMode::Grid => {
            let is_empty = manager.cached_albums.as_ref().map(|v| v.is_empty()).unwrap_or(true);

            if is_empty {
                container(text("La biblioteca está vacía o cargando...").color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into()
            } else {
                let res_grid = iced::widget::responsive(|size| {
                    let db_albums = manager.cached_albums.as_ref().unwrap();
                    let mut grid_col = column![].spacing(0);
                    
                    // Cálculo de columnas
                    let card_w = 180.0;
                    let mut columns_count = (size.width / card_w).floor() as usize;
                    if columns_count < 2 { columns_count = 2; }
                    if columns_count > 7 { columns_count = 7; }
                    // Actualiza el valor real de columnas para que LibraryKeyNav lo use
                    manager.albums_per_row.set(columns_count);

                    for row_chunk in db_albums.chunks(columns_count) {
                        let mut current_row = row![].spacing(5);
                        let mut active_expansion: Option<String> = None;

                        for (album, artist, genre, year, cover_path) in row_chunk {
                            let is_expanded = manager.expanded_album.as_deref() == Some(album.as_str());
                            if is_expanded {
                                active_expansion = Some(album.clone());
                            }

                            let album_art: Element<'a, Message> = if let Some(path) = cover_path {
                                container(
                                    image(iced::widget::image::Handle::from_path(path.clone()))
                                        .width(Length::Fixed(158.0))
                                        .height(Length::Fixed(158.0))
                                        .content_fit(iced::ContentFit::Cover)
                                        .border_radius(8.0)
                                )
                                .width(Length::Fixed(158.0))
                                .height(Length::Fixed(158.0))
                                .style(|_t| container::Style::default().border(iced::Border { radius: 8.0.into(), ..Default::default() }))
                                .clip(true)
                                .into()
                            } else {
                                let icon = iced::widget::svg(iced::widget::svg::Handle::from_path("assets/icons/album.svg"))
                                    .width(Length::Fixed(96.0))
                                    .height(Length::Fixed(96.0))
                                    .style(|_t: &Theme, _s| iced::widget::svg::Style { color: Some(COLOR_TEXT_SECONDARY) });
                                let title_text = text("AuDoxiDY").font(crate::gui::theme::FONT_STAGE_WANDER).size(11).color(COLOR_TEXT_SECONDARY);
                                
                                container(column![icon, title_text].align_x(Alignment::Center).spacing(5))
                                    .width(Length::Fixed(158.0))
                                    .height(Length::Fixed(158.0))
                                    .center_x(Length::Fill)
                                    .center_y(Length::Fill)
                                    .style(|_t: &Theme| container::Style::default().background(Color::from(COLOR_BG)).border(iced::Border { radius: 8.0.into(), ..Default::default() }))
                                    .clip(true)
                                    .into()
                            };
                            
                            let info_col = column![
                                text(truncate(artist, 20)).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM).line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(14.0))),
                                text(truncate(album, 19)).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM).line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(14.0))),
                                text(truncate(genre, 20)).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM).line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(14.0))),
                                text(year.as_str()).size(12).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM).line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(14.0))),
                            ].spacing(2).width(Length::Fill);

                            let chevron_svg = if is_expanded { "arrow-up-chevron.svg" } else { "arrow-down-chevron.svg" };
                            let chevron_btn = button(
                                iced::widget::svg(iced::widget::svg::Handle::from_path(format!("assets/icons/{}", chevron_svg)))
                                    .width(30)
                                    .height(30)
                                    .style(move |_t: &Theme, _s: iced::widget::svg::Status| iced::widget::svg::Style { color: Some(COLOR_TEXT_PRIMARY) })
                            )
                                .padding(0)
                                .on_press(Message::ToggleAlbumExpansion(album.clone()))
                                .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT));

                            let card_bottom = row![
                                info_col,
                                chevron_btn
                            ].align_y(Alignment::Center).width(Length::Fill);

                            let is_selected = manager.selected_album.as_deref() == Some(album.as_str());
                            
                            let item_col = column![
                                album_art,
                                card_bottom
                            ].spacing(5);

                            let card_wrapper = iced::widget::mouse_area(item_col)
                                .on_press(Message::SelectAlbum(album.clone()))
                                .interaction(iced::mouse::Interaction::Pointer);
                            
                            let card_container = container(card_wrapper)
                                .width(Length::Fixed(188.0))
                                .padding(iced::Padding { top: 18.0, bottom: 15.0, left: 15.0, right: 15.0 })
                                .style(move |_t: &Theme| {
                                    if is_expanded {
                                        container::Style::default().background(COLOR_CONTRAST).border(iced::Border { radius: 10.0.into(), ..Default::default() })
                                    } else if is_selected {
                                        container::Style::default().background(COLOR_CONTRAST).border(iced::Border { radius: 10.0.into(), ..Default::default() })
                                    } else {
                                        container::Style::default()
                                    }
                                });
                                
                            current_row = current_row.push(card_container);
                        }

                        grid_col = grid_col.push(current_row);

                        // --- Expansión Inline debajo de la fila ---
                        if let Some(_exp_album) = active_expansion {
                            let mut album_songs_col = column![].spacing(0).padding([25, 0]);
                            
                            if let Some(songs) = manager.expanded_album_songs.as_ref() {
                                if !songs.is_empty() {
                                    for (song_i, song) in songs.iter().enumerate() {
                                        let s_clone = song.clone();
                                        let is_song_selected = manager.selected_song_idx == Some(song_i);
                                        
                                        let format = song.format.clone().unwrap_or_else(|| "-".to_string());
                                        let rate = song.sample_rate.map_or("-".to_string(), |r| format!("{:.1} kHz", r as f64 / 1000.0));
                                        let chans = song.channels.map_or("-".to_string(), |c| c.to_string());
                                        let b_depth = song.bit_depth.map_or("-".to_string(), |b| format!("{} bits", b));
                                        let sz = song.size.map_or("-".to_string(), |s| {
                                            let mb = s as f64 / 1048576.0;
                                            if mb >= 1024.0 {
                                                format!("{:.2} GB", mb / 1024.0)
                                            } else {
                                                format!("{:.2} MB", mb)
                                            }
                                        });
                                        
                                        let bitrate_str = if let (Some(s), Some(d)) = (song.size, song.duration_secs) {
                                            if d > 0.0 {
                                                format!("{} kbps", ((s as f64 * 8.0) / (d * 1000.0)) as u32)
                                            } else {
                                                "-".to_string()
                                            }
                                        } else {
                                            "-".to_string()
                                        };
                                        
                                        let dur_secs = song.duration_secs.unwrap_or(0.0) as u64;
                                        let dur_str = format!("{}:{:02}", dur_secs / 60, dur_secs % 60);
                                        
                                        let txt_color = if is_song_selected { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };
                                        
                                        let get_col = |col: SortColumn, val: String| -> Element<'a, Message> {
                                            let w = *manager.column_widths.get(&col).unwrap_or(&100) as f32;
                                            let max_chars = ((w - 10.0) / 7.0).max(1.0) as usize;
                                            let truncated = truncate(&val, max_chars);
                                            
                                            container(text(truncated).size(13).color(Color::from(txt_color)).font(FONT_INTER_SANS_MEDIUM))
                                                .width(Length::Fixed(w))
                                                .height(Length::Fixed(15.0))
                                                .center_y(Length::Fill)
                                                .padding(iced::Padding { left: 5.0, right: 5.0, top: 0.0, bottom: 0.0 })
                                                .clip(true)
                                                .into()
                                        };

                                        let song_row_inner = row![
                                            get_col(SortColumn::TrackNumber, song.track_number.unwrap_or(0).to_string()),
                                            get_col(SortColumn::Title, song.title.clone().unwrap_or_else(|| "Unknown".into())),
                                            get_col(SortColumn::Artist, song.artist.clone().unwrap_or_else(|| "Unknown".into())),
                                            get_col(SortColumn::Album, song.album.clone().unwrap_or_else(|| "Unknown".into())),
                                            get_col(SortColumn::Genre, song.genre.clone().unwrap_or_else(|| "".into())),
                                            get_col(SortColumn::Year, song.release_year.clone().unwrap_or_else(|| "".into())),
                                            get_col(SortColumn::Duration, dur_str),
                                            get_col(SortColumn::Format, format),
                                            get_col(SortColumn::SampleRate, rate),
                                            get_col(SortColumn::Channels, chans),
                                            get_col(SortColumn::BitDepth, b_depth),
                                            get_col(SortColumn::Bitrate, bitrate_str),
                                            get_col(SortColumn::Size, sz),
                                            button(text("►").size(11).color(Color::from(txt_color))).on_press(Message::AddSongToPlaylist(s_clone)).style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT)),
                                        ].align_y(Alignment::Center).padding([0, 15]).height(Length::Fixed(15.0));
                                        
                                        let song_row = iced::widget::mouse_area(
                                            container(song_row_inner)
                                                .width(Length::Fill)
                                                .height(Length::Fixed(32.0))
                                                .align_y(Alignment::Center)
                                                .style(move |_t: &Theme| {
                                                    if is_song_selected {
                                                        container::Style::default().background(Color::from(COLOR_CONTRAST))
                                                    } else {
                                                        container::Style::default()
                                                    }
                                                })
                                        ).on_press(Message::SelectSong(Some(song_i)))
                                         .interaction(iced::mouse::Interaction::Pointer);
                                        
                                        album_songs_col = album_songs_col.push(song_row);
                                    }
                                } else {
                                    album_songs_col = album_songs_col.push(text("No se encontraron canciones.").color(COLOR_TEXT_SECONDARY));
                                }
                            } else {
                                album_songs_col = album_songs_col.push(text("Cargando...").color(COLOR_TEXT_SECONDARY));
                            }

                            let exp_container = container(album_songs_col)
                                .width(Length::Fill)
                                .padding([5, 15])
                                .style(|_t: &Theme| container::Style::default().background(COLOR_BG));
                            grid_col = grid_col.push(exp_container);
                        }
                    }

                    // El closure ahora solo devuelve el grid (SIN scrollable)
                    grid_col.into()
                });
                
                // El scrollable envuelve el responsive directamente - así snap_to puede encontrarlo
                scrollable(res_grid)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .direction(iced::widget::scrollable::Direction::Vertical(
                        iced::widget::scrollable::Scrollbar::new()
                            .width(4)
                            .margin(0)
                            .scroller_width(4)
                    ))
                    .id(LIBRARY_SCROLL_ID.clone())
                    .on_scroll(Message::LibraryScroll)
                    .style(crate::gui::theme::custom_scrollbar_style)
                    .into()
            }
        }
        LibraryViewMode::DetailedList => {
            container(text("Vista Detallada - En desarrollo").color(COLOR_TEXT_PRIMARY)).into()
        }
        LibraryViewMode::ThumbnailList => {
            container(text("Vista con Thumbnail - En desarrollo").color(COLOR_TEXT_PRIMARY)).into()
        }
        LibraryViewMode::SimpleList => {
             container(text("Vista Simple - En desarrollo").color(COLOR_TEXT_PRIMARY)).into()
        }
    };

    // --- BARRA INFERIOR (40px) ---
    // Buscar sin bordes
    let search_input = container(
        text_input("Buscar...", &manager.search_query)
            .on_input(Message::LibrarySearchQueryChanged)
            .font(FONT_INTER_SANS_MEDIUM)
            .width(Length::Fixed(200.0))
    ).padding([0, 0]).center_y(Length::Fill);

    // Estadísticas
    let total_hrs = (manager.total_duration_secs / 3600.0) as u32;
    let total_mins = ((manager.total_duration_secs % 3600.0) / 60.0) as u32;
    let total_gb = manager.total_size_bytes / 1024.0 / 1024.0 / 1024.0;
    
    let stats_text = if total_gb >= 1.0 {
        format!("{} Canciones | {} Álbumes | {}:{:02} hrs | {:.2} GB", manager.total_songs, manager.total_albums, total_hrs, total_mins, total_gb)
    } else {
        let total_mb = manager.total_size_bytes / 1024.0 / 1024.0;
        format!("{} Canciones | {} Álbumes | {}:{:02} hrs | {:.2} MB", manager.total_songs, manager.total_albums, total_hrs, total_mins, total_mb)
    };
    
    // Icono vista actual
    let view_icon_str = match manager.view_mode {
        LibraryViewMode::Grid => "view-grid-outlined.svg",
        LibraryViewMode::ThumbnailList => "view-list-thumbnail-outlined.svg",
        LibraryViewMode::DetailedList => "view-list-thumbnail-fill.svg",
        LibraryViewMode::SimpleList => "view-list.svg",
    };

    let bottom_actions = row![
        icon_btn_size("play-rounded.outlined.svg", Message::PlayLibrarySelection, 36.0),
        Space::new().width(5.0),
        icon_btn_size("more-small.svg", Message::OpenFolderPicker, 31.0),
        Space::new().width(5.0),
        icon_btn_size(view_icon_str, Message::ToggleLibraryViewDropdown, 30.0),
    ].align_y(Alignment::Center).height(Length::Fill);

    let info_text_el = text(stats_text).size(13).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM);

    let bottom_bar = row![
        search_input,
        Space::new().width(15.0),
        info_text_el,
        Space::new().width(Length::Fill),
        bottom_actions,
    ]
    .padding([0, 15])
    .height(Length::Fill)
    .align_y(Alignment::Center);

    let bottom_container = container(bottom_bar)
        .width(Length::Fill)
        .height(Length::Fixed(40.0))
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));


    // Todo junto con padding a los lados donde corresponda
    container(
        column![
            top_container,
            sort_container,
            container(content).padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 5.0 }), // Padding global del área de contenido
            bottom_container
        ]
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
    .into()
}
