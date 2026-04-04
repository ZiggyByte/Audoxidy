use iced::{Element, Task, Theme};
use std::sync::{Arc, Mutex};
use crate::audio::AudioManager;
use crate::db::{Database, scanner::Scanner};
use crate::gui::playlist::PlaylistManager;
use crate::gui::library_filters::LibraryFiltersManager;
use crate::gui::library::LibraryManager;
use crate::gui::audio_center::{AudioCenterManager, AudioCenterMessage};
use crate::integrations::media_controls::SystemMediaControls;

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
    GlobalMouseRelease,
    PlayerActivityTimeout(u64),
    GlobalClick,
    WindowResized(u32, u32),
    NoOp,
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
    pub low_resource_mode: bool,
    pub db_needs_refresh: bool,
}

impl AudoxidyApp {
    pub fn new(audio_manager: Arc<AudioManager>) -> (Self, Task<Message>) {
        let db = Database::new().expect("Error crítico al crear/iniciar la base de datos.");
        let mut library_manager = LibraryManager::default();
        if let Ok(albums) = db.get_all_albums() {
            library_manager.cached_albums = Some(albums);
        }
        if let Ok((total_songs, total_albums, total_duration, total_size, total_artists)) = db.get_library_stats() {
            library_manager.total_songs = total_songs;
            library_manager.total_albums = total_albums;
            library_manager.total_artists = total_artists;
            library_manager.total_duration_secs = total_duration;
            library_manager.total_size_bytes = total_size;
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

        (
            Self {
                audio_manager,
                database: database_arc,
                scanner: scanner_arc,
                media_controls,
                playlist_manager: PlaylistManager::default(),
                filters_manager: LibraryFiltersManager::default(),
                library_manager,
                audio_center_manager: AudioCenterManager::default(),
                player_ui_state: crate::gui::player::PlayerUiState::default(),
                window_id: None,
                last_artist_header_click: None,
                last_album_header_click: None,
                low_resource_mode,
                db_needs_refresh: true,
            },
            Task::none(),
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
                        if let Ok(mut albums) = db.get_all_albums() {
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
                            if let Some(next_song) = self.playlist_manager.get_next_song() {
                                if let Some(ref cover_path) = next_song.cover_cache_path {
                                    // Comparar hash del álbum actual vs siguiente
                                    let current_cover = self.playlist_manager.playing_song_idx
                                        .and_then(|idx| self.playlist_manager.lists[self.playlist_manager.active_list_idx].1.get(idx))
                                        .and_then(|s| s.cover_cache_path.clone());
                                    if current_cover.as_deref() != Some(cover_path.as_str()) {
                                        // Álbum diferente: precargar handle
                                        self.player_ui_state.prefetched_next_handle = 
                                            crate::utils::covers::load_cover_handle(cover_path);
                                    }
                                    // Si es el mismo álbum, no hacer nada — se reusa el handle actual
                                }
                            }
                        }
                    }
                }

                if state.eof_reached {
                    self.audio_manager.clear_eof();
                    self.playlist_manager.play_next(&self.audio_manager);
                    self.sync_player_art();
                }
                Task::none()
            }
            Message::PlayPause => {
                let _ = self.audio_manager.toggle_play_pause();
                self.wake_up_controls(false)
            }
            Message::NextTrack => {
                crate::utils::covers::clear_raw_cache();
                self.playlist_manager.play_next(&self.audio_manager);
                self.sync_player_art();
                self.wake_up_controls(false)
            }
            Message::PreviousTrack => {
                crate::utils::covers::clear_raw_cache();
                self.playlist_manager.play_prev(&self.audio_manager);
                self.sync_player_art();
                self.wake_up_controls(false)
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
                self.wake_up_controls(false)
            }
            Message::ToggleRepeat => {
                self.playlist_manager.repeat_mode = (self.playlist_manager.repeat_mode + 1) % 3;
                Task::none()
            }
            Message::ToggleShuffle => {
                self.playlist_manager.shuffle_active = !self.playlist_manager.shuffle_active;
                Task::none()
            }
            Message::AddSongToPlaylist(song) => {
                if self.playlist_manager.lists.is_empty() {
                    self.playlist_manager.lists.push(("Default".to_string(), vec![]));
                }
                
                let playlist_item = crate::gui::playlist::PlaylistItem {
                    title: song.title.clone().unwrap_or_else(|| "Unknown".to_string()),
                    artist: song.artist.clone().unwrap_or_else(|| "Unknown".to_string()),
                    album: song.album.clone().unwrap_or_else(|| "Unknown".to_string()),
                    duration_sec: 0.0,
                    year: song.release_year.clone().unwrap_or_else(|| "".to_string()),
                    path: song.full_file_path.clone(),
                    cover_cache_path: song.compressed_cached_cover_root.clone(),
                };

                self.playlist_manager.lists[self.playlist_manager.active_list_idx].1.push(playlist_item);
                Task::none()
            }
            Message::PlayAlbum(songs) => {
                if self.playlist_manager.lists.is_empty() {
                    self.playlist_manager.lists.push(("Default".to_string(), vec![]));
                }
                let list = &mut self.playlist_manager.lists[self.playlist_manager.active_list_idx].1;
                let start_idx = list.len();
                
                let playlist_items: Vec<crate::gui::playlist::PlaylistItem> = songs.into_iter().map(|s| {
                    crate::gui::playlist::PlaylistItem {
                        title: s.title.clone().unwrap_or_else(|| "Unknown".to_string()),
                        artist: s.artist.clone().unwrap_or_else(|| "Unknown".to_string()),
                        album: s.album.clone().unwrap_or_else(|| "Unknown".to_string()),
                        duration_sec: 0.0, // Default duration, will be updated by active player if needed
                        year: s.release_year.clone().unwrap_or_else(|| "".to_string()),
                        path: s.full_file_path.clone(),
                        cover_cache_path: s.compressed_cached_cover_root.clone(),
                    }
                }).collect();

                list.extend(playlist_items);
                if self.playlist_manager.playing_song_idx.is_none() {
                    return self.update(Message::PlaySongIndex(start_idx));
                }
                Task::none()
            }
            Message::PlaySongIndex(idx) => {
                self.playlist_manager.playing_song_idx = Some(idx);
                if let Some(song) = self.playlist_manager.lists[self.playlist_manager.active_list_idx].1.get(idx).cloned() {
                    self.sync_player_art();
                    if let Err(e) = self.audio_manager.load_file(&song.path) {
                        tracing::error!("Error reproduciendo archivo: {}", e);
                    } else {
                        self.audio_manager.play();
                    }
                }
                Task::none()
            }
            Message::ClearPlaylist => {
                if !self.playlist_manager.lists.is_empty() {
                    self.playlist_manager.lists[self.playlist_manager.active_list_idx].1.clear();
                    self.playlist_manager.playing_song_idx = None;
                    self.audio_manager.stop();
                }
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
                self.library_manager.search_query = q;
                self.library_manager.apply_filter();
                self.library_manager.last_viewport = None;
                Task::none()
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
                        if let Ok(albums) = db.get_all_albums() {
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
                            if let Ok(mut songs) = db.get_songs_by_album(&album_id) {
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
                        // Store exact Y position so get_scroll_task can use it directly
                        // instead of searching by name (which jumps to first occurrence of same album)
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
                                // NOTE: Do NOT overwrite selected_album here
                            }
                        }
                    }
                    
