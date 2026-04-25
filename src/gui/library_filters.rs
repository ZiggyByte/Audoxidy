use iced::{
    widget::{button, column, container, mouse_area, row, text, Space},
    Alignment, Color, Element, Length, Theme,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use crate::gui::app::Message;
use crate::gui::theme::*;
use crate::gui::widgets::{standard_scrollable, standard_scrollbar, chevron_btn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Índice pre-calculado de filtros. Se construye UNA sola vez al cargar canciones.
struct FilterIndex {
    /// Árbol completo y ordenado por cada FilterType
    trees: HashMap<FilterType, Vec<TreeNode>>,
    /// Letras del abecedario disponibles por cada FilterType
    subfilters: HashMap<FilterType, Vec<String>>,
}

pub struct LibraryFiltersManager {
    pub current_filter: FilterType,
    pub menu_open: bool,
    pub selected_subfilter: Option<String>,
    pub expanded_nodes: HashSet<String>,
    pub search_query: String, // internal search specific to this module

    // Índice pre-calculado (se construye una vez con build_filter_index)
    filter_index: Option<FilterIndex>,

    // Datos visibles para la UI (se actualizan con apply_view)
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
            filter_index: None,
            active_subfilters: Vec::new(),
            tree_data: Vec::new(),
            selected_tree_node: None,
        }
    }
}

impl LibraryFiltersManager {
    /// Obtiene el grupo inicial ("#", "•", o letra mayúscula) a partir del texto
    fn get_group_char(s: &str) -> String {
        let first = s.chars().next().unwrap_or('?').to_uppercase().next().unwrap_or('?');
        if first.is_numeric() {
            "#".to_string()
        } else if first.is_alphabetic() {
            first.to_string()
        } else {
            "•".to_string()
        }
    }

    fn sort_subfilters(chars: &mut Vec<String>) {
        chars.sort_by(|a, b| {
            if a == "#" && b != "#" { return std::cmp::Ordering::Less; }
            if b == "#" && a != "#" { return std::cmp::Ordering::Greater; }
            if a == "•" && b != "•" { return std::cmp::Ordering::Less; }
            if b == "•" && a != "•" { return std::cmp::Ordering::Greater; }
            a.cmp(b)
        });
    }

    fn sort_tree(nodes: &mut Vec<TreeNode>) {
        nodes.sort_by(|a, b| {
            let group_a = Self::get_group_char(&a.label);
            let group_b = Self::get_group_char(&b.label);
            let w_a = if group_a == "#" { 0 } else if group_a == "•" { 1 } else { 2 };
            let w_b = if group_b == "#" { 0 } else if group_b == "•" { 1 } else { 2 };
            w_a.cmp(&w_b).then_with(|| a.label.cmp(&b.label))
        });
        for node in nodes.iter_mut() {
            if !node.children.is_empty() {
                Self::sort_tree(&mut node.children);
            }
        }
    }

