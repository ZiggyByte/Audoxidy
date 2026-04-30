use iced::{
    widget::{button, column, container, row, text, svg, mouse_area, Space},
    Alignment, Color, Element, Length, Theme, Background, Padding,
};
use crate::audio::AudioManager;
use crate::db::database::{PlaylistFolderGroup, PlaylistSongRef, ShuffleSession};
use crate::gui::app::Message;
use crate::gui::theme::*;
use crate::gui::widgets::{standard_scrollable, standard_scrollbar};
use crate::utils::{format_duration};

// ============================================================
// Constantes y IDs
// ============================================================

pub static PLAYLIST_SCROLL_ID: std::sync::LazyLock<iced::widget::Id> = std::sync::LazyLock::new(iced::widget::Id::unique);
pub static PLAYLIST_TABS_SCROLL_ID: std::sync::LazyLock<iced::widget::Id> = std::sync::LazyLock::new(iced::widget::Id::unique);
pub static PLAYLIST_SEARCH_ID: std::sync::LazyLock<iced::widget::Id> = std::sync::LazyLock::new(|| iced::widget::Id::new("playlist_search_input"));

// ============================================================
// Tipos auxiliares
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum PlaylistItemType {
    Separator(String, i64), // folder_path, first_item_id
    Song(String, usize, i64), // folder_path, index local dentro del grupo, first_item_id
}

// ============================================================
// Estado del PlaylistManager
// ============================================================

#[derive(Clone)]
pub struct PlaylistManager {
    /// ID de la playlist activa (referencia a BD)
    pub active_playlist_id: i64,

    /// Canciones de la playlist activa, agrupadas por carpeta
    pub groups: Vec<PlaylistFolderGroup>,

    /// Índice de la canción que se está reproduciendo (dentro del grupo aplanado)
    pub playing_song_idx: Option<usize>,

    /// Selected items in the current playlist
    pub selected_idxs: std::collections::HashSet<usize>,

    /// Currently focused item for keyboard navigation
    pub focused_idx: Option<usize>,

    /// Selection pivot for Shift-range selection
    pub selection_pivot: Option<usize>,

    /// Estado shuffle/repeat
    pub shuffle_active: bool,
    pub repeat_mode: u8, // 0=Off, 1=All, 2=One

    /// Búsqueda local dentro de la playlist
    pub search_query: String,
    pub filtered_groups: Option<Vec<PlaylistFolderGroup>>,

    /// Estado de la barra de pestañas
    pub show_tab_dropdown: bool,

    /// IDs de pestañas visibles (para overflow management)
    pub visible_tabs_count: Option<usize>,

    /// Sesión de shuffle activa
    pub shuffle_session: Option<ShuffleSession>,

    /// Lista de todas las playlists disponibles (para la barra de pestañas)
    pub playlists: Vec<crate::db::database::PlaylistData>,

    /// Carpetas plegadas (IDs de la primera canción del grupo como anchor)
    pub collapsed_groups: std::collections::HashSet<i64>,

    /// Información del último clic para detectar doble clic (linear_idx, instant)
    pub last_click_info: Option<(usize, std::time::Instant)>,

    /// Guardar el último viewport (ventana visible) para autoscroll
    pub last_viewport: Option<iced::Rectangle>,

    /// Guardar el último viewport de la barra de pestañas
    pub tabs_viewport: Option<iced::Rectangle>,
}

impl Default for PlaylistManager {
    fn default() -> Self {
        Self {
            active_playlist_id: 1,
            groups: Vec::new(),
            playing_song_idx: None,
            selected_idxs: std::collections::HashSet::new(),
            focused_idx: None,
            selection_pivot: None,
            shuffle_active: false,
            repeat_mode: 0,
            search_query: String::new(),
            filtered_groups: None,
            show_tab_dropdown: false,
            visible_tabs_count: None,
            shuffle_session: None,
            playlists: Vec::new(),
            collapsed_groups: std::collections::HashSet::new(),
            last_click_info: None,
            last_viewport: None,
            tabs_viewport: None,
        }
    }
}

/// Aplanar todos los grupos en un solo índice lineal
pub fn flatten_groups(groups: &[PlaylistFolderGroup]) -> Vec<&PlaylistSongRef> {
    groups.iter()
        .flat_map(|g| g.songs.iter())
        .collect()
}

/// Contar items lineales (separadores + canciones)
pub fn count_linear_items(groups: &[PlaylistFolderGroup]) -> usize {
    groups.iter().map(|g| 1 + g.songs.len()).sum()
}

