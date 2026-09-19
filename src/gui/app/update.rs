//! Update dispatcher module root.
//!
//! Declares the handler submodules extracted out of `AudoxidyApp::update` and
//! holds the thin dispatcher itself.

mod audio_center;
mod context_menu;
mod core_event;
mod dialogs;
mod library;
pub(in crate::gui::app) mod memory;
mod meter;
pub(in crate::gui::app) mod playback;
mod playlist;
mod ticks;
mod window;

use crate::gui::app::Message;
use iced::Task;

impl crate::gui::app::AudoxidyApp {
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
            Message::Tick
            | Message::UITick
            | Message::DragTick
            | Message::MeterTick
            | Message::SearchPulse => self.handle_ticks(message, &state),
            Message::GlobalMemoryPurge => self.handle_memory(message),
            Message::PlayPause
            | Message::NextTrack
            | Message::PreviousTrack
            | Message::Stop
            | Message::VolumeChanged(..)
            | Message::SeekTo(..)
            | Message::SetPlaying(..)
            | Message::ExternalControl(..)
            | Message::ToggleRepeat
            | Message::ToggleShuffle => self.handle_playback(message, &state),
            Message::AddSongToPlaylist(..)
            | Message::PlayAlbum(..)
            | Message::PlaySongIndex(..)
            | Message::ClearPlaylist
            | Message::ToggleSongEnabled(..)
            | Message::ToggleGroupEnabled(..)
            | Message::TogglePlaylistFolder(..)
            | Message::CreatePlaylist
            | Message::RefreshPlaylists(..)
            | Message::InternalPlaylistsRefreshed(..)
            | Message::ExportPlaylist(..)
            | Message::SwitchPlaylist(..)
            | Message::InternalPlaylistLoaded(..)
            | Message::PlaylistShowAllTabs(..)
            | Message::PlaylistTabsScrolled(..)
            | Message::ToggleTabDropdown
            | Message::PlaylistSearchChanged(..)
            | Message::PlaylistFocus
            | Message::PlaylistDeselect
            | Message::ToggleLyrics
            | Message::PlaylistContextMenuPlay(..)
            | Message::PlaylistAddFiles
            | Message::PlaylistAddFolder
            | Message::PlaylistProcessExternalFiles(..)
            | Message::PlaylistShowFileLocation(..)
            | Message::PlaylistShowInLibrary(..)
            | Message::PlaylistDeleteSelection(..)
            | Message::PlaylistRequestSubMenu(..)
            | Message::PlaylistSendToNewList(..)
            | Message::PlaylistSendToList(..)
            | Message::PlaylistToggleAllFolders
            | Message::PlaylistToggleEnabled(..)
            | Message::PlaylistScrolled(..)
            | Message::PlaylistMouseOver(..) => self.handle_playlist(message),

            Message::OpenDialog(..)
            | Message::CloseDialog
            | Message::UpdateDialogInput(..)
            | Message::ConfirmDialogAction
            | Message::OpenFolderPicker
            | Message::OpenPlaylistFilePicker
            | Message::ImportPlaylistFile(..)
            | Message::SelectExportFormat(..)
            | Message::SelectExportMode(..) => self.handle_dialogs(message),

            Message::LibraryFocus
            | Message::ScanLibrary(..)
            | Message::SearchQueryChanged(..)
            | Message::LibrarySearchQueryChanged(..)
            | Message::LibrarySourceSelected(..)
            | Message::LibrarySortChanged(..)
            | Message::StartColumnResize(..)
            | Message::ColumnHover(..)
            | Message::ToggleAlbumExpansion(..)
            | Message::ToggleArtistExpansion(..)
            | Message::LibraryAllSongsLoaded(..)
            | Message::SelectAlbum(..)
            | Message::SelectSong(..)
            | Message::SelectArtistHeader(..)
            | Message::LibraryKeyNav(..)
            | Message::ExecutePendingScroll
            | Message::LibraryMarqueeStart(..)
            | Message::LibraryDeselect
            | Message::ChangeLibraryViewMode(..)
            | Message::ToggleLibraryViewDropdown
            | Message::ToggleLibraryAddDropdown
            | Message::PlayLibrarySelection
            | Message::PlayLibraryAll
            | Message::LibraryDeleteSelection
            | Message::LibraryRefresh
            | Message::LibraryFilteredLoaded(..)
            | Message::InternalLibraryRefreshed(..)
            | Message::LibraryShowPlaying
            | Message::ToggleFilterMenu
            | Message::ChangeGeneralFilter(..)
            | Message::SelectSubfilter(..)
            | Message::ToggleTreeNode(..)
            | Message::SelectTreeNode(..)
            | Message::SelectFolder(..)
            | Message::FilterSearchChanged(..)
            | Message::LibraryScroll(..)
            | Message::FilterScroll(..) => self.handle_library(message),

            Message::GlobalKeyDown(..)
            | Message::WindowResized(..)
            | Message::SetWindowId(..)
            | Message::ModifiersChanged(..)
            | Message::PlayerHoverZone(..)
            | Message::PlayerScroll(..)
            | Message::PlayerVolumeTimeout(..)
            | Message::ToggleMenu
            | Message::GlobalClick
            | Message::PlayerWindowAction(..)
            | Message::PlayerMouseMoved(..)
            | Message::GlobalMouseRelease
            | Message::PlayerActivityTimeout(..)
            | Message::InitStartup => self.handle_window(message),

            Message::ToggleAudioCenter(..)
            | Message::EqPresetImportFile
            | Message::EqPresetExportFile(..)
            | Message::AudioCenterMsg(..) => self.handle_audio_center(message),

            Message::OpenContextMenu(..)
            | Message::RequestContextMenu(..)
            | Message::CloseContextMenu
            | Message::ContextMenuAction(..) => self.handle_context_menu(message),
            // VU Meter — configuración
            Message::MeterRightClicked(..)
            | Message::MeterPopupClose
            | Message::MeterHoldTimeChanged(..)
            | Message::MeterRmsWindowChanged(..)
            | Message::MeterMsModeToggle(..)
            | Message::MeterInfiniteHoldToggle(..)
            | Message::MeterResetPeak
            | Message::MeterToggleEnabled(..) => self.handle_meter(message),
            Message::NoOp => Task::none(),
            Message::CoreEvent(event) => self.on_core_event(event),
        }
    }
}
