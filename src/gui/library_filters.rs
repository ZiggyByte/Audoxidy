use iced::{
    widget::{button, column, container, row, scrollable, text, text_input, Space},
    Alignment, Color, Element, Length, Theme,
};
use std::collections::{BTreeMap, HashSet};
use crate::gui::app::Message;
use crate::gui::theme::*;
use crate::gui::widgets::{custom_scrollbar_style, chevron_btn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    Folder,
    Artist,
    Album,
    Genre,
    Year,
}

impl FilterType {
    pub fn label(&self) -> &'static str {
        match self {
            FilterType::Folder => "Carpeta",
            FilterType::Artist => "Artista",
            FilterType::Album => "Álbum",
            FilterType::Genre => "Género",
            FilterType::Year => "Año",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub label: String,
    pub id: String, // format: "FilterType|L1|L2|...". e.g. "Genre|Rock|Nirvana|Nevermind"
    pub children: Vec<TreeNode>,
}

pub struct LibraryFiltersManager {
    pub current_filter: FilterType,
    pub menu_open: bool,
    pub selected_subfilter: Option<String>,
    pub expanded_nodes: HashSet<String>,
    pub search_query: String, // internal search internal specific to this module

    // Cached built variables
    pub active_subfilters: Vec<String>, // available characters e.g. ["#", "·", "A", "C", "R"]
    pub tree_data: Vec<TreeNode>,
    pub selected_tree_node: Option<String>,
}

impl Default for LibraryFiltersManager {
    fn default() -> Self {
        Self {
            current_filter: FilterType::Genre,
            menu_open: false,
            selected_subfilter: None,
            expanded_nodes: HashSet::new(),
            search_query: String::new(),
            active_subfilters: Vec::new(),
            tree_data: Vec::new(),
            selected_tree_node: None,
        }
    }
}

impl LibraryFiltersManager {
    /// Obtiene el grupo inicial ("#", "·", o letra mayúscula) a partir del texto
    fn get_group_char(s: &str) -> String {
        let first = s.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?');
        if first.is_numeric() {
            "#".to_string()
        } else if first.is_alphabetic() {
            first.to_string()
        } else {
            "·".to_string()
        }
    }

