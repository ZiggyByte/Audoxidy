use crate::audio::AudioManager;
use crate::db::database::{PlaylistFolderGroup, PlaylistSongRef, ShuffleSession};

use super::state::{PlaylistItemType, PlaylistManager, count_linear_items};
use super::view::{FOLDER_SEPARATOR_HEIGHT, SONG_ROW_HEIGHT};

use std::sync::Arc;

impl PlaylistManager {
    pub fn apply_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_groups = None;
            self.invalidate_cache();
            return;
        }

        use rayon::prelude::*;

        let query = self.search_query.to_lowercase();

        let filtered: Vec<PlaylistFolderGroup> = self
            .groups
            .par_iter()
            .filter_map(|group| {
                let matching_songs: Vec<PlaylistSongRef> = group
                    .songs
                    .iter()
                    .filter(|s| {
                        // Búsqueda insensible a mayúsculas
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
                    Some(g)
                } else {
                    None
                }
            })
            .collect();

        self.filtered_groups = if filtered.is_empty() {
            None
        } else {
            Some(filtered)
        };
        self.invalidate_cache();
    }

    /// Obtiene la siguiente canción para precarga de carátulas y audio.
    ///
    /// Debe coincidir EXACTAMENTE con la lógica de `play_next` (incluido el salto de
    /// canciones deshabilitadas), o la pre-carga cargaría una canción distinta a la
    /// que realmente se reproducirá — reintroduciendo el corte entre canciones.
    pub fn get_next_song_ref(&self) -> Option<&PlaylistSongRef> {
        let current = self.playing_song_idx?;
        let next_idx = if self.repeat_mode == 2 {
            Some(current)
        } else if self.shuffle_active {
            // En shuffle la siguiente es la primera canción HABILITADA desde la
            // posición actual de la sesión (idéntico a play_next).
            self.shuffle_session.as_ref().and_then(|session| {
                session.shuffle_order[session.current_position..]
                    .iter()
                    .find_map(|&song_id| {
                        let l_idx = self.get_linear_index_by_song_id(song_id)?;
                        let song = self.get_song_at_linear_index(l_idx)?;
                        if song.enabled { Some(l_idx) } else { None }
                    })
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

    /// Establece los grupos de la playlist e invalida el caché
    pub fn set_groups(&mut self, groups: Vec<PlaylistFolderGroup>) {
        self.groups = groups;
        self.invalidate_cache();
    }

    /// Limpia los grupos e invalida el caché
    pub fn clear_groups(&mut self) {
        self.groups.clear();
        self.filtered_groups = None;
        self.invalidate_cache();
    }

    /// Descarga los datos pesados de la RAM si no se están usando
    pub fn unload(&mut self, is_focused: bool, is_playing: bool, is_active_tab: bool) {
        if is_focused || is_playing || is_active_tab {
            return;
        }

        if !self.groups.is_empty() {
            tracing::debug!("Audoxidy GC: Unloading inactive Playlist data.");
            self.groups.clear();
            self.groups.shrink_to_fit();
            self.filtered_groups = None;
            self.cached_visible_items.borrow_mut().take();
            self.data_unloaded = true;
        }
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

    /// Invalida el caché de ítems visibles
    pub fn invalidate_cache(&mut self) {
        self.cached_visible_items.replace(None);
    }

    /// Obtener los ítems que son actualmente visibles (no están dentro de grupos plegados)
    /// Retorna una copia compartida (Arc) de los ítems visibles (Tipo, GlobalLinearIndex)
    /// Es O(1) y evita clonar el vector completo.
    pub fn get_visible_items(&self) -> Arc<Vec<(PlaylistItemType, usize)>> {
        if self.cached_visible_items.borrow().is_none() {
            let groups = self.active_groups();
            let mut visible = Vec::new();
            let mut global_idx = 0;

            for (group_idx, group) in groups.iter().enumerate() {
                visible.push((
                    PlaylistItemType::Separator(group_idx, group.first_item_id),
                    global_idx,
                ));
                let is_collapsed = self.collapsed_groups.contains(&group.first_item_id);
                let songs_len = group.songs.len();

                if !is_collapsed {
                    for song_i in 0..songs_len {
                        visible.push((
                            PlaylistItemType::Song(group_idx, song_i, group.first_item_id),
                            global_idx + 1 + song_i,
                        ));
                    }
                }
                global_idx += 1 + songs_len;
            }
            self.cached_visible_items.replace(Some(Arc::new(visible)));
        }

        self.cached_visible_items.borrow().as_ref().unwrap().clone()
    }

    /// Alternar expansion de grupo
    pub fn toggle_group_expansion(&mut self, anchor: i64) {
        if self.collapsed_groups.contains(&anchor) {
            self.collapsed_groups.remove(&anchor);
        } else {
            self.collapsed_groups.insert(anchor);
        }
        self.invalidate_cache();
    }

    /// Obtiene información de un item en base a su índice lineal (Canónico)
    pub fn get_item_info_at_linear_index(&self, linear_idx: usize) -> Option<PlaylistItemType> {
        let groups = self.active_groups();
        let mut count = 0;
        for (group_idx, group) in groups.iter().enumerate() {
            if linear_idx == count {
                return Some(PlaylistItemType::Separator(group_idx, group.first_item_id));
            }
            let song_local_idx = linear_idx - count - 1;
            if song_local_idx < group.songs.len() {
                return Some(PlaylistItemType::Song(
                    group_idx,
                    song_local_idx,
                    group.first_item_id,
                ));
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
    pub fn handle_key_nav(
        &mut self,
        dir: crate::gui::library::LibraryNavDir,
        modifiers: iced::keyboard::Modifiers,
    ) {
        let visible_items = self.get_visible_items();
        if visible_items.is_empty() {
            return;
        }

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
                let next_v_idx = if found {
                    current_visible_idx.saturating_sub(1)
                } else {
                    0
                };
                let next_linear_idx = visible_items[next_v_idx].1;

                if modifiers.shift() {
                    let pivot = self
                        .selection_pivot
                        .unwrap_or(visible_items[current_visible_idx].1);
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
                let next_v_idx = if found {
                    (current_visible_idx + 1).min(visible_items.len() - 1)
                } else {
                    0
                };
                let next_linear_idx = visible_items[next_v_idx].1;

                if modifiers.shift() {
                    let pivot = self
                        .selection_pivot
                        .unwrap_or(visible_items[current_visible_idx].1);
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
                let mut anchors_to_collapse = std::collections::HashSet::new();

                // Recopilar todos los grupos afectados por la selección actual
                for &sel in &self.selected_idxs {
                    if let Some(info) = self.get_item_info_at_linear_index(sel) {
                        match info {
                            PlaylistItemType::Separator(_, anchor) => {
                                anchors_to_collapse.insert(anchor);
                            }
                            PlaylistItemType::Song(_, _, anchor) => {
                                anchors_to_collapse.insert(anchor);
                            }
                        }
                    }
                }

                if !anchors_to_collapse.is_empty() {
                    for anchor in anchors_to_collapse {
                        self.collapsed_groups.insert(anchor);
                    }

                    // Si el foco estaba en una canción, moverlo al separador del grupo
                    if let Some(sel) = self.focused_idx {
                        if let Some(PlaylistItemType::Song(_, _, anchor)) =
                            self.get_item_info_at_linear_index(sel)
                        {
                            if let Some(new_sel) = self.find_separator_index_for_anchor(anchor) {
                                self.focused_idx = Some(new_sel);
                                self.selected_idxs.clear();
                                self.selected_idxs.insert(new_sel);
                                self.selection_pivot = Some(new_sel);
                            }
                        }
                    }
                    self.invalidate_cache();
                }
            }
            crate::gui::library::LibraryNavDir::Right => {
                let mut anchors_to_expand = std::collections::HashSet::new();

                for &sel in &self.selected_idxs {
                    if let Some(info) = self.get_item_info_at_linear_index(sel) {
                        match info {
                            PlaylistItemType::Separator(_, anchor) => {
                                anchors_to_expand.insert(anchor);
                            }
                            PlaylistItemType::Song(_, _, anchor) => {
                                anchors_to_expand.insert(anchor);
                            }
                        }
                    }
                }

                if !anchors_to_expand.is_empty() {
                    for anchor in anchors_to_expand {
                        self.collapsed_groups.remove(&anchor);
                    }
                    self.invalidate_cache();
                }
            }
            _ => {}
        }
    }

    pub fn select_all(&mut self) {
        let visible_items = self.get_visible_items();
        self.selected_idxs.clear();
        for (_, g_idx) in visible_items.iter() {
            self.selected_idxs.insert(*g_idx);
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

        for (v_idx, (_, g_idx)) in visible_items.iter().enumerate() {
            if *g_idx == start_idx {
                start_v = Some(v_idx);
            }
            if *g_idx == end_idx {
                end_v = Some(v_idx);
            }
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
    pub fn get_song_at_linear_index_mut(
        &mut self,
        linear_idx: usize,
    ) -> Option<&mut PlaylistSongRef> {
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
        let mut song_id = None;
        let mut new_state = None;

        if let Some(song) = self.get_song_at_linear_index_mut(linear_idx) {
            song.enabled = !song.enabled;
            song_id = Some(song.song_id);
            new_state = Some(song.enabled);
        }

        if let (Some(sid), Some(enabled), true) =
            (song_id, new_state, self.filtered_groups.is_some())
        {
            for group in &mut self.groups {
                for song in &mut group.songs {
                    if song.song_id == sid {
                        song.enabled = enabled;
                        break;
                    }
                }
            }
        }
        new_state
    }

    pub fn set_song_enabled_at_linear_index(&mut self, linear_idx: usize, enabled: bool) {
        let mut song_id = None;
        if let Some(song) = self.get_song_at_linear_index_mut(linear_idx) {
            song.enabled = enabled;
            song_id = Some(song.song_id);
        }

        if let (Some(sid), true) = (song_id, self.filtered_groups.is_some()) {
            for group in &mut self.groups {
                for song in &mut group.songs {
                    if song.song_id == sid {
                        song.enabled = enabled;
                        break;
                    }
                }
            }
        }
    }

    /// Toggle habilitado de todo un grupo
    pub fn toggle_group_enabled_at_linear_index(&mut self, linear_idx: usize) {
        let mut anchor = None;
        let mut new_state = None;

        if let Some((gi, is_folder)) = self.get_group_info_at_linear_index(linear_idx) {
            if !is_folder {
                return;
            }

            let groups = if self.filtered_groups.is_some() {
                self.filtered_groups.as_mut().unwrap()
            } else {
                &mut self.groups
            };

            if let Some(group) = groups.get_mut(gi) {
                let any_enabled = group.songs.iter().any(|s| s.enabled);
                let state = !any_enabled;
                for song in &mut group.songs {
                    song.enabled = state;
                }
                anchor = Some(group.first_item_id);
                new_state = Some(state);
            }
        }

        if let (Some(a), Some(enabled), true) = (anchor, new_state, self.filtered_groups.is_some())
        {
            if let Some(group) = self.groups.iter_mut().find(|g| g.first_item_id == a) {
                for song in &mut group.songs {
                    song.enabled = enabled;
                }
            }
        }
    }

    pub fn set_group_enabled_at_linear_index(&mut self, linear_idx: usize, enabled: bool) {
        let mut anchor = None;

        if let Some((gi, is_folder)) = self.get_group_info_at_linear_index(linear_idx) {
            if !is_folder {
                return;
            }

            let groups = if self.filtered_groups.is_some() {
                self.filtered_groups.as_mut().unwrap()
            } else {
                &mut self.groups
            };

            if let Some(group) = groups.get_mut(gi) {
                for song in &mut group.songs {
                    song.enabled = enabled;
                }
                anchor = Some(group.first_item_id);
            }
        }

        if let (Some(a), true) = (anchor, self.filtered_groups.is_some()) {
            if let Some(group) = self.groups.iter_mut().find(|g| g.first_item_id == a) {
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
        let mut songs: Vec<i64> = groups
            .iter()
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
        let current_song_id = self
            .playing_song_idx
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
// Compatibilidad temporal con app.rs existente
// ============================================================

impl PlaylistManager {
    /// Avanza a la siguiente canción (auto-advance EOF).
    pub fn play_next(&mut self, audio_manager: &AudioManager) {
        if self.repeat_mode == 2 {
            // Repeat one
            if let Some(idx) = self.playing_song_idx {
                if let Some(song) = self.get_song_at_linear_index(idx) {
                    let _ = audio_manager.load_file(
                        &song.file_path,
                        song.title.to_string(),
                        song.artist_name.to_string(),
                        song.album_title.to_string(),
                        song.cover_path.as_deref().map(str::to_string),
                        None,
                        None,
                    );
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
                                found_next = Some((
                                    song_id,
                                    l_idx,
                                    song.file_path.clone(),
                                    song.title.to_string(),
                                    song.artist_name.to_string(),
                                    song.album_title.to_string(),
                                    song.cover_path.as_deref().map(str::to_string),
                                    pos,
                                ));
                                break;
                            }
                        }
                    }
                }
            }

            // Segunda pasada: Si encontramos una canción, actualizamos el estado mutando la sesión
            if let Some((song_id, l_idx, path, title, artist, album, cover, new_pos)) = found_next {
                if let Some(session) = &mut self.shuffle_session {
                    session.history.push(song_id);
                    session.current_position = new_pos;
                }

                self.playing_song_idx = Some(l_idx);
                let _ = audio_manager.load_file(&path, title, artist, album, cover, None, None);
                audio_manager.play();
                return;
            } else if self.shuffle_active && self.repeat_mode == 1 {
                // Re-barajar si Repeat All está activo en shuffle y llegamos al final
                self.activate_shuffle();
                if !self
                    .shuffle_session
                    .as_ref()
                    .map(|s| s.shuffle_order.is_empty())
                    .unwrap_or(true)
                {
                    self.play_next(audio_manager);
                }
                return;
            }
            if self.shuffle_active {
                return;
            }
        }

        // Sequential logic
        let current = self.playing_song_idx.unwrap_or(usize::MAX);
        if let Some(next_idx) = self.find_next_enabled_song_internal(current) {
            if let Some(song) = self.get_song_at_linear_index(next_idx) {
                let _ = audio_manager.load_file(
                    &song.file_path,
                    song.title.to_string(),
                    song.artist_name.to_string(),
                    song.album_title.to_string(),
                    song.cover_path.as_deref().map(str::to_string),
                    None,
                    None,
                );
                self.playing_song_idx = Some(next_idx);
                audio_manager.play();
            }
        }
    }

    /// Retrocede a la canción anterior.
    pub fn play_prev(&mut self, audio_manager: &AudioManager) {
        if self.shuffle_active {
            let prev_song_data = if let Some(session) = &mut self.shuffle_session {
                if session.history.len() > 1 {
                    session.history.pop(); // Quitar la actual
                    if session.current_position > 0 {
                        session.current_position -= 1;
                    }
                    if let Some(prev_id) = session.history.last().cloned() {
                        if let Some(l_idx) = self.get_linear_index_by_song_id(prev_id) {
                            if let Some(song) = self.get_song_at_linear_index(l_idx) {
                                Some((
                                    l_idx,
                                    song.file_path.clone(),
                                    song.title.to_string(),
                                    song.artist_name.to_string(),
                                    song.album_title.to_string(),
                                    song.cover_path.as_deref().map(str::to_string),
                                ))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            if let Some((l_idx, path, title, artist, album, cover)) = prev_song_data {
                self.playing_song_idx = Some(l_idx);
                let _ = audio_manager.load_file(&path, title, artist, album, cover, None, None);
                audio_manager.play();
                return;
            }
        }

        let current = self.playing_song_idx.unwrap_or(0);

        let prev_idx = if current == 0 {
            if self.repeat_mode == 1 {
                self.find_last_enabled()
            } else {
                None
            }
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
                if idx == 0 {
                    break;
                }
                idx -= 1;
            }
            found
        };

        if let Some(idx) = prev_idx {
            if let Some(song) = self.get_song_at_linear_index(idx) {
                let _ = audio_manager.load_file(
                    &song.file_path,
                    song.title.to_string(),
                    song.artist_name.to_string(),
                    song.album_title.to_string(),
                    song.cover_path.as_deref().map(str::to_string),
                    None,
                    None,
                );
                self.playing_song_idx = Some(idx);
                audio_manager.play();
            }
        }
    }

    fn find_next_enabled_song_internal(&self, current: usize) -> Option<usize> {
        let groups = self.active_groups();
        let total = count_linear_items(groups);
        if total == 0 {
            return None;
        }

        let mut idx = if current == usize::MAX {
            0
        } else {
            current + 1
        };
        let mut wrapped = false;

        loop {
            if idx >= total {
                if wrapped || self.repeat_mode != 1 {
                    return None;
                }
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
            if idx == 0 {
                return None;
            }
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
                    if let Some(idx) = session.shuffle_order[session.current_position..]
                        .iter()
                        .position(|&x| x == song_id)
                    {
                        session.shuffle_order.remove(session.current_position + idx);
                    }
                }
            }
        }
    }
}