    /// Construye el índice de filtros UNA sola vez. Llamar al cargar canciones o cuando el escáner detecta cambios.
    pub fn build_filter_index(&mut self, library_manager: &crate::gui::library::LibraryManager) {
        let songs = match &library_manager.cached_all_songs {
            Some(s) => s,
            None => {
                self.filter_index = None;
                self.tree_data.clear();
                self.active_subfilters.clear();
                return;
            }
        };

        let all_types = [FilterType::Genre, FilterType::Artist, FilterType::Album, FilterType::Year, FilterType::Folder];

        // Estructura intermedia con HashMap para O(1) lookup de hijos
        // L1 -> HashMap<L1_key, HashMap<L2_key, HashSet<L3_key>>>
        let mut indices: HashMap<FilterType, BTreeMap<String, HashMap<String, HashSet<String>>>> = HashMap::new();
        let mut char_sets: HashMap<FilterType, HashSet<String>> = HashMap::new();

        for ft in &all_types {
            indices.insert(*ft, BTreeMap::new());
            char_sets.insert(*ft, HashSet::new());
        }

        // UNA sola iteración sobre todas las canciones
        for song in songs {
            let artist_val = song.artist.as_deref()
                .or(song.album_artist.as_deref())
                .unwrap_or("Desconocido");
            let album_val = song.album.as_deref().unwrap_or("Desconocido");
            let genre_val = song.genre.as_deref().unwrap_or("Desconocido");
            let year_val = song.release_year.as_deref().unwrap_or("Desconocido");

            // Para cada FilterType, extraer (L1, L2, L3)
            let mappings: [(FilterType, &str, &str, Option<&str>); 5] = [
                (FilterType::Genre,  genre_val,  artist_val, Some(album_val)),
                (FilterType::Artist, artist_val, album_val,  None),
                (FilterType::Album,  album_val,  artist_val, None),
                (FilterType::Year,   year_val,   artist_val, Some(album_val)),
                (FilterType::Folder, "Raiz",     artist_val, None),
            ];

            for (ft, l1, l2, l3) in &mappings {
                // Registrar la letra del abecedario
                char_sets.get_mut(ft).unwrap().insert(Self::get_group_char(l1));

                // Insertar en el índice
                let l1_map = indices.get_mut(ft).unwrap();
                let l2_map = l1_map.entry(l1.to_string()).or_default();
                let l3_set = l2_map.entry(l2.to_string()).or_default();
                if let Some(l3_val) = l3 {
                    l3_set.insert(l3_val.to_string());
                }
            }
        }

        // Convertir índices a TreeNodes
        let mut trees: HashMap<FilterType, Vec<TreeNode>> = HashMap::new();
        let mut subfilters: HashMap<FilterType, Vec<String>> = HashMap::new();

        for ft in &all_types {
            let ft_label = format!("{:?}", ft);
            let l1_map = indices.remove(ft).unwrap();
            let mut tree: Vec<TreeNode> = Vec::with_capacity(l1_map.len());

            for (l1_key, l2_map) in l1_map {
                let id1 = format!("{}|{}", ft_label, l1_key);
                let mut children1: Vec<TreeNode> = Vec::with_capacity(l2_map.len());

                for (l2_key, l3_set) in l2_map {
                    let id2 = format!("{}|{}", id1, l2_key);
                    let mut children2: Vec<TreeNode> = Vec::with_capacity(l3_set.len());

                    for l3_key in l3_set {
                        let id3 = format!("{}|{}", id2, l3_key);
                        children2.push(TreeNode { label: l3_key, id: id3, children: Vec::new() });
                    }

                    Self::sort_tree(&mut children2);
                    children1.push(TreeNode { label: l2_key, id: id2, children: children2 });
                }

                Self::sort_tree(&mut children1);
                tree.push(TreeNode { label: l1_key, id: id1, children: children1 });
            }

            Self::sort_tree(&mut tree);
            trees.insert(*ft, tree);

            let mut chars: Vec<String> = char_sets.remove(ft).unwrap().into_iter().collect();
            Self::sort_subfilters(&mut chars);
            subfilters.insert(*ft, chars);
        }

        self.filter_index = Some(FilterIndex { trees, subfilters });

        // Actualizar la vista inmediatamente
        self.apply_view();
    }

    /// Actualiza tree_data y active_subfilters desde el índice pre-calculado.
    /// Operación instantánea (<1ms). Llamar al cambiar filtro general, subfiltro o búsqueda.
    pub fn apply_view(&mut self) {
        self.tree_data.clear();
        self.active_subfilters.clear();

        let index = match &self.filter_index {
            Some(idx) => idx,
            None => return,
        };

        // 1. Subfilters (letras del abecedario) - directo de la caché
        if let Some(chars) = index.subfilters.get(&self.current_filter) {
            self.active_subfilters = chars.clone();
        }

        // 2. Árbol filtrado
        if let Some(full_tree) = index.trees.get(&self.current_filter) {
            let has_search = !self.search_query.is_empty();
            let query_lower = self.search_query.to_lowercase();

            for node in full_tree {
                // Filtro por subfiltro (letra del abecedario)
                if let Some(ref sub) = self.selected_subfilter {
                    let group = Self::get_group_char(&node.label);
                    if &group != sub {
                        continue;
                    }
                }

                // Filtro por búsqueda de texto
                if has_search {
                    let filtered = Self::filter_node_by_search(node, &query_lower);
                    if let Some(filtered_node) = filtered {
                        self.tree_data.push(filtered_node);
                    }
                } else {
                    self.tree_data.push(node.clone());
                }
            }
        }
    }

    /// Filtra recursivamente un nodo del árbol por búsqueda de texto.
    /// Retorna None si ni el nodo ni sus hijos coinciden.
    fn filter_node_by_search(node: &TreeNode, query: &str) -> Option<TreeNode> {
        let self_matches = node.label.to_lowercase().contains(query);

        // Filtrar hijos recursivamente
        let filtered_children: Vec<TreeNode> = node.children.iter()
            .filter_map(|child| Self::filter_node_by_search(child, query))
            .collect();

        if self_matches || !filtered_children.is_empty() {
            Some(TreeNode {
                label: node.label.clone(),
                id: node.id.clone(),
                children: if self_matches {
                    // Si el padre coincide, mostrar todos sus hijos
                    node.children.clone()
                } else {
                    // Si solo coinciden hijos, mostrar solo los que coinciden
                    filtered_children
                },
            })
        } else {
            None
        }
    }
}

