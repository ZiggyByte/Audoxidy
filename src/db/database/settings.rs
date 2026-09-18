//! Global application settings stored in the APP_SETTINGS table.

use rusqlite::{Result, params};

use super::Database;

impl Database {
    /// Genera un nuevo ID de sesión de shuffle (UUID simple basado en timestamp + random).
    /// Obtiene un valor de configuración de la tabla APP_SETTINGS.
    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT value FROM APP_SETTINGS WHERE key = ?1",
                [key],
                |r| r.get(0),
            )
            .ok()
    }

    /// Establece un valor de configuración en la tabla APP_SETTINGS.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO APP_SETTINGS (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }
}
