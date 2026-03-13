use iced::{Element, Task, Theme};
use std::sync::{Arc, Mutex};
use crate::audio::AudioManager;
use crate::db::{Database, scanner::Scanner};
use crate::gui::playlist::PlaylistManager;
use crate::gui::library_filters::LibraryFiltersManager;
use crate::gui::library::LibraryManager;
use crate::gui::audio_center::{AudioCenterManager, AudioCenterMessage};
use crate::integrations::media_controls::SystemMediaControls;

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
    AddSongToPlaylist(crate::db::database::SongRecord),
    PlayAlbum(Vec<crate::db::database::SongRecord>),
    PlaySongIndex(usize),
    ClearPlaylist,

    // Library
    ScanLibrary(String),
    SearchQueryChanged(String),
    LibrarySearchQueryChanged(String),
    LibrarySourceSelected(crate::gui::library::LibrarySource),
    LibrarySortChanged(crate::gui::library::SortColumn),
    StartColumnResize(crate::gui::library::SortColumn),
    ColumnHover(Option<crate::gui::library::SortColumn>),
    ToggleAlbumExpansion(String),
    ChangeLibraryViewMode(crate::gui::library::LibraryViewMode),
    ToggleLibraryViewDropdown,
    ToggleLibraryAddDropdown,
    PlayLibrarySelection,
    PlayLibraryAll,
    OpenFolderPicker,

    // Filters
    ToggleGenreFilter,
    ToggleArtistFilter,
    ToggleAlbumFilter,
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
}

impl AudoxidyApp {
    pub fn new(audio_manager: Arc<AudioManager>) -> (Self, Task<Message>) {
        let db = Database::new().expect("Error crítico al crear/iniciar la base de datos.");
        let mut library_manager = LibraryManager::default();
        if let Ok(albums) = db.get_all_albums() {
            library_manager.cached_albums = Some(albums);
        }
        if let Ok((total_songs, total_albums, total_duration, total_size)) = db.get_library_stats() {
            library_manager.total_songs = total_songs;
            library_manager.total_albums = total_albums;
            library_manager.total_duration_secs = total_duration;
            library_manager.total_size_bytes = total_size;
        }

        let database_arc = Arc::new(Mutex::new(db));
        let scanner_arc = Arc::new(Scanner::new(Arc::clone(&database_arc)));

        let media_controls = SystemMediaControls::new(audio_manager.clone())
            .expect("Error al inicializar controles multimedia del sistema");

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
            },
            Task::none()
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
                
                // Actualizador perezoso de la base de datos de Biblioteca cada 5 segundos (50 ticks)
                if self.player_ui_state.tick_count % 50 == 0 {
                    if let Ok(db) = self.database.lock() {
                        if let Ok(mut albums) = db.get_all_albums() {
                            if let Some(col_ref) = self.library_manager.sort_column {
                                let is_asc = self.library_manager.sort_ascending.unwrap_or(true);
                                albums.sort_by(|a, b| {
                                    let res = match col_ref {
                                        crate::gui::library::SortColumn::Album => a.0.cmp(&b.0),
                                        crate::gui::library::SortColumn::Artist => a.1.cmp(&b.1),
                                        crate::gui::library::SortColumn::Genre => a.2.cmp(&b.2),
                                        crate::gui::library::SortColumn::Year => a.3.cmp(&b.3),
                                        _ => std::cmp::Ordering::Equal,
                                    };
                                    if is_asc { res } else { res.reverse() }
                                });
                            }
                            self.library_manager.cached_albums = Some(albums);
                        }
                    }
                }
                
                // --- Cachear Portada del Álbum para evitar colapsar la VRAM de WGPU ---
                if let Some(ref art) = state.album_art {
                    if art.len() != self.player_ui_state.current_art_len {
                        self.player_ui_state.current_art_len = art.len();
                        self.player_ui_state.cached_art_handle = Some(iced::widget::image::Handle::from_bytes(art.clone()));
                    }
                } else if self.player_ui_state.current_art_len != 0 {
                    self.player_ui_state.current_art_len = 0;
                    self.player_ui_state.cached_art_handle = None;
                }
                
