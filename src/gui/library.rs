use iced::{
    widget::{button, column, container, row, scrollable, text, text_input, Space, image},
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
    SimpleList,
}

pub struct LibraryManager {
    pub view_mode: LibraryViewMode,
    pub search_query: String,
    pub expanded_album: Option<String>,
}

impl Default for LibraryManager {
    fn default() -> Self {
        Self {
            view_mode: LibraryViewMode::Grid,
            search_query: String::new(),
            expanded_album: None,
        }
    }
}

pub fn view<'a>(
    manager: &'a LibraryManager,
    database: &'a Arc<Mutex<Database>>,
) -> Element<'a, Message> {
    
    // Contenido Superior (Lista o Cuadrícula)
    let content: Element<'a, Message> = match manager.view_mode {
        LibraryViewMode::Grid => {
            // Recolectar álbumes en items con Strings owned para evitar el error E0597 de lifetimes.
            let mut db_albums: Vec<(String, String, String, String, Option<String>)> = Vec::new();
            if let Ok(db) = database.lock() {
                if let Ok(albums) = db.get_all_albums() {
                    db_albums = albums;
                }
            }

            if db_albums.is_empty() {
                container(text("No hay álbumes o escaneando la biblioteca...").color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into()
            } else {
                // Dibujamos una columna principal con el scroll
                let mut grid_col = column![].spacing(20).padding(20);
                
                // Agrupamos de a 4 o 5 (hardcoded "responsive" manual en row por simplicidad de flex box)
                // TODO: Wrap widget de Iced 0.14 nativo.
                let columns_count = 5; 
                for row_chunk in db_albums.chunks(columns_count) {
                    let mut current_row = row![].spacing(20);
                    let mut active_expansion = None;

                    for (album, artist, _genre, _year, cover_path) in row_chunk {
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
                            container(text("No Cover").color(COLOR_TEXT_SECONDARY))
                                .width(Length::Fixed(150.0))
                                .height(Length::Fixed(150.0))
                                .center_x(Length::Fill)
                                .center_y(Length::Fill)
                                .style(|_t: &Theme| container::Style::default().background(Color::from_rgb(0.05, 0.05, 0.05)))
                                .into()
                        };

                        let item_col = column![
                            album_art,
                            text(artist.clone()).size(13).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM),
                            text(album.clone()).size(13).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM)
                        ].spacing(5);

                        let card = button(item_col)
                            .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
                            .on_press(Message::ToggleAlbumExpansion(album.clone()));
                        
                        current_row = current_row.push(card);
                    }

                    grid_col = grid_col.push(current_row);

                    // Si hay una expansión activa en esta fila, la dibujamos debajo
                    if let Some(exp_album) = active_expansion {
                        let mut album_songs_col = column![].spacing(8).padding(15);
                        
                        if let Ok(db) = database.lock() {
                            if let Ok(songs) = db.get_songs_by_album(&exp_album) {
                                // Muestra botón para reproducir todo
                                let play_album_btn = button(text("▶ Reproducir Todo").color(COLOR_BG).font(FONT_INTER_SANS_MEDIUM))
                                    .style(|_t: &Theme, _s| button::Style::default().with_background(COLOR_ACCENT))
                                    .on_press(Message::PlayAlbum(songs.clone()));

                                album_songs_col = album_songs_col.push(
                                    row![
                                        text(format!("Canciones de: {}", exp_album)).size(16).color(COLOR_TEXT_PRIMARY).width(Length::Fill).font(FONT_INTER_SANS_MEDIUM),
                                        play_album_btn
                                    ].align_y(Alignment::Center)
                                );

                                for song in songs {
                                    let s_clone = song.clone();
                                    
                                    let song_row = row![
                                        text(song.title.clone().unwrap_or_else(|| "Unknown Track".to_string())).color(COLOR_TEXT_PRIMARY).size(14).width(Length::Fill).font(FONT_INTER_SANS_MEDIUM),
                                        iced::widget::Space::new().width(Length::Fixed(15.0)),
                                        button(text("+").size(14)).on_press(Message::AddSongToPlaylist(s_clone))
                                    ].align_y(Alignment::Center);
                                    
                                    album_songs_col = album_songs_col.push(song_row);
                                }
                            } else {
                                album_songs_col = album_songs_col.push(text("No se encontraron canciones.").color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM));
                            }
                        }

                        let exp_container = container(album_songs_col)
                            .width(Length::Fill)
                            .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));
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
            container(text("Vista Detallada - En desarrollo").color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM)).into()
        }
        LibraryViewMode::SimpleList => {
             container(text("Vista Simple - En desarrollo").color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM)).into()
        }
    };

    // Barra de Búsqueda Inferior
    let bottom_bar = row![
        text_input("Buscar en Biblioteca...", &manager.search_query)
            .on_input(Message::LibrarySearchQueryChanged)
            .font(FONT_INTER_SANS_MEDIUM)
            .width(Length::Fixed(200.0)),
        
        Space::new().width(Length::Fill),
        
        button(text("▶ Todo").size(14).font(FONT_INTER_SANS_MEDIUM))
            .on_press(Message::PlayLibraryAll),
        button(text(" Añadir 📁").size(14).font(FONT_INTER_SANS_MEDIUM))
            .on_press(Message::OpenFolderPicker),
        button(text(" Cuadrícula").size(14).font(FONT_INTER_SANS_MEDIUM))
            .on_press(Message::ChangeLibraryViewMode(LibraryViewMode::Grid)),
        button(text(" Lista").size(14).font(FONT_INTER_SANS_MEDIUM))
            .on_press(Message::ChangeLibraryViewMode(LibraryViewMode::DetailedList))
    ]
    .padding(10)
    .spacing(10)
    .align_y(Alignment::Center);

    let bottom_container = container(bottom_bar)
        .width(Length::Fill)
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    // Estructura Final (Resto -> Búsqueda)
    container(
        column![
            content,
            bottom_container
        ]
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
    .into()
}
