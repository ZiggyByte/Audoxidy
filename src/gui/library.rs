use iced::{
    widget::{button, column, container, row, scrollable, text, text_input, Space, image, tooltip},
    Alignment, Color, Element, Length, Theme,
};
use std::sync::{Arc, Mutex};
use crate::db::Database;
use crate::gui::app::Message;
use crate::gui::theme::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LibraryViewMode {
    Grid,
    DetailedList,
    ThumbnailList,
    SimpleList,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LibrarySource {
    Local,
    Spotify,
    Tidal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortColumn {
    TrackNumber,
    Title,
    Artist,
    Album,
    Genre,
    Year,
    Format,
    SampleRate,
    Channels,
    Size,
    Duration,
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
            SortColumn::Format => "Formato",
            SortColumn::SampleRate => "Muestreo",
            SortColumn::Channels => "Canales",
            SortColumn::Size => "Tamaño",
            SortColumn::Duration => "Duración",
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
}

impl Default for LibraryManager {
    fn default() -> Self {
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
        }
    }
}

pub fn view<'a>(
    manager: &'a LibraryManager,
    _database: &'a Arc<Mutex<Database>>,
) -> Element<'a, Message> {
    
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

    let icon_btn = |icon: &str, action: Message| -> Element<'a, Message> {
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
    // Pestañas Source
    let create_source_tab = |label: &'a str, source: LibrarySource, current: LibrarySource| -> Element<'a, Message> {
        let is_active = source == current;
        let text_el = text(label)
            .size(14)
            .font(FONT_INTER_SANS_MEDIUM)
            .color(if is_active { Color::WHITE } else { COLOR_TEXT_SECONDARY });
        
        let container_style = if is_active {
            |_t: &Theme| container::Style::default()
                .background(COLOR_ACCENT)
                .border(iced::Border {
                    radius: 16.0.into(),
                    ..Default::default()
                })
        } else {
            |_t: &Theme| container::Style::default()
        };

        let btn_container = container(text_el)
            .padding([3, 10]) // 3px arriba/abajo, 10px lados
            .style(container_style);

        iced::widget::mouse_area(btn_container)
            .on_press(Message::LibrarySourceSelected(source))
            .interaction(iced::mouse::Interaction::Pointer)
            .into()
    };

    let source_tabs = row![
        create_source_tab("Biblioteca", LibrarySource::Local, manager.source),
        create_source_tab("Spotify", LibrarySource::Spotify, manager.source),
        create_source_tab("Tidal", LibrarySource::Tidal, manager.source),
    ].spacing(10).align_y(Alignment::Center).height(Length::Fill);

    let top_bar = row![
        source_tabs,
        Space::new().width(Length::Fill),
        row![
            win_action_btn("minimize.svg", Message::PlayerWindowAction(crate::gui::player::WindowAction::Minimize), 30.0),
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
        .style(|_t: &Theme| container::Style::default().background(COLOR_BG));

    // --- BARRA DE ORDENAMIENTO (30px) ---
    let create_sort_col = |sort: SortColumn, current: Option<SortColumn>, asc: Option<bool>, weight: u16| -> Element<'a, Message> {
        let is_active = current == Some(sort);
        let t = text(sort.as_str())
            .size(12)
            .font(FONT_INTER_SANS_MEDIUM)
            .color(COLOR_TEXT_SECONDARY);

        let icon_el = if is_active {
             match asc {
                 Some(true) => Some(iced::widget::svg(iced::widget::svg::Handle::from_path("assets/icons/arrow-up-chevron.svg")).width(24).height(24)),
                 Some(false) => Some(iced::widget::svg(iced::widget::svg::Handle::from_path("assets/icons/arrow-down-chevron.svg")).width(24).height(24)),
                 _ => None,
             }
        } else { None };
        
        // Elemento visual de separador básico
        let separator = container(Space::new().width(1.0).height(Length::Fixed(15.0)))
            .style(|_t: &Theme| container::Style::default().background(Color::from(COLOR_TEXT_SECONDARY)));
            
        let inner_row = if let Some(ic) = icon_el {
            row![t, Space::new().width(4.0), ic].align_y(Alignment::Center)
        } else {
            row![t].align_y(Alignment::Center)
        };

        let content = row![
            inner_row,
            Space::new().width(Length::Fill),
            separator
        ].align_y(Alignment::Center).width(Length::Fill).height(Length::Fill);

        button(content)
            .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
            .on_press(Message::LibrarySortChanged(sort))
            .width(Length::FillPortion(weight))
            .height(Length::Fill)
            .into()
    };

    let sort_bar = row![
        create_sort_col(SortColumn::TrackNumber, manager.sort_column, manager.sort_ascending, 1),
        create_sort_col(SortColumn::Title, manager.sort_column, manager.sort_ascending, 3),
        create_sort_col(SortColumn::Artist, manager.sort_column, manager.sort_ascending, 2),
        create_sort_col(SortColumn::Album, manager.sort_column, manager.sort_ascending, 2),
        create_sort_col(SortColumn::Genre, manager.sort_column, manager.sort_ascending, 2),
        create_sort_col(SortColumn::Year, manager.sort_column, manager.sort_ascending, 1),
        create_sort_col(SortColumn::Format, manager.sort_column, manager.sort_ascending, 1),
        create_sort_col(SortColumn::SampleRate, manager.sort_column, manager.sort_ascending, 1),
        create_sort_col(SortColumn::Channels, manager.sort_column, manager.sort_ascending, 1),
        create_sort_col(SortColumn::Size, manager.sort_column, manager.sort_ascending, 1),
    ].align_y(Alignment::Center).height(Length::Fill).padding([0, 15]);

    let sort_container = container(sort_bar)
        .width(Length::Fill)
        .height(Length::Fixed(30.0))
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

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
                let db_albums = manager.cached_albums.as_ref().unwrap();
                let mut grid_col = column![].spacing(20);
                let columns_count = 5; // WIP: Dinamismo posterior
                
                for row_chunk in db_albums.chunks(columns_count) {
                    let mut current_row = row![].spacing(20);
                    let mut active_expansion = None;

                    for (album, artist, genre, year, cover_path) in row_chunk {
                        let is_expanded = manager.expanded_album.as_deref() == Some(album.as_str());
                        if is_expanded {
                            active_expansion = Some(album.clone());
                        }

                        let album_art: Element<'a, Message> = if let Some(path) = cover_path {
                            image(iced::widget::image::Handle::from_path(path.clone()))
                                .width(Length::Fixed(150.0))
                                .height(Length::Fixed(150.0))
                                .into()
                        } else {
                            let icon = iced::widget::svg(iced::widget::svg::Handle::from_path("assets/icons/album.svg"))
                                .width(Length::Fixed(50.0))
                                .height(Length::Fixed(50.0));
                            
                            container(icon)
                                .width(Length::Fixed(150.0))
                                .height(Length::Fixed(150.0))
                                .center_x(Length::Fill)
                                .center_y(Length::Fill)
                                .style(|_t: &Theme| container::Style::default().background(Color::from(COLOR_TEXT_SECONDARY)))
                                .into()
                        };
                        
                        fn truncate(s: &str, limit: usize) -> String {
                            let len = s.chars().count();
                            if len > limit {
                                format!("{}...", s.chars().take(limit.saturating_sub(3)).collect::<String>())
                            } else {
                                s.to_string()
                            }
                        }

                        let text_w_tooltip = |t: &'a str| -> Element<'a, Message> {
                            tooltip(
                                text(truncate(t, 16)).size(13).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
                                t,
                                tooltip::Position::Top
                            ).style(|_t| container::Style::default().background(Color::from(COLOR_CONTRAST)).border(iced::Border {
                                color: Color::from(COLOR_ACCENT),
                                width: 1.0,
                                radius: 5.0.into()
                            })).padding(3).into()
                        };

                        let info_col = column![
                            text(truncate(artist, 16)).size(13).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
                            text_w_tooltip(album),
                            text(truncate(genre, 16)).size(13).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
                            text(year).size(13).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
                        ].spacing(2).width(Length::Fill);

                        let card_bottom = row![
                            info_col,
                            icon_btn_size(
                                if is_expanded { "arrow-up-chevron.svg" } else { "arrow-down-chevron.svg" },
                                Message::ToggleAlbumExpansion(album.clone()),
                                21.0
                            )
                        ].align_y(Alignment::Center).width(Length::Fill);

                        let item_col = column![
                            album_art,
                            card_bottom
                        ].spacing(0);

                        let card_wrapper = iced::widget::mouse_area(item_col)
                            .on_press(Message::ToggleAlbumExpansion(album.clone())) 
                            .interaction(iced::mouse::Interaction::Pointer);
                        
                        let card_container = container(card_wrapper)
                            .width(Length::Fixed(150.0))
                            .padding(iced::Padding { top: 5.0, bottom: 8.0, left: 15.0, right: 15.0 })
                            .style(move |_t: &Theme| if is_expanded { 
                                container::Style::default().background(COLOR_CONTRAST).border(iced::Border { radius: 8.0.into(), ..Default::default() }) 
                            } else { 
                                container::Style::default() 
                            });
                            
                        current_row = current_row.push(card_container);
                    }

                    grid_col = grid_col.push(current_row);

                    // --- Expansión Inline debajo de la fila ---
                    if let Some(_exp_album) = active_expansion {
                        let mut album_songs_col = column![].spacing(8).padding([1, 0]);
                        
                        if let Some(songs) = &manager.expanded_album_songs {
                            if !songs.is_empty() {
                                for song in songs {
                                    let s_clone = song.clone();
                                    
                                    let format = song.format.clone().unwrap_or_else(|| "-".to_string());
                                    let rate = song.sample_rate.map_or("-".to_string(), |r| format!("{} hz", r));
                                    let chans = song.channels.map_or("-".to_string(), |c| c.to_string());
                                    let sz = song.size.map_or("-".to_string(), |s| format!("{} MB", s / 1024 / 1024));
                                    let _dur = song.duration_secs.map_or("-".to_string(), |d| format!("{}:{:02}", (d/60.0) as u32, (d%60.0) as u32));
                                    
                                    let song_row = row![
                                        text(song.track_number.unwrap_or(0).to_string()).size(13).color(COLOR_TEXT_PRIMARY).width(Length::FillPortion(1)),
                                        text(song.title.clone().unwrap_or_else(|| "Unknown".into())).size(13).color(COLOR_TEXT_PRIMARY).width(Length::FillPortion(3)),
                                        text(song.artist.clone().unwrap_or_else(|| "Unknown".into())).size(13).color(COLOR_TEXT_PRIMARY).width(Length::FillPortion(2)),
                                        text(song.album.clone().unwrap_or_else(|| "Unknown".into())).size(13).color(COLOR_TEXT_PRIMARY).width(Length::FillPortion(2)),
                                        text(song.genre.clone().unwrap_or_else(|| "".into())).size(13).color(COLOR_TEXT_PRIMARY).width(Length::FillPortion(2)),
                                        text(song.release_year.clone().unwrap_or_else(|| "".into())).size(13).color(COLOR_TEXT_PRIMARY).width(Length::FillPortion(1)),
                                        text(format).size(13).color(COLOR_TEXT_PRIMARY).width(Length::FillPortion(1)),
                                        text(rate).size(13).color(COLOR_TEXT_PRIMARY).width(Length::FillPortion(1)),
                                        text(chans).size(13).color(COLOR_TEXT_PRIMARY).width(Length::FillPortion(1)),
                                        text(sz).size(13).color(COLOR_TEXT_PRIMARY).width(Length::FillPortion(1)),
                                        button(text("▶").size(12)).on_press(Message::AddSongToPlaylist(s_clone)).style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT)),
                                    ].align_y(Alignment::Center);
                                    
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
                            .padding([15, 10])
                            .style(|_t: &Theme| container::Style::default().background(COLOR_BG));
                        grid_col = grid_col.push(exp_container);
                    }
                }

                scrollable(grid_col)
                    .width(Length::Fill)
                    .height(Length::Fill)
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
        icon_btn_size("more-small.svg", Message::OpenFolderPicker, 31.0),
        Space::new().width(5.0),
        icon_btn_size("play-rounded.outlined.svg", Message::PlayLibrarySelection, 36.0),
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
            container(content).padding(15), // Padding global del área de contenido
            bottom_container
        ]
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
    .into()
}
