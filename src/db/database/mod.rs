//! SQLite music library database.
//!
//! Defines the connection type and the module tree that owns schema
//! creation, song and playlist persistence, settings, presets, and search.

mod connection;
mod fts;
mod library;
mod models;
mod playlists;
mod presets;
mod schema;
mod search;
mod settings;
mod songs;

pub use connection::Database;
pub use models::LibrarySearchParams;
pub use models::PlaylistData;
pub use models::PlaylistFolderGroup;
pub use models::PlaylistSongRef;
pub use models::ShuffleSession;
pub use models::SongData;
pub use models::SongMetadataExtended;
pub(in crate::db::database) use search::fts5_match_query;

#[cfg(test)]
mod eq_preset_tests;

#[cfg(test)]
mod fts_tests;

#[cfg(test)]
mod cache_tests;

#[cfg(test)]
mod db_round_trip_tests;

#[cfg(test)]
mod cover_tests;

#[cfg(test)]
mod crud_tests;

#[cfg(test)]
mod fts_consistency_tests;

#[cfg(test)]
mod real_library_tests;

#[cfg(test)]
mod schema_tests;

#[cfg(test)]
mod settings_tests;
