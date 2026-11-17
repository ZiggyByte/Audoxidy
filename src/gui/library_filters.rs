use crate::gui::app::Message;
use crate::gui::dimensions::{
    BAR_BOTTOM, BAR_SUB, BAR_TOP, DIVIDER_THIN, FILTER_DROPDOWN_ICON, FILTER_DROPDOWN_WIDTH,
    FILTER_MENU_ICON, FILTER_OVERLAY_TOP, FILTER_PANEL_WIDTH, IconSize, LIST_ROW_HEADER, RADIUS_M,
    SPACE_LG, SPACE_MD, SPACE_SM, SPACE_XS, STROKE_HAIRLINE, WIDTH_SEARCH,
};
use crate::gui::theme::*;
use crate::gui::widgets::{chevron_btn_sized, standard_scrollable, standard_scrollbar};
use iced::{
    Alignment, Color, Element, Length, Theme,
    widget::{Space, button, column, container, mouse_area, row, text},
};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};

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

/// Three-level hierarchy read from the database before it is flattened into the
/// arena: L1 → L2 → set of L3. The keys are the interned strings the database
/// already returns, so the build shares them instead of copying the text.
type FilterHierarchy =
    BTreeMap<std::sync::Arc<str>, HashMap<std::sync::Arc<str>, HashSet<std::sync::Arc<str>>>>;

/// Identifier of a node inside the filter arena.
pub type FilterNodeId = u32;

/// Sentinel for a node without a parent (a root).
const NO_PARENT: FilterNodeId = FilterNodeId::MAX;

/// One node of the precomputed filter index.
///
/// Labels are the interned `Arc<str>` the database layer already returns, so the
/// index shares them with the song and album snapshots instead of copying the
/// text. A node's identity is its arena index and its ancestors are reached
/// through `parent`, which removes the per-node `"Genre|Rock|Queen"` path
/// strings the old tree stored.
#[derive(Debug, Clone)]
struct FilterNode {
    label: std::sync::Arc<str>,
    parent: FilterNodeId,
    children: Vec<FilterNodeId>,
    depth: u8,
    filter: FilterType,
}

/// Índice pre-calculado de filtros. Se construye UNA sola vez al cargar canciones.
struct FilterIndex {
    /// Flat arena; children are indices, never owned nodes.
    nodes: Vec<FilterNode>,
    /// Roots of each filter tree, sorted.
    roots: HashMap<FilterType, Vec<FilterNodeId>>,
    /// Letras del abecedario disponibles por cada FilterType
    subfilters: HashMap<FilterType, Vec<String>>,
}

#[derive(Debug, Clone, Copy)]
pub struct VisibleTreeItem {
    pub id: FilterNodeId,
    pub depth: usize,
    pub has_children: bool,
    pub is_expanded: bool,
}

pub struct LibraryFiltersManager {
    pub current_filter: FilterType,
    pub menu_open: bool,
    pub selected_subfilter: Option<String>,
    pub expanded_nodes: HashSet<FilterNodeId>,
    pub search_query: String, // internal search specific to this module

    // Índice pre-calculado (se construye una vez con build_filter_index)
    filter_index: Option<FilterIndex>,

    // Datos visibles para la UI (se actualizan con apply_view)
    pub active_subfilters: Vec<String>, // available characters e.g. ["#", "·", "A", "C", "R"]
    /// Roots of the active tree that pass the subfilter and the search.
    pub tree_data: Vec<FilterNodeId>,
    pub selected_tree_node: Option<FilterNodeId>,
    /// Per-node visibility of the active filter type for the current subfilter
    /// and search. Indexed like `FilterIndex::nodes`; rebuilt on `apply_view`.
    visible: Vec<bool>,

    // Virtualización
    pub scroll_offset: iced::Vector,
    pub last_viewport: Option<iced::Rectangle>,
    pub cached_flattened_tree: RefCell<Option<Vec<VisibleTreeItem>>>,
}

pub static FILTERS_SCROLL_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(iced::widget::Id::unique);

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
            visible: Vec::new(),
            scroll_offset: iced::Vector::new(0.0, 0.0),
            last_viewport: None,
            cached_flattened_tree: RefCell::new(None),
        }
    }
}

