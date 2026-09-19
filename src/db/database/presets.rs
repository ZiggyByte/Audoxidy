//! Persistence for user-defined equalizer presets.
//!
//! The methods below use raw f32 parameters and byte blobs, removing
//! the cross-layer dependency on `audio::preset::EqPreset`. Preset
//! CRUD at the Core boundary lives in `core::settings`.

use rusqlite::Result;

use super::Database;

impl Database {
    /// Saves or overwrites a preset using raw f32 parameters.
    pub fn save_eq_preset_raw(
        &self,
        name: &str,
        preamp_gain: f32,
        bands_20: Option<Vec<u8>>,
        bands_31: Option<Vec<u8>>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO eq_presets (name, preamp_gain, bands_20, bands_31) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![name, preamp_gain, bands_20, bands_31],
        )?;
        Ok(())
    }

    /// Loads all presets as raw tuples of (name, preamp_gain, bands_20_blob,
    /// bands_31_blob), ordered alphabetically.
    pub fn load_eq_presets_raw(
        &self,
    ) -> Result<Vec<(String, f32, Option<Vec<u8>>, Option<Vec<u8>>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, preamp_gain, bands_20, bands_31 FROM eq_presets ORDER BY name COLLATE NOCASE"
        )?;
        let presets = stmt
            .query_map([], |row| {
                let name: String = row.get(0)?;
                let preamp_gain: f32 = row.get(1)?;
                let bands_20_blob: Option<Vec<u8>> = row.get(2)?;
                let bands_31_blob: Option<Vec<u8>> = row.get(3)?;
                Ok((name, preamp_gain, bands_20_blob, bands_31_blob))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(presets)
    }

    /// Deletes a preset by name.
    pub fn delete_eq_preset_by_name(&self, name: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM eq_presets WHERE name = ?1",
            rusqlite::params![name],
        )?;
        Ok(())
    }

    /// Deletes all presets.
    pub fn clear_all_eq_presets(&self) -> Result<()> {
        self.conn.execute("DELETE FROM eq_presets", [])?;
        Ok(())
    }
}
