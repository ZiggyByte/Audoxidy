//! SQLite connection lifecycle for the library database.

use rusqlite::{Connection, Result};

/// Base de datos SQLite de la biblioteca musical de Audoxidy.
///
/// Gestiona el esquema relacional completo: carpetas, artistas, álbumes,
/// canciones, metadatos extendidos, playlists y sesiones de shuffle.
/// Usa WAL, FTS5 para búsqueda de texto completo y pragmas de rendimiento.
pub struct Database {
    pub(in crate::db::database) conn: Connection,
}

impl Default for Database {
    fn default() -> Self {
        Self::new().expect("No se pudo iniciar la DB local SQLite")
    }
}

impl Database {
    /// Opens or creates the library database and initializes the schema.
    pub fn new() -> Result<Self> {
        Self::open_at(&crate::utils::paths::get().db_path)
    }

    /// Abre una base de datos en la ruta indicada e inicializa el esquema.
    /// Todas las variantes comparten los mismos pragmas y caché de sentencias.
    fn open_at(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        conn.set_prepared_statement_cache_capacity(32);
        Self::create_schema(&conn)?;
        Ok(Self { conn })
    }

    /// Creates an on-disk database at an arbitrary path.
    ///
    /// Used by the entry point to inject the resolved library path, and by
    /// tests that need a temporary file-backed database.
    pub fn new_at(path: &std::path::Path) -> Result<Self> {
        Self::open_at(path)
    }

    /// Crea una base de datos en memoria para pruebas.
    #[cfg(test)]
    pub fn new_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        conn.set_prepared_statement_cache_capacity(32);
        Self::create_schema(&conn)?;
        Ok(Self { conn })
    }
}