impl PlaylistManager {
    pub fn apply_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_groups = None;
            return;
        }

        let query = self.search_query.to_lowercase();
        let mut filtered: Vec<PlaylistFolderGroup> = Vec::new();

        for group in &self.groups {
            let matching_songs: Vec<PlaylistSongRef> = group.songs.iter()
                .filter(|s| {
                    s.title.to_lowercase().contains(&query)
                        || s.artist_name.to_lowercase().contains(&query)
                        || s.album_artist_name.to_lowercase().contains(&query)
                        || s.album_title.to_lowercase().contains(&query)
                })
                .cloned()
                .collect();

            if !matching_songs.is_empty() {
                let mut g = group.clone();
                g.songs = matching_songs;
                g.total_duration = g.songs.iter().map(|s| s.duration).sum();
                filtered.push(g);
            }
        }

        self.filtered_groups = if filtered.is_empty() { None } else { Some(filtered) };
    }

    /// Obtiene la siguiente canción para precarga de carátulas (basado en lógica secuencial)
    pub fn get_next_song_ref(&self) -> Option<&PlaylistSongRef> {
        let current = self.playing_song_idx?;
        let next_idx = if self.repeat_mode == 2 {
            Some(current)
        } else if self.shuffle_active {
            // En shuffle es difícil predecir sin mirar la sesión, pero podemos intentar
            self.shuffle_session.as_ref().and_then(|session| {
                if session.current_position < session.shuffle_order.len() {
                    let next_song_id = session.shuffle_order[session.current_position];
                    self.get_linear_index_by_song_id(next_song_id)
                } else {
                    None
                }
            })
        } else {
            self.find_next_enabled_song_internal(current)
        };

        next_idx.and_then(|idx| self.get_song_at_linear_index(idx))
    }

    /// Obtener los grupos activos (filtrados o no)
    pub fn active_groups(&self) -> &[PlaylistFolderGroup] {
        self.filtered_groups.as_ref().unwrap_or(&self.groups)
    }

    /// Obtener la canción en un índice lineal
    pub fn get_song_at_linear_index(&self, linear_idx: usize) -> Option<&PlaylistSongRef> {
        let groups = self.active_groups();
        let mut count = 0;
        for group in groups {
            if linear_idx == count {
                return None; // Separador
            }
            let song_local_idx = linear_idx - count - 1;
            if song_local_idx < group.songs.len() {
                return Some(&group.songs[song_local_idx]);
            }
            count += 1 + group.songs.len();
        }
        None
    }

    /// Obtener los ítems que son actualmente visibles (no están dentro de grupos plegados)
    /// Retorna (Tipo, GlobalLinearIndex)
    pub fn get_visible_items(&self) -> Vec<(PlaylistItemType, usize)> {
        let groups = self.active_groups();
        let mut visible = Vec::new();
        let mut global_idx = 0;

        for group in groups {
            // El separador siempre es visible
            visible.push((PlaylistItemType::Separator(group.folder_path.clone(), group.first_item_id), global_idx));
            
            let is_collapsed = self.collapsed_groups.contains(&group.first_item_id);
            let songs_len = group.songs.len();
            
            if !is_collapsed {
                for song_i in 0..songs_len {
                    visible.push((PlaylistItemType::Song(group.folder_path.clone(), song_i, group.first_item_id), global_idx + 1 + song_i));
                }
            }
            
            global_idx += 1 + songs_len;
        }
        visible
    }

    /// Alternar expansion de grupo
    pub fn toggle_group_expansion(&mut self, anchor: i64) {
        if self.collapsed_groups.contains(&anchor) {
            self.collapsed_groups.remove(&anchor);
        } else {
            self.collapsed_groups.insert(anchor);
        }
    }

    /// Obtiene información de un item en base a su índice lineal (Canónico)
    pub fn get_item_info_at_linear_index(&self, linear_idx: usize) -> Option<PlaylistItemType> {
        let groups = self.active_groups();
        let mut count = 0;
        for group in groups {
            if linear_idx == count {
                return Some(PlaylistItemType::Separator(group.folder_path.clone(), group.first_item_id));
            }
            let song_local_idx = linear_idx - count - 1;
            if song_local_idx < group.songs.len() {
                return Some(PlaylistItemType::Song(group.folder_path.clone(), song_local_idx, group.first_item_id));
            }
            count += 1 + group.songs.len();
        }
        None
    }

    /// Buscar el índice lineal del separador para un anchor (first_item_id)
    pub fn find_separator_index_for_anchor(&self, anchor: i64) -> Option<usize> {
        let groups = self.active_groups();
        let mut count = 0;
        for group in groups {
            if group.first_item_id == anchor {
                return Some(count);
            }
            count += 1 + group.songs.len();
        }
        None
    }

    /// Multi-selection: handles keyboard navigation with modifiers (Shift for range)
    pub fn handle_key_nav(&mut self, dir: crate::gui::library::LibraryNavDir, modifiers: iced::keyboard::Modifiers) {
        let visible_items = self.get_visible_items();
        if visible_items.is_empty() { return; }

        let mut current_visible_idx = 0;
        let mut found = false;

        if let Some(sel) = self.focused_idx {
            for (i, (_, g_idx)) in visible_items.iter().enumerate() {
                if *g_idx == sel {
                    current_visible_idx = i;
                    found = true;
                    break;
                }
            }
        }

        match dir {
            crate::gui::library::LibraryNavDir::Up => {
                let next_v_idx = if found { current_visible_idx.saturating_sub(1) } else { 0 };
                let next_linear_idx = visible_items[next_v_idx].1;
                
                if modifiers.shift() {
                    let pivot = self.selection_pivot.unwrap_or(visible_items[current_visible_idx].1);
                    self.selection_pivot = Some(pivot);
                    self.select_range(pivot, next_linear_idx);
                } else {
                    self.selected_idxs.clear();
                    self.selected_idxs.insert(next_linear_idx);
                    self.focused_idx = Some(next_linear_idx);
                    self.selection_pivot = Some(next_linear_idx);
                }
            }
            crate::gui::library::LibraryNavDir::Down => {
                let next_v_idx = if found { (current_visible_idx + 1).min(visible_items.len() - 1) } else { 0 };
                let next_linear_idx = visible_items[next_v_idx].1;

                if modifiers.shift() {
                    let pivot = self.selection_pivot.unwrap_or(visible_items[current_visible_idx].1);
                    self.selection_pivot = Some(pivot);
                    self.select_range(pivot, next_linear_idx);
                } else {
                    self.selected_idxs.clear();
                    self.selected_idxs.insert(next_linear_idx);
                    self.focused_idx = Some(next_linear_idx);
                    self.selection_pivot = Some(next_linear_idx);
                }
            }
            crate::gui::library::LibraryNavDir::Left => {
                if let Some(sel) = self.focused_idx {
                    if let Some(info) = self.get_item_info_at_linear_index(sel) {
                        match info {
                            PlaylistItemType::Separator(_, anchor) => {
                                self.collapsed_groups.insert(anchor);
                            }
                            PlaylistItemType::Song(_, _, anchor) => {
                                self.collapsed_groups.insert(anchor);
                                self.focused_idx = self.find_separator_index_for_anchor(anchor);
                                if let Some(new_sel) = self.focused_idx {
                                    self.selected_idxs.clear();
                                    self.selected_idxs.insert(new_sel);
                                    self.selection_pivot = Some(new_sel);
                                }
                            }
                        }
                    }
                }
            }
            crate::gui::library::LibraryNavDir::Right => {
                if let Some(sel) = self.focused_idx {
                    if let Some(info) = self.get_item_info_at_linear_index(sel) {
                        match info {
                            PlaylistItemType::Separator(_, anchor) => {
                                self.collapsed_groups.remove(&anchor);
                            }
                            PlaylistItemType::Song(_, _, anchor) => {
                                self.collapsed_groups.remove(&anchor);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub fn select_all(&mut self) {
        let visible_items = self.get_visible_items();
        self.selected_idxs.clear();
        for (_, g_idx) in visible_items {
            self.selected_idxs.insert(g_idx);
        }
        self.selection_pivot = None;
    }

    pub fn handle_click(&mut self, linear_idx: usize, modifiers: iced::keyboard::Modifiers) {
        if modifiers.command() {
            // Ctrl + Click: toggle individual item
            if self.selected_idxs.contains(&linear_idx) {
                self.selected_idxs.remove(&linear_idx);
            } else {
                self.selected_idxs.insert(linear_idx);
            }
            self.focused_idx = Some(linear_idx);
            self.selection_pivot = Some(linear_idx);
        } else if modifiers.shift() {
            // Shift + Click: range selection
            if let Some(pivot) = self.selection_pivot {
                self.select_range(pivot, linear_idx);
            } else {
                self.selected_idxs.clear();
                self.selected_idxs.insert(linear_idx);
                self.focused_idx = Some(linear_idx);
                self.selection_pivot = Some(linear_idx);
            }
        } else {
            // Normal Click: reset selection
            self.selected_idxs.clear();
            self.selected_idxs.insert(linear_idx);
            self.focused_idx = Some(linear_idx);
            self.selection_pivot = Some(linear_idx);
        }
    }

    fn select_range(&mut self, start_idx: usize, end_idx: usize) {
        let visible_items = self.get_visible_items();
        let mut start_v = None;
        let mut end_v = None;

        for (i, (_, g_idx)) in visible_items.iter().enumerate() {
            if *g_idx == start_idx { start_v = Some(i); }
            if *g_idx == end_idx { end_v = Some(i); }
        }

        if let (Some(s), Some(e)) = (start_v, end_v) {
            let (min, max) = if s < e { (s, e) } else { (e, s) };
            self.selected_idxs.clear();
            for i in min..=max {
                self.selected_idxs.insert(visible_items[i].1);
            }
            self.focused_idx = Some(end_idx);
        }
    }

    /// Obtener canción en un índice lineal (mutable)
    pub fn get_song_at_linear_index_mut(&mut self, linear_idx: usize) -> Option<&mut PlaylistSongRef> {
        let groups = if self.filtered_groups.is_some() {
            self.filtered_groups.as_mut().unwrap()
        } else {
            &mut self.groups
        };

        let mut count = 0;
        for group in groups {
            if linear_idx == count {
                return None; // Separador
            }
            let song_local_idx = linear_idx - count - 1;
            if song_local_idx < group.songs.len() {
                return Some(&mut group.songs[song_local_idx]);
            }
            count += 1 + group.songs.len();
        }
        None
    }

    /// Obtener información del grupo en un índice lineal
    pub fn get_group_info_at_linear_index(&self, linear_idx: usize) -> Option<(usize, bool)> {
        let groups = self.active_groups();
        let mut count = 0;
        for (gi, group) in groups.iter().enumerate() {
            if linear_idx == count {
                return Some((gi, true)); // Es el divisor
            }
            let song_local_idx = linear_idx - count - 1;
            if song_local_idx < group.songs.len() {
                return Some((gi, false)); // Es una canción
            }
            count += 1 + group.songs.len();
        }
        None
    }

    /// Toggle enabled de una canción por índice lineal
    pub fn toggle_song_enabled_at_linear_index(&mut self, linear_idx: usize) -> Option<bool> {
        if let Some(song) = self.get_song_at_linear_index_mut(linear_idx) {
            song.enabled = !song.enabled;
            Some(song.enabled)
        } else {
            None
        }
    }

    pub fn set_song_enabled_at_linear_index(&mut self, linear_idx: usize, enabled: bool) {
        if let Some(song) = self.get_song_at_linear_index_mut(linear_idx) {
            song.enabled = enabled;
        }
    }

    /// Toggle habilitado de todo un grupo
    pub fn toggle_group_enabled_at_linear_index(&mut self, linear_idx: usize) {
        if let Some((gi, is_folder)) = self.get_group_info_at_linear_index(linear_idx) {
            if !is_folder { return; }

            let groups = if self.filtered_groups.is_some() {
                self.filtered_groups.as_mut().unwrap()
            } else {
                &mut self.groups
            };

            if let Some(group) = groups.get_mut(gi) {
                // Si alguna está habilitada, deshabilitamos todas. Si no, habilitamos todas.
                let any_enabled = group.songs.iter().any(|s| s.enabled);
                let new_state = !any_enabled;
                for song in &mut group.songs {
                    song.enabled = new_state;
                }
            }
        }
    }

    pub fn set_group_enabled_at_linear_index(&mut self, linear_idx: usize, enabled: bool) {
        if let Some((gi, is_folder)) = self.get_group_info_at_linear_index(linear_idx) {
            if !is_folder { return; }

            let groups = if self.filtered_groups.is_some() {
                self.filtered_groups.as_mut().unwrap()
            } else {
                &mut self.groups
            };

            if let Some(group) = groups.get_mut(gi) {
                for song in &mut group.songs {
                    song.enabled = enabled;
                }
            }
        }
    }

    pub fn get_first_enabled_song_idx(&self) -> Option<usize> {
        self.find_next_enabled_song_internal(usize::MAX)
    }

    /// Returns the list of song IDs currently selected (including songs within selected folders)
    /// Preserves the order in which they appear in the playlist.
    pub fn get_selected_song_ids(&self) -> Vec<i64> {
        let mut song_linear_idxs = std::collections::BTreeSet::new();
        let groups = self.active_groups();
        
        for &idx in &self.selected_idxs {
            if let Some(info) = self.get_item_info_at_linear_index(idx) {
                match info {
                    PlaylistItemType::Separator(_, anchor) => {
                        // Find the group to add ALL its songs
                        if let Some(pos) = self.find_separator_index_for_anchor(anchor) {
                            if let Some(si) = self.get_group_info_at_linear_index(pos) {
                                if let Some(group) = groups.get(si.0) {
                                    for song_offset in 0..group.songs.len() {
                                        song_linear_idxs.insert(pos + 1 + song_offset);
                                    }
                                }
                            }
                        }
                    }
                    PlaylistItemType::Song(..) => {
                        song_linear_idxs.insert(idx);
                    }
                }
            }
        }
        
        let mut result = Vec::new();
        for idx in song_linear_idxs {
            if let Some(song) = self.get_song_at_linear_index(idx) {
                result.push(song.song_id);
            }
        }
        result
    }

    /// Obtiene el índice lineal de una canción por su song_id de base de datos.
    pub fn get_linear_index_by_song_id(&self, song_id: i64) -> Option<usize> {
        let groups = self.active_groups();
        let mut count = 0;
        for group in groups {
            count += 1; // Separador
            for (idx, song) in group.songs.iter().enumerate() {
                if song.song_id == song_id {
                    return Some(count + idx);
                }
            }
            count += group.songs.len();
        }
        None
    }

    /// Activa el modo shuffle generando una nueva sesión aleatoria.
    pub fn activate_shuffle(&mut self) {
        let groups = self.active_groups();
        let mut songs: Vec<i64> = groups.iter()
            .flat_map(|g| g.songs.iter())
            .filter(|s| s.enabled)
            .map(|s| s.song_id)
            .collect();
        
        if songs.is_empty() { 
            self.shuffle_active = false;
            return; 
        }

        // Fisher-Yates Shuffle
        for i in (1..songs.len()).rev() {
            let j = fastrand::usize(0..=i);
            songs.swap(i, j);
        }

        // Si hay una canción sonando, ponerla al principio del historial para no interrumpir el flujo
        let current_song_id = self.playing_song_idx
            .and_then(|idx| self.get_song_at_linear_index(idx))
            .map(|s| s.song_id);

        if let Some(sid) = current_song_id {
            // Eliminar de la lista aleatoria si ya está ahí (para que no se repita pronto)
            songs.retain(|&id| id != sid);
        }

        self.shuffle_session = Some(ShuffleSession {
            session_id: format!("sess-{}", fastrand::u64(..)),
            shuffle_order: songs,
            current_position: 0,
            history: current_song_id.into_iter().collect(),
        });
        self.shuffle_active = true;
    }

    /// Desactiva el modo shuffle.
    pub fn deactivate_shuffle(&mut self) {
        self.shuffle_active = false;
        self.shuffle_session = None;
    }

    /// Calcula el rango [top, bottom] en píxeles de un ítem basado en su índice lineal.
    /// Si una canción está en una carpeta plegada, devuelve la posición del separador.
    pub fn get_item_y_bounds(&self, target_idx: usize) -> (f32, f32) {
        let groups = self.active_groups();
        let mut current_y = 0.0;
        let mut count = 0;
        
        for group in groups {
            // Posición del separador de carpeta
            let sep_top = current_y;
            let sep_bottom = current_y + FOLDER_SEPARATOR_HEIGHT;
            
            if count == target_idx {
                return (sep_top, sep_bottom);
            }
            
            let is_collapsed = self.collapsed_groups.contains(&group.first_item_id);
            current_y += FOLDER_SEPARATOR_HEIGHT;
            count += 1;
            
            if !is_collapsed {
                if target_idx >= count && target_idx < count + group.songs.len() {
                    let song_inner_idx = target_idx - count;
                    let song_top = current_y + (song_inner_idx as f32 * SONG_ROW_HEIGHT);
                    return (song_top, song_top + SONG_ROW_HEIGHT);
                }
                current_y += group.songs.len() as f32 * SONG_ROW_HEIGHT;
                count += group.songs.len();
            } else {
                // Si el ítem buscado está dentro de este grupo plegado, devolvemos el separador.
                if target_idx >= count && target_idx < count + group.songs.len() {
                    return (sep_top, sep_bottom);
                }
                count += group.songs.len();
            }
        }
        (0.0, 0.0)
    }
}

// ============================================================
// Vista principal del módulo de Playlist
// ============================================================

const TABS_BAR_HEIGHT: f32 = 40.0;
const BOTTOM_BAR_HEIGHT: f32 = 40.0;
const TABS_PADDING_H: u16 = 7;

pub fn view<'a>(manager: &'a PlaylistManager, _audio_manager: &AudioManager) -> Element<'a, Message> {
    let tabs_container = build_tabs_bar(manager);
    let playlist_scroll = build_song_list(manager);
    let bottom_container = build_bottom_bar(manager);

    mouse_area(
        container(
            column![
                tabs_container,
                playlist_scroll,
                bottom_container
            ]
        )
        .width(Length::Fixed(400.0))
        .height(Length::Fill)
        .style(|_t: &Theme| container::Style::default().background(COLOR_BG))
    )
    .on_press(Message::PlaylistDeselect)
    .into()
}

// ============================================================
// Barra superior de pestañas
// ============================================================

fn build_tabs_bar<'a>(manager: &'a PlaylistManager) -> Element<'a, Message> {
    let playlists = &manager.playlists;

    // Use zero padding internally so the scrollable bounds determine visibility
    let mut tabs_row = row![]
        .spacing(5)
        .padding([0, 0])
        .align_y(Alignment::Center);

    let mut total_tabs_width: f32 = 0.0;

    for p_data in playlists.iter() {
        let name = &p_data.name;
        let id = p_data.id;
        
        let is_active = id == manager.active_playlist_id;
        let tab_color = if is_active { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };
        let tab = text(name.clone())
            .size(14)
            .color(tab_color)
            .font(FONT_INTER_SANS_MEDIUM);

        let tab_btn = button(tab)
            .padding([5, 5])
            .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
            .on_press(Message::SwitchPlaylist(id, false));

        let tab_entries = vec![
            crate::gui::widgets::ContextMenuEntry {
                label: "Reproducir".to_string(),
                icon: Some("playlist-play-straight.svg".to_string()),
                action: Some(Message::SwitchPlaylist(id, true)),
            },
            crate::gui::widgets::ContextMenuEntry {
                label: "Nueva lista".to_string(),
                icon: Some("playlist-add-straight.svg".to_string()),
                action: Some(Message::OpenDialog(crate::gui::app::ActiveDialog::CreatePlaylist { name: "".into(), pending_add_songs: Vec::new() })),
            },
            crate::gui::widgets::ContextMenuEntry {
                label: "Renombrar lista".to_string(),
                icon: Some("playlist-edit-straight.svg".to_string()),
                action: Some(Message::OpenDialog(crate::gui::app::ActiveDialog::RenamePlaylist { id, current_name: name.clone(), new_name: name.clone() })),
            },
            crate::gui::widgets::ContextMenuEntry {
                label: "Eliminar lista".to_string(),
                icon: Some("playlist-remove-straight.svg".to_string()),
                action: Some(Message::OpenDialog(crate::gui::app::ActiveDialog::DeleteConfirm { id, name: name.clone() })),
            },
            crate::gui::widgets::ContextMenuEntry {
                label: "".to_string(), // Divisor
                icon: None,
                action: None,
            },
            crate::gui::widgets::ContextMenuEntry {
                label: "Guardar lista".to_string(),
                icon: Some("playlist-add-check-straight.svg".to_string()),
                action: Some(Message::OpenDialog(crate::gui::app::ActiveDialog::ExportConfirm { 
                    id, 
                    name: name.clone(),
                    format: crate::gui::app::ExportFormat::M3U8,
                    mode: crate::gui::app::ExportMode::SingleFile
                })),
            },
            crate::gui::widgets::ContextMenuEntry {
                label: "Importar lista".to_string(),
                icon: Some("import-straight.svg".to_string()),
                action: Some(Message::OpenPlaylistFilePicker),
            },
            crate::gui::widgets::ContextMenuEntry {
                label: "Exportar lista".to_string(),
                icon: Some("export-straight.svg".to_string()),
                action: Some(Message::OpenDialog(crate::gui::app::ActiveDialog::ExportConfirm { 
                    id, 
                    name: name.clone(),
                    format: crate::gui::app::ExportFormat::M3U8,
                    mode: crate::gui::app::ExportMode::PortableFolder
                })),
            },
        ];

        tabs_row = tabs_row.push(
            mouse_area(tab_btn)
                .on_right_press(Message::RequestContextMenu(tab_entries.clone()))
        );
        total_tabs_width += estimate_tab_width(name) + 5.0;
    }

    // Usamos padding 12 a la izquierda para el corte visual, y el limite real a la derecha
    let bar_width = 380.0;
    let has_overflow = total_tabs_width > bar_width;

    let scrollable_tabs = iced::widget::scrollable(tabs_row)
        .id(PLAYLIST_TABS_SCROLL_ID.clone())
        .direction(iced::widget::scrollable::Direction::Horizontal(
            iced::widget::scrollable::Scrollbar::new()
                .width(0)
                .scroller_width(0)
                .margin(0)
        ))
        .on_scroll(Message::PlaylistTabsScrolled);

    let mut bar_content = row![
        container(scrollable_tabs)
            .width(Length::Fill)
            .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 12.0 }),
    ]
    .align_y(Alignment::Center);

    if has_overflow {
        bar_content = bar_content.push(build_tab_dropdown_button(manager));
    }

    let content = column![bar_content];

    let bar_entries = vec![
        crate::gui::widgets::ContextMenuEntry {
            label: "Nueva lista".to_string(),
            icon: Some("playlist-add-straight.svg".to_string()),
            action: Some(Message::OpenDialog(crate::gui::app::ActiveDialog::CreatePlaylist { name: "".into(), pending_add_songs: Vec::new() })),
        },
        crate::gui::widgets::ContextMenuEntry {
            label: "Importar lista".to_string(),
            icon: Some("import-straight.svg".to_string()),
            action: Some(Message::OpenPlaylistFilePicker),
        },
    ];

    mouse_area(
        container(content)
            .width(Length::Fill)
            .height(Length::Fixed(TABS_BAR_HEIGHT))
            .align_y(iced::alignment::Vertical::Center)
            .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST))
    )
    .on_right_press(Message::RequestContextMenu(bar_entries))
    .into()
}

