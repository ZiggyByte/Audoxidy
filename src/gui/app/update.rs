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
        let state = self.core.audio_state();
        let refresh_media_controls = matches!(
            &message,
            Message::PlayPause
                | Message::NextTrack
                | Message::PreviousTrack
                | Message::Stop
                | Message::SetPlaying(..)
                | Message::VolumeChanged(..)
                | Message::SeekTo(..)
                | Message::ExternalControl(..)
                | Message::PlaySongIndex(..)
                | Message::PlayAlbum(..)
                | Message::PlayLibrarySelection
                | Message::PlayLibraryAll
                | Message::PlaylistContextMenuPlay(..)
                | Message::CoreEvent(..)
        );

        let task = match message {
            Message::FrameTick
            | Message::UiTick
            | Message::TransportTick
            | Message::ScanTick
            | Message::IdleTick => self.handle_ticks(message, &state),
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
        };

        // The panel is refreshed only after messages that can change the
        // published playback state or track identity, plus the coarse progress
        // snapshot carried by the 500 ms transport tick. Publishing on every
        // message would put the D-Bus call on the render cadence.
        if refresh_media_controls {
            let state = self.core.audio_state();
            self.media_controls.update(&state);
        }

        task
    }
}
