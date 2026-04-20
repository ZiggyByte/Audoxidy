use iced::{Element, Task, Theme, Color};
use std::sync::{Arc, Mutex};
use crate::audio::AudioManager;
use crate::db::{Database, scanner::Scanner};
use crate::gui::playlist::PlaylistManager;
use crate::gui::library_filters::LibraryFiltersManager;
use crate::gui::library::{LibraryManager, LIBRARY_SCROLL_ID};
use iced::widget::operation::{scroll_to, AbsoluteOffset};
use crate::gui::audio_center::{AudioCenterManager, AudioCenterMessage};
use crate::integrations::media_controls::SystemMediaControls;
use crate::gui::theme::{
    COLOR_ACCENT, COLOR_CONTRAST, COLOR_TEXT_PRIMARY, COLOR_TEXT_SECONDARY,
};

pub mod helpers {
    use iced::advanced::{Widget, layout, mouse, Clipboard, Shell, Layout};
    use iced::advanced::widget::{Tree, Operation};
    use iced::{Element, Length, Rectangle, Size, Event};

    pub struct CursorOff<'a, Message, Theme, Renderer> {
        content: Element<'a, Message, Theme, Renderer>,
    }

    impl<'a, Message, Theme, Renderer> CursorOff<'a, Message, Theme, Renderer> {
        pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
            Self { content: content.into() }
        }
    }

    impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer> for CursorOff<'a, Message, Theme, Renderer>
    where
        Renderer: iced::advanced::Renderer,
    {
        fn size(&self) -> Size<Length> {
            self.content.as_widget().size()
        }

        fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
            self.content.as_widget_mut().layout(tree, renderer, limits)
        }

        fn draw(
            &self,
            tree: &Tree,
            renderer: &mut Renderer,
            theme: &Theme,
            style: &iced::advanced::renderer::Style,
            layout: Layout<'_>,
            cursor: mouse::Cursor,
            viewport: &Rectangle,
        ) {
            self.content.as_widget().draw(tree, renderer, theme, style, layout, cursor, viewport)
        }

        fn tag(&self) -> iced::advanced::widget::tree::Tag {
            self.content.as_widget().tag()
        }

        fn state(&self) -> iced::advanced::widget::tree::State {
            self.content.as_widget().state()
        }

        fn children(&self) -> Vec<Tree> {
            self.content.as_widget().children()
        }

        fn diff(&self, tree: &mut Tree) {
            self.content.as_widget().diff(tree)
        }

        fn operate(
            &mut self,
            tree: &mut Tree,
            layout: Layout<'_>,
            renderer: &Renderer,
            operation: &mut dyn Operation,
        ) {
            self.content.as_widget_mut().operate(tree, layout, renderer, operation);
        }

        fn update(
            &mut self,
            state: &mut Tree,
            event: &Event,
            layout: Layout<'_>,
            cursor: mouse::Cursor,
            renderer: &Renderer,
            clipboard: &mut dyn Clipboard,
            shell: &mut Shell<'_, Message>,
            viewport: &Rectangle,
        ) {
            self.content.as_widget_mut().update(
                state, event, layout, cursor, renderer, clipboard, shell, viewport,
            )
        }

        fn mouse_interaction(
            &self,
            state: &Tree,
            layout: Layout<'_>,
            cursor: mouse::Cursor,
            viewport: &Rectangle,
            renderer: &Renderer,
        ) -> mouse::Interaction {
            let _ = self.content.as_widget().mouse_interaction(
                state, layout, cursor, viewport, renderer,
            );
            mouse::Interaction::Idle
        }

        fn overlay<'b>(
            &'b mut self,
            state: &'b mut Tree,
            layout: Layout<'b>,
            renderer: &Renderer,
            viewport: &Rectangle,
            translation: iced::Vector,
        ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
            self.content.as_widget_mut().overlay(state, layout, renderer, viewport, translation)
        }
    }

    impl<'a, Message, Theme, Renderer> From<CursorOff<'a, Message, Theme, Renderer>> for Element<'a, Message, Theme, Renderer>
    where
        Message: 'a,
        Theme: 'a,
        Renderer: iced::advanced::Renderer + 'a,
    {
        fn from(widget: CursorOff<'a, Message, Theme, Renderer>) -> Self {
            Element::new(widget)
        }
    }
}

#[derive(Debug, Clone)]

pub enum Message {
    // Engine & Playback
    Tick,
    PlayPause,
    NextTrack,
    PreviousTrack,
    Stop,
    VolumeChanged(f32),
    SeekTo(f32),
    ToggleRepeat,
    ToggleShuffle,

    // Playlist
    AddSongToPlaylist(std::sync::Arc<crate::db::database::SongData>),
    PlayAlbum(Vec<std::sync::Arc<crate::db::database::SongData>>),
    PlaySongIndex(usize),
    ClearPlaylist,
    ToggleSongEnabled(usize),
    SwitchPlaylist(i64, bool), // id, auto_play
    ToggleTabDropdown,
    PlaylistSearchChanged(String),
    PlaylistFocus,
    ToggleLyrics,
    ToggleGroupEnabled(usize),
    TogglePlaylistFolder(usize),
    GlobalKeyDown(iced::keyboard::Key, iced::keyboard::Modifiers),
    CreatePlaylist,
    ExportPlaylist(i64, String), // id, target_path
    OpenPlaylistFilePicker,
    ImportPlaylistFile(String),   // path

    // Library
    ScanLibrary(String),
    SearchQueryChanged(String),
    LibrarySearchQueryChanged(String),
    LibrarySourceSelected(crate::gui::library::LibrarySource),
    LibrarySortChanged(crate::utils::SortColumn),
    StartColumnResize(crate::utils::SortColumn),
    ColumnHover(Option<crate::utils::SortColumn>),
    ToggleAlbumExpansion(String),
    ToggleArtistExpansion(String),
    SelectAlbum(String),
    SelectSong(Option<usize>),
    SelectArtistHeader(String),
    LibraryKeyNav(crate::gui::library::LibraryNavDir, iced::keyboard::Modifiers),
    LibraryScroll(iced::widget::scrollable::Viewport),
    LibraryDeselect,
    LibraryDeleteSelection,
    ChangeLibraryViewMode(crate::gui::library::LibraryViewMode),
    ToggleLibraryViewDropdown,
    ToggleLibraryAddDropdown,
    PlayLibrarySelection,
    PlayLibraryAll,
    LibraryAllSongsLoaded(Vec<std::sync::Arc<crate::db::database::SongData>>),
    OpenFolderPicker,

    // Filters
    ToggleFilterMenu,
    ChangeGeneralFilter(crate::gui::library_filters::FilterType),
    SelectSubfilter(Option<String>),
    SelectTreeNode(String),
    ToggleTreeNode(String),
    SelectFolder(i64),
    FilterSearchChanged(String),

    // Audio Center
    ToggleAudioCenter,
    AudioCenterMsg(AudioCenterMessage),

    // Player Módulo 1
    PlayerHoverZone(crate::gui::player::HoverZone),
    PlayerScroll(f32),
    PlayerVolumeTimeout(u64),
    ToggleMenu,
    PlayerWindowAction(crate::gui::player::WindowAction),
    SetWindowId(iced::window::Id),
    PlayerMouseMoved(iced::Point),
    PlaylistScrolled(iced::widget::scrollable::Viewport),
    PlaylistMouseOver(bool),
    GlobalMouseRelease,
    PlayerActivityTimeout(u64),
    GlobalClick,
    WindowResized(u32, u32),
    InitStartup,

    // Dialogs
    OpenDialog(ActiveDialog),
    CloseDialog,
    UpdateDialogInput(String),
    ConfirmDialogAction,

    // Context Menu
    OpenContextMenu(iced::Point, Vec<crate::gui::widgets::ContextMenuEntry<Message>>),
    CloseContextMenu,