pub fn estimate_tab_width(name: &str) -> f32 {
    name.chars().count() as f32 * 7.5 + 10.0
}

fn build_tab_dropdown_button<'a>(manager: &'a PlaylistManager) -> Element<'a, Message> {
    let icon_path = if manager.show_tab_dropdown {
        "assets/icons/arrow-up-chevron.svg"
    } else {
        "assets/icons/arrow-down-chevron.svg"
    };

    let chevron = svg(svg::Handle::from_path(icon_path))
        .width(28)
        .height(28)
        .style(|_t: &Theme, _s| svg::Style { color: Some(COLOR_TEXT_SECONDARY) });

    button(chevron)
        .padding(4)
        .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
        .on_press(Message::ToggleTabDropdown)
        .into()
}

fn build_dropdown_menu<'a>(
    playlists: &[crate::db::database::PlaylistData],
    visible_count: usize,
) -> Element<'a, Message> {
    let mut items = column![]
        .padding([5, TABS_PADDING_H as u16])
        .spacing(4);

    for p_data in playlists.iter().skip(visible_count) {
        let name = &p_data.name;
        let id = p_data.id;
        
        let item = text(name.clone())
            .size(14)
            .color(COLOR_TEXT_SECONDARY)
            .font(FONT_INTER_SANS_MEDIUM);

        let item_btn = button(item)
            .width(Length::Fill)
            .padding([5, 5])
            .style(|_t: &Theme, _s| button::Style::default().with_background(COLOR_BG))
            .on_press(Message::SwitchPlaylist(id, false));

        items = items.push(item_btn);
    }

    container(items)
        .width(Length::Fill)
        .style(|_t: &Theme| container::Style {
            background: Some(Background::Color(COLOR_CONTRAST)),
            ..container::Style::default()
        })
        .into()
}

