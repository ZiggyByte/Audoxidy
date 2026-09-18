//! Playlist CRUD, item ordering, and shuffle-session persistence.

use rusqlite::{OptionalExtension, Result, params};

use super::Database;
use super::models::{PlaylistData, PlaylistFolderGroup, PlaylistSongRef, ShuffleSession};

impl Database {
    // ============================================================
    // PLAYLISTS - CRUD
    // ============================================================

    /// Crea una nueva playlist. Retorna el ID generado.
    pub fn create_playlist(&self, name: &str, is_system: bool) -> Result<i64> {
        // Obtener el siguiente sort_order disponible
        let max_order: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(sort_order), 0) FROM PLAYLISTS",
            [],
            |r| r.get(0),
        )?;

        self.conn.execute(
            "INSERT INTO PLAYLISTS (name, sort_order, is_system) VALUES (?1, ?2, ?3)",
            params![name, max_order + 1, if is_system { 1 } else { 0 }],
        )?;

        self.conn
            .query_row("SELECT id FROM PLAYLISTS WHERE name = ?1", [name], |r| {
                r.get(0)
            })
    }

    /// Elimina una playlist por ID. No permite eliminar playlists del sistema.
    pub fn delete_playlist(&self, playlist_id: i64) -> Result<bool> {
        // Verificar que no sea del sistema
        let is_system: bool = self
            .conn
            .query_row(
                "SELECT is_system = 1 FROM PLAYLISTS WHERE id = ?1",
                [playlist_id],
                |r| r.get(0),
            )
            .unwrap_or(false);

        if is_system {
            return Ok(false); // No se puede eliminar
        }

        let changed = self
            .conn
            .execute("DELETE FROM PLAYLISTS WHERE id = ?1", [playlist_id])?;

        Ok(changed > 0)
    }

    /// Renombra una playlist.
    pub fn rename_playlist(&self, playlist_id: i64, new_name: &str) -> Result<bool> {
        let changed = self.conn.execute(
            "UPDATE PLAYLISTS SET name = ?1 WHERE id = ?2 AND is_system = 0",
            params![new_name, playlist_id],
        )?;
        Ok(changed > 0)
    }

    /// Obtiene todas las playlists ordenadas por sort_order.
    pub fn get_all_playlists(&self) -> Result<Vec<PlaylistData>> {
        let mut stmt = self.conn.prepare("
            SELECT id, name, sort_order, is_system, created_at, last_song_id, last_pos_sec, is_playing, shuffle_active, repeat_mode, shuffle_pos, shuffle_id
             FROM PLAYLISTS ORDER BY sort_order ASC")?;

        let rows = stmt.query_map([], |r| {
            Ok(PlaylistData {
                id: r.get(0)?,
                name: r.get(1)?,
                sort_order: r.get(2)?,
                is_system: r.get::<_, i32>(3)? != 0,
                created_at: r.get(4)?,
                last_song_id: r.get(5)?,
                last_pos_sec: r.get(6)?,
                is_playing: r.get::<_, i32>(7)? != 0,
                shuffle_active: r.get::<_, i32>(8)? != 0,
                repeat_mode: r.get(9)?,
                shuffle_pos: r.get::<_, i64>(10)? as usize,
                shuffle_id: r.get(11)?,
            })
        })?;

        let mut playlists = Vec::new();
        for row in rows {
            playlists.push(row?);
        }
        Ok(playlists)
    }

    /// Obtiene una playlist por ID.
    pub fn get_playlist_by_id(&self, playlist_id: i64) -> Result<Option<PlaylistData>> {
        self.conn.query_row(
            "SELECT id, name, sort_order, is_system, created_at, last_song_id, last_pos_sec, is_playing, shuffle_active, repeat_mode, shuffle_pos, shuffle_id
             FROM PLAYLISTS WHERE id = ?1",
            [playlist_id],
            |r| {
                Ok(PlaylistData {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    sort_order: r.get(2)?,
                    is_system: r.get::<_, i32>(3)? != 0,
                    created_at: r.get(4)?,
                    last_song_id: r.get(5)?,
                    last_pos_sec: r.get(6)?,
                    is_playing: r.get::<_, i32>(7)? != 0,
                    shuffle_active: r.get::<_, i32>(8)? != 0,
                    repeat_mode: r.get(9)?,
                    shuffle_pos: r.get::<_, i64>(10)? as usize,
                    shuffle_id: r.get(11)?,
                })
            },
        ).optional()
    }

    /// Obtiene una playlist por nombre.
    pub fn get_playlist_by_name(&self, name: &str) -> Result<Option<PlaylistData>> {
        self.conn.query_row(
            "SELECT id, name, sort_order, is_system, created_at, last_song_id, last_pos_sec, is_playing, shuffle_active, repeat_mode, shuffle_pos, shuffle_id
             FROM PLAYLISTS WHERE name = ?1",
            [name],
            |r| {
                Ok(PlaylistData {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    sort_order: r.get(2)?,
                    is_system: r.get::<_, i32>(3)? != 0,
                    created_at: r.get(4)?,
                    last_song_id: r.get(5)?,
                    last_pos_sec: r.get(6)?,
                    is_playing: r.get::<_, i32>(7)? != 0,
                    shuffle_active: r.get::<_, i32>(8)? != 0,
                    repeat_mode: r.get(9)?,
                    shuffle_pos: r.get::<_, i64>(10)? as usize,
                    shuffle_id: r.get(11)?,
                })
            }
        ).optional()
    }

    /// Actualiza el orden de las playlists (para drag-and-drop de pestañas).
    pub fn update_playlist_order(&self, playlist_id: i64, new_order: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE PLAYLISTS SET sort_order = ?1 WHERE id = ?2 AND is_system = 0",
            params![new_order, playlist_id],
        )?;
        Ok(())
    }

    // ============================================================
    // PLAYLIST ITEMS - Gestión de canciones en playlists
    // ============================================================

    /// Agrega una canción a una playlist. Si ya existe, no hace nada.
    /// Retorna true si se agregó, false si ya existía.
    pub fn add_song_to_playlist(&self, playlist_id: i64, song_id: i64) -> Result<bool> {
        // Obtener el siguiente sequence_order disponible
        let max_seq: Option<f64> = self
            .conn
            .query_row(
                "SELECT MAX(sequence_order) FROM PLAYLIST_ITEMS WHERE playlist_id = ?1",
                [playlist_id],
                |r| r.get(0),
            )
            .ok()
            .flatten();

        let next_seq = max_seq.map(|s| s + 1.0).unwrap_or(1.0);

        let result = self.conn.execute(
            "INSERT OR IGNORE INTO PLAYLIST_ITEMS (playlist_id, song_id, sequence_order) VALUES (?1, ?2, ?3)",
            params![playlist_id, song_id, next_seq],
        )?;

        Ok(result > 0)
    }

    /// Agrega múltiples canciones a una playlist en orden secuencial.
    /// Retorna la cantidad de canciones agregadas exitosamente.
    pub fn add_songs_to_playlist(&mut self, playlist_id: i64, song_ids: &[i64]) -> Result<usize> {
        if song_ids.is_empty() {
            return Ok(0);
        }

        let tx = self.conn.transaction()?;

        let max_seq: Option<f64> = tx
            .query_row(
                "SELECT MAX(sequence_order) FROM PLAYLIST_ITEMS WHERE playlist_id = ?1",
                [playlist_id],
                |r| r.get(0),
            )
            .ok()
            .flatten();

        let mut next_seq = max_seq.map(|s| s + 1.0).unwrap_or(1.0);
        let mut count = 0;

        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR IGNORE INTO PLAYLIST_ITEMS (playlist_id, song_id, sequence_order) VALUES (?1, ?2, ?3)"
            )?;

            for &song_id in song_ids {
                let result = stmt.execute(params![playlist_id, song_id, next_seq])?;
                if result > 0 {
                    count += 1;
                    next_seq += 1.0;
                }
            }
        }

        tx.commit()?;
        Ok(count)
    }

    /// Remueve una canción específica de una playlist.
    pub fn remove_song_from_playlist(&self, playlist_id: i64, song_id: i64) -> Result<bool> {
        let changed = self.conn.execute(
            "DELETE FROM PLAYLIST_ITEMS WHERE playlist_id = ?1 AND song_id = ?2",
            params![playlist_id, song_id],
        )?;
        Ok(changed > 0)
    }

    /// Remueve múltiples canciones de una playlist de forma eficiente.
    pub fn batch_remove_songs_from_playlist(
        &mut self,
        playlist_id: i64,
        song_ids: &[i64],
    ) -> Result<()> {
        if song_ids.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "DELETE FROM PLAYLIST_ITEMS WHERE playlist_id = ?1 AND song_id = ?2",
            )?;
            for &song_id in song_ids {
                stmt.execute(params![playlist_id, song_id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Remueve todas las canciones de una playlist (vaciar playlist).
    pub fn clear_playlist(&self, playlist_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM PLAYLIST_ITEMS WHERE playlist_id = ?1",
            [playlist_id],
        )?;
        Ok(())
    }

    /// Cambia el estado enabled/disabled de una canción en una playlist.
    pub fn toggle_song_enabled_in_playlist(&self, playlist_id: i64, song_id: i64) -> Result<bool> {
        // Leer estado actual
        let current: bool = self.conn.query_row(
            "SELECT enabled = 1 FROM PLAYLIST_ITEMS WHERE playlist_id = ?1 AND song_id = ?2",
            params![playlist_id, song_id],
            |r| r.get(0),
        )?;

        let new_val = if current { 0 } else { 1 };
        self.conn.execute(
            "UPDATE PLAYLIST_ITEMS SET enabled = ?1 WHERE playlist_id = ?2 AND song_id = ?3",
            params![new_val, playlist_id, song_id],
        )?;

        Ok(!current) // Retorna el nuevo estado
    }

    /// Establece explícitamente el estado enabled/disabled de una canción.
    pub fn set_song_enabled_in_playlist(
        &self,
        playlist_id: i64,
        song_id: i64,
        enabled: bool,
    ) -> Result<()> {
        let val = if enabled { 1 } else { 0 };
        self.conn.execute(
            "UPDATE PLAYLIST_ITEMS SET enabled = ?1 WHERE playlist_id = ?2 AND song_id = ?3",
            params![val, playlist_id, song_id],
        )?;
        Ok(())
    }

    /// Cambia el estado enabled/disabled de todas las canciones de un folder/álbum en una playlist.
    pub fn toggle_folder_enabled_in_playlist(
        &self,
        playlist_id: i64,
        folder_path: &str,
    ) -> Result<bool> {
        // Leer estado de la primera canción del folder
        let any_enabled: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM PLAYLIST_ITEMS pi JOIN SONGS s ON s.id = pi.song_id WHERE pi.playlist_id = ?1 AND s.folder_id = (SELECT id FROM FOLDERS WHERE path = ?2) AND pi.enabled = 1)",
            params![playlist_id, folder_path],
            |r| r.get(0)
        )?;

        let new_val = if any_enabled { 0 } else { 1 };

        self.conn.execute(
            "UPDATE PLAYLIST_ITEMS SET enabled = ?1 WHERE playlist_id = ?2 AND song_id IN (SELECT id FROM SONGS WHERE folder_id = (SELECT id FROM FOLDERS WHERE path = ?3))",
            params![new_val, playlist_id, folder_path],
        )?;

        Ok(!any_enabled)
    }

    /// Establece explícitamente el estado enabled/disabled para todas las canciones de un folder.
    pub fn set_folder_enabled_in_playlist(
        &self,
        playlist_id: i64,
        folder_path: &str,
        enabled: bool,
    ) -> Result<()> {
        let val = if enabled { 1 } else { 0 };
        self.conn.execute(
            "UPDATE PLAYLIST_ITEMS SET enabled = ?1 WHERE playlist_id = ?2 AND song_id IN (SELECT id FROM SONGS WHERE folder_id = (SELECT id FROM FOLDERS WHERE path = ?3))",
            params![val, playlist_id, folder_path],
        )?;
        Ok(())
    }

    /// Actualiza el sequence_order de una canción (para reordenamiento drag-and-drop).
    pub fn update_playlist_item_order(
        &self,
        playlist_id: i64,
        song_id: i64,
        new_sequence: f64,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE PLAYLIST_ITEMS SET sequence_order = ?1 WHERE playlist_id = ?2 AND song_id = ?3",
            params![new_sequence, playlist_id, song_id],
        )?;
        Ok(())
    }

    /// Persiste el estado de reproducción de una playlist (última canción, posición, shuffle).
    pub fn update_playlist_persistence(
        &self,
        playlist_id: i64,
        last_song_id: Option<i64>,
        last_pos_sec: f64,
        is_playing: bool,
        shuffle_active: bool,
        repeat_mode: i32,
        shuffle_pos: usize,
        shuffle_id: Option<String>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE PLAYLISTS SET
                last_song_id = ?1,
                last_pos_sec = ?2,
                is_playing = ?3,
                shuffle_active = ?4,
                repeat_mode = ?5,
                shuffle_pos = ?6,
                shuffle_id = ?7
             WHERE id = ?8",
            params![
                last_song_id,
                last_pos_sec,
                if is_playing { 1 } else { 0 },
                if shuffle_active { 1 } else { 0 },
                repeat_mode,
                shuffle_pos as i64,
                shuffle_id,
                playlist_id
            ],
        )?;
        Ok(())
    }

    /// Mueve múltiples canciones de una playlist a otra de forma eficiente.
    /// Mueve canciones de una playlist a otra de forma eficiente.
    pub fn batch_move_songs_between_playlists(
        &mut self,
        from_playlist_id: i64,
        to_playlist_id: i64,
        song_ids: &[i64],
    ) -> Result<()> {
        if song_ids.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;

        let max_seq: Option<f64> = tx
            .query_row(
                "SELECT MAX(sequence_order) FROM PLAYLIST_ITEMS WHERE playlist_id = ?1",
                [to_playlist_id],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        let mut next_seq = max_seq.map(|s| s + 1.0).unwrap_or(1.0);

        {
            let mut stmt_del = tx.prepare_cached(
                "DELETE FROM PLAYLIST_ITEMS WHERE playlist_id = ?1 AND song_id = ?2",
            )?;
            let mut stmt_ins = tx.prepare_cached("INSERT INTO PLAYLIST_ITEMS (playlist_id, song_id, sequence_order) VALUES (?1, ?2, ?3)")?;

            for &song_id in song_ids {
                stmt_del.execute(params![from_playlist_id, song_id])?;
                stmt_ins.execute(params![to_playlist_id, song_id, next_seq])?;
                next_seq += 1.0;
            }
        }

        tx.commit()?;
        Ok(())
    }

    // ============================================================
    // PLAYLIST QUERIES - Consultas optimizadas
    // ============================================================

    /// Obtiene todas las canciones de una playlist con metadata completa (JOIN).
    /// Retorna las canciones ordenadas por sequence_order.
    /// Obtiene todas las canciones de una playlist con metadatos completo (JOIN).
    pub fn get_playlist_songs(&self, playlist_id: i64) -> Result<Vec<PlaylistSongRef>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT pi.id, pi.song_id, pi.sequence_order, pi.enabled = 1,
                   s.title, ar.name, al.title, al_ar.name,
                   al.year, al.genre, s.track_num, s.duration, s.file_path,
                   al.cover_path, s.cover_override,
                   f.path, f.name
            FROM PLAYLIST_ITEMS pi
            JOIN SONGS s ON s.id = pi.song_id
            JOIN ALBUMS al ON al.id = s.album_id
            JOIN ARTISTS ar ON ar.id = s.artist_id
            JOIN ARTISTS al_ar ON al_ar.id = al.artist_id
            JOIN FOLDERS f ON f.id = s.folder_id
            WHERE pi.playlist_id = ?1
            ORDER BY pi.sequence_order ASC
        ",
        )?;

        let rows = stmt.query_map([playlist_id], |r| {
            let title_val: Option<String> = r.get(4)?;
            let artist_val: Option<String> = r.get(5)?;
            let album_val: Option<String> = r.get(6)?;

            Ok(PlaylistSongRef {
                item_id: r.get(0)?,
                song_id: r.get(1)?,
                sequence_order: r.get(2)?,
                enabled: r.get(3)?,
                title: crate::utils::interner::intern_string(
                    &title_val.unwrap_or_else(|| "Sin título".to_string()),
                ),
                artist_name: crate::utils::interner::intern_string(
                    &artist_val.unwrap_or_else(|| "Artista desconocido".to_string()),
                ),
                album_title: crate::utils::interner::intern_string(
                    &album_val.unwrap_or_else(|| "Álbum desconocido".to_string()),
                ),
                album_artist_name: crate::utils::interner::intern_string(
                    &r.get::<_, Option<String>>(7)?
                        .unwrap_or_else(|| "Artista desconocido".to_string()),
                ),
                year: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(8)?.as_deref(),
                ),
                genre: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(9)?.as_deref(),
                ),
                track_number: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(10)?.as_deref(),
                ),
                duration: r.get::<_, Option<f64>>(11)?.unwrap_or(0.0),
                file_path: crate::utils::interner::intern_string(&r.get::<_, String>(12)?),
                cover_path: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(13)?.as_deref(),
                ),
                cover_override: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(14)?.as_deref(),
                ),
                folder_path: crate::utils::interner::intern_string(&r.get::<_, String>(15)?),
                folder_name: crate::utils::interner::intern_string(&r.get::<_, String>(16)?),
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            if let Ok(s) = row {
                results.push(s);
            }
        }
        Ok(results)
    }

    /// Obtiene canciones de una playlist paginadas (para virtualización).
    /// LIMIT + OFFSET para renderizado eficiente.
    /// Obtiene canciones de una playlist paginadas para virtualización.
    pub fn get_playlist_songs_paginated(
        &self,
        playlist_id: i64,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<PlaylistSongRef>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT pi.id, pi.song_id, pi.sequence_order, pi.enabled = 1,
                   s.title, ar.name, al.title, al_ar.name,
                   al.year, al.genre, s.track_num, s.duration, s.file_path,
                   al.cover_path, s.cover_override,
                   f.path, f.name
            FROM PLAYLIST_ITEMS pi
            JOIN SONGS s ON s.id = pi.song_id
            JOIN ALBUMS al ON al.id = s.album_id
            JOIN ARTISTS ar ON ar.id = s.artist_id
            JOIN ARTISTS al_ar ON al_ar.id = al.artist_id
            JOIN FOLDERS f ON f.id = s.folder_id
            WHERE pi.playlist_id = ?1
            ORDER BY pi.sequence_order ASC
            LIMIT ?2 OFFSET ?3
        ",
        )?;

        let rows = stmt.query_map(params![playlist_id, limit as i64, offset as i64], |r| {
            let title_val: Option<String> = r.get(4)?;
            let artist_val: Option<String> = r.get(5)?;
            let album_val: Option<String> = r.get(6)?;

            Ok(PlaylistSongRef {
                item_id: r.get(0)?,
                song_id: r.get(1)?,
                sequence_order: r.get(2)?,
                enabled: r.get(3)?,
                title: crate::utils::interner::intern_string(
                    &title_val.unwrap_or_else(|| "Sin título".to_string()),
                ),
                artist_name: crate::utils::interner::intern_string(
                    &artist_val.unwrap_or_else(|| "Artista desconocido".to_string()),
                ),
                album_title: crate::utils::interner::intern_string(
                    &album_val.unwrap_or_else(|| "Álbum desconocido".to_string()),
                ),
                album_artist_name: crate::utils::interner::intern_string(
                    &r.get::<_, Option<String>>(7)?
                        .unwrap_or_else(|| "Artista desconocido".to_string()),
                ),
                year: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(8)?.as_deref(),
                ),
                genre: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(9)?.as_deref(),
                ),
                track_number: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(10)?.as_deref(),
                ),
                duration: r.get::<_, Option<f64>>(11)?.unwrap_or(0.0),
                file_path: crate::utils::interner::intern_string(&r.get::<_, String>(12)?),
                cover_path: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(13)?.as_deref(),
                ),
                cover_override: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(14)?.as_deref(),
                ),
                folder_path: crate::utils::interner::intern_string(&r.get::<_, String>(15)?),
                folder_name: crate::utils::interner::intern_string(&r.get::<_, String>(16)?),
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            if let Ok(s) = row {
                results.push(s);
            }
        }
        Ok(results)
    }

    /// Obtiene el total de canciones de una playlist (para virtualización).
    /// Obtiene el total de canciones en una playlist.
    pub fn get_playlist_song_count(&self, playlist_id: i64) -> Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM PLAYLIST_ITEMS WHERE playlist_id = ?1",
            [playlist_id],
            |r| r.get(0),
        )
    }

    /// Obtiene canciones de una playlist agrupadas por folder_path.
    /// Retorna grupos con nombre de carpeta, canciones y estadísticas.
    /// Obtiene canciones de una playlist agrupadas por carpeta.
    pub fn get_playlist_songs_grouped_by_folder(
        &self,
        playlist_id: i64,
    ) -> Result<Vec<PlaylistFolderGroup>> {
        let songs = self.get_playlist_songs(playlist_id)?;
        let mut groups: Vec<PlaylistFolderGroup> = Vec::new();

        for song in songs {
            // Encontrar o crear el grupo (solo si es el último grupo para mantener el orden de la lista)
            let is_same_folder = groups
                .last()
                .map(|g| g.folder_path == song.folder_path)
                .unwrap_or(false);

            if is_same_folder {
                if let Some(group) = groups.last_mut() {
                    group.total_duration += song.duration;
                    group.songs.push(song);
                }
            } else {
                let first_item_id = song.item_id;
                let folder_path = song.folder_path.clone();
                let folder_name = song.folder_name.clone();
                let duration = song.duration;

                groups.push(PlaylistFolderGroup {
                    folder_path,
                    folder_name,
                    total_duration: duration,
                    first_item_id,
                    songs: vec![song],
                });
            }
        }

        Ok(groups)
    }

    /// Obtiene estadísticas de una playlist: total canciones, duración total, tamaño total.
    /// Obtiene estadísticas de una playlist (conteo, duración total, tamaño total).
    pub fn get_playlist_stats(&self, playlist_id: i64) -> Result<(usize, f64, f64)> {
        self.conn.query_row(
            "
            SELECT COUNT(*), COALESCE(SUM(s.duration), 0.0), COALESCE(SUM(s.size), 0.0)
            FROM PLAYLIST_ITEMS pi
            JOIN SONGS s ON s.id = pi.song_id
            WHERE pi.playlist_id = ?1
            ",
            [playlist_id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)? as usize,
                    r.get::<_, f64>(1)?,
                    r.get::<_, f64>(2)?,
                ))
            },
        )
    }

    /// Verifica si una canción específica está en una playlist.
    /// Verifica si una canción está en una playlist.
    pub fn is_song_in_playlist(&self, playlist_id: i64, song_id: i64) -> Result<bool> {
        self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM PLAYLIST_ITEMS WHERE playlist_id = ?1 AND song_id = ?2)",
            params![playlist_id, song_id],
            |r| r.get(0),
        )
    }

    // ============================================================
    // SHUFFLE HISTORY - Persistencia de sesiones de reproducción aleatoria
    // ============================================================

    /// Genera un ID único de sesión de shuffle (timestamp + random).
    pub fn generate_shuffle_session_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let random = fastrand::u64(0..u64::MAX);
        format!("{}-{:x}", timestamp, random)
    }

    /// Guarda una sesión de shuffle completa en la base de datos.
    /// Esto persiste el orden aleatorio y el historial para navegación backward.
    /// Persiste una sesión de shuffle completa (orden + historial) en la BD.
    pub fn save_shuffle_session(
        &mut self,
        playlist_id: i64,
        session: &ShuffleSession,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;

        // Limpiar sesión previa del mismo playlist (si existe)
        tx.execute(
            "DELETE FROM PLAYLIST_SHUFFLE_HISTORY WHERE playlist_id = ?1",
            [playlist_id],
        )?;

        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO PLAYLIST_SHUFFLE_HISTORY (playlist_id, session_id, song_id, play_order) VALUES (?1, ?2, ?3, ?4)"
            )?;

            // Insertar shuffle_order (canciones pendientes)
            for (i, &song_id) in session.shuffle_order.iter().enumerate() {
                // play_order negativo para diferenciar de history
                stmt.execute(params![
                    playlist_id,
                    session.session_id,
                    song_id,
                    -(i as i64)
                ])?;
            }

            // Insertar history (canciones ya reproducidas)
            for (i, &song_id) in session.history.iter().enumerate() {
                stmt.execute(params![playlist_id, session.session_id, song_id, i as i64])?;
            }
        }

        // Actualizar también en la tabla PLAYLISTS
        tx.execute(
            "UPDATE PLAYLISTS SET shuffle_pos = ?1, shuffle_id = ?2 WHERE id = ?3",
            params![
                session.current_position as i64,
                session.session_id,
                playlist_id
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Carga la última sesión de shuffle de una playlist.
    /// Carga la última sesión de shuffle de una playlist desde la BD.
    pub fn load_shuffle_session(&self, playlist_id: i64) -> Result<Option<ShuffleSession>> {
        // Obtener la sesión más reciente
        let session_id: Option<String> = self.conn.query_row(
            "SELECT session_id FROM PLAYLIST_SHUFFLE_HISTORY WHERE playlist_id = ?1 ORDER BY played_at DESC LIMIT 1",
            [playlist_id],
            |r| r.get(0)
        ).ok().flatten();

        let session_id = match session_id {
            Some(id) => id,
            None => return Ok(None),
        };

        // Cargar history (play_order >= 0)
        let mut stmt_history = self.conn.prepare(
            "SELECT song_id FROM PLAYLIST_SHUFFLE_HISTORY WHERE playlist_id = ?1 AND session_id = ?2 AND play_order >= 0 ORDER BY play_order ASC"
        )?;

        let mut history = Vec::new();
        let history_rows =
            stmt_history.query_map(params![playlist_id, session_id], |r| r.get(0))?;
        for row in history_rows {
            if let Ok(song_id) = row {
                history.push(song_id);
            }
        }

        // Cargar shuffle_order (play_order < 0), ordenado por valor absoluto
        let mut stmt_order = self.conn.prepare(
            "SELECT song_id FROM PLAYLIST_SHUFFLE_HISTORY WHERE playlist_id = ?1 AND session_id = ?2 AND play_order < 0 ORDER BY play_order DESC"
        )?;

        let mut shuffle_order = Vec::new();
        let order_rows = stmt_order.query_map(params![playlist_id, session_id], |r| r.get(0))?;
        for row in order_rows {
            if let Ok(song_id) = row {
                shuffle_order.push(song_id);
            }
        }

        // 3. Obtener la posición actual desde PLAYLISTS
        let current_position: usize = self
            .conn
            .query_row(
                "SELECT shuffle_pos FROM PLAYLISTS WHERE id = ?1",
                [playlist_id],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0) as usize;

        Ok(Some(ShuffleSession {
            session_id,
            shuffle_order,
            current_position,
            history,
        }))
    }

    /// Limpia la sesión de shuffle de una playlist.
    /// Limpia la sesión de shuffle de una playlist.
    pub fn clear_shuffle_session(&self, playlist_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM PLAYLIST_SHUFFLE_HISTORY WHERE playlist_id = ?1",
            [playlist_id],
        )?;
        Ok(())
    }

    /// Obtiene el total de canciones en una playlist (para uso en shuffle).
    /// Incluye canciones disabled también.
    /// Obtiene el total de canciones en una playlist (incluyendo disabled).
    pub fn get_playlist_total_count(&self, playlist_id: i64) -> Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM PLAYLIST_ITEMS WHERE playlist_id = ?1",
            [playlist_id],
            |r| r.get(0),
        )
    }

    /// Obtiene las canciones enabled de una playlist (para shuffle).
    /// Obtiene los IDs de canciones habilitadas en una playlist.
    pub fn get_playlist_enabled_songs(&self, playlist_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT song_id FROM PLAYLIST_ITEMS WHERE playlist_id = ?1 AND enabled = 1 ORDER BY sequence_order ASC"
        )?;

        let rows = stmt.query_map([playlist_id], |r| r.get(0))?;

        let mut results = Vec::new();
        for row in rows {
            if let Ok(song_id) = row {
                results.push(song_id);
            }
        }
        Ok(results)
    }

    /// Obtiene todos los IDs de canciones de una playlist en orden.
    pub fn get_playlist_all_song_ids(&self, playlist_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT song_id FROM PLAYLIST_ITEMS WHERE playlist_id = ?1 ORDER BY sequence_order ASC",
        )?;

        let rows = stmt.query_map([playlist_id], |r| r.get(0))?;

        let mut results = Vec::new();
        for row in rows {
            if let Ok(song_id) = row {
                results.push(song_id);
            }
        }
        Ok(results)
    }
}
