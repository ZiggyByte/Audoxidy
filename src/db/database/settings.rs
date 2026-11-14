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

    /// Loads every stored setting in one statement.
    ///
    /// Rows whose value is null are left out, which is what reproduces the
    /// single-key read for that key: an absent key and a null value are
    /// indistinguishable to a caller, and both resolve to `None`.
    ///
    /// # Errors
    ///
    /// Returns an error when the statement cannot be prepared or a row cannot be
    /// decoded.
    pub fn load_all_settings(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT key, value FROM APP_SETTINGS WHERE value IS NOT NULL")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut settings = std::collections::HashMap::new();
        for row in rows {
            let (key, value) = row?;
            settings.insert(key, value);
        }
        Ok(settings)
    }

    /// Establece un valor de configuración en la tabla APP_SETTINGS.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO APP_SETTINGS (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Deletes a setting.
    ///
    /// This is the delete counterpart of [`Self::set_setting`] and the third verb
    /// of the settings surface; removing a key that does not exist is a no-op.
    ///
    /// # Errors
    ///
    /// Returns an error when the statement cannot be executed.
    pub fn delete_setting(&self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM APP_SETTINGS WHERE key = ?1", [key])?;
        Ok(())
    }
}