impl LibraryFiltersManager {
    /// Invalida el caché del árbol aplanado para liberar memoria
    pub fn invalidate_cache(&mut self) {
        *self.cached_flattened_tree.borrow_mut() = None;
    }

    /// Obtiene el grupo inicial ("#", "•", o letra mayúscula) a partir del texto
    fn get_group_char(s: &str) -> String {
        let first = s
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .next()
            .unwrap_or('?');
        if first.is_numeric() {
            "#".to_string()
        } else if first.is_alphabetic() {
            first.to_string()
        } else {
            "•".to_string()
        }
    }

    fn sort_subfilters(chars: &mut [String]) {
        chars.sort_by(|a, b| {
            if a == "#" && b != "#" {
                return std::cmp::Ordering::Less;
            }
            if b == "#" && a != "#" {
                return std::cmp::Ordering::Greater;
            }
            if a == "•" && b != "•" {
                return std::cmp::Ordering::Less;
            }
            if b == "•" && a != "•" {
                return std::cmp::Ordering::Greater;
            }
            a.cmp(b)
        });
    }

    fn sort_node_ids(ids: &mut [FilterNodeId], nodes: &[FilterNode]) {
        ids.sort_by(|&a, &b| {
            let node_a = &nodes[a as usize];
            let node_b = &nodes[b as usize];
            let group_a = Self::get_group_char(&node_a.label);
            let group_b = Self::get_group_char(&node_b.label);
            let w_a = if group_a == "#" {
                0
            } else if group_a == "•" {
                1
            } else {
                2
            };
            let w_b = if group_b == "#" {
                0
            } else if group_b == "•" {
                1
            } else {
                2
            };
            w_a.cmp(&w_b).then_with(|| {
                crate::utils::compare_strings_ignore_case(&node_a.label, &node_b.label)
            })
        });
    }

    /// Construye el índice de filtros UNA sola vez. Llamar al cargar canciones o cuando el escáner detecta cambios.
    pub fn build_filter_index(
        &mut self,
        db_m: &std::sync::Arc<std::sync::Mutex<crate::db::database::Database>>,
    ) {
        let all_types = [
            FilterType::Genre,
            FilterType::Artist,
            FilterType::Album,
            FilterType::Year,
            FilterType::Folder,
        ];

        let mut nodes: Vec<FilterNode> = Vec::new();
        let mut roots: HashMap<FilterType, Vec<FilterNodeId>> = HashMap::new();
        let mut subfilters: HashMap<FilterType, Vec<String>> = HashMap::new();

        // db-lock: solo lectura — refresco best-effort, se omite si el lock falla
        if let Ok(db) = db_m.lock() {
            for ft in &all_types {
                let filter_str = match ft {
                    FilterType::Genre => "Genre",
                    FilterType::Artist => "Artist",
                    FilterType::Album => "Album",
                    FilterType::Year => "Year",
                    FilterType::Folder => "Folder",
                };

                if let Ok(hierarchy) = db.get_filter_hierarchy(filter_str) {
                    // Estructura intermedia: L1 -> L2 -> Set<L3>. The database
                    // already returns interned strings, so this map keeps the
                    // same allocation the songs use instead of copying the text.
                    let mut indices: FilterHierarchy = BTreeMap::new();
                    let mut char_sets: HashSet<String> = HashSet::new();

                    for (l1, l2, l3_opt) in hierarchy {
                        char_sets.insert(Self::get_group_char(&l1));

                        let l2_map = indices.entry(l1).or_default();
                        let l3_set = l2_map.entry(l2).or_default();

                        if let Some(l3) = l3_opt {
                            l3_set.insert(l3);
                        }
                    }

                    // Convertir a la arena plana. Children are pushed before
                    // their parent, so a child index is known when the parent is
                    // created; the parent link is fixed right after.
                    let mut root_ids: Vec<FilterNodeId> = Vec::with_capacity(indices.len());
                    for (l1_label, l2_map) in indices {
                        let mut l2_ids: Vec<FilterNodeId> = Vec::with_capacity(l2_map.len());

                        for (l2_label, l3_set) in l2_map {
                            let mut l3_ids: Vec<FilterNodeId> = l3_set
                                .into_iter()
                                .map(|l3_label| {
                                    let id = nodes.len() as FilterNodeId;
                                    nodes.push(FilterNode {
                                        label: l3_label,
                                        parent: NO_PARENT,
                                        children: Vec::new(),
                                        depth: 2,
                                        filter: *ft,
                                    });
                                    id
                                })
                                .collect();
                            Self::sort_node_ids(&mut l3_ids, &nodes);

                            let id2 = nodes.len() as FilterNodeId;
                            let l3_children = l3_ids.clone();
                            nodes.push(FilterNode {
                                label: l2_label,
                                parent: NO_PARENT,
                                children: l3_ids,
                                depth: 1,
                                filter: *ft,
                            });
                            for child in l3_children {
                                nodes[child as usize].parent = id2;
                            }
                            l2_ids.push(id2);
                        }

                        Self::sort_node_ids(&mut l2_ids, &nodes);

                        let id1 = nodes.len() as FilterNodeId;
                        let l2_children = l2_ids.clone();
                        nodes.push(FilterNode {
                            label: l1_label,
                            parent: NO_PARENT,
                            children: l2_ids,
                            depth: 0,
                            filter: *ft,
                        });
                        for child in l2_children {
                            nodes[child as usize].parent = id1;
                        }
                        root_ids.push(id1);
                    }

                    Self::sort_node_ids(&mut root_ids, &nodes);
                    roots.insert(*ft, root_ids);

                    let mut chars: Vec<String> = char_sets.into_iter().collect();
                    Self::sort_subfilters(&mut chars);
                    subfilters.insert(*ft, chars);
                }
            }
        }

        self.filter_index = Some(FilterIndex {
            nodes,
            roots,
            subfilters,
        });

        // Actualizar la vista inmediatamente
        self.apply_view();
    }

