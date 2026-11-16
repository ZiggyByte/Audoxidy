//! Library panel: state, virtualization, selection, data processing, and rendering.
//!
//! The module root declares the concern submodules and re-exports the public
//! surface so existing `crate::gui::library::*` paths keep resolving.

pub(crate) mod cover_cache;
mod data;
pub(crate) use data::song_belongs_to_album;
pub(crate) mod grid_metrics;
pub(crate) mod lists;
mod selection;
mod state;
mod types;
mod view;
mod virtualization;

pub use grid_metrics::{GridMetrics, RowSpan};
pub use state::{
    GRID_ID_A, GRID_ID_B, LIBRARY_SCROLL_ID, LIBRARY_SEARCH_ID, LibraryManager,
    UNLOAD_SAFETY_LIMIT, get_library_redraw_count, library_redraw_count_log,
};
pub use types::{
    AlbumEntry, ArtistGroup, LibraryAlbumGroup, LibraryGridElement, LibraryListItem, LibraryNavDir,
    LibrarySource, LibraryStats, LibraryViewMode, LibraryVirtualArtist, LibraryVirtualRow,
};
pub use view::view;

#[cfg(test)]
mod library_tests;

#[cfg(test)]
mod data_tests;

#[cfg(test)]
mod grid_metrics_tests;

#[cfg(test)]
mod sort_tests;

#[cfg(test)]
mod cover_cache_tests;
