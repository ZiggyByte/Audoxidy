use crate::audio::AudioError;
use crate::audio::AudioManager;
use crate::db::{Database, scanner::Scanner};
use crate::gui::audio_center::{AudioCenterManager, AudioCenterMessage};
use crate::gui::library::{LIBRARY_SCROLL_ID, LibraryManager};
use crate::gui::library_filters::LibraryFiltersManager;
use crate::gui::playlist::PLAYLIST_TABS_SCROLL_ID;
use crate::gui::playlist::PlaylistManager;
use crate::gui::theme::{COLOR_ACCENT, COLOR_CONTRAST, COLOR_TEXT_PRIMARY, COLOR_TEXT_SECONDARY};
use crate::integrations::media_controls::SystemMediaControls;
use iced::widget::operation::{AbsoluteOffset, focus, scroll_to};
use iced::{Color, Element, Task, Theme};
use std::sync::{Arc, Mutex};

pub static DIALOG_TEXT_INPUT_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(iced::widget::Id::unique);

pub mod helpers {
    use iced::advanced::widget::{Operation, Tree};
    use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse};
    use iced::{Element, Event, Length, Rectangle, Size};

    pub struct CursorOff<'a, Message, Theme, Renderer> {
        content: Element<'a, Message, Theme, Renderer>,
    }

    impl<'a, Message, Theme, Renderer> CursorOff<'a, Message, Theme, Renderer> {
        pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
            Self {
                content: content.into(),
            }
        }
    }

    impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
        for CursorOff<'a, Message, Theme, Renderer>
    where
        Renderer: iced::advanced::Renderer,
    {
        fn size(&self) -> Size<Length> {
            self.content.as_widget().size()
        }

        fn layout(
            &mut self,
            tree: &mut Tree,
            renderer: &Renderer,
            limits: &layout::Limits,
        ) -> layout::Node {
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
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport)
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
            self.content
                .as_widget_mut()
                .operate(tree, layout, renderer, operation);
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
            let _ = self
                .content
                .as_widget()
                .mouse_interaction(state, layout, cursor, viewport, renderer);
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
            self.content
                .as_widget_mut()
                .overlay(state, layout, renderer, viewport, translation)
        }
    }

    impl<'a, Message, Theme, Renderer> From<CursorOff<'a, Message, Theme, Renderer>>
        for Element<'a, Message, Theme, Renderer>
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
    UITick,
    DragTick,
    SearchPulse,
    GlobalMemoryPurge,
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
    ToggleSongEnabled(usize, Option<bool>),
    SwitchPlaylist(i64, bool), // id, auto_play
    ToggleTabDropdown,
    PlaylistSearchChanged(String),
    PlaylistFocus,
    PlaylistDeselect,
    ToggleLyrics,
    ToggleGroupEnabled(usize, Option<bool>),
    TogglePlaylistFolder(usize),
    GlobalKeyDown(iced::keyboard::Key, iced::keyboard::Modifiers),
    CreatePlaylist,
    ExportPlaylist(i64, String, ExportFormat, ExportMode), // id, target_path, format, mode
    RefreshPlaylists(Option<i64>),
    InternalPlaylistsRefreshed(Vec<crate::db::database::PlaylistData>, Option<i64>),
    InternalPlaylistLoaded(
        i64,
        bool,
        Option<Vec<crate::db::database::PlaylistFolderGroup>>,
        Option<crate::db::database::PlaylistData>,
        Option<crate::db::database::ShuffleSession>,
    ),
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
    SelectSong(Option<i64>),
    SelectArtistHeader(String),
    LibraryKeyNav(
        crate::gui::library::LibraryNavDir,
        iced::keyboard::Modifiers,
    ),
    LibraryScroll(iced::widget::scrollable::Viewport),
    LibraryFocus,
    LibraryDeselect,
    LibraryDeleteSelection,
    LibraryRefresh,
    InternalLibraryRefreshed(
        Vec<std::sync::Arc<crate::db::database::SongData>>,
        Vec<(String, String, String, String, String, Option<String>)>,
        (usize, usize, f64, f64, usize), // songs, albums, duration, size, artists
    ),
    ChangeLibraryViewMode(crate::gui::library::LibraryViewMode),
    ToggleLibraryViewDropdown,
    LibraryShowPlaying,
    ToggleLibraryAddDropdown,
    PlayLibrarySelection,
    PlayLibraryAll,
    LibraryAllSongsLoaded(Vec<std::sync::Arc<crate::db::database::SongData>>),
    LibraryFilteredLoaded(Vec<std::sync::Arc<crate::db::database::SongData>>),
    LibraryMarqueeStart(iced::Point),
    ExecutePendingScroll,
    OpenFolderPicker,
    OpenPlaylistFilePicker,
    ImportPlaylistFile(String), // path
    SelectExportFormat(ExportFormat),
    SelectExportMode(ExportMode),

    // Filters
    ToggleFilterMenu,
    ChangeGeneralFilter(crate::gui::library_filters::FilterType),
    SelectSubfilter(Option<String>),
    SelectTreeNode(String),
    ToggleTreeNode(String),
    SelectFolder(i64),
    FilterSearchChanged(String),
    FilterScroll(iced::widget::scrollable::Viewport),

    // Audio Center
    ToggleAudioCenter(Option<usize>),
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
    PlaylistTabsScrolled(iced::widget::scrollable::Viewport),
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
    OpenContextMenu(
        iced::Point,
        Vec<crate::gui::widgets::ContextMenuEntry<Message>>,
    ),
    RequestContextMenu(Vec<crate::gui::widgets::ContextMenuEntry<Message>>),
    CloseContextMenu,
    ContextMenuAction(Box<Message>),

    // Context Menu Actions
    PlaylistContextMenuPlay(usize),
    PlaylistAddFiles,
    PlaylistAddFolder,
    PlaylistProcessExternalFiles(Vec<std::path::PathBuf>),
    PlaylistShowFileLocation(usize),
    PlaylistShowInLibrary(usize),
    PlaylistDeleteSelection(usize),
    PlaylistRequestSubMenu(usize),
    PlaylistSendToNewList(usize),
    PlaylistSendToList(usize, i64),
    PlaylistToggleAllFolders,
    PlaylistToggleEnabled(usize),
    PlaylistShowAllTabs(iced::Point),

    ModifiersChanged(iced::keyboard::Modifiers),

    NoOp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    M3U,
    M3U8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportMode {
    SingleFile,
    PortableFolder,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActiveDialog {
    None,
    CreatePlaylist {
        name: String,
        pending_add_songs: Vec<i64>,
    },
    RenamePlaylist {
        id: i64,
        current_name: String,
        new_name: String,
    },
    DeleteConfirm {
        id: i64,
        name: String,
    },
    ExportConfirm {
        id: i64,
        name: String,
        format: ExportFormat,
        mode: ExportMode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppFocus {
    Library,
    Playlist,
    AudioCenter,
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
    pub previous_focus: AppFocus,
    pub is_mouse_over_playlist: bool,
    pub low_resource_mode: bool,
    pub db_needs_refresh: bool,
    pub active_dialog: ActiveDialog,
    pub last_mouse_pos: iced::Point,
    pub dialog_pos: Option<iced::Point>,
    pub context_menu: Option<(
        iced::Point,
        Vec<crate::gui::widgets::ContextMenuEntry<Message>>,
    )>,
    pub window_size: (u32, u32),
    pub modifiers: iced::keyboard::Modifiers,
    pub was_scanning: bool,
    pub scan_finished_at: Option<u64>,
}

impl AudoxidyApp {
    pub fn new(audio_manager: Arc<AudioManager>) -> (Self, Task<Message>) {
        // Inicializar temporizadores de memoria (Ciclo de 2 minutos)
        crate::utils::memory_manager::MemoryManager::init();

        let db = Database::new().expect("Error crítico al crear/iniciar la base de datos.");
        let mut library_manager = LibraryManager::default();

        if let Ok((total_songs, total_albums, total_duration, total_size, total_artists)) =
            db.get_library_stats()
        {
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

        // Cargar datos iniciales de forma unificada (Orden y Filtros)
        let initial_songs = db.get_all_songs().unwrap_or_default();
        let initial_albums_raw = db.get_grid_items_by_artist().unwrap_or_default();
        let initial_albums = initial_albums_raw
            .into_iter()
            .map(|a| crate::gui::library::AlbumEntry {
                id: a.0,
                title: a.1,
                artist: a.2,
                genre: a.3,
                year: a.4,
                cover_path: a.5,
            })
            .collect();

        library_manager.update_data(initial_songs, initial_albums);

        let database_arc = Arc::new(Mutex::new(db));
        audio_manager.set_database(database_arc.clone());
        let db_scanner =
            Database::new().expect("Error al inicializar la base de datos del escáner.");
        let scanner_arc = Arc::new(Scanner::new(Arc::new(Mutex::new(db_scanner))));

        let media_controls = SystemMediaControls::new(audio_manager.clone())
            .expect("Error al inicializar controles multimedia del sistema");

        // Detección de Hardware para modo de bajos recursos (RAM < 8GB o Cores < 4)
        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();
        let total_ram_gb = sys.total_memory() / 1024 / 1024 / 1024;
        let cpu_cores = sys.cpus().len();
        let low_resource_mode = total_ram_gb < 8 || cpu_cores < 4;

        // Configurar flag global para que todos los módulos puedan consultarlo
        crate::utils::LOW_RESOURCE_MODE
            .store(low_resource_mode, std::sync::atomic::Ordering::Relaxed);

        if low_resource_mode {
            println!(
                "Audoxidy Performance: Low resource mode ENABLED (RAM: {}GB, Cores: {})",
                total_ram_gb, cpu_cores
            );
        }

        // Construir el índice de filtros con las canciones ya cargadas
        let mut filters_manager = LibraryFiltersManager::default();
        filters_manager.build_filter_index(&database_arc);

        let mut playlist_manager = PlaylistManager::default();
        if let Ok(db_lock) = database_arc.lock() {
            // 0. Cargar persistencia de ajustes de audio del reproductor
            let host_id = db_lock.get_setting("audio_host").filter(|s| !s.is_empty());
            let device_name = db_lock
                .get_setting("audio_device")
                .filter(|s| !s.is_empty());
            let sample_rate = db_lock.get_setting("audio_sample_rate").and_then(|s| {
                if s == "auto" {
                    None
                } else {
                    s.parse::<u32>().ok()
                }
            });
            let bit_depth = db_lock
                .get_setting("audio_bit_depth")
                .and_then(|s| match s.as_str() {
                    "16" => Some(crate::audio::engine::BitDepth::Bits16),
                    "24" => Some(crate::audio::engine::BitDepth::Bits24),
                    "32" => Some(crate::audio::engine::BitDepth::Bits32Float),
                    _ => None,
                });
            let channels_val = db_lock
                .get_setting("audio_channels")
                .and_then(|s| s.parse::<u16>().ok());
            let buffer_size = db_lock.get_setting("audio_buffer_size").and_then(|s| {
                if s == "auto" {
                    None
                } else {
                    s.parse::<u32>().ok()
                }
            });
            let auto_upsample = db_lock
                .get_setting("audio_auto_upsample")
                .map(|s| s == "true")
                .unwrap_or(false);

            if host_id.is_some()
                || device_name.is_some()
                || sample_rate.is_some()
                || bit_depth.is_some()
                || buffer_size.is_some()
                || auto_upsample
            {
                let settings = crate::audio::engine::AudioSettings {
                    host_id,
                    device_name,
                    sample_rate,
                    bit_depth,
                    channels: crate::audio::engine::ChannelConfig::Manual(
                        channels_val.unwrap_or(2),
                    ),
                    buffer_size,
                    auto_upsample,
                };
                let _ = audio_manager.apply_audio_settings(settings);
            }

            // 1. Cargar última playlist activa
            if let Some(last_id_str) = db_lock.get_setting("last_active_playlist_id") {
                if let Ok(last_id) = last_id_str.parse::<i64>() {
                    playlist_manager.active_playlist_id = last_id;
                }
            }

            let active_id = playlist_manager.active_playlist_id;
            if let Ok(groups) = db_lock.get_playlist_songs_grouped_by_folder(active_id) {
                playlist_manager.set_groups(groups);
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
                            if let Some(pos) =
                                session.shuffle_order.iter().position(|&id| id == song_id)
                            {
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
                            let title = song.title.clone();
                            let artist = song.artist_name.to_string();

                            playlist_manager.playing_song_idx = Some(l_idx);
                            let (tg, ag) = db_lock
                                .get_replay_gain_by_path(&path)
                                .unwrap_or((None, None));
                            let _ = audio_manager.load_file(
                                &path,
                                title.to_string(),
                                artist.to_string(),
                                tg,
                                ag,
                            );
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
                previous_focus: AppFocus::Library,
                is_mouse_over_playlist: false,
                low_resource_mode,
                db_needs_refresh: false,
                active_dialog: ActiveDialog::None,
                last_mouse_pos: iced::Point::ORIGIN,
                dialog_pos: None,
                context_menu: None,
                window_size: (1360, 880),
                modifiers: iced::keyboard::Modifiers::default(),
                was_scanning: false,
                scan_finished_at: None,
            },
            Task::perform(
                async {
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                },
                |_| Message::InitStartup,
            ),
        )
    }
    fn wake_up_controls(&mut self, is_mouse_move: bool) -> Task<Message> {
        let new_until = self.player_ui_state.tick_count.wrapping_add(3); // 1.5s a 500ms/tick

        // Optimización: solo actualizar si la diferencia es significativa (> 1 tick) o si estaba inactivo
        let needs_update = !self.player_ui_state.is_active
            || (new_until > self.player_ui_state.active_until_tick
                && new_until - self.player_ui_state.active_until_tick > 1);

        if needs_update {
            self.player_ui_state.is_active = true;
            self.player_ui_state.active_until_tick = new_until;
        } else if !is_mouse_move {
            // Si no es movimiento (ej. click), forzar activación
            self.player_ui_state.is_active = true;
        }

        let mut tasks = vec![];

        if is_mouse_move {
            if self.player_ui_state.showing_volume.is_some()
                && !self.player_ui_state.volume_clearing
            {
                self.player_ui_state.volume_clearing = true;
                let current_vol_tick = self.player_ui_state.volume_tick_id;
                tasks.push(iced::Task::perform(
                    async { tokio::time::sleep(std::time::Duration::from_millis(1000)).await },
                    move |_| Message::PlayerVolumeTimeout(current_vol_tick),
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

    /// Helper centralizado que carga un archivo de audio con sus valores de ReplayGain.
    /// Busca track_gain y album_gain en la base de datos por ruta y los pasa al motor.
    fn load_file_with_gains(
        &self,
        path: &str,
        title: impl Into<String>,
        artist: impl Into<String>,
    ) -> Result<(), AudioError> {
        let (track_gain, album_gain) = if let Ok(db) = self.database.lock() {
            match db.get_replay_gain_by_path(path) {
                Ok(gains) => gains,
                Err(e) => {
                    tracing::error!("Error consultando ReplayGain en BD: {}", e);
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        if track_gain.is_none() && album_gain.is_none() {
            tracing::debug!("DB: No se encontró ReplayGain para la ruta: {}", path);
        }

        self.audio_manager
            .load_file(path, title, artist, track_gain, album_gain)
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

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let state = self.audio_manager.get_state();
        self.media_controls.update(&state);

        // Registro de actividad global y por sección
        match &message {
            // Actividad de Playlist
            Message::PlaylistFocus
            | Message::SwitchPlaylist(..)
            | Message::PlaylistSearchChanged(..)
            | Message::PlaylistScrolled(..)
            | Message::PlaylistTabsScrolled(..)
            | Message::PlaylistDeselect
            | Message::AddSongToPlaylist(..)
            | Message::ToggleSongEnabled(..)
            | Message::ToggleGroupEnabled(..)
            | Message::TogglePlaylistFolder(..)
            | Message::PlaySongIndex(..) => {
                crate::utils::memory_manager::MemoryManager::register_playlist_activity();
            }
            // Actividad de Biblioteca
            Message::LibraryFocus
            | Message::LibraryScroll(..)
            | Message::LibraryKeyNav(..)
            | Message::LibrarySearchQueryChanged(..)
            | Message::ChangeGeneralFilter(..)
            | Message::SelectSubfilter(..)
            | Message::SelectTreeNode(..)
            | Message::SelectAlbum(..)
            | Message::SelectArtistHeader(..)
            | Message::SelectSong(..)
            | Message::LibraryRefresh
            | Message::LibrarySourceSelected(..)
            | Message::FilterSearchChanged(..)
            | Message::FilterScroll(..)
            | Message::SelectFolder(..) => {
                crate::utils::memory_manager::MemoryManager::register_library_activity();
            }
            // Actividad Global (Interacciones directas del usuario)
            Message::GlobalClick
            | Message::GlobalKeyDown(..)
            | Message::GlobalMouseRelease
            | Message::PlayerMouseMoved(..)
            | Message::PlayerScroll(..)
            | Message::WindowResized(..)
            | Message::OpenFolderPicker
            | Message::OpenPlaylistFilePicker
            | Message::ToggleMenu
            | Message::ToggleAudioCenter(..)
            | Message::ToggleLyrics => {
                crate::utils::memory_manager::MemoryManager::register_activity();
            }
            // Los demás mensajes (Tick, NoOp, Mensajes internos del Player, etc.) NO cuentan como actividad
            _ => {}
        }

        match message {
            Message::Tick => {
                // 1. Consultar al scanner si hay datos nuevos en la BD
                if self
                    .scanner
                    .db_dirty
                    .swap(false, std::sync::atomic::Ordering::Relaxed)
                {
                    self.db_needs_refresh = true;
                }

                // 2. Actualizar BD SOLO cuando el escáner marca cambios
                if self.db_needs_refresh {
                    self.db_needs_refresh = false;
                    let db_arc = self.database.clone();
                    return Task::perform(
                        async move {
                            if let Ok(db) = db_arc.lock() {
                                let songs = db.get_all_songs().unwrap_or_default();
                                let albums = db.get_grid_items_by_artist().unwrap_or_default();
                                let stats = db.get_library_stats().unwrap_or((0, 0, 0.0, 0.0, 0));
                                (songs, albums, stats)
                            } else {
                                (Vec::new(), Vec::new(), (0, 0, 0.0, 0.0, 0))
                            }
                        },
                        |(songs, albums, stats)| {
                            Message::InternalLibraryRefreshed(songs, albums, stats)
                        },
                    );
                }

                // 3. Sincronización de carátulas solo si hay reproducción activa
                if state.is_playing {
                    self.sync_player_art();

                    let duration = state.total_duration_sec;
                    let position = state.current_pos_sec;
                    if duration > 0.0 {
                        let remaining = duration - position;
                        if remaining <= 30.0 && remaining > 29.5 {
                            if let Some(next_song) = self.playlist_manager.get_next_song_ref() {
                                if let Some(ref cover_path) = next_song.cover_path {
                                    if self.player_ui_state.current_cover_path
                                        != cover_path.as_ref()
                                    {
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
                            if let Ok(mut db) = self.database.lock() {
                                let _ = db.save_shuffle_session(
                                    self.playlist_manager.active_playlist_id,
                                    session,
                                );
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

                // 4. Actualizar Caché de caracteres y textos base (para detección inteligente)
                if state.title != self.player_ui_state.last_title {
                    self.player_ui_state.last_title = state.title.clone();
                    self.player_ui_state.title_chars = state.title.chars().collect();
                    if self.player_ui_state.title_chars.len() <= 39 {
                        self.player_ui_state.display_title = state.title.clone();
                    }
                }
                if state.artist != self.player_ui_state.last_artist {
                    self.player_ui_state.last_artist = state.artist.clone();
                    self.player_ui_state.artist_chars = state.artist.chars().collect();
                    if self.player_ui_state.artist_chars.len() <= 40 {
                        self.player_ui_state.display_artist = state.artist.clone();
                    }
                }

                // Auto-guardado de persistencia cada ~20 segundos (40 ticks de 500ms)
                if self.player_ui_state.tick_count % 40 == 0 {
                    self.persist_playlist_state();
                }

                // 5a. Hard Cap de RAM: si supera el 75%, purgar inmediatamente (D-03)
                if crate::utils::memory_manager::MemoryManager::is_ram_over_hard_cap() {
                    println!("Audoxidy GC: RAM over 75% hard cap — forcing immediate purge");
                    return Task::done(Message::GlobalMemoryPurge);
                }

                // 5b. Purga por timer (solo cuando RAM está por debajo del hard cap)
                let is_scanning_now = self
                    .scanner
                    .is_scanning
                    .load(std::sync::atomic::Ordering::Relaxed);
                if crate::utils::memory_manager::MemoryManager::should_run_global_purge(
                    2,
                    is_scanning_now,
                ) {
                    return Task::done(Message::GlobalMemoryPurge);
                }

                // Detección de fin de escáner para liberar memoria 30s después
                if self.was_scanning && !is_scanning_now {
                    self.scan_finished_at = Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    );
                }
                self.was_scanning = is_scanning_now;

                if let Some(finished_at) = self.scan_finished_at {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    // D-02: Liberar memoria 40s después del escaneo
                    if now.saturating_sub(finished_at) >= 40 {
                        self.scan_finished_at = None;
                        println!("Audoxidy GC: Scan complete, clearing memory");
                        crate::utils::covers::purge_old_covers(32); // Vaciar caché de covers generado por escáner

                        // Reiniciar el temporizador global para que no haga otra purga en 2 mins
                        crate::utils::memory_manager::MemoryManager::reset_global_purge_timer();

                        return Task::done(Message::GlobalMemoryPurge);
                    }
                }

                Task::none()
            }
            Message::GlobalMemoryPurge => {
                println!("Audoxidy GC: Purging Memory (Ghost Mode)");

                // 1. Unload de recursos pesados con márgenes de seguridad
                let is_library_focused = self.focus == AppFocus::Library;
                let is_playlist_focused = self.focus == AppFocus::Playlist;
                let is_playing = self.audio_manager.is_playing();
                let is_loaded_in_player = self.playlist_manager.playing_song_idx.is_some();

                // D-06: Descargar listas inactivas (la pestaña activa siempre se conserva)
                self.playlist_manager
                    .unload(is_playlist_focused, is_playing || is_loaded_in_player, true);

                // Descargar biblioteca inactiva
                self.library_manager.unload(is_library_focused);

                // 2. Limpieza de cachés de virtualización UI (Siempre se limpian para liberar RAM, se regeneran al vuelo)
                self.library_manager.invalidate_cache();
                self.library_manager.invalidate_visible_cache();
                self.playlist_manager.invalidate_cache();

                // Solo limpiar filtros si no estamos en la biblioteca para evitar que desaparezcan visualmente
                if !is_library_focused {
                    self.filters_manager.invalidate_cache();
                }

                // 3. Limpiar caché global de strings (Interner)
                crate::utils::interner::clear_interner();

                // 4. Purgar carátulas LRU (Purga suave de 16 elementos)
                crate::utils::covers::purge_old_covers(16);

                // 5. Purga de buffers de audio solo si no hay música sonando
                if !is_playing {
                    let _ = self.audio_manager.purge_buffers();
                }

                // 6. FINAL: Reclamar memoria física al SO (Linux)
                crate::utils::memory_manager::MemoryManager::force_free_to_os();

                Task::none()
            }
            Message::SearchPulse => Task::none(),
            Message::DragTick => {
                if self.library_manager.is_dragging {
                    if let Some(pos) = self.player_ui_state.mouse_pos {
                        let win_h = self.window_size.1 as f32;
                        let threshold_top = 150.0;
                        let threshold_bottom = win_h - 100.0;

                        let mut scroll_delta = 0.0;
                        if pos.y < threshold_top {
                            scroll_delta = (pos.y - threshold_top) / 5.0;
                        } else if pos.y > threshold_bottom {
                            scroll_delta = (pos.y - threshold_bottom) / 5.0;
                        }

                        if scroll_delta != 0.0 {
                            let scroll_id = crate::gui::library::LIBRARY_SCROLL_ID.clone();
                            return iced::widget::operation::scroll_by(
                                scroll_id,
                                iced::widget::scrollable::AbsoluteOffset {
                                    x: 0.0,
                                    y: scroll_delta,
                                },
                            );
                        }
                    }
                }
                Task::none()
            }
            Message::UITick => {
                self.player_ui_state.tick_count = self.player_ui_state.tick_count.wrapping_add(1);

                let state = self.audio_manager.get_state();

                // Cálculo de marquesina cada 500ms
                let limit_t = 39;
                let limit_a = 40;

                self.player_ui_state.display_title = if self.player_ui_state.title_chars.len()
                    > limit_t
                {
                    let chars = &self.player_ui_state.title_chars;
                    let offset = (self.player_ui_state.tick_count as usize) % (chars.len() + 10);
                    if offset < chars.len() {
                        let end = (offset + limit_t).min(chars.len());
                        let mut s: String = chars[offset..end].iter().collect();
                        if offset + limit_t > chars.len() {
                            s.push_str("   ");
                            let needed = (offset + limit_t) - chars.len();
                            if needed > 3 {
                                let rem = needed - 3;
                                s.push_str(
                                    &chars[0..rem.min(chars.len())].iter().collect::<String>(),
                                );
                            }
                        }
                        s
                    } else {
                        chars[0..limit_t.min(chars.len())].iter().collect()
                    }
                } else {
                    state.title.clone()
                };

                self.player_ui_state.display_artist = if self.player_ui_state.artist_chars.len()
                    > limit_a
                {
                    let chars = &self.player_ui_state.artist_chars;
                    let offset = (self.player_ui_state.tick_count as usize) % (chars.len() + 10);
                    if offset < chars.len() {
                        let end = (offset + limit_a).min(chars.len());
                        let mut s: String = chars[offset..end].iter().collect();
                        if offset + limit_a > chars.len() {
                            s.push_str("   ");
                            let needed = (offset + limit_a) - chars.len();
                            if needed > 3 {
                                let rem = needed - 3;
                                s.push_str(
                                    &chars[0..rem.min(chars.len())].iter().collect::<String>(),
                                );
                            }
                        }
                        s
                    } else {
                        chars[0..limit_a.min(chars.len())].iter().collect()
                    }
                } else {
                    state.artist.clone()
                };

                if self.player_ui_state.is_active
                    && self.player_ui_state.tick_count > self.player_ui_state.active_until_tick
                {
                    self.player_ui_state.is_active = false;
                }
                Task::none()
            }
            Message::PlayPause => {
                let _ = self.audio_manager.toggle_play_pause();

                // Si acabamos de pausar (no está reproduciendo), purgamos buffers para liberar RAM
                if !self.audio_manager.is_playing() {
                    println!("Audoxidy Audio: Cleaning Buffers on Pause (Memory Recovery).");
                    let _ = self.audio_manager.purge_buffers();
                }

                self.persist_playlist_state();
                self.wake_up_controls(false)
            }
            Message::NextTrack => {
                crate::utils::covers::clear_raw_cache();
                self.playlist_manager.play_next(&self.audio_manager);
                self.persist_playlist_state();

                if self.playlist_manager.shuffle_active {
                    if let Some(session) = &self.playlist_manager.shuffle_session {
                        if let Ok(mut db) = self.database.lock() {
                            let _ = db.save_shuffle_session(
                                self.playlist_manager.active_playlist_id,
                                session,
                            );
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
                        if let Ok(mut db) = self.database.lock() {
                            let _ = db.save_shuffle_session(
                                self.playlist_manager.active_playlist_id,
                                session,
                            );
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
                let _ = self.audio_manager.purge_buffers(); // Purga profunda al detener
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
                        if let Ok(mut db) = self.database.lock() {
                            let _ = db.save_shuffle_session(
                                self.playlist_manager.active_playlist_id,
                                session,
                            );
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
                let db_arc = self.database.clone();
                let active_id = self.playlist_manager.active_playlist_id;
                return Task::perform(
                    async move {
                        if let Ok(db) = db_arc.lock() {
                            let _ = db.add_song_to_playlist(active_id, song.id);
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
                        if let Ok(mut db) = db_arc.lock() {
                            let _ = db.add_songs_to_playlist(active_id, &song_ids);
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
                        let path = song.file_path.clone();
                        let title = song.title.clone();
                        let artist = song.artist_name.to_string();
                        let s_id = song.song_id;

                        self.sync_player_art();
                        if let Err(e) =
                            self.load_file_with_gains(&path, title.to_string(), artist.to_string())
                        {
                            tracing::error!("Error reproduciendo archivo: {}", e);
                        } else {
                            self.audio_manager.play();
                            self.playlist_manager.playing_song_idx = Some(idx);
                            self.playlist_manager.notify_manual_play(s_id);
                            self.persist_playlist_state();
                        }
                    }
                }
                focus_task
            }
            Message::ClearPlaylist => {
                self.focus = AppFocus::Playlist;
                if let Ok(db) = self.database.lock() {
                    let _ = db.clear_playlist(self.playlist_manager.active_playlist_id);
                    self.playlist_manager.clear_groups();
                    self.playlist_manager.playing_song_idx = None;
                    self.audio_manager.stop();
                    self.persist_playlist_state();

                    return Task::done(Message::GlobalMemoryPurge);
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

                if let Ok(db) = self.database.lock() {
                    if let Some(song_id) = song_id_to_update {
                        if let Some(state) = force_state {
                            let _ = db.set_song_enabled_in_playlist(
                                self.playlist_manager.active_playlist_id,
                                song_id,
                                state,
                            );
                        } else {
                            let _ = db.toggle_song_enabled_in_playlist(
                                self.playlist_manager.active_playlist_id,
                                song_id,
                            );
                        }
                    }
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
                    if let Ok(db) = self.database.lock() {
                        if let Some(state) = force_state {
                            let _ = db.set_folder_enabled_in_playlist(
                                self.playlist_manager.active_playlist_id,
                                &path,
                                state,
                            );
                        } else {
                            let _ = db.toggle_folder_enabled_in_playlist(
                                self.playlist_manager.active_playlist_id,
                                &path,
                            );
                        }
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

            Message::OpenDialog(dialog) => {
                let focus_task = match &dialog {
                    ActiveDialog::CreatePlaylist { .. } | ActiveDialog::RenamePlaylist { .. } => {
                        focus(DIALOG_TEXT_INPUT_ID.clone())
                    }
                    _ => Task::none(),
                };
                self.active_dialog = dialog;
                self.dialog_pos = Some(self.last_mouse_pos);
                focus_task
            }

            Message::CloseDialog => {
                self.active_dialog = ActiveDialog::None;
                self.dialog_pos = None;
                Task::none()
            }

            Message::UpdateDialogInput(s) => {
                match &mut self.active_dialog {
                    ActiveDialog::CreatePlaylist { name, .. } => *name = s,
                    ActiveDialog::RenamePlaylist { new_name, .. } => *new_name = s,
                    _ => {}
                }
                Task::none()
            }

            Message::ConfirmDialogAction => {
                let dialog = std::mem::replace(&mut self.active_dialog, ActiveDialog::None);
                self.active_dialog = ActiveDialog::None;
                self.dialog_pos = None;
                let db = self.database.clone();

                match dialog {
                    ActiveDialog::CreatePlaylist {
                        name,
                        pending_add_songs,
                    } => {
                        let final_name = if name.trim().is_empty() {
                            format!("Nueva lista {}", self.playlist_manager.playlists.len() + 1)
                        } else {
                            name
                        };

                        let target_songs = pending_add_songs;
                        return Task::perform(
                            async move {
                                if let Ok(mut db_lock) = db.lock() {
                                    if let Ok(new_id) = db_lock.create_playlist(&final_name, false)
                                    {
                                        if !target_songs.is_empty() {
                                            let _ = db_lock
                                                .add_songs_to_playlist(new_id, &target_songs);
                                        }
                                        return Some(new_id);
                                    }
                                }
                                None
                            },
                            Message::RefreshPlaylists,
                        );
                    }
                    ActiveDialog::RenamePlaylist { id, new_name, .. } => {
                        if !new_name.trim().is_empty() {
                            let is_system = self
                                .playlist_manager
                                .playlists
                                .iter()
                                .find(|p| p.id == id)
                                .map(|p| p.is_system)
                                .unwrap_or(false);

                            if is_system {
                                let final_name = new_name.trim().to_string();
                                let source_id = id;
                                return Task::perform(
                                    async move {
                                        if let Ok(mut db_lock) = db.lock() {
                                            if let Ok(new_id) =
                                                db_lock.create_playlist(&final_name, false)
                                            {
                                                if let Ok(all_songs) =
                                                    db_lock.get_playlist_all_song_ids(source_id)
                                                {
                                                    let _ = db_lock
                                                        .add_songs_to_playlist(new_id, &all_songs);
                                                }
                                                return Some(new_id);
                                            }
                                        }
                                        None
                                    },
                                    Message::RefreshPlaylists,
                                );
                            } else {
                                return Task::perform(
                                    async move {
                                        let db_lock = db.lock().unwrap();
                                        if db_lock.rename_playlist(id, &new_name).is_ok() {
                                            Some(id)
                                        } else {
                                            None
                                        }
                                    },
                                    Message::RefreshPlaylists,
                                );
                            }
                        }
                    }
                    ActiveDialog::DeleteConfirm { id, .. } => {
                        return Task::perform(
                            async move {
                                let db_lock = db.lock().unwrap();
                                if db_lock.delete_playlist(id).is_ok() {
                                    Some(0)
                                } else {
                                    None
                                }
                            },
                            Message::RefreshPlaylists,
                        );
                    }
                    ActiveDialog::ExportConfirm {
                        id,
                        name: _,
                        format,
                        mode,
                    } => {
                        return Task::perform(
                            async move {
                                match mode {
                                    ExportMode::PortableFolder => {
                                        if let Some(folder) = rfd::AsyncFileDialog::new()
                                            .set_title("Seleccionar carpeta de exportación")
                                            .pick_folder()
                                            .await
                                        {
                                            Some((
                                                id,
                                                folder.path().to_string_lossy().into_owned(),
                                                format,
                                                mode,
                                            ))
                                        } else {
                                            None
                                        }
                                    }
                                    ExportMode::SingleFile => {
                                        let ext = match format {
                                            ExportFormat::M3U => "m3u",
                                            ExportFormat::M3U8 => "m3u8",
                                        };
                                        if let Some(file) = rfd::AsyncFileDialog::new()
                                            .set_title("Guardar lista de reproducción")
                                            .add_filter("Lista de reproducción", &[ext])
                                            .save_file()
                                            .await
                                        {
                                            Some((
                                                id,
                                                file.path().to_string_lossy().into_owned(),
                                                format,
                                                mode,
                                            ))
                                        } else {
                                            None
                                        }
                                    }
                                }
                            },
                            |res| {
                                if let Some((pid, path, fmt, m)) = res {
                                    Message::ExportPlaylist(pid, path, fmt, m)
                                } else {
                                    Message::NoOp
                                }
                            },
                        );
                    }
                    _ => {}
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

                        let songs = if let Ok(db_lock) = db.lock() {
                            db_lock.search_playlist_songs(id, "").unwrap_or_default()
                        } else {
                            vec![]
                        };

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

                                let _ = std::fs::create_dir_all(&full_target_dir);
                                let _ = std::fs::copy(&*song.file_path, &full_target_path);
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
                        let _ = std::fs::write(&final_m3u_path, m3u_content);
                    },
                    |_| Message::NoOp,
                );
            }

            Message::SwitchPlaylist(id, auto_play) => {
                self.focus = AppFocus::Playlist;
                self.persist_playlist_state();

                if let Ok(db) = self.database.lock() {
                    let _ = db.set_setting("last_active_playlist_id", &id.to_string());
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
                // Fase 6: abrir módulo de letras
                Task::none()
            }
            Message::LibraryFocus => {
                self.focus = AppFocus::Library;
                crate::utils::memory_manager::MemoryManager::register_library_activity();
                if self.library_manager.data_unloaded {
                    self.library_manager.data_unloaded = false;
                    return self.update(Message::LibraryRefresh);
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
                self.focus = AppFocus::Library;
                crate::utils::memory_manager::MemoryManager::register_library_activity();
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
                    if let Ok(db) = self.database.lock() {
                        if let Ok(albums) = db.get_grid_items_by_artist() {
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
                        if let Some(album_id) = &self.library_manager.expanded_album {
                            if let Ok(songs) = db.get_songs_by_album(album_id) {
                                self.library_manager.expanded_album_songs = Some(songs);
                            }
                        }
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
                                if let Ok(db) = self.database.lock() {
                                    if let Ok(songs) =
                                        db.get_songs_by_album_and_artist(hash, artist)
                                    {
                                        alb_songs = songs;
                                    }
                                }
                            } else if let Ok(db) = self.database.lock() {
                                if let Ok(songs) = db.get_songs_by_album(album_id.as_str()) {
                                    alb_songs = songs;
                                }
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
                crate::utils::memory_manager::MemoryManager::register_library_activity();
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
            Message::GlobalKeyDown(key, modifiers) => {
                use crate::gui::library::LibraryNavDir;
                use iced::keyboard::Key;
                use iced::keyboard::key::Named;

                match key {
                    Key::Named(Named::ArrowUp) => {
                        if self.focus == AppFocus::Playlist {
                            self.playlist_manager
                                .handle_key_nav(LibraryNavDir::Up, modifiers);
                            if let Some(idx) = self.playlist_manager.focused_idx {
                                return self.execute_playlist_autoscroll(idx);
                            }
                            Task::none()
                        } else if self.focus == AppFocus::AudioCenter {
                            Task::none()
                        } else {
                            self.update(Message::LibraryKeyNav(LibraryNavDir::Up, modifiers))
                        }
                    }
                    Key::Named(Named::ArrowDown) => {
                        if self.focus == AppFocus::Playlist {
                            self.playlist_manager
                                .handle_key_nav(LibraryNavDir::Down, modifiers);
                            if let Some(idx) = self.playlist_manager.focused_idx {
                                return self.execute_playlist_autoscroll(idx);
                            }
                            Task::none()
                        } else if self.focus == AppFocus::AudioCenter {
                            Task::none()
                        } else {
                            self.update(Message::LibraryKeyNav(LibraryNavDir::Down, modifiers))
                        }
                    }
                    Key::Character(ref c) if c.to_lowercase() == "a" && modifiers.command() => {
                        if self.focus == AppFocus::Playlist {
                            self.playlist_manager.select_all();
                            Task::none()
                        } else {
                            self.library_manager.select_all();
                            Task::none()
                        }
                    }
                    Key::Named(Named::ArrowLeft) => {
                        if self.focus == AppFocus::Playlist {
                            self.playlist_manager
                                .handle_key_nav(LibraryNavDir::Left, modifiers);
                            Task::none()
                        } else if self.focus == AppFocus::AudioCenter {
                            Task::none()
                        } else {
                            self.update(Message::LibraryKeyNav(LibraryNavDir::Left, modifiers))
                        }
                    }
                    Key::Named(Named::ArrowRight) => {
                        if self.focus == AppFocus::Playlist {
                            self.playlist_manager
                                .handle_key_nav(LibraryNavDir::Right, modifiers);
                            Task::none()
                        } else if self.focus == AppFocus::AudioCenter {
                            Task::none()
                        } else {
                            self.update(Message::LibraryKeyNav(LibraryNavDir::Right, modifiers))
                        }
                    }
                    Key::Named(Named::Enter) => {
                        if self.focus == AppFocus::Playlist {
                            // Reproducción inmediata para Enter en Playlist
                            if let Some(idx) = self.playlist_manager.focused_idx {
                                if let Some(song) =
                                    self.playlist_manager.get_song_at_linear_index(idx)
                                {
                                    let path = song.file_path.clone();
                                    let title = song.title.clone();
                                    let artist = song.artist_name.to_string();

                                    self.sync_player_art();
                                    if let Err(e) = self.load_file_with_gains(
                                        &path,
                                        title.to_string(),
                                        artist.to_string(),
                                    ) {
                                        tracing::error!(
                                            "Error reproducidendo archivo con Enter: {}",
                                            e
                                        );
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
                            if let Some(idx) = self.playlist_manager.focused_idx {
                                return self.update(Message::PlaylistDeleteSelection(idx));
                            }
                            Task::none()
                        } else {
                            self.update(Message::LibraryDeleteSelection)
                        }
                    }
                    Key::Named(Named::Space) => {
                        if self.focus == AppFocus::Playlist {
                            if let Some(sel) = self.playlist_manager.focused_idx {
                                return self.update(Message::PlaylistToggleEnabled(sel));
                            }
                        }
                        Task::none()
                    }
                    _ => Task::none(),
                }
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
                crate::utils::covers::clear_all_cover_cache();

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
                        if let Ok(db) = self.database.lock() {
                            if let Ok(albums) = db.get_grid_items_by_artist() {
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
            Message::WindowResized(w, h) => {
                self.window_size = (w, h);
                // La biblioteca ocupa todo el ancho menos el panel izquierdo (~520px) y filtros (~202px)
                let sidebar_w = 722.0_f32;
                self.library_manager.library_area_width = (w as f32 - sidebar_w).max(202.0);
                Task::none()
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
                        let s_id = song.song_id;

                        self.sync_player_art();
                        if let Err(e) =
                            self.load_file_with_gains(&path, title.to_string(), artist.to_string())
                        {
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
                let selected_songs = self.library_manager.get_selected_songs();
                if !selected_songs.is_empty() {
                    return self.update(Message::PlayAlbum(selected_songs));
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
                Task::perform(
                    async move { rfd::FileDialog::new().pick_folder() },
                    move |folder| {
                        if let Some(f) = folder {
                            let path_str = f.to_string_lossy().to_string();
                            scanner_arc.scan_folder_async(path_str);
                        }
                        Message::NoOp
                    },
                )
            }
            Message::OpenPlaylistFilePicker => Task::perform(
                async move {
                    rfd::FileDialog::new()
                        .add_filter("Lista de reproducción", &["m3u", "m3u8"])
                        .pick_file()
                },
                |file| {
                    if let Some(f) = file {
                        Message::ImportPlaylistFile(f.to_string_lossy().to_string())
                    } else {
                        Message::NoOp
                    }
                },
            ),

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

                            self.sync_player_art();
                            if let Err(e) =
                                self.load_file_with_gains(&path, title.to_string(), artist)
                            {
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
                println!(
                    "Audoxidy Debug: PlaylistShowInLibrary path_opt: {:?}",
                    path_opt
                );
                self.handle_library_reveal(path_opt.map(|s| s.to_string()))
            }
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

                let db_arc = self.database.clone();
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

                        if let Ok(db) = db_arc.lock() {
                            let _ = db.begin_transaction();
                            let _ = db.batch_delete_songs(&ids_to_delete);
                            let _ = db.cleanup_empty_metadata();
                            let _ = db.commit_transaction();

                            for hash in covers_to_check {
                                if let Ok(false) = db.is_cover_hash_in_use(&hash) {
                                    let path = format!("cache/covers/{}.avif", hash);
                                    let _ = std::fs::remove_file(path);
                                }
                            }
                        }
                    },
                    |_| Message::LibraryRefresh,
                );
            }

            Message::LibraryRefresh => {
                self.focus = AppFocus::Library;
                crate::utils::memory_manager::MemoryManager::register_library_activity();
                self.library_manager.data_unloaded = false;

                let db_arc = self.database.clone();
                let params = self.library_manager.get_search_params();

                return Task::batch(vec![Task::perform(
                    async move {
                        let db = db_arc.lock().unwrap();
                        let songs = db.get_library_songs_filtered(&params).unwrap_or_default();
                        let albums = db.get_grid_items_by_artist().unwrap_or_default();
                        let stats = db.get_library_stats().unwrap_or((0, 0, 0.0, 0.0, 0));
                        (songs, albums, stats)
                    },
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
                self.filters_manager.build_filter_index(&self.database);
                self.update_selection_stats();

                // Si había una petición de revelado pendiente (esperando a que los filtros se limpiaran), continuar ahora.
                if let Some(path) = self.library_manager.pending_reveal_path.clone() {
                    return self.handle_library_reveal(Some(path));
                }

                Task::none()
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
                                if let Ok(mut db) = db_arc.lock() {
                                    let _ = db.batch_remove_songs_from_playlist(
                                        active_playlist_id,
                                        &target_songs,
                                    );
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
                            if let Ok(mut db) = db_arc.lock() {
                                let _ = db.add_songs_to_playlist(target_playlist_id, &target_songs);
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

            Message::LibraryShowPlaying => {
                self.focus = AppFocus::Library;
                let path_opt = self
                    .playlist_manager
                    .playing_song_idx
                    .and_then(|idx| self.playlist_manager.get_song_at_linear_index(idx))
                    .map(|s| s.file_path.clone());
                self.handle_library_reveal(path_opt.map(|s| s.to_string()))
            }

            Message::ImportPlaylistFile(path) => {
                let db_arc = self.database.clone();
                let path_clone = path.clone();

                // Usamos perform para no bloquear la UI si el M3U es muy grande
                Task::perform(
                    async move { crate::db::scanner::Scanner::import_m3u(&db_arc, &path_clone) },
                    |playlist_id| {
                        if let Some(id) = playlist_id {
                            Message::SwitchPlaylist(id, true)
                        } else {
                            Message::NoOp
                        }
                    },
                )
            }
            Message::SelectExportFormat(fmt) => {
                if let ActiveDialog::ExportConfirm { format, .. } = &mut self.active_dialog {
                    *format = fmt;
                }
                Task::none()
            }
            Message::SelectExportMode(m) => {
                if let ActiveDialog::ExportConfirm { mode, .. } = &mut self.active_dialog {
                    *mode = m;
                }
                Task::none()
            }

            Message::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers;
                Task::none()
            }

            Message::ToggleFilterMenu => {
                self.filters_manager.menu_open = !self.filters_manager.menu_open;
                Task::none()
            }
            Message::ChangeGeneralFilter(ft) => {
                self.focus = AppFocus::Library;
                crate::utils::memory_manager::MemoryManager::register_library_activity();
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
            Message::ToggleAudioCenter(opt_tab) => {
                if let Some(tab_idx) = opt_tab {
                    if self.audio_center_manager.open
                        && self.audio_center_manager.selected_tab == tab_idx
                    {
                        // Si ya está abierto en la misma pestaña, lo cerramos
                        self.audio_center_manager.open = false;
                        self.audio_center_manager.window_pos = None;
                    } else {
                        // Si está cerrado o en otra pestaña, lo abrimos/cambiamos a esa pestaña
                        if !self.audio_center_manager.open {
                            self.audio_center_manager.open = true;
                            self.audio_center_manager.window_pos = None;
                        }
                        self.audio_center_manager.selected_tab = tab_idx;
                        if self.audio_center_manager.first_open {
                            self.audio_center_manager
                                .sync_from_engine(&self.audio_manager);
                            self.audio_center_manager.first_open = false;
                        }
                    }
                } else {
                    // Alternancia genérica (comportamiento anterior)
                    self.audio_center_manager.open = !self.audio_center_manager.open;
                    if self.audio_center_manager.open {
                        self.audio_center_manager.window_pos = None;
                        if self.audio_center_manager.first_open {
                            self.audio_center_manager
                                .sync_from_engine(&self.audio_manager);
                            self.audio_center_manager.first_open = false;
                        }
                    }
                }
                Task::none()
            }
            Message::AudioCenterMsg(ac_msg) => {
                match &ac_msg {
                    crate::gui::audio_center::AudioCenterMessage::SliderHoverActive(true) => {
                        self.previous_focus = self.focus;
                        self.focus = AppFocus::AudioCenter;
                    }
                    crate::gui::audio_center::AudioCenterMessage::SliderHoverActive(false) => {
                        self.focus = self.previous_focus;
                    }
                    _ => {}
                }

                if let crate::gui::audio_center::AudioCenterMessage::DragStart = ac_msg {
                    self.audio_center_manager.drag_start = Some(self.last_mouse_pos);
                    self.audio_center_manager.is_dragging = true;
                }
                self.audio_center_manager
                    .update(ac_msg, &self.audio_manager);
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
                self.player_ui_state.volume_tick_id =
                    self.player_ui_state.volume_tick_id.wrapping_add(1);
                self.player_ui_state.volume_clearing = false;
                let current_tick = self.player_ui_state.volume_tick_id;

                iced::Task::perform(
                    async { tokio::time::sleep(std::time::Duration::from_millis(1500)).await },
                    move |_| Message::PlayerVolumeTimeout(current_tick),
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
                self.playlist_manager.show_tab_dropdown = false;
                self.filters_manager.menu_open = false;
                self.context_menu = None;
                Task::none()
            }
            Message::PlayerWindowAction(action) => match action {
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
            },
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

                if self.library_manager.marquee_start_pos.is_some()
                    && !self.library_manager.is_dragging
                {
                    if let Some(start_pos) = self.library_manager.marquee_start_pos {
                        let dx = pos.x - start_pos.x;
                        let dy = pos.y - start_pos.y;
                        let dist_sq = dx * dx + dy * dy;

                        if dist_sq > 9.0 {
                            // Umbral de 3px (3*3=9)
                            self.library_manager.is_dragging = true;
                        }
                    }
                }

                if self.library_manager.is_dragging {
                    let lib_x = (pos.x - 602.0).max(0.0);
                    let scroll_y = self.library_manager.scroll_offset.y;
                    let lib_y = pos.y - 72.0 + scroll_y;
                    self.library_manager.marquee_end = Some(iced::Point::new(lib_x, lib_y));
                }

                if self.audio_center_manager.is_dragging {
                    if let Some(start_pos) = self.audio_center_manager.drag_start {
                        let delta_x = pos.x - start_pos.x;
                        let delta_y = pos.y - start_pos.y;

                        let current_pos =
                            self.audio_center_manager.window_pos.unwrap_or_else(|| {
                                let win_w = 940.0;
                                let win_h = 474.0;
                                let x = (self.window_size.0 as f32 - win_w) / 2.0;
                                let y = (self.window_size.1 as f32 - win_h) / 2.0;
                                iced::Point::new(x.max(0.0), y.max(0.0))
                            });

                        let new_x = (current_pos.x + delta_x)
                            .clamp(0.0, (self.window_size.0 as f32 - 100.0).max(0.0));
                        let new_y = (current_pos.y + delta_y)
                            .clamp(0.0, (self.window_size.1 as f32 - 40.0).max(0.0));

                        self.audio_center_manager.window_pos = Some(iced::Point::new(new_x, new_y));
                        self.audio_center_manager.drag_start = Some(pos);
                    }
                }

                self.player_ui_state.mouse_pos = Some(pos);
                self.wake_up_controls(true)
            }
            Message::LibraryScroll(offset) => {
                crate::utils::memory_manager::MemoryManager::register_library_activity();
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
            Message::GlobalMouseRelease => {
                if self.library_manager.resizing_column.is_some() {
                    self.library_manager.resizing_column = None;
                }

                if self.library_manager.is_dragging {
                    if let Some(start) = self.library_manager.marquee_start {
                        if let Some(pos) = self.player_ui_state.mouse_pos {
                            let lib_x = (pos.x - 602.0).max(0.0);
                            let scroll_y = self.library_manager.scroll_offset.y;
                            let lib_y = pos.y - 72.0 + scroll_y;
                            let end = iced::Point::new(lib_x, lib_y);

                            let dx = start.x - end.x;
                            let dy = start.y - end.y;
                            let dist_sq = dx * dx + dy * dy;

                            // Solo procesar como marquee si ha habido un arrastre real (más de 3 píxeles)
                            if dist_sq > 9.0 {
                                let rect_x = start.x.min(end.x);
                                let rect_y = start.y.min(end.y);
                                let rect_w = (start.x - end.x).abs().max(1.0);
                                let rect_h = (start.y - end.y).abs().max(1.0);

                                let rect = iced::Rectangle {
                                    x: rect_x,
                                    y: rect_y,
                                    width: rect_w,
                                    height: rect_h,
                                };

                                self.library_manager.select_marquee(rect, self.modifiers);
                            }
                        }
                    }
                    self.library_manager.is_dragging = false;
                    self.library_manager.marquee_start = None;
                    self.library_manager.marquee_end = None;
                    self.library_manager.marquee_start_pos = None;
                }

                if self.audio_center_manager.is_dragging {
                    self.audio_center_manager.is_dragging = false;
                    self.audio_center_manager.drag_start = None;
                }

                // Aseguramos que marquee_start_pos se limpie incluso si no hubo dragging real
                self.library_manager.marquee_start_pos = None;
                self.library_manager.is_dragging = false;

                Task::none()
            }
            Message::PlayerActivityTimeout(_tick) => Task::none(),
            Message::InitStartup => {
                // Sincronizar carátula inicial
                self.sync_player_art();

                let mut tasks = Vec::new();

                // Forzar auto-scroll a la canción que se restauró
                if let Some(idx) = self.playlist_manager.playing_song_idx {
                    tasks.push(self.execute_playlist_autoscroll(idx));
                }

                // Forzar auto-focus a la pestaña de playlist activa
                let active_id = self.playlist_manager.active_playlist_id;
                tasks.push(Task::done(Message::SwitchPlaylist(active_id, false)));

                // Carga inicial de la biblioteca para asegurar que las vistas de lista no estén vacías
                tasks.push(self.trigger_library_search());

                Task::batch(tasks)
            }
            Message::OpenContextMenu(pos, entries) => {
                let wrapped = self.wrap_context_menu(entries);
                self.context_menu = Some((pos, wrapped));
                Task::none()
            }
            Message::RequestContextMenu(entries) => {
                // Auto-seleccionar si el mensaje viene del sistema de playlist
                // (Normalmente esto lo gestionamos filtrando por el tipo de entrada o mensaje)
                // Pero como RequestContextMenu es genérico, nos apoyamos en que si hay un linear_idx
                // en la primera entrada, es de playlist.
                if let Some(first) = entries.first() {
                    match &first.action {
                        Some(Message::PlaylistContextMenuPlay(idx))
                        | Some(Message::PlaylistShowFileLocation(idx))
                        | Some(Message::PlaylistShowInLibrary(idx))
                        | Some(Message::PlaylistDeleteSelection(idx))
                        | Some(Message::PlaylistToggleEnabled(idx))
                        | Some(Message::PlaylistRequestSubMenu(idx)) => {
                            if !self.playlist_manager.selected_idxs.contains(idx) {
                                self.playlist_manager.selected_idxs.clear();
                                self.playlist_manager.selected_idxs.insert(*idx);
                                self.playlist_manager.focused_idx = Some(*idx);
                                self.playlist_manager.selection_pivot = Some(*idx);
                            }
                        }
                        _ => {}
                    }
                }

                let pos = self.last_mouse_pos;
                let wrapped = self.wrap_context_menu(entries);
                self.context_menu = Some((pos, wrapped));
                Task::none()
            }
            Message::CloseContextMenu => {
                self.context_menu = None;
                self.playlist_manager.show_tab_dropdown = false;
                Task::none()
            }
            Message::ContextMenuAction(msg) => {
                self.context_menu = None;
                self.playlist_manager.show_tab_dropdown = false;
                self.update(*msg)
            }
            Message::NoOp => Task::none(),
        }
    }

    fn wrap_context_menu(
        &self,
        entries: Vec<crate::gui::widgets::ContextMenuEntry<Message>>,
    ) -> Vec<crate::gui::widgets::ContextMenuEntry<Message>> {
        entries
            .into_iter()
            .map(|mut e| {
                if let Some(a) = e.action.take() {
                    // No envolver selectores recursivos o NoOp
                    match &a {
                        Message::ContextMenuAction(_) | Message::NoOp => {
                            e.action = Some(a);
                        }
                        _ => e.action = Some(Message::ContextMenuAction(Box::new(a))),
                    }
                }
                e
            })
            .collect()
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
                if song.file_path.as_ref() == audio_state.path {
                    if let Some(ref cp) = song.cover_path {
                        new_cover_path = cp.to_string();
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

        // 6. Fallback final (Sin carátula)
        self.player_ui_state.cached_art_handle = None;
        self.player_ui_state.current_art_id = audio_state.path.clone();
        self.player_ui_state.current_cover_path.clear();
    }

    fn persist_playlist_state(&self) {
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

        if let Ok(db) = self.database.lock() {
            let _ = db.update_playlist_persistence(
                playlist_id,
                last_song_id,
                pos,
                is_p,
                shuffle,
                repeat,
                s_pos,
                s_id,
            );
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let _ = &self.database; // Hack temporal para el warning
        let _ = &self.scanner; // Hack temporal para el warning

        let player_view = crate::gui::player::view(&self.audio_manager, &self.player_ui_state);
        let playlist_view = crate::gui::playlist::view(&self.playlist_manager, &self.audio_manager);
        let (filters_view, filters_menu_overlay) =
            crate::gui::library_filters::view(&self.filters_manager);
        let library_view = crate::gui::library::view(
            &self.library_manager,
            &self.database,
            &self.player_ui_state.current_art_id,
        );

        // Apilamos el reproductor (carátula y controles) arriba de la playlist en una sola columna izquierda
        let left_column = iced::widget::column![player_view, playlist_view]
            .width(iced::Length::Shrink)
            .height(iced::Length::Fill)
            .align_x(iced::Alignment::Start);

        let main_row = iced::widget::row![left_column, filters_view, library_view]
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .align_y(iced::Alignment::Start);

        let final_content: Element<'_, Message> = {
            let mut stack = iced::widget::Stack::new().push(main_row);

            if self.audio_center_manager.open {
                let ac_view =
                    crate::gui::audio_center::view(&self.audio_center_manager, &self.audio_manager);

                // Contenedor posicionado o centrado para el Control Center
                let positioned_modal = if let Some(pos) = self.audio_center_manager.window_pos {
                    iced::widget::container(ac_view)
                        .width(iced::Length::Fill)
                        .height(iced::Length::Fill)
                        .align_x(iced::Alignment::Start)
                        .align_y(iced::Alignment::Start)
                        .padding(iced::Padding {
                            top: pos.y,
                            right: 0.0,
                            bottom: 0.0,
                            left: pos.x,
                        })
                } else {
                    iced::widget::container(ac_view)
                        .width(iced::Length::Fill)
                        .height(iced::Length::Fill)
                        .center_x(iced::Fill)
                        .center_y(iced::Fill)
                };

                stack = stack.push(positioned_modal);
            }
            stack
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .into()
        };

        let app_underlay = iced::widget::mouse_area(
            iced::widget::Space::new()
                .width(iced::Length::Fill)
                .height(iced::Length::Fill),
        )
        .on_press(Message::GlobalClick)
        .on_right_press(Message::GlobalClick)
        .interaction(iced::mouse::Interaction::Idle);

        let mut final_stack = iced::widget::Stack::new()
            .push(app_underlay)
            .push(final_content);

        // --- Capa Global para Menú de Filtros ---
        if let Some(menu) = filters_menu_overlay {
            let filters_underlay = iced::widget::mouse_area(
                iced::widget::Space::new()
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill),
            )
            .on_press(Message::GlobalClick)
            .on_right_press(Message::GlobalClick)
            .interaction(iced::mouse::Interaction::Idle);

            let positioned_menu = iced::widget::container(menu)
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .padding(iced::Padding {
                    top: 0.0,
                    bottom: 0.0,
                    left: 400.0,
                    right: 0.0,
                })
                .align_x(iced::alignment::Horizontal::Left)
                .align_y(iced::alignment::Vertical::Top);

            final_stack = final_stack.push(filters_underlay).push(positioned_menu);
        }

        // --- Capa de Diálogos Modales ---
        let mut final_stack = final_stack;

        if self.active_dialog != ActiveDialog::None {
            let dialog_content = match &self.active_dialog {
                ActiveDialog::CreatePlaylist { name, .. } => {
                    crate::gui::widgets::standard_modal(
                        "Nueva lista de reproducción".to_string(),
                        iced::widget::column![
                            crate::gui::widgets::modal_text("Nombre para la nueva lista:".to_string()),
                            iced::widget::text_input("Nombre de la lista...", name)
                                .id(DIALOG_TEXT_INPUT_ID.clone())
                                .on_input(Message::UpdateDialogInput)
                                .on_submit(Message::ConfirmDialogAction)
                                .padding([4, 5])
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
                        ].spacing(12).into(),
                        Some(Message::CloseDialog),
                        Some(Message::ConfirmDialogAction),
                        "Crear".to_string(),
                    )
                },
                ActiveDialog::RenamePlaylist { new_name, .. } => {
                    crate::gui::widgets::standard_modal(
                        "Renombrar lista".to_string(),
                        iced::widget::column![
                            crate::gui::widgets::modal_text("Nuevo nombre de la lista:".to_string()),
                            iced::widget::text_input("Nuevo nombre...", new_name)
                                .id(DIALOG_TEXT_INPUT_ID.clone())
                                .on_input(Message::UpdateDialogInput)
                                .on_submit(Message::ConfirmDialogAction)
                                .padding([4, 5])
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
                        ].spacing(12).into(),
                        Some(Message::CloseDialog),
                        Some(Message::ConfirmDialogAction),
                        "Renombrar".to_string(),
                    )
                },
                ActiveDialog::DeleteConfirm { name, .. } => {
                    crate::gui::widgets::standard_modal(
                        "¿Eliminar lista?".to_string(),
                        crate::gui::widgets::modal_text(format!("¿Estás seguro de querer eliminar\nla lista \"{}\"?\nEsta acción no se puede deshacer.", name)),
                        Some(Message::CloseDialog),
                        Some(Message::ConfirmDialogAction),
                        "Eliminar".to_string(),
                    )
                },
                ActiveDialog::ExportConfirm { name, id: _, format, mode } => {
                    let format_choices = iced::widget::row![
                        export_choice_button("M3U", *format == ExportFormat::M3U, Message::SelectExportFormat(ExportFormat::M3U)),
                        export_choice_button("M3U8", *format == ExportFormat::M3U8, Message::SelectExportFormat(ExportFormat::M3U8)),
                    ].spacing(10).align_y(iced::Alignment::Center);
                    
                    let mode_choices = iced::widget::row![
                        export_choice_button("Solo Lista", *mode == ExportMode::SingleFile, Message::SelectExportMode(ExportMode::SingleFile)),
                        export_choice_button("Carpeta Portable", *mode == ExportMode::PortableFolder, Message::SelectExportMode(ExportMode::PortableFolder)),
                    ].spacing(10).align_y(iced::Alignment::Center);

                    crate::gui::widgets::standard_modal(
                        "Guardar | Exportar lista".to_string(),
                        iced::widget::column![
                            crate::gui::widgets::modal_text(format!("Puedes guardar solo la lista o también\nexportar todas las canciones de la lista\n\"{}\".", name)),
                            iced::widget::Space::new().height(iced::Length::Fixed(0.0)),
                            crate::gui::widgets::modal_text("Formato de la lista:".to_string()),
                            format_choices,
                            iced::widget::Space::new().height(iced::Length::Fixed(0.0)),
                            crate::gui::widgets::modal_text("Modo:".to_string()),
                            mode_choices,
                        ].spacing(10).align_x(iced::Alignment::Center).into(),
                        Some(Message::CloseDialog),
                        Some(Message::ConfirmDialogAction),
                        "Continuar".to_string(),
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
                iced::widget::Space::new()
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill),
            )
            .on_press(Message::CloseDialog);
            final_stack = final_stack.push(click_blocker).push(
                iced::widget::container(dialog_content)
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill)
                    .padding(iced::Padding {
                        top: target_y,
                        left: target_x,
                        ..Default::default()
                    })
                    .align_x(iced::Alignment::Start)
                    .align_y(iced::Alignment::Start),
            );
        }

        fn export_choice_button<'a>(
            label: &'a str,
            active: bool,
            msg: Message,
        ) -> iced::Element<'a, Message> {
            iced::widget::button(
                iced::widget::text(label)
                    .size(14)
                    .font(crate::gui::theme::FONT_INTER_SANS_MEDIUM),
            )
            .padding([5, 10])
            .on_press(msg)
            .style(move |_t, status| {
                let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                iced::widget::button::Style {
                    background: if active {
                        Some(COLOR_ACCENT.into())
                    } else if is_hovered {
                        Some(COLOR_ACCENT.into())
                    } else {
                        Some(COLOR_CONTRAST.into())
                    },
                    text_color: if active {
                        COLOR_TEXT_PRIMARY
                    } else if is_hovered {
                        COLOR_TEXT_PRIMARY
                    } else {
                        COLOR_TEXT_SECONDARY
                    },
                    border: iced::Border {
                        radius: 6.0.into(),
                        width: 0.0,
                        color: if active {
                            iced::Color::TRANSPARENT
                        } else {
                            COLOR_TEXT_SECONDARY
                        },
                    },
                    ..Default::default()
                }
            })
            .into()
        }

        // --- Renderizar Menú Contextual Nativo ---
        if let Some((pos, entries)) = &self.context_menu {
            let menu_content = crate::gui::widgets::build_context_menu_content(entries.clone());

            // Área de bloqueo para el menú
            let menu_blocker = iced::widget::mouse_area(
                iced::widget::Space::new()
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill),
            )
            .on_press(Message::GlobalClick)
            .on_right_press(Message::GlobalClick);

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

            final_stack = final_stack.push(menu_blocker).push(
                iced::widget::container(menu_content)
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill)
                    .padding(iced::Padding {
                        top: target_y,
                        left: target_x,
                        ..Default::default()
                    })
                    .align_x(iced::Alignment::Start)
                    .align_y(iced::Alignment::Start),
            );
        }

        let wrapped_app = helpers::CursorOff::new(
            iced::widget::container(final_stack)
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .center_x(iced::Fill)
                .center_y(iced::Fill),
        );

        wrapped_app.into()
    }

    pub fn theme(&self) -> Theme {
        crate::gui::theme::custom_theme()
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        // 1. Tick Standard (Progreso): 500ms
        // 2. Tick Idle (Scanner/Pausa): 5000ms
        let is_scanning = self
            .scanner
            .is_scanning
            .load(std::sync::atomic::Ordering::Relaxed);
        let tick_interval = if self.audio_manager.get_state().is_playing || is_scanning {
            std::time::Duration::from_millis(500)
        } else {
            std::time::Duration::from_millis(5000)
        };
        let mut subs = vec![iced::time::every(tick_interval).map(|_| Message::Tick)];

        // 3. UI/Marquee Tick: 500ms
        // Solo se activa si la UI está activa (controles) o si algún texto requiere efecto marquesina
        let needs_marquee = self.player_ui_state.title_chars.len() > 39
            || self.player_ui_state.artist_chars.len() > 40;

        if self.player_ui_state.is_active
            || (self.audio_manager.get_state().is_playing && needs_marquee)
        {
            subs.push(
                iced::time::every(std::time::Duration::from_millis(500)).map(|_| Message::UITick),
            );
        }

        // 4. Hybrid Auto-scroll: 32ms (solo si se está arrastrando)
        if self.library_manager.is_dragging {
            subs.push(
                iced::time::every(std::time::Duration::from_millis(32)).map(|_| Message::DragTick),
            );
        }

        // 5. Search Pulse: 100ms (solo si hay algo en búsqueda para el Grid)
        if !self.library_manager.search_query.is_empty()
            || !self.filters_manager.search_query.is_empty()
        {
            subs.push(
                iced::time::every(std::time::Duration::from_millis(100))
                    .map(|_| Message::SearchPulse),
            );
        }

        let mouse_evs = iced::event::listen_with(|event, _status, window_id| {
            // Registrar actividad global (Modo Fantasma / Garbage Collector)
            match event {
                iced::Event::Mouse(_) | iced::Event::Keyboard(_) | iced::Event::Touch(_) => {
                    crate::utils::memory_manager::MemoryManager::register_activity();
                }
                _ => {}
            }

            // Capturar el ID de la ventana si aún no lo tenemos
            if let iced::Event::Window(iced::window::Event::Opened { .. }) = event {
                return Some(Message::SetWindowId(window_id));
            }

            if let iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) = event {
                Some(Message::PlayerMouseMoved(position))
            } else if let iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                iced::mouse::Button::Left,
            )) = event
            {
                if _status == iced::event::Status::Ignored {
                    Some(Message::LibraryMarqueeStart(iced::Point::new(0.0, 0.0)))
                } else {
                    None
                }
            } else if let iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                iced::mouse::Button::Left,
            )) = event
            {
                Some(Message::GlobalMouseRelease)
            } else if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key,
                modifiers,
                ..
            }) = event
            {
                if _status == iced::event::Status::Ignored {
                    Some(Message::GlobalKeyDown(key, modifiers))
                } else {
                    None
                }
            } else if let iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(
                modifiers,
            )) = event
            {
                Some(Message::ModifiersChanged(modifiers))
            } else if let iced::Event::Window(iced::window::Event::Resized(new_size)) = event {
                Some(Message::WindowResized(
                    new_size.width as u32,
                    new_size.height as u32,
                ))
            } else {
                None
            }
        });

        subs.push(mouse_evs);
        iced::Subscription::batch(subs)
    }

    fn get_library_scroll_task(&self, force_top: bool) -> Task<Message> {
        self.library_manager.get_scroll_task(force_top)
    }

    fn update_selection_stats(&mut self) {
        self.library_manager.update_selection_stats();
    }

    fn handle_library_reveal(&mut self, path_opt: Option<String>) -> Task<Message> {
        println!(
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
                    println!("Audoxidy Debug: lib_song found in cached_all_songs");
                    self.focus = crate::gui::app::AppFocus::Library;

                    // 1. Determinar si la canción es visible actualmente en la lista procesada
                    let is_visible = self
                        .library_manager
                        .filtered_songs
                        .as_ref()
                        .map(|songs| songs.iter().any(|s| s.full_file_path.as_ref() == path))
                        .unwrap_or(false); // Si no hay filtered_songs, NO es visible (lista vacía)

                    println!("Audoxidy Debug: is_visible: {}", is_visible);

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
                                        if let Ok(db) = self.database.lock() {
                                            if let Ok(mut songs) = db.get_songs_by_album_and_artist(
                                                &hash_id,
                                                artist_name,
                                            ) {
                                                if self.library_manager.sort_column.is_some() {
                                                    self.library_manager.sort_songs(&mut songs);
                                                }
                                                self.library_manager.expanded_album_songs =
                                                    Some(songs);
                                            }
                                        }
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
    fn trigger_library_search(&self) -> Task<Message> {
        let db_arc = self.database.clone();
        let params = self.library_manager.get_search_params();

        Task::perform(
            async move {
                let db = db_arc.lock().unwrap();
                db.get_library_songs_filtered(&params).unwrap_or_default()
            },
            Message::LibraryFilteredLoaded,
        )
    }
}
