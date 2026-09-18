//! Library selection: focus, keyboard navigation, marquee, and range selection.

use std::sync::Arc;

use super::state::LibraryManager;
use super::types::{LibraryListItem, LibraryNavDir, LibraryViewMode};

impl LibraryManager {
    /// Busca el ID compuesto de un álbum (Artist|AlbumID) para una canción dada,
    /// garantizando consistencia absoluta con la estructura visual actual.
    pub fn find_album_id_for_song(&self, song_id: i64) -> Option<String> {
        let (_, v_artists) = self.get_view_structure();
        for va in v_artists {
            for alb in va.albums {
                if alb.songs.iter().any(|(s, _)| s.id == song_id) {
                    return Some(format!("{}|{}", va.artist_name, alb.album_hash));
                }
            }
        }
        None
    }

    /// Cambia el estado de expansión de un álbum y actualiza el foco/selección.
    pub fn set_album_collapsed(&mut self, album_id: String, collapsed: bool) {
        if collapsed {
            self.collapsed_albums.insert(album_id.clone());
        } else {
            self.collapsed_albums.remove(&album_id);
        }

        // Sincronizar foco y selección
        let item = LibraryListItem::Album(album_id.clone());
        self.selected_items.clear();
        self.selected_items.insert(item.clone());
        self.focused_item = Some(item);

        self.selected_song_idx = None;
        self.selected_header = None;
        self.selected_album = Some(album_id);
        self.invalidate_cache();
    }

    /// Cambia el estado de expansión de un artista y actualiza el foco/selección.
    pub fn set_artist_collapsed(&mut self, artist_name: String, collapsed: bool) {
        if collapsed {
            self.collapsed_artists.insert(artist_name.clone());
        } else {
            self.collapsed_artists.remove(&artist_name);
        }

        // Sincronizar foco y selección
        let item = LibraryListItem::Artist(artist_name.clone());
        self.selected_items.clear();
        self.selected_items.insert(item.clone());
        self.focused_item = Some(item);

        self.selected_song_idx = None;
        self.selected_album = None;
        self.selected_header = Some(artist_name);
        self.invalidate_cache();
    }

    pub fn get_song_by_global_idx(
        &self,
        target_idx: usize,
    ) -> Option<std::sync::Arc<crate::db::database::SongData>> {
        let mut current_idx = 0;
        for group in &self.artist_groups {
            if current_idx + group.songs.len() > target_idx {
                return group.songs.get(target_idx - current_idx).cloned();
            }
            current_idx += group.songs.len();
        }
        None
    }

