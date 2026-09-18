//! Playlist message handlers extracted from the update dispatcher.

use crate::gui::app::persistence::log_persist;
use crate::gui::app::{ActiveDialog, AppFocus, ExportFormat, ExportMode, Message};
use crate::gui::playlist::PLAYLIST_TABS_SCROLL_ID;
use iced::Task;
use iced::widget::operation::{AbsoluteOffset, focus, scroll_to};

impl crate::gui::app::AudoxidyApp {
    pub(in crate::gui::app) fn handle_playlist(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::AddSongToPlaylist(song) => {
                let db_arc = self.database.clone();
                let active_id = self.playlist_manager.active_playlist_id;
                return Task::perform(
                    async move {
                        // db-lock: persistencia — el guardado se omite si el lock falla
                        if let Ok(db) = db_arc.lock() {
                            if let Err(e) = db.add_song_to_playlist(active_id, song.id) {
                                tracing::warn!(
                                    target: "audoxidy::persistence",
                                    playlist_id = active_id,
                                    song_id = song.id,
                                    error = %e,
                                    "No se pudo añadir la canción a la playlist"
                                );
                            }
                        } else {
                            tracing::warn!("db-lock: lock no disponible");
                        }
                        active_id
                    },
                    |id| Message::SwitchPlaylist(id, false),
                );
            }
            Message::PlayAlbum(songs) => {
                let db_arc = self.database.clone();
                let active_id = self.playlist_manager.active_playlist_id;
                let song_ids: Vec<i64> = songs.iter().map(|s| s.id).collect();
                return Task::perform(
                    async move {
                        // db-lock: persistencia — el guardado se omite si el lock falla
                        if let Ok(mut db) = db_arc.lock() {
                            if let Err(e) = db.add_songs_to_playlist(active_id, &song_ids) {
                                tracing::warn!(
                                    target: "audoxidy::persistence",
                                    playlist_id = active_id,
                                    count = song_ids.len(),
                                    error = %e,
                                    "No se pudieron añadir las canciones a la playlist"
                                );
                            }
                        } else {
                            tracing::warn!("db-lock: lock no disponible");
                        }
                        active_id
                    },
                    |id| Message::SwitchPlaylist(id, false),
                );
            }
            Message::PlaySongIndex(idx) => {
                self.focus = AppFocus::Playlist;
                let now = std::time::Instant::now();
                let is_double_click = if let Some((last_idx, last_time)) = self.last_playlist_click
                {
                    last_idx == idx && now.duration_since(last_time).as_millis() < 400
                } else {
                    false
                };

                self.last_playlist_click = Some((idx, now));
                self.playlist_manager.handle_click(idx, self.modifiers);

                let focus_task = focus(crate::gui::playlist::PLAYLIST_SCROLL_ID.clone());

                if is_double_click {
                    if let Some(song) = self.playlist_manager.get_song_at_linear_index(idx) {
                        // Reproduce la canción (o la reinicia si ya es la actual).
                        self.play_song_from_playlist(idx);
                    }
                }
                focus_task
            }
            Message::ClearPlaylist => {
                self.focus = AppFocus::Playlist;
                // db-lock: persistencia — el guardado se omite si el lock falla
                if let Ok(db) = self.database.lock() {
                    if let Err(e) = db.clear_playlist(self.playlist_manager.active_playlist_id) {
                        tracing::warn!(
                            target: "audoxidy::persistence",
                            playlist_id = self.playlist_manager.active_playlist_id,
                            error = %e,
                            "No se pudo vaciar la playlist"
                        );
                    }
                    self.playlist_manager.clear_groups();
                    self.playlist_manager.playing_song_idx = None;
                    self.audio_manager.stop();
                    self.persist_playlist_state();

                    return Task::done(Message::GlobalMemoryPurge);
                } else {
                    tracing::warn!("db-lock: lock no disponible");
                }
                Task::none()
            }
            Message::ToggleSongEnabled(linear_idx, force_state) => {
                self.focus = AppFocus::Playlist;
                let mut song_id_to_update = None;
                if let Some(song) = self.playlist_manager.get_song_at_linear_index(linear_idx) {
                    song_id_to_update = Some(song.song_id);
                }

                if let Some(state) = force_state {
                    self.playlist_manager
                        .set_song_enabled_at_linear_index(linear_idx, state);
                } else {
                    self.playlist_manager
                        .toggle_song_enabled_at_linear_index(linear_idx);
                }

                // db-lock: persistencia — el guardado se omite si el lock falla
                if let Ok(db) = self.database.lock() {
                    if let Some(song_id) = song_id_to_update {
                        if let Some(state) = force_state {
                            if let Err(e) = db.set_song_enabled_in_playlist(
                                self.playlist_manager.active_playlist_id,
                                song_id,
                                state,
                            ) {
                                tracing::warn!(
                                    target: "audoxidy::persistence",
                                    playlist_id = self.playlist_manager.active_playlist_id,
                                    song_id,
                                    error = %e,
                                    "No se pudo actualizar el estado de la canción en la playlist"
                                );
                            }
                        } else {
                            if let Err(e) = db.toggle_song_enabled_in_playlist(
                                self.playlist_manager.active_playlist_id,
                                song_id,
                            ) {
                                tracing::warn!(
                                    target: "audoxidy::persistence",
                                    playlist_id = self.playlist_manager.active_playlist_id,
                                    song_id,
                                    error = %e,
                                    "No se pudo alternar el estado de la canción en la playlist"
                                );
                            }
                        }
                    }
                } else {
                    tracing::warn!("db-lock: lock no disponible");
                }

                self.playlist_manager.invalidate_cache();
                focus(crate::gui::playlist::PLAYLIST_SCROLL_ID.clone())
            }
            Message::ToggleGroupEnabled(linear_idx, force_state) => {
                self.focus = AppFocus::Playlist;
                let mut folder_to_update = None;
                use crate::gui::playlist::PlaylistItemType;
                if let Some(PlaylistItemType::Separator(g_idx, _)) = self
                    .playlist_manager
                    .get_item_info_at_linear_index(linear_idx)
                {
                    if let Some(group) = self.playlist_manager.active_groups().get(g_idx) {
                        folder_to_update = Some(group.folder_path.clone());
                    }
                }

                if let Some(state) = force_state {
                    self.playlist_manager
                        .set_group_enabled_at_linear_index(linear_idx, state);
                } else {
                    self.playlist_manager
                        .toggle_group_enabled_at_linear_index(linear_idx);
                }

                if let Some(path) = folder_to_update {
                    // db-lock: persistencia — el guardado se omite si el lock falla
                    if let Ok(db) = self.database.lock() {
                        if let Some(state) = force_state {
                            if let Err(e) = db.set_folder_enabled_in_playlist(
                                self.playlist_manager.active_playlist_id,
                                &path,
                                state,
                            ) {
                                tracing::warn!(
                                    target: "audoxidy::persistence",
                                    playlist_id = self.playlist_manager.active_playlist_id,
                                    path = %path,
                                    error = %e,
                                    "No se pudo actualizar el estado del folder en la playlist"
                                );
                            }
                        } else {
                            if let Err(e) = db.toggle_folder_enabled_in_playlist(
                                self.playlist_manager.active_playlist_id,
                                &path,
                            ) {
                                tracing::warn!(
                                    target: "audoxidy::persistence",
                                    playlist_id = self.playlist_manager.active_playlist_id,
                                    path = %path,
                                    error = %e,
                                    "No se pudo alternar el estado del folder en la playlist"
                                );
                            }
                        }
                    } else {
                        tracing::warn!("db-lock: lock no disponible");
                    }
                }

                Task::none()
            }
            Message::TogglePlaylistFolder(linear_idx) => {
                self.focus = AppFocus::Playlist;
                let now = std::time::Instant::now();
                let is_double_click = if let Some((last_idx, last_time)) = self.last_playlist_click
                {
                    last_idx == linear_idx && now.duration_since(last_time).as_millis() < 400
                } else {
                    false
                };

                self.last_playlist_click = Some((linear_idx, now));
                self.playlist_manager
                    .handle_click(linear_idx, self.modifiers);

                if is_double_click {
                    use crate::gui::playlist::PlaylistItemType;
                    if let Some(PlaylistItemType::Separator(_, anchor)) = self
                        .playlist_manager
                        .get_item_info_at_linear_index(linear_idx)
                    {
                        self.playlist_manager.toggle_group_expansion(anchor);
                    }
                }
                Task::none()
            }
            Message::CreatePlaylist => {
                let name = format!("Nueva lista {}", self.playlist_manager.playlists.len() + 1);
                if let Ok(new_id) = self.database.lock().unwrap().create_playlist(&name, false) {
                    if let Ok(playlists) = self.database.lock().unwrap().get_all_playlists() {
                        self.playlist_manager.playlists = playlists;
                        return Task::done(Message::SwitchPlaylist(new_id, false));
                    }
                }
                Task::none()
            }
            Message::RefreshPlaylists(id_opt) => {
                let db = self.database.clone();
                return Task::perform(
                    async move {
                        let db_lock = db.lock().unwrap();
                        db_lock.get_all_playlists().ok()
                    },
                    move |playlists_opt| {
                        if let Some(playlists) = playlists_opt {
                            Message::InternalPlaylistsRefreshed(playlists, id_opt)
                        } else {
                            Message::NoOp
                        }
                    },
                );
            }
            Message::InternalPlaylistsRefreshed(playlists, id_opt) => {
                let active_id = self.playlist_manager.active_playlist_id;
                self.playlist_manager.playlists = playlists;

                if let Some(id) = id_opt {
                    if id > 0 {
                        // Es un rename o create con ID específico
                        return Task::done(Message::SwitchPlaylist(id, false));
                    } else if id == 0 {
                        // Es un delete, verificar si la lista activa murió
                        let still_exists = self
                            .playlist_manager
                            .playlists
                            .iter()
                            .any(|p| p.id == active_id);
                        if !still_exists {
                            if let Some(first) = self.playlist_manager.playlists.first() {
                                return Task::done(Message::SwitchPlaylist(first.id, false));
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::ExportPlaylist(id, target_path, format, mode) => {
                let db = self.database.clone();

                return Task::perform(
                    async move {
                        let sanitize_name = |s: &str| {
                            s.chars()
                                .map(|c| if "/\\?%*:|\"<>".contains(c) { '_' } else { c })
                                .collect::<String>()
                        };

                        // db-lock: solo lectura — refresco best-effort, se omite si el lock falla
                        let songs = if let Ok(db_lock) = db.lock() {
                            db_lock.search_playlist_songs(id, "").unwrap_or_default()
                        } else {
                            vec![]
                        };

                        // db-lock: solo lectura — refresco best-effort, se omite si el lock falla
                        let p_name = if let Ok(db_lock) = db.lock() {
                            db_lock
                                .get_playlist_by_id(id)
                                .ok()
                                .flatten()
                                .map(|p| p.name)
                                .unwrap_or_else(|| "Playlist_Exportada".to_string().into())
                        } else {
                            "Playlist_Exportada".into()
                        };

                        let is_portable = mode == ExportMode::PortableFolder;
                        let mut m3u_content = String::from("#EXTM3U\n");
                        let base_target_path = std::path::PathBuf::from(target_path);

                        for song in songs {
                            let path_to_write = if is_portable {
                                let artist_dir = sanitize_name(&song.artist_name);
                                let album_dir = sanitize_name(&song.album_title);
                                let song_file = std::path::Path::new(song.file_path.as_ref())
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy();

                                let relative_path =
                                    format!("{}/{}/{}", artist_dir, album_dir, song_file);
                                let full_target_dir =
                                    base_target_path.join(&artist_dir).join(&album_dir);
                                let full_target_path = full_target_dir.join(song_file.as_ref());

                                if let Err(e) = std::fs::create_dir_all(&full_target_dir) {
                                    tracing::warn!(
                                        "No se pudo crear el directorio {:?}: {}",
                                        full_target_dir,
                                        e
                                    );
                                }
                                if let Err(e) = std::fs::copy(&*song.file_path, &full_target_path) {
                                    tracing::warn!(
                                        "No se pudo copiar {:?} a {:?}: {}",
                                        song.file_path,
                                        full_target_path,
                                        e
                                    );
                                }
                                relative_path
                            } else {
                                song.file_path.to_string()
                            };

                            m3u_content.push_str(&format!(
                                "#EXTINF:{},{}\n{}\n",
                                song.duration as i32, song.title, path_to_write
                            ));
                        }

                        // Escribir el archivo
                        let final_m3u_path = if is_portable {
                            let ext = match format {
                                ExportFormat::M3U => "m3u",
                                ExportFormat::M3U8 => "m3u8",
                            };
                            base_target_path.join(format!("{}.{}", sanitize_name(&p_name), ext))
                        } else {
                            base_target_path
                        };
                        if let Err(e) = std::fs::write(&final_m3u_path, m3u_content) {
                            tracing::warn!(
                                "No se pudo escribir la lista M3U en {:?}: {}",
                                final_m3u_path,
                                e
                            );
                        }
                    },
                    |_| Message::NoOp,
                );
            }
            Message::SwitchPlaylist(id, auto_play) => {
                self.focus = AppFocus::Playlist;
                self.persist_playlist_state();

                // db-lock: persistencia — el guardado se omite si el lock falla
                if let Ok(db) = self.database.lock() {
                    log_persist(
                        "last_active_playlist_id",
                        db.set_setting("last_active_playlist_id", &id.to_string()),
                    );
                } else {
                    tracing::warn!("db-lock: lock no disponible");
                }

                self.playlist_manager.active_playlist_id = id;
                let db_arc = self.database.clone();

                return Task::perform(
                    async move {
                        let db = db_arc.lock().unwrap();
                        let groups = db.get_playlist_songs_grouped_by_folder(id).ok();
                        let p_data = db.get_playlist_by_id(id).ok().flatten();
                        let mut session_opt = None;

                        if let Some(p) = &p_data {
                            if p.shuffle_active {
                                if let Ok(Some(mut session)) = db.load_shuffle_session(id) {
                                    if let Some(song_id) = p.last_song_id {
                                        if let Some(pos) = session
                                            .shuffle_order
                                            .iter()
                                            .position(|&sid| sid == song_id)
                                        {
                                            session.current_position = pos + 1;
                                            if !session.history.contains(&song_id) {
                                                session.history.push(song_id);
                                            }
                                        }
                                    }
                                    session_opt = Some(session);
                                }
                            }
                        }
                        (id, auto_play, groups, p_data, session_opt)
                    },
                    |(id, auto_play, groups, p_data, session)| {
                        Message::InternalPlaylistLoaded(id, auto_play, groups, p_data, session)
                    },
                );
            }
            Message::InternalPlaylistLoaded(id, auto_play, groups, p_data_opt, session_opt) => {
                if id != self.playlist_manager.active_playlist_id {
                    return Task::none(); // El usuario cambió a otra lista mientras cargaba
                }

                if let Some(groups) = groups {
                    self.playlist_manager.set_groups(groups);
                }

                let mut was_restored = false;
                if let Some(p_data) = p_data_opt {
                    self.playlist_manager.shuffle_active = p_data.shuffle_active;
                    self.playlist_manager.repeat_mode = p_data.repeat_mode as u8;
                    self.playlist_manager.shuffle_session = session_opt;

                    // Determinar qué canción reproducir/restaurar
                    let song_id_to_play = if auto_play {
                        self.playlist_manager
                            .get_first_enabled_song_idx()
                            .and_then(|idx| self.playlist_manager.get_song_at_linear_index(idx))
                            .map(|s| s.song_id)
                    } else {
                        p_data.last_song_id
                    };

                    if let Some(song_id) = song_id_to_play {
                        if let Some(l_idx) =
                            self.playlist_manager.get_linear_index_by_song_id(song_id)
                        {
                            if let Some(song) =
                                self.playlist_manager.get_song_at_linear_index(l_idx)
                            {
                                let path = song.file_path.clone();
                                let title = song.title.clone();
                                let artist = song.artist_name.to_string();
                                let album = song.album_title.to_string();
                                let cover_path = song.cover_path.as_deref().map(str::to_string);

                                let is_playing_needed =
                                    if auto_play { true } else { p_data.is_playing };
                                let last_pos = if auto_play { 0.0 } else { p_data.last_pos_sec };

                                self.playlist_manager.playing_song_idx = Some(l_idx);
                                self.playlist_manager.focused_idx = Some(l_idx);
                                self.playlist_manager.selected_idxs.clear();
                                self.playlist_manager.selected_idxs.insert(l_idx);
                                self.playlist_manager.selection_pivot = Some(l_idx);

                                let _ = self.load_file_with_gains(
                                    &path,
                                    title.to_string(),
                                    artist.to_string(),
                                    album,
                                    cover_path,
                                );
                                self.audio_manager.seek(last_pos);
                                self.audio_manager.set_playing(is_playing_needed);
                                self.sync_player_art();
                                was_restored = true;
                            }
                        }
                    }
                }

                self.playlist_manager.show_tab_dropdown = false;
                // Mantenemos la selección y foco si se restauró arriba, si no, limpiamos
                if !was_restored {
                    self.playlist_manager.selected_idxs.clear();
                    self.playlist_manager.focused_idx = None;
                    self.playlist_manager.selection_pivot = None;
                }
                self.playlist_manager.apply_filter();

                // --- Autoscroll de pestaña activa ---
                let playlists = &self.playlist_manager.playlists;
                let mut tasks = Vec::new();

                if let Some(pos_idx) = playlists.iter().position(|p| p.id == id) {
                    let mut used_x = 0.0;
                    for p in playlists.iter().take(pos_idx) {
                        used_x += crate::gui::playlist::estimate_tab_width(&p.name) + 5.0;
                    }

                    let tab_width =
                        crate::gui::playlist::estimate_tab_width(&playlists[pos_idx].name);

                    let mut needs_tab_scroll = true;
                    if let Some(vp) = self.playlist_manager.tabs_viewport {
                        let left_visible = used_x >= (vp.x - 2.0);
                        let right_visible = (used_x + tab_width) <= (vp.x + vp.width + 2.0);
                        if left_visible && right_visible {
                            needs_tab_scroll = false;
                        }
                    }

                    if needs_tab_scroll {
                        let target_x = (used_x - 15.0).max(0.0);
                        tasks.push(scroll_to(
                            PLAYLIST_TABS_SCROLL_ID.clone(),
                            AbsoluteOffset {
                                x: target_x,
                                y: 0.0,
                            },
                        ));
                    }
                }

                // --- Autoscroll de canción (reproduciendo o pausada) ---
                if let Some(idx) = self.playlist_manager.playing_song_idx {
                    tasks.push(self.execute_playlist_autoscroll(idx));
                } else {
                    // Si no hay canción reproduciendo/pausada, ir al inicio de la lista
                    tasks.push(scroll_to(
                        crate::gui::playlist::PLAYLIST_SCROLL_ID.clone(),
                        AbsoluteOffset { x: 0.0, y: 0.0 },
                    ));
                }

                if tasks.is_empty() {
                    Task::none()
                } else {
                    Task::batch(tasks)
                }
            }
            Message::PlaylistShowAllTabs(pos) => {
                use crate::gui::widgets::ContextMenuEntry;
                let mut entries = vec![
                    ContextMenuEntry {
                        label: "Nueva lista".to_string(),
                        icon: Some("playlist-add-straight.svg".to_string()),
                        action: Some(Message::OpenDialog(
                            crate::gui::app::ActiveDialog::CreatePlaylist {
                                name: "".into(),
                                pending_add_songs: Vec::new(),
                            },
                        )),
                    },
                    ContextMenuEntry {
                        label: "".to_string(),
                        icon: None,
                        action: None,
                    },
                ];

                let (mut system, others): (Vec<_>, Vec<_>) = self
                    .playlist_manager
                    .playlists
                    .iter()
                    .partition(|p| p.is_system);

                // Ordenamos sistema brevemente para asegurar Audoxidy -> Oxidy Drift
                system.sort_by_key(|p| {
                    if p.name == "Audoxidy" {
                        0
                    } else if p.name == "Oxidy Drift" {
                        1
                    } else {
                        2
                    }
                });

                let prioritized = system.into_iter().chain(others.into_iter());

                for pl in prioritized {
                    entries.push(ContextMenuEntry {
                        label: pl.name.clone(),
                        icon: Some("playlist-music.svg".to_string()),
                        action: Some(Message::SwitchPlaylist(pl.id, false)),
                    });
                }

                let wrapped = self.wrap_context_menu(entries);
                self.context_menu = Some((pos, wrapped));
                Task::none()
            }
            Message::PlaylistTabsScrolled(viewport) => {
                self.playlist_manager.tabs_viewport = Some(iced::Rectangle {
                    x: viewport.absolute_offset().x,
                    y: viewport.absolute_offset().y,
                    width: viewport.bounds().width,
                    height: viewport.bounds().height,
                });
                Task::none()
            }
            Message::ToggleTabDropdown => {
                self.playlist_manager.show_tab_dropdown = !self.playlist_manager.show_tab_dropdown;
                if self.playlist_manager.show_tab_dropdown {
                    // Posición fija relativa a la lista: a 10px del borde derecho.
                    // Ancho del panel (400) - Ancho Menú (180) - Margen derecho demandado (10) = 210.0
                    let fixed_pos = iced::Point { x: 220.0, y: 440.0 };
                    return Task::done(Message::PlaylistShowAllTabs(fixed_pos));
                }
                Task::none()
            }
            Message::PlaylistSearchChanged(q) => {
                self.focus = AppFocus::Playlist;
                self.playlist_manager.search_query = q;
                self.playlist_manager.apply_filter();
                scroll_to(
                    crate::gui::playlist::PLAYLIST_SCROLL_ID.clone(),
                    AbsoluteOffset { x: 0.0, y: 0.0 },
                )
            }
            Message::PlaylistFocus => {
                self.focus = AppFocus::Playlist;
                crate::utils::memory_manager::MemoryManager::register_playlist_activity();
                if self.playlist_manager.data_unloaded {
                    self.playlist_manager.data_unloaded = false;
                    let id = self.playlist_manager.active_playlist_id;
                    return self.update(Message::SwitchPlaylist(id, false));
                }
                focus(crate::gui::playlist::PLAYLIST_SCROLL_ID.clone())
            }
            Message::PlaylistDeselect => {
                self.focus = AppFocus::Playlist;
                self.playlist_manager.selected_idxs.clear();
                self.playlist_manager.focused_idx = None;
                self.playlist_manager.selection_pivot = None;
                focus(crate::gui::playlist::PLAYLIST_SCROLL_ID.clone())
            }
            Message::ToggleLyrics => {
                // Abrir módulo de letras
                Task::none()
            }
            Message::PlaylistContextMenuPlay(linear_idx) => {
                self.focus = AppFocus::Playlist;
                use crate::gui::playlist::PlaylistItemType;
                if let Some(item) = self
                    .playlist_manager
                    .get_item_info_at_linear_index(linear_idx)
                {
                    let target_song_idx = match item {
                        PlaylistItemType::Song(..) => Some(linear_idx),
                        PlaylistItemType::Separator(..) => {
                            let groups = self.playlist_manager.active_groups();
                            let mut current_idx = 0;
                            let mut found_idx = None;
                            for group in groups {
                                if current_idx == linear_idx {
                                    for (i, song) in group.songs.iter().enumerate() {
                                        if song.enabled {
                                            found_idx = Some(linear_idx + 1 + i);
                                            break;
                                        }
                                    }
                                    break;
                                }
                                current_idx += 1 + group.songs.len();
                            }
                            found_idx
                        }
                    };

                    if let Some(idx) = target_song_idx {
                        if let Some(song) = self.playlist_manager.get_song_at_linear_index(idx) {
                            let path = song.file_path.clone();
                            let title = song.title.clone();
                            let artist = song.artist_name.to_string();
                            let album = song.album_title.to_string();
                            let cover_path = song.cover_path.as_deref().map(str::to_string);

                            self.sync_player_art();
                            if let Err(e) = self.load_file_with_gains(
                                &path,
                                title.to_string(),
                                artist,
                                album,
                                cover_path,
                            ) {
                                tracing::error!("Error reproduciendo archivo: {}", e);
                            } else {
                                self.audio_manager.play();
                                self.playlist_manager.playing_song_idx = Some(idx);
                                self.playlist_manager.selected_idxs.clear();
                                self.playlist_manager.selected_idxs.insert(idx);
                                self.playlist_manager.focused_idx = Some(idx);
                                self.playlist_manager.selection_pivot = Some(idx);
                                self.persist_playlist_state();
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::PlaylistAddFiles => {
                self.focus = AppFocus::Playlist;
                Task::perform(
                    async move {
                        rfd::FileDialog::new()
                            .add_filter(
                                "Audio",
                                &[
                                    "mp3", "flac", "wav", "ogg", "m4a", "aac", "ape", "aiff",
                                    "mpc", "opus", "spx", "wv",
                                ],
                            )
                            .add_filter("Listas", &["m3u", "m3u8"])
                            .pick_files()
                    },
                    |files| {
                        if let Some(f) = files {
                            Message::PlaylistProcessExternalFiles(f)
                        } else {
                            Message::NoOp
                        }
                    },
                )
            }
            Message::PlaylistAddFolder => {
                self.focus = AppFocus::Playlist;
                Task::perform(
                    async move { rfd::FileDialog::new().pick_folder() },
                    |folder| {
                        if let Some(f) = folder {
                            Message::PlaylistProcessExternalFiles(vec![f])
                        } else {
                            Message::NoOp
                        }
                    },
                )
            }
            Message::PlaylistProcessExternalFiles(paths) => {
                let db_arc = self.database.clone();
                let active_id = self.playlist_manager.active_playlist_id;
                Task::perform(
                    async move {
                        crate::db::scanner::Scanner::process_external_batch(
                            &db_arc, paths, active_id,
                        );
                    },
                    move |_| Message::RefreshPlaylists(Some(active_id)),
                )
            }
            Message::PlaylistShowFileLocation(linear_idx) => {
                self.focus = AppFocus::Playlist;
                if let Some(song) = self.playlist_manager.get_song_at_linear_index(linear_idx) {
                    let path_str = song.file_path.clone();
                    Task::perform(
                        async move {
                            let path = std::path::Path::new(path_str.as_ref());
                            if let Some(parent) = path.parent() {
                                #[cfg(target_os = "linux")]
                                {
                                    let _ =
                                        std::process::Command::new("xdg-open").arg(parent).spawn();
                                }
                                #[cfg(target_os = "windows")]
                                {
                                    let _ = std::process::Command::new("explorer")
                                        .arg(format!("/select,\"{}\"", path_str))
                                        .spawn();
                                }
                                #[cfg(target_os = "macos")]
                                {
                                    let _ = std::process::Command::new("open")
                                        .arg("-R")
                                        .arg(&path_str)
                                        .spawn();
                                }
                            }
                        },
                        |_| Message::NoOp,
                    )
                } else {
                    Task::none()
                }
            }
            Message::PlaylistShowInLibrary(linear_idx) => {
                let mut path_opt = None;
                if let Some(item_info) = self
                    .playlist_manager
                    .get_item_info_at_linear_index(linear_idx)
                {
                    match item_info {
                        crate::gui::playlist::PlaylistItemType::Song(
                            _folder,
                            local_idx,
                            anchor,
                        ) => {
                            if let Some(group) = self
                                .playlist_manager
                                .active_groups()
                                .iter()
                                .find(|g| g.first_item_id == anchor)
                            {
                                if let Some(song) = group.songs.get(local_idx) {
                                    path_opt = Some(song.file_path.clone());
                                }
                            }
                        }
                        crate::gui::playlist::PlaylistItemType::Separator(_folder, anchor) => {
                            if let Some(group) = self
                                .playlist_manager
                                .active_groups()
                                .iter()
                                .find(|g| g.first_item_id == anchor)
                            {
                                if let Some(first_song) = group.songs.first() {
                                    path_opt = Some(first_song.file_path.clone());
                                }
                            }
                        }
                    }
                }
                tracing::debug!(
                    "Audoxidy Debug: PlaylistShowInLibrary path_opt: {:?}",
                    path_opt
                );
                self.handle_library_reveal(path_opt.map(|s| s.to_string()))
            }
            Message::PlaylistDeleteSelection(linear_idx) => {
                self.focus = AppFocus::Playlist;
                let db_arc = self.database.clone();
                let active_playlist_id = self.playlist_manager.active_playlist_id;

                // Si el índice está en la selección actual, eliminamos todo lo seleccionado
                let target_songs = if self.playlist_manager.selected_idxs.contains(&linear_idx) {
                    self.playlist_manager.get_selected_song_ids()
                } else {
                    // Si no, eliminamos solo este elemento (ya sea canción o carpeta completa)
                    let mut ids = Vec::new();
                    if let Some(item_info) = self
                        .playlist_manager
                        .get_item_info_at_linear_index(linear_idx)
                    {
                        match item_info {
                            crate::gui::playlist::PlaylistItemType::Song(g_idx, s_idx, _) => {
                                if let Some(group) =
                                    self.playlist_manager.active_groups().get(g_idx)
                                {
                                    if let Some(song_ref) = group.songs.get(s_idx) {
                                        ids.push(song_ref.song_id);
                                    }
                                }
                            }
                            crate::gui::playlist::PlaylistItemType::Separator(_, anchor) => {
                                if let Some(group) = self
                                    .playlist_manager
                                    .groups
                                    .iter()
                                    .find(|g| g.first_item_id == anchor)
                                {
                                    ids.extend(group.songs.iter().map(|s| s.song_id));
                                }
                            }
                        }
                    }
                    ids
                };

                if !target_songs.is_empty() {
                    return Task::batch(vec![
                        Task::perform(
                            async move {
                                // db-lock: persistencia — el guardado se omite si el lock falla
                                if let Ok(mut db) = db_arc.lock() {
                                    if let Err(e) = db.batch_remove_songs_from_playlist(
                                        active_playlist_id,
                                        &target_songs,
                                    ) {
                                        tracing::warn!(
                                            target: "audoxidy::persistence",
                                            playlist_id = active_playlist_id,
                                            count = target_songs.len(),
                                            error = %e,
                                            "No se pudieron quitar las canciones de la playlist"
                                        );
                                    }
                                } else {
                                    tracing::warn!("db-lock: lock no disponible");
                                }
                                Some(active_playlist_id)
                            },
                            Message::RefreshPlaylists,
                        ),
                        focus(crate::gui::playlist::PLAYLIST_SCROLL_ID.clone()),
                        // Purga inmediata de memoria tras eliminación masiva
                        Task::done(Message::GlobalMemoryPurge),
                    ]);
                }
                focus(crate::gui::playlist::PLAYLIST_SCROLL_ID.clone())
            }
            Message::PlaylistRequestSubMenu(linear_idx) => {
                let pos = self.last_mouse_pos;
                use crate::gui::widgets::ContextMenuEntry;
                let mut entries = vec![
                    ContextMenuEntry {
                        label: "Atrás".to_string(),
                        icon: Some("double-arrow-left.svg".to_string()),
                        action: Some(Message::RequestContextMenu(
                            crate::gui::playlist::get_item_context_menu_entries(linear_idx),
                        )),
                    },
                    ContextMenuEntry {
                        label: "".to_string(),
                        icon: None,
                        action: None,
                    },
                    ContextMenuEntry {
                        label: "Nueva lista".to_string(),
                        icon: Some("playlist-add-straight.svg".to_string()),
                        action: Some(Message::PlaylistSendToNewList(linear_idx)),
                    },
                    ContextMenuEntry {
                        label: "".to_string(),
                        icon: None,
                        action: None,
                    },
                ];

                let (mut system, mut others): (Vec<_>, Vec<_>) = self
                    .playlist_manager
                    .playlists
                    .clone()
                    .into_iter()
                    .partition(|p| p.is_system);

                // Priorizar Audoxidy -> Oxidy Drift
                system.sort_by_key(|p| {
                    if p.name == "Audoxidy" {
                        0
                    } else if p.name == "Oxidy Drift" {
                        1
                    } else {
                        2
                    }
                });

                // Ordenar las de usuario alfabéticamente (case-insensitive)
                others.sort_by(|a, b| crate::utils::compare_strings_ignore_case(&a.name, &b.name));

                let prioritized = system.into_iter().chain(others.into_iter());

                for pl in prioritized {
                    entries.push(ContextMenuEntry {
                        label: pl.name.clone(),
                        icon: Some("playlist-music.svg".to_string()),
                        action: Some(Message::PlaylistSendToList(linear_idx, pl.id)),
                    });
                }

                self.update(Message::OpenContextMenu(pos, entries))
            }
            Message::PlaylistSendToNewList(linear_idx) => {
                self.focus = AppFocus::Playlist;
                self.context_menu = None;
                let target_songs = if self.playlist_manager.selected_idxs.contains(&linear_idx) {
                    self.playlist_manager.get_selected_song_ids()
                } else {
                    let mut ids = Vec::new();
                    if let Some(item_info) = self
                        .playlist_manager
                        .get_item_info_at_linear_index(linear_idx)
                    {
                        match item_info {
                            crate::gui::playlist::PlaylistItemType::Song(
                                _folder,
                                local_idx,
                                anchor,
                            ) => {
                                if let Some(group) = self
                                    .playlist_manager
                                    .groups
                                    .iter()
                                    .find(|g| g.first_item_id == anchor)
                                {
                                    if let Some(song_ref) = group.songs.get(local_idx) {
                                        ids.push(song_ref.song_id);
                                    }
                                }
                            }
                            crate::gui::playlist::PlaylistItemType::Separator(_folder, anchor) => {
                                if let Some(group) = self
                                    .playlist_manager
                                    .groups
                                    .iter()
                                    .find(|g| g.first_item_id == anchor)
                                {
                                    ids.extend(group.songs.iter().map(|s| s.song_id));
                                }
                            }
                        }
                    }
                    ids
                };
                self.update(Message::OpenDialog(ActiveDialog::CreatePlaylist {
                    name: "".to_string(),
                    pending_add_songs: target_songs,
                }))
            }
            Message::PlaylistSendToList(linear_idx, target_playlist_id) => {
                self.focus = AppFocus::Playlist;
                self.context_menu = None;
                let db_arc = self.database.clone();

                let target_songs = if self.playlist_manager.selected_idxs.contains(&linear_idx) {
                    self.playlist_manager.get_selected_song_ids()
                } else {
                    let mut ids = Vec::new();
                    if let Some(item_info) = self
                        .playlist_manager
                        .get_item_info_at_linear_index(linear_idx)
                    {
                        match item_info {
                            crate::gui::playlist::PlaylistItemType::Song(
                                _folder,
                                local_idx,
                                anchor,
                            ) => {
                                if let Some(group) = self
                                    .playlist_manager
                                    .groups
                                    .iter()
                                    .find(|g| g.first_item_id == anchor)
                                {
                                    if let Some(song_ref) = group.songs.get(local_idx) {
                                        ids.push(song_ref.song_id);
                                    }
                                }
                            }
                            crate::gui::playlist::PlaylistItemType::Separator(_folder, anchor) => {
                                if let Some(group) = self
                                    .playlist_manager
                                    .groups
                                    .iter()
                                    .find(|g| g.first_item_id == anchor)
                                {
                                    ids.extend(group.songs.iter().map(|s| s.song_id));
                                }
                            }
                        }
                    }
                    ids
                };

                if !target_songs.is_empty() {
                    return Task::perform(
                        async move {
                            // db-lock: persistencia — el guardado se omite si el lock falla
                            if let Ok(mut db) = db_arc.lock() {
                                if let Err(e) =
                                    db.add_songs_to_playlist(target_playlist_id, &target_songs)
                                {
                                    tracing::warn!(
                                        target: "audoxidy::persistence",
                                        playlist_id = target_playlist_id,
                                        count = target_songs.len(),
                                        error = %e,
                                        "No se pudieron añadir las canciones a la playlist"
                                    );
                                }
                            } else {
                                tracing::warn!("db-lock: lock no disponible");
                            }
                        },
                        |_| Message::NoOp,
                    );
                }
                Task::none()
            }
            Message::PlaylistToggleAllFolders => {
                self.focus = AppFocus::Playlist;
                let any_expanded = self.playlist_manager.groups.iter().any(|g| {
                    !self
                        .playlist_manager
                        .collapsed_groups
                        .contains(&g.first_item_id)
                });

                if any_expanded {
                    // Plegar TODO
                    for g in &self.playlist_manager.groups {
                        self.playlist_manager
                            .collapsed_groups
                            .insert(g.first_item_id);
                    }
                } else {
                    // Desplegar TODO
                    self.playlist_manager.collapsed_groups.clear();
                }
                self.playlist_manager.invalidate_cache();
                Task::none()
            }
            Message::PlaylistToggleEnabled(linear_idx) => {
                self.focus = AppFocus::Playlist;
                self.context_menu = None;
                use crate::gui::playlist::PlaylistItemType;

                let targets = if self.playlist_manager.selected_idxs.contains(&linear_idx) {
                    self.playlist_manager
                        .selected_idxs
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                } else {
                    vec![linear_idx]
                };

                // Determinar el estado objetivo: si hay ALGO desactivado, activamos todo.
                // Si todo está ya activado, entonces desactivamos todo.
                let mut any_disabled = false;
                for &idx in &targets {
                    if let Some(info) = self.playlist_manager.get_item_info_at_linear_index(idx) {
                        match info {
                            PlaylistItemType::Song(..) => {
                                if let Some(song) =
                                    self.playlist_manager.get_song_at_linear_index(idx)
                                {
                                    if !song.enabled {
                                        any_disabled = true;
                                        break;
                                    }
                                }
                            }
                            PlaylistItemType::Separator(_folder, anchor) => {
                                if let Some(group) = self
                                    .playlist_manager
                                    .groups
                                    .iter()
                                    .find(|g| g.first_item_id == anchor)
                                {
                                    if group.songs.iter().any(|s| !s.enabled) {
                                        any_disabled = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                let target_enabled = any_disabled;

                let mut tasks = Vec::new();
                for idx in targets {
                    if let Some(info) = self.playlist_manager.get_item_info_at_linear_index(idx) {
                        match info {
                            PlaylistItemType::Song(..) => tasks.push(
                                self.update(Message::ToggleSongEnabled(idx, Some(target_enabled))),
                            ),
                            PlaylistItemType::Separator(..) => tasks.push(
                                self.update(Message::ToggleGroupEnabled(idx, Some(target_enabled))),
                            ),
                        }
                    }
                }
                Task::batch(vec![
                    Task::batch(tasks),
                    focus(crate::gui::playlist::PLAYLIST_SCROLL_ID.clone()),
                ])
            }
            Message::PlaylistScrolled(viewport) => {
                self.playlist_manager.last_viewport = Some(viewport.bounds());
                self.playlist_manager.last_viewport = Some(iced::Rectangle {
                    x: viewport.absolute_offset().x,
                    y: viewport.absolute_offset().y,
                    width: viewport.bounds().width,
                    height: viewport.bounds().height,
                });
                Task::none()
            }
            Message::PlaylistMouseOver(is_over) => {
                self.is_mouse_over_playlist = is_over;
                Task::none()
            }
            _ => Task::none(),
        }
    }

    pub(in crate::gui::app) fn execute_playlist_autoscroll(
        &self,
        target_idx: usize,
    ) -> Task<Message> {
        if let Some(viewport) = &self.playlist_manager.last_viewport {
            let (item_top, item_bottom) = self.playlist_manager.get_item_y_bounds(target_idx);

            let view_min = viewport.y;
            let view_max = view_min + viewport.height;

            if item_top < view_min {
                // Scroll hacia arriba para mostrar el tope del ítem
                return iced::widget::operation::scroll_to(
                    crate::gui::playlist::PLAYLIST_SCROLL_ID.clone(),
                    iced::widget::scrollable::AbsoluteOffset {
                        x: 0.0,
                        y: item_top,
                    },
                );
            } else if item_bottom > view_max {
                // Scroll hacia abajo para mostrar el fondo del ítem al límite inferior del viewport
                let offset = item_bottom - viewport.height;
                return iced::widget::operation::scroll_to(
                    crate::gui::playlist::PLAYLIST_SCROLL_ID.clone(),
                    iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: offset },
                );
            }
        }
        Task::none()
    }

    pub(in crate::gui::app) fn persist_playlist_state(&self) {
        let state = self.audio_manager.get_state();
        let playlist_id = self.playlist_manager.active_playlist_id;
        let last_song_id = self
            .playlist_manager
            .playing_song_idx
            .and_then(|idx| self.playlist_manager.get_song_at_linear_index(idx))
            .map(|s| s.song_id);
        let pos = state.current_pos_sec;
        let is_p = state.is_playing;
        let shuffle = self.playlist_manager.shuffle_active;
        let repeat = self.playlist_manager.repeat_mode as i32;
        let (s_pos, s_id) = if let Some(session) = &self.playlist_manager.shuffle_session {
            (session.current_position, Some(session.session_id.clone()))
        } else {
            (0, None)
        };

        // db-lock: persistencia — el guardado se omite si el lock falla
        if let Ok(db) = self.database.lock() {
            if let Err(e) = db.update_playlist_persistence(
                playlist_id,
                last_song_id,
                pos,
                is_p,
                shuffle,
                repeat,
                s_pos,
                s_id,
            ) {
                tracing::warn!(
                    target: "audoxidy::persistence",
                    playlist_id,
                    error = %e,
                    "No se pudo guardar el estado de reproducción de la playlist"
                );
            }
        } else {
            tracing::warn!("db-lock: lock no disponible");
        }
    }
}
