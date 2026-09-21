//! Library message handlers extracted from the update dispatcher.

use crate::gui::app::{AppFocus, Message};
use crate::gui::library::LIBRARY_SCROLL_ID;
use iced::Task;
use iced::widget::operation::{AbsoluteOffset, focus, scroll_to};

impl crate::gui::app::AudoxidyApp {
    pub(in crate::gui::app) fn handle_library(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::LibraryFocus => {
                self.focus = AppFocus::Library;
                if self.library_manager.data_unloaded {
                    self.library_manager.data_unloaded = false;
                    return self.update(Message::LibraryRefresh);
                }
                Task::none()
            }
            Message::ScanLibrary(folder) => {
                self.core.scan_folder(folder);
                // Marcar que la BD necesita refrescarse tras el escaneo
                self.db_needs_refresh = true;
                self.update(Message::ScanTick)
            }
            Message::SearchQueryChanged(q) => {
                self.playlist_manager.search_query = q;
                self.playlist_manager.apply_filter();
                Task::none()
            }
            Message::LibrarySearchQueryChanged(q) => {
                self.focus = AppFocus::Library;
                if self.library_manager.data_unloaded {
                    self.library_manager.data_unloaded = false;
                    self.library_manager.search_query = q;
                    return self.update(Message::LibraryRefresh);
                }
                self.library_manager.search_query = q;
                self.library_manager.apply_filter();
                self.library_manager.last_viewport = None;
                Task::batch(vec![
                    self.trigger_library_search(),
                    scroll_to(LIBRARY_SCROLL_ID.clone(), AbsoluteOffset { x: 0.0, y: 0.0 }),
                ])
            }
            Message::LibrarySourceSelected(source) => {
                self.focus = AppFocus::Library;
                self.library_manager.source = source;
                // En el futuro, disparar recarga desde la fuente elegida
                Task::none()
            }
            Message::LibrarySortChanged(col) => {
                self.focus = AppFocus::Library;
                if self.library_manager.sort_column == Some(col) {
                    if self.library_manager.sort_ascending == Some(true) {
                        self.library_manager.sort_ascending = Some(false);
                    } else {
                        self.library_manager.sort_column = None;
                        self.library_manager.sort_ascending = None;
                    }
                } else {
                    self.library_manager.sort_column = Some(col);
                    self.library_manager.sort_ascending = Some(true);
                }

                if self.library_manager.sort_column.is_none() {
                    // Restaurar orden original desde DB
                    let albums = self.core.get_albums();
                    self.library_manager.cached_albums = Some(
                        albums
                            .into_iter()
                            .map(|a| crate::gui::library::AlbumEntry {
                                id: a.0,
                                title: a.1,
                                artist: a.2,
                                genre: a.3,
                                year: a.4,
                                cover_path: a.5,
                            })
                            .collect(),
                    );
                    if let Some(album_id) = &self.library_manager.expanded_album {
                        self.library_manager.expanded_album_songs =
                            Some(self.core.get_songs_by_album(album_id));
                    }
                } else {
                    // Aplicar el sort en memoria a albums
                    if let Some(albums) = &mut self.library_manager.cached_albums {
                        crate::gui::library::LibraryManager::sort_albums_static(
                            albums,
                            self.library_manager.sort_column,
                            self.library_manager.sort_ascending,
                        );
                    }

                    if let Some(mut songs) = self.library_manager.expanded_album_songs.take() {
                        self.library_manager.sort_songs(&mut songs);
                        self.library_manager.expanded_album_songs = Some(songs);
                    }

                    if let Some(mut songs) = self.library_manager.cached_all_songs.take() {
                        self.library_manager.sort_songs(&mut songs);
                        self.library_manager.cached_all_songs = Some(songs);
                        self.library_manager.apply_filter();
                    }
                }
                Task::none()
            }
            Message::StartColumnResize(col) => {
                self.library_manager.resizing_column = Some(col);
                self.library_manager.resizing_start_x =
                    self.player_ui_state.mouse_pos.map(|p| p.x).unwrap_or(0.0);
                if let Some(&w) = self.library_manager.column_widths.get(&col) {
                    self.library_manager.resizing_start_w = w;
                }
                Task::none()
            }
            Message::ColumnHover(col) => {
                self.library_manager.hovered_column = col;
                Task::none()
            }
            Message::ToggleAlbumExpansion(album_id) => {
                self.focus = AppFocus::Library;
                if self.library_manager.is_list_mode() {
                    let is_collapsed = self.library_manager.collapsed_albums.contains(&album_id);
                    self.library_manager
                        .set_album_collapsed(album_id, !is_collapsed);
                } else {
                    // Flecha: toggle expansión Y seleccionar álbum (sincronizado)
                    if self.library_manager.expanded_album.as_deref() == Some(album_id.as_str()) {
                        self.library_manager.expanded_album = None;
                        self.library_manager.expanded_album_songs = None;
                        self.library_manager.selected_song_idx = None;

                        // Sincronizar selección visual para que el álbum aparezca resaltado al plegar
                        let item = crate::gui::library::LibraryListItem::Album(album_id.clone());
                        self.library_manager.selected_items.clear();
                        self.library_manager.selected_items.insert(item.clone());
                        self.library_manager.focused_item = Some(item);
                        self.library_manager.selected_album = Some(album_id);
                    } else {
                        self.library_manager.expanded_album = Some(album_id.clone());
                        self.library_manager.selected_album = Some(album_id.clone());
                        self.library_manager.selected_song_idx = None;
                        self.library_manager.selected_item_hint = None;

                        // Sincronizar selección visual
                        let item = crate::gui::library::LibraryListItem::Album(album_id.clone());
                        self.library_manager.selected_items.clear();
                        self.library_manager.selected_items.insert(item.clone());
                        self.library_manager.focused_item = Some(item);

                        // Unificado: En lugar de ir a la DB, intentamos filtrar de los datos en RAM.
                        // Pero si los datos fueron descargados por el GC (data_unloaded), forzamos carga desde DB.
                        let mut alb_songs = Vec::new();
                        let mut found_in_cache = false;

                        if !self.library_manager.data_unloaded {
                            if let Some(all_songs) = &self.library_manager.cached_all_songs {
                                if let Some(album_entry) =
                                    self.library_manager.cached_albums.as_ref().and_then(|all| {
                                        all.iter().find(|a| {
                                            (format!("{}|{}", a.artist, a.id) == album_id)
                                                || (a.id == album_id)
                                        })
                                    })
                                {
                                    alb_songs = all_songs
                                        .iter()
                                        .filter(|s| {
                                            let s_alb = s.album.as_deref().unwrap_or("Desconocido");
                                            let s_art = crate::utils::get_effective_artist(s);
                                            s_alb == album_entry.title
                                                && s_art == album_entry.artist
                                        })
                                        .cloned()
                                        .collect();

                                    if !alb_songs.is_empty() {
                                        found_in_cache = true;
                                    }
                                }
                            }
                        }

                        // Fallback a Base de Datos: Si no está en caché o el GC limpió la RAM
                        if !found_in_cache {
                            if let Some(pos) = album_id.find('|') {
                                let artist = &album_id[..pos];
                                let hash = &album_id[pos + 1..];
                                alb_songs = self.core.get_songs_by_album_and_artist(hash, artist);
                            } else {
                                alb_songs = self.core.get_songs_by_album(album_id.as_str());
                            }
                        }

                        self.library_manager.sort_songs(&mut alb_songs);
                        self.library_manager.expanded_album_songs = Some(alb_songs);
                    }
                }
                self.library_manager.invalidate_cache();
                self.update_selection_stats();
                Task::none()
            }
            Message::ToggleArtistExpansion(name) => {
                self.focus = AppFocus::Library;
                let is_collapsed = self.library_manager.collapsed_artists.contains(&name);
                self.library_manager
                    .set_artist_collapsed(name, !is_collapsed);
                self.update_selection_stats();
                Task::none()
            }
            Message::LibraryAllSongsLoaded(songs) => {
                self.library_manager.cached_all_songs = Some(songs);
                self.library_manager.apply_filter();
                Task::none()
            }
            Message::SelectAlbum(album_id) => {
                self.focus = AppFocus::Library;
                let now = std::time::Instant::now();
                let is_double_click = if let Some((last_id, last_time)) =
                    &self.last_album_header_click
                {
                    last_id == &album_id
                        && now.duration_since(*last_time) < std::time::Duration::from_millis(500)
                } else {
                    false
                };

                if is_double_click {
                    self.last_album_header_click = None;
                    return self.update(Message::ToggleAlbumExpansion(album_id));
                } else {
                    self.last_album_header_click = Some((album_id.clone(), now));
                    self.library_manager.selected_album = Some(album_id.clone());
                    self.library_manager.selected_song_idx = None;
                    self.library_manager.selected_header = None;
                    self.library_manager.selected_item_hint = None;

                    let item = crate::gui::library::LibraryListItem::Album(album_id);
                    self.library_manager.handle_click(item, self.modifiers);

                    // También iniciamos el marquee al hacer clic
                    let pos = self.last_mouse_pos;
                    if pos.x >= 602.0 {
                        let lib_x = (pos.x - 602.0).max(0.0);
                        let scroll_y = self.library_manager.scroll_offset.y;
                        let lib_y = pos.y - 72.0 + scroll_y;
                        self.library_manager.marquee_start = Some(iced::Point::new(lib_x, lib_y));
                        self.library_manager.marquee_end = Some(iced::Point::new(lib_x, lib_y));
                        self.library_manager.marquee_start_pos = Some(pos);
                        self.library_manager.is_dragging = false;
                    }
                    Task::batch(vec![
                        focus(crate::gui::library::LIBRARY_SCROLL_ID.clone()),
                        Task::none(),
                    ])
                }
            }
            Message::SelectSong(song_id_opt) => {
                self.focus = AppFocus::Library;
                let task = if let Some(song_id) = song_id_opt {
                    let songs_opt = if self.library_manager.view_mode
                        == crate::gui::library::LibraryViewMode::Grid
                    {
                        self.library_manager.expanded_album_songs.as_ref()
                    } else {
                        self.library_manager.filtered_songs.as_ref()
                    };

                    if let Some(songs) = songs_opt {
                        if let Some(pos) = songs.iter().position(|s| s.id == song_id) {
                            let song = &songs[pos];
                            let artist = crate::utils::get_effective_artist(song).to_string();
                            self.library_manager
                                .artist_last_selection
                                .insert(artist, pos);
                            self.library_manager.selected_song_idx = Some(pos);
                        }
                    }

                    let item = crate::gui::library::LibraryListItem::Song(song_id);
                    self.library_manager.handle_click(item, self.modifiers);

                    // También iniciamos el marquee al hacer clic
                    let pos = self.last_mouse_pos;
                    if pos.x >= 602.0 {
                        let lib_x = (pos.x - 602.0).max(0.0);
                        let scroll_y = self.library_manager.scroll_offset.y;
                        let lib_y = pos.y - 72.0 + scroll_y;
                        self.library_manager.marquee_start = Some(iced::Point::new(lib_x, lib_y));
                        self.library_manager.marquee_end = Some(iced::Point::new(lib_x, lib_y));
                        self.library_manager.marquee_start_pos = Some(pos);
                        self.library_manager.is_dragging = false;
                    }
                    Task::batch(vec![
                        focus(crate::gui::library::LIBRARY_SCROLL_ID.clone()),
                        Task::none(),
                    ])
                } else {
                    self.library_manager.selected_song_idx = None;
                    self.library_manager.selected_items.clear();
                    self.library_manager.focused_item = None;
                    self.library_manager.selection_pivot = None;
                    self.update_selection_stats();
                    Task::none()
                };

                self.library_manager.selected_header = None;
                if self.library_manager.is_list_mode() {
                    self.library_manager.selected_album = None;
                } else if song_id_opt.is_some() {
                    if let Some(expanded) = self.library_manager.expanded_album.clone() {
                        self.library_manager.selected_album = Some(expanded);
                    }
                }
                self.update_selection_stats();
                task
            }
            Message::SelectArtistHeader(name) => {
                self.focus = AppFocus::Library;
                let now = std::time::Instant::now();
                let is_double_click = if let Some((last_name, last_time)) =
                    &self.last_artist_header_click
                {
                    last_name == &name
                        && now.duration_since(*last_time) < std::time::Duration::from_millis(500)
                } else {
                    false
                };

                if is_double_click {
                    self.last_artist_header_click = None;
                    return self.update(Message::ToggleArtistExpansion(name));
                } else {
                    self.last_artist_header_click = Some((name.clone(), now));
                    self.library_manager.selected_header = Some(name.clone());
                    self.library_manager.selected_song_idx = None;
                    self.library_manager.selected_album = None;

                    let item = crate::gui::library::LibraryListItem::Artist(name);
                    self.library_manager.handle_click(item, self.modifiers);

                    // También iniciamos el marquee al hacer clic
                    let pos = self.last_mouse_pos;
                    if pos.x >= 602.0 {
                        let lib_x = (pos.x - 602.0).max(0.0);
                        let scroll_y = self.library_manager.scroll_offset.y;
                        let lib_y = pos.y - 72.0 + scroll_y;
                        self.library_manager.marquee_start = Some(iced::Point::new(lib_x, lib_y));
                        self.library_manager.marquee_end = Some(iced::Point::new(lib_x, lib_y));
                        self.library_manager.marquee_start_pos = Some(pos);
                        self.library_manager.is_dragging = false;
                    }
                    self.get_library_scroll_task(false)
                }
            }
            Message::LibraryKeyNav(dir, modifiers) => {
                self.focus = AppFocus::Library;
                if self.library_manager.data_unloaded {
                    self.library_manager.data_unloaded = false;
                    return self.update(Message::LibraryRefresh);
                }
                use crate::gui::library::LibraryNavDir;

                if self.library_manager.is_list_mode() {
                    // Flecha Izquierda/Derecha -> Colapsar/Expandir jerarquías
                    if dir == LibraryNavDir::Left || dir == LibraryNavDir::Right {
                        let (albums, artists) = self.library_manager.get_bulk_toggles(dir);

                        if !albums.is_empty() || !artists.is_empty() {
                            for alb_id in albums {
                                let collapsed = dir == LibraryNavDir::Left;
                                self.library_manager.set_album_collapsed(alb_id, collapsed);
                            }
                            for art_name in artists {
                                let collapsed = dir == LibraryNavDir::Left;
                                self.library_manager
                                    .set_artist_collapsed(art_name, collapsed);
                            }

                            self.update_selection_stats();
                            return self.library_manager.get_scroll_task(false);
                        }
                        return Task::none();
                    }
                } else {
                    // Modo Grid: Alt + Arriba/Abajo -> Colapsar/Expandir álbum seleccionado
                    if (dir == LibraryNavDir::Up || dir == LibraryNavDir::Down)
                        && modifiers.alt()
                        && !modifiers.control()
                    {
                        if let Some(sel) = self.library_manager.selected_album.clone() {
                            let is_expanded = self.library_manager.expanded_album.as_deref()
                                == Some(sel.as_str());
                            if (dir == LibraryNavDir::Up && is_expanded)
                                || (dir == LibraryNavDir::Down && !is_expanded)
                            {
                                return self.update(Message::ToggleAlbumExpansion(sel));
                            }
                        }
                    }

                    // Al subir desde un álbum que ya está expandido, queremos colapsarlo (requisito de flujo de navegación)
                    if dir == LibraryNavDir::Up
                        && !modifiers.shift()
                        && !modifiers.alt()
                        && !modifiers.control()
                    {
                        if let Some(crate::gui::library::LibraryListItem::Album(sel)) =
                            &self.library_manager.focused_item
                        {
                            if self.library_manager.expanded_album.as_deref() == Some(sel.as_str())
                            {
                                return self.update(Message::ToggleAlbumExpansion(sel.clone()));
                            }
                        }
                    }
                }

                // Navegación Unificada delegada a LibraryManager
                if let Some((_new_item, _item_y, _item_h)) =
                    self.library_manager.handle_key_nav(dir, modifiers)
                {
                    // Navegación exitosa en biblioteca
                }

                self.library_manager.update_selection_memory();
                self.update_selection_stats();
                return self.library_manager.get_scroll_task(false);
            }
            Message::ExecutePendingScroll => self.library_manager.get_scroll_task(true),
            Message::LibraryMarqueeStart(pos) => {
                // Si la posición es (0,0), usamos la última posición conocida del mouse
                let actual_pos = if pos.x == 0.0 && pos.y == 0.0 {
                    self.last_mouse_pos
                } else {
                    pos
                };

                // Determinamos si el mouse está sobre el área de la biblioteca
                // Sidebar(400) + Filters(202) = 602
                if actual_pos.x >= 602.0 {
                    let lib_x = (actual_pos.x - 602.0).max(0.0);
                    let scroll_y = self.library_manager.scroll_offset.y;
                    let lib_y = actual_pos.y - 72.0 + scroll_y;

                    self.library_manager.marquee_start = Some(iced::Point::new(lib_x, lib_y));
                    self.library_manager.marquee_end = Some(iced::Point::new(lib_x, lib_y));
                    self.library_manager.marquee_start_pos = Some(actual_pos);
                    self.library_manager.is_dragging = false;
                    self.focus = AppFocus::Library;
                }
                Task::none()
            }
            Message::LibraryDeselect => {
                self.focus = AppFocus::Library;
                self.library_manager.selected_header = None;
                self.library_manager.selected_song_idx = None;
                self.library_manager.selected_album = None;
                self.library_manager.selected_items.clear();
                self.library_manager.focused_item = None;
                self.library_manager.selection_pivot = None;
                self.update_selection_stats();

                // También iniciamos el marquee al hacer clic en el fondo
                let pos = self.last_mouse_pos;
                let lib_x = (pos.x - 602.0).max(0.0);
                let scroll_y = self.library_manager.scroll_offset.y;
                let lib_y = pos.y - 72.0 + scroll_y;
                self.library_manager.marquee_start = Some(iced::Point::new(lib_x, lib_y));
                self.library_manager.marquee_end = Some(iced::Point::new(lib_x, lib_y));
                self.library_manager.marquee_start_pos = Some(pos);
                self.library_manager.is_dragging = false;

                Task::none()
            }
            Message::ChangeLibraryViewMode(mode) => {
                // Limpiar caché negativa al cambiar de vista
                crate::utils::covers::clear_negative_cover_cache();

                self.library_manager.view_mode = mode;
                self.library_manager.view_menu_open = false;

                if self.library_manager.data_unloaded {
                    // Si los datos fueron descargados por el GC, forzar una recarga completa desde la DB
                    self.library_manager.data_unloaded = false;
                    return self.update(Message::LibraryRefresh);
                }

                let mut tasks = Vec::new();

                if mode == crate::gui::library::LibraryViewMode::Grid {
                    // Al cambiar a Grid: asegurar que cached_albums esté cargado
                    if self.library_manager.cached_albums.is_none() {
                        let albums = self.core.get_albums();
                        self.library_manager.cached_albums = Some(
                            albums
                                .into_iter()
                                .map(|a| crate::gui::library::AlbumEntry {
                                    id: a.0,
                                    title: a.1,
                                    artist: a.2,
                                    genre: a.3,
                                    year: a.4,
                                    cover_path: a.5,
                                })
                                .collect(),
                        );
                    }
                }

                // Si por alguna razón filtered_songs es None, asegurar carga
                if self.library_manager.filtered_songs.is_none() {
                    tasks.push(self.trigger_library_search());
                } else {
                    self.library_manager.apply_filter();
                }

                // Liberar solo datos del Grid que no necesitamos en lista
                self.library_manager.expanded_album = None;
                self.library_manager.expanded_album_songs = None;

                // Forzar scroll al inicio al cambiar de vista (a pedido del usuario)
                self.library_manager.last_viewport = None;
                self.library_manager.scroll_offset = iced::Vector::new(0.0, 0.0);

                tasks.push(scroll_to(
                    crate::gui::library::LIBRARY_SCROLL_ID.clone(),
                    iced::widget::operation::AbsoluteOffset { x: 0.0, y: 0.0 },
                ));

                Task::batch(tasks)
            }
            Message::ToggleLibraryViewDropdown => {
                self.focus = AppFocus::Library;
                let next = match self.library_manager.view_mode {
                    crate::gui::library::LibraryViewMode::Grid => {
                        crate::gui::library::LibraryViewMode::DetailedList
                    }
                    crate::gui::library::LibraryViewMode::DetailedList => {
                        crate::gui::library::LibraryViewMode::ThumbnailList
                    }
                    crate::gui::library::LibraryViewMode::ThumbnailList => {
                        crate::gui::library::LibraryViewMode::SimpleList
                    }
                    crate::gui::library::LibraryViewMode::SimpleList => {
                        crate::gui::library::LibraryViewMode::Grid
                    }
                };
                // Delegar a ChangeLibraryViewMode para que cargue los datos correctos
                return self.update(Message::ChangeLibraryViewMode(next));
            }
            Message::ToggleLibraryAddDropdown => {
                self.library_manager.add_menu_open = !self.library_manager.add_menu_open;
                Task::none()
            }
            Message::PlayLibrarySelection => {
                // Prioridad: Si hay una canción seleccionada en la playlist (Enter en Playlist)
                if let Some(idx) = self.playlist_manager.focused_idx {
                    if let Some(song) = self.playlist_manager.get_song_at_linear_index(idx) {
                        let path = song.file_path.clone();
                        let title = song.title.clone();
                        let artist = song.artist_name.to_string();
                        let album = song.album_title.to_string();
                        let cover_path = song.cover_path.as_deref().map(str::to_string);
                        let s_id = song.song_id;

                        self.sync_player_art();
                        if let Err(e) = self.load_file_with_gains(
                            &path,
                            title.to_string(),
                            artist.to_string(),
                            album,
                            cover_path,
                        ) {
                            tracing::error!("Error reproduciendo archivo de playlist: {}", e);
                        } else {
                            self.core.play();
                            self.playlist_manager.playing_song_idx = Some(idx);
                            self.playlist_manager.notify_manual_play(s_id);
                            self.persist_playlist_state();
                        }
                        return Task::none();
                    }
                }

                // Si no, proceder con la lógica de selección de la biblioteca
                let selected_songs = self.library_manager.get_selected_songs();
                if !selected_songs.is_empty() {
                    return self.update(Message::PlayAlbum(selected_songs));
                }
                Task::none()
            }
            Message::PlayLibraryAll => Task::none(), // TODO
            Message::LibraryDeleteSelection => {
                let items = self.library_manager.selected_items.clone();
                if items.is_empty() {
                    return Task::none();
                }

                let mut sel_songs = std::collections::HashSet::new();
                let mut sel_albums = std::collections::HashSet::new();
                let mut sel_artists = std::collections::HashSet::new();

                for item in items {
                    match item {
                        crate::gui::library::LibraryListItem::Song(id) => {
                            sel_songs.insert(id);
                        }
                        crate::gui::library::LibraryListItem::Album(cid) => {
                            sel_albums.insert(cid);
                        }
                        crate::gui::library::LibraryListItem::Artist(name) => {
                            sel_artists.insert(name);
                        }
                    }
                }

                let mut ids_to_delete_set = std::collections::HashSet::new();
                let filtered_songs = self
                    .library_manager
                    .filtered_songs
                    .clone()
                    .unwrap_or_default();
                let cached_albums = self
                    .library_manager
                    .cached_albums
                    .clone()
                    .unwrap_or_default();

                // Mapa de (Artista, Álbum) -> Hash para reconstruir composite_id rápidamente
                let mut album_hash_map = std::collections::HashMap::new();
                for alb in &cached_albums {
                    album_hash_map.insert((alb.artist.clone(), alb.title.clone()), alb.id.clone());
                }

                for song in &filtered_songs {
                    // 1. Por ID de canción directo
                    if sel_songs.contains(&song.id) {
                        ids_to_delete_set.insert(song.id);
                        continue;
                    }

                    let art = crate::utils::get_effective_artist(song);

                    // 2. Por Artista
                    if sel_artists.contains(art) {
                        ids_to_delete_set.insert(song.id);
                        continue;
                    }

                    // 3. Por Álbum (usando composite_id)
                    let alb_name = song.album.as_deref().unwrap_or("Desconocido");
                    if let Some(hash) = album_hash_map.get(&(art.to_string(), alb_name.to_string()))
                    {
                        let composite_id = format!("{}|{}", art, hash);
                        if sel_albums.contains(&composite_id) {
                            ids_to_delete_set.insert(song.id);
                        }
                    }
                }

                if ids_to_delete_set.is_empty() {
                    return Task::none();
                }

                let ids_to_delete: Vec<_> = ids_to_delete_set.into_iter().collect();

                let core = self.core.clone();
                let filtered_songs_clone = self
                    .library_manager
                    .filtered_songs
                    .clone()
                    .unwrap_or_default();
                let cached_albums_clone = self
                    .library_manager
                    .cached_albums
                    .clone()
                    .unwrap_or_default();

                return Task::perform(
                    async move {
                        let mut covers_to_check = Vec::new();

                        {
                            let mut album_map = std::collections::HashMap::new();
                            for alb in &cached_albums_clone {
                                album_map.insert(
                                    (alb.artist.clone(), alb.title.clone()),
                                    alb.id.clone(),
                                );
                            }

                            let mut song_map = std::collections::HashMap::new();
                            for song in &filtered_songs_clone {
                                song_map.insert(song.id, song);
                            }

                            for id in &ids_to_delete {
                                if let Some(song) = song_map.get(id) {
                                    let art = crate::utils::get_effective_artist(song);
                                    let alb_name = song.album.as_deref().unwrap_or("Desconocido");

                                    if let Some(album_id) =
                                        album_map.get(&(art.to_string(), alb_name.to_string()))
                                    {
                                        covers_to_check.push(album_id.clone());
                                    }
                                    if let Some(ovr) = &song.cover_override {
                                        covers_to_check.push(ovr.to_string());
                                    }
                                }
                            }
                        }
                        covers_to_check.sort();
                        covers_to_check.dedup();

                        if let Err(e) =
                            core.delete_songs_with_transaction(&ids_to_delete, &covers_to_check)
                        {
                            tracing::warn!(
                                target: "audoxidy::persistence",
                                error = %e,
                                "No se pudo completar el borrado de canciones"
                            );
                        }
                    },
                    |_| Message::LibraryRefresh,
                );
            }
            Message::LibraryRefresh => {
                self.focus = AppFocus::Library;
                self.library_manager.data_unloaded = false;

                let core = self.core.clone();
                let params = self.library_manager.get_search_params();

                return Task::batch(vec![Task::perform(
                    async move { core.refresh_library_filtered(&params) },
                    |(songs, albums, stats)| {
                        Message::InternalLibraryRefreshed(songs, albums, stats)
                    },
                )]);
            }
            Message::LibraryFilteredLoaded(songs) => {
                self.library_manager.update_processed_data(songs);
                self.update_selection_stats();

                // Si había una petición de revelado pendiente (esperando a que los filtros se limpiaran), continuar ahora.
                if let Some(path) = self.library_manager.pending_reveal_path.clone() {
                    return self.handle_library_reveal(Some(path));
                }

                Task::none()
            }
            Message::InternalLibraryRefreshed(songs, albums, stats) => {
                self.library_manager.cached_all_songs = Some(songs.clone());
                self.library_manager.cached_albums = Some(
                    albums
                        .into_iter()
                        .map(|a| crate::gui::library::AlbumEntry {
                            id: a.0,
                            title: a.1,
                            artist: a.2,
                            genre: a.3,
                            year: a.4,
                            cover_path: a.5,
                        })
                        .collect(),
                );

                // Actualizar estadísticas globales
                self.library_manager.total_songs = stats.0;
                self.library_manager.total_albums = stats.1;
                self.library_manager.total_duration_secs = stats.2;
                self.library_manager.total_size_bytes = stats.3;
                self.library_manager.total_artists = stats.4;

                self.library_manager.selected_items.clear();
                self.library_manager.update_processed_data(songs);
                self.filters_manager
                    .build_filter_index(self.core.database());
                self.update_selection_stats();

                // Si había una petición de revelado pendiente (esperando a que los filtros se limpiaran), continuar ahora.
                if let Some(path) = self.library_manager.pending_reveal_path.clone() {
                    return self.handle_library_reveal(Some(path));
                }

                Task::none()
            }
            Message::LibraryShowPlaying => {
                self.focus = AppFocus::Library;
                let path_opt = self
                    .playlist_manager
                    .playing_song_idx
                    .and_then(|idx| self.playlist_manager.get_song_at_linear_index(idx))
                    .map(|s| s.file_path.clone());
                self.handle_library_reveal(path_opt.map(|s| s.to_string()))
            }
            Message::ToggleFilterMenu => {
                self.filters_manager.menu_open = !self.filters_manager.menu_open;
                Task::none()
            }
            Message::ChangeGeneralFilter(ft) => {
                self.focus = AppFocus::Library;
                if self.library_manager.data_unloaded {
                    self.library_manager.data_unloaded = false;
                    self.filters_manager.current_filter = ft;
                    return self.update(Message::LibraryRefresh);
                }
                self.filters_manager.current_filter = ft;
                self.filters_manager.selected_subfilter = None;
                self.filters_manager.selected_tree_node = None;
                self.filters_manager.expanded_nodes.clear();
                self.filters_manager.menu_open = false;

                // Reset internal library filters when changing general filter type
                self.library_manager.filter_artist = None;
                self.library_manager.filter_album = None;
                self.library_manager.filter_genre = None;
                self.library_manager.filter_year = None;
                self.library_manager.filter_folder_id = None;
                // Limpiar selección de la biblioteca para que filter_stats tenga prioridad
                self.library_manager.selected_album = None;
                self.library_manager.selected_song_idx = None;
                self.library_manager.selected_header = None;
                self.library_manager.selection_stats = None;
                self.library_manager.apply_filter();
                self.filters_manager.apply_view();
                self.library_manager.last_viewport = None;
                self.library_manager.scroll_offset = iced::Vector::new(0.0, 0.0);
                self.filters_manager.scroll_offset = iced::Vector::new(0.0, 0.0);

                Task::batch(vec![
                    self.trigger_library_search(),
                    scroll_to(LIBRARY_SCROLL_ID.clone(), AbsoluteOffset { x: 0.0, y: 0.0 }),
                    scroll_to(
                        crate::gui::library_filters::FILTERS_SCROLL_ID.clone(),
                        AbsoluteOffset { x: 0.0, y: 0.0 },
                    ),
                ])
            }
            Message::SelectSubfilter(sub) => {
                self.focus = AppFocus::Library;
                if sub == self.filters_manager.selected_subfilter {
                    self.filters_manager.selected_subfilter = None;
                } else {
                    self.filters_manager.selected_subfilter = sub;
                }
                self.filters_manager.selected_tree_node = None;
                self.filters_manager.scroll_offset = iced::Vector::new(0.0, 0.0);

                // Limpiar selección de la biblioteca
                self.library_manager.selected_album = None;
                self.library_manager.selected_song_idx = None;
                self.library_manager.selected_header = None;
                self.library_manager.selection_stats = None;

                // Si se selecciona None (Mostrar todo), resetear filtros de la biblioteca
                if self.filters_manager.selected_subfilter.is_none() {
                    self.library_manager.filter_artist = None;
                    self.library_manager.filter_album = None;
                    self.library_manager.filter_genre = None;
                    self.library_manager.filter_year = None;
                    self.library_manager.filter_folder_id = None;
                    self.library_manager.apply_filter();
                }
                self.filters_manager.apply_view();
                self.library_manager.last_viewport = None;
                self.library_manager.scroll_offset = iced::Vector::new(0.0, 0.0);
                Task::batch(vec![
                    self.trigger_library_search(),
                    scroll_to(LIBRARY_SCROLL_ID.clone(), AbsoluteOffset { x: 0.0, y: 0.0 }),
                ])
            }
            Message::ToggleTreeNode(name) => {
                if self.filters_manager.expanded_nodes.contains(&name) {
                    self.filters_manager.expanded_nodes.remove(&name);
                } else {
                    self.filters_manager.expanded_nodes.insert(name.clone());
                }
                self.filters_manager.refresh_flattened_tree();
                Task::none()
            }
            Message::SelectTreeNode(name) => {
                self.focus = AppFocus::Library;
                self.filters_manager.selected_tree_node = Some(name.clone());

                // Limpiar selección de la biblioteca para que filter_stats tenga prioridad
                self.library_manager.selected_album = None;
                self.library_manager.selected_song_idx = None;
                self.library_manager.selected_header = None;
                self.library_manager.selection_stats = None;

                // Apply filter to library
                self.library_manager.filter_artist = None;
                self.library_manager.filter_album = None;
                self.library_manager.filter_genre = None;
                self.library_manager.filter_year = None;
                self.library_manager.filter_folder_id = None;

                let parts: Vec<&str> = name.split('|').collect();
                // parts[0] is the FilterType label string (e.g. "Genre")
                let main_val = parts.get(1).copied(); // Level 1 (e.g. Genre name)
                let sub_val = parts.get(2).copied(); // Level 2 (e.g. Artist name)
                let third_val = parts.get(3).copied(); // Level 3 (e.g. Album name)

                match self.filters_manager.current_filter {
                    crate::gui::library_filters::FilterType::Artist => {
                        if let Some(art) = main_val {
                            self.library_manager.filter_artist = Some(art.to_string());
                        }
                        if let Some(alb) = sub_val {
                            self.library_manager.filter_album = Some(alb.to_string());
                        }
                    }
                    crate::gui::library_filters::FilterType::Album => {
                        if let Some(alb) = main_val {
                            self.library_manager.filter_album = Some(alb.to_string());
                        }
                        if let Some(art) = sub_val {
                            self.library_manager.filter_artist = Some(art.to_string());
                        }
                    }
                    crate::gui::library_filters::FilterType::Genre => {
                        if let Some(gnr) = main_val {
                            self.library_manager.filter_genre = Some(gnr.to_string());
                        }
                        if let Some(art) = sub_val {
                            self.library_manager.filter_artist = Some(art.to_string());
                        }
                        if let Some(alb) = third_val {
                            self.library_manager.filter_album = Some(alb.to_string());
                        }
                    }
                    crate::gui::library_filters::FilterType::Year => {
                        if let Some(yr) = main_val {
                            self.library_manager.filter_year = Some(yr.to_string());
                        }
                        if let Some(art) = sub_val {
                            self.library_manager.filter_artist = Some(art.to_string());
                        }
                        if let Some(alb) = third_val {
                            self.library_manager.filter_album = Some(alb.to_string());
                        }
                    }
                    crate::gui::library_filters::FilterType::Folder => {
                        // Manejado por Message::SelectFolder
                    }
                }

                self.library_manager.apply_filter();
                self.library_manager.last_viewport = None;
                self.library_manager.scroll_offset = iced::Vector::new(0.0, 0.0);
                Task::batch(vec![
                    self.trigger_library_search(),
                    scroll_to(LIBRARY_SCROLL_ID.clone(), AbsoluteOffset { x: 0.0, y: 0.0 }),
                ])
            }
            Message::SelectFolder(id) => {
                self.focus = AppFocus::Library;
                // Limpiar otros filtros para evitar conflictos si estamos en modo carpeta
                self.library_manager.filter_artist = None;
                self.library_manager.filter_album = None;
                self.library_manager.filter_genre = None;
                self.library_manager.filter_year = None;
                // Limpiar selección de la biblioteca
                self.library_manager.selected_album = None;
                self.library_manager.selected_song_idx = None;
                self.library_manager.selected_header = None;
                self.library_manager.selection_stats = None;

                self.library_manager.filter_folder_id = Some(id);
                self.library_manager.apply_filter();
                self.library_manager.last_viewport = None;
                self.library_manager.scroll_offset = iced::Vector::new(0.0, 0.0);
                Task::batch(vec![
                    self.trigger_library_search(),
                    scroll_to(LIBRARY_SCROLL_ID.clone(), AbsoluteOffset { x: 0.0, y: 0.0 }),
                ])
            }
            Message::FilterSearchChanged(q) => {
                self.focus = AppFocus::Library;
                self.filters_manager.search_query = q;
                self.filters_manager.apply_view();
                self.filters_manager.scroll_offset = iced::Vector::new(0.0, 0.0);
                scroll_to(
                    crate::gui::library_filters::FILTERS_SCROLL_ID.clone(),
                    iced::widget::operation::AbsoluteOffset { x: 0.0, y: 0.0 },
                )
            }
            Message::LibraryScroll(offset) => {
                if self.library_manager.data_unloaded {
                    self.library_manager.data_unloaded = false;
                    return self.update(Message::LibraryRefresh);
                }

                let abs_offset = offset.absolute_offset();
                self.library_manager.scroll_offset = iced::Vector::new(abs_offset.x, abs_offset.y);
                self.library_manager.last_viewport = Some(iced::Rectangle {
                    x: offset.absolute_offset().x,
                    y: offset.absolute_offset().y,
                    width: offset.bounds().width,
                    height: offset.bounds().height,
                });

                // Si estamos arrastrando, actualizamos el punto final del marquee basado en la nueva posición de scroll
                if self.library_manager.is_dragging {
                    if let Some(pos) = self.player_ui_state.mouse_pos {
                        let lib_x = (pos.x - 602.0).max(0.0);
                        let lib_y = pos.y - 72.0 + offset.absolute_offset().y;
                        self.library_manager.marquee_end = Some(iced::Point::new(lib_x, lib_y));
                    }
                }

                self.library_manager.invalidate_visible_cache();
                Task::none()
            }
            Message::FilterScroll(viewport) => {
                let abs_offset = viewport.absolute_offset();
                self.filters_manager.scroll_offset = iced::Vector::new(abs_offset.x, abs_offset.y);
                self.filters_manager.last_viewport = Some(iced::Rectangle {
                    x: viewport.absolute_offset().x,
                    y: viewport.absolute_offset().y,
                    width: viewport.bounds().width,
                    height: viewport.bounds().height,
                });
                self.filters_manager.refresh_flattened_tree();
                Task::none()
            }
            _ => Task::none(),
        }
    }

    pub(in crate::gui::app) fn get_library_scroll_task(&self, force_top: bool) -> Task<Message> {
        self.library_manager.get_scroll_task(force_top)
    }

    pub(in crate::gui::app) fn update_selection_stats(&mut self) {
        self.library_manager.update_selection_stats();
    }

    pub(in crate::gui::app) fn handle_library_reveal(
        &mut self,
        path_opt: Option<String>,
    ) -> Task<Message> {
        tracing::debug!(
            "Audoxidy Debug: handle_library_reveal path_opt: {:?}",
            path_opt
        );
        if let Some(path) = path_opt {
            if let Some(all_songs) = &self.library_manager.cached_all_songs {
                if let Some(lib_song) = all_songs
                    .iter()
                    .find(|s| s.full_file_path.as_ref() == path)
                    .cloned()
                {
                    tracing::debug!("Audoxidy Debug: lib_song found in cached_all_songs");
                    self.focus = crate::gui::app::AppFocus::Library;

                    // 1. Determinar si la canción es visible actualmente en la lista procesada
                    let is_visible = self
                        .library_manager
                        .filtered_songs
                        .as_ref()
                        .map(|songs| songs.iter().any(|s| s.full_file_path.as_ref() == path))
                        .unwrap_or(false); // Si no hay filtered_songs, NO es visible (lista vacía)

                    tracing::debug!("Audoxidy Debug: is_visible: {}", is_visible);

                    if !is_visible {
                        // Si no es visible, limpiamos todo y disparamos una recarga asíncrona.
                        // Guardamos la ruta para que InternalLibraryRefreshed la retome al terminar.
                        self.library_manager.search_query.clear();
                        self.library_manager.filter_artist = None;
                        self.library_manager.filter_album = None;
                        self.library_manager.filter_genre = None;
                        self.library_manager.filter_year = None;
                        self.library_manager.filter_folder_id = None;

                        self.filters_manager.selected_subfilter = None;
                        self.filters_manager.selected_tree_node = None;
                        self.filters_manager.apply_view();

                        self.library_manager.pending_reveal_path = Some(path.clone());
                        self.library_manager.apply_filter();

                        // IMPORTANTE: Disparar la búsqueda asíncrona
                        return self.trigger_library_search();
                    }

                    // Si llegamos aquí, la canción ES visible en filtered_songs.
                    // Limpiamos cualquier reveal pendiente.
                    self.library_manager.pending_reveal_path = None;

                    match self.library_manager.view_mode {
                        crate::gui::library::LibraryViewMode::Grid => {
                            let album_name = lib_song.album.as_deref().unwrap_or("Desconocido");
                            let artist_name = lib_song.artist.as_deref().unwrap_or("Desconocido");

                            let albums_list = self
                                .library_manager
                                .filtered_albums
                                .as_ref()
                                .or(self.library_manager.cached_albums.as_ref());

                            if let Some(albums) = albums_list {
                                if let Some(album_info) = albums
                                    .iter()
                                    .find(|a| a.title == album_name && a.artist == artist_name)
                                {
                                    let hash_id = album_info.id.clone();
                                    let composite_id = format!("{}|{}", artist_name, hash_id);

                                    if self.library_manager.expanded_album.as_deref()
                                        != Some(&composite_id)
                                    {
                                        self.library_manager.expanded_album =
                                            Some(composite_id.clone());
                                        // Poblar expanded_album_songs inmediatamente para el cálculo de scroll
                                        let mut songs = self
                                            .core
                                            .get_songs_by_album_and_artist(&hash_id, artist_name);
                                        if self.library_manager.sort_column.is_some() {
                                            self.library_manager.sort_songs(&mut songs);
                                        }
                                        self.library_manager.expanded_album_songs = Some(songs);
                                    }

                                    let cols = self.library_manager.albums_per_row.get().max(2);
                                    let mut current_y = 0.0;
                                    let mut found_y = None;
                                    let search_query = self.library_manager.search_query.trim();

                                    for chunk in albums.chunks(cols) {
                                        let mut row_h = 252.0;
                                        let mut _has_expanded_here = false;
                                        for a in chunk {
                                            let comp = format!("{}|{}", a.artist, a.id);
                                            if self.library_manager.expanded_album.as_deref()
                                                == Some(&comp)
                                            {
                                                let s_count = if let Some(songs) =
                                                    &self.library_manager.expanded_album_songs
                                                {
                                                    if search_query.is_empty() {
                                                        songs.len()
                                                    } else {
                                                        songs
                                                            .iter()
                                                            .filter(|s| {
                                                                crate::utils::song_matches_search(
                                                                    s,
                                                                    search_query,
                                                                )
                                                            })
                                                            .count()
                                                    }
                                                } else {
                                                    0
                                                };
                                                row_h += 60.0 + (s_count.max(1) as f32 * 32.0);
                                                _has_expanded_here = true;
                                                break;
                                            }
                                        }

                                        if chunk
                                            .iter()
                                            .any(|a| a.id == hash_id && a.artist == artist_name)
                                        {
                                            if let Some(expanded) =
                                                &self.library_manager.expanded_album_songs
                                            {
                                                let mut rendered_idx = 0;
                                                let mut found_match = false;
                                                for (idx, s) in expanded.iter().enumerate() {
                                                    let matches = search_query.is_empty()
                                                        || crate::utils::song_matches_search(
                                                            s,
                                                            search_query,
                                                        );
                                                    if matches {
                                                        if s.full_file_path.as_ref() == path {
                                                            self.library_manager
                                                                .selected_song_idx = Some(idx);
                                                            self.library_manager.selected_album =
                                                                Some(composite_id.clone());

                                                            let item = crate::gui::library::LibraryListItem::Song(s.id);
                                                            self.library_manager
                                                                .selected_items
                                                                .clear();
                                                            self.library_manager
                                                                .selected_items
                                                                .insert(item.clone());
                                                            self.library_manager.focused_item =
                                                                Some(item.clone());
                                                            self.library_manager.selection_pivot =
                                                                Some(item);

                                                            found_y = Some(
                                                                current_y
                                                                    + 252.0
                                                                    + 30.0
                                                                    + (rendered_idx as f32 * 32.0),
                                                            );
                                                            found_match = true;
                                                            break;
                                                        }
                                                        rendered_idx += 1;
                                                    }
                                                }
                                                if found_match {
                                                    if let Some(y) = found_y {
                                                        self.library_manager.selected_item_hint =
                                                            Some((y, 32.0));
                                                    }
                                                    break;
                                                }
                                            }
                                        }
                                        current_y += row_h;
                                    }
                                }
                            }
                        }
                        _ => {
                            let artist_name =
                                crate::utils::get_effective_artist(&lib_song).to_string();
                            self.library_manager.collapsed_artists.remove(&artist_name);

                            if self.library_manager.view_mode
                                == crate::gui::library::LibraryViewMode::DetailedList
                            {
                                if let Some(album_name) = lib_song.album.as_deref() {
                                    let albums_list = self
                                        .library_manager
                                        .filtered_albums
                                        .as_ref()
                                        .or(self.library_manager.cached_albums.as_ref());

                                    if let Some(albums) = albums_list {
                                        if let Some(album_info) = albums.iter().find(|a| {
                                            a.title == album_name && a.artist == artist_name
                                        }) {
                                            let composite_id =
                                                format!("{}|{}", artist_name, album_info.id);
                                            self.library_manager
                                                .collapsed_albums
                                                .remove(&composite_id);
                                        }
                                    }
                                }
                            }

                            // Invalidar caché visual para que get_item_y_range pueda encontrar el ítem ahora que expandimos los nodos
                            self.library_manager.invalidate_cache();

                            if let Some(filtered) = &self.library_manager.filtered_songs {
                                if let Some(pos) = filtered
                                    .iter()
                                    .position(|s| s.full_file_path.as_ref() == path)
                                {
                                    let song_id = filtered[pos].id;
                                    self.library_manager.selected_song_idx = Some(pos);
                                    self.library_manager.selected_header = None;
                                    self.library_manager.selected_album = None;

                                    let item = crate::gui::library::LibraryListItem::Song(song_id);
                                    self.library_manager.selected_items.clear();
                                    self.library_manager.selected_items.insert(item.clone());
                                    self.library_manager.focused_item = Some(item.clone());
                                    self.library_manager.selection_pivot = Some(item.clone());

                                    if self.library_manager.view_mode
                                        != crate::gui::library::LibraryViewMode::Grid
                                    {
                                        if let Some((y, h)) =
                                            self.library_manager.get_item_y_range(&item)
                                        {
                                            self.library_manager.selected_item_hint = Some((y, h));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    self.update_selection_stats();
                    // Forzar top_scroll o center scroll para que la canción siempre se vea al revelar
                    // Se difiere un frame para permitir que la interfaz (Iced) construya los widgets expandidos primero
                    return Task::perform(async {}, |_| Message::ExecutePendingScroll);
                }
            }
        }
        Task::none()
    }

    /// Dispara una búsqueda asíncrona en la base de datos basada en los filtros actuales.
    pub(in crate::gui::app) fn trigger_library_search(&self) -> Task<Message> {
        let core = self.core.clone();
        let params = self.library_manager.get_search_params();

        Task::perform(
            async move { core.get_library_songs_filtered(&params) },
            Message::LibraryFilteredLoaded,
        )
    }
}