// ============================================================
// Lista de canciones con separadores de carpeta
// ============================================================

const SONG_ROW_HEIGHT: f32 = 46.0;
const FOLDER_SEPARATOR_HEIGHT: f32 = 46.0;

fn build_song_list<'a>(manager: &'a PlaylistManager) -> Element<'a, Message> {
    let groups = manager.active_groups();

    let mut songs_col = column![].spacing(0).padding(Padding { top: 0.0, right: 9.0, bottom: 0.0, left: 0.0 });

    let mut linear_idx = 0;

    for (_gi, group) in groups.iter().enumerate() {
        // Separador de carpeta
        let is_folder_selected = manager.selected_idxs.contains(&linear_idx);
        let is_group_selected = is_folder_selected;
        let any_song_enabled = group.songs.iter().any(|s| s.enabled);
        
        let song_count = group.songs.len();
        let total_dur = format_duration(group.total_duration);
        let folder_sep = build_folder_separator(&group.folder_name, song_count, total_dur, linear_idx, is_folder_selected, any_song_enabled);
        songs_col = songs_col.push(folder_sep);
        
        let is_collapsed = manager.collapsed_groups.contains(&group.first_item_id);
        linear_idx += 1;

        if !is_collapsed {
            // Canciones del grupo
            for song in group.songs.iter() {
                let is_playing = manager.playing_song_idx == Some(linear_idx);
                let is_selected = manager.selected_idxs.contains(&linear_idx);
                let song_row = build_song_row(song, linear_idx, is_playing, is_selected, is_group_selected);
                songs_col = songs_col.push(song_row);
                linear_idx += 1;
            }
        } else {
            // Aún si está colapsado, debemos avanzar el linear_idx para que los siguientes 
            // folder separators mantengan su índice global correcto si es que se usan como IDs.
            // Pero si usamos get_visible_items para navegación, la consistencia de linear_idx global 
            // sigue siendo útil para ToggleGroupEnabled.
            linear_idx += group.songs.len();
        }
    }

    if groups.is_empty() {
        songs_col = songs_col.push(
            container(
                text("No hay canciones en esta lista")
                    .size(16)
                    .color(COLOR_TEXT_SECONDARY)
                    .font(FONT_INTER_SANS_MEDIUM)
            )
            .width(Length::Fill)
            .padding(15)
        );
    }

    let scroll = standard_scrollable(
        PLAYLIST_SCROLL_ID.clone(),
        songs_col,
        iced::widget::scrollable::Direction::Vertical(standard_scrollbar())
    )
    .height(Length::Fill)
    .on_scroll(Message::PlaylistScrolled);

    mouse_area(scroll)
        .on_enter(Message::PlaylistMouseOver(true))
        .on_exit(Message::PlaylistMouseOver(false))
        .on_press(Message::PlaylistDeselect)
        .on_right_press(Message::RequestContextMenu(get_empty_playlist_context_menu_entries()))
        .into()
}

