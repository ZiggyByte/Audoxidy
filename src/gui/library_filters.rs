use iced::{
    widget::{button, column, container, row, scrollable, text, text_input, Space, svg, pick_list},
    Alignment, Color, Element, Length, Theme,
};
use crate::gui::app::Message;
use crate::gui::theme::*;
use crate::gui::widgets::custom_scrollbar_style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    Folder,
    Artist,
    Album,
    Genre,
    Year,
}

impl FilterType {
    pub fn as_str(&self) -> &str {
        match self {
            FilterType::Folder => "Carpeta",
            FilterType::Artist => "Artista",
            FilterType::Album => "Álbum",
            FilterType::Genre => "Género",
            FilterType::Year => "Año",
        }
    }

    pub fn all() -> Vec<FilterType> {
        vec![
            FilterType::Folder,
            FilterType::Artist,
            FilterType::Album,
            FilterType::Genre,
            FilterType::Year,
        ]
    }
}

impl std::fmt::Display for FilterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub struct LibraryFiltersManager {
    pub selected_type: FilterType,
    pub selected_subfilter: Option<String>, // "A", "B", "#", "·"
    pub expanded_nodes: std::collections::HashSet<String>,
    pub menu_open: bool,
    pub search_query: String,
}

impl Default for LibraryFiltersManager {
    fn default() -> Self {
        Self {
            selected_type: FilterType::Genre, // Filtro por defecto
            selected_subfilter: None,
            expanded_nodes: std::collections::HashSet::new(),
            menu_open: false,
            search_query: String::new(),
        }
    }
}

