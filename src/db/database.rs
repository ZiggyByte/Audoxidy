use hex;
use rusqlite::{Connection, OptionalExtension, Result, params};
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::audio::preset::EqPreset;

/// Base de datos SQLite de la biblioteca musical de Audoxidy.
///
/// Gestiona el esquema relacional completo: carpetas, artistas, álbumes,
/// canciones, metadatos extendidos, playlists y sesiones de shuffle.
/// Usa WAL, FTS5 para búsqueda de texto completo y pragmas de rendimiento.
pub struct Database {
    conn: Connection,
}

/// Datos esenciales de la canción para visualización en listas, filtros y búsqueda.
/// Optimizado para bajo consumo de RAM.
#[derive(Debug, Default, Clone)]
pub struct SongData {
    pub id: i64,
    pub folder_id: i64,
    pub artist_id: i64,
    pub album_id: i64,
    pub full_file_path: std::sync::Arc<str>,
    pub title: Option<std::sync::Arc<str>>,
    pub artist: Option<std::sync::Arc<str>>,
    pub album: Option<std::sync::Arc<str>>,
    pub album_artist: Option<std::sync::Arc<str>>,
    pub genre: Option<std::sync::Arc<str>>,
    pub release_year: Option<std::sync::Arc<str>>,
    pub track_number: Option<std::sync::Arc<str>>,
    pub duration_secs: Option<f64>,
    pub format: Option<std::sync::Arc<str>>,
    pub bit_depth: Option<i64>,
    pub sample_rate: Option<i64>,
    pub size: Option<i64>,
    pub channels: Option<i64>,
    pub embedded_cover: bool,
    pub compressed_cached_cover_root: Option<std::sync::Arc<str>>,
    pub cover_override: Option<std::sync::Arc<str>>, // Hash de carátula específica si difiere del álbum
    pub import_order: i64,
}

/// Metadatos extendidos para edición y visualización de propiedades detalladas.
#[derive(Debug, Default, Clone)]
pub struct SongMetadataExtended {
    pub song_id: i64,
    pub lyrics: Option<String>,
    pub comments: Option<String>,
    pub composer: Option<String>,
    pub lyricist: Option<String>,
    pub publisher: Option<String>,
    pub url: Option<String>,
    pub copyright: Option<String>,
    pub encoded_by: Option<String>,
    pub catalog: Option<String>,
    pub isrc: Option<String>,
    pub key: Option<String>,
    pub bpm: Option<String>,
    pub track_gain: Option<f64>,
    pub album_gain: Option<f64>,
}

/// Datos esenciales de una lista de reproducción.
#[derive(Debug, Clone)]
pub struct PlaylistData {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
    pub is_system: bool,
    pub created_at: i64,
    pub last_song_id: Option<i64>,
    pub last_pos_sec: f64,
    pub is_playing: bool,
    pub shuffle_active: bool,
    pub repeat_mode: i32,
    pub shuffle_pos: usize,
    pub shuffle_id: Option<String>,
}

/// Item de una lista de reproducción (referencia + estado, sin duplicar metadata).
#[derive(Debug, Clone)]
pub struct PlaylistSongRef {
    pub item_id: i64,
    pub song_id: i64,
    pub sequence_order: f64,
    pub enabled: bool,

    // Metadata JOIN de SONGS + ALBUMS + ARTISTS
    pub title: std::sync::Arc<str>,
    pub artist_name: std::sync::Arc<str>,
    pub album_title: std::sync::Arc<str>,
    pub album_artist_name: std::sync::Arc<str>,
    pub year: Option<std::sync::Arc<str>>,
    pub genre: Option<std::sync::Arc<str>>,
    pub track_number: Option<std::sync::Arc<str>>,
    pub duration: f64,
    pub file_path: std::sync::Arc<str>,
    pub folder_path: std::sync::Arc<str>,
    pub folder_name: std::sync::Arc<str>,
    pub cover_path: Option<std::sync::Arc<str>>,
    pub cover_override: Option<std::sync::Arc<str>>,
}

/// Grupo de canciones de playlist agrupadas por carpeta/álbum.
#[derive(Debug, Clone)]
pub struct PlaylistFolderGroup {
    pub folder_path: std::sync::Arc<str>,
    pub folder_name: std::sync::Arc<str>,
    pub songs: Vec<PlaylistSongRef>,
    pub total_duration: f64,
    pub first_item_id: i64,
}