fn build_folder_separator<'a>(
    folder_name: &'a str,
    song_count: usize,
    total_duration: String,
    linear_idx: usize,
    is_selected: bool,
    is_enabled: bool,
) -> Element<'a, Message> {
    let indicator_color = if is_enabled { COLOR_TEXT_SECONDARY } else { Color::TRANSPARENT };
    
    let indicator = text("•")
        .size(19)
        .color(indicator_color);

    let name_text = crate::gui::widgets::smart_truncate_text(folder_name.to_string(), 14.0, FONT_INTER_SANS_MEDIUM, COLOR_TEXT_SECONDARY);

    let stats_text = text(format!("{} | {}", song_count, total_duration))
        .size(13)
        .color(COLOR_TEXT_SECONDARY)
        .font(FONT_INTER_SANS_MEDIUM)
        .wrapping(iced::widget::text::Wrapping::None);

    let content = row![
        mouse_area(
            container(indicator)
                .width(Length::Fixed(12.0))
                .height(Length::Fixed(FOLDER_SEPARATOR_HEIGHT))
                .align_y(Alignment::Center)
        )
        .on_press(Message::ToggleGroupEnabled(linear_idx, None)),
        Space::new().width(Length::Fixed(5.0)),
        name_text,
        Space::new().width(Length::Shrink),
        stats_text
    ]
    .spacing(0)
    .align_y(Alignment::Center);

    let bg_color = if is_selected { COLOR_CONTRAST } else { Color::TRANSPARENT };

    mouse_area(
        container(
            column![
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fixed(FOLDER_SEPARATOR_HEIGHT - 11.0))
                    .align_y(iced::alignment::Vertical::Center),
                container(Space::new().width(Length::Fill).height(1.0))
                    .style(move |_t: &Theme| {
                        let col = if is_selected { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };
                        container::Style::default().background(col)
                    }),
                Space::new().height(10.0)
            ]
        )
        .width(Length::Fill)
        .padding(Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 10.0 })
        .style(move |_t: &Theme| container::Style {
            background: Some(Background::Color(bg_color)),
            ..container::Style::default()
        })
    )
    .on_press(Message::TogglePlaylistFolder(linear_idx))
    .on_right_press(Message::RequestContextMenu(get_item_context_menu_entries(linear_idx)))
    .interaction(iced::mouse::Interaction::Pointer)
    .into()
}