    pub fn refresh_flattened_tree(&self) {
        let mut flattened = Vec::new();
        if let Some(index) = self.filter_index.as_ref() {
            for &root in &self.tree_data {
                Self::flatten_node(
                    index,
                    root,
                    self.expanded_nodes.contains(&root),
                    &self.visible,
                    &self.expanded_nodes,
                    &mut flattened,
                );
            }
        }
        *self.cached_flattened_tree.borrow_mut() = Some(flattened);
    }

    fn flatten_node(
        index: &FilterIndex,
        id: FilterNodeId,
        is_expanded: bool,
        visible: &[bool],
        expanded: &HashSet<FilterNodeId>,
        result: &mut Vec<VisibleTreeItem>,
    ) {
        let node = &index.nodes[id as usize];
        let has_children = node.children.iter().any(|&child| visible[child as usize]);

        result.push(VisibleTreeItem {
            id,
            depth: node.depth as usize,
            has_children,
            is_expanded,
        });

        if is_expanded {
            for &child in &node.children {
                if visible[child as usize] {
                    Self::flatten_node(
                        index,
                        child,
                        expanded.contains(&child),
                        visible,
                        expanded,
                        result,
                    );
                }
            }
        }
    }

    pub fn get_visible_tree_items(&self) -> (f32, f32, Vec<VisibleTreeItem>) {
        if self.cached_flattened_tree.borrow().is_none() {
            self.refresh_flattened_tree();
        }

        let flattened_borrow = self.cached_flattened_tree.borrow();
        let flattened = flattened_borrow.as_ref().unwrap();

        let item_height = LIST_ROW_HEADER;
        let total_items = flattened.len();

        let viewport_height = self.last_viewport.map(|v| v.height).unwrap_or(800.0);
        let scroll_y = self.scroll_offset.y;

        let start_index = (scroll_y / item_height).floor() as usize;
        // Margen dinámico = max(5, total_visible_items / 2)
        let total_visible_items = (viewport_height / item_height).ceil() as usize;
        let margin = (total_visible_items / 2).max(5);
        let start_index = start_index.saturating_sub(margin);

        let visible_count = (viewport_height / item_height).ceil() as usize + (margin * 2);

        let end_index = (start_index + visible_count).min(total_items);
        let start_index = start_index.min(total_items);

        let top_space = start_index as f32 * item_height;
        let bottom_space = (total_items.saturating_sub(end_index)) as f32 * item_height;

        // The items are plain indices and flags now, so this is a cheap copy
        // without a single string allocation.
        let visible_items = flattened[start_index..end_index].to_vec();

        (top_space, bottom_space, visible_items)
    }

