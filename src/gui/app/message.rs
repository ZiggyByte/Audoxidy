//! UI event vocabulary for the application shell.
//!
//! `Message` is the single event type the iced runtime delivers to
//! `AudoxidyApp::update`; its companion enums describe message payloads.

use crate::gui::audio_center::AudioCenterMessage;
use crate::integrations::control::ExternalControlEvent;

#[derive(Debug, Clone)]

pub enum Message {
    // Engine & Playback
    Tick,
    UITick,
    DragTick,
    MeterTick,
    SearchPulse,
    GlobalMemoryPurge,
    PlayPause,
    NextTrack,
    PreviousTrack,
    Stop,
    VolumeChanged(f32),
    SeekTo(f32),
    ExternalControl(ExternalControlEvent),
    SetPlaying(bool),
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

    // EQ Preset Import/Export
    EqPresetImportFile,
    EqPresetExportFile(Option<String>),

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

    // VU Meter — configuración
    MeterRightClicked(iced::Point),
    MeterPopupClose,
    MeterHoldTimeChanged(f32),
    MeterRmsWindowChanged(f32),
    MeterMsModeToggle(bool),
    MeterInfiniteHoldToggle(bool),
    MeterResetPeak,
    MeterToggleEnabled(bool),

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
    EqPresetLoad {
        selected_preset_idx: Option<usize>,
        preview_backup: Option<crate::gui::audio_center::EqStateBackup>,
    },
    EqPresetSave {
        name_input: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppFocus {
    Library,
    Playlist,
    AudioCenter,
}