fn build_song_row<'a>(
    song: &'a PlaylistSongRef,
    linear_idx: usize,
    is_playing: bool,
    is_selected: bool,
    is_group_selected: bool,
) -> Element<'a, Message> {
    let text_color = if is_playing { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };
    let duration_color = if is_playing { COLOR_TEXT_PRIMARY } else { COLOR_TEXT_SECONDARY };

    // Indicador •
    let indicator_color = if !song.enabled {
        Color::TRANSPARENT
    } else if is_playing {
        COLOR_TEXT_PRIMARY
    } else {
        COLOR_TEXT_SECONDARY
    };
    let indicator = text("•")
        .size(17)
        .color(indicator_color);

    // Fila 1: "Nº. Nombre de canción .... Duración"
    let track_prefix = if let Some(ref tn) = song.track_number {
        format!("{}.", tn)
    } else {
        format!("{:02}.", linear_idx)
    };

    let prefix_width = 20.0;
    let prefix_spacing = 5.0;

    let prefix_container = container(
        text(track_prefix)
            .size(13)
            .color(text_color)
            .font(FONT_INTER_SANS_MEDIUM)
    )
    .width(Length::Fixed(prefix_width))
    .align_x(Alignment::End);

    let title_text = crate::gui::widgets::smart_truncate_text(song.title.clone(), 13.0, FONT_INTER_SANS_MEDIUM, text_color);

    let duration_text = text(format_duration(song.duration))
        .size(13)
        .color(duration_color)
        .font(FONT_INTER_SANS_MEDIUM)
        .wrapping(iced::widget::text::Wrapping::None);

    let row1 = row![
        prefix_container,
        title_text,
        duration_text
    ]
    .spacing(prefix_spacing)
    .align_y(Alignment::Center);

    // Fila 2: "Artista - Álbum :: Año" (Alineada con el Título de la Fila 1)
    let year_str = song.year.as_deref().unwrap_or("-");
    let row2_text = format!("{} - {} :: {}", song.artist_name, song.album_title, year_str);
    
    let row2 = row![
        Space::new().width(Length::Fixed(prefix_width + prefix_spacing)),
        crate::gui::widgets::smart_truncate_text(row2_text, 13.0, FONT_INTER_SANS_MEDIUM, text_color)
    ]
    .spacing(0)
    .align_y(Alignment::Center);

    let content = column![row1, row2]
        .spacing(2)
        .padding([0, 0]);

    let bg_color = if is_selected || is_playing {
        COLOR_CONTRAST
    } else if is_group_selected {
        COLOR_CONTRAST
    } else {
        Color::TRANSPARENT
    };

    let song_row_inner = row![
        mouse_area(
            container(indicator)
                .width(Length::Fixed(12.0))
                .height(Length::Fixed(SONG_ROW_HEIGHT))
                .align_y(Alignment::Center)
        )
        .on_press(Message::ToggleSongEnabled(linear_idx, None)),
        Space::new().width(Length::Fixed(0.0)),
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Center)
    ]
    .spacing(0)
    .align_y(Alignment::Center);

    mouse_area(
        container(song_row_inner)
            .width(Length::Fill)
            .height(Length::Fixed(SONG_ROW_HEIGHT))
            .padding(Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 15.0 })
            .style(move |_t: &Theme| container::Style {
                background: Some(Background::Color(bg_color)),
                ..container::Style::default()
            })
    )
    .on_press(Message::PlaySongIndex(linear_idx))
    .on_right_press(Message::RequestContextMenu(get_item_context_menu_entries(linear_idx)))
    .interaction(iced::mouse::Interaction::Pointer)
    .into()
}