pub fn view<'a>(manager: &'a LibraryFiltersManager, library_manager: &'a crate::gui::library::LibraryManager) -> Element<'a, Message> {
    // 1. Barra Superior (40px fija)
    let header = container(
        text("AuDoxiDY")
            .size(16)
            .font(FONT_STAGE_WANDER)
            .color(COLOR_TEXT_PRIMARY)
    )
    .width(Length::Fill)
    .height(Length::Fixed(40.0))
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    // 2. Menú de Filtros Generales
    let filter_selector = container(
        pick_list(
            FilterType::all(),
            Some(manager.selected_type),
            Message::ChangeGeneralFilter,
        )
        .width(Length::Fill)
        .padding(7)
        .font(FONT_INTER_SANS_MEDIUM)
        .text_size(13)
        .style(|_t: &Theme, _s| pick_list::Style {
            background: COLOR_CONTRAST.into(),
            border: iced::Border {
                color: COLOR_ACCENT,
                width: 1.0,
                radius: 4.0.into(),
            },
            text_color: COLOR_TEXT_PRIMARY,
            placeholder_color: COLOR_TEXT_SECONDARY,
            handle_color: COLOR_TEXT_PRIMARY,
        })
    )
    .width(Length::Fill)
    .padding(iced::Padding { top: 5.0, right: 15.0, bottom: 5.0, left: 15.0 });

    // 3. Subfiltros (Abecedario dinámico)
    let alphabet_row = render_alphabet(manager, library_manager);
    
    let subfilters = column![
        alphabet_row,
        button(
            text("mostrar todo")
                .size(11)
                .font(FONT_INTER_SANS_MEDIUM)
                .color(COLOR_TEXT_SECONDARY)
        )
        .on_press(Message::SelectSubfilter(None))
        .padding([5, 10])
        .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
    ]
    .spacing(5)
    .padding([5, 15])
    .align_x(iced::Alignment::Center);

    // 4. Árbol de Resultados (Cuerpo central)
    let tree_content = render_tree(manager, library_manager);
    let scrollable_tree = scrollable(tree_content)
        .height(Length::Fill)
        .style(custom_scrollbar_style);

    // 5. Barra Inferior de Búsqueda (40px fija)
    let search_bar = container(
        text_input("Búsqueda rápida", &manager.search_query)
            .on_input(Message::FilterSearchChanged)
            .size(13)
            .font(FONT_INTER_SANS_MEDIUM)
            .padding(5)
            .style(|_t: &Theme, _s| text_input::Style {
                background: COLOR_BG.into(),
                border: iced::Border {
                    color: COLOR_TEXT_SECONDARY,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                placeholder: COLOR_TEXT_SECONDARY,
                value: COLOR_TEXT_PRIMARY,
                selection: COLOR_ACCENT,
                icon: COLOR_TEXT_SECONDARY,
            })
    )
    .width(Length::Fill)
    .height(Length::Fixed(40.0))
    .center_y(Length::Fill)
    .padding([0, 15])
    .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    // Layout Final
    container(
        column![
            header,
            column![
                filter_selector,
                subfilters,
                scrollable_tree,
            ].height(Length::Fill).spacing(0),
            search_bar
        ].spacing(0)
    )
    .width(Length::Fixed(200.0))
    .height(Length::Fill)
    .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
    .into()
}

fn render_alphabet<'a>(manager: &'a LibraryFiltersManager, library_manager: &'a crate::gui::library::LibraryManager) -> Element<'a, Message> {
    let mut chars = std::collections::BTreeSet::new();
    
    // Obtener caracteres con resultados basados en el filtro general seleccionado
    if let Some(songs) = &library_manager.cached_all_songs {
        for song in songs {
            let val = match manager.selected_type {
                FilterType::Artist => song.artist.as_deref().or(song.album_artist.as_deref()),
                FilterType::Album => song.album.as_deref(),
                FilterType::Genre => song.genre.as_deref(),
                FilterType::Year => song.release_year.as_deref(),
                FilterType::Folder => None, // TODO: Implementar búsqueda por carpeta relacional
            };
            
            if let Some(s) = val {
                if let Some(first_char) = s.chars().next() {
                    let first_char_upper = first_char.to_uppercase().next().unwrap();
                    if first_char_upper.is_alphabetic() {
                        chars.insert(first_char_upper.to_string());
                    } else if first_char_upper.is_numeric() {
                        chars.insert("#".to_string());
                    } else {
                        chars.insert("·".to_string());
                    }
                }
            }
        }
    }

    let mut wrap = iced_aw::widget::Wrap::new()
        .spacing(0.0)
        .line_spacing(0.0)
        .align_items(Alignment::Center);
    
    // Símbolos primero
    let symbols = vec!["#", "·"];
    for sym in symbols {
        if chars.contains(sym) {
            let is_selected = manager.selected_subfilter.as_deref() == Some(sym);
            wrap = wrap.push(create_alphabet_btn(sym.to_string(), is_selected));
        }
    }

    // Abecedario
    for c in 'A'..='Z' {
        let s = c.to_string();
        if chars.contains(&s) {
            let is_selected = manager.selected_subfilter.as_deref() == Some(&s);
            wrap = wrap.push(create_alphabet_btn(s, is_selected));
        }
    }

    container(wrap).into()
}

fn create_alphabet_btn<'a>(label: String, is_selected: bool) -> Element<'a, Message> {
    container(
        button(
            container(
                text(label.clone())
                    .size(11)
                    .font(FONT_INTER_SANS_MEDIUM)
                    .color(if is_selected { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY })
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
        )
        .width(Length::Fixed(17.0))
        .height(Length::Fixed(17.0))
        .padding(0)
        .on_press(Message::SelectSubfilter(Some(label)))
        .style(move |_, status| {
            let mut style = button::Style::default().with_background(Color::TRANSPARENT);
            if is_selected {
                style.background = Some(COLOR_ACCENT.into());
                style.border.radius = 9.0.into();
            } else if status == iced::widget::button::Status::Hovered {
                style.text_color = COLOR_TEXT_PRIMARY;
            }
            style
        })
    )
    .width(Length::Fixed(17.0))
    .height(Length::Fixed(17.0))
    .into()
}

fn render_tree<'a>(manager: &'a LibraryFiltersManager, library_manager: &'a crate::gui::library::LibraryManager) -> Element<'a, Message> {
    let mut tree_col = column![].spacing(0);
    
    // Obtener datos agrupados basados en el filtro
    if let Some(songs) = &library_manager.cached_all_songs {
        let mut groups: std::collections::BTreeMap<String, Vec<&std::sync::Arc<crate::db::database::SongData>>> = std::collections::BTreeMap::new();
        
        for song in songs {
            let key = match manager.selected_type {
                FilterType::Artist => song.artist.clone().or(song.album_artist.clone()),
                FilterType::Album => song.album.clone(),
                FilterType::Genre => song.genre.clone(),
                FilterType::Year => song.release_year.clone(),
                FilterType::Folder => None,
            }.unwrap_or_else(|| "Desconocido".to_string());
            
            // Filtro por subfiltro (letra/signo)
            if let Some(sub) = &manager.selected_subfilter {
                if let Some(first) = key.chars().next() {
                    let first_upper = first.to_uppercase().next().unwrap();
                    if sub == "#" && !first_upper.is_numeric() { continue; }
                    if sub == "·" && (first_upper.is_alphabetic() || first_upper.is_numeric()) { continue; }
                    if sub.len() == 1 && sub.chars().next().unwrap().is_alphabetic() && first_upper.to_string() != *sub { continue; }
                } else { continue; }
            }

            // Filtro por búsqueda rápida
            if !manager.search_query.is_empty() && !key.to_lowercase().contains(&manager.search_query.to_lowercase()) {
                continue;
            }

            groups.entry(key).or_default().push(song);
        }

        for (name, group_songs) in groups {
            let is_expanded = manager.expanded_nodes.contains(&name);
            let has_children = !group_songs.is_empty();
            let icon = if is_expanded { "arrow-down-chevron.svg" } else { "arrow-right-chevron.svg" };
            
            let row_content = row![
                // Botón de expansión (solo el icono)
                if has_children {
                    iced::Element::from(
                        button(
                            container(
                                svg(svg::Handle::from_path(format!("assets/icons/{}", icon)))
                                    .width(22)
                                    .height(22)
                                    .style(|_t_theme: &Theme, _s| svg::Style { color: Some(COLOR_TEXT_SECONDARY) })
                            )
                            .width(Length::Fixed(32.0))
                            .height(Length::Fixed(32.0))
                            .center_x(Length::Fill)
                            .center_y(Length::Fill)
                        )
                        .padding(0)
                        .style(|_t, _s| button::Style::default().with_background(Color::TRANSPARENT))
                        .on_press(Message::ToggleTreeNode(name.clone()))
                    )
                } else {
                    iced::Element::from(Space::new().width(Length::Fixed(32.0)))
                },
                // Botón de selección (el texto)
                button(
                    container(
                        text(name.clone())
                            .size(13)
                            .font(FONT_INTER_SANS_MEDIUM)
                            .color(COLOR_TEXT_SECONDARY)
                    )
                    .width(Length::Fill)
                    .height(Length::Fixed(32.0))
                    .center_y(Length::Fill)
                    .clip(true)
                )
                .width(Length::Fill)
                .padding([0, 5])
                .style(|_t: &Theme, status| {
                    let mut style = button::Style::default().with_background(Color::TRANSPARENT);
                    if status == iced::widget::button::Status::Hovered {
                        style.background = Some(COLOR_CONTRAST.into());
                    }
                    style
                })
                .on_press(Message::SelectTreeNode(name.clone()))
            ]
            .align_y(iced::Alignment::Center)
            .height(Length::Fixed(32.0));

            tree_col = tree_col.push(row_content);
            
            if is_expanded {
                // Renderizar sub-nodos (Álbumes si es Artista, etc.)
                let mut sub_groups = std::collections::BTreeSet::new();
                for s in group_songs {
                    let sub_key = match manager.selected_type {
                        FilterType::Artist => s.album.clone(),
                        FilterType::Album => s.artist.clone(),
                        FilterType::Genre => s.artist.clone(),
                        FilterType::Year => s.artist.clone(),
                        FilterType::Folder => None,
                    }.unwrap_or_else(|| "Desconocido".to_string());
                    sub_groups.insert(sub_key);
                }

                for sub_name in sub_groups {
                    let full_name = format!("{}|{}", name, sub_name);
                    tree_col = tree_col.push(
                        row![
                            Space::new().width(Length::Fixed(32.0)),
                            button(
                                container(
                                    text(sub_name.clone())
                                        .size(12)
                                        .font(FONT_INTER_SANS_MEDIUM)
                                        .color(COLOR_TEXT_SECONDARY)
                                )
                                .width(Length::Fill)
                                .height(Length::Fixed(32.0))
                                .center_y(Length::Fill)
                                .clip(true)
                            )
                            .width(Length::Fill)
                            .padding([0, 10])
                            .style(|_t: &Theme, status| {
                                let mut style = button::Style::default().with_background(Color::TRANSPARENT);
                                if status == iced::widget::button::Status::Hovered {
                                    style.background = Some(COLOR_CONTRAST.into());
                                }
                                style
                            })
                            .on_press(Message::SelectTreeNode(full_name))
                        ]
                        .align_y(iced::Alignment::Center)
                        .height(Length::Fixed(32.0))
                    );
                }
            }
        }
    }

    tree_col.into()
    
}
