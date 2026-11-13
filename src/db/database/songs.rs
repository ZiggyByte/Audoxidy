//! Song, artist, album, and folder persistence.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::Path;

use hex;
use rusqlite::{OptionalExtension, Result, params};
use sha2::{Digest, Sha256};

use super::Database;
use super::models::{SongData, SongMetadataExtended};

impl Database {
    // --- Helpers de Hashing para Identidad Basada en Rutas ---

    /// Genera un hash SHA-256 para identificación única de rutas y entidades.
    pub fn generate_hash(input: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input);
        hex::encode(hasher.finalize())
    }

    // --- Métodos de Inserción Relacional ---

    /// Inserta o recupera el ID de una carpeta por su ruta.
    pub fn upsert_folder(&self, path: &str) -> Result<i64> {
        self.conn
            .prepare_cached(
                "INSERT INTO FOLDERS (path, name) VALUES (?1, ?2)
             ON CONFLICT(path) DO NOTHING",
            )?
            .execute(params![
                path,
                std::path::Path::new(path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Música")
            ])?;
        self.conn
            .prepare_cached("SELECT id FROM FOLDERS WHERE path = ?1")?
            .query_row([path], |row| row.get(0))
    }

    /// Inserta o recupera el ID de un artista por nombre (hash normalizado).
    pub fn upsert_artist(&self, name: &str) -> Result<i64> {
        let normalized_name = name.trim();
        let hash_id = Self::generate_hash(&normalized_name.to_lowercase());
        self.conn
            .prepare_cached(
                "INSERT INTO ARTISTS (name, hash_id) VALUES (?1, ?2)
             ON CONFLICT(hash_id) DO NOTHING",
            )?
            .execute(params![normalized_name, hash_id])?;
        self.conn
            .prepare_cached("SELECT id FROM ARTISTS WHERE hash_id = ?1")?
            .query_row([hash_id], |row| row.get(0))
    }

    /// Inserta o actualiza un álbum por título, artista, carpeta, año y género.
    pub fn upsert_album(
        &self,
        title: &str,
        artist_id: i64,
        folder_id: i64,
        folder_path: &str,
        year: Option<&str>,
        genre: Option<&str>,
    ) -> Result<i64> {
        let hash_id = Self::generate_hash(&format!("{}{}{}", title, artist_id, folder_path));
        self.conn
            .prepare_cached(
                "INSERT INTO ALBUMS (title, artist_id, folder_id, hash_id, year, genre)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(hash_id) DO UPDATE SET
                year = COALESCE(year, excluded.year),
                genre = COALESCE(genre, excluded.genre)",
            )?
            .execute(params![title, artist_id, folder_id, hash_id, year, genre])?;
        self.conn
            .prepare_cached("SELECT id FROM ALBUMS WHERE hash_id = ?1")?
            .query_row([hash_id], |row| row.get(0))
    }

    /// Inserts one song with its metadata and its cover decision.
    ///
    /// Returns (cover_file_name, needs_processing). The name is the complete file
    /// name the writer stores and the scanner checks, and it includes the
    /// extension; it arrives empty when the insert produces no file.
    pub fn insert_song_full(
        &mut self,
        song: &SongData,
        extended: &SongMetadataExtended,
        pic_bytes_hash: Option<String>,
        is_external: bool,
    ) -> Result<(String, bool)> {
        let folder_path = std::path::Path::new(song.full_file_path.as_ref())
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("");

        let folder_id = self.upsert_folder(folder_path)?;

        // Identidades de Artista por Separado (Pista vs Álbum)
        let raw_track_artist = song.artist.as_deref().unwrap_or("Desconocido");
        let raw_album_artist = song
            .album_artist
            .as_deref()
            .or(song.artist.as_deref())
            .unwrap_or("Desconocido");

        let track_artist_id = self.upsert_artist(raw_track_artist)?;
        let album_artist_id = self.upsert_artist(raw_album_artist)?;

        let album_title = song.album.as_deref().unwrap_or("Desconocido");
        let album_hash_id = Self::generate_hash(&format!(
            "{}{}{}",
            album_title, album_artist_id, folder_path
        ));

        // 1. Resolver o crear álbum (Aseguramos que el álbum pertenezca al artista del álbum)
        self.conn
            .prepare_cached(
                "INSERT INTO ALBUMS (title, artist_id, folder_id, hash_id, year, genre)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(hash_id) DO UPDATE SET
                artist_id = excluded.artist_id,
                year = COALESCE(year, excluded.year),
                genre = COALESCE(genre, excluded.genre)",
            )?
            .execute(params![
                album_title,
                album_artist_id,
                folder_id,
                album_hash_id,
                song.release_year.as_deref(),
                song.genre.as_deref()
            ])?;

        let (_, _): (Option<String>, Option<String>) = self
            .conn
            .prepare_cached("SELECT cover_path, cover_hash FROM ALBUMS WHERE hash_id = ?1")?
            .query_row([&album_hash_id], |row| Ok((row.get(0)?, row.get(1)?)))?;

        let mut final_song_cover_override = None;
        let mut needs_processing = false;
        // Complete file name the writer stores and the scanner joins; stays empty
        // when this insert produces no cover file.
        let mut target_file_name_for_processing = String::new();

        if let Some(current_pic_hash) = pic_bytes_hash {
            // Verificar si el álbum ya tiene carátula
            let album_has_cover: bool = self
                .conn
                .prepare_cached("SELECT cover_hash IS NOT NULL FROM ALBUMS WHERE hash_id = ?1")?
                .query_row([&album_hash_id], |row| row.get(0))?;

            if !album_has_cover {
                // Es la primera carátula del álbum, la asignamos como principal
                match crate::utils::covers::album_cover_file_name(&album_hash_id, &current_pic_hash)
                {
                    Ok(file_name) => {
                        let path = crate::utils::paths::get()
                            .cover_cache_dir
                            .join(&file_name)
                            .to_string_lossy()
                            .into_owned();

                        self.conn
                            .prepare_cached(
                                "UPDATE ALBUMS SET cover_hash = ?1, cover_path = ?2 WHERE hash_id = ?3",
                            )?
                            .execute(params![current_pic_hash, path, album_hash_id])?;
                        target_file_name_for_processing = file_name;
                        needs_processing = true;
                    }
                    Err(error) => tracing::warn!(
                        target: "audoxidy::persistence",
                        album = %album_hash_id,
                        error = %error,
                        "No se pudo componer el nombre de la carátula del álbum"
                    ),
                }
            } else {
                let album_cover_hash: Option<String> = self
                    .conn
                    .prepare_cached("SELECT cover_hash FROM ALBUMS WHERE hash_id = ?1")?
                    .query_row([&album_hash_id], |row| row.get(0))?;

                if album_cover_hash.as_ref() != Some(&current_pic_hash) {
                    // A different image than the album's cover. The hash is stored
                    // pure: the file name is composed from the song id, which does
                    // not exist yet.
                    let song_cover_hash = Self::generate_hash(&format!(
                        "{}{}",
                        song.full_file_path, current_pic_hash
                    ));
                    final_song_cover_override = Some(song_cover_hash);
                    needs_processing = true;
                }
            }
        }

        // 2. Inserción de la canción (Con su propio artista de pista)
        self.conn.prepare_cached(
            "INSERT INTO SONGS (
                album_id, artist_id, folder_id, file_path, title, track_num, duration, format,
                bit_depth, sample_rate, channels, size, cover_override, import_order, is_external
            ) VALUES (
                (SELECT id FROM ALBUMS WHERE hash_id = ?1), ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
            ) ON CONFLICT(file_path) DO UPDATE SET
                title = excluded.title, track_num = excluded.track_num, duration = excluded.duration,
                artist_id = excluded.artist_id, cover_override = excluded.cover_override, is_external = excluded.is_external"
        )?.execute(
            params![
                album_hash_id, track_artist_id, folder_id, song.full_file_path, song.title,
                song.track_number,
                song.duration_secs, song.format, song.bit_depth, song.sample_rate,
                song.channels, song.size, final_song_cover_override, song.import_order,
                if is_external { 1 } else { 0 }
            ],
        )?;

        let song_id: i64 = self
            .conn
            .prepare_cached("SELECT id FROM SONGS WHERE file_path = ?1")?
            .query_row([&song.full_file_path], |row| row.get(0))?;

        // The per-song name carries the song id, which only exists after the insert.
        if let Some(cover_hash) = final_song_cover_override.as_deref() {
            match crate::utils::covers::song_cover_file_name(song_id, cover_hash) {
                Ok(file_name) => target_file_name_for_processing = file_name,
                Err(error) => {
                    tracing::warn!(
                        target: "audoxidy::persistence",
                        song_id,
                        error = %error,
                        "No se pudo componer el nombre de la carátula propia de una canción"
                    );
                    needs_processing = false;
                }
            }
        }

        // 3. Metadatos extendidos (Vista unificada)
        self.conn.prepare_cached(
            "INSERT INTO SONG_METADATA (
                song_id, lyrics, comments, composer, lyricist, publisher, url, copyright,
                encoded_by, catalog, isrc, key, bpm, track_gain, album_gain
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ON CONFLICT(song_id) DO UPDATE SET lyrics = excluded.lyrics, comments = excluded.comments"
        )?.execute(
            params![
                song_id, extended.lyrics, extended.comments, extended.composer, extended.lyricist,
                extended.publisher, extended.url, extended.copyright, extended.encoded_by,
                extended.catalog, extended.isrc, extended.key, extended.bpm, extended.track_gain, extended.album_gain
            ],
        )?;

        Ok((target_file_name_for_processing, needs_processing))
    }

    /// Obtiene el ID de una canción por su ruta de archivo.
    pub fn get_song_id_by_path(&self, path: &str) -> Result<Option<i64>> {
        self.conn
            .query_row("SELECT id FROM SONGS WHERE file_path = ?1", [path], |row| {
                row.get(0)
            })
            .optional()
    }

    /// Obtiene los valores de ReplayGain (track_gain, album_gain) por ruta de archivo.
    /// Consulta eficiente con JOIN directo (evita dos queries separadas).
    pub fn get_replay_gain_by_path(&self, path: &str) -> Result<(Option<f64>, Option<f64>)> {
        self.conn
            .prepare_cached(
                "SELECT m.track_gain, m.album_gain
             FROM SONGS s
             JOIN SONG_METADATA m ON m.song_id = s.id
             WHERE s.file_path = ?1",
            )?
            .query_row([path], |row| Ok((row.get(0)?, row.get(1)?)))
            .optional()
            .map(|opt| opt.unwrap_or((None, None)))
    }

    /// Obtiene las características técnicas de la canción (sample_rate, bit_depth, channels) por ruta.
    pub fn get_song_technical_meta_by_path(&self, path: &str) -> Result<Option<(u32, u32, u32)>> {
        self.conn
            .prepare_cached(
                "SELECT sample_rate, bit_depth, channels FROM SONGS WHERE file_path = ?1",
            )?
            .query_row([path], |row| {
                Ok((
                    row.get::<_, Option<i32>>(0)?.unwrap_or(0) as u32,
                    row.get::<_, Option<i32>>(1)?.unwrap_or(0) as u32,
                    row.get::<_, Option<i32>>(2)?.unwrap_or(0) as u32,
                ))
            })
            .optional()
    }

    pub fn get_song_format_by_path(&self, path: &str) -> Result<Option<String>> {
        self.conn
            .prepare_cached("SELECT format FROM SONGS WHERE file_path = ?1")?
            .query_row([path], |row| row.get::<_, Option<String>>(0))
            .optional()
            .map(|opt| opt.flatten())
    }

    // --- Métodos de Transacción ---

    pub fn begin_transaction(&self) -> Result<()> {
        self.conn.execute("BEGIN TRANSACTION", [])?;
        Ok(())
    }

    pub fn commit_transaction(&self) -> Result<()> {
        self.conn.execute("COMMIT", [])?;
        Ok(())
    }

    pub fn rollback_transaction(&self) -> Result<()> {
        self.conn.execute("ROLLBACK", [])?;
        Ok(())
    }

    /// Vacía la caché de sentencias preparadas de la conexión.
    ///
    /// Se invoca al completar un escaneo masivo para acotar la memoria que
    /// retiene la caché; las sentencias se vuelven a preparar de forma
    /// transparente en el siguiente uso.
    pub fn flush_prepared_statements(&self) {
        self.conn.flush_prepared_statement_cache();
    }

    /// Elimina una canción por su ruta de archivo.
    pub fn delete_song(&self, path: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM SONGS WHERE file_path = ?1", [path])?;
        Ok(())
    }

    /// Elimina un álbum completo y todas sus canciones.
    pub fn delete_album_group(&self, album_hash: &str) -> Result<()> {
        // En el nuevo esquema usamos hash_id para identificar álbumes
        self.conn.execute(
            "DELETE FROM SONGS WHERE album_id = (SELECT id FROM ALBUMS WHERE hash_id = ?1)",
            [album_hash],
        )?;
        self.conn
            .execute("DELETE FROM ALBUMS WHERE hash_id = ?1", [album_hash])?;
        Ok(())
    }

    /// Elimina múltiples canciones por sus IDs.
    pub fn batch_delete_songs(&self, ids: &[i64]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut stmt = self.conn.prepare("DELETE FROM SONGS WHERE id = ?1")?;
        for id in ids {
            stmt.execute([id])?;
        }
        Ok(())
    }

    /// Limpia álbumes sin canciones y artistas huérfanos.
    pub fn cleanup_empty_metadata(&self) -> Result<()> {
        // Eliminar álbumes sin canciones
        self.conn.execute(
            "DELETE FROM ALBUMS WHERE NOT EXISTS (SELECT 1 FROM SONGS WHERE album_id = ALBUMS.id)",
            [],
        )?;
        // Eliminar artistas sin álbumes ni canciones
        self.conn.execute(
            "DELETE FROM ARTISTS WHERE NOT EXISTS (SELECT 1 FROM ALBUMS WHERE artist_id = ARTISTS.id)
             AND NOT EXISTS (SELECT 1 FROM SONGS WHERE artist_id = ARTISTS.id)",
            []
        )?;
        Ok(())
    }

    /// Returns the cover references of the given songs.
    ///
    /// Each returned tuple is `(album cover path, raw override hash, song id)`.
    /// Reading it before a delete transaction commits is what lets the caller
    /// know which files the disappearing rows still contributed.
    pub fn cover_refs_for_songs(
        &self,
        ids: &[i64],
    ) -> Result<Vec<(Option<String>, Option<String>, i64)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = self.conn.prepare_cached(
            "SELECT al.cover_path, s.cover_override, s.id
             FROM SONGS s
             JOIN ALBUMS al ON al.id = s.album_id
             WHERE s.id = ?1",
        )?;

        let mut references = Vec::with_capacity(ids.len());
        for id in ids {
            let reference = stmt
                .query_row([id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .optional()?;
            if let Some(reference) = reference {
                references.push(reference);
            }
        }

        Ok(references)
    }

    /// Returns whether any album row still stores a cover path with this file name.
    ///
    /// The stored path can move with the working directory or the configured
    /// cache, so the comparison is by file name: the rows are selected and their
    /// `file_name()` compared in Rust. A count cannot be compared against a name.
    pub fn is_cover_path_referenced(&self, file_name: &str) -> Result<bool> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT cover_path FROM ALBUMS WHERE cover_path IS NOT NULL")?;
        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            let cover_path: String = row.get(0)?;
            if std::path::Path::new(&cover_path)
                .file_name()
                .and_then(|name| name.to_str())
                == Some(file_name)
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Returns whether a song still holds this override hash.
    ///
    /// Override file names embed the song id, so the owner is part of the
    /// predicate: a surviving row with a different owner cannot reference it.
    pub fn is_cover_override_referenced(&self, song_id: i64, cover_hash: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM SONGS WHERE id = ?1 AND cover_override = ?2",
            params![song_id, cover_hash],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Returns every cover file name a surviving row references.
    ///
    /// Two statements, one lock scope: every non-null `ALBUMS.cover_path` reduced
    /// to its file name, and every non-null `SONGS.cover_override` composed into
    /// the name owned by its song. The set holds file names, never full paths, so
    /// a moved working directory or a differently configured cache directory
    /// cannot make the comparison disagree with the files on disk.
    ///
    /// A failed statement propagates: a partial set would look like a set of
    /// orphans, so the caller must refuse to delete anything rather than guess.
    pub fn referenced_cover_file_names(&self) -> Result<HashSet<OsString>> {
        let mut referenced = HashSet::new();

        {
            let mut stmt = self
                .conn
                .prepare_cached("SELECT cover_path FROM ALBUMS WHERE cover_path IS NOT NULL")?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let cover_path: String = row.get(0)?;
                if let Some(file_name) = Path::new(&cover_path).file_name() {
                    referenced.insert(file_name.to_os_string());
                }
            }
        }

        {
            let mut stmt = self.conn.prepare_cached(
                "SELECT id, cover_override FROM SONGS WHERE cover_override IS NOT NULL",
            )?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let song_id: i64 = row.get(0)?;
                let cover_hash: String = row.get(1)?;
                match crate::utils::covers::song_cover_file_name(song_id, &cover_hash) {
                    Ok(file_name) => {
                        referenced.insert(OsString::from(file_name));
                    }
                    Err(error) => tracing::warn!(
                        target: "audoxidy::persistence",
                        song_id,
                        error = %error,
                        "No se pudo componer el nombre de la carátula propia de una canción"
                    ),
                }
            }
        }

        Ok(referenced)
    }
}