    /// Label of a node, borrowed from the index.
    pub fn node_label(&self, id: FilterNodeId) -> &str {
        self.filter_index
            .as_ref()
            .and_then(|index| index.nodes.get(id as usize))
            .map(|node| node.label.as_ref())
            .unwrap_or("")
    }

    /// Filter type and the labels from the root to `id`, both included.
    ///
    /// Built only when a node is selected, never per frame.
    pub fn node_levels(&self, id: FilterNodeId) -> Option<(FilterType, Vec<&str>)> {
        let index = self.filter_index.as_ref()?;
        let node = index.nodes.get(id as usize)?;
        let mut labels = Vec::with_capacity(node.depth as usize + 1);
        let mut current = id;
        loop {
            let current_node = index.nodes.get(current as usize)?;
            labels.push(current_node.label.as_ref());
            if current_node.parent == NO_PARENT {
                break;
            }
            current = current_node.parent;
        }
        labels.reverse();
        Some((node.filter, labels))
    }

    /// Actualiza tree_data y active_subfilters desde el índice pre-calculado.
    /// Operación instantánea (<1ms). Llamar al cambiar filtro general, subfiltro o búsqueda.
    pub fn apply_view(&mut self) {
        self.tree_data.clear();
        self.active_subfilters.clear();
        self.visible.clear();

        let Some(index) = self.filter_index.as_ref() else {
            self.refresh_flattened_tree();
            return;
        };

        if let Some(chars) = index.subfilters.get(&self.current_filter) {
            self.active_subfilters = chars.clone();
        }

        let Some(roots) = index.roots.get(&self.current_filter) else {
            self.refresh_flattened_tree();
            return;
        };

        let mut visible = vec![false; index.nodes.len()];
        let has_search = !self.search_query.is_empty();
        let query_lower = self.search_query.to_lowercase();
        let subfilter = self.selected_subfilter.as_deref();

        for &root in roots {
            if let Some(sub) = subfilter
                && Self::get_group_char(&index.nodes[root as usize].label) != sub
            {
                continue;
            }
            if has_search {
                Self::mark_search_visible(index, root, &query_lower, &mut visible);
            } else {
                Self::mark_subtree_visible(index, root, &mut visible);
            }
        }

        self.tree_data = roots
            .iter()
            .copied()
            .filter(|&root| visible[root as usize])
            .collect();
        self.visible = visible;
        self.refresh_flattened_tree();
    }

    /// Marks a node and all its descendants visible.
    fn mark_subtree_visible(index: &FilterIndex, id: FilterNodeId, visible: &mut [bool]) {
        visible[id as usize] = true;
        for &child in &index.nodes[id as usize].children {
            Self::mark_subtree_visible(index, child, visible);
        }
    }

    /// Marks what a text search shows, mirroring the old tree filter: a node
    /// whose label matches keeps every descendant; otherwise only the branches
    /// that contain a match survive.
    fn mark_search_visible(
        index: &FilterIndex,
        id: FilterNodeId,
        query: &str,
        visible: &mut [bool],
    ) -> bool {
        let node = &index.nodes[id as usize];
        if node.label.to_lowercase().contains(query) {
            Self::mark_subtree_visible(index, id, visible);
            return true;
        }

        let mut any_child = false;
        for &child in &node.children {
            any_child |= Self::mark_search_visible(index, child, query, visible);
        }
        visible[id as usize] = any_child;
        any_child
    }
}