    /// Returns the new navigation item and its exact Y position from the items list.
    pub fn handle_key_nav(
        &mut self,
        dir: LibraryNavDir,
        modifiers: iced::keyboard::Modifiers,
    ) -> Option<(LibraryListItem, f32, f32)> {
        let items = self.get_visible_items();
        if items.is_empty() {
            return None;
        }

        let mut current_idx = 0;
        let mut found = false;

        // 1. Intentar encontrar por focused_item
        if let Some(focused) = &self.focused_item {
            if let Some(pos) = items.iter().position(|(it, _)| it == focused) {
                current_idx = pos;
                found = true;
            }
        }

        // 2. Fallback a campos legados si no se encontró foco explícito
        if !found {
            for (i, (item, _)) in items.iter().enumerate() {
                let match_found = match item {
                    LibraryListItem::Song(id) => {
                        // Si tenemos un list mode con filtered_songs, buscamos el índice
                        if let Some(sel_idx) = self.selected_song_idx {
                            if let Some(songs) = self.filtered_songs.as_ref() {
                                songs.get(sel_idx).map(|s| s.id == *id).unwrap_or(false)
                            } else if let Some(songs) = self.expanded_album_songs.as_ref() {
                                songs.get(sel_idx).map(|s| s.id == *id).unwrap_or(false)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    LibraryListItem::Album(id) => Some(id) == self.selected_album.as_ref(),
                    LibraryListItem::Artist(name) => Some(name) == self.selected_header.as_ref(),
                };
                if match_found {
                    current_idx = i;
                    found = true;
                    break;
                }
            }
        }

        if !found {
            current_idx = 0;
        }

        // Navegación en Grid vs Lista
        let next_idx = if self.view_mode == LibraryViewMode::Grid {
            let per_row = self.albums_per_row.get().max(1);
            let is_current_song =
                matches!(items.get(current_idx), Some((LibraryListItem::Song(_), _)));

            match dir {
                LibraryNavDir::Up => {
                    if is_current_song {
                        let prev = current_idx.saturating_sub(1);
                        if let Some((LibraryListItem::Song(_), _)) = items.get(prev) {
                            prev
                        } else {
                            // Al subir desde la primera canción, ir al álbum que está expandido
                            if let Some(exp_id) = &self.expanded_album {
                                items
                                    .iter()
                                    .position(|(it, _)| {
                                        if let LibraryListItem::Album(id) = it {
                                            id == exp_id
                                        } else {
                                            false
                                        }
                                    })
                                    .unwrap_or(prev)
                            } else {
                                prev
                            }
                        }
                    } else {
                        current_idx.saturating_sub(per_row)
                    }
                }
                LibraryNavDir::Down => {
                    if is_current_song {
                        let next = (current_idx + 1).min(items.len() - 1);
                        if let Some((LibraryListItem::Song(_), _)) = items.get(next) {
                            next
                        } else {
                            current_idx // Se DETIENE al final de la lista de canciones
                        }
                    } else {
                        let mut target = (current_idx + per_row).min(items.len() - 1);

                        // Si hay una lista de canciones expandida justo después de esta fila,
                        // entrar en ella por la primera canción.
                        for i in (current_idx + 1)..=target {
                            if let Some((LibraryListItem::Song(_), _)) = items.get(i) {
                                target = i;
                                break;
                            }
                        }
                        target
                    }
                }
                LibraryNavDir::Left => {
                    if is_current_song {
                        current_idx
                    } else {
                        current_idx.saturating_sub(1)
                    }
                }
                LibraryNavDir::Right => {
                    if is_current_song {
                        current_idx
                    } else {
                        (current_idx + 1).min(items.len() - 1)
                    }
                }
                _ => current_idx,
            }
        } else {
            match dir {
                LibraryNavDir::Up => current_idx.saturating_sub(1),
                LibraryNavDir::Down => (current_idx + 1).min(items.len() - 1),
                _ => current_idx,
            }
        };

        if items.is_empty() {
            return None;
        }
        let item_tuple = &items[next_idx.min(items.len() - 1)];
        let new_item = item_tuple.0.clone();
        let (y, h) = (item_tuple.1.y, item_tuple.1.height);

        if modifiers.shift() {
            let pivot = self
                .selection_pivot
                .clone()
                .unwrap_or_else(|| items[current_idx].0.clone());
            self.selection_pivot = Some(pivot.clone());
            self.select_range(&pivot, &new_item);
        } else {
            self.selected_items.clear();
            self.selected_items.insert(new_item.clone());
            self.selection_pivot = Some(new_item.clone());
        }

        self.focused_item = Some(new_item.clone());
        self.selected_item_hint = Some((y, h));

        // Sincronizar legacy fields para compatibilidad con el resto de la app
        self.sync_legacy_selection(&new_item, next_idx);
        self.update_selection_stats();

        Some((new_item, y, h))
    }

    pub fn select_marquee(
        &mut self,
        marquee_rect: iced::Rectangle,
        modifiers: iced::keyboard::Modifiers,
    ) {
        let items = self.get_visible_items();

        if !modifiers.shift() && !modifiers.command() && !modifiers.control() {
            self.selected_items.clear();
        }

        for (item, bounds) in items {
            if marquee_rect.intersects(&bounds) {
                if (modifiers.command() || modifiers.control())
                    && self.selected_items.contains(&item)
                {
                    self.selected_items.remove(&item);
                } else {
                    self.selected_items.insert(item);
                }
            }
        }
        self.update_selection_stats();
    }

    /// Maneja la selección por clic, soportando modificadores para selección múltiple o por rango.
    pub fn handle_click(&mut self, item: LibraryListItem, modifiers: iced::keyboard::Modifiers) {
        if modifiers.command() || modifiers.control() {
            // Ctrl + Clic: alternar selección individual
            if self.selected_items.contains(&item) {
                self.selected_items.remove(&item);
            } else {
                self.selected_items.insert(item.clone());
            }
            self.focused_item = Some(item.clone());
            self.selection_pivot = Some(item);
        } else if modifiers.shift() {
            // Shift + Clic: selección por rango
            if let Some(pivot) = self.selection_pivot.clone() {
                self.select_range(&pivot, &item);
            } else {
                self.selected_items.clear();
                self.selected_items.insert(item.clone());
                self.focused_item = Some(item.clone());
                self.selection_pivot = Some(item);
            }
        } else {
            // Clic normal: resetear selección
            self.selected_items.clear();
            self.selected_items.insert(item.clone());
            self.focused_item = Some(item.clone());
            self.selection_pivot = Some(item);
        }
        self.update_selection_stats();
    }

    /// Selecciona todos los elementos visibles (incluyendo cabeceras colapsadas) según los filtros actuales.
    pub fn select_all(&mut self) {
        let items = self.get_all_list_items();
        self.selected_items.clear();
        for item in items {
            self.selected_items.insert(item);
        }
        self.selection_pivot = None;
        self.focused_item = None;
        self.update_selection_stats();
    }

    pub fn select_range(&mut self, start_item: &LibraryListItem, end_item: &LibraryListItem) {
        let items = self.get_visible_items();
        let mut start_v = None;
        let mut end_v = None;

        for (i, (it, _)) in items.iter().enumerate() {
            if it == start_item {
                start_v = Some(i);
            }
            if it == end_item {
                end_v = Some(i);
            }
        }

        if let (Some(s), Some(e)) = (start_v, end_v) {
            let (min, max) = if s < e { (s, e) } else { (e, s) };
            self.selected_items.clear();
            for i in min..=max {
                let (item, _) = &items[i];
                self.selected_items.insert(item.clone());

                // Si seleccionamos una cabecera, expandimos la selección a sus hijos según el requisito del usuario
                // REGLA: Solo auto-expandir si es el inicio/fin del rango O si está colapsado (en medio).
                // Si está expandido y en medio, dejamos que el bucle del rango seleccione los elementos naturales.
                let is_boundary = item == start_item || item == end_item;

                match item {
                    LibraryListItem::Artist(name) => {
                        let is_collapsed = self.collapsed_artists.contains(name);
                        if is_boundary || is_collapsed {
                            if let Some(group) = self.artist_groups.iter().find(|g| g.name == *name)
                            {
                                for song in &group.songs {
                                    self.selected_items.insert(LibraryListItem::Song(song.id));
                                }
                            }
                        }
                    }
                    LibraryListItem::Album(id) => {
                        let is_collapsed = self.collapsed_albums.contains(id);
                        if is_boundary || is_collapsed {
                            // Intentar encontrar las canciones del álbum
                            if let Some(songs) = self.get_songs_for_album_id(id) {
                                for song in songs {
                                    self.selected_items.insert(LibraryListItem::Song(song.id));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn get_songs_for_album_id(
        &self,
        composite_id: &str,
    ) -> Option<Vec<std::sync::Arc<crate::db::database::SongData>>> {
        let parts: Vec<&str> = composite_id.split('|').collect();
        if parts.len() == 2 {
            let artist = parts[0];
            let album_id = parts[1];

            // Priorizar canciones FILTRADAS (búsqueda activa) para que las estadísticas y selección sean coherentes
            let songs_source = self
                .filtered_songs
                .as_ref()
                .or(self.cached_all_songs.as_ref());

            if let Some(songs) = songs_source {
                if let Some(alb_entry) = self
                    .cached_albums
                    .as_ref()
                    .and_then(|all| all.iter().find(|a| a.id == album_id && a.artist == artist))
                {
                    return Some(
                        songs
                            .iter()
                            .filter(|s| {
                                s.album.as_deref() == Some(&alb_entry.title)
                                    && crate::utils::get_effective_artist(s) == artist
                            })
                            .cloned()
                            .collect(),
                    );
                }
            }
        }
        None
    }

    fn sync_legacy_selection(&mut self, item: &LibraryListItem, _current_idx_in_visible: usize) {
        match item {
            LibraryListItem::Artist(name) => {
                self.selected_header = Some(name.clone());
                self.selected_album = None;
                self.selected_song_idx = None;
            }
            LibraryListItem::Album(id) => {
                self.selected_header = None;
                self.selected_album = Some(id.clone());
                self.selected_song_idx = None;
            }
            LibraryListItem::Song(id) => {
                self.selected_header = None;
                if self.view_mode != LibraryViewMode::Grid {
                    self.selected_album = None;
                } else {
                    self.selected_album = self.expanded_album.clone();
                }
                // Encontrar el índice local en la vista actual
                let songs_list = if self.view_mode == LibraryViewMode::Grid {
                    self.expanded_album_songs.as_ref()
                } else {
                    self.filtered_songs.as_ref()
                };

                if let Some(songs) = songs_list {
                    self.selected_song_idx = songs.iter().position(|s| s.id == *id);
                }
            }
        }
    }

    pub fn get_selected_songs(&self) -> Vec<Arc<crate::db::database::SongData>> {
        if self.selected_items.is_empty() {
            return Vec::new();
        }

        let mut explicit_song_ids = std::collections::HashSet::new();
        let mut albums_to_check = Vec::new();
        let mut artists_to_check = Vec::new();

        for item in &self.selected_items {
            match item {
                LibraryListItem::Song(id) => {
                    explicit_song_ids.insert(*id);
                }
                LibraryListItem::Album(id) => {
                    albums_to_check.push(id);
                }
                LibraryListItem::Artist(name) => {
                    artists_to_check.push(name);
                }
            }
        }

        let mut final_song_ids = explicit_song_ids.clone();

        // Optimización: Mapa de grupos de artistas para búsqueda O(1)
        let mut artist_map = std::collections::HashMap::new();
        for group in &self.artist_groups {
            artist_map.insert(&group.name, group);
        }

        // Expansión inteligente de álbumes
        for album_id in albums_to_check {
            if let Some(songs) = self.get_songs_for_album_id(album_id) {
                let has_explicit_songs = songs.iter().any(|s| explicit_song_ids.contains(&s.id));
                if !has_explicit_songs {
                    for s in songs {
                        final_song_ids.insert(s.id);
                    }
                }
            }
        }

        // Expansión inteligente de artistas
        for artist_name in artists_to_check {
            if let Some(group) = artist_map.get(artist_name) {
                let has_explicit_songs = group
                    .songs
                    .iter()
                    .any(|s| explicit_song_ids.contains(&s.id));
                if !has_explicit_songs {
                    for s in &group.songs {
                        final_song_ids.insert(s.id);
                    }
                }
            }
        }

        // Recolectar objetos SongData (Optimizado con mapa global si existe)
        let mut results = Vec::new();
        let songs_source = self
            .filtered_songs
            .as_ref()
            .or(self.cached_all_songs.as_ref());

        if let Some(songs) = songs_source {
            for song in songs {
                if final_song_ids.contains(&song.id) {
                    results.push(song.clone());
                }
            }
        }
        results
    }

    /// Obtiene el nombre del artista que se debe contraer/expandir según la dirección de la tecla flecha.
    pub fn get_artist_to_toggle(&self, dir: LibraryNavDir) -> Option<String> {
        let artist_name = if let Some(h) = &self.selected_header {
            Some(h.clone())
        } else if let Some(song_idx) = self.selected_song_idx {
            self.filtered_songs
                .as_ref()
                .and_then(|songs| songs.get(song_idx))
                .and_then(|s| {
                    s.artist
                        .as_ref()
                        .map(|s| s.to_string())
                        .or_else(|| s.album_artist.as_ref().map(|s| s.to_string()))
                })
        } else {
            None
        };

        if let Some(artist) = artist_name {
            let is_collapsed = self.collapsed_artists.contains(&artist);
            if (dir == LibraryNavDir::Left && !is_collapsed)
                || (dir == LibraryNavDir::Right && is_collapsed)
            {
                return Some(artist);
            }
        }
        None
    }

    /// Obtiene listas de álbumes y artistas a contraer/expandir en bloque.
    pub fn get_bulk_toggles(&self, dir: LibraryNavDir) -> (Vec<String>, Vec<String>) {
        let mut albums_to_toggle = std::collections::HashSet::new();
        let mut artists_to_toggle = std::collections::HashSet::new();

        let mut items_to_process = Vec::new();
        if !self.selected_items.is_empty() {
            items_to_process.extend(self.selected_items.iter().cloned());
        } else if let Some(focused) = &self.focused_item {
            items_to_process.push(focused.clone());
        }

        for item in items_to_process {
            match item {
                LibraryListItem::Artist(name) => {
                    let is_collapsed = self.collapsed_artists.contains(&name);
                    if (dir == LibraryNavDir::Left && !is_collapsed)
                        || (dir == LibraryNavDir::Right && is_collapsed)
                    {
                        artists_to_toggle.insert(name);
                    }
                }
                LibraryListItem::Album(id) => {
                    let is_collapsed = self.collapsed_albums.contains(&id);
                    if (dir == LibraryNavDir::Left && !is_collapsed)
                        || (dir == LibraryNavDir::Right && is_collapsed)
                    {
                        albums_to_toggle.insert(id);
                    }
                }
                LibraryListItem::Song(id) => {
                    if let Some(composite_id) = self.find_album_id_for_song(id) {
                        let is_collapsed = self.collapsed_albums.contains(&composite_id);
                        if (dir == LibraryNavDir::Left && !is_collapsed)
                            || (dir == LibraryNavDir::Right && is_collapsed)
                        {
                            albums_to_toggle.insert(composite_id);
                        }
                    }
                }
            }
        }

        (
            albums_to_toggle.into_iter().collect(),
            artists_to_toggle.into_iter().collect(),
        )
    }

    pub fn get_album_to_toggle(&self, dir: LibraryNavDir) -> Option<String> {
        let album_id = if let Some(alb) = &self.selected_album {
            Some(alb.clone())
        } else if let Some(song_idx) = self.selected_song_idx {
            if let Some(songs) = &self.filtered_songs {
                if let Some(s) = songs.get(song_idx) {
                    let art = crate::utils::get_effective_artist(s).to_string();
                    let alb_name = s
                        .album
                        .as_ref()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "Desconocido".to_string());

                    // Buscar hash para ID compuesto
                    let alb_hash = if let Some(cache) = &self.cached_albums {
                        cache
                            .iter()
                            .find(|a| a.title == alb_name && a.artist == art)
                            .map(|a| a.id.clone())
                            .unwrap_or_else(|| alb_name.clone())
                    } else {
                        alb_name.clone()
                    };
                    Some(format!("{}|{}", art, alb_hash))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(id) = album_id {
            let is_collapsed = self.collapsed_albums.contains(&id);
            if (dir == LibraryNavDir::Left && !is_collapsed)
                || (dir == LibraryNavDir::Right && is_collapsed)
            {
                return Some(id);
            }
        }
        None
    }

    /// Actualiza la memoria de la última canción seleccionada para un artista.
    pub fn update_selection_memory(&mut self) {
        if let Some(song_idx) = self.selected_song_idx {
            if let Some(songs) = &self.filtered_songs {
                if let Some(song) = songs.get(song_idx) {
                    let artist = song
                        .artist
                        .as_ref()
                        .map(|s| s.to_string())
                        .or_else(|| song.album_artist.as_ref().map(|s| s.to_string()))
                        .unwrap_or_else(|| "Artista Desconocido".to_string());
                    self.artist_last_selection.insert(artist, song_idx);
                }
            }
        }
    }

    pub fn select_song(&mut self, idx: Option<usize>) {
        self.selected_song_idx = idx;
    }
}
