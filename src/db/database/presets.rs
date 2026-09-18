//! Persistence for user-defined equalizer presets.

use rusqlite::Result;

use super::Database;
use crate::audio::preset::EqPreset;

impl Database {
    /// Guarda o sobrescribe un preset personalizado en la base de datos.
    pub fn save_eq_preset(&self, preset: &EqPreset) -> rusqlite::Result<()> {
        let bands_20_bytes = serialize_f32_blob(preset.bands_20.as_ref());
        let bands_31_bytes = serialize_f32_blob(preset.bands_31.as_ref());
        self.conn.execute(
            "INSERT OR REPLACE INTO eq_presets (name, preamp_gain, bands_20, bands_31) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                preset.name,
                preset.preamp_gain,
                bands_20_bytes,
                bands_31_bytes,
            ],
        )?;
        Ok(())
    }

    /// Carga todos los presets personalizados ordenados alfabéticamente (case-insensitive).
    pub fn load_eq_presets(&self) -> rusqlite::Result<Vec<EqPreset>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, preamp_gain, bands_20, bands_31 FROM eq_presets ORDER BY name COLLATE NOCASE"
        )?;
        let presets = stmt
            .query_map([], |row| {
                let name: String = row.get(0)?;
                let preamp_gain: f32 = row.get(1)?;
                let bands_20_blob: Option<Vec<u8>> = row.get(2)?;
                let bands_31_blob: Option<Vec<u8>> = row.get(3)?;

                let bands_20 = deserialize_f32_blob(bands_20_blob);
                let bands_31 = deserialize_f32_blob(bands_31_blob);

                Ok(EqPreset::new(&name, preamp_gain, bands_20, bands_31))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(presets)
    }

    /// Elimina un preset personalizado por su nombre.
    pub fn delete_eq_preset(&self, name: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "DELETE FROM eq_presets WHERE name = ?1",
            rusqlite::params![name],
        )?;
        Ok(())
    }

    /// Elimina todos los presets personalizados (restaurar valores predeterminados).
    pub fn clear_eq_presets(&self) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM eq_presets", [])?;
        Ok(())
    }
}

// ──────────────────────────────────────────────
// BLOB serialization helpers (manual f32 ↔ [u8;4], no bytemuck)
// ──────────────────────────────────────────────

/// Serializa un slice de f32 a Vec<u8> usando little-endian byte representation.
fn serialize_f32_blob(bands: Option<&Vec<f32>>) -> Option<Vec<u8>> {
    bands.map(|v| v.iter().flat_map(|f| f.to_le_bytes()).collect())
}

/// Deserializa un blob de bytes a Vec<f32>, validando que la longitud sea múltiplo de 4.
/// Retorna None si el blob está vacío, es inválido, o tiene longitud no múltiplo de 4.
fn deserialize_f32_blob(blob: Option<Vec<u8>>) -> Option<Vec<f32>> {
    blob.and_then(|bytes| {
        if bytes.is_empty() || bytes.len() % 4 != 0 {
            return None;
        }
        Some(
            bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                .collect(),
        )
    })
}
