//! Application state for the GUI shell.

use crate::audio::AudioManager;
use crate::db::{Database, scanner::Scanner};
use crate::gui::audio_center::AudioCenterManager;
use crate::gui::library::LibraryManager;
use crate::gui::library_filters::LibraryFiltersManager;
use crate::gui::playlist::PlaylistManager;
use crate::integrations::control::ExternalControlEvent;
use crate::integrations::media_controls::SystemMediaControls;
use std::sync::{Arc, Mutex};

use super::{ActiveDialog, AppFocus, Message};

pub struct AudoxidyApp {
    pub(in crate::gui::app) audio_manager: Arc<AudioManager>,
    pub(in crate::gui::app) database: Arc<Mutex<Database>>,
    pub(in crate::gui::app) scanner: Arc<Scanner>,
    pub(in crate::gui::app) media_controls: SystemMediaControls,
    pub(in crate::gui::app) external_control_rx: crossbeam::channel::Receiver<ExternalControlEvent>,
    pub(in crate::gui::app) playlist_manager: PlaylistManager,
    pub(in crate::gui::app) filters_manager: LibraryFiltersManager,
    pub(in crate::gui::app) library_manager: LibraryManager,
    pub(in crate::gui::app) audio_center_manager: AudioCenterManager,
    pub(in crate::gui::app) player_ui_state: crate::gui::player::PlayerUiState,
    pub(in crate::gui::app) window_id: Option<iced::window::Id>,
    pub(in crate::gui::app) last_artist_header_click: Option<(String, std::time::Instant)>,
    pub(in crate::gui::app) last_album_header_click: Option<(String, std::time::Instant)>,
    pub(in crate::gui::app) last_playlist_click: Option<(usize, std::time::Instant)>,
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
