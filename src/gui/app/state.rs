//! Application state for the GUI shell.

use crate::core::{Core, CoreEvent};
use crate::gui::audio_center::AudioCenterManager;
use crate::gui::library::LibraryManager;
use crate::gui::library_filters::LibraryFiltersManager;
use crate::gui::playlist::PlaylistManager;
use crate::integrations::control::ExternalControlEvent;
use crate::integrations::media_controls::SystemMediaControls;
use std::sync::Arc;

use super::{ActiveDialog, AppFocus, Message};

pub struct AudoxidyApp {
    pub(in crate::gui::app) core: Arc<Core>,
    pub(in crate::gui::app) media_controls: SystemMediaControls,
    pub(in crate::gui::app) external_control_rx: crossbeam::channel::Receiver<ExternalControlEvent>,
    pub(in crate::gui::app) core_event_rx: crossbeam::channel::Receiver<CoreEvent>,
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
    /// Tamaño de la ventana cuando no está maximizada; es el valor que se
    /// persiste, de modo que restaurar la ventana use el tamaño previo y no
    /// el tamaño de pantalla completa.
    pub unmaximized_size: (u32, u32),
    /// Estado maximizado real de la ventana, sincronizado desde el runtime.
    pub window_maximized: bool,
    /// Evita apilar consultas de maximizado durante un arrastre de
    /// redimensionado: solo hay una consulta diferida pendiente a la vez.
    pub maximized_query_pending: bool,
    /// Último tamaño de pantalla completa conocido. Permite crear la ventana
    /// ya maximizada al arrancar sin el parpadeo de abrirla pequeña primero.
    pub maximized_size: Option<(u32, u32)>,
    /// Arrastre de ventana armado: el press sobre una barra lo activa, pero el
    /// arrastre del sistema solo comienza cuando el puntero supera el umbral.
    pub window_drag_armed: bool,
    /// Posición del puntero cuando se armó el arrastre de ventana.
    pub window_drag_origin: Option<iced::Point>,
    /// Última posición conocida de la ventana en el escritorio.
    pub window_position: Option<iced::Point>,
    pub modifiers: iced::keyboard::Modifiers,
    pub was_scanning: bool,
    pub scan_finished_at: Option<u64>,
    pub(in crate::gui::app) geometry_dirty_at: Option<std::time::Instant>,
    pub(in crate::gui::app) geometry_last_saved: Option<std::time::Instant>,
}