/// Sesión de shuffle: orden aleatorio + historial para navegación backward.
#[derive(Debug, Clone)]
pub struct ShuffleSession {
    pub session_id: String,
    pub shuffle_order: Vec<i64>, // song_ids en orden aleatorio
    pub current_position: usize,
    pub history: Vec<i64>, // song_ids ya reproducidos en orden
}

impl Default for Database {
    fn default() -> Self {
        Self::new().expect("No se pudo iniciar la DB local SQLite")
    }
}

impl Database {
    /// Abre o crea la base de datos `library.db` e inicializa el esquema.
    pub fn new() -> Result<Self> {
        let db_path = "library.db";
        let conn = Connection::open(db_path)?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        Self::create_schema(&conn)?;
        Ok(Self { conn })
    }

    /// Crea una base de datos en memoria para pruebas.
    #[cfg(test)]
    pub fn new_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        Self::create_schema(&conn)?;
        Ok(Self { conn })
    }

    fn create_schema(conn: &Connection) -> Result<()> {
        // Configuraciones de rendimiento
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "cache_size", -8000)?;
        conn.pragma_update(None, "mmap_size", 16777216)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        // 1. CARPETAS (Identidad física)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS FOLDERS (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL
            )",
            [],
        )?;

        // 2. ARTISTAS (Identidad global por nombre)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS ARTISTS (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                hash_id TEXT UNIQUE NOT NULL
            )",
            [],
        )?;

        // 3. ALBUMES
        conn.execute(
            "CREATE TABLE IF NOT EXISTS ALBUMS (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                artist_id INTEGER NOT NULL,
                folder_id INTEGER NOT NULL,
                title TEXT NOT NULL,
                year TEXT,
                genre TEXT,
                cover_path TEXT,
                cover_hash TEXT,
                hash_id TEXT UNIQUE NOT NULL,
                FOREIGN KEY(artist_id) REFERENCES ARTISTS(id) ON DELETE CASCADE,
                FOREIGN KEY(folder_id) REFERENCES FOLDERS(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // 4. CANCIONES (Esenciales)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS SONGS (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                album_id INTEGER NOT NULL,
                artist_id INTEGER NOT NULL,
                folder_id INTEGER NOT NULL,
                file_path TEXT UNIQUE NOT NULL,
                title TEXT,
                track_num TEXT,
                duration REAL,
                format TEXT,
                bit_depth INTEGER,
                sample_rate INTEGER,
                channels INTEGER,
                size INTEGER,
                embedded_cover BOOLEAN DEFAULT 0,
                cover_override TEXT,
                import_order INTEGER DEFAULT 0,
                is_external INTEGER DEFAULT 0,
                FOREIGN KEY(album_id) REFERENCES ALBUMS(id) ON DELETE CASCADE,
                FOREIGN KEY(artist_id) REFERENCES ARTISTS(id) ON DELETE CASCADE,
                FOREIGN KEY(folder_id) REFERENCES FOLDERS(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // 5. METADATOS EXTENDIDOS (Relación 1:1 con SONGS)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS SONG_METADATA (
                song_id INTEGER PRIMARY KEY,
                lyrics TEXT,
                comments TEXT,
                composer TEXT,
                lyricist TEXT,
                publisher TEXT,
                url TEXT,
                copyright TEXT,
                encoded_by TEXT,
                catalog TEXT,
                isrc TEXT,
                key TEXT,
                bpm TEXT,
                track_gain REAL,
                album_gain REAL,
                FOREIGN KEY(song_id) REFERENCES SONGS(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // 6. METADATOS TÉCNICOS CRUDOS POR FORMATO (ID3, Vorbis, APE, etc.)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS SONG_TAG_ITEMS (
                song_id INTEGER NOT NULL,
                tag_type TEXT NOT NULL,
                item_key TEXT NOT NULL,
                raw_value TEXT NOT NULL,
                PRIMARY KEY (song_id, tag_type, item_key),
                FOREIGN KEY(song_id) REFERENCES SONGS(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // 7. LISTAS DE REPRODUCCIÓN
        conn.execute(
            "CREATE TABLE IF NOT EXISTS PLAYLISTS (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                sort_order INTEGER NOT NULL DEFAULT 0,
                is_system INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                last_song_id INTEGER,
                last_pos_sec REAL NOT NULL DEFAULT 0.0,
                is_playing INTEGER NOT NULL DEFAULT 0,
                shuffle_active INTEGER NOT NULL DEFAULT 0,
                repeat_mode INTEGER NOT NULL DEFAULT 0,
                shuffle_pos INTEGER NOT NULL DEFAULT 0,
                shuffle_id TEXT
            )",
            [],
        )?;

        // 8. ITEMS DE LISTA DE REPRODUCCIÓN
        conn.execute(
            "CREATE TABLE IF NOT EXISTS PLAYLIST_ITEMS (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                playlist_id INTEGER NOT NULL,
                song_id INTEGER NOT NULL,
                sequence_order REAL NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                added_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                FOREIGN KEY(playlist_id) REFERENCES PLAYLISTS(id) ON DELETE CASCADE,
                FOREIGN KEY(song_id) REFERENCES SONGS(id) ON DELETE CASCADE,
                UNIQUE(playlist_id, song_id)
            )",
            [],
        )?;

        // 9. HISTORIAL DE SHUFFLE POR SESIÓN
        conn.execute(
            "CREATE TABLE IF NOT EXISTS PLAYLIST_SHUFFLE_HISTORY (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                playlist_id INTEGER NOT NULL,
                session_id TEXT NOT NULL,
                song_id INTEGER NOT NULL,
                play_order INTEGER NOT NULL,
                played_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                FOREIGN KEY(playlist_id) REFERENCES PLAYLISTS(id) ON DELETE CASCADE,
                FOREIGN KEY(song_id) REFERENCES SONGS(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // 11. AJUSTES APP (Configuración global)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS APP_SETTINGS (
                key TEXT PRIMARY KEY,
                value TEXT
            )",
            [],
        )?;

        // Índices para playlists
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_playlist_items_order ON PLAYLIST_ITEMS(playlist_id, sequence_order)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_playlist_items_song ON PLAYLIST_ITEMS(song_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_shuffle_session ON PLAYLIST_SHUFFLE_HISTORY(playlist_id, session_id, play_order)",
            [],
        )?;

        // 10. FTS5 - Búsqueda de texto completo
        // Nota: Solo indexamos lo que el usuario busca activamente.
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS SONGS_FTS USING fts5(
                title,
                artist_name,
                album_title,
                content='SONGS',
                content_rowid='id'
            )",
            [],
        )?;

        // Triggers para mantener FTS5 sincronizado automáticamente
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS songs_ai AFTER INSERT ON SONGS BEGIN
            INSERT INTO SONGS_FTS(rowid, title, artist_name, album_title)
            VALUES (new.id, new.title,
                   (SELECT name FROM ARTISTS WHERE id = new.artist_id),
                   (SELECT title FROM ALBUMS WHERE id = new.album_id));
        END;",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS songs_ad AFTER DELETE ON SONGS BEGIN
            INSERT INTO SONGS_FTS(SONGS_FTS, rowid, title, artist_name, album_title)
            VALUES('delete', old.id, old.title,
                   (SELECT name FROM ARTISTS WHERE id = old.artist_id),
                   (SELECT title FROM ALBUMS WHERE id = old.album_id));
        END;",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS songs_au AFTER UPDATE ON SONGS BEGIN
            INSERT INTO SONGS_FTS(SONGS_FTS, rowid, title, artist_name, album_title)
            VALUES('delete', old.id, old.title,
                   (SELECT name FROM ARTISTS WHERE id = old.artist_id),
                   (SELECT title FROM ALBUMS WHERE id = old.album_id));
            INSERT INTO SONGS_FTS(rowid, title, artist_name, album_title)
            VALUES (new.id, new.title,
                   (SELECT name FROM ARTISTS WHERE id = new.artist_id),
                   (SELECT title FROM ALBUMS WHERE id = new.album_id));
        END;",
            [],
        )?;

        // Índices adicionales para filtros rápidos (agrupación)
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_songs_folder ON SONGS(folder_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_songs_album ON SONGS(album_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_songs_artist ON SONGS(artist_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_albums_year ON ALBUMS(year)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_albums_genre ON ALBUMS(genre)",
            [],
        )?;

        // Inicializar playlists del sistema si no existen
        Self::init_system_playlists(conn)?;

        // Tabla de presets de ecualizador personalizados (Decisión D-01)
        Self::init_eq_presets_table(conn)?;

        Ok(())
    }

    /// Crea las playlists del sistema si no existen ya.
    fn init_system_playlists(conn: &Connection) -> Result<()> {
        // "Audoxidy" - playlist principal para archivos dentro la biblioteca.
        conn.execute(
            "INSERT OR IGNORE INTO PLAYLISTS (name, sort_order, is_system) VALUES (?1, ?2, ?3)",
            params!["Audoxidy", 0, 1],
        )?;

        // "Oxidy Drift" - playlist para archivos fuera de la biblioteca.
        conn.execute(
            "INSERT OR IGNORE INTO PLAYLISTS (name, sort_order, is_system) VALUES (?1, ?2, ?3)",
            params!["Oxidy Drift", 1, 1],
        )?;

        Ok(())
    }

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

    /// Inserta una canción con todos sus metadatos, carátula y tags RAW.
    ///
    /// Retorna (hash_de_carátula, necesita_procesamiento).
    pub fn insert_song_full(
        &mut self,
        song: &SongData,
        extended: &SongMetadataExtended,
        pic_bytes_hash: Option<String>,
        raw_tags: Vec<(String, String, String)>, // (tag_type, item_key, raw_value)
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
        let mut target_hash_for_processing = album_hash_id.clone();

        if let Some(current_pic_hash) = pic_bytes_hash {
            // Verificar si el álbum ya tiene carátula
            let album_has_cover: bool = self
                .conn
                .prepare_cached("SELECT cover_hash IS NOT NULL FROM ALBUMS WHERE hash_id = ?1")?
                .query_row([&album_hash_id], |row| row.get(0))?;

            if !album_has_cover {
                // Es la primera carátula del álbum, la asignamos como principal
                let path = format!("cache/covers/{}.avif", album_hash_id);

                self.conn
                    .prepare_cached(
                        "UPDATE ALBUMS SET cover_hash = ?1, cover_path = ?2 WHERE hash_id = ?3",
                    )?
                    .execute(params![current_pic_hash, path, album_hash_id])?;
                needs_processing = true;
            } else {
                let album_cover_hash: Option<String> = self
                    .conn
                    .prepare_cached("SELECT cover_hash FROM ALBUMS WHERE hash_id = ?1")?
                    .query_row([&album_hash_id], |row| row.get(0))?;

                if album_cover_hash.as_ref() != Some(&current_pic_hash) {
                    // Es una carátula diferente a la del álbum (Override)
                    let song_cover_hash = Self::generate_hash(&format!(
                        "{}{}",
                        song.full_file_path, current_pic_hash
                    ));
                    final_song_cover_override = Some(song_cover_hash.clone());
                    target_hash_for_processing = song_cover_hash;
                    needs_processing = true;
                }
            }
        }

        // 2. Inserción de la canción (Con su propio artista de pista)
        self.conn.prepare_cached(
            "INSERT INTO SONGS (
                album_id, artist_id, folder_id, file_path, title, track_num, duration, format,
                bit_depth, sample_rate, channels, size, embedded_cover, cover_override, import_order, is_external
            ) VALUES (
                (SELECT id FROM ALBUMS WHERE hash_id = ?1), ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16
            ) ON CONFLICT(file_path) DO UPDATE SET
                title = excluded.title, track_num = excluded.track_num, duration = excluded.duration,
                artist_id = excluded.artist_id, cover_override = excluded.cover_override, is_external = excluded.is_external"
        )?.execute(
            params![
                album_hash_id, track_artist_id, folder_id, song.full_file_path, song.title,
                song.track_number,
                song.duration_secs, song.format, song.bit_depth, song.sample_rate,
                song.channels, song.size, song.embedded_cover, final_song_cover_override, song.import_order,
                if is_external { 1 } else { 0 }
            ],
        )?;

        let song_id: i64 = self
            .conn
            .prepare_cached("SELECT id FROM SONGS WHERE file_path = ?1")?
            .query_row([&song.full_file_path], |row| row.get(0))?;

        // 3. Guardar metadatos técnicos crudos de todos los formatos presentes
        self.conn
            .prepare_cached("DELETE FROM SONG_TAG_ITEMS WHERE song_id = ?1")?
            .execute([song_id])?;
        for (tag_type, key, value) in raw_tags {
            self.conn
                .prepare_cached(
                    "INSERT OR REPLACE INTO SONG_TAG_ITEMS (song_id, tag_type, item_key, raw_value)
                 VALUES (?1, ?2, ?3, ?4)",
                )?
                .execute(params![song_id, tag_type, key, value])?;
        }

        // 4. Metadatos extendidos (Vista unificada)
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

        Ok((target_hash_for_processing, needs_processing))
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

    // --- Consultas de Alto Rendimiento ---

    /// Busca canciones usando FTS5 con el término de búsqueda dado.
    pub fn search_songs(&self, query: &str) -> Result<Vec<Arc<SongData>>> {
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

        let rows = stmt.query_map([format!("{}*", query)], |row| {
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
        if let Some(q) = search_query {
            if !q.is_empty() {
                sql.push_str(" AND s.id IN (SELECT rowid FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1)");
                params_vec.push(format!("{}*", q));
            }
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

    /// Verifica si un hash de carátula está siendo usado por algún álbum o canción.
    pub fn is_cover_hash_in_use(&self, hash: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT (SELECT COUNT(*) FROM ALBUMS WHERE hash_id = ? OR cover_hash = ?) + (SELECT COUNT(*) FROM SONGS WHERE cover_override = ?)",
            [hash, hash, hash],
            |row: &rusqlite::Row| row.get::<_, i64>(0),
        )?;
        Ok(count > 0)
    }

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

    // ============================================================
    // SHUFFLE HISTORY - Persistencia de sesiones de reproducción aleatoria
    // ============================================================

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
}

/// Parámetros para la búsqueda y filtrado de la biblioteca.
#[derive(Debug, Default, Clone)]
pub struct LibrarySearchParams {
    pub query: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<String>,
    pub folder_id: Option<i64>,
}

impl Database {
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
        if let Some(q) = &params.query {
            if !q.trim().is_empty() {
                sql.push_str(" JOIN SONGS_FTS fts ON fts.rowid = s.id ");
                // Limpiar query para FTS5 y añadir asterisco para búsqueda parcial
                let clean_q = q.replace("\"", "").replace("'", "");
                let fts_query = format!("{}*", clean_q.trim());
                conditions.push("SONGS_FTS MATCH ?".to_string());
                sql_params.push(Box::new(fts_query));
            }
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

    // ──────────────────────────────────────────────
    // EQ Preset CRUD (Decisión D-01: SQLite persistence)
    // ──────────────────────────────────────────────

    /// Crea la tabla `eq_presets` si no existe.
    fn init_eq_presets_table(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS eq_presets (
                name TEXT PRIMARY KEY,
                preamp_gain REAL NOT NULL DEFAULT 0.0,
                bands_20 BLOB,
                bands_31 BLOB
            );",
        )?;
        Ok(())
    }

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

#[cfg(test)]
mod eq_preset_tests {
    use super::*;
    use crate::audio::preset::EqPreset;

    #[test]
    fn test_save_and_load_eq_presets() {
        let db = Database::new_memory().unwrap();
        let preset = EqPreset::new("Test Preset", 2.5, Some(vec![1.0; 20]), Some(vec![2.0; 31]));
        db.save_eq_preset(&preset).unwrap();

        let loaded = db.load_eq_presets().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Test Preset");
        assert_eq!(loaded[0].preamp_gain, 2.5);
        assert_eq!(loaded[0].bands_20.as_ref().unwrap().len(), 20);
        assert_eq!(loaded[0].bands_31.as_ref().unwrap().len(), 31);
        assert_eq!(loaded[0].bands_20.as_ref().unwrap()[0], 1.0);
        assert_eq!(loaded[0].bands_31.as_ref().unwrap()[0], 2.0);
    }

    #[test]
    fn test_save_and_load_eq_preset_with_none_bands() {
        let db = Database::new_memory().unwrap();
        let preset = EqPreset::new("No Bands", 0.0, None, None);
        db.save_eq_preset(&preset).unwrap();

        let loaded = db.load_eq_presets().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "No Bands");
        assert!(loaded[0].bands_20.is_none());
        assert!(loaded[0].bands_31.is_none());
    }

    #[test]
    fn test_delete_eq_preset() {
        let db = Database::new_memory().unwrap();
        db.save_eq_preset(&EqPreset::new("To Delete", 0.0, None, None))
            .unwrap();
        db.save_eq_preset(&EqPreset::new("Keep", 0.0, None, None))
            .unwrap();
        assert_eq!(db.load_eq_presets().unwrap().len(), 2);

        db.delete_eq_preset("To Delete").unwrap();
        let loaded = db.load_eq_presets().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Keep");
    }

    #[test]
    fn test_clear_eq_presets() {
        let db = Database::new_memory().unwrap();
        db.save_eq_preset(&EqPreset::new("A", 0.0, None, None))
            .unwrap();
        db.save_eq_preset(&EqPreset::new("B", 0.0, None, None))
            .unwrap();
        assert_eq!(db.load_eq_presets().unwrap().len(), 2);

        db.clear_eq_presets().unwrap();
        assert!(db.load_eq_presets().unwrap().is_empty());
    }

    #[test]
    fn test_insert_or_replace_eq_preset() {
        let db = Database::new_memory().unwrap();
        db.save_eq_preset(&EqPreset::new("Same", 1.0, None, None))
            .unwrap();
        db.save_eq_preset(&EqPreset::new("Same", 5.0, None, None))
            .unwrap();

        let loaded = db.load_eq_presets().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].preamp_gain, 5.0);
    }
}