                    self.library_manager.update_selection_memory();
                    self.update_selection_stats();
                    return self.library_manager.get_scroll_task(false);
                }

                // Obtener álbumes para navegación (Filtrados si hay búsqueda, de lo contrario todos)
                let albums_ref = self.library_manager.filtered_albums.as_deref()
                    .or(self.library_manager.cached_albums.as_deref())
                    .unwrap_or(&[]);
                let total = albums_ref.len();
                if total == 0 { return Task::none(); }
                
                // Restaurar selección si no hay ninguna activa y se presiona una tecla
                if self.library_manager.selected_album.is_none() && self.library_manager.selected_song_idx.is_none() {
                    if let Some(last_album) = self.library_manager.last_selected_album.clone() {
                        self.library_manager.selected_album = Some(last_album);
                        self.library_manager.selected_song_idx = self.library_manager.last_selected_song_idx;
                        return Task::none();
                    }
                }

                // Alt + Abajo -> Expandir álbum seleccionado
                if dir == LibraryNavDir::Down && modifiers.alt() {
                    if let Some(sel) = self.library_manager.selected_album.clone() {
                        if self.library_manager.expanded_album.as_deref() != Some(sel.as_str()) {
                            return self.update(Message::ToggleAlbumExpansion(sel));
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
                            if let Some(album_idx) = albums_ref.iter().position(|a| &a.0 == album_id) {
                                let row_of_album = album_idx / per_row;
                                // Height of each grid row (album cards) + header + song row height
                                let album_card_top = row_of_album as f32 * 258.0;
                                let song_header_h = 40.0; // album header inside expanded
                                let song_row_h = 32.0;
                                let song_y = album_card_top + 258.0 + song_header_h + new_idx as f32 * song_row_h;
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
                    albums_ref.iter().position(|a| a.0 == sel)
                }).unwrap_or(0);
                
                let new_idx = match dir {
                    LibraryNavDir::Left => current_idx.saturating_sub(1),
                    LibraryNavDir::Right => (current_idx + 1).min(total - 1),
                    LibraryNavDir::Up => current_idx.saturating_sub(per_row),
                    LibraryNavDir::Down => (current_idx + per_row).min(total - 1),
                    LibraryNavDir::None => current_idx,
                };
                
                if let Some(album) = albums_ref.get(new_idx) {
                    self.library_manager.selected_album = Some(album.0.clone());
                    self.library_manager.selected_song_idx = None;
                }
                
                // Smart Auto-scroll (Keep in View)
                let actual_new_idx = self.library_manager.selected_album.as_deref().and_then(|sel| {
                    albums_ref.iter().position(|a| a.0 == sel)
                }).unwrap_or(new_idx);

                let row_idx = actual_new_idx / per_row.max(1);
                let item_top = row_idx as f32 * 258.0;    // Alto de fila calculado
                let item_bottom = item_top + 258.0;
                
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
            Message::LibraryScroll(viewport) => {
                self.library_manager.last_viewport = Some(viewport);
                Task::none()
            }
            Message::LibraryDeselect => {
                if self.library_manager.selected_album.is_some() {
                    self.library_manager.last_selected_album = self.library_manager.selected_album.clone();
                    self.library_manager.last_selected_song_idx = self.library_manager.selected_song_idx;
                    self.library_manager.selected_album = None;
                    self.library_manager.selected_song_idx = None;
                }
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
                            if let Ok(albums) = db.get_all_albums() {
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
                    self.library_manager.apply_filter();
                }
                    
                // Liberar solo datos del Grid que no necesitamos en lista
                self.library_manager.expanded_album = None;
                self.library_manager.expanded_album_songs = None;
                // NOTA: cached_albums se mantiene (es ligero) para evitar recarga al volver al Grid
                
                Task::none()
            }
            Message::WindowResized(w, _h) => {
                // La biblioteca ocupa todo el ancho menos el panel izquierdo (~520px) y filtros (~180px)
                let sidebar_w = 700.0_f32;
                self.library_manager.library_area_width = (w as f32 - sidebar_w).max(180.0);
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
                // Determinar qué lista de canciones usar (Grid usa las del álbum expandido, resto usa filtered_songs)
                let songs_opt = if self.library_manager.view_mode == crate::gui::library::LibraryViewMode::Grid {
                    self.library_manager.expanded_album_songs.as_ref()
                } else {
                    self.library_manager.filtered_songs.as_ref()
                };

                if let (Some(song_idx), Some(songs)) = (self.library_manager.selected_song_idx, songs_opt) {
                    if let Some(song) = songs.get(song_idx) {
                        return self.update(Message::AddSongToPlaylist(song.clone()));
                    }
                } else if let Some(album_name) = &self.library_manager.selected_album {
                    // Buscar álbum en self.library_manager.filtered_songs
                    if let Some(songs) = &self.library_manager.filtered_songs {
                        let alb_songs: Vec<_> = songs.iter()
                            .filter(|s| s.album.as_ref() == Some(album_name))
                            .cloned()
                            .collect();
                        if !alb_songs.is_empty() {
                            return self.update(Message::PlayAlbum(alb_songs));
                        }
                    }
                    // Fallback a db si no se encontró en la lista actual
                    let mut db_songs_to_play = None;
                    if let Ok(db) = self.database.lock() {
                        if let Ok(songs) = db.get_songs_by_album(album_name) {
                            if !songs.is_empty() { db_songs_to_play = Some(songs); }
                        }
                    }
                    if let Some(songs) = db_songs_to_play {
                        return self.update(Message::PlayAlbum(songs));
                    }
                } else if let Some(artist_name) = &self.library_manager.selected_header {
                    // Buscar artista en self.library_manager.artist_groups
                    if let Some(group) = self.library_manager.artist_groups.iter().find(|g| &g.name == artist_name) {
                        let art_songs = group.songs.clone();
                        if !art_songs.is_empty() {
                            return self.update(Message::PlayAlbum(art_songs));
                        }
                    }
                } else if !self.library_manager.is_list_mode() {
                    if let Some(songs) = &self.library_manager.expanded_album_songs {
                        if !songs.is_empty() {
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
                // TODO: Iced no soporta rfd síncrono muy bien durante update. Se requerirá un thread spawn.
                // Para simplificar, usaremos the rfd en un custom Task.
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    let path_str = folder.to_string_lossy().to_string();
                    self.scanner.scan_folder_async(path_str);
                }
                Task::none()
            }
            Message::ToggleFilterMenu => {
                self.filters_manager.menu_open = !self.filters_manager.menu_open;
                Task::none()
            }
            Message::ChangeGeneralFilter(ft) => {
                self.filters_manager.selected_type = ft;
                self.filters_manager.selected_subfilter = None;
                self.filters_manager.expanded_nodes.clear();
                self.filters_manager.menu_open = false;
                
                // Reset internal library filters when changing general filter type
                self.library_manager.filter_artist = None;
                self.library_manager.filter_album = None;
                self.library_manager.filter_genre = None;
                self.library_manager.filter_year = None;
                self.library_manager.filter_folder = None;
                self.library_manager.apply_filter();
                Task::none()
            }
            Message::SelectSubfilter(sub) => {
                if sub == self.filters_manager.selected_subfilter {
                     self.filters_manager.selected_subfilter = None;
                } else {
                     self.filters_manager.selected_subfilter = sub;
                }
                Task::none()
            }
            Message::SelectTreeNode(name) => {
                // Si es un nodo de segundo nivel (contiene |), no colapsar/expandir, solo filtrar
                if !name.contains('|') {
                    if self.filters_manager.expanded_nodes.contains(&name) {
                        self.filters_manager.expanded_nodes.remove(&name);
                    } else {
                        self.filters_manager.expanded_nodes.insert(name.clone());
                    }
                }

                // Apply filter to library
                self.library_manager.filter_artist = None;
                self.library_manager.filter_album = None;
                self.library_manager.filter_genre = None;
                self.library_manager.filter_year = None;
                self.library_manager.filter_folder = None;

                let parts: Vec<&str> = name.split('|').collect();
                let main_val = parts[0];
                let sub_val = parts.get(1).cloned();

                match self.filters_manager.selected_type {
                    crate::gui::library_filters::FilterType::Artist => {
                        self.library_manager.filter_artist = Some(main_val.to_string());
                        if let Some(alb) = sub_val {
                            self.library_manager.filter_album = Some(alb.to_string());
                        }
                    }
                    crate::gui::library_filters::FilterType::Album => {
                        self.library_manager.filter_album = Some(main_val.to_string());
                        if let Some(art) = sub_val {
                            self.library_manager.filter_artist = Some(art.to_string());
                        }
                    }
                    crate::gui::library_filters::FilterType::Genre => {
                        self.library_manager.filter_genre = Some(main_val.to_string());
                        if let Some(art) = sub_val {
                            self.library_manager.filter_artist = Some(art.to_string());
                        }
                    }
                    crate::gui::library_filters::FilterType::Year => {
                        self.library_manager.filter_year = Some(main_val.to_string());
                        if let Some(art) = sub_val {
                            self.library_manager.filter_artist = Some(art.to_string());
                        }
                    }
                    crate::gui::library_filters::FilterType::Folder => {
                        self.library_manager.filter_folder = Some(main_val.to_string());
                    }
                }
                
                self.library_manager.apply_filter();
                Task::none()
            }
            Message::FilterSearchChanged(q) => {
                self.filters_manager.search_query = q;
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
            Message::GlobalMouseRelease => {
                if self.library_manager.resizing_column.is_some() {
                    self.library_manager.resizing_column = None;
                }
                Task::none()
            }
            Message::PlayerActivityTimeout(_tick) => {
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
            if let Some(song) = self.playlist_manager.lists[self.playlist_manager.active_list_idx].1.get(idx) {
                if song.path == audio_state.path {
                    if let Some(ref cp) = song.cover_cache_path {
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

    pub fn view(&self) -> Element<'_, Message> {
        let _ = &self.database; // Hack temporal para el warning
        let _ = &self.scanner;  // Hack temporal para el warning
        
        let player_view = crate::gui::player::view(&self.audio_manager, &self.player_ui_state);
        let playlist_view = crate::gui::playlist::view(&self.playlist_manager, &self.audio_manager);
        let filters_view = crate::gui::library_filters::view(&self.filters_manager, &self.library_manager);
        let library_view = crate::gui::library::view(&self.library_manager, &self.database);

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
            
            // Falso modal: fondo negro semitransparente
            let modal_bg = iced::widget::mouse_area(
                iced::widget::container(iced::widget::Space::new().width(iced::Length::Fill).height(iced::Length::Fill))
                .style(|_t: &iced::Theme| iced::widget::container::Style::default().background(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.8)))
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
        // Tick adaptivo: 1000ms durante reproducción (1 FPS), 4000ms en reposo, 3000ms low-resource
        let tick_interval = if self.audio_manager.get_state().is_playing {
            let state = self.audio_manager.get_state();
            let remaining = state.total_duration_sec - state.current_pos_sec;
            if remaining > 0.0 && remaining < 0.5 {
                std::time::Duration::from_millis(50)
            } else {
                std::time::Duration::from_millis(1000)
            }
        } else if self.low_resource_mode {
            std::time::Duration::from_millis(3000)
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
                    Key::Named(Named::ArrowUp) => Some(Message::LibraryKeyNav(crate::gui::library::LibraryNavDir::Up, modifiers)),
                    Key::Named(Named::ArrowDown) => Some(Message::LibraryKeyNav(crate::gui::library::LibraryNavDir::Down, modifiers)),
                    Key::Named(Named::ArrowLeft) => Some(Message::LibraryKeyNav(crate::gui::library::LibraryNavDir::Left, modifiers)),
                    Key::Named(Named::ArrowRight) => Some(Message::LibraryKeyNav(crate::gui::library::LibraryNavDir::Right, modifiers)),
                    Key::Named(Named::Delete) => Some(Message::LibraryDeleteSelection),
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
        self.library_manager.selection_stats = if self.library_manager.selected_song_idx.is_some() {
            // Si hay una canción seleccionada, mostramos las estadísticas totales de la biblioteca (solicitado)
            None
        } else if let Some(album_id) = &self.library_manager.selected_album {
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
            // Prioridad 2: Buscar en DB por hash (Grid sin expansión)
            } else if let Ok(db) = self.database.lock() {
                if let Ok((songs_count, dur, size)) = db.get_album_stats_by_hash(album_id) {
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
                    // Prioridad 3: Buscar en filtered_songs por nombre (Listas)
                    let alb_songs: Vec<_> = songs.iter()
                        .filter(|s| s.album.as_ref() == Some(album_id))
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
                     songs: group.songs.len() as u64,
                     albums: group.albums.len() as u64,
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

