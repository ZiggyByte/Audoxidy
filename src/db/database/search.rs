//! Full-text search translation and playlist-local search.

use rusqlite::{Result, params};

use super::Database;
use super::models::PlaylistSongRef;

impl Database {
    /// Busca canciones dentro de una playlist por query (búsqueda local).
    /// Busca canciones dentro de una playlist por texto (título, artista, álbum).
    pub fn search_playlist_songs(
        &self,
        playlist_id: i64,
        query: &str,
    ) -> Result<Vec<PlaylistSongRef>> {
        let search_pattern = format!("%{}%", query);

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
              AND (
                  LOWER(s.title) LIKE LOWER(?2)
                  OR LOWER(ar.name) LIKE LOWER(?2)
                  OR LOWER(al.title) LIKE LOWER(?2)
              )
            ORDER BY pi.sequence_order ASC
        ",
        )?;

        let rows = stmt.query_map(params![playlist_id, search_pattern], |r| {
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
}

/// Convierte texto libre en una expresión `MATCH` segura para FTS5.
///
/// Cada término separado por espacios se encierra entre comillas dobles y las
/// comillas internas se duplican, de modo que los operadores de FTS5 (`*`, `OR`,
/// `NEAR`, `{`, `}`, `^`, `:`...) se tratan como texto literal. El comodín de
/// prefijo `*` se coloca fuera de las comillas del último término para conservar
/// la búsqueda por prefijo. Devuelve `None` cuando la entrada no tiene términos.
pub(in crate::db::database) fn fts5_match_query(input: &str) -> Option<String> {
    let terms: Vec<String> = input
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(format!("{} *", terms.join(" ")))
    }
}