/// Genera las opciones del menú contextual para canciones y carpetas en la lista de reproducción.
pub fn get_item_context_menu_entries(linear_idx: usize) -> Vec<crate::gui::widgets::ContextMenuEntry<Message>> {
    use crate::gui::widgets::ContextMenuEntry;
    vec![
        ContextMenuEntry {
            label: "Reproducir".to_string(),
            icon: Some("play-circle-straight-outlined.svg".to_string()),
            action: Some(Message::PlaylistContextMenuPlay(linear_idx)),
        },
        ContextMenuEntry {
            label: "".to_string(),
            icon: None,
            action: None,
        },
        ContextMenuEntry {
            label: "Agregar archivo".to_string(),
            icon: Some("audio-file-outlined-straight.svg".to_string()),
            action: Some(Message::PlaylistAddFiles),
        },
        ContextMenuEntry {
            label: "Agregar carpeta".to_string(),
            icon: Some("folder-add-outlined.svg".to_string()),
            action: Some(Message::PlaylistAddFolder),
        },
        ContextMenuEntry {
            label: "".to_string(),
            icon: None,
            action: None,
        },
        ContextMenuEntry {
            label: "Información".to_string(),
            icon: Some("info-outlined-straight.svg".to_string()),
            action: Some(Message::NoOp),
        },
        ContextMenuEntry {
            label: "Ubicación del archivo".to_string(),
            icon: Some("folder-open-outlined.svg".to_string()),
            action: Some(Message::PlaylistShowFileLocation(linear_idx)),
        },
        ContextMenuEntry {
            label: "Mostrar en la biblioteca".to_string(),
            icon: Some("show-library-outlined-straight.svg".to_string()),
            action: Some(Message::PlaylistShowInLibrary(linear_idx)),
        },
        ContextMenuEntry {
            label: "".to_string(),
            icon: None,
            action: None,
        },
        ContextMenuEntry {
            label: "Enviar a otra lista".to_string(),
            icon: Some("send-straight.svg".to_string()),
            action: Some(Message::PlaylistRequestSubMenu(linear_idx)),
        },
        ContextMenuEntry {
            label: "Editor de Etiquetas".to_string(),
            icon: Some("edit-tag.svg".to_string()),
            action: Some(Message::NoOp),
        },
        ContextMenuEntry {
            label: "Convertidor de audio".to_string(),
            icon: Some("convert.svg".to_string()),
            action: Some(Message::NoOp),
        },
        ContextMenuEntry {
            label: "".to_string(),
            icon: None,
            action: None,
        },
        ContextMenuEntry {
            label: "Activar | Desactivar".to_string(),
            icon: Some("indicator-outlined.svg".to_string()),
            action: Some(Message::PlaylistToggleEnabled(linear_idx)),
        },
        ContextMenuEntry {
            label: "Plegar | Desplegar".to_string(),
            icon: Some("expand-collapse-straight.svg".to_string()),
            action: Some(Message::PlaylistToggleAllFolders),
        },
        ContextMenuEntry {
            label: "Eliminar de la lista".to_string(),
            icon: Some("delete-oulined.svg".to_string()),
            action: Some(Message::PlaylistDeleteSelection(linear_idx)),
        },
    ]
}

/// Genera las opciones del menú contextual para el área vacía de la lista de reproducción.
fn get_empty_playlist_context_menu_entries() -> Vec<crate::gui::widgets::ContextMenuEntry<Message>> {
    use crate::gui::widgets::ContextMenuEntry;
    vec![
        ContextMenuEntry {
            label: "Agregar archivo".to_string(),
            icon: Some("audio-file-outlined-straight.svg".to_string()),
            action: Some(Message::PlaylistAddFiles),
        },
        ContextMenuEntry {
            label: "Agregar carpeta".to_string(),
            icon: Some("folder-add-outlined.svg".to_string()),
            action: Some(Message::PlaylistAddFolder),
        },
    ]
}

// ============================================================
// Barra inferior
// ============================================================

fn build_bottom_bar<'a>(manager: &'a PlaylistManager) -> Element<'a, Message> {
    let search_box = crate::gui::widgets::standard_search_input(
        Some(PLAYLIST_SEARCH_ID.clone()),
        "Buscar...",
        &manager.search_query,
        Message::PlaylistSearchChanged,
        Message::PlaylistSearchChanged(String::new()),
        Length::Fixed(180.0),
    );

    let eq_btn = icon_button("equalizer-straight.svg", false, Message::ToggleAudioCenter);
    let shuffle_btn = icon_button("shuffle-straight.svg", manager.shuffle_active, Message::ToggleShuffle);
    let (repeat_icon, repeat_active) = if manager.repeat_mode == 2 {
        ("repeat-one-straight-outlined.svg", true)
    } else {
        ("repeat-straight-outlined.svg", manager.repeat_mode > 0)
    };
    let repeat_btn = icon_button(repeat_icon, repeat_active, Message::ToggleRepeat);
    let lyrics_icon = icon_button("lyrics-straight-outlined.svg", false, Message::ToggleLyrics);

    let icons_group = row![
        container(lyrics_icon).padding(Padding { top: 3.0, right: 0.0, bottom: 0.0, left: 0.0 }),
        repeat_btn,
        shuffle_btn,
        eq_btn
    ]
        .spacing(10)
        .align_y(Alignment::Center);


    let bar_content = row![
        container(search_box).center_y(Length::Fill),
        Space::new().width(Length::Fill),
        container(icons_group).center_y(Length::Fill)
    ]
    .padding([0, 15])
    .spacing(10)
    .align_y(Alignment::Center);

    container(bar_content)
        .width(Length::Fill)
        .height(Length::Fixed(BOTTOM_BAR_HEIGHT))
        .style(|_t: &Theme| container::Style::default().background(COLOR_CONTRAST))
        .into()
}

