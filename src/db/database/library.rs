//! Library-wide read queries for songs, albums, folders, and filters.

use rusqlite::{Result, params};
use std::sync::Arc;

use super::Database;
use super::fts5_match_query;
use super::models::{LibrarySearchParams, SongData};

impl Database {
    // --- Consultas de Alto Rendimiento ---

    /// Busca canciones usando FTS5 con el término de búsqueda dado.
    pub fn search_songs(&self, query: &str) -> Result<Vec<Arc<SongData>>> {
        let Some(fts_query) = fts5_match_query(query) else {
            return Ok(Vec::new());
        };

        let mut stmt = self.conn.prepare(
            "
            SELECT s.id, s.file_path, s.title, ar.name, al.title, al.cover_path,
                   s.duration, s.format, s.size, s.track_num, s.bit_depth, s.sample_rate,
                   s.channels, s.embedded_cover, s.cover_override, s.import_order,
                   al.genre, al.year, al_ar.name
            FROM SONGS_FTS f
            JOIN SONGS s ON f.rowid = s.id
            JOIN ARTISTS ar ON s.artist_id = ar.id
            JOIN ALBUMS al ON s.album_id = al.id
            JOIN ARTISTS al_ar ON al.artist_id = al_ar.id
            WHERE SONGS_FTS MATCH ?1
            ORDER BY rank
            LIMIT 100
        ",
        )?;

        let rows = stmt.query_map([fts_query.as_str()], |row| {
            let mut song = SongData::default();
            song.id = row.get(0)?;
            song.full_file_path = crate::utils::interner::intern_string(&row.get::<_, String>(1)?);
            song.title = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(2)?.as_deref(),
            );
            song.artist = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(3)?.as_deref(),
            );
            song.album = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(4)?.as_deref(),
            );
            song.compressed_cached_cover_root = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(5)?.as_deref(),
            );
            song.duration_secs = row.get(6)?;
            song.format = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(7)?.as_deref(),
            );
            song.size = row.get(8)?;
            song.track_number = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(9)?.as_deref(),
            ); // TEXT -> Option<Arc<str>>
            song.bit_depth = row.get(10)?;
            song.sample_rate = row.get(11)?;
            song.channels = row.get(12)?;
            song.embedded_cover = row.get(13)?;
            song.cover_override = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(14)?.as_deref(),
            );
            song.import_order = row.get(15)?;
            song.genre = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(16)?.as_deref(),
            );
            song.release_year = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(17)?.as_deref(),
            );
            song.album_artist = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(18)?.as_deref(),
            );
            Ok(Arc::new(song))
        })?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(s) = r {
                results.push(s);
            }
        }
        Ok(results)
    }

    /// Obtiene todas las canciones de la biblioteca (no externas) ordenadas.
    pub fn get_all_songs(&self) -> Result<Vec<Arc<SongData>>> {
        let mut stmt = self.conn.prepare("
            SELECT s.id, s.file_path, s.title, ar.name, al.title, al.cover_path,
                   s.duration, s.format, s.size, s.track_num, s.bit_depth, s.sample_rate,
                   s.channels, s.embedded_cover, s.cover_override, s.import_order,
                   al.genre, al.year, al_ar.name
            FROM SONGS s
            JOIN ARTISTS ar ON s.artist_id = ar.id
            JOIN ALBUMS al ON s.album_id = al.id
            JOIN ARTISTS al_ar ON al.artist_id = al_ar.id
            WHERE s.is_external = 0
            ORDER BY ar.name COLLATE NOCASE ASC, al.year COLLATE NOCASE ASC, al.title COLLATE NOCASE ASC, s.track_num ASC
        ")?;

        let rows = stmt.query_map([], |row| {
            let mut song = SongData::default();
            song.id = row.get(0)?;
            song.full_file_path = crate::utils::interner::intern_string(&row.get::<_, String>(1)?);
            song.title = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(2)?.as_deref(),
            );
            song.artist = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(3)?.as_deref(),
            );
            song.album = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(4)?.as_deref(),
            );
            song.album_id = 0; // Se puede añadir al SELECT si es crítico, por ahora 0 es seguro
            song.compressed_cached_cover_root = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(5)?.as_deref(),
            );
            song.duration_secs = row.get(6)?;
            song.format = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(7)?.as_deref(),
            );
            song.size = row.get(8)?;
            song.track_number = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(9)?.as_deref(),
            );
            song.bit_depth = row.get(10)?;
            song.sample_rate = row.get(11)?;
            song.channels = row.get(12)?;
            song.embedded_cover = row.get(13)?;
            song.cover_override = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(14)?.as_deref(),
            );
            song.import_order = row.get(15)?;
            song.genre = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(16)?.as_deref(),
            );
            song.release_year = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(17)?.as_deref(),
            );
            song.album_artist = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(18)?.as_deref(),
            );
            Ok(Arc::new(song))
        })?;

        let mut songs = Vec::new();
        for r in rows {
            if let Ok(s) = r {
                songs.push(s);
            }
        }
        Ok(songs)
    }

    /// Obtiene todos los álbumes de la biblioteca con metadatos básicos.
    pub fn get_all_albums(
        &self,
    ) -> Result<Vec<(String, String, String, String, String, Option<String>)>> {
        let mut stmt = self.conn.prepare("
            SELECT DISTINCT al.hash_id, al.title, ar.name, al.genre, al.year, al.cover_path
            FROM ALBUMS al
            JOIN ARTISTS ar ON al.artist_id = ar.id
            JOIN SONGS s ON s.album_id = al.id
            WHERE s.is_external = 0
            ORDER BY ar.name COLLATE NOCASE ASC, al.year COLLATE NOCASE ASC, al.title COLLATE NOCASE ASC
        ")?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3).unwrap_or_default(),
                row.get(4).unwrap_or_default(),
                row.get(5)?,
            ))
        })?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(a) = r {
                results.push(a);
            }
        }
        Ok(results)
    }

    /// Obtiene álbumes para la vista Grid, agrupados correctamente para evitar duplicados en compilaciones
    /// Obtiene álbumes para la vista Grid, agrupados por artista sin duplicados.
    pub fn get_grid_items_by_artist(
        &self,
    ) -> Result<Vec<(String, String, String, String, String, Option<String>)>> {
        let mut stmt = self.conn.prepare("
            SELECT al.hash_id, al.title, ar.name, al.genre, al.year, al.cover_path
            FROM ALBUMS al
            JOIN ARTISTS ar ON al.artist_id = ar.id
            JOIN SONGS s ON s.album_id = al.id
            WHERE s.is_external = 0
            GROUP BY al.id
            ORDER BY ar.name COLLATE NOCASE ASC, al.year COLLATE NOCASE ASC, al.title COLLATE NOCASE ASC
        ")?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3).unwrap_or_default(),
                row.get(4).unwrap_or_default(),
                row.get(5)?,
            ))
        })?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(a) = r {
                results.push(a);
            }
        }
        Ok(results)
    }

    /// Obtiene todas las carpetas registradas en la biblioteca.
    pub fn get_all_folders(&self) -> Result<Vec<(i64, String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path, name FROM FOLDERS ORDER BY path COLLATE NOCASE ASC")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(f) = r {
                results.push(f);
            }
        }
        Ok(results)
    }

    /// Obtiene estadísticas de un álbum (conteo, duración total, tamaño total).
    pub fn get_album_stats_by_hash(&self, album_hash: &str) -> Result<(u64, f64, f64)> {
        let mut stmt = self.conn.prepare(
            "
            SELECT COUNT(*), SUM(duration), SUM(size)
            FROM SONGS s
            JOIN ALBUMS al ON s.album_id = al.id
            WHERE al.hash_id = ?1
        ",
        )?;

        let stats = stmt.query_row(params![album_hash], |row| {
            Ok((
                row.get::<_, i64>(0)? as u64,
                row.get::<_, f64>(1).unwrap_or(0.0),
                row.get::<_, f64>(2).unwrap_or(0.0) as f64,
            ))
        })?;
        Ok(stats)
    }

    /// Obtiene estadísticas de un álbum filtrado por artista.
    pub fn get_album_stats_by_hash_and_artist(
        &self,
        album_hash: &str,
        artist_name: &str,
    ) -> Result<(u64, f64, f64)> {
        let mut stmt = self.conn.prepare(
            "
            SELECT COUNT(*), SUM(s.duration), SUM(s.size)
            FROM SONGS s
            JOIN ALBUMS al ON s.album_id = al.id
            JOIN ARTISTS ar ON s.artist_id = ar.id
            WHERE al.hash_id = ?1 AND ar.name = ?2
        ",
        )?;

        let stats = stmt.query_row(params![album_hash, artist_name], |row| {
            Ok((
                row.get::<_, i64>(0)? as u64,
                row.get::<_, f64>(1).unwrap_or(0.0),
                row.get::<_, f64>(2).unwrap_or(0.0) as f64,
            ))
        })?;
        Ok(stats)
    }

    /// Obtiene las canciones de un álbum específico por su hash.
    pub fn get_songs_by_album(&self, album_hash_id: &str) -> Result<Vec<Arc<SongData>>> {
        let mut stmt = self.conn.prepare("
            SELECT s.id, s.file_path, s.title, ar.name, al.title, al.cover_path,
                   s.duration, s.format, s.size, s.track_num, s.bit_depth, s.sample_rate,
                   s.channels, s.embedded_cover, s.cover_override, s.import_order,
                   al.genre, al.year, al_ar.name
            FROM SONGS s
            JOIN ARTISTS ar ON s.artist_id = ar.id
            JOIN ALBUMS al ON s.album_id = al.id
            JOIN ARTISTS al_ar ON al.artist_id = al_ar.id
            WHERE al.hash_id = ?1
            ORDER BY ar.name COLLATE NOCASE ASC, al.year COLLATE NOCASE ASC, al.title COLLATE NOCASE ASC, CAST(s.track_num AS INTEGER) ASC, s.track_num ASC
        ")?;

        let rows = stmt.query_map([album_hash_id], |row| {
            let mut song = SongData::default();
            song.id = row.get(0)?;
            song.full_file_path = crate::utils::interner::intern_string(&row.get::<_, String>(1)?);
            song.title = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(2)?.as_deref(),
            );
            song.artist = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(3)?.as_deref(),
            );
            song.album = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(4)?.as_deref(),
            );
            song.compressed_cached_cover_root = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(5)?.as_deref(),
            );
            song.duration_secs = row.get(6)?;
            song.format = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(7)?.as_deref(),
            );
            song.size = row.get(8)?;
            song.track_number = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(9)?.as_deref(),
            );
            song.bit_depth = row.get(10)?;
            song.sample_rate = row.get(11)?;
            song.channels = row.get(12)?;
            song.embedded_cover = row.get(13)?;
            song.cover_override = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(14)?.as_deref(),
            );
            song.import_order = row.get(15)?;
            song.genre = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(16)?.as_deref(),
            );
            song.release_year = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(17)?.as_deref(),
            );
            song.album_artist = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(18)?.as_deref(),
            );
            Ok(Arc::new(song))
        })?;

        let mut songs = Vec::new();
        for r in rows {
            if let Ok(s) = r {
                songs.push(s);
            }
        }
        Ok(songs)
    }

    /// Obtiene las canciones de un álbum filtradas por nombre de artista.
    pub fn get_songs_by_album_and_artist(
        &self,
        album_hash_id: &str,
        artist_name: &str,
    ) -> Result<Vec<Arc<SongData>>> {
        let mut stmt = self.conn.prepare("
            SELECT s.id, s.file_path, s.title, ar.name, al.title, al.cover_path,
                   s.duration, s.format, s.size, s.track_num, s.bit_depth, s.sample_rate,
                   s.channels, s.embedded_cover, s.cover_override, s.import_order,
                   al.genre, al.year, al_ar.name
            FROM SONGS s
            JOIN ARTISTS ar ON s.artist_id = ar.id
            JOIN ALBUMS al ON s.album_id = al.id
            JOIN ARTISTS al_ar ON al.artist_id = al_ar.id
            WHERE al.hash_id = ?1 AND ar.name = ?2
            ORDER BY ar.name ASC, al.year ASC, al.title ASC, CAST(s.track_num AS INTEGER) ASC, s.track_num ASC
        ")?;

        let rows = stmt.query_map([album_hash_id, artist_name], |row| {
            let mut song = SongData::default();
            song.id = row.get(0)?;
            song.full_file_path = crate::utils::interner::intern_string(&row.get::<_, String>(1)?);
            song.title = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(2)?.as_deref(),
            );
            song.artist = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(3)?.as_deref(),
            );
            song.album = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(4)?.as_deref(),
            );
            song.compressed_cached_cover_root = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(5)?.as_deref(),
            );
            song.duration_secs = row.get(6)?;
            song.format = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(7)?.as_deref(),
            );
            song.size = row.get(8)?;
            song.track_number = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(9)?.as_deref(),
            );
            song.bit_depth = row.get(10)?;
            song.sample_rate = row.get(11)?;
            song.channels = row.get(12)?;
            song.embedded_cover = row.get(13)?;
            song.cover_override = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(14)?.as_deref(),
            );
            song.import_order = row.get(15)?;
            song.genre = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(16)?.as_deref(),
            );
            song.release_year = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(17)?.as_deref(),
            );
            song.album_artist = crate::utils::interner::intern_string_opt(
                row.get::<_, Option<String>>(18)?.as_deref(),
            );
            Ok(Arc::new(song))
        })?;

        let mut songs = Vec::new();
        for r in rows {
            if let Ok(s) = r {
                songs.push(s);
            }
        }
        Ok(songs)
    }

    /// Obtiene agrupaciones de artistas con conteos de canciones, álbumes y duración.
    pub fn get_artist_groups_sql(
        &self,
        search_query: Option<&str>,
    ) -> Result<Vec<(String, usize, usize, f64)>> {
        let mut sql = String::from(
            "
            SELECT ar.name, COUNT(s.id), COUNT(DISTINCT al.id), SUM(s.duration)
            FROM SONGS s
            JOIN ARTISTS ar ON s.artist_id = ar.id
            JOIN ALBUMS al ON s.album_id = al.id
            WHERE s.is_external = 0
        ",
        );

        let mut params_vec = Vec::new();
        if let Some(fts_query) = search_query.and_then(fts5_match_query) {
            sql.push_str(" AND s.id IN (SELECT rowid FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1)");
            params_vec.push(fts_query);
        }

        sql.push_str(" GROUP BY ar.name ORDER BY ar.name ASC");

        let rows = if params_vec.is_empty() {
            let mut stmt = self.conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get::<_, i64>(1)? as usize,
                    row.get::<_, i64>(2)? as usize,
                    row.get::<_, f64>(3).unwrap_or(0.0),
                ))
            })?;
            let mut res = Vec::new();
            for r in mapped {
                if let Ok(v) = r {
                    res.push(v);
                }
            }
            res
        } else {
            let mut stmt = self.conn.prepare(&sql)?;
            let mapped = stmt.query_map([&params_vec[0]], |row| {
                Ok((
                    row.get(0)?,
                    row.get::<_, i64>(1)? as usize,
                    row.get::<_, i64>(2)? as usize,
                    row.get::<_, f64>(3).unwrap_or(0.0),
                ))
            })?;
            let mut res = Vec::new();
            for r in mapped {
                if let Ok(v) = r {
                    res.push(v);
                }
            }
            res
        };

        Ok(rows)
    }

    /// Obtiene el valor máximo de `import_order` entre todas las canciones.
    pub fn get_max_import_order(&self) -> Result<i64> {
        let mut stmt = self.conn.prepare("SELECT MAX(import_order) FROM SONGS")?;
        let res = stmt.query_row([], |row| {
            let val: Option<i64> = row.get(0)?;
            Ok(val.unwrap_or(0))
        });
        res
    }

    /// Obtiene estadísticas generales de la biblioteca (canciones, álbumes, artistas, duración, tamaño).
    pub fn get_library_stats(&self) -> Result<(usize, usize, f64, f64, usize)> {
        let songs: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM SONGS WHERE is_external = 0",
            [],
            |r| r.get(0),
        )?;
        let albums: i64 = self.conn.query_row("SELECT COUNT(*) FROM ALBUMS WHERE id IN (SELECT album_id FROM SONGS WHERE is_external = 0)", [], |r| r.get(0))?;
        let artists: i64 = self.conn.query_row("SELECT COUNT(*) FROM ARTISTS WHERE id IN (SELECT artist_id FROM SONGS WHERE is_external = 0)", [], |r| r.get(0))?;
        let duration: f64 = self.conn.query_row(
            "SELECT SUM(duration) FROM SONGS WHERE is_external = 0",
            [],
            |r| Ok(r.get::<_, Option<f64>>(0)?.unwrap_or(0.0)),
        )?;
        let size: f64 = self.conn.query_row(
            "SELECT SUM(size) FROM SONGS WHERE is_external = 0",
            [],
            |r| Ok(r.get::<_, Option<i64>>(0)?.unwrap_or(0) as f64),
        )?;

        Ok((
            songs as usize,
            albums as usize,
            duration,
            size,
            artists as usize,
        ))
    }

    // ============================================================
    // FILTROS DE BIBLIOTECA - Consultas jerárquicas SQL
    // ============================================================

    /// Obtiene los datos necesarios para construir el árbol de filtros de forma eficiente.
    /// Retorna una lista de tuplas (L1, L2, L3) según el tipo de filtro solicitado.
    /// Obtiene la jerarquía de filtros (L1, L2, L3) para el árbol de navegación.
    pub fn get_filter_hierarchy(
        &self,
        filter_type: &str,
    ) -> Result<
        Vec<(
            std::sync::Arc<str>,
            std::sync::Arc<str>,
            Option<std::sync::Arc<str>>,
        )>,
    > {
        let query = match filter_type {
            "Genre" => {
                "
                SELECT DISTINCT
                    COALESCE(NULLIF(al.genre, ''), 'Desconocido') as l1,
                    COALESCE(NULLIF(ar.name, ''), 'Artista Desconocido') as l2,
                    COALESCE(NULLIF(al.title, ''), 'Álbum Desconocido') as l3
                FROM ALBUMS al
                JOIN ARTISTS ar ON al.artist_id = ar.id
                ORDER BY l1, l2, l3"
            }
            "Artist" => {
                "
                SELECT DISTINCT
                    COALESCE(NULLIF(ar.name, ''), 'Artista Desconocido') as l1,
                    COALESCE(NULLIF(al.title, ''), 'Álbum Desconocido') as l2,
                    NULL as l3
                FROM ALBUMS al
                JOIN ARTISTS ar ON al.artist_id = ar.id
                ORDER BY l1, l2"
            }
            "Album" => {
                "
                SELECT DISTINCT
                    COALESCE(NULLIF(al.title, ''), 'Álbum Desconocido') as l1,
                    COALESCE(NULLIF(ar.name, ''), 'Artista Desconocido') as l2,
                    NULL as l3
                FROM ALBUMS al
                JOIN ARTISTS ar ON al.artist_id = ar.id
                ORDER BY l1, l2"
            }
            "Year" => {
                "
                SELECT DISTINCT
                    COALESCE(NULLIF(al.year, ''), 'Desconocido') as l1,
                    COALESCE(NULLIF(ar.name, ''), 'Artista Desconocido') as l2,
                    COALESCE(NULLIF(al.title, ''), 'Álbum Desconocido') as l3
                FROM ALBUMS al
                JOIN ARTISTS ar ON al.artist_id = ar.id
                ORDER BY l1, l2, l3"
            }
            "Folder" => {
                "
                SELECT DISTINCT
                    COALESCE(NULLIF(f.name, ''), 'Raiz') as l1,
                    COALESCE(NULLIF(ar.name, ''), 'Artista Desconocido') as l2,
                    COALESCE(NULLIF(al.title, ''), 'Álbum Desconocido') as l3
                FROM SONGS s
                JOIN FOLDERS f ON s.folder_id = f.id
                JOIN ALBUMS al ON s.album_id = al.id
                JOIN ARTISTS ar ON s.artist_id = ar.id
                ORDER BY l1, l2, l3"
            }
            _ => return Ok(Vec::new()),
        };

        let mut stmt = self.conn.prepare(query)?;
        let rows = stmt.query_map([], |r| {
            let l1_str: String = r.get(0)?;
            let l2_str: String = r.get(1)?;
            let l3_str: Option<String> = r.get(2)?;

            Ok((
                crate::utils::interner::intern_string(&l1_str),
                crate::utils::interner::intern_string(&l2_str),
                crate::utils::interner::intern_string_opt(l3_str.as_deref()),
            ))
        })?;

        let mut results = Vec::new();
        for row in rows {
            if let Ok(entry) = row {
                results.push(entry);
            }
        }
        Ok(results)
    }

    /// Realiza una búsqueda y filtrado unificado en la base de datos usando FTS5 y filtros relacionales.
    /// Realiza una búsqueda y filtrado unificado con FTS5 y filtros relacionales.
    pub fn get_library_songs_filtered(
        &self,
        params: &LibrarySearchParams,
    ) -> Result<Vec<Arc<SongData>>> {
        let mut sql = "
            SELECT s.id, s.folder_id, s.artist_id, s.album_id, s.file_path, s.title,
                   ar.name as artist_name, al.title as album_title, al.year, al.genre,
                   s.track_num, s.duration, s.format, s.bit_depth, s.sample_rate,
                   s.size, s.channels, s.embedded_cover, s.cover_override, s.import_order,
                   al.cover_path, aar.name as album_artist_name
            FROM SONGS s
            JOIN ARTISTS ar ON s.artist_id = ar.id
            JOIN ALBUMS al ON s.album_id = al.id
            LEFT JOIN ARTISTS aar ON al.artist_id = aar.id
        "
        .to_string();

        let mut conditions = Vec::new();
        let mut sql_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // 1. Integración de Búsqueda FTS5 (si hay query)
        if let Some(fts_query) = params.query.as_deref().and_then(fts5_match_query) {
            sql.push_str(" JOIN SONGS_FTS fts ON fts.rowid = s.id ");
            conditions.push("SONGS_FTS MATCH ?".to_string());
            sql_params.push(Box::new(fts_query));
        }

        // 2. Filtros Relacionales
        if let Some(art) = &params.artist {
            conditions.push("ar.name = ?".to_string());
            sql_params.push(Box::new(art.clone()));
        }
        if let Some(alb) = &params.album {
            conditions.push("al.title = ?".to_string());
            sql_params.push(Box::new(alb.clone()));
        }
        if let Some(gnr) = &params.genre {
            conditions.push("al.genre = ?".to_string());
            sql_params.push(Box::new(gnr.clone()));
        }
        if let Some(yr) = &params.year {
            conditions.push("al.year = ?".to_string());
            sql_params.push(Box::new(yr.clone()));
        }
        if let Some(fid) = params.folder_id {
            conditions.push("s.folder_id = ?".to_string());
            sql_params.push(Box::new(fid));
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        // Orden por defecto: Artista Álbum -> Año -> Álbum -> Track (Natural)
        sql.push_str(" ORDER BY COALESCE(aar.name, ar.name) COLLATE NOCASE, al.year, al.title, CAST(s.track_num AS INTEGER), s.track_num");

        let mut stmt = self.conn.prepare(&sql)?;

        // Convertir Box<dyn ToSql> a &dyn ToSql para rusqlite
        let query_params: Vec<&dyn rusqlite::ToSql> =
            sql_params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(query_params.as_slice(), |r| {
            Ok(Arc::new(SongData {
                id: r.get(0)?,
                folder_id: r.get(1)?,
                artist_id: r.get(2)?,
                album_id: r.get(3)?,
                full_file_path: crate::utils::interner::intern_string(&r.get::<_, String>(4)?),
                title: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(5)?.as_deref(),
                ),
                artist: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(6)?.as_deref(),
                ),
                album: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(7)?.as_deref(),
                ),
                release_year: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(8)?.as_deref(),
                ),
                genre: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(9)?.as_deref(),
                ),
                track_number: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(10)?.as_deref(),
                ),
                duration_secs: r.get(11)?,
                format: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(12)?.as_deref(),
                ),
                bit_depth: r.get(13)?,
                sample_rate: r.get(14)?,
                size: r.get(15)?,
                channels: r.get(16)?,
                embedded_cover: r.get(17)?,
                cover_override: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(18)?.as_deref(),
                ),
                import_order: r.get(19)?,
                compressed_cached_cover_root: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(20)?.as_deref(),
                ),
                album_artist: crate::utils::interner::intern_string_opt(
                    r.get::<_, Option<String>>(21)?.as_deref(),
                ),
            }))
        })?;

        let mut songs = Vec::new();
        for row in rows {
            if let Ok(s) = row {
                songs.push(s);
            }
        }
        Ok(songs)
    }
}
