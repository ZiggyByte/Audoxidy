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
    ToggleAlbumExpansion(String),
    ChangeLibraryViewMode(crate::gui::library::LibraryViewMode),
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
    PlayerActivityTimeout(u64),
    GlobalClick,
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
                library_manager: LibraryManager::default(),
                audio_center_manager: AudioCenterManager::default(),
                player_ui_state: crate::gui::player::PlayerUiState::default(),
                window_id: None,
            },
            Task::none()
        )
    }
    fn wake_up_controls(&mut self, is_mouse_move: bool) -> Task<Message> {
        self.player_ui_state.is_active = true;
        self.player_ui_state.activity_tick = self.player_ui_state.activity_tick.wrapping_add(1);
        let current_act = self.player_ui_state.activity_tick;
        
        let mut tasks = vec![iced::Task::perform(
            async { tokio::time::sleep(std::time::Duration::from_millis(1000)).await },
            move |_| Message::PlayerActivityTimeout(current_act)
        )];
        
        if is_mouse_move {
            if self.player_ui_state.showing_volume.is_some() {
                let current_vol_tick = self.player_ui_state.volume_tick_id;
                tasks.push(iced::Task::perform(
                    async { tokio::time::sleep(std::time::Duration::from_millis(1000)).await },
                    move |_| Message::PlayerVolumeTimeout(current_vol_tick)
                ));
            }
        } else {
            self.player_ui_state.showing_volume = None;
        }
        
        Task::batch(tasks)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let state = self.audio_manager.get_state();
        self.media_controls.update(&state);

        match message {
            Message::Tick => {
                self.player_ui_state.tick_count = self.player_ui_state.tick_count.wrapping_add(1);
                
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
            Message::ToggleAlbumExpansion(album_id) => {
                if self.library_manager.expanded_album.as_deref() == Some(album_id.as_str()) {
                    self.library_manager.expanded_album = None;
                } else {
                    self.library_manager.expanded_album = Some(album_id);
                }
                Task::none()
            }
            Message::ChangeLibraryViewMode(mode) => {
                self.library_manager.view_mode = mode;
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
                let current_tick = self.player_ui_state.volume_tick_id;
                
                iced::Task::perform(
                    async { tokio::time::sleep(std::time::Duration::from_millis(1500)).await },
                    move |_| Message::PlayerVolumeTimeout(current_tick)
                )
            }
            Message::PlayerVolumeTimeout(tick) => {
                if self.player_ui_state.volume_tick_id == tick {
                    self.player_ui_state.showing_volume = None;
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
                self.player_ui_state.mouse_pos = Some(pos);
                self.wake_up_controls(true)
            }
            Message::PlayerActivityTimeout(tick) => {
                if self.player_ui_state.activity_tick == tick {
                    self.player_ui_state.is_active = false;
                }
                Task::none()
            }
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

        iced::widget::container(final_stack)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .center_x(iced::Fill)
            .center_y(iced::Fill)
            .into()
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
            } else {
                None
            }
        });
        iced::Subscription::batch([tick, win_ids, mouse_evs])
    }
}