    NoOp,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActiveDialog {
    None,
    CreatePlaylist { name: String },
    RenamePlaylist { id: i64, current_name: String, new_name: String },
    DeleteConfirm { id: i64, name: String },
    ExportConfirm { id: i64, name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppFocus {
    Library,
    Playlist,
}

pub struct AudoxidyApp {
    audio_manager: Arc<AudioManager>,
    database: Arc<Mutex<Database>>,
    scanner: Arc<Scanner>,
    media_controls: SystemMediaControls,
    playlist_manager: PlaylistManager,
    filters_manager: LibraryFiltersManager,
    library_manager: LibraryManager,
    audio_center_manager: AudioCenterManager,
    player_ui_state: crate::gui::player::PlayerUiState,
    window_id: Option<iced::window::Id>,
    last_artist_header_click: Option<(String, std::time::Instant)>,
    last_album_header_click: Option<(String, std::time::Instant)>,
    last_playlist_click: Option<(usize, std::time::Instant)>,
    pub focus: AppFocus,
    pub is_mouse_over_playlist: bool,
    pub low_resource_mode: bool,
    pub db_needs_refresh: bool,
    pub active_dialog: ActiveDialog,
    pub last_mouse_pos: iced::Point,
    pub dialog_pos: Option<iced::Point>,
    pub context_menu: Option<(iced::Point, Vec<crate::gui::widgets::ContextMenuEntry<Message>>)>,
    pub window_size: (u32, u32),
}

impl AudoxidyApp {
    pub fn new(audio_manager: Arc<AudioManager>) -> (Self, Task<Message>) {
        let db = Database::new().expect("Error crítico al crear/iniciar la base de datos.");
        let mut library_manager = LibraryManager::default();
        if let Ok(albums) = db.get_grid_items_by_artist() {
            library_manager.cached_albums = Some(albums);
        }
        if let Ok((total_songs, total_albums, total_duration, total_size, total_artists)) = db.get_library_stats() {
            library_manager.total_songs = total_songs;
            library_manager.total_albums = total_albums;
            library_manager.total_artists = total_artists;
            library_manager.total_duration_secs = total_duration;
            library_manager.total_size_bytes = total_size;
        }

        if let Ok(folders) = db.get_all_folders() {
            library_manager.cached_folders = Some(folders);
        }

        // Obtener puntos de montaje para el sistema de rutas inteligente
        let mut mount_points = Vec::new();
        if let Ok(content) = std::fs::read_to_string("/proc/mounts") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let mnt = parts[1];
                    if mnt.starts_with("/mnt/") || mnt.starts_with("/media/") || mnt == "/" {
                        mount_points.push((mnt.to_string(), mnt.to_string()));
                    }
                }
            }
        }
        library_manager.mount_points = mount_points;

        // Cargar canciones inmediatamente para que los filtros estén disponibles al instante
        if let Ok(mut songs) = db.get_all_songs() {
            if library_manager.sort_column.is_some() {
                library_manager.sort_songs(&mut songs);
            }
            library_manager.cached_all_songs = Some(songs);
            library_manager.apply_filter();
        }

        let database_arc = Arc::new(Mutex::new(db));
        let scanner_arc = Arc::new(Scanner::new(Arc::clone(&database_arc)));

        let media_controls = SystemMediaControls::new(audio_manager.clone())
            .expect("Error al inicializar controles multimedia del sistema");

        // Detección de Hardware para modo de bajos recursos (RAM < 8GB o Cores < 4)
        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();
        let total_ram_gb = sys.total_memory() / 1024 / 1024 / 1024;
        let cpu_cores = sys.cpus().len();
        let low_resource_mode = total_ram_gb < 8 || cpu_cores < 4;

        // Configurar flag global para que todos los módulos puedan consultarlo
        crate::utils::LOW_RESOURCE_MODE.store(low_resource_mode, std::sync::atomic::Ordering::Relaxed);

        if low_resource_mode {
            println!("Audoxidy Performance: Low resource mode ENABLED (RAM: {}GB, Cores: {})", total_ram_gb, cpu_cores);
        }

        // Construir el índice de filtros con las canciones ya cargadas
        let mut filters_manager = LibraryFiltersManager::default();
        filters_manager.build_filter_index(&library_manager);

        let mut playlist_manager = PlaylistManager::default();
        if let Ok(db_lock) = database_arc.lock() {
            // 1. Cargar última playlist activa
            if let Some(last_id_str) = db_lock.get_setting("last_active_playlist_id") {
                if let Ok(last_id) = last_id_str.parse::<i64>() {
                    playlist_manager.active_playlist_id = last_id;
                }
            }
            
            let active_id = playlist_manager.active_playlist_id;
            if let Ok(groups) = db_lock.get_playlist_songs_grouped_by_folder(active_id) {
                playlist_manager.groups = groups;
            }
            
            if let Ok(all_p) = db_lock.get_all_playlists() {
                playlist_manager.playlists = all_p;
            }
            
            // Cargar persistencia inicial
            if let Ok(Some(p_data)) = db_lock.get_playlist_by_id(active_id) {
                playlist_manager.shuffle_active = p_data.shuffle_active;
                playlist_manager.repeat_mode = p_data.repeat_mode as u8;
                
                if p_data.shuffle_active {
                    if let Ok(Some(mut session)) = db_lock.load_shuffle_session(active_id) {
                        // Sincronizar posición de sesión con la canción restaurada
                        if let Some(song_id) = p_data.last_song_id {
                            if let Some(pos) = session.shuffle_order.iter().position(|&id| id == song_id) {
                                session.current_position = pos + 1; // Apuntar a la siguiente en la cola
                                if !session.history.contains(&song_id) {
                                    session.history.push(song_id);
                                }
                            }
                        }
                        playlist_manager.shuffle_session = Some(session);
                    }
                }

                // Restaurar canción y posición (esto se carga pero el 'play' lo haremos vía Task para mayor seguridad)
                if let Some(song_id) = p_data.last_song_id {
                    if let Some(l_idx) = playlist_manager.get_linear_index_by_song_id(song_id) {
                        if let Some(song) = playlist_manager.get_song_at_linear_index(l_idx) {
                             let path = song.file_path.clone();
                             playlist_manager.playing_song_idx = Some(l_idx);
                             let _ = audio_manager.load_file(&path);
                             audio_manager.seek(p_data.last_pos_sec);
                             audio_manager.set_playing(p_data.is_playing);
                        }
                    }
                }
            }
        }

        (
            Self {
                audio_manager,
                database: database_arc,
                scanner: scanner_arc,
                media_controls,
                playlist_manager,
                filters_manager,
                library_manager,
                audio_center_manager: AudioCenterManager::default(),
                player_ui_state: crate::gui::player::PlayerUiState::default(),
                window_id: None,
                last_artist_header_click: None,
                last_album_header_click: None,
                last_playlist_click: None,
                focus: AppFocus::Library,
                is_mouse_over_playlist: false,
                low_resource_mode,
                db_needs_refresh: false,
                active_dialog: ActiveDialog::None,
                last_mouse_pos: iced::Point::ORIGIN,
                dialog_pos: None,
                context_menu: None,
                window_size: (1280, 720), // Default inicial
            },
            Task::perform(
                async { tokio::time::sleep(std::time::Duration::from_millis(1000)).await; },
                |_| Message::InitStartup
            )
        )
    }
    fn wake_up_controls(&mut self, is_mouse_move: bool) -> Task<Message> {
        self.player_ui_state.is_active = true;
        self.player_ui_state.active_until_tick = self.player_ui_state.tick_count.wrapping_add(10); // 10 ticks = 1000ms
        
        let mut tasks = vec![];
        
        if is_mouse_move {
            if self.player_ui_state.showing_volume.is_some() && !self.player_ui_state.volume_clearing {
                self.player_ui_state.volume_clearing = true;
                let current_vol_tick = self.player_ui_state.volume_tick_id;
                tasks.push(iced::Task::perform(
                    async { tokio::time::sleep(std::time::Duration::from_millis(1000)).await },
                    move |_| Message::PlayerVolumeTimeout(current_vol_tick)
                ));
            }
        } else {
            self.player_ui_state.showing_volume = None;
            self.player_ui_state.volume_clearing = false;
        }
        
        if tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(tasks)
        }
    }

    fn execute_playlist_autoscroll(&self, target_idx: usize) -> Task<Message> {
        if let Some(viewport) = &self.playlist_manager.last_viewport {
            let (item_top, item_bottom) = self.playlist_manager.get_item_y_bounds(target_idx);
            
            let view_min = viewport.y;
            let view_max = view_min + viewport.height;
            
            if item_top < view_min {
                // Scroll hacia arriba para mostrar el tope del ítem
                return iced::widget::operation::scroll_to(
                    crate::gui::playlist::PLAYLIST_SCROLL_ID.clone(), 
                    iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: item_top }
                );
            } else if item_bottom > view_max {
                // Scroll hacia abajo para mostrar el fondo del ítem al límite inferior del viewport
                let offset = item_bottom - viewport.height;
                return iced::widget::operation::scroll_to(
                    crate::gui::playlist::PLAYLIST_SCROLL_ID.clone(), 
                    iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: offset }
                );
            }
        }
        Task::none()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let state = self.audio_manager.get_state();
        self.media_controls.update(&state);

        match message {
            Message::Tick => {
                self.player_ui_state.tick_count = self.player_ui_state.tick_count.wrapping_add(1);
                
                if self.player_ui_state.is_active && self.player_ui_state.tick_count > self.player_ui_state.active_until_tick {
                    self.player_ui_state.is_active = false;
                }
                
                // Consultar al scanner si hay datos nuevos en la BD
                if self.scanner.db_dirty.swap(false, std::sync::atomic::Ordering::Relaxed) {
                    self.db_needs_refresh = true;
                }
                
                // Actualizar BD SOLO cuando el escáner marca cambios (no periódicamente)
                if self.db_needs_refresh {
                    self.db_needs_refresh = false;
                    if let Ok(db) = self.database.lock() {
                        // Álbumes: siempre cargar (son ligeros y necesarios para Grid)
                        if let Ok(mut albums) = db.get_grid_items_by_artist() {
                            if let Some(col_ref) = self.library_manager.sort_column {
                                let is_asc = self.library_manager.sort_ascending.unwrap_or(true);
                                albums.sort_by(|a, b| {
                                    let res = match col_ref {
                                        crate::utils::SortColumn::Album => a.1.cmp(&b.1).then(a.2.cmp(&b.2)).then(a.4.cmp(&b.4)),
                                        crate::utils::SortColumn::Artist => a.2.cmp(&b.2).then(a.4.cmp(&b.4)).then(a.1.cmp(&b.1)),
                                        crate::utils::SortColumn::AlbumArtist => a.2.cmp(&b.2).then(a.4.cmp(&b.4)).then(a.1.cmp(&b.1)),
                                        crate::utils::SortColumn::Genre => a.3.cmp(&b.3).then(a.2.cmp(&b.2)).then(a.4.cmp(&b.4)).then(a.1.cmp(&b.1)),
                                        crate::utils::SortColumn::Year => a.4.cmp(&b.4).then(a.2.cmp(&b.2)).then(a.1.cmp(&b.1)),
                                        _ => std::cmp::Ordering::Equal,
                                    };
                                    if is_asc { res } else { res.reverse() }
                                });
                            }
                            self.library_manager.cached_albums = Some(albums);
                        }
                        
                        // Canciones: cargar siempre para habilitar búsqueda global (títulos, artistas, etc.)
                        if let Ok(mut songs) = db.get_all_songs() {
                            if self.library_manager.sort_column.is_some() {
                                self.library_manager.sort_songs(&mut songs);
                            }
                            self.library_manager.cached_all_songs = Some(songs);
                            self.library_manager.apply_filter();
                            self.filters_manager.build_filter_index(&self.library_manager);
                        }
                        
                        // Estadísticas siempre (son ligeras)
                        if let Ok((songs, albums, duration, size, total_artists)) = db.get_library_stats() {
                            self.library_manager.total_songs = songs;
                            self.library_manager.total_albums = albums;
                            self.library_manager.total_artists = total_artists;
                            self.library_manager.total_duration_secs = duration;
                            self.library_manager.total_size_bytes = size;
                        }
                    }
                }
                
                // Sincronización de carátulas solo si hay reproducción activa
                if state.is_playing {
                    self.sync_player_art();

                    // Precarga inteligente (30 segundos antes de finalizar)
                    let duration = state.total_duration_sec;
                    let position = state.current_pos_sec;
                    if duration > 0.0 {
                        let remaining = duration - position;
                        if remaining <= 30.0 && remaining > 29.5 {
                            // Precargar carátula de la siguiente canción
                            if let Some(next_song) = self.playlist_manager.get_next_song_ref() {
                                if let Some(ref cover_path) = next_song.cover_path {
                                    // Comparar hash del álbum actual vs siguiente
                                    let current_cover = self.playlist_manager.playing_song_idx
                                        .and_then(|idx| self.playlist_manager.get_song_at_linear_index(idx))
                                        .and_then(|s| s.cover_path.clone());
                                    
                                    if current_cover.as_deref() != Some(cover_path.as_str()) {
                                        // Álbum diferente: precargar handle
                                        self.player_ui_state.prefetched_next_handle = 
                                            crate::utils::covers::load_cover_handle(cover_path);
                                    }
                                }
                            }
                        }
                    }
                }

                if state.eof_reached {
                    self.audio_manager.clear_eof();
                    self.playlist_manager.play_next(&self.audio_manager);
                    
                    if self.playlist_manager.shuffle_active {
                        if let Some(session) = &self.playlist_manager.shuffle_session {
                            if let Ok(db) = self.database.lock() {
                                let _ = db.save_shuffle_session(self.playlist_manager.active_playlist_id, session);
                            }
                        }
                    }
                    
                    self.sync_player_art();
                    self.persist_playlist_state();
                    
                    if !self.is_mouse_over_playlist {
                        if let Some(idx) = self.playlist_manager.playing_song_idx {
                            return self.execute_playlist_autoscroll(idx);
                        }
                    }
                }

                // Auto-guardado de persistencia cada ~20 segundos (120 ticks de ~166ms)
                if self.player_ui_state.tick_count % 120 == 0 {
                    self.persist_playlist_state();
                }

                Task::none()
            }
            Message::PlayPause => {
                let _ = self.audio_manager.toggle_play_pause();
                self.persist_playlist_state();
                self.wake_up_controls(false)
            }
            Message::NextTrack => {
                crate::utils::covers::clear_raw_cache();
                self.playlist_manager.play_next(&self.audio_manager);
                self.persist_playlist_state();
                
                if self.playlist_manager.shuffle_active {
                    if let Some(session) = &self.playlist_manager.shuffle_session {
                        if let Ok(db) = self.database.lock() {
                            let _ = db.save_shuffle_session(self.playlist_manager.active_playlist_id, session);
                        }
                    }
                }
                
                self.sync_player_art();
                let wake_task = self.wake_up_controls(false);
                if !self.is_mouse_over_playlist {
                    if let Some(idx) = self.playlist_manager.playing_song_idx {
                        let scroll_task = self.execute_playlist_autoscroll(idx);
                        return Task::batch(vec![wake_task, scroll_task]);
                    }
                }
                wake_task
            }
            Message::PreviousTrack => {
                crate::utils::covers::clear_raw_cache();
                self.playlist_manager.play_prev(&self.audio_manager);
                self.persist_playlist_state();
                
                if self.playlist_manager.shuffle_active {
                    if let Some(session) = &self.playlist_manager.shuffle_session {
                        if let Ok(db) = self.database.lock() {
                            let _ = db.save_shuffle_session(self.playlist_manager.active_playlist_id, session);
                        }
                    }
                }
                
                self.sync_player_art();
                let wake_task = self.wake_up_controls(false);
                if !self.is_mouse_over_playlist {
                    if let Some(idx) = self.playlist_manager.playing_song_idx {
                        let scroll_task = self.execute_playlist_autoscroll(idx);
                        return Task::batch(vec![wake_task, scroll_task]);
                    }
                }
                wake_task
            }
            Message::Stop => {
                self.audio_manager.stop();
                Task::none()
            }
            Message::VolumeChanged(vol) => {
                self.audio_manager.set_volume(vol);
                Task::none()
            }
            Message::SeekTo(pos) => {
                self.audio_manager.seek(pos as f64);
                self.persist_playlist_state();
                self.wake_up_controls(false)
            }
            Message::ToggleRepeat => {
                self.playlist_manager.repeat_mode = (self.playlist_manager.repeat_mode + 1) % 3;
                self.persist_playlist_state();
                Task::none()
            }
            Message::ToggleShuffle => {
                if !self.playlist_manager.shuffle_active {
                    self.playlist_manager.activate_shuffle();
                    if let Some(session) = &self.playlist_manager.shuffle_session {
                        if let Ok(db) = self.database.lock() {
                            let _ = db.save_shuffle_session(self.playlist_manager.active_playlist_id, session);
                        }
                    }
                } else {
                    self.playlist_manager.deactivate_shuffle();
                    if let Ok(db) = self.database.lock() {
                        let _ = db.clear_shuffle_session(self.playlist_manager.active_playlist_id);
                    }
                }
                self.persist_playlist_state();
                Task::none()
            }
            Message::AddSongToPlaylist(song) => {
                if let Ok(db) = self.database.lock() {
                    let _ = db.add_song_to_playlist(self.playlist_manager.active_playlist_id, song.id);
                    if let Ok(groups) = db.get_playlist_songs_grouped_by_folder(self.playlist_manager.active_playlist_id) {
                        self.playlist_manager.groups = groups;
                    }
                }
                Task::none()
            }
            Message::PlayAlbum(songs) => {
                if let Ok(db) = self.database.lock() {
                    let song_ids: Vec<i64> = songs.iter().map(|s| s.id).collect();
                    let _ = db.add_songs_to_playlist(self.playlist_manager.active_playlist_id, &song_ids);
                    if let Ok(groups) = db.get_playlist_songs_grouped_by_folder(self.playlist_manager.active_playlist_id) {
                        self.playlist_manager.groups = groups;
                    }
                }
                Task::none()
            }
            Message::PlaySongIndex(idx) => {
                self.focus = AppFocus::Playlist;
                let now = std::time::Instant::now();
                let is_double_click = if let Some((last_idx, last_time)) = self.last_playlist_click {
                    last_idx == idx && now.duration_since(last_time).as_millis() < 400
                } else {
                    false
                };

                self.last_playlist_click = Some((idx, now));
                self.playlist_manager.selected_song_idx = Some(idx);

                if is_double_click {
                    if let Some(song) = self.playlist_manager.get_song_at_linear_index(idx) {
                        let path = song.file_path.clone();
                        self.sync_player_art();
                        if let Err(e) = self.audio_manager.load_file(&path) {
                            tracing::error!("Error reproduciendo archivo: {}", e);
                        } else {
                            self.audio_manager.play();
                            self.playlist_manager.playing_song_idx = Some(idx);
                            self.persist_playlist_state();
                        }
                    }
                }
                Task::none()
            }
            Message::ClearPlaylist => {
                if let Ok(db) = self.database.lock() {
                    let _ = db.clear_playlist(self.playlist_manager.active_playlist_id);
                    self.playlist_manager.groups = Vec::new();
                    self.playlist_manager.playing_song_idx = None;
                    self.audio_manager.stop();
                    self.persist_playlist_state();
                }
                Task::none()
            }
            Message::ToggleSongEnabled(linear_idx) => {
                self.focus = AppFocus::Playlist;
                self.playlist_manager.toggle_song_enabled_at_linear_index(linear_idx);
                // Persistir en BD (Fase 4)
                Task::none()
            }
            Message::ToggleGroupEnabled(linear_idx) => {
                self.focus = AppFocus::Playlist;
                let _ = self.playlist_manager.toggle_group_enabled_at_linear_index(linear_idx);
                Task::none()
            }
            Message::TogglePlaylistFolder(linear_idx) => {
                self.focus = AppFocus::Playlist;
                let now = std::time::Instant::now();
                let is_double_click = if let Some((last_idx, last_time)) = self.last_playlist_click {
                    last_idx == linear_idx && now.duration_since(last_time).as_millis() < 400
                } else {
                    false
                };

                self.last_playlist_click = Some((linear_idx, now));
                self.playlist_manager.selected_song_idx = Some(linear_idx);

                if is_double_click {
                    use crate::gui::playlist::PlaylistItemType;
                    if let Some(PlaylistItemType::Separator(path)) = self.playlist_manager.get_item_info_at_linear_index(linear_idx) {
                        self.playlist_manager.toggle_group_expansion(path);
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

            Message::OpenDialog(dialog) => {
                self.active_dialog = dialog;
                self.dialog_pos = Some(self.last_mouse_pos);
                Task::none()
            }

            Message::CloseDialog => {
                self.active_dialog = ActiveDialog::None;
                self.dialog_pos = None;
                Task::none()
            }

            Message::UpdateDialogInput(s) => {
                match &mut self.active_dialog {
                    ActiveDialog::CreatePlaylist { name } => *name = s,
                    ActiveDialog::RenamePlaylist { new_name, .. } => *new_name = s,
                    _ => {}
                }
                Task::none()
            }

            Message::ConfirmDialogAction => {
                let dialog = std::mem::replace(&mut self.active_dialog, ActiveDialog::None);
                match dialog {
                    ActiveDialog::CreatePlaylist { name } => {
                        let final_name = if name.trim().is_empty() { 
                            format!("Nueva lista {}", self.playlist_manager.playlists.len() + 1) 
                        } else { name };

                        if let Ok(new_id) = self.database.lock().unwrap().create_playlist(&final_name, false) {
                            if let Ok(playlists) = self.database.lock().unwrap().get_all_playlists() {
                                self.playlist_manager.playlists = playlists;
                                // Cierre inmediato
                                self.active_dialog = ActiveDialog::None;
                                self.dialog_pos = None;
                                return Task::done(Message::SwitchPlaylist(new_id, false));
                            }
                        }
                    }
                    ActiveDialog::RenamePlaylist { id, new_name, .. } => {
                        if !new_name.trim().is_empty() {
                            if let Ok(_) = self.database.lock().unwrap().rename_playlist(id, &new_name) {
                                if let Ok(playlists) = self.database.lock().unwrap().get_all_playlists() {
                                    self.playlist_manager.playlists = playlists;
                                    self.active_dialog = ActiveDialog::None;
                                    self.dialog_pos = None;
                                }
                            }
                        }
                    }
                    ActiveDialog::DeleteConfirm { id, .. } => {
                        if let Ok(_) = self.database.lock().unwrap().delete_playlist(id) {
                            if let Ok(playlists) = self.database.lock().unwrap().get_all_playlists() {
                                if self.playlist_manager.active_playlist_id == id {
                                    if let Some(first) = playlists.first() {
                                        let first_id = first.id;
                                        self.playlist_manager.playlists = playlists;
                                        self.active_dialog = ActiveDialog::None;
                                        self.dialog_pos = None;
                                        return Task::done(Message::SwitchPlaylist(first_id, false));
                                    }
                                }
                                self.playlist_manager.playlists = playlists;
                                self.active_dialog = ActiveDialog::None;
                                self.dialog_pos = None;
                            }
                        }
                    }
                    ActiveDialog::ExportConfirm { id, name: _ } => {
                        return Task::perform(async move {
                            if let Some(folder) = rfd::AsyncFileDialog::new()
                                .set_title("Seleccionar carpeta de exportación")
                                .pick_folder().await {
                                    Some((id, folder.path().to_string_lossy().into_owned()))
                            } else { None }
                        }, |res| {
                            if let Some((pid, path)) = res {
                                Message::ExportPlaylist(pid, path)
                            } else { Message::NoOp }
                        });
                    }
                    _ => {}
                }
                Task::none()
            }

            Message::ExportPlaylist(id, target_path) => {
                let db = self.database.clone();
                
                return Task::perform(async move {
                    let sanitize_name = |s: &str| s.chars().map(|c| if "/\\?%*:|\"<>".contains(c) { '_' } else { c }).collect::<String>();
                    
                    let songs = if let Ok(db_lock) = db.lock() {
                        db_lock.search_playlist_songs(id, "").unwrap_or_default()
                    } else { vec![] };

                    let p_name = if let Ok(db_lock) = db.lock() {
                        db_lock.get_playlist_by_id(id).ok().flatten().map(|p| p.name).unwrap_or_else(|| "Playlist_Exportada".to_string())
                    } else { "Playlist_Exportada".to_string() };

                    let base_dir = std::path::PathBuf::from(target_path);
                    let mut m3u_content = String::from("#EXTM3U\n");

                    for song in songs {
                        let artist_dir = sanitize_name(&song.artist_name);
                        let album_dir = sanitize_name(&song.album_title);
                        let song_file = std::path::Path::new(&song.file_path).file_name().unwrap_or_default().to_string_lossy();
                        
                        let relative_path = format!("{}/{}/{}", artist_dir, album_dir, song_file);
                        let full_target_dir = base_dir.join(&artist_dir).join(&album_dir);
                        let full_target_path = full_target_dir.join(song_file.as_ref());

                        let _ = std::fs::create_dir_all(&full_target_dir);
                        let _ = std::fs::copy(&song.file_path, &full_target_path);

                        m3u_content.push_str(&format!("#EXTINF:{},{}\n{}\n", song.duration as i32, song.title, relative_path));
                    }

                    let m3u_path = base_dir.join(format!("{}.m3u8", sanitize_name(&p_name)));
                    let _ = std::fs::write(m3u_path, m3u_content);
                    
                }, |_| Message::NoOp);
            }

            Message::SwitchPlaylist(id, auto_play) => {
                // 1. Guardar estado de la lista actual antes de cambiar
                self.persist_playlist_state();
                if let Ok(db) = self.database.lock() {
                    let _ = db.set_setting("last_active_playlist_id", &id.to_string());
                }

                // 2. Cambiar a la nueva lista
                self.playlist_manager.active_playlist_id = id;
                
                let mut p_data_opt = None;
                let mut groups_opt = None;
                let mut session_opt = None;

                if let Ok(db) = self.database.lock() {
                    groups_opt = db.get_playlist_songs_grouped_by_folder(id).ok();
                    p_data_opt = db.get_playlist_by_id(id).ok().flatten();
                    if let Some(p) = &p_data_opt {
                        if p.shuffle_active {
                            if let Ok(Some(mut session)) = db.load_shuffle_session(id) {
                                // Sincronizar posición de sesión con la canción restaurada de la nueva lista
                                if let Some(song_id) = p.last_song_id {
                                    if let Some(pos) = session.shuffle_order.iter().position(|&sid| sid == song_id) {
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
                }

                if let Some(groups) = groups_opt {
                    self.playlist_manager.groups = groups;
                }

                let mut was_restored = false;
                if let Some(p_data) = p_data_opt {
                    self.playlist_manager.shuffle_active = p_data.shuffle_active;
                    self.playlist_manager.repeat_mode = p_data.repeat_mode as u8;
                    self.playlist_manager.shuffle_session = session_opt;

                    // Restaurar canción y posición si existen
                    if let Some(song_id) = p_data.last_song_id {
                        if let Some(l_idx) = self.playlist_manager.get_linear_index_by_song_id(song_id) {
                            if let Some(song) = self.playlist_manager.get_song_at_linear_index(l_idx) {
                                let path = song.file_path.clone();
                                let is_playing_needed = if auto_play { true } else { p_data.is_playing };
                                let last_pos = p_data.last_pos_sec;
                                
                                self.playlist_manager.playing_song_idx = Some(l_idx);
                                let _ = self.audio_manager.load_file(&path);
                                self.audio_manager.seek(last_pos);
                                self.audio_manager.set_playing(is_playing_needed);
                                self.sync_player_art();
                                was_restored = true;
                            }
                        }
                    }
                }
                
                self.playlist_manager.show_tab_dropdown = false;
                self.playlist_manager.selected_song_idx = None;
                self.playlist_manager.apply_filter();
                
                if auto_play && !was_restored {
                    // Si auto_play es true y no había canción previa guardada, reproducimos la primera
                    return Task::done(Message::PlaySongIndex(0));
                }
                
                Task::none()
            }
            Message::ToggleTabDropdown => {
                self.playlist_manager.show_tab_dropdown = !self.playlist_manager.show_tab_dropdown;
                Task::none()
            }
            Message::PlaylistSearchChanged(q) => {
                self.focus = AppFocus::Playlist;
                self.playlist_manager.search_query = q;
                self.playlist_manager.apply_filter();
                Task::none()
            }
            Message::PlaylistFocus => {
                self.focus = AppFocus::Playlist;
                Task::none()
            }
            Message::ToggleLyrics => {
                // Fase 6: abrir módulo de letras
                Task::none()
            }
            Message::ScanLibrary(folder) => {
                self.scanner.scan_folder_async(folder);
                // Marcar que la BD necesita refrescarse tras el escaneo
                self.db_needs_refresh = true;
                self.player_ui_state.tick_count = 0;
                self.update(Message::Tick)
            }
            Message::SearchQueryChanged(q) => {
                self.playlist_manager.search_query = q;
                self.playlist_manager.apply_filter();
                Task::none()
            }
            Message::LibrarySearchQueryChanged(q) => {
                self.focus = AppFocus::Library;
                self.library_manager.search_query = q;
                self.library_manager.apply_filter();
                self.library_manager.last_viewport = None;
                scroll_to(LIBRARY_SCROLL_ID.clone(), AbsoluteOffset { x: 0.0, y: 0.0 })
            }
            Message::LibrarySourceSelected(source) => {
                self.library_manager.source = source;
                // En el futuro, disparar recarga desde la fuente elegida
                Task::none()
            }
            Message::LibrarySortChanged(col) => {
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
                    if let Ok(db) = self.database.lock() {
                        if let Ok(albums) = db.get_grid_items_by_artist() {
                            self.library_manager.cached_albums = Some(albums);
                        }
                        if let Some(album_id) = &self.library_manager.expanded_album {
                            if let Ok(songs) = db.get_songs_by_album(album_id) {
                                self.library_manager.expanded_album_songs = Some(songs);
                            }
                        }
                    }
                } else {
                    let col_ref = self.library_manager.sort_column.unwrap();
                    let is_asc = self.library_manager.sort_ascending.unwrap_or(true);
                    
                    // Aplicar el sort en memoria a albums
                    if let Some(albums) = &mut self.library_manager.cached_albums {
                        albums.sort_by(|a, b| {
                            let res = match col_ref {
                                crate::utils::SortColumn::Album => a.1.cmp(&b.1).then(a.2.cmp(&b.2)).then(a.4.cmp(&b.4)),
                                crate::utils::SortColumn::Artist => a.2.cmp(&b.2).then(a.4.cmp(&b.4)).then(a.1.cmp(&b.1)),
                                crate::utils::SortColumn::AlbumArtist => a.2.cmp(&b.2).then(a.4.cmp(&b.4)).then(a.1.cmp(&b.1)), // Fallback grouped_artist uses AlbumArtist anyway
                                crate::utils::SortColumn::Genre => a.3.cmp(&b.3).then(a.2.cmp(&b.2)).then(a.4.cmp(&b.4)).then(a.1.cmp(&b.1)),
                                crate::utils::SortColumn::Year => a.4.cmp(&b.4).then(a.2.cmp(&b.2)).then(a.1.cmp(&b.1)),
                                _ => std::cmp::Ordering::Equal,
                            };
                            if is_asc { res } else { res.reverse() }
                        });
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
                self.library_manager.resizing_start_x = self.player_ui_state.mouse_pos.map(|p| p.x).unwrap_or(0.0);
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
                if self.library_manager.view_mode == crate::gui::library::LibraryViewMode::DetailedList {
                    if self.library_manager.collapsed_albums.contains(&album_id) {
                        self.library_manager.collapsed_albums.remove(&album_id);
                    } else {
                        self.library_manager.collapsed_albums.insert(album_id.clone());
                    }
                    self.library_manager.selected_album = Some(album_id.clone());
                    self.library_manager.selected_song_idx = None;
                    self.library_manager.selected_item_hint = None; // Reset para forzar búsqueda por nombre tras cambio estructural
                } else {
                    // Flecha: toggle expansión Y seleccionar álbum (sincronizado)
                    if self.library_manager.expanded_album.as_deref() == Some(album_id.as_str()) {
                        self.library_manager.expanded_album = None;
                        self.library_manager.expanded_album_songs = None;
                        self.library_manager.selected_song_idx = None;
                    } else {
                        self.library_manager.expanded_album = Some(album_id.clone());
                        self.library_manager.selected_album = Some(album_id.clone()); // Sincronizamos selección
                        self.library_manager.selected_song_idx = None;
                        self.library_manager.selected_item_hint = None; // Reset hint tras expansión
                        if let Ok(db) = self.database.lock() {
                            // Soporte para identidades compuestas (Artista|Hash) para filtrar por artista en Grid
                            let query_res = if album_id.contains('|') {
                                let parts: Vec<&str> = album_id.split('|').collect();
                                if parts.len() >= 2 {
                                    db.get_songs_by_album_and_artist(parts[1], parts[0])
                                } else {
                                    db.get_songs_by_album(&album_id)
                                }
                            } else {
                                db.get_songs_by_album(&album_id)
                            };

                            if let Ok(mut songs) = query_res {
                                if self.library_manager.sort_column.is_some() {
                                    self.library_manager.sort_songs(&mut songs);
                                }
                                self.library_manager.expanded_album_songs = Some(songs);
                            }
                        }
                    }
                }
                self.update_selection_stats();
                Task::none()
            }
            Message::ToggleArtistExpansion(name) => {
                self.focus = AppFocus::Library;
                let is_collapsing = !self.library_manager.collapsed_artists.contains(&name);
                
                if is_collapsing {
                    self.library_manager.collapsed_artists.insert(name.clone());
                    
                    // Forzar foco ÚNICAMENTE en la cabecera del artista que colapsa
                    self.library_manager.selected_header = Some(name.clone());
                    self.library_manager.selected_song_idx = None;
                    self.library_manager.selected_album = None;
                    self.library_manager.selected_item_hint = None; // Reset hint tras colapso de artista
                    
                    // Aseguramos que el estado de 'last_artist_header_click' se limpie para evitar dobles clics accidentales inmediatos
                    self.last_artist_header_click = None;
                } else {
                    self.library_manager.collapsed_artists.remove(&name);
                    
                    // Al expandir, mantenemos el comportamiento de ir a la canción recordada o a la primera
                    self.library_manager.selected_header = None;
                    if let Some(last_song_idx) = self.library_manager.artist_last_selection.get(&name) {
                        self.library_manager.selected_song_idx = Some(*last_song_idx);
                    } else {
                        // Ir a la primera canción del grupo
                        let mut start_idx = 0;
                        for g in &self.library_manager.artist_groups {
                            if g.name == name {
                                if !g.songs.is_empty() {
                                    self.library_manager.selected_song_idx = Some(start_idx);
                                }
                                break;
                            }
                            start_idx += g.songs.len();
                        }
                    }
                }
                self.update_selection_stats();
                self.get_library_scroll_task(is_collapsing)
            }
            Message::LibraryAllSongsLoaded(songs) => {
                self.library_manager.cached_all_songs = Some(songs);
                self.library_manager.apply_filter();
                Task::none()
            }
            Message::SelectAlbum(album_id) => {
                self.focus = AppFocus::Library;
                let now = std::time::Instant::now();
                let is_double_click = if let Some((last_id, last_time)) = &self.last_album_header_click {
                    last_id == &album_id && now.duration_since(*last_time) < std::time::Duration::from_millis(500)
                } else {
                    false
                };

                if is_double_click {
                    self.last_album_header_click = None;
                    return self.update(Message::ToggleAlbumExpansion(album_id));
                } else {
                    self.last_album_header_click = Some((album_id.clone(), now));
                    self.library_manager.selected_album = Some(album_id);
                    self.library_manager.selected_song_idx = None;
                    self.library_manager.selected_header = None; // Limpiar cabecera
                    self.library_manager.selected_item_hint = None; // Reset hint tras selección de álbum
                    self.update_selection_stats();
                    Task::none()
                }
            }
            Message::SelectSong(idx) => {
                self.focus = AppFocus::Library;
                if let Some(i) = idx {
                    let songs_opt = if self.library_manager.view_mode == crate::gui::library::LibraryViewMode::Grid {
                        self.library_manager.expanded_album_songs.as_ref()
                    } else {
                        self.library_manager.filtered_songs.as_ref()
                    };

                    if let Some(songs) = songs_opt {
                        if let Some(song) = songs.get(i) {
                            let artist = song.artist.clone().or(song.album_artist.clone()).unwrap_or_else(|| "Artista Desconocido".to_string());
                            self.library_manager.artist_last_selection.insert(artist, i);
                        }
                    }
                }
                self.library_manager.selected_song_idx = idx;
                self.library_manager.selected_header = None;
                // Mantenemos lógica de álbum seleccionado para grid, pero NO para listas
                if self.library_manager.is_list_mode() {
                    self.library_manager.selected_album = None;
                } else if idx.is_some() {
                    if let Some(expanded) = self.library_manager.expanded_album.clone() {
                        self.library_manager.selected_album = Some(expanded);
                    }
                }
                self.update_selection_stats();
                Task::none()
            }
            Message::SelectArtistHeader(name) => {
                self.focus = AppFocus::Library;
                let now = std::time::Instant::now();
                let is_double_click = if let Some((last_name, last_time)) = &self.last_artist_header_click {
                    last_name == &name && now.duration_since(*last_time) < std::time::Duration::from_millis(500)
                } else {
                    false
                };

                if is_double_click {
                    self.last_artist_header_click = None;
                    return self.update(Message::ToggleArtistExpansion(name));
                } else {
                    self.last_artist_header_click = Some((name.clone(), now));
                    self.library_manager.selected_header = Some(name);
                    self.library_manager.selected_song_idx = None;
                    self.library_manager.selected_album = None;
                    self.update_selection_stats();
                    self.get_library_scroll_task(false)
                }
            }
            Message::LibraryKeyNav(dir, modifiers) => {
                self.focus = AppFocus::Library;
                use crate::gui::library::LibraryNavDir;
                // Modo Lista Simple/Detallada/Thumbnail: Navegación Vertical Unificada
                if self.library_manager.is_list_mode() {
                    // Flecha Izquierda/Derecha -> Colapsar/Expandir
                    if dir == LibraryNavDir::Left || dir == LibraryNavDir::Right {
                        // Priority 0: En DetailedList, si estamos en una canción y pulsamos izquierda, contraer su álbum.
                        if dir == LibraryNavDir::Left && self.library_manager.view_mode == crate::gui::library::LibraryViewMode::DetailedList {
                            if let Some(song_idx) = self.library_manager.selected_song_idx {
                                if let Some(songs) = &self.library_manager.filtered_songs {
                                    if let Some(song) = songs.get(song_idx) {
                                        if let Some(album_name) = &song.album {
                                            if !self.library_manager.collapsed_albums.contains(album_name) {
                                                return self.update(Message::ToggleAlbumExpansion(album_name.clone()));
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Priority 1: If an Album is selected, toggle it
                        if let Some(album_id) = self.library_manager.selected_album.clone() {
                            let is_col = self.library_manager.collapsed_albums.contains(&album_id);
                            if (dir == LibraryNavDir::Left && !is_col) || (dir == LibraryNavDir::Right && is_col) {
                                return self.update(Message::ToggleAlbumExpansion(album_id));
                            }
                        }

                        // Priority 2: Toggle Artist
                        if let Some(artist) = self.library_manager.get_artist_to_toggle(dir) {
                            return self.update(Message::ToggleArtistExpansion(artist));
                        }
                        return Task::none();
                    }

                // Navegación Vertical Unificada
                if let Some((new_item, item_y, item_h)) = self.library_manager.handle_key_nav(dir) {
                    self.library_manager.selected_header = None;
                    self.library_manager.selected_song_idx = None;
                    self.library_manager.selected_album = None;
                    // Al seleccionar algo en la biblioteca, limpiamos la selección de la playlist
                    self.playlist_manager.selected_song_idx = None;

                    self.library_manager.selected_item_hint = Some((item_y, item_h));

                    match new_item {
                        crate::gui::library::LibraryListItem::Artist(name) => {
                            self.library_manager.selected_header = Some(name);
                        }
                        crate::gui::library::LibraryListItem::Album(name) => {
                            self.library_manager.selected_album = Some(name);
                        }
                        crate::gui::library::LibraryListItem::Song(idx) => {
                            self.library_manager.selected_song_idx = Some(idx);
                        }
                    }
                }
                
                self.library_manager.update_selection_memory();
                self.update_selection_stats();
                return self.library_manager.get_scroll_task(false);
            } else {
                // Modo Grid o Celdas
                // Obtener álbumes para navegación (Filtrados si hay búsqueda, de lo contrario todos)
                let albums_ref = self.library_manager.filtered_albums.as_deref()
                    .or(self.library_manager.cached_albums.as_deref())
                    .unwrap_or(&[]);
                let total = albums_ref.len();
                if total == 0 { return Task::none(); }
                
                // Restaurar selección si no hay ninguna activa y se presiona una tecla
                if self.library_manager.selected_album.is_none() && self.library_manager.selected_song_idx.is_none() {
                    if let Some(_last_album) = self.library_manager.last_selected_album.clone() {
                        if let Some(sel) = self.library_manager.selected_album.clone() {
                            if self.library_manager.expanded_album.as_deref() != Some(sel.as_str()) {
                                return self.update(Message::ToggleAlbumExpansion(sel));
                            }
                        }
                    }
                }

                // Alt + Arriba -> Colapsar álbum seleccionado
                if dir == LibraryNavDir::Up && modifiers.alt() {
                    if let Some(sel) = self.library_manager.selected_album.clone() {
                        if self.library_manager.expanded_album.as_deref() == Some(sel.as_str()) {
                            return self.update(Message::ToggleAlbumExpansion(sel));
                        }
                    }
                }

                let is_expanded = self.library_manager.expanded_album.is_some();
                let expanded_name = self.library_manager.expanded_album.as_deref();
                let selected_name = self.library_manager.selected_album.as_deref();
                
                // Caso especial: El álbum seleccionado está expandido -> Navegar hacia canciones o colapsar
                if is_expanded && selected_name == expanded_name {
                    let song_count = self.library_manager.expanded_album_songs.as_ref().map(|s| s.len()).unwrap_or(0);
                    
                    if dir == LibraryNavDir::Down && self.library_manager.selected_song_idx.is_none() {
                         // De Álbum a Primera Canción
                         if song_count > 0 {
                             self.library_manager.selected_song_idx = Some(0);
                             return Task::none();
                         }
                    } else if dir == LibraryNavDir::Up && self.library_manager.selected_song_idx == Some(0) {
                        // De Primera Canción a Álbum y COLAPSAR
                        self.library_manager.selected_song_idx = None;
                        if let Some(exp) = self.library_manager.expanded_album.clone() {
                            return self.update(Message::ToggleAlbumExpansion(exp));
                        }
                    } else if self.library_manager.selected_song_idx.is_some() && (dir == LibraryNavDir::Up || dir == LibraryNavDir::Down) {
                        // Navegación normal entre canciones
                        let current = self.library_manager.selected_song_idx.unwrap_or(0);
                        let new_idx = match dir {
                             LibraryNavDir::Up => current.saturating_sub(1),
                             LibraryNavDir::Down => (current.saturating_add(1)).min(song_count - 1),
                             _ => current,
                        };
                        self.library_manager.selected_song_idx = Some(new_idx);

                        // Auto-scroll (Grid): Usar albums_ref para encontrar la posición correcta
                        let per_row = self.library_manager.albums_per_row.get().max(1);
                        if let Some(album_id) = &self.library_manager.selected_album {
                            if let Some(album_idx) = albums_ref.iter().position(|a| {
                                if album_id.contains('|') {
                                    let parts: Vec<&str> = album_id.split('|').collect();
                                    a.0 == parts[1] && a.2 == parts[0]
                                } else {
                                    &a.0 == album_id
                                }
                            }) {
                                let row_of_album = album_idx / per_row;
                                // Height of each grid row (album cards) + header + song row height
                                let album_card_top = row_of_album as f32 * 252.0;
                                let song_header_h = 40.0; // album header inside expanded
                                let song_row_h = 32.0;
                                let song_y = album_card_top + 252.0 + song_header_h + new_idx as f32 * song_row_h;
                                let song_bottom = song_y + song_row_h;

                                if let Some(viewport) = &self.library_manager.last_viewport {
                                    let scroll_id = crate::gui::library::LIBRARY_SCROLL_ID.clone();
                                    let view_min = viewport.absolute_offset().y;
                                    let view_max = view_min + viewport.bounds().height;

                                    if song_y < view_min + 64.0 {
                                        return iced::widget::operation::scroll_to(scroll_id,
                                            iced::widget::operation::AbsoluteOffset { x: 0.0, y: (song_y - 64.0).max(0.0) });
                                    } else if song_bottom > view_max - song_row_h {
                                        let offset = (song_bottom - viewport.bounds().height + song_row_h).max(0.0);
                                        return iced::widget::operation::scroll_to(scroll_id,
                                            iced::widget::operation::AbsoluteOffset { x: 0.0, y: offset });
                                    }
                                }
                            }
                        }
                        return Task::none();
                    }
                }


                // Navegación entre álbumes
                let per_row = self.library_manager.albums_per_row.get().max(1);
                let current_idx = selected_name.and_then(|sel| {
                    if sel.contains('|') {
                        let parts: Vec<&str> = sel.split('|').collect();
                        let target_artist = parts[0];
                        let target_hash = parts[1];
                        albums_ref.iter().position(|a| a.0 == target_hash && a.2 == target_artist)
                    } else {
                        albums_ref.iter().position(|a| a.0 == sel)
                    }
                }).unwrap_or(0);
                
                let new_idx = match dir {
                    LibraryNavDir::Left => current_idx.saturating_sub(1),
                    LibraryNavDir::Right => (current_idx + 1).min(total - 1),
                    LibraryNavDir::Up => current_idx.saturating_sub(per_row),
                    LibraryNavDir::Down => (current_idx + per_row).min(total - 1),
                    LibraryNavDir::None => current_idx,
                };
                
                if let Some(album) = albums_ref.get(new_idx) {
                    let album_id = &album.0;
                    let artist = &album.2;
                    // ID compuesto para Grid (Soporte Recopilatorios)
                    self.library_manager.selected_album = Some(format!("{}|{}", artist, album_id));
                    self.library_manager.selected_song_idx = None;
                }
                
                // Smart Auto-scroll (Keep in View)
                let actual_new_idx = self.library_manager.selected_album.as_deref().and_then(|sel| {
                    if sel.contains('|') {
                        let parts: Vec<&str> = sel.split('|').collect();
                        let target_artist = parts[0];
                        let target_hash = parts[1];
                        albums_ref.iter().position(|a| a.0 == target_hash && a.2 == target_artist)
                    } else {
                        albums_ref.iter().position(|a| a.0 == sel)
                    }
                }).unwrap_or(new_idx);

                let row_idx = actual_new_idx / per_row.max(1);
                let item_top = row_idx as f32 * 252.0;    // Alto de fila calculado
                let item_bottom = item_top + 252.0;
                
                // Llamamos a las estadísticas después de usar 'albums' para evitar conflictos de Borrow Checker
                self.update_selection_stats();

                if let Some(viewport) = &self.library_manager.last_viewport {
                    let scroll_id = crate::gui::library::LIBRARY_SCROLL_ID.clone();
                    let margin = 0.0;
                    if viewport.bounds().height > 0.1 { // Evitar división por cero o scroll en un viewport inválido
                        let view_min = viewport.absolute_offset().y;
                        let view_max = view_min + viewport.bounds().height;

                        if item_top < view_min + margin {
                            return iced::widget::operation::scroll_to(scroll_id, iced::widget::operation::AbsoluteOffset { x: 0.0, y: (item_top - margin).max(0.0) });
                        } else if item_bottom > view_max - margin {
                            let offset = (item_bottom - viewport.bounds().height + margin).max(0.0);
                            return iced::widget::operation::scroll_to(scroll_id, iced::widget::operation::AbsoluteOffset { x: 0.0, y: offset });
                        }
                    }
                    Task::none()
                } else {
                    let scroll_id = crate::gui::library::LIBRARY_SCROLL_ID.clone();
                    iced::widget::operation::scroll_to(scroll_id, iced::widget::operation::AbsoluteOffset { x: 0.0, y: item_top })
                }
            }
        }
        Message::GlobalKeyDown(key, modifiers) => {
            use iced::keyboard::Key;
                use iced::keyboard::key::Named;
                use crate::gui::library::LibraryNavDir;

                match key {
                    Key::Named(Named::ArrowUp) => {
                        if self.focus == AppFocus::Playlist {
                             self.playlist_manager.handle_key_nav(LibraryNavDir::Up);
                             if let Some(idx) = self.playlist_manager.selected_song_idx {
                                 return self.execute_playlist_autoscroll(idx);
                             }
                             Task::none()
                        } else {
                             self.update(Message::LibraryKeyNav(LibraryNavDir::Up, modifiers))
                        }
                    }
                    Key::Named(Named::ArrowDown) => {
                        if self.focus == AppFocus::Playlist {
                             self.playlist_manager.handle_key_nav(LibraryNavDir::Down);
                             if let Some(idx) = self.playlist_manager.selected_song_idx {
                                 return self.execute_playlist_autoscroll(idx);
                             }
                             Task::none()
                        } else {
                             self.update(Message::LibraryKeyNav(LibraryNavDir::Down, modifiers))
                        }
                    }
                    Key::Named(Named::ArrowLeft) => {
                        if self.focus == AppFocus::Playlist {
                             self.playlist_manager.handle_key_nav(LibraryNavDir::Left);
                             Task::none()
                        } else {
                             self.update(Message::LibraryKeyNav(LibraryNavDir::Left, modifiers))
                        }
                    }
                    Key::Named(Named::ArrowRight) => {
                        if self.focus == AppFocus::Playlist {
                             self.playlist_manager.handle_key_nav(LibraryNavDir::Right);
                             Task::none()
                        } else {
                             self.update(Message::LibraryKeyNav(LibraryNavDir::Right, modifiers))
                        }
                    }
                    Key::Named(Named::Enter) => {
                        if self.focus == AppFocus::Playlist {
                            // Reproducción inmediata para Enter en Playlist
                            if let Some(idx) = self.playlist_manager.selected_song_idx {
                                if let Some(song) = self.playlist_manager.get_song_at_linear_index(idx) {
                                    let path = song.file_path.clone();
                                    self.sync_player_art();
                                    if let Err(e) = self.audio_manager.load_file(&path) {
                                        tracing::error!("Error reproducidendo archivo con Enter: {}", e);
                                    } else {
                                        self.audio_manager.set_playing(true);
                                        self.sync_player_art();
                                        self.audio_manager.play();
                                        self.playlist_manager.playing_song_idx = Some(idx);
                                        self.persist_playlist_state();
                                    }
                                }
                            }
                            Task::none()
                        } else {
                             self.update(Message::PlayLibrarySelection)
                        }
                    }
                    Key::Named(Named::Delete) => {
                        if self.focus == AppFocus::Playlist {
                             Task::none()
                        } else {
                             self.update(Message::LibraryDeleteSelection)
                        }
                    }
                    Key::Named(Named::Space) => {
                        if self.focus == AppFocus::Playlist {
                            if let Some(sel) = self.playlist_manager.selected_song_idx {
                                use crate::gui::playlist::PlaylistItemType;
                                if let Some(info) = self.playlist_manager.get_item_info_at_linear_index(sel) {
                                    match info {
                                        PlaylistItemType::Song(..) => return self.update(Message::ToggleSongEnabled(sel)),
                                        PlaylistItemType::Separator(..) => return self.update(Message::ToggleGroupEnabled(sel)),
                                    }
                                }
                            }
                        }
                        Task::none()
                    }
                    _ => Task::none()
                }
            }
            Message::LibraryScroll(viewport) => {
                self.library_manager.last_viewport = Some(viewport);
                Task::none()
            }
            Message::LibraryDeselect => {
                self.library_manager.last_selected_album = self.library_manager.selected_album.clone();
                self.library_manager.last_selected_song_idx = self.library_manager.selected_song_idx;
                
                self.library_manager.selected_album = None;
                self.library_manager.selected_song_idx = None;
                self.library_manager.selected_header = None;
                self.library_manager.selected_item_hint = None;
                
                self.update_selection_stats();
                Task::none()
            }
            Message::LibraryDeleteSelection => {
                enum PendingDelete {
                    Song(String, String), // album_name, file_path
                    Album(String),       // album_name
                }
                let mut pending = None;
                
                // Paso 1: Eliminar del CACHÉ LOCAL inmediatamente para evitar parpadeo visual
                {
                    if self.library_manager.is_list_mode() || self.library_manager.selected_song_idx.is_some() {
                        if let Some(song_idx) = self.library_manager.selected_song_idx {
                            if let Some(song) = self.library_manager.get_song_by_global_idx(song_idx) {
                                let path = song.full_file_path.clone();
                                let al_name = song.album.clone().unwrap_or_else(|| "Desconocido".to_string());
                                pending = Some(PendingDelete::Song(al_name, path.clone()));
                                
                                // Borrar localmente por ruta, no por índice ciego
                                if let Some(all) = &mut self.library_manager.cached_all_songs {
                                    all.retain(|s| s.full_file_path != path);
                                }
                                if let Some(filt) = &mut self.library_manager.filtered_songs {
                                    filt.retain(|s| s.full_file_path != path);
                                }
                                self.library_manager.selected_song_idx = None;
                                self.library_manager.apply_filter();
                            }
                        }
                    } else if let Some(album_sample_path) = self.library_manager.selected_album.clone() {
                        if let Some(albums) = &mut self.library_manager.cached_albums {
                            albums.retain(|a| a.0 != album_sample_path);
                        }
                        pending = Some(PendingDelete::Album(album_sample_path));
                        self.library_manager.selected_album = None;
                    }
                }

                // Paso 2: Borrar de la DB y refrescar estadísticas
                if let Some(p) = pending {
                    if let Ok(db) = self.database.lock() {
                        match p {
                            PendingDelete::Song(_, path) => { let _ = db.delete_song(&path); },
                            PendingDelete::Album(sample_path) => {
                                let _ = db.delete_album_group(&sample_path);
                            },
                        }
                        // Actualización inmediata de estadísticas
                        if let Ok((songs, albums, duration, size, artists)) = db.get_library_stats() {
                            self.library_manager.total_songs = songs;
                            self.library_manager.total_albums = albums;
                            self.library_manager.total_artists = artists;
                            self.library_manager.total_duration_secs = duration;
                            self.library_manager.total_size_bytes = size;
                        }
                    }
                    // Forzar recarga de álbumes (sin parpadeo porque ya limpiamos arriba)
                    self.player_ui_state.tick_count = 0; 
                    self.update_selection_stats();
                    return self.update(Message::Tick);
                }

                Task::none()
            }
            Message::ChangeLibraryViewMode(mode) => {
                // Limpiar caché negativa al cambiar de vista
                crate::utils::covers::clear_all_cover_cache();
                
                self.library_manager.view_mode = mode;
                self.library_manager.view_menu_open = false;
                
                if mode == crate::gui::library::LibraryViewMode::Grid {
                    // Al cambiar a Grid: asegurar que cached_albums esté cargado
                    if self.library_manager.cached_albums.is_none() {
                        if let Ok(db) = self.database.lock() {
                            if let Ok(albums) = db.get_grid_items_by_artist() {
                                self.library_manager.cached_albums = Some(albums);
                            }
                        }
                    }
                } else {
                    // Al cambiar a cualquier vista de lista: asegurar que las canciones estén cargadas
                    if self.library_manager.cached_all_songs.is_none() {
                        if let Ok(db) = self.database.lock() {
                            if let Ok(mut songs) = db.get_all_songs() {
                                if self.library_manager.sort_column.is_some() {
                                    self.library_manager.sort_songs(&mut songs);
                                }
                                self.library_manager.cached_all_songs = Some(songs);
                            }
                        }
                    }
                }
                
                self.library_manager.apply_filter();
                    
                // Liberar solo datos del Grid que no necesitamos en lista
                self.library_manager.expanded_album = None;
                self.library_manager.expanded_album_songs = None;
                
                Task::none()
            }
            Message::WindowResized(w, h) => {
                self.window_size = (w, h);
                // La biblioteca ocupa todo el ancho menos el panel izquierdo (~520px) y filtros (~202px)
                let sidebar_w = 722.0_f32;
                self.library_manager.library_area_width = (w as f32 - sidebar_w).max(202.0);
                Task::none()
            }
            Message::ToggleLibraryViewDropdown => {
                let next = match self.library_manager.view_mode {
                    crate::gui::library::LibraryViewMode::Grid => crate::gui::library::LibraryViewMode::DetailedList,
                    crate::gui::library::LibraryViewMode::DetailedList => crate::gui::library::LibraryViewMode::ThumbnailList,
                    crate::gui::library::LibraryViewMode::ThumbnailList => crate::gui::library::LibraryViewMode::SimpleList,
                    crate::gui::library::LibraryViewMode::SimpleList => crate::gui::library::LibraryViewMode::Grid,
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
                if let Some(idx) = self.playlist_manager.selected_song_idx {
                    if let Some(song) = self.playlist_manager.get_song_at_linear_index(idx) {
                        let path = song.file_path.clone();
                        let s_id = song.song_id;
                        self.sync_player_art();
                        if let Err(e) = self.audio_manager.load_file(&path) {
                            tracing::error!("Error reproduciendo archivo de playlist: {}", e);
                        } else {
                            self.audio_manager.play();
                            self.playlist_manager.playing_song_idx = Some(idx);
                            self.playlist_manager.notify_manual_play(s_id);
                            self.persist_playlist_state();
                        }
                        return Task::none();
                    }
                }

                // Si no, proceder con la lógica de selección de la biblioteca
                let songs_opt = if self.library_manager.view_mode == crate::gui::library::LibraryViewMode::Grid {
                    self.library_manager.expanded_album_songs.as_ref()
                } else {
                    self.library_manager.filtered_songs.as_ref()
                };

                if let (Some(song_idx), Some(songs)) = (self.library_manager.selected_song_idx, songs_opt) {
                    if let Some(song) = songs.get(song_idx) {
                        self.persist_playlist_state();
                        return self.update(Message::AddSongToPlaylist(song.clone()));
                    }
                } else if let Some(album_id) = &self.library_manager.selected_album {
                    // Soporte para identidades compuestas en Grid (Artista|Hash)
                    let (target_artist, target_hash) = if album_id.contains('|') {
                        let parts: Vec<&str> = album_id.split('|').collect();
                        (Some(parts[0].to_string()), parts[1].to_string())
                    } else {
                        (None, album_id.clone())
                    };

                    // Buscar álbum en self.library_manager.filtered_songs (Listas)
                    if let Some(songs) = &self.library_manager.filtered_songs {
                        let alb_songs: Vec<_> = songs.iter()
                            .filter(|s| {
                                let _match_hash = s.album_id == 0 || true; // La lista suele filtrar por nombre, pero el Grid por Hash
                                // Para simplificar en listas, si no hay hash en el item, usamos el nombre (album_id es hash en Grid)
                                let name_match = s.album.as_deref() == Some(&target_hash) || s.album.as_deref() == Some(album_id);
                                let artist_match = target_artist.is_none() || s.artist.as_deref() == target_artist.as_deref();
                                name_match && artist_match
                            })
                            .cloned()
                            .collect();
                        if !alb_songs.is_empty() {
                            self.persist_playlist_state();
                            return self.update(Message::PlayAlbum(alb_songs));
                        }
                    }

                    // Fallback a db si no se encontró en la lista actual o es Grid sin canciones cacheadas
                    let mut db_songs_to_play = None;
                    if let Ok(db) = self.database.lock() {
                        let res = if let Some(artist) = target_artist {
                            db.get_songs_by_album_and_artist(&target_hash, &artist)
                        } else {
                            db.get_songs_by_album(&target_hash)
                        };
                        
                        if let Ok(songs) = res {
                            if !songs.is_empty() { db_songs_to_play = Some(songs); }
                        }
                    }
                    if let Some(songs) = db_songs_to_play {
                        self.persist_playlist_state();
                        return self.update(Message::PlayAlbum(songs));
                    }
                } else if let Some(artist_name) = &self.library_manager.selected_header {
                    // Buscar artista en self.library_manager.artist_groups
                    if let Some(group) = self.library_manager.artist_groups.iter().find(|g| &g.name == artist_name) {
                        let art_songs = group.songs.clone();
                        if !art_songs.is_empty() {
                            self.persist_playlist_state();
                            return self.update(Message::PlayAlbum(art_songs));
                        }
                    }
                } else if !self.library_manager.is_list_mode() {
                    if let Some(songs) = &self.library_manager.expanded_album_songs {
                        if !songs.is_empty() {
                            self.persist_playlist_state();
                            return self.update(Message::PlayAlbum(songs.clone()));
                        }
                    }
                }
                Task::none()
            }
            Message::PlayLibraryAll => Task::none(), // TODO
            Message::SetWindowId(id) => {
                self.window_id = Some(id);
                Task::none()
            }
            Message::OpenFolderPicker => {
                let scanner_arc = self.scanner.clone();
                Task::perform(async move {
                    rfd::FileDialog::new().pick_folder()
                }, move |folder| {
                    if let Some(f) = folder {
                        let path_str = f.to_string_lossy().to_string();
                        scanner_arc.scan_folder_async(path_str);
                    }
                    Message::NoOp
                })
            }
            Message::OpenPlaylistFilePicker => {
                Task::perform(async move {
                    rfd::FileDialog::new()
                        .add_filter("Lista de reproducción", &["m3u", "m3u8"])
                        .pick_file()
                }, |file| {
                    if let Some(f) = file {
                        Message::ImportPlaylistFile(f.to_string_lossy().to_string())
                    } else {
                        Message::NoOp
                    }
                })
            }

            Message::ImportPlaylistFile(path) => {
                let db_arc = self.database.clone();
                let path_clone = path.clone();
                
                // Usamos perform para no bloquear la UI si el M3U es muy grande
                Task::perform(async move {
                    crate::db::scanner::Scanner::import_m3u(&db_arc, &path_clone)
                }, |playlist_id| {
                    if let Some(id) = playlist_id {
                        Message::SwitchPlaylist(id, true)
                    } else {
                        Message::NoOp
                    }
                })
            }
            Message::ToggleFilterMenu => {
                self.filters_manager.menu_open = !self.filters_manager.menu_open;
                Task::none()
            }
            Message::ChangeGeneralFilter(ft) => {
                self.focus = AppFocus::Library;
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
                scroll_to(LIBRARY_SCROLL_ID.clone(), AbsoluteOffset { x: 0.0, y: 0.0 })
            }
            Message::SelectSubfilter(sub) => {
                self.focus = AppFocus::Library;
                if sub == self.filters_manager.selected_subfilter {
                     self.filters_manager.selected_subfilter = None;
                } else {
                     self.filters_manager.selected_subfilter = sub;
                }
                self.filters_manager.selected_tree_node = None;
                
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
                scroll_to(LIBRARY_SCROLL_ID.clone(), AbsoluteOffset { x: 0.0, y: 0.0 })
            }
            Message::ToggleTreeNode(name) => {
                if self.filters_manager.expanded_nodes.contains(&name) {
                    self.filters_manager.expanded_nodes.remove(&name);
                } else {
                    self.filters_manager.expanded_nodes.insert(name.clone());
                }
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
                let sub_val = parts.get(2).copied();  // Level 2 (e.g. Artist name)
                let third_val = parts.get(3).copied(); // Level 3 (e.g. Album name)

                match self.filters_manager.current_filter {
                    crate::gui::library_filters::FilterType::Artist => {
                        if let Some(art) = main_val { self.library_manager.filter_artist = Some(art.to_string()); }
                        if let Some(alb) = sub_val { self.library_manager.filter_album = Some(alb.to_string()); }
                    }
                    crate::gui::library_filters::FilterType::Album => {
                        if let Some(alb) = main_val { self.library_manager.filter_album = Some(alb.to_string()); }
                        if let Some(art) = sub_val { self.library_manager.filter_artist = Some(art.to_string()); }
                    }
                    crate::gui::library_filters::FilterType::Genre => {
                        if let Some(gnr) = main_val { self.library_manager.filter_genre = Some(gnr.to_string()); }
                        if let Some(art) = sub_val { self.library_manager.filter_artist = Some(art.to_string()); }
                        if let Some(alb) = third_val { self.library_manager.filter_album = Some(alb.to_string()); }
                    }
                    crate::gui::library_filters::FilterType::Year => {
                        if let Some(yr) = main_val { self.library_manager.filter_year = Some(yr.to_string()); }
                        if let Some(art) = sub_val { self.library_manager.filter_artist = Some(art.to_string()); }
                        if let Some(alb) = third_val { self.library_manager.filter_album = Some(alb.to_string()); }
                    }
                    crate::gui::library_filters::FilterType::Folder => {
                        // Manejado por Message::SelectFolder
                    }
                }
                
                self.library_manager.apply_filter();
                self.library_manager.last_viewport = None;
                scroll_to(LIBRARY_SCROLL_ID.clone(), AbsoluteOffset { x: 0.0, y: 0.0 })
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
                scroll_to(LIBRARY_SCROLL_ID.clone(), AbsoluteOffset { x: 0.0, y: 0.0 })
            }
            Message::FilterSearchChanged(q) => {
                self.focus = AppFocus::Library;
                self.filters_manager.search_query = q;
                self.filters_manager.apply_view();
                Task::none()
            }
            Message::ToggleAudioCenter => {
                self.audio_center_manager.open = !self.audio_center_manager.open;
                if self.audio_center_manager.open && self.audio_center_manager.first_open {
                    self.audio_center_manager.sync_from_engine(&self.audio_manager);
                    self.audio_center_manager.first_open = false;
                }
                Task::none()
            }
            Message::AudioCenterMsg(ac_msg) => {
                self.audio_center_manager.update(ac_msg, &self.audio_manager);
                Task::none()
            }
            Message::PlayerHoverZone(zone) => {
                self.player_ui_state.hover_zone = zone;
                Task::none()
            }
            Message::PlayerScroll(delta) => {
                let current_vol = self.audio_manager.get_state().volume;
                let direction = if delta > 0.0 { 1.0 } else { -1.0 };
                let vol_percent = current_vol * 100.0;
                let new_vol_percent = (vol_percent + direction * 5.0).clamp(0.0, 100.0);
                self.audio_manager.set_volume(new_vol_percent / 100.0);
                self.player_ui_state.showing_volume = Some(new_vol_percent);
                self.player_ui_state.volume_tick_id = self.player_ui_state.volume_tick_id.wrapping_add(1);
                self.player_ui_state.volume_clearing = false;
                let current_tick = self.player_ui_state.volume_tick_id;
                
                iced::Task::perform(
                    async { tokio::time::sleep(std::time::Duration::from_millis(1500)).await },
                    move |_| Message::PlayerVolumeTimeout(current_tick)
                )
            }
            Message::PlayerVolumeTimeout(tick) => {
                if self.player_ui_state.volume_tick_id == tick {
                    self.player_ui_state.showing_volume = None;
                    self.player_ui_state.volume_clearing = false;
                }
                Task::none()
            }
            Message::ToggleMenu => {
                self.player_ui_state.is_menu_open = !self.player_ui_state.is_menu_open;
                Task::none()
            }
            Message::GlobalClick => {
                if self.player_ui_state.is_menu_open {
                    self.player_ui_state.is_menu_open = false;
                }
                self.context_menu = None;
                Task::none()
            }
            Message::PlayerWindowAction(action) => {
                match action {
                    crate::gui::player::WindowAction::Minimize => {
                        if let Some(id) = self.window_id {
                            iced::window::minimize(id, true).map(|_: ()| Message::Tick)
                        } else {
                            Task::none()
                        }
                    }
                    crate::gui::player::WindowAction::Maximize => {
                        if let Some(id) = self.window_id {
                            iced::window::toggle_maximize(id).map(|_: ()| Message::Tick)
                        } else {
                            Task::none()
                        }
                    }
                    crate::gui::player::WindowAction::Close => {
                        std::process::exit(0);
                    }
                }
            }
            Message::PlayerMouseMoved(pos) => {
                self.last_mouse_pos = pos;
                let is_resizing = self.library_manager.resizing_column.is_some();
                if is_resizing {
                    let col = self.library_manager.resizing_column.unwrap();
                    let start_x = self.library_manager.resizing_start_x;
                    let start_w = self.library_manager.resizing_start_w;
                    
                    let diff = pos.x - start_x;
                    let new_w = (start_w as f32 + diff).max(30.0) as u16;
                    
                    self.library_manager.column_widths.insert(col, new_w);
                }
                
                self.player_ui_state.mouse_pos = Some(pos);
                self.wake_up_controls(true)
            }
            Message::PlaylistScrolled(viewport) => {
                self.playlist_manager.last_viewport = Some(viewport.bounds());
                // El viewport de iced 0.14 en on_scroll contiene bounds() y absolute_offset()
                // Guardamos el rectángulo que representa la ventana visible
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
            Message::GlobalMouseRelease => {
                if self.library_manager.resizing_column.is_some() {
                    self.library_manager.resizing_column = None;
                }
                Task::none()
            }
            Message::PlayerActivityTimeout(_tick) => {
                Task::none()
            }
            Message::InitStartup => {
                // Sincronizar carátula inicial
                self.sync_player_art();
                
                // Forzar auto-scroll a la canción que se restauró
                if let Some(idx) = self.playlist_manager.playing_song_idx {
                    return self.execute_playlist_autoscroll(idx);
                }
                Task::none()
            }
            Message::OpenContextMenu(pos, entries) => {
                self.context_menu = Some((pos, entries));
                Task::none()
            }
            Message::CloseContextMenu => {
                self.context_menu = None;
                Task::none()
            }
            Message::NoOp => Task::none(),
        }
    }

    fn sync_player_art(&mut self) {
        let audio_state = self.audio_manager.get_state();
        
        // 0. No hacer nada si el audio está en transición (path vacío temporalmente)
        if audio_state.path.is_empty() {
            return;
        }
        
        // 1. Verificar si la pista ha cambiado realmente
        if self.player_ui_state.current_art_id == audio_state.path {
             return; 
        }

        // 2. Obtener la ruta AVIF de la canción actual desde la playlist
        let mut new_cover_path = String::new();
        if let Some(idx) = self.playlist_manager.playing_song_idx {
            if let Some(song) = self.playlist_manager.get_song_at_linear_index(idx) {
                if song.file_path == audio_state.path {
                    if let Some(ref cp) = song.cover_path {
                        new_cover_path = cp.clone();
                    }
                }
            }
        }

        // 3. Reusar carátula si es el MISMO álbum (misma ruta de caché)
        if !new_cover_path.is_empty() && new_cover_path == self.player_ui_state.current_cover_path {
            // Mismo álbum — reusar el handle actual, solo actualizar el art_id
            self.player_ui_state.current_art_id = audio_state.path.clone();
            self.player_ui_state.prefetched_next_handle = None;
            return;
        }

        // 4. Usar handle precargado si está disponible
        if let Some(prefetched) = self.player_ui_state.prefetched_next_handle.take() {
            self.player_ui_state.cached_art_handle = Some(prefetched);
            self.player_ui_state.current_art_id = audio_state.path.clone();
            self.player_ui_state.current_cover_path = new_cover_path;
            return;
        }

        // 5. Cargar desde disco (Handle::from_path — sin caché RAM)
        if !new_cover_path.is_empty() {
            if let Some(handle) = crate::utils::covers::load_cover_handle(&new_cover_path) {
                self.player_ui_state.cached_art_handle = Some(handle);
                self.player_ui_state.current_art_id = audio_state.path.clone();
                self.player_ui_state.current_cover_path = new_cover_path;
                return;
            }
        }

        // 6. Fallback RAW (datos embebidos del archivo de audio)
        if let Some(ref art) = audio_state.album_art {
             self.player_ui_state.cached_art_handle = Some(iced::widget::image::Handle::from_bytes(art.clone()));
             self.player_ui_state.current_art_id = audio_state.path.clone();
             self.player_ui_state.current_cover_path.clear();
        } else {
             self.player_ui_state.cached_art_handle = None;
             self.player_ui_state.current_art_id = audio_state.path.clone();
             self.player_ui_state.current_cover_path.clear();
        }
    }

    fn persist_playlist_state(&self) {
        let state = self.audio_manager.get_state();
        let playlist_id = self.playlist_manager.active_playlist_id;
        let last_song_id = self.playlist_manager.playing_song_idx
            .and_then(|idx| self.playlist_manager.get_song_at_linear_index(idx))
            .map(|s| s.song_id);
        let pos = state.current_pos_sec;
        let is_p = state.is_playing;
        let shuffle = self.playlist_manager.shuffle_active;
        let repeat = self.playlist_manager.repeat_mode as i32;
        let (s_pos, s_id) = if let Some(session) = &self.playlist_manager.shuffle_session {
            (session.current_position, Some(session.session_id.clone()))
        } else { (0, None) };

        if let Ok(db) = self.database.lock() {
            let _ = db.update_playlist_persistence(playlist_id, last_song_id, pos, is_p, shuffle, repeat, s_pos, s_id);
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let _ = &self.database; // Hack temporal para el warning
        let _ = &self.scanner;  // Hack temporal para el warning
        
        let player_view = crate::gui::player::view(&self.audio_manager, &self.player_ui_state);
        let playlist_view = crate::gui::playlist::view(&self.playlist_manager, &self.audio_manager);
        let filters_view = crate::gui::library_filters::view(&self.filters_manager);
        let library_view = crate::gui::library::view(&self.library_manager, &self.database, &self.player_ui_state.current_art_id);

        // Apilamos el reproductor (carátula y controles) arriba de la playlist en una sola columna izquierda
        let left_column = iced::widget::column![player_view, playlist_view]
            .width(iced::Length::Shrink)
            .height(iced::Length::Fill)
            .align_x(iced::Alignment::Start);

        let main_row = iced::widget::row![left_column, filters_view, library_view]
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .align_y(iced::Alignment::Start);

        let final_content: Element<'_, Message> = if self.audio_center_manager.open {
            let ac_view = crate::gui::audio_center::view(&self.audio_center_manager, &self.audio_manager);
            
            // Falso modal: fondo transparente para bloqueo de clics
            let modal_bg = iced::widget::mouse_area(
                iced::widget::container(iced::widget::Space::new().width(iced::Length::Fill).height(iced::Length::Fill))
                .style(|_t: &iced::Theme| iced::widget::container::Style::default().background(iced::Color::TRANSPARENT))
            );

            // Contenedor centrado para el Control Center
            let centered_modal = iced::widget::container(ac_view)
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .center_x(iced::Fill)
                .center_y(iced::Fill);

            // Apilamos Interfaz Principal, luego el fondo oscuro, luego el contenido centrado
            let stack = iced::widget::Stack::new()
                .push(main_row)
                .push(modal_bg)
                .push(centered_modal);
                
            stack.width(iced::Length::Fill).height(iced::Length::Fill).into()
        } else {
            main_row.into()
        };

        let app_underlay = iced::widget::mouse_area(
            iced::widget::Space::new().width(iced::Length::Fill).height(iced::Length::Fill)
        )
        .on_press(Message::GlobalClick)
        .interaction(iced::mouse::Interaction::Idle);

        let final_stack = iced::widget::Stack::new()
            .push(app_underlay)
            .push(final_content);

        // --- Capa de Diálogos Modales ---
        let mut final_stack = final_stack;
        
        if self.active_dialog != ActiveDialog::None {
            let dialog_content = match &self.active_dialog {
                ActiveDialog::CreatePlaylist { name } => {
                    crate::gui::widgets::standard_modal(
                        "Nueva lista de reproducción".to_string(),
                        iced::widget::column![
                            iced::widget::text("Escribe el nombre:").size(14).color(COLOR_TEXT_SECONDARY),
                            iced::widget::text_input("Nombre de la lista...", name)
                                .on_input(Message::UpdateDialogInput)
                                .on_submit(Message::ConfirmDialogAction)
                                .padding(10)
                                .size(14)
                                .style(|_t: &iced::Theme, _status: iced::widget::text_input::Status| {
                                    iced::widget::text_input::Style {
                                        background: COLOR_CONTRAST.into(),
                                        border: iced::Border { radius: 4.0.into(), width: 1.0, color: COLOR_ACCENT },
                                        icon: Color::TRANSPARENT,
                                        placeholder: COLOR_TEXT_SECONDARY,
                                        value: COLOR_TEXT_PRIMARY,
                                        selection: COLOR_ACCENT,
                                    }
                                })
                        ].spacing(10).into(),
                        Some(Message::CloseDialog),
                        Some(Message::ConfirmDialogAction),
                        "Crear".to_string(),
                    )
                },
                ActiveDialog::RenamePlaylist { new_name, .. } => {
                    crate::gui::widgets::standard_modal(
                        "Renombrar lista".to_string(),
                        iced::widget::column![
                            iced::widget::text("Nuevo nombre:").size(14).color(COLOR_TEXT_SECONDARY),
                            iced::widget::text_input("Nuevo nombre...", new_name)
                                .on_input(Message::UpdateDialogInput)
                                .on_submit(Message::ConfirmDialogAction)
                                .padding(10)
                                .size(14)
                                .style(|_t: &iced::Theme, _status: iced::widget::text_input::Status| {
                                    iced::widget::text_input::Style {
                                        background: COLOR_CONTRAST.into(),
                                        border: iced::Border { radius: 4.0.into(), width: 1.0, color: COLOR_ACCENT },
                                        icon: Color::TRANSPARENT,
                                        placeholder: COLOR_TEXT_SECONDARY,
                                        value: COLOR_TEXT_PRIMARY,
                                        selection: COLOR_ACCENT,
                                    }
                                })
                        ].spacing(10).into(),
                        Some(Message::CloseDialog),
                        Some(Message::ConfirmDialogAction),
                        "Renombrar".to_string(),
                    )
                },
                ActiveDialog::DeleteConfirm { name, .. } => {
                    crate::gui::widgets::standard_modal(
                        "¿Eliminar lista?".to_string(),
                        iced::widget::text(format!("¿Estás seguro de que quieres eliminar \"{}\"? Esta acción no se puede deshacer.", name))
                            .size(14)
                            .color(COLOR_TEXT_SECONDARY).into(),
                        Some(Message::CloseDialog),
                        Some(Message::ConfirmDialogAction),
                        "Eliminar".to_string(),
                    )
                },
                ActiveDialog::ExportConfirm { name, .. } => {
                    crate::gui::widgets::standard_modal(
                        "Exportar lista".to_string(),
                        iced::widget::text(format!("¿Estas seguro de querer exportar la lista \"{}\" como una carpeta portátil junto con sus archivos de audio.?", name))
                            .size(14)
                            .color(COLOR_TEXT_SECONDARY).into(),
                        Some(Message::CloseDialog),
                        Some(Message::ConfirmDialogAction),
                        "Exportar".to_string(),
                    )
                },
                _ => iced::widget::Space::new().into(),
            };

            // Cálculo de posición dinámica
            let (target_x, target_y) = if let Some(pos) = self.dialog_pos {
                let modal_w = 350.0;
                let modal_h = 240.0;
                let mut x = pos.x - 20.0;
                let mut y = pos.y - 20.0;

                if x + modal_w + 40.0 > self.window_size.0 as f32 {
                    x = (self.window_size.0 as f32 - modal_w - 40.0).max(10.0);
                }
                if y + modal_h + 40.0 > self.window_size.1 as f32 {
                    y = (self.window_size.1 as f32 - modal_h - 40.0).max(10.0);
                }
                (x, y)
            } else {
                (200.0, 200.0)
            };

            // Área de bloqueo invisible - Fill para asegurar cobertura total
            let click_blocker = iced::widget::mouse_area(
                iced::widget::Space::new().width(iced::Length::Fill).height(iced::Length::Fill)
            )
            .on_press(Message::CloseDialog);
            final_stack = final_stack
                .push(click_blocker)
                .push(
                    iced::widget::container(dialog_content)
                        .width(iced::Length::Fill)
                        .height(iced::Length::Fill)
                        .padding(iced::Padding {
                            top: target_y,
                            left: target_x,
                            ..Default::default()
                        })
                        .align_x(iced::Alignment::Start)
                        .align_y(iced::Alignment::Start)
                );
        }

        // --- Renderizar Menú Contextual Nativo ---
        if let Some((pos, entries)) = &self.context_menu {
            let menu_content = crate::gui::widgets::build_context_menu_content(entries.clone());
            
            // Área de bloqueo para el menú
            let menu_blocker = iced::widget::mouse_area(
                iced::widget::Space::new().width(iced::Length::Fill).height(iced::Length::Fill)
            )
            .on_press(Message::GlobalClick);

            // Ajuste de bordes (Stay within window)
            let mut target_x = pos.x;
            let mut target_y = pos.y;
            let menu_w = 260.0;
            let menu_h = (entries.len() as f32 * 32.0) + 10.0; // aprox

            if target_x + menu_w > self.window_size.0 as f32 {
                target_x = (self.window_size.0 as f32 - menu_w - 5.0).max(5.0);
            }
            if target_y + menu_h > self.window_size.1 as f32 {
                target_y = (self.window_size.1 as f32 - menu_h - 5.0).max(5.0);
            }

            final_stack = final_stack
                .push(menu_blocker)
                .push(
                    iced::widget::container(menu_content)
                        .width(iced::Length::Fill)
                        .height(iced::Length::Fill)
                        .padding(iced::Padding {
                            top: target_y,
                            left: target_x,
                            ..Default::default()
                        })
                        .align_x(iced::Alignment::Start)
                        .align_y(iced::Alignment::Start)
                );
        }

        let wrapped_app = helpers::CursorOff::new(
            iced::widget::container(final_stack)
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .center_x(iced::Fill)
                .center_y(iced::Fill)
        );

        wrapped_app.into()
    }

    pub fn theme(&self) -> Theme {
        crate::gui::theme::custom_theme()
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        // Tick dinámico inteligente: 1000ms normal, 100ms en transiciones (inicio/fin canción)
        let tick_interval = if self.audio_manager.get_state().is_playing {
            let state = self.audio_manager.get_state();
            let remaining = state.total_duration_sec - state.current_pos_sec;
            let current = state.current_pos_sec;
            
            // "Zona de Alta Sensibilidad": 0.5s antes de acabar y 0.5s después de empezar
            if (remaining > 0.0 && remaining < 0.5) || current < 0.5 {
                std::time::Duration::from_millis(100)
            } else {
                std::time::Duration::from_millis(1000)
            }
        } else if self.low_resource_mode {
            std::time::Duration::from_millis(1000)
        } else {
            std::time::Duration::from_millis(4000)
        };
        let tick = iced::time::every(tick_interval).map(|_| Message::Tick);
        
        // --- Search Heartbeat (Search-Pulse) ---
        // Emitimos un Tick extra cada 100ms solo si se está buscando algo.
        // Esto fuerza a Iced a reconstruir la vista Grid que suele estancarse en Iced 0.14.
        let search_tick = if !self.library_manager.search_query.is_empty() {
            iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::Tick)
        } else {
            iced::Subscription::none()
        };

        let win_ids = iced::window::open_events().map(Message::SetWindowId);
        let mouse_evs = iced::event::listen_with(|event, _status, _window_id| {
            if let iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) = event {
                Some(Message::PlayerMouseMoved(position))
            } else if let iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) = event {
                Some(Message::GlobalMouseRelease)
            } else if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) = event {
                use iced::keyboard::Key;
                use iced::keyboard::key::Named;
                match key {
                    Key::Named(Named::ArrowUp) | Key::Named(Named::ArrowDown) |
                    Key::Named(Named::ArrowLeft) | Key::Named(Named::ArrowRight) |
                    Key::Named(Named::Enter) | Key::Named(Named::Delete) |
                    Key::Named(Named::Space) => {
                        Some(Message::GlobalKeyDown(key, modifiers))
                    }
                    _ => None,
                }
            } else if let iced::Event::Window(iced::window::Event::Resized(new_size)) = event {
                Some(Message::WindowResized(new_size.width as u32, new_size.height as u32))
            } else {
                None
            }
        });
        iced::Subscription::batch([tick, search_tick, win_ids, mouse_evs])
    }

    fn get_library_scroll_task(&self, force_top: bool) -> Task<Message> {
        self.library_manager.get_scroll_task(force_top)
    }

    fn update_selection_stats(&mut self) {
        self.library_manager.selection_stats = if let Some(song_idx) = self.library_manager.selected_song_idx {
            // Prioridad 1: Si hay una canción seleccionada, mostrar estadísticas de SU ÁLBUM (en cualquier vista)
            let songs_opt = if self.library_manager.view_mode == crate::gui::library::LibraryViewMode::Grid {
                self.library_manager.expanded_album_songs.as_ref()
            } else {
                self.library_manager.filtered_songs.as_ref()
            };

            if let Some(songs) = songs_opt {
                if let Some(song) = songs.get(song_idx) {
                    let album_name = song.album.as_deref().unwrap_or("Desconocido");
                    let artist_name = song.artist.as_deref().unwrap_or("Desconocido");
                    
                    if let Ok(db) = self.database.lock() {
                        let res = if self.library_manager.view_mode == crate::gui::library::LibraryViewMode::Grid {
                            if let Some(composite) = &self.library_manager.selected_album {
                                let parts: Vec<&str> = composite.split('|').collect();
                                if parts.len() >= 2 {
                                    db.get_album_stats_by_hash_and_artist(parts[1], parts[0])
                                } else {
                                    db.get_album_stats_by_hash(composite)
                                }
                            } else {
                                db.get_album_stats_by_hash_and_artist(album_name, artist_name)
                            }
                        } else {
                            db.get_album_stats_by_hash_and_artist(album_name, artist_name)
                        };

                        if let Ok((songs_count, dur, size)) = res {
                            if songs_count > 0 {
                                Some(crate::gui::library::LibraryStats {
                                    songs: songs_count,
                                    albums: 1,
                                    artists: 1,
                                    duration_secs: dur,
                                    size_bytes: size,
                                })
                            } else {
                                let alb_songs: Vec<_> = songs.iter()
                                    .filter(|s| s.album.as_deref() == Some(album_name) && s.artist.as_deref() == Some(artist_name))
                                    .collect();
                                Some(crate::gui::library::LibraryStats {
                                    songs: alb_songs.len() as u64,
                                    albums: 1,
                                    artists: 1,
                                    duration_secs: alb_songs.iter().map(|s| s.duration_secs.unwrap_or(0.0)).sum(),
                                    size_bytes: alb_songs.iter().map(|s| s.size.unwrap_or(0) as f64).sum(),
                                })
                            }
                        } else { None }
                    } else { None }
                } else { None }
            } else { None }
        } else if let Some(album_id) = &self.library_manager.selected_album {
            // Soporte para identidades compuestas en Grid (Artista|Hash)
            let (target_artist, target_hash) = if album_id.contains('|') {
                let parts: Vec<&str> = album_id.split('|').collect();
                (Some(parts[0].to_string()), parts[1].to_string())
            } else {
                (None, album_id.clone())
            };

            // Prioridad 1: Si el álbum está expandido y tenemos sus canciones cargadas (Grid)
            if self.library_manager.expanded_album.as_deref() == Some(album_id) {
                if let Some(songs) = &self.library_manager.expanded_album_songs {
                    let total_dur: f64 = songs.iter().map(|s| s.duration_secs.unwrap_or(0.0)).sum();
                    let total_size: f64 = songs.iter().map(|s| s.size.unwrap_or(0) as f64).sum();
                    Some(crate::gui::library::LibraryStats {
                        songs: songs.len() as u64,
                        albums: 1,
                        artists: 1,
                        duration_secs: total_dur,
                        size_bytes: total_size,
                    })
                } else { None }
            // Prioridad 2: Buscar en DB por hash (Grid sin expansión o Listas con Hash)
            } else if let Ok(db) = self.database.lock() {
                let res = if let Some(artist) = target_artist {
                    db.get_album_stats_by_hash_and_artist(&target_hash, &artist)
                } else {
                    db.get_album_stats_by_hash(&target_hash)
                };

                if let Ok((songs_count, dur, size)) = res {
                    if songs_count > 0 {
                        Some(crate::gui::library::LibraryStats {
                            songs: songs_count,
                            albums: 1,
                            artists: 1,
                            duration_secs: dur,
                            size_bytes: size,
                        })
                    } else { None }
                } else if let Some(songs) = &self.library_manager.filtered_songs {
                    // Prioridad 3: Buscar en filtered_songs por nombre (Listas - Fallback)
                    let alb_songs: Vec<_> = songs.iter()
                        .filter(|s| s.album.as_ref() == Some(&target_hash) || s.album.as_ref() == Some(album_id))
                        .collect();
                    if !alb_songs.is_empty() {
                        Some(crate::gui::library::LibraryStats {
                            songs: alb_songs.len() as u64,
                            albums: 1,
                            artists: 1,
                            duration_secs: alb_songs.iter().map(|s| s.duration_secs.unwrap_or(0.0)).sum(),
                            size_bytes: alb_songs.iter().map(|s| s.size.unwrap_or(0) as f64).sum(),
                        })
                    } else { None }
                } else { None }
            } else { None }
        } else if let Some(artist_name) = &self.library_manager.selected_header {
            if let Some(group) = self.library_manager.artist_groups.iter().find(|g| &g.name == artist_name) {
                 Some(crate::gui::library::LibraryStats {
                     songs: group.songs_count as u64,
                     albums: group.albums_count as u64,
                     artists: 1,
                     duration_secs: group.duration_secs,
                     size_bytes: group.songs.iter().map(|s| s.size.unwrap_or(0) as f64).sum(),
                 })
            } else { None }
        } else {
            None
        };
    }

}