fn icon_button<'a>(icon_name: &str, active: bool, msg: Message) -> Element<'a, Message> {
    let icon_color = if active {
        COLOR_ACCENT
    } else {
        COLOR_TEXT_SECONDARY
    };

    let icon = svg(svg::Handle::from_path(format!("assets/icons/{}", icon_name)))
        .width(24)
        .height(24)
        .style(move |_t: &Theme, _s| svg::Style { color: Some(icon_color) });

    button(icon)
        .padding(2)
        .style(|_t: &Theme, _s| button::Style::default().with_background(Color::TRANSPARENT))
        .on_press(msg)
        .into()
}

// ============================================================
// Compatibilidad temporal con app.rs existente
// ============================================================

impl PlaylistManager {
    /// Avanza a la siguiente canción (auto-advance EOF).
    pub fn play_next(&mut self, audio_manager: &AudioManager) {
        if self.repeat_mode == 2 {
            // Repeat one
            if let Some(idx) = self.playing_song_idx {
                if let Some(song) = self.get_song_at_linear_index(idx) {
                    let _ = audio_manager.load_file(&song.file_path);
                    audio_manager.play();
                }
            }
            return;
        }

        if self.shuffle_active {
            let mut found_next = None;
            
            // Primera pasada: Búsqueda inmutable para evitar conflictos de préstamo
            if let Some(session) = &self.shuffle_session {
                let mut pos = session.current_position;
                while pos < session.shuffle_order.len() {
                    let song_id = session.shuffle_order[pos];
                    pos += 1;

                    if let Some(l_idx) = self.get_linear_index_by_song_id(song_id) {
                        if let Some(song) = self.get_song_at_linear_index(l_idx) {
                            if song.enabled {
                                found_next = Some((song_id, l_idx, song.file_path.clone(), pos));
                                break;
                            }
                        }
                    }
                }
            }

            // Segunda pasada: Si encontramos una canción, actualizamos el estado mutando la sesión
            if let Some((song_id, l_idx, path, new_pos)) = found_next {
                if let Some(session) = &mut self.shuffle_session {
                    session.history.push(song_id);
                    session.current_position = new_pos;
                }
                
                self.playing_song_idx = Some(l_idx);
                let _ = audio_manager.load_file(&path);
                audio_manager.play();
                return;
            } else if self.shuffle_active && self.repeat_mode == 1 {
                // Re-barajar si Repeat All está activo en shuffle y llegamos al final
                self.activate_shuffle();
                if !self.shuffle_session.as_ref().map(|s| s.shuffle_order.is_empty()).unwrap_or(true) {
                    self.play_next(audio_manager);
                }
                return;
            }
            if self.shuffle_active { return; } 
        }

        // Sequential logic
        let current = self.playing_song_idx.unwrap_or(usize::MAX);
        if let Some(next_idx) = self.find_next_enabled_song_internal(current) {
            if let Some(song) = self.get_song_at_linear_index(next_idx) {
                let path = song.file_path.clone();
                self.playing_song_idx = Some(next_idx);
                let _ = audio_manager.load_file(&path);
                audio_manager.play();
            }
        }
    }

    /// Retrocede a la canción anterior.
    pub fn play_prev(&mut self, audio_manager: &AudioManager) {
        if self.shuffle_active {
            let prev_song_id = if let Some(session) = &mut self.shuffle_session {
                if session.history.len() > 1 {
                    session.history.pop(); // Quitar la actual
                    if session.current_position > 0 {
                        session.current_position -= 1;
                    }
                    session.history.last().cloned()
                } else { None }
            } else { None };

            if let Some(prev_id) = prev_song_id {
                if let Some(l_idx) = self.get_linear_index_by_song_id(prev_id) {
                    if let Some(song) = self.get_song_at_linear_index(l_idx) {
                        let path = song.file_path.clone();
                        self.playing_song_idx = Some(l_idx);
                        let _ = audio_manager.load_file(&path);
                        audio_manager.play();
                        return;
                    }
                }
            }
        }

        let current = self.playing_song_idx.unwrap_or(0);
        
        let prev_idx = if current == 0 {
            if self.repeat_mode == 1 { self.find_last_enabled() } else { None }
        } else {
            // Buscar hacia atrás el anterior habilitado
            let mut idx = current - 1;
            let mut found = None;
            loop {
                if let Some(song) = self.get_song_at_linear_index(idx) {
                    if song.enabled {
                        found = Some(idx);
                        break;
                    }
                }
                if idx == 0 { break; }
                idx -= 1;
            }
            found
        };

        if let Some(idx) = prev_idx {
            if let Some(song) = self.get_song_at_linear_index(idx) {
                let path = song.file_path.clone();
                self.playing_song_idx = Some(idx);
                let _ = audio_manager.load_file(&path);
                audio_manager.play();
            }
        }
    }

    fn find_next_enabled_song_internal(&self, current: usize) -> Option<usize> {
        let groups = self.active_groups();
        let total = count_linear_items(groups);
        if total == 0 { return None; }

        let mut idx = if current == usize::MAX { 0 } else { current + 1 };
        let mut wrapped = false;

        loop {
            if idx >= total {
                if wrapped || self.repeat_mode != 1 { return None; }
                idx = 0;
                wrapped = true;
            }

            if let Some(song) = self.get_song_at_linear_index(idx) {
                if song.enabled {
                    return Some(idx);
                }
            }
            idx += 1;
        }
    }

    fn find_last_enabled(&self) -> Option<usize> {
        let groups = self.active_groups();
        let total = count_linear_items(groups);
        let mut idx = total;
        loop {
            if idx == 0 { return None; }
            idx -= 1;
            if let Some(song) = self.get_song_at_linear_index(idx) {
                if song.enabled {
                    return Some(idx);
                }
            }
        }
    }

    /// Notifica que una canción fue reproducida manualmente (por doble clic o enter).
    /// Esto es vital para el modo Shuffle para evitar repeticiones y mantener el historial.
    pub fn notify_manual_play(&mut self, song_id: i64) {
        if self.shuffle_active {
            if let Some(session) = &mut self.shuffle_session {
                // 1. Añadir al historial
                session.history.push(song_id);
                
                // 2. Eliminar de la cola futura si estaba presente para evitar que se repita
                // Solo buscamos desde la posición actual en adelante
                if session.current_position < session.shuffle_order.len() {
                    if let Some(idx) = session.shuffle_order[session.current_position..].iter().position(|&x| x == song_id) {
                        session.shuffle_order.remove(session.current_position + idx);
                    }
                }
            }
        }
    }
}
