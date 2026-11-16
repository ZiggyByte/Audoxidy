//! Playlist panel: state, list logic, rendering, and context-menu construction.
//!
//! The module root declares the concern submodules and re-exports the public
//! surface so existing `crate::gui::playlist::*` paths keep resolving.

mod context_menu;
mod logic;
mod state;
mod view;

pub use context_menu::get_item_context_menu_entries;
pub use state::{PlaylistItemType, PlaylistManager};
pub use state::{count_linear_items, flatten_groups};
pub use view::PLAYLIST_SCROLL_ID;
pub use view::PLAYLIST_SEARCH_ID;
pub use view::PLAYLIST_TABS_SCROLL_ID;
pub use view::estimate_tab_width;
pub use view::view;

#[cfg(test)]
#[path = "playlist_tests.rs"]
mod tests;