pub fn view<'a>(
    manager: &'a LibraryFiltersManager,
) -> (Element<'a, Message>, Option<Element<'a, Message>>) {
    
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
    let drop_icon = if manager.menu_open { "arrow-up-chevron.svg" } else { "arrow-down-chevron.svg" };
    let dropdown_header = button(
        row![
            text(manager.current_filter.label()).size(14).font(FONT_INTER_SANS_MEDIUM),
            Space::new().width(Length::Fill),
            iced::widget::svg(iced::widget::svg::Handle::from_path(format!("assets/icons/{}", drop_icon)))
                .width(28).height(28)
                .style(|_t: &Theme, _s: iced::widget::svg::Status| iced::widget::svg::Style { color: Some(COLOR_TEXT_SECONDARY) })
        ]
        .align_y(Alignment::Center)
        .padding(iced::Padding { top: 0.0, bottom: 0.0, left: 15.0, right: 7.0 })
    )
    .width(Length::Fill)
    .height(Length::Fixed(28.0))
    .padding(0)
    .style(move |_t: &Theme, _s: button::Status| {
        let mut st = button::Style::default().with_background(COLOR_BG);
        st.border.radius = 0.0.into();
        st.text_color = COLOR_TEXT_SECONDARY;
        st
    })
    .on_press(Message::ToggleFilterMenu);

    middle_content = middle_content.push(
        column![
            dropdown_header,
            container(iced::widget::Space::new().height(2.0))
                .width(Length::Fill)
                .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST))
        ]
    );

    // Build Menu Options layout (to be overlayed)
    let menu_options_container = if manager.menu_open {
        let all_filters = vec![
            FilterType::Folder, FilterType::Artist, FilterType::Album, FilterType::Genre, FilterType::Year
        ];
        
        let mut content = column![].spacing(0).width(Length::Fixed(172.0));
        
        for f in all_filters {
            let is_selected = manager.current_filter == f;
            
            let icon_path = match f {
                FilterType::Folder => "folder-open-outlined.svg",
                FilterType::Artist => "artist-outlined-straight.svg",
                FilterType::Album => "album.svg",
                FilterType::Genre => "music-note-2-straight.svg",
                FilterType::Year => "calendar-rounded.svg",
            };
            
            let btn = button(
                container(
                    row![
                        container(
                            iced::widget::svg(iced::widget::svg::Handle::from_path(format!("assets/icons/{}", icon_path)))
                                .width(Length::Fixed(18.0))
                                .height(Length::Fixed(18.0))
                                .style(|_t, _s| iced::widget::svg::Style {
                                    color: Some(COLOR_TEXT_PRIMARY),
                                })
                        ).width(Length::Fixed(18.0)),
                        Space::new().width(Length::Fixed(15.0)),
                        text(f.label())
                            .size(14)
                            .font(FONT_INTER_SANS_NORMAL)
                            .wrapping(iced::widget::text::Wrapping::None)
                    ]
                    .align_y(Alignment::Center)
                )
                .width(Length::Fill)
                .height(Length::Fixed(32.0))
                .padding(iced::Padding { left: 15.0, right: 10.0, ..Default::default() })
                .align_y(iced::alignment::Vertical::Center)
            )
            .on_press(Message::ChangeGeneralFilter(f))
            .padding(0)
            .style(move |_t: &Theme, status: iced::widget::button::Status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                
                button::Style {
                    background: if is_hovered { Some(COLOR_CONTRAST.into()) } else { None },
                    text_color: if is_selected { COLOR_ACCENT } else { COLOR_TEXT_PRIMARY },
                    border: iced::Border { radius: 0.0.into(), width: 0.0, color: Color::TRANSPARENT },
                    ..Default::default()
                }
            });

            content = content.push(btn);
        }
        
        Some(
            container(content)
                .width(Length::Fixed(172.0))
                .style(|_t: &Theme| {
                    container::Style::default()
                        .background(COLOR_BG)
                        .border(iced::Border {
                            color: COLOR_TEXT_SECONDARY,
                            width: 1.0,
                            radius: 8.0.into(),
                        })
                        .shadow(iced::Shadow {
                            offset: iced::Vector::new(0.0, 4.0),
                            blur_radius: 10.0,
                            color: Color::from_rgba8(0, 0, 0, 0.5),
                        })
                })
                .padding(iced::Padding { top: 5.0, bottom: 5.0, left: 1.0, right: 1.0 })
        )
    } else {
        None
    };

    // Subfilters (Alfabeto/Numeros/Signos)
    if !manager.active_subfilters.is_empty() {
        let mut rows_of_chars = column![].spacing(4).padding(iced::Padding { top: 10.0, bottom: 10.0, left: 15.0, right: 15.0 });
        
        let mut line_width = 0.0;
        let max_w = 202.0 - 30.0; // 202 total - 15 padding L/R
        let mut current_row = iced::widget::Row::new().spacing(0);
        
        let restore_icon = container(
            iced::widget::svg(iced::widget::svg::Handle::from_path("assets/icons/restore-straight.svg"))
                .width(14).height(14)
                .style(move |_t, s: iced::widget::svg::Status| {
                    if s == iced::widget::svg::Status::Hovered {
                        iced::widget::svg::Style { color: Some(COLOR_TEXT_PRIMARY) }
                    } else {
                        iced::widget::svg::Style { color: Some(COLOR_TEXT_SECONDARY) }
                    }
                })
        )
            .width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).center_x(Length::Fill).center_y(Length::Fill);

        let restore_btn = button(restore_icon)
             .width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).padding(0)
             .style(move |_t: &Theme, _s: button::Status| {
                 let mut st = button::Style::default().with_background(Color::TRANSPARENT);
                 if _s == button::Status::Pressed {
                     st.background = Some(iced::Background::Color(COLOR_ACCENT));
                     st.text_color = COLOR_TEXT_PRIMARY;
                     st.border.radius = 18.0.into();
                 }
                 else {
                     st.text_color = COLOR_TEXT_SECONDARY;
                 }
                 st
             })
             .on_press(Message::SelectSubfilter(None));
        
        current_row = current_row.push(restore_btn);
        line_width += 16.0;
        
        for c in &manager.active_subfilters {
            let is_selected = manager.selected_subfilter.as_deref() == Some(c.as_str());
            let clr = if is_selected { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };
            let size = 12.0; // 13px typopgraphy
            let char_width = 16.0; // exact layout width

            if line_width + char_width > max_w {
                rows_of_chars = rows_of_chars.push(current_row.align_y(iced::alignment::Vertical::Center));
                current_row = iced::widget::Row::new().spacing(0);
                line_width = 0.0;
            }
            
            let char_content = container(text(c).size(size as f32).color(clr).font(FONT_INTER_SANS_MEDIUM))
                .width(Length::Fixed(16.0)).height(Length::Fixed(16.0)).center_x(Length::Fill).center_y(Length::Fill);

            let btn = button(char_content)
            .width(Length::Fixed(char_width))
            .height(Length::Fixed(char_width))
            .padding(0)
            .style(move |_t: &Theme, _s: button::Status| {
                 let mut st = button::Style::default();
                 if is_selected {
                     st.background = Some(iced::Background::Color(COLOR_ACCENT));
                     st.border.radius = 18.0.into();
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
        
    }
    
    let mut tree_col_content: Element<'a, Message> = column![].into();

    if !manager.active_subfilters.is_empty() {
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
                let t_len = (23 - (depth * 2)).max(1);
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

    let scrollable_tree = standard_scrollable(
        iced::widget::Id::unique(),
        tree_col_content,
        iced::widget::scrollable::Direction::Vertical(standard_scrollbar())
    );



    // --- 3. BOTTOM BAR (40px) ---
    let search_input = crate::gui::widgets::standard_search_input(
        "Buscar...",
        &manager.search_query,
        Message::FilterSearchChanged,
        Message::FilterSearchChanged(String::new()),
        Length::Fixed(180.0),
    );

    let bottom_bar = container(search_input)
        .width(Length::Fill)
        .height(Length::Fixed(40.0))
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));


    // Container base de contenido central inferior a The Top Bar
    let base_middle = column![
        middle_content, // header and subfilters
        container(scrollable_tree).width(Length::Fill).height(Length::Fill),
    ];

    let main_col = column![
        top_bar,
        base_middle,
        bottom_bar,
    ];

    let base_view = container(main_col)
        .width(Length::Fixed(202.0))
        .height(Length::Fill)
        .style(|_t: &Theme| container::Style::default().background(COLOR_BG));

    let overlay = if let Some(menu) = menu_options_container {
        Some(
            container(
                mouse_area(menu).on_press(Message::NoOp)
            )
            .width(Length::Fixed(202.0))
            .height(Length::Fill)
            .padding(iced::Padding { top: 70.0, bottom: 0.0, left: 0.0, right: 0.0 })
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Top)
            .into()
        )
    } else {
        None
    };

    (base_view.into(), overlay)
}