                if state.eof_reached {
                    self.audio_manager.clear_eof();
                    self.playlist_manager.play_next(&self.audio_manager);
                }
                Task::none()
            }
            Message::PlayPause => {
                let _ = self.audio_manager.toggle_play_pause();
                self.wake_up_controls(false)
            }
            Message::NextTrack => {
                self.playlist_manager.play_next(&self.audio_manager);
                self.wake_up_controls(false)
            }
            Message::PreviousTrack => {
                self.playlist_manager.play_prev(&self.audio_manager);
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
                if let Some(song) = self.playlist_manager.lists[self.playlist_manager.active_list_idx].1.get(idx) {
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
                Task::none()
            }
            Message::SearchQueryChanged(q) => {
                self.playlist_manager.search_query = q;
                Task::none()
            }
            Message::LibrarySearchQueryChanged(q) => {
                self.library_manager.search_query = q;
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
                                crate::gui::library::SortColumn::Album => a.0.cmp(&b.0).then(a.1.cmp(&b.1)),
                                crate::gui::library::SortColumn::Artist => a.1.cmp(&b.1).then(a.0.cmp(&b.0)),
                                crate::gui::library::SortColumn::Genre => a.2.cmp(&b.2).then(a.1.cmp(&b.1)).then(a.0.cmp(&b.0)),
                                crate::gui::library::SortColumn::Year => a.3.cmp(&b.3).then(a.1.cmp(&b.1)).then(a.0.cmp(&b.0)),
                                _ => std::cmp::Ordering::Equal,
                            };
                            if is_asc { res } else { res.reverse() }
                        });
                    }
                    
                    if let Some(mut songs) = self.library_manager.expanded_album_songs.take() {
                        self.library_manager.sort_songs(&mut songs);
                        self.library_manager.expanded_album_songs = Some(songs);
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
                if self.library_manager.expanded_album.as_deref() == Some(album_id.as_str()) {
                    self.library_manager.expanded_album = None;
                    self.library_manager.expanded_album_songs = None;
                } else {
                    self.library_manager.expanded_album = Some(album_id.clone());
                    if let Ok(db) = self.database.lock() {
                        if let Ok(mut songs) = db.get_songs_by_album(&album_id) {
                            if self.library_manager.sort_column.is_some() {
                                self.library_manager.sort_songs(&mut songs);
                            }
                            self.library_manager.expanded_album_songs = Some(songs);
                        }
                    }
                }
                Task::none()
            }
            Message::ChangeLibraryViewMode(mode) => {
                self.library_manager.view_mode = mode;
                self.library_manager.view_menu_open = false;
                Task::none()
            }
            Message::ToggleLibraryViewDropdown => {
                let next = match self.library_manager.view_mode {
                    crate::gui::library::LibraryViewMode::Grid => crate::gui::library::LibraryViewMode::DetailedList,
                    crate::gui::library::LibraryViewMode::DetailedList => crate::gui::library::LibraryViewMode::ThumbnailList,
                    crate::gui::library::LibraryViewMode::ThumbnailList => crate::gui::library::LibraryViewMode::SimpleList,
                    crate::gui::library::LibraryViewMode::SimpleList => crate::gui::library::LibraryViewMode::Grid,
                };
                self.library_manager.view_mode = next;
                Task::none()
            }
            Message::ToggleLibraryAddDropdown => {
                self.library_manager.add_menu_open = !self.library_manager.add_menu_open;
                Task::none()
            }
            Message::PlayLibrarySelection => {
                if let Some(songs) = &self.library_manager.expanded_album_songs {
                    if !songs.is_empty() {
                        return self.update(Message::PlayAlbum(songs.clone()));
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
            Message::ToggleGenreFilter => {
                self.filters_manager.tree_open_genre = !self.filters_manager.tree_open_genre;
                Task::none()
            }
            Message::ToggleArtistFilter => {
                self.filters_manager.tree_open_artist = !self.filters_manager.tree_open_artist;
                Task::none()
            }
            Message::ToggleAlbumFilter => {
                self.filters_manager.tree_open_album = !self.filters_manager.tree_open_album;
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

    pub fn view(&self) -> Element<'_, Message> {
        let _ = &self.database; // Hack temporal para el warning
        let _ = &self.scanner;  // Hack temporal para el warning
        
        let player_view = crate::gui::player::view(&self.audio_manager, &self.player_ui_state);
        let playlist_view = crate::gui::playlist::view(&self.playlist_manager, &self.audio_manager);
        let filters_view = crate::gui::library_filters::view(&self.filters_manager);
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
        let tick = iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::Tick);
        let win_ids = iced::window::open_events().map(Message::SetWindowId);
        let mouse_evs = iced::event::listen_with(|event, _status, _window_id| {
            if let iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) = event {
                Some(Message::PlayerMouseMoved(position))
            } else if let iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) = event {
                Some(Message::GlobalMouseRelease)
            } else {
                None
            }
        });
        iced::Subscription::batch([tick, win_ids, mouse_evs])
    }
}

pub mod helpers {
    use iced::advanced::{Widget, layout, mouse, Clipboard, Shell, Layout};
    use iced::advanced::widget::Tree;
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
