use iced::{
    widget::{button, column, container, scrollable, text, text_input},
    Color, Element, Length, Theme,
};
use crate::gui::app::Message;

// TODO: Consolidar globales en theme.rs
use crate::gui::theme::*;

pub struct LibraryFiltersManager {
    pub search_query: String,
    
    // Estado Abierto/Cerrado del Arbol
    pub tree_open_genre: bool,
    pub tree_open_artist: bool,
    pub tree_open_album: bool,
}

impl Default for LibraryFiltersManager {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            tree_open_genre: false,
            tree_open_artist: false,
            tree_open_album: false,
        }
    }
}

pub fn view<'a>(manager: &'a LibraryFiltersManager, library_manager: &'a crate::gui::library::LibraryManager) -> Element<'a, Message> {
    
    // Top Bar (Titulo AuDoxiDY)
    let title_btn = button(
        text("AuDoxiDY")
            .size(16)
            .font(FONT_STAGE_WANDER)
    )
    .on_press(Message::NoOp)
    .padding(0)
    .style(|_theme: &Theme, status| {
        let mut style = button::Style::default().with_background(Color::TRANSPARENT);
        if status == iced::widget::button::Status::Hovered {
            style.text_color = COLOR_ACCENT;
        } else {
            style.text_color = COLOR_TEXT_PRIMARY;
        }
        style
    });

    let header_container = container(title_btn)
        .width(Length::Fill)
        .height(Length::Fixed(40.0))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    // Tree nodes (Listas de filtros expandibles)
    let mut tree_col = column![].spacing(5).padding(10);
    
    // Custom inline helper macro-like pattern (desenrollado por move semantics de Iced Builder)
    let icon_genre = if manager.tree_open_genre { "v " } else { "> " };
    tree_col = tree_col.push(
        button(text(format!("{}Generos", icon_genre)).size(14).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM))
            .width(Length::Fill)
            .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
            .on_press(Message::ToggleGenreFilter)
    );
    if manager.tree_open_genre {
        tree_col = tree_col.push(
            container(text(" Progressive Rock").size(13).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM))
                .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 15.0 })
        );
    }

    let q_lower = manager.search_query.to_lowercase();
    
    // Artistas
    let icon_artist = if manager.tree_open_artist { "v " } else { "> " };
    tree_col = tree_col.push(
        button(text(format!("{}Artistas", icon_artist)).size(14).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM))
            .width(Length::Fill)
            .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
            .on_press(Message::ToggleArtistFilter)
    );
    if manager.tree_open_artist {
        let mut artist_count = 0;
        for group in &library_manager.artist_groups {
            if manager.search_query.is_empty() || group.name.to_lowercase().contains(&q_lower) {
                tree_col = tree_col.push(
                    container(text(format!("  • {}", group.name)).size(13).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM))
                        .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 15.0 })
                );
                artist_count += 1;
                if artist_count >= 50 { break; } // Limitar DOM virtual
            }
        }
    }

    // Álbumes
    let icon_album = if manager.tree_open_album { "v " } else { "> " };
    tree_col = tree_col.push(
        button(text(format!("{}Albumes", icon_album)).size(14).color(COLOR_TEXT_PRIMARY).font(FONT_INTER_SANS_MEDIUM))
            .width(Length::Fill)
            .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
            .on_press(Message::ToggleAlbumFilter)
    );
    if manager.tree_open_album {
        if let Some(albums) = &library_manager.cached_albums {
            let mut album_count = 0;
            for alb in albums {
                if manager.search_query.is_empty() || alb.1.to_lowercase().contains(&q_lower) || alb.2.to_lowercase().contains(&q_lower) {
                    tree_col = tree_col.push(
                        container(text(format!("  • {}", alb.1)).size(13).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM))
                            .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 15.0 })
                    );
                    album_count += 1;
                    if album_count >= 50 { break; }
                }
            }
        }
    }

    let filters_scroll = scrollable(tree_col).height(Length::Fill);

    // Barra inferior de busqueda
    let search_bar = container(
        text_input("Buscar...", &manager.search_query)
            .on_input(Message::FilterSearchChanged)
            .font(FONT_INTER_SANS_MEDIUM)
            .padding(5)
    )
    .width(Length::Fill)
    .height(Length::Fixed(40.0))
    .center_y(Length::Fill)
    .padding([0, 15])
    .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    // Consolidar Layout (Vertical de 180px Ancho y 100% de Alto)
    container(
        column![
            header_container,
            filters_scroll,
            search_bar
        ]
    )
    .width(Length::Fixed(160.0))
    .height(Length::Fill)
    .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
    .into()
}