    /// Refresh method to rebuild the tree when DB or options change
    pub fn refresh_data(&mut self, library_manager: &crate::gui::library::LibraryManager) {
        self.tree_data.clear();
        self.active_subfilters.clear();

        // 1. Recolectar pares dependiendo del `current_filter`
        // Para Genre: Genre -> Artist -> Album
        // Para Artist: Artist -> Album
        // etc...
        let songs_opt = &library_manager.cached_all_songs;
        let mut level1_map: BTreeMap<String, TreeNode> = BTreeMap::new();

        if let Some(songs) = songs_opt {
            for song in songs {
                // Ignore songs that don't match the internal text search (if not empty)
                if !self.search_query.is_empty() {
                    let q = self.search_query.to_lowercase();
                    let t = song.title.as_deref().unwrap_or("").to_lowercase();
                    let a = song.artist.as_deref().unwrap_or("").to_lowercase();
                    let al = song.album.as_deref().unwrap_or("").to_lowercase();
                    let g = song.genre.as_deref().unwrap_or("").to_lowercase();
                    let y = song.release_year.as_deref().unwrap_or("").to_lowercase();
                    if !t.contains(&q) && !a.contains(&q) && !al.contains(&q) && !g.contains(&q) && !y.contains(&q) {
                        continue;
                    }
                }

                let artist_val = song.artist.clone().or_else(|| song.album_artist.clone()).unwrap_or_else(|| "Desconocido".to_string());
                let album_val = song.album.clone().unwrap_or_else(|| "Desconocido".to_string());
                let genre_val = song.genre.clone().unwrap_or_else(|| "Desconocido".to_string());
                let year_val = song.release_year.clone().unwrap_or_else(|| "Desconocido".to_string());
                // let track_val = song.title.clone().unwrap_or_else(|| "Pista Desconocida".to_string());

                let (l1, l2, l3, l4) = match self.current_filter {
                    FilterType::Genre => (Some(genre_val), Some(artist_val), Some(album_val), None),
                    FilterType::Artist => (Some(artist_val), Some(album_val), None, None),
                    FilterType::Album => (Some(album_val), Some(artist_val), None, None), // Album -> Artist
                    FilterType::Year => (Some(year_val), Some(artist_val), Some(album_val), None),
                    FilterType::Folder => {
                        // TODO: Map Folders logic. Temporarily using track details for structural check.
                        // Wait, Folders usually uses the file path parts.
                        // Let's fallback to artists for structure tests, we will improve folder logic later.
                        (Some("Raiz".to_string()), Some(artist_val), None, None)
                    }
                };

                if let Some(lvl1_key) = l1 {
                    let group_char = Self::get_group_char(&lvl1_key);
                    
                    // Solo lo agregamos si no hay subfiltro seleccionado, O si coincide con el subfiltro.
                    if self.selected_subfilter.is_none() || self.selected_subfilter.as_deref() == Some(group_char.as_str()) {
                        let id1 = format!("{:?}|{}", self.current_filter, lvl1_key);
                        let node1 = level1_map.entry(lvl1_key.clone()).or_insert_with(|| TreeNode {
                            label: lvl1_key,
                            id: id1.clone(),
                            children: Vec::new(),
                        });

                        if let Some(lvl2_key) = l2 {
                            let id2 = format!("{}|{}", id1, lvl2_key);
                            let node2_idx_opt = node1.children.iter().position(|c| c.label == lvl2_key);
                            
                            let node2_idx = if let Some(idx) = node2_idx_opt {
                                idx
                            } else {
                                node1.children.push(TreeNode {
                                    label: lvl2_key.clone(),
                                    id: id2.clone(),
                                    children: Vec::new(),
                                });
                                node1.children.len() - 1
                            };

                            if let Some(lvl3_key) = l3 {
                                let id3 = format!("{}|{}", id2, lvl3_key);
                                let node3_idx_opt = node1.children[node2_idx].children.iter().position(|c| c.label == lvl3_key);
                                
                                let node3_idx = if let Some(idx) = node3_idx_opt {
                                    idx
                                } else {
                                    node1.children[node2_idx].children.push(TreeNode {
                                        label: lvl3_key,
                                        id: id3.clone(),
                                        children: Vec::new(),
                                    });
                                    node1.children[node2_idx].children.len() - 1
                                };
                                
                                if let Some(lvl4_key) = l4 {
                                    let id4 = format!("{}|{}", id3, lvl4_key);
                                    let has_n4 = node1.children[node2_idx].children[node3_idx].children.iter().any(|c| c.label == lvl4_key);
                                    if !has_n4 {
                                        node1.children[node2_idx].children[node3_idx].children.push(TreeNode {
                                            label: lvl4_key,
                                            id: id4,
                                            children: Vec::new(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            // Collect the active subfilters by inspecting all songs regardless of selected_subfilter
            // (We want to show the full alphabet ribbon always, but only letters that have at least one match).
            let mut unique_chars = HashSet::new();
            for song in songs {
                if !self.search_query.is_empty() {
                    let q = self.search_query.to_lowercase();
                    let t = song.title.as_deref().unwrap_or("").to_lowercase();
                    let a = song.artist.as_deref().unwrap_or("").to_lowercase();
                    let al = song.album.as_deref().unwrap_or("").to_lowercase();
                    let g = song.genre.as_deref().unwrap_or("").to_lowercase();
                    let y = song.release_year.as_deref().unwrap_or("").to_lowercase();
                    if !t.contains(&q) && !a.contains(&q) && !al.contains(&q) && !g.contains(&q) && !y.contains(&q) {
                        continue;
                    }
                }

                let artist_val = song.artist.clone().or_else(|| song.album_artist.clone()).unwrap_or_else(|| "Desconocido".to_string());
                let album_val = song.album.clone().unwrap_or_else(|| "Desconocido".to_string());
                let genre_val = song.genre.clone().unwrap_or_else(|| "Desconocido".to_string());
                let year_val = song.release_year.clone().unwrap_or_else(|| "Desconocido".to_string());

                let l1 = match self.current_filter {
                    FilterType::Genre => genre_val,
                    FilterType::Artist => artist_val,
                    FilterType::Album => album_val,
                    FilterType::Year => year_val,
                    FilterType::Folder => "Raiz".to_string(), // Placeholder
                };
                unique_chars.insert(Self::get_group_char(&l1));
            }
            
            let mut sorted_chars: Vec<String> = unique_chars.into_iter().collect();
            sorted_chars.sort_by(|a, b| {
                if a == "#" && b != "#" { return std::cmp::Ordering::Less; }
                if b == "#" && a != "#" { return std::cmp::Ordering::Greater; }
                if a == "·" && b != "·" { return std::cmp::Ordering::Less; }
                if b == "·" && a != "·" { return std::cmp::Ordering::Greater; }
                a.cmp(b)
            });
            self.active_subfilters = sorted_chars;
        }

        self.tree_data = level1_map.into_values().collect();
    }
}

pub fn view<'a>(
    manager: &'a LibraryFiltersManager,
) -> Element<'a, Message> {
    
    // --- 1. TOP BAR (40px) ---
    let top_bar = container(
        text("AuDoxiDY")
            .font(FONT_STAGE_WANDER)
            .size(19)
            .color(COLOR_TEXT_PRIMARY)
    )
    .width(Length::Fill)
    .height(Length::Fixed(40.0))
    .align_x(iced::alignment::Horizontal::Center)
    .padding(iced::Padding { top: 9.0, bottom: 0.0, left: 0.0, right: 0.0 })
    .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    // --- 2. MIDDLE SECTION ---
    let mut middle_content = column![].spacing(0);

    // Dropdown Header
    let dropdown_header = button(
        row![
            text(manager.current_filter.label()).size(15).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM),
            Space::new().width(Length::Fill),
            text(if manager.menu_open { "▲" } else { "▼" }).size(10).color(COLOR_TEXT_SECONDARY)
        ]
        .align_y(Alignment::Center)
        .padding(iced::Padding { top: 0.0, bottom: 0.0, left: 15.0, right: 15.0 })
    )
    .width(Length::Fill)
    .height(Length::Fixed(30.0))
    .style(move |_t: &Theme, status: button::Status| {
        let mut st = button::Style::default().with_background(if manager.menu_open { COLOR_CONTRAST } else { COLOR_CONTRAST });
        st.border.radius = 0.0.into();
        st
    })
    .on_press(Message::ToggleFilterMenu);

    middle_content = middle_content.push(dropdown_header);

    // Dropdown Items List
    if manager.menu_open {
        let all_filters = vec![
            FilterType::Folder, FilterType::Artist, FilterType::Album, FilterType::Genre, FilterType::Year
        ];
        let mut menu_options = column![].spacing(0);
        for f in all_filters {
            let is_selected = manager.current_filter == f;
            menu_options = menu_options.push(
                button(
                    container(text(f.label()).size(15).color(if is_selected { COLOR_ACCENT } else { COLOR_TEXT_SECONDARY }).font(FONT_INTER_SANS_MEDIUM))
                        .padding(iced::Padding { top: 0.0, bottom: 0.0, left: 15.0, right: 15.0 })
                        .height(Length::Fixed(30.0))
                        .center_y(Length::Fill)
                )
                .width(Length::Fill)
                .style(|_t: &Theme, _s| {
                    let mut st = button::Style::default().with_background(COLOR_CONTRAST);
                    st.border.radius = 0.0.into();
                    st
                })
                .on_press(Message::ChangeGeneralFilter(f))
            );
        }
        middle_content = middle_content.push(container(menu_options).width(Length::Fill).style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST)));
    }

    // Subfilters (Alfabeto/Numeros/Signos)
    if !manager.menu_open && !manager.active_subfilters.is_empty() {
        let mut rows_of_chars = column![].spacing(0).padding(iced::Padding { top: 10.0, bottom: 10.0, left: 15.0, right: 15.0 });
        
        let mut line_width = 0.0;
        let max_w = 202.0 - 30.0; // 202 total - 15 padding L/R
        let mut current_row = iced::widget::Row::new().spacing(0);
        
        for c in &manager.active_subfilters {
            let is_selected = manager.selected_subfilter.as_deref() == Some(c.as_str());
            let clr = if is_selected { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };
            let size = 13.0; // 13px typopgraphy
            let char_width = 17.0; // exact layout width

            if line_width + char_width > max_w {
                rows_of_chars = rows_of_chars.push(current_row.align_y(iced::alignment::Vertical::Center));
                current_row = iced::widget::Row::new().spacing(0);
                line_width = 0.0;
            }
            
            let char_content = container(text(c).size(size as f32).color(clr).font(FONT_INTER_SANS_MEDIUM))
                .width(Length::Fixed(17.0)).height(Length::Fixed(17.0)).center_x(Length::Fill).center_y(Length::Fill);

            let btn = button(char_content)
            .width(Length::Fixed(char_width))
            .height(Length::Fixed(char_width))
            .padding(0)
            .style(move |_t: &Theme, _s: button::Status| {
                 let mut st = button::Style::default();
                 if is_selected {
                     st.background = Some(iced::Background::Color(COLOR_ACCENT));
                     st.border.radius = 4.0.into();
                 } else {
                     st.background = Some(iced::Background::Color(Color::TRANSPARENT));
                 }
                 st
             })
            .on_press(Message::SelectSubfilter(Some(c.clone())));
            
            current_row = current_row.push(btn);
            line_width += char_width; // no spacing
        }
        if line_width > 0.0 {
            rows_of_chars = rows_of_chars.push(current_row.align_y(iced::alignment::Vertical::Center));
        }
        
        middle_content = middle_content.push(rows_of_chars);
        
        // (mostrar todo) link
        middle_content = middle_content.push(
            container(
                button(text("mostrar todo").size(11).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM))
                    .padding([5, 10])
                    .style(|_t: &Theme, _s: button::Status| {
                        let mut st = button::Style::default();
                        st.background = Some(iced::Background::Color(if _s == button::Status::Hovered { COLOR_CONTRAST } else { Color::TRANSPARENT }));
                        st.border.radius = 4.0.into();
                        st
                    })
                    .on_press(Message::SelectSubfilter(None))
            )
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .padding(iced::Padding { top: 10.0, bottom: 10.0, left: 0.0, right: 0.0 })
        );
    }
    
    let mut tree_col_content: Element<'a, Message> = column![].into();

    if !manager.menu_open && !manager.active_subfilters.is_empty() {
        // Tree Recursive Rendering
        fn render_tree<'a>(nodes: &'a [TreeNode], expanded: &'a HashSet<String>, depth: usize, manager: &'a LibraryFiltersManager) -> Element<'a, Message> {
            let mut col = column![].spacing(0);
            for node in nodes {
                let is_expanded = expanded.contains(&node.id);
                let has_children = !node.children.is_empty();
                
                let icon = if node.id.contains("FilterType::Folder") {
                    if is_expanded { "arrow-down-chevron.svg" } else { "arrow-right-chevron.svg" }
                } else {
                    if is_expanded { "arrow-down-chevron.svg" } else { "arrow-right-chevron.svg" }
                };

                let padding_left = 0.0 + (depth as f32 * 12.0); // Indentation
                let t_len = (28 - (depth * 2)).max(1);
                let t_label = text(crate::utils::truncate_text(&node.label, t_len)).size(13).color(COLOR_TEXT_SECONDARY).font(FONT_INTER_SANS_MEDIUM).wrapping(iced::widget::text::Wrapping::None);
                
                let row_content = if has_children {
                    row![
                        chevron_btn(icon, Message::ToggleTreeNode(node.id.clone()), 22.0, 22.0),
                        Space::new().width(0.0),
                        t_label,
                    ]
                } else {
                    row![
                        Space::new().width(16.0),
                        Space::new().width(0.0),
                        t_label,
                    ]
                };

                let is_active = manager.selected_tree_node.as_deref() == Some(node.id.as_str());

                let interactable = button(
                    container(row_content.align_y(Alignment::Center))
                        .padding(iced::Padding { top: 0.0, bottom: 0.0, left: padding_left, right: 15.0 })
                        .height(Length::Fixed(32.0))
                        .center_y(Length::Fill)
                )
                .width(Length::Fill)
                .style(move |_t: &Theme, _s: button::Status| {
                    let mut st = button::Style::default();
                    if is_active || _s == button::Status::Hovered {
                        st.background = Some(iced::Background::Color(COLOR_CONTRAST));
                    } else {
                        st.background = Some(iced::Background::Color(Color::TRANSPARENT));
                    }
                    st
                })
                .on_press(Message::SelectTreeNode(node.id.clone()));

                col = col.push(interactable);

                if is_expanded && has_children {
                    col = col.push(render_tree(&node.children, expanded, depth + 1, manager));
                }
            }
            col.into()
        }

        tree_col_content = render_tree(&manager.tree_data, &manager.expanded_nodes, 0, manager);
    }

    let scrollable_tree = scrollable(tree_col_content)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(4)
                .scroller_width(4)
                .margin(0),
        ))
        .style(custom_scrollbar_style);



    // --- 3. BOTTOM BAR (40px) ---
    let search_input = text_input("Búsqueda rápida", &manager.search_query)
        .on_input(Message::FilterSearchChanged)
        .size(13)
        .padding([4, 8])
        .font(FONT_INTER_SANS_MEDIUM)
        .width(Length::Fixed(160.0));

    let bottom_bar = container(search_input)
        .width(Length::Fill)
        .height(Length::Fixed(40.0))
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));


    // Main wrapping
    container(
        column![
            top_bar,
            middle_content, // static fixed
            container(scrollable_tree).width(Length::Fill).height(Length::Fill),
            bottom_bar,
        ]
    )
    .width(Length::Fixed(202.0))
    .height(Length::Fill)
    .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
    .into()
}
