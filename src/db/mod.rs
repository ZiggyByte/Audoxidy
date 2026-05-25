//! Módulo de base de datos de Audoxidy.
//!
//! Gestiona la biblioteca musical en SQLite mediante `rusqlite`, incluyendo
//! el esquema relacional (carpetas, artistas, álbumes, canciones, playlists),
//! búsqueda de texto completo con FTS5, y persistencia de sesiones de reproducción.

pub mod database;
pub mod scanner;

pub use database::Database;
// pub use database::{
//     PlaylistData, PlaylistSongRef, PlaylistFolderGroup, ShuffleSession,
// };
// pub use scanner::Scanner; // a desarrollar