pub fn view<'a>(
    manager: &'a LibraryFiltersManager,
) -> (Element<'a, Message>, Option<Element<'a, Message>>) {
    // --- 1. TOP BAR (40px) ---
    let top_bar = mouse_area(
        container(
            text("AuDoxiDY")
                .font(FONT_STAGE_WANDER)
                .size(18)
                .color(COLOR_TEXT_PRIMARY),
        )
        .width(Length::Fill)
        .height(Length::Fixed(BAR_TOP))
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .padding(iced::Padding {
            top: 4.0,
            bottom: 0.0,
            left: 0.0,
            right: 0.0,
        })
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST)),
    )
    .on_press(Message::WindowDrag)
    .interaction(iced::mouse::Interaction::Pointer);

    // --- 2. MIDDLE SECTION ---
    let mut middle_content = column![].spacing(0);

    // Dropdown Header
    let drop_icon = if manager.menu_open {
        "arrow-up-chevron.svg"
    } else {
        "arrow-down-chevron.svg"
    };
    let dropdown_header = button(
        row![
            text(manager.current_filter.label())
                .size(14)
                .font(FONT_INTER_SANS_MEDIUM),
            Space::new().width(Length::Fill),
            iced::widget::svg(iced::widget::svg::Handle::from_path(
                crate::gui::widgets::icon_asset_path(drop_icon),
            ))
            .width(FILTER_DROPDOWN_ICON)
            .height(FILTER_DROPDOWN_ICON)
            .style(
                |_t: &Theme, _s: iced::widget::svg::Status| iced::widget::svg::Style {
                    color: Some(COLOR_TEXT_SECONDARY)
                }
            )
        ]
        .align_y(Alignment::Center)
        .padding(iced::Padding {
            top: 0.0,
            bottom: 0.0,
            left: SPACE_LG,
            right: SPACE_SM,
        }),
    )
    .width(Length::Fill)
    .height(Length::Fixed(BAR_SUB))
    .padding(0)
    .style(move |_t: &Theme, _s: button::Status| {
        let mut style = button::Style::default().with_background(COLOR_BG);
        style.border.radius = 0.0.into();
        style.text_color = COLOR_TEXT_SECONDARY;
        style
    })
    .on_press(Message::ToggleFilterMenu);

    middle_content = middle_content.push(column![
        dropdown_header,
        container(iced::widget::Space::new().height(DIVIDER_THIN))
            .width(Length::Fill)
            .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST))
    ]);

    // Build Menu Options layout (to be overlayed)
    let menu_options_container = if manager.menu_open {
        let all_filters = vec![
            FilterType::Folder,
            FilterType::Artist,
            FilterType::Album,
            FilterType::Genre,
            FilterType::Year,
        ];

        let mut content = column![]
            .spacing(0)
            .width(Length::Fixed(FILTER_DROPDOWN_WIDTH));

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
                            iced::widget::svg(iced::widget::svg::Handle::from_path(
                                crate::gui::widgets::icon_asset_path(icon_path),
                            ))
                            .width(Length::Fixed(FILTER_MENU_ICON))
                            .height(Length::Fixed(FILTER_MENU_ICON))
                            .style(|_t, _s| {
                                iced::widget::svg::Style {
                                    color: Some(COLOR_TEXT_PRIMARY),
                                }
                            })
                        )
                        .width(Length::Fixed(FILTER_MENU_ICON)),
                        Space::new().width(Length::Fixed(SPACE_LG)),
                        text(f.label())
                            .size(14)
                            .font(FONT_INTER_SANS_NORMAL)
                            .wrapping(iced::widget::text::Wrapping::None)
                    ]
                    .align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .height(Length::Fixed(LIST_ROW_HEADER))
                .padding(iced::Padding {
                    left: SPACE_LG,
                    right: SPACE_MD,
                    ..Default::default()
                })
                .align_y(iced::alignment::Vertical::Center),
            )
            .on_press(Message::ChangeGeneralFilter(f))
            .padding(0)
            .style(move |_t: &Theme, status: iced::widget::button::Status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);

                button::Style {
                    background: if is_hovered {
                        Some(COLOR_CONTRAST.into())
                    } else {
                        None
                    },
                    text_color: if is_selected {
                        COLOR_ACCENT
                    } else {
                        COLOR_TEXT_PRIMARY
                    },
                    border: iced::Border {
                        radius: 0.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                }
            });

            content = content.push(btn);
        }

        Some(
            container(content)
                .width(Length::Fixed(FILTER_DROPDOWN_WIDTH))
                .style(|_t: &Theme| {
                    container::Style::default()
                        .background(COLOR_BG)
                        .border(iced::Border {
                            color: COLOR_TEXT_SECONDARY,
                            width: STROKE_HAIRLINE,
                            radius: RADIUS_M.into(),
                        })
                        .shadow(iced::Shadow {
                            offset: iced::Vector::new(0.0, 4.0),
                            blur_radius: 10.0,
                            color: Color::from_rgba8(0, 0, 0, 0.5),
                        })
                })
                .padding(iced::Padding {
                    top: SPACE_XS,
                    bottom: SPACE_XS,
                    left: STROKE_HAIRLINE,
                    right: STROKE_HAIRLINE,
                }),
        )
    } else {
        None
    };

    // Subfilters (Alfabeto/Numeros/Signos)
    if !manager.active_subfilters.is_empty() {
        let mut rows_of_chars = column![].spacing(SPACE_XS).padding(iced::Padding {
            top: SPACE_MD,
            bottom: SPACE_MD,
            left: SPACE_LG,
            right: SPACE_LG,
        });

        let mut line_width = 0.0;
        let max_w = 202.0 - 30.0; // 202 total - 15 padding L/R
        let mut current_row = iced::widget::Row::new().spacing(0);

        let restore_icon = container(
            iced::widget::svg(iced::widget::svg::Handle::from_path(
                crate::gui::widgets::icon_asset_path("restore-straight.svg"),
            ))
            .width(IconSize::S.px())
            .height(IconSize::S.px())
            .style(move |_t, s: iced::widget::svg::Status| {
                if s == iced::widget::svg::Status::Hovered {
                    iced::widget::svg::Style {
                        color: Some(COLOR_TEXT_PRIMARY),
                    }
                } else {
                    iced::widget::svg::Style {
                        color: Some(COLOR_TEXT_SECONDARY),
                    }
                }
            }),
        )
        .width(IconSize::S.px())
        .height(IconSize::S.px())
        .center_x(Length::Fill)
        .center_y(Length::Fill);

        let restore_btn = button(restore_icon)
            .width(IconSize::S.px())
            .height(IconSize::S.px())
            .padding(0)
            .style(move |_t: &Theme, _s: button::Status| {
                let mut style = button::Style::default().with_background(Color::TRANSPARENT);
                if _s == button::Status::Pressed {
                    style.background = Some(iced::Background::Color(COLOR_ACCENT));
                    style.text_color = COLOR_TEXT_PRIMARY;
                    style.border.radius = 18.0.into();
                } else {
                    style.text_color = COLOR_TEXT_SECONDARY;
                }
                style
            })
            .on_press(Message::SelectSubfilter(None));

        current_row = current_row.push(restore_btn);
        line_width += 16.0;

        for c in &manager.active_subfilters {
            let is_selected = manager.selected_subfilter.as_deref() == Some(c.as_str());
            let clr = if is_selected {
                COLOR_TEXT_PRIMARY
            } else {
                COLOR_TEXT_SECONDARY
            };
            let size = 12.0; // 13px typopgraphy
            let char_width = 16.0; // exact layout width

            if line_width + char_width > max_w {
                rows_of_chars =
                    rows_of_chars.push(current_row.align_y(iced::alignment::Vertical::Center));
                current_row = iced::widget::Row::new().spacing(0);
                line_width = 0.0;
            }

            let char_content = container(
                text(c)
                    .size(size as f32)
                    .color(clr)
                    .font(FONT_INTER_SANS_MEDIUM),
            )
            .width(IconSize::S.px())
            .height(IconSize::S.px())
            .center_x(Length::Fill)
            .center_y(Length::Fill);

            let btn = button(char_content)
                .width(Length::Fixed(char_width))
                .height(Length::Fixed(char_width))
                .padding(0)
                .style(move |_t: &Theme, _s: button::Status| {
                    let mut style = button::Style::default();
                    if is_selected {
                        style.background = Some(iced::Background::Color(COLOR_ACCENT));
                        style.border.radius = 18.0.into();
                    } else {
                        style.background = Some(iced::Background::Color(Color::TRANSPARENT));
                    }
                    style
                })
                .on_press(Message::SelectSubfilter(Some(c.clone())));

            current_row = current_row.push(btn);
            line_width += char_width; // no spacing
        }
        if line_width > 0.0 {
            rows_of_chars =
                rows_of_chars.push(current_row.align_y(iced::alignment::Vertical::Center));
        }

        middle_content = middle_content.push(rows_of_chars);
    }

    let mut tree_col_content = column![].spacing(0);

    if !manager.active_subfilters.is_empty() {
        let (top, bottom, visible_items) = manager.get_visible_tree_items();

        if top > 0.0 {
            tree_col_content = tree_col_content.push(Space::new().height(Length::Fixed(top)));
        }

        for item in visible_items {
            let icon = if item.is_expanded {
                "arrow-down-chevron.svg"
            } else {
                "arrow-right-chevron.svg"
            };
            let padding_left = item.depth as f32 * SPACE_MD;

            let t_label = crate::gui::widgets::smart_truncate_text(
                manager.node_label(item.id).to_string(),
                13.0,
                FONT_INTER_SANS_MEDIUM,
                COLOR_TEXT_SECONDARY,
            );

            let row_content = if item.has_children {
                row![
                    chevron_btn_sized(icon, Message::ToggleTreeNode(item.id), IconSize::M),
                    Space::new().width(0.0),
                    t_label,
                ]
            } else {
                row![
                    Space::new().width(IconSize::S.px()),
                    Space::new().width(0.0),
                    t_label,
                ]
            };

            let is_active = manager.selected_tree_node == Some(item.id);

            let interactable = button(
                container(row_content.align_y(Alignment::Center))
                    .padding(iced::Padding {
                        top: 0.0,
                        bottom: 0.0,
                        left: padding_left,
                        right: SPACE_LG,
                    })
                    .height(Length::Fixed(LIST_ROW_HEADER))
                    .center_y(Length::Fill),
            )
            .width(Length::Fill)
            .style(move |_t: &Theme, _s: button::Status| {
                let mut style = button::Style::default();
                if is_active || _s == button::Status::Hovered {
                    style.background = Some(iced::Background::Color(COLOR_CONTRAST));
                } else {
                    style.background = Some(iced::Background::Color(Color::TRANSPARENT));
                }
                style
            })
            .on_press(Message::SelectTreeNode(item.id));

            tree_col_content = tree_col_content.push(interactable);
        }

        if bottom > 0.0 {
            tree_col_content = tree_col_content.push(Space::new().height(Length::Fixed(bottom)));
        }
    }

    let scrollable_tree = standard_scrollable(
        FILTERS_SCROLL_ID.clone(),
        tree_col_content,
        iced::widget::scrollable::Direction::Vertical(standard_scrollbar()),
    )
    .on_scroll(Message::FilterScroll);

    // --- 3. BOTTOM BAR (40px) ---
    let search_input = crate::gui::widgets::standard_search_input(
        None,
        "Buscar...",
        &manager.search_query,
        Message::FilterSearchChanged,
        Message::FilterSearchChanged(String::new()),
        Length::Fixed(WIDTH_SEARCH),
    );

    let bottom_bar = container(search_input)
        .width(Length::Fill)
        .height(Length::Fixed(BAR_BOTTOM))
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .padding(iced::Padding {
            top: 0.0,
            bottom: 0.0,
            left: SPACE_LG,
            right: SPACE_LG,
        })
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST));

    // Container base de contenido central inferior a The Top Bar
    let base_middle = column![
        middle_content, // header and subfilters
        container(scrollable_tree)
            .width(Length::Fill)
            .height(Length::Fill),
    ];

    let main_col = column![top_bar, base_middle, bottom_bar,];

    let base_view = container(main_col)
        .width(Length::Fixed(FILTER_PANEL_WIDTH))
        .height(Length::Fill)
        .style(|_t: &Theme| container::Style::default().background(COLOR_BG));

    let overlay = menu_options_container.map(|menu| {
        container(mouse_area(menu).on_press(Message::NoOp))
            .width(Length::Fixed(FILTER_PANEL_WIDTH))
            .height(Length::Fill)
            .padding(iced::Padding {
                top: FILTER_OVERLAY_TOP,
                bottom: 0.0,
                left: SPACE_LG,
                right: SPACE_LG,
            })
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Top)
            .into()
    });

    (base_view.into(), overlay)
}

#[cfg(test)]
#[path = "filter_arena_tests.rs"]
mod filter_arena_tests;
