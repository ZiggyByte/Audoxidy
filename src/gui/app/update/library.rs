//! Library message handlers extracted from the update dispatcher.

use crate::gui::app::{AppFocus, Message};
use crate::gui::dimensions::LIBRARY_CONTENT_X_ORIGIN;
use crate::gui::library::cover_cache::SCROLL_IDLE_MS;
use crate::gui::library::grid_metrics::anchor_viewport_offset;
use crate::gui::library::{GridMetrics, LIBRARY_SCROLL_ID};
use iced::Task;
use iced::widget::operation::{AbsoluteOffset, focus, scroll_to};

/// Delayed message that requests the look-ahead band once the scroll is idle.
fn scroll_idle_task() -> Task<Message> {
    Task::perform(
        async {
            tokio::time::sleep(std::time::Duration::from_millis(SCROLL_IDLE_MS)).await;
        },
        |_| Message::CoverPrefetchDue,
    )
}

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
                // A stale element or a test can deliver a column the header bar no
                // longer emits; a display-only column has no ordering to apply.
                if !col.is_sortable() {
                    return Task::none();
                }
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
                        let hash = album_id
                            .split_once('|')
                            .map(|(_, hash)| hash)
                            .unwrap_or(album_id.as_str());
                        self.library_manager.expanded_album_songs =
                            Some(self.core.get_songs_by_album(hash));
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

                // Sorting changes the first rows the viewport draws, so the decode
                // window has to follow the new order.
                self.library_manager.request_cover_window();
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

                        if !self.library_manager.data_unloaded
                            && let Some(all_songs) = &self.library_manager.cached_all_songs
                            && let Some(album_entry) =
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
                                    crate::gui::library::song_belongs_to_album(s, album_entry)
                                })
                                .cloned()
                                .collect();

                            if !alb_songs.is_empty() {
                                found_in_cache = true;
                            }
                        }

                        // Fallback a Base de Datos: Si no está en caché o el GC limpió la RAM
                        if !found_in_cache {
                            // The hash identifies one album row on its own, so the
                            // track artist is not part of the lookup: a compilation
                            // whose tracks credit different artists still expands
                            // complete.
                            if let Some(pos) = album_id.find('|') {
                                let hash = &album_id[pos + 1..];
                                alb_songs = self.core.get_songs_by_album(hash);
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
                // Expanding or collapsing an album changes the rows around it
                // without a scroll, so the window follows the new set.
                self.library_manager.request_cover_window();
                Task::none()
            }
            Message::ToggleArtistExpansion(name) => {
                self.focus = AppFocus::Library;
                let is_collapsed = self.library_manager.collapsed_artists.contains(&name);
                self.library_manager
                    .set_artist_collapsed(name, !is_collapsed);
                self.update_selection_stats();
                // A collapsed artist hides every album below it, so the visible
                // set shrinks without a scroll and the window must follow.
                self.library_manager.request_cover_window();
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
                    self.update(Message::ToggleAlbumExpansion(album_id))
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
                    if pos.x >= LIBRARY_CONTENT_X_ORIGIN {
                        let lib_x = (pos.x - LIBRARY_CONTENT_X_ORIGIN).max(0.0);
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

                    if let Some(songs) = songs_opt
                        && let Some(pos) = songs.iter().position(|s| s.id == song_id)
                    {
                        let song = &songs[pos];
                        let artist = crate::utils::get_effective_artist(song).to_string();
                        self.library_manager
                            .artist_last_selection
                            .insert(artist, pos);
                        self.library_manager.selected_song_idx = Some(pos);
                    }

                    let item = crate::gui::library::LibraryListItem::Song(song_id);
                    self.library_manager.handle_click(item, self.modifiers);

                    // También iniciamos el marquee al hacer clic
                    let pos = self.last_mouse_pos;
                    if pos.x >= LIBRARY_CONTENT_X_ORIGIN {
                        let lib_x = (pos.x - LIBRARY_CONTENT_X_ORIGIN).max(0.0);
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
                } else if song_id_opt.is_some()
                    && let Some(expanded) = self.library_manager.expanded_album.clone()
                {
                    self.library_manager.selected_album = Some(expanded);
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
                    self.update(Message::ToggleArtistExpansion(name))
                } else {
                    self.last_artist_header_click = Some((name.clone(), now));
                    self.library_manager.selected_header = Some(name.clone());
                    self.library_manager.selected_song_idx = None;
                    self.library_manager.selected_album = None;

                    let item = crate::gui::library::LibraryListItem::Artist(name);
                    self.library_manager.handle_click(item, self.modifiers);

                    // También iniciamos el marquee al hacer clic
                    let pos = self.last_mouse_pos;
                    if pos.x >= LIBRARY_CONTENT_X_ORIGIN {
                        let lib_x = (pos.x - LIBRARY_CONTENT_X_ORIGIN).max(0.0);
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
                        && let Some(sel) = self.library_manager.selected_album.clone()
                    {
                        let is_expanded =
                            self.library_manager.expanded_album.as_deref() == Some(sel.as_str());
                        if (dir == LibraryNavDir::Up && is_expanded)
                            || (dir == LibraryNavDir::Down && !is_expanded)
                        {
                            return self.update(Message::ToggleAlbumExpansion(sel));
                        }
                    }

                    // Al subir desde un álbum que ya está expandido, queremos colapsarlo (requisito de flujo de navegación)
                    if dir == LibraryNavDir::Up
                        && !modifiers.shift()
                        && !modifiers.alt()
                        && !modifiers.control()
                        && let Some(crate::gui::library::LibraryListItem::Album(sel)) =
                            &self.library_manager.focused_item
                        && self.library_manager.expanded_album.as_deref() == Some(sel.as_str())
                    {
                        return self.update(Message::ToggleAlbumExpansion(sel.clone()));
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
                self.library_manager.get_scroll_task(false)
            }
            Message::ExecutePendingScroll => self.library_manager.get_scroll_task(true),
            Message::LibraryMarqueeStart(pos) => {
                // Si la posición es (0,0), usamos la última posición conocida del mouse
                let actual_pos = if pos.x == 0.0 && pos.y == 0.0 {
                    self.last_mouse_pos
                } else {
                    pos
                };

                // A marquee only starts inside the library content, whose left
                // edge begins after the player column and the filters panel.
                if actual_pos.x >= LIBRARY_CONTENT_X_ORIGIN {
                    let lib_x = (actual_pos.x - LIBRARY_CONTENT_X_ORIGIN).max(0.0);
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
                let lib_x = (pos.x - LIBRARY_CONTENT_X_ORIGIN).max(0.0);
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
                // The grid anchor belongs to the grid geometry; a list view has
                // fixed row heights and must never inherit a grid correction.
                self.library_manager.anchored_metrics = None;

                // The choice is stored before the reload path returns, so it
                // survives a restart even when the library data had been
                // unloaded. Only the mode persists.
                crate::gui::app::persistence::log_persist(
                    "library_view_mode",
                    self.core
                        .persist_audio_setting("library_view_mode", mode.as_setting_value()),
                );

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
                self.library_manager.scroll_offset = iced::Vector::new(0.0, 0.0);

                // Both branches replace the rows the viewport draws, and the mode
                // also changes the decode bucket, so the window is requested for
                // the top of the view that is about to render, at the height the
                // current viewport reports. The search branch requests it again
                // when its result arrives.
                self.library_manager.request_cover_window();

                self.library_manager.last_viewport = None;

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
                self.update(Message::ChangeLibraryViewMode(next))
            }
            Message::ToggleLibraryAddDropdown => {
                self.library_manager.add_menu_open = !self.library_manager.add_menu_open;
                Task::none()
            }
            Message::PlayLibrarySelection => {
                // Prioridad: Si hay una canción seleccionada en la playlist (Enter en Playlist)
                if let Some(idx) = self.playlist_manager.focused_idx
                    && let Some(song) = self.playlist_manager.get_song_at_linear_index(idx)
                {
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

                Task::perform(
                    async move {
                        if let Err(e) = core.delete_songs_with_transaction(&ids_to_delete) {
                            tracing::warn!(
                                target: "audoxidy::persistence",
                                error = %e,
                                "No se pudo completar el borrado de canciones"
                            );
                        }
                    },
                    |_| Message::LibraryRefresh,
                )
            }
            Message::FtsRebuildFinished(rebuilt) => {
                if rebuilt {
                    tracing::info!(
                        target: "audoxidy::persistence",
                        "índice de búsqueda reconstruido al iniciar"
                    );
                } else {
                    tracing::debug!(
                        target: "audoxidy::persistence",
                        "índice de búsqueda verificado al iniciar"
                    );
                }
                Task::none()
            }
            Message::LibraryRefresh => {
                self.focus = AppFocus::Library;
                self.library_manager.data_unloaded = false;

                let core = self.core.clone();
                let params = self.library_manager.get_search_params();

                Task::batch(vec![Task::perform(
                    async move { core.refresh_library_filtered(&params) },
                    |(songs, albums, stats)| {
                        Message::InternalLibraryRefreshed(songs, albums, stats)
                    },
                )])
            }
            Message::LibraryFilteredLoaded(songs) => {
                self.library_manager.update_processed_data(songs);
                self.update_selection_stats();

                // A search, a filter or any other query changes which rows are
                // drawn without a scroll, so the decode window follows the new
                // snapshot. The pass is idempotent: with everything cached it
                // accepts no job.
                self.library_manager.request_cover_window();

                // Si había una petición de revelado pendiente (esperando a que los filtros se limpiaran), continuar ahora.
                if let Some(path) = self.library_manager.pending_reveal_path.clone() {
                    return self.handle_library_reveal(Some(path));
                }

                Task::none()
            }
            Message::InternalLibraryRefreshed(songs, albums, stats) => {
                // Read before either collection is consumed: the sweep trigger
                // below needs this snapshot's emptiness, not the rows built from it.
                let library_is_empty = songs.is_empty() && albums.is_empty();
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
                // The snapshot now holds the cover paths the next layout will read.
                self.library_manager.request_cover_window();

                // Si había una petición de revelado pendiente (esperando a que los filtros se limpiaran), continuar ahora.
                if let Some(path) = self.library_manager.pending_reveal_path.clone() {
                    return self.handle_library_reveal(Some(path));
                }

                // An empty snapshot with no filter active means the library is
                // genuinely empty, so no row can own a cover file any more.
                let params = self.library_manager.get_search_params();
                let unfiltered = params.query.is_none()
                    && params.artist.is_none()
                    && params.album.is_none()
                    && params.genre.is_none()
                    && params.year.is_none()
                    && params.folder_id.is_none();
                if library_is_empty && unfiltered {
                    // Read both guards inside the worker: a scan can start, and a
                    // cover write can be enqueued, between this dispatch and the
                    // sweep itself, so a value captured here would guard the
                    // wrong instant. A decode is read-only, so it is not a guard.
                    let core = self.core.clone();
                    return Task::perform(
                        async move {
                            let scanning = core.is_scanning();
                            let cover_writes = crate::utils::covers::cover_writes_in_flight() > 0;
                            core.sweep_orphan_covers(scanning, cover_writes)
                        },
                        |result| match result {
                            Ok(report) => Message::CoverSweepFinished {
                                removed: report.removed,
                                failed: report.failed,
                            },
                            Err(_) => Message::NoOp,
                        },
                    );
                }

                Task::none()
            }
            Message::CoverDecoded(outcome) => {
                // A failed decode releases its key without storing a handle, so
                // asking for the window again would immediately re-enqueue the
                // same file and reopen it until its write lands. Only a kept
                // handle changes what the window draws; a failed key stays
                // retryable for the next scroll, refresh or post-scan cleanup.
                if self.library_manager.apply_cover_decoded(outcome) {
                    self.library_manager.request_cover_window();
                }

                Task::none()
            }
            Message::CoverSweepFinished { removed, failed } => {
                if removed + failed > 0 {
                    tracing::info!(
                        target: "audoxidy::metrics",
                        metric = "cover_sweep",
                        removed,
                        failed,
                        "cover sweep reported unreferenced files"
                    );
                }
                Task::none()
            }
            Message::CoverPrefetchDue => {
                self.library_manager.prefetch_scheduled = false;

                // A scroll that arrived while this timer was pending could not
                // schedule its own delayed message, so the request is rescheduled
                // instead of dropped: otherwise that gesture would never
                // prefetch at all.
                if let Some(last_scroll_at) = self.library_manager.last_scroll_at
                    && last_scroll_at.elapsed() < std::time::Duration::from_millis(SCROLL_IDLE_MS)
                {
                    self.library_manager.prefetch_scheduled = true;
                    return scroll_idle_task();
                }

                // Only the settled gesture looks ahead: one screen in the
                // direction the last scroll moved, half of one in low-resource
                // mode. The band is speculative work, so a scroll event itself
                // never requests it.
                let screens = if crate::utils::is_low_resource() {
                    0.5
                } else {
                    1.0
                };
                self.library_manager.request_cover_window_ahead(screens);
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

                let previous_y = self.library_manager.scroll_offset.y;
                let abs_offset = offset.absolute_offset();
                self.library_manager.scroll_offset = iced::Vector::new(abs_offset.x, abs_offset.y);
                self.library_manager.last_viewport = Some(iced::Rectangle {
                    x: offset.absolute_offset().x,
                    y: offset.absolute_offset().y,
                    width: offset.bounds().width,
                    height: offset.bounds().height,
                });

                // Si estamos arrastrando, actualizamos el punto final del marquee basado en la nueva posición de scroll
                if self.library_manager.is_dragging
                    && let Some(pos) = self.player_ui_state.mouse_pos
                {
                    let lib_x = (pos.x - LIBRARY_CONTENT_X_ORIGIN).max(0.0);
                    let lib_y = pos.y - 72.0 + offset.absolute_offset().y;
                    self.library_manager.marquee_end = Some(iced::Point::new(lib_x, lib_y));
                }

                // A geometry change (resize or column change) moves the content
                // under a fixed pixel offset; re-anchor before the normal cache
                // refresh so the visible albums keep their viewport position.
                if let Some(task) = self.apply_grid_anchor(abs_offset.y) {
                    return task;
                }

                // Direction describes the gesture, not the frame: the look-ahead
                // band reads the sign. A scroll event that did not move keeps the
                // previous direction, because the sign of a zero delta would
                // point the band the wrong way.
                let new_y = abs_offset.y;
                let delta = new_y - previous_y;
                if delta != 0.0 {
                    self.library_manager.scroll_direction = delta.signum();
                }
                self.library_manager.last_scroll_at = Some(std::time::Instant::now());

                self.library_manager.invalidate_visible_cache();
                self.library_manager.request_cover_window();

                // A wheel or trackpad burst must not request decodes: only the
                // first event of the gesture schedules the delayed look-ahead,
                // which fires once the scroll has actually stopped.
                if self.library_manager.prefetch_scheduled {
                    return Task::none();
                }

                self.library_manager.prefetch_scheduled = true;
                scroll_idle_task()
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

    /// Re-anchors the library scroll offset after a geometry change.
    ///
    /// The anchor is always the first album of the first visible row, with or
    /// without an expanded album: the expansion only contributes its height to
    /// the rows above the viewport. Applying the corrected offset in the same
    /// update that predicts the new geometry keeps the layout from rendering a
    /// frame with the old offset and the new row heights.
    pub(in crate::gui::app) fn apply_grid_anchor_to(
        &mut self,
        next: GridMetrics,
        offset_y: f32,
    ) -> Option<Task<Message>> {
        let previous = self.library_manager.anchored_metrics?;
        if previous.columns == next.columns
            && (previous.card_height - next.card_height).abs() <= 0.01
        {
            return None;
        }

        self.library_manager.anchored_metrics = Some(next);
        self.library_manager.grid_metrics.set(next);
        self.library_manager.invalidate_cache();

        let target = self.anchored_grid_offset(previous, next, offset_y)?;
        self.library_manager.scroll_offset = iced::Vector::new(0.0, target);
        Some(scroll_to(
            LIBRARY_SCROLL_ID.clone(),
            AbsoluteOffset { x: 0.0, y: target },
        ))
    }

    /// Fallback path for a geometry change observed from a scroll notification.
    ///
    /// The resize path predicts the geometry and applies the anchor before the
    /// layout, so it usually sees an unchanged generation and does nothing.
    fn apply_grid_anchor(&mut self, offset_y: f32) -> Option<Task<Message>> {
        if self.library_manager.view_mode != crate::gui::library::LibraryViewMode::Grid {
            return None;
        }
        let metrics = self.library_manager.grid_metrics.get();
        if self.library_manager.anchored_metrics.is_none() {
            self.library_manager.anchored_metrics = Some(metrics);
            return None;
        }
        self.apply_grid_anchor_to(metrics, offset_y)
    }

    fn anchored_grid_offset(
        &self,
        previous: GridMetrics,
        metrics: GridMetrics,
        offset_y: f32,
    ) -> Option<f32> {
        let viewport = self.library_manager.last_viewport?;
        let album_count = self
            .library_manager
            .filtered_albums
            .as_ref()
            .or(self.library_manager.cached_albums.as_ref())
            .map_or(0, Vec::len);
        let (expanded_index, rendered_songs) = match self.library_manager.expanded_album_anchor() {
            Some((index, songs)) => (Some(index), songs),
            None => (None, 0),
        };

        Some(anchor_viewport_offset(
            &previous,
            &metrics,
            offset_y,
            album_count,
            expanded_index,
            rendered_songs,
            viewport.height,
        ))
    }

    pub(in crate::gui::app) fn handle_library_reveal(
        &mut self,
        path_opt: Option<String>,
    ) -> Task<Message> {
        tracing::debug!(
            "Audoxidy Debug: handle_library_reveal path_opt: {:?}",
            path_opt
        );
        if let Some(path) = path_opt
            && let Some(all_songs) = &self.library_manager.cached_all_songs
            && let Some(lib_song) = all_songs
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
                        // The album row's hash is the identity the grid draws;
                        // resolving by title and artist would pick the first
                        // version and expand the wrong one.
                        let album_total = albums.len();
                        let album_hash = lib_song.album_hash.clone();
                        let album_index = albums.iter().position(|a| match album_hash.as_deref() {
                            Some(hash) => a.id == hash,
                            None => a.title == album_name && a.artist == artist_name,
                        });

                        if let Some(album_index) = album_index {
                            let album_artist = albums[album_index].artist.clone();
                            let hash_id = albums[album_index].id.clone();
                            let composite_id = format!("{}|{}", album_artist, hash_id);

                            if self.library_manager.expanded_album.as_deref() != Some(&composite_id)
                            {
                                self.library_manager.expanded_album = Some(composite_id.clone());
                                // Poblar expanded_album_songs inmediatamente para el cálculo de scroll
                                let mut songs = self.core.get_songs_by_album(&hash_id);
                                if self.library_manager.sort_column.is_some() {
                                    self.library_manager.sort_songs(&mut songs);
                                }
                                self.library_manager.expanded_album_songs = Some(songs);
                                // Expanding changes the rows around the album
                                // without a scroll, so the memoized rows must be
                                // rebuilt and the decode window must follow them.
                                self.library_manager.invalidate_cache();
                                self.library_manager.request_cover_window();
                            }

                            // The scroll hint is the expression the virtualizer
                            // uses for the song rectangle: the expanded row's top
                            // plus the card height, one expanded padding and the
                            // rendered song index. The rendered index counts only
                            // the songs the renderer keeps, so a filtered-out song
                            // never shifts the target.
                            let metrics = self.library_manager.grid_metrics.get();
                            let row = album_index / metrics.columns;
                            let row_top = crate::gui::library::grid_metrics::row_top_of(
                                &metrics,
                                album_total,
                                Some(row),
                                self.library_manager.rendered_expanded_song_count(),
                                row,
                            );

                            let mut reveal = None;
                            let mut rendered_index = 0;

                            if let Some(expanded) =
                                self.library_manager.expanded_album_songs.as_ref()
                            {
                                let query = self.library_manager.search_query.trim();

                                for (idx, song) in expanded.iter().enumerate() {
                                    if !query.is_empty()
                                        && !crate::utils::song_matches_search(song, query)
                                    {
                                        continue;
                                    }

                                    if song.full_file_path.as_ref() == path {
                                        reveal = Some((idx, rendered_index, song.id));
                                        break;
                                    }

                                    rendered_index += 1;
                                }
                            }

                            if let Some((song_index, rendered_index, song_id)) = reveal {
                                self.library_manager.selected_song_idx = Some(song_index);
                                self.library_manager.selected_album = Some(composite_id.clone());

                                let item = crate::gui::library::LibraryListItem::Song(song_id);
                                self.library_manager.selected_items.clear();
                                self.library_manager.selected_items.insert(item.clone());
                                self.library_manager.focused_item = Some(item.clone());
                                self.library_manager.selection_pivot = Some(item);

                                let song_y = crate::gui::library::grid_metrics::song_y_in_row(
                                    &metrics,
                                    row_top,
                                    rendered_index,
                                );
                                self.library_manager.selected_item_hint =
                                    Some((song_y, crate::gui::dimensions::LIST_ROW_HEADER));
                            }
                        }
                    }
                }
                _ => {
                    let artist_name = crate::utils::get_effective_artist(&lib_song).to_string();
                    self.library_manager.collapsed_artists.remove(&artist_name);

                    if self.library_manager.view_mode
                        == crate::gui::library::LibraryViewMode::DetailedList
                        && let Some(album_name) = lib_song.album.as_deref()
                    {
                        let albums_list = self
                            .library_manager
                            .filtered_albums
                            .as_ref()
                            .or(self.library_manager.cached_albums.as_ref());

                        if let Some(albums) = albums_list
                            && let Some(album_info) = albums
                                .iter()
                                .find(|a| a.title == album_name && a.artist == artist_name)
                        {
                            let composite_id = format!("{}|{}", artist_name, album_info.id);
                            self.library_manager.collapsed_albums.remove(&composite_id);
                        }
                    }

                    // Invalidar caché visual para que get_item_y_range pueda encontrar el ítem ahora que expandimos los nodos
                    self.library_manager.invalidate_cache();

                    if let Some(filtered) = &self.library_manager.filtered_songs
                        && let Some(pos) = filtered
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
                            && let Some((y, h)) = self.library_manager.get_item_y_range(&item)
                        {
                            self.library_manager.selected_item_hint = Some((y, h));
                        }
                    }
                }
            }

            self.update_selection_stats();
            // Forzar top_scroll o center scroll para que la canción siempre se vea al revelar
            // Se difiere un frame para permitir que la interfaz (Iced) construya los widgets expandidos primero
            return Task::perform(async {}, |_| Message::ExecutePendingScroll);
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
