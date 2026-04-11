use rusqlite::{params, Connection, Result};
use std::sync::Arc;
use sha2::{Sha256, Digest};
use hex;

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
    pub full_file_path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub release_year: Option<String>,
    pub track_number: Option<String>,
    pub duration_secs: Option<f64>,
    pub format: Option<String>,
    pub bit_depth: Option<i64>,
    pub sample_rate: Option<i64>,
    pub size: Option<i64>,
    pub channels: Option<i64>,
    pub embedded_cover: bool,
    pub compressed_cached_cover_root: Option<String>,
    pub cover_override: Option<String>, // Hash de carátula específica si difiere del álbum
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

impl Default for Database {
    fn default() -> Self {
        Self::new().expect("No se pudo iniciar la DB local SQLite")
    }
}

impl Database {
    pub fn new() -> Result<Self> {
        let db_path = "library.db";
        let conn = Connection::open(db_path)?;
        Self::create_schema(&conn)?;
        Ok(Self { conn })
    }

    fn create_schema(conn: &Connection) -> Result<()> {
        // Configuraciones de rendimiento
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "cache_size", -64000)?; 
        conn.pragma_update(None, "mmap_size", 268435456)?;
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

        // 7. FTS5 - Búsqueda de texto completo
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
        conn.execute("CREATE TRIGGER IF NOT EXISTS songs_ai AFTER INSERT ON SONGS BEGIN
            INSERT INTO SONGS_FTS(rowid, title, artist_name, album_title)
            VALUES (new.id, new.title, 
                   (SELECT name FROM ARTISTS WHERE id = new.artist_id), 
                   (SELECT title FROM ALBUMS WHERE id = new.album_id));
        END;", [])?;

        conn.execute("CREATE TRIGGER IF NOT EXISTS songs_ad AFTER DELETE ON SONGS BEGIN
            INSERT INTO SONGS_FTS(SONGS_FTS, rowid, title, artist_name, album_title)
            VALUES('delete', old.id, old.title, 
                   (SELECT name FROM ARTISTS WHERE id = old.artist_id), 
                   (SELECT title FROM ALBUMS WHERE id = old.album_id));
        END;", [])?;

        // Índices adicionales para filtros rápidos (agrupación)
        conn.execute("CREATE INDEX IF NOT EXISTS idx_songs_folder ON SONGS(folder_id)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_songs_album ON SONGS(album_id)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_songs_artist ON SONGS(artist_id)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_albums_year ON ALBUMS(year)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_albums_genre ON ALBUMS(genre)", [])?;

        Ok(())
    }

    // --- Helpers de Hashing para Identidad Basada en Rutas ---

    pub fn generate_hash(input: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input);
        hex::encode(hasher.finalize())
    }

    // --- Métodos de Inserción Relacional ---

    pub fn upsert_folder(&self, path: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO FOLDERS (path, name) VALUES (?1, ?2)
             ON CONFLICT(path) DO NOTHING",
            params![path, std::path::Path::new(path).file_name().and_then(|s| s.to_str()).unwrap_or("Música")],
        )?;
        self.conn.query_row("SELECT id FROM FOLDERS WHERE path = ?1", [path], |row| row.get(0))
    }

    pub fn upsert_artist(&self, name: &str) -> Result<i64> {
        let normalized_name = name.trim();
        let hash_id = Self::generate_hash(&normalized_name.to_lowercase());
        self.conn.execute(
            "INSERT INTO ARTISTS (name, hash_id) VALUES (?1, ?2)
             ON CONFLICT(hash_id) DO NOTHING",
            params![normalized_name, hash_id],
        )?;
        self.conn.query_row("SELECT id FROM ARTISTS WHERE hash_id = ?1", [hash_id], |row| row.get(0))
    }

    pub fn upsert_album(&self, title: &str, artist_id: i64, folder_id: i64, folder_path: &str, year: Option<&str>, genre: Option<&str>) -> Result<i64> {
        let hash_id = Self::generate_hash(&format!("{}{}{}", title, artist_id, folder_path));
        self.conn.execute(
            "INSERT INTO ALBUMS (title, artist_id, folder_id, hash_id, year, genre) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(hash_id) DO UPDATE SET
                year = COALESCE(year, excluded.year),
                genre = COALESCE(genre, excluded.genre)",
            params![title, artist_id, folder_id, hash_id, year, genre],
        )?;
        self.conn.query_row("SELECT id FROM ALBUMS WHERE hash_id = ?1", [hash_id], |row| row.get(0))
    }

    pub fn insert_song_full(
        &mut self, 
        song: &SongData, 
        extended: &SongMetadataExtended, 
        pic_bytes_hash: Option<String>,
        raw_tags: Vec<(String, String, String)> // (tag_type, item_key, raw_value)
    ) -> Result<(String, bool)> {
        let folder_path = std::path::Path::new(&song.full_file_path).parent().and_then(|p| p.to_str()).unwrap_or("");
        
        let folder_id = self.upsert_folder(folder_path)?;
        
        // Identidades de Artista por Separado (Pista vs Álbum)
        let raw_track_artist = song.artist.as_deref().unwrap_or("Desconocido");
        let raw_album_artist = song.album_artist.as_deref().or(song.artist.as_deref()).unwrap_or("Desconocido");
        
        let track_artist_id = self.upsert_artist(raw_track_artist)?;
        let album_artist_id = self.upsert_artist(raw_album_artist)?;
        
        let album_title = song.album.as_deref().unwrap_or("Desconocido");
        let album_hash_id = Self::generate_hash(&format!("{}{}{}", album_title, album_artist_id, folder_path));

        // 1. Resolver o crear álbum (Aseguramos que el álbum pertenezca al artista del álbum)
        self.conn.execute(
            "INSERT INTO ALBUMS (title, artist_id, folder_id, hash_id, year, genre) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(hash_id) DO UPDATE SET
                artist_id = excluded.artist_id,
                year = COALESCE(year, excluded.year),
                genre = COALESCE(genre, excluded.genre)",
            params![album_title, album_artist_id, folder_id, album_hash_id, song.release_year.as_deref(), song.genre.as_deref()],
        )?;

        let (_, _): (Option<String>, Option<String>) = self.conn.query_row(
            "SELECT cover_path, cover_hash FROM ALBUMS WHERE hash_id = ?1",
            [&album_hash_id],
            |row| Ok((row.get(0)?, row.get(1)?))
        )?;

        let mut final_song_cover_override = None;
        let mut needs_processing = false;
        let mut target_hash_for_processing = album_hash_id.clone();

        if let Some(current_pic_hash) = pic_bytes_hash {
            // Verificar si el álbum ya tiene carátula
            let album_has_cover: bool = self.conn.query_row(
                "SELECT cover_hash IS NOT NULL FROM ALBUMS WHERE hash_id = ?1",
                [&album_hash_id],
                |row| row.get(0)
            )?;

            if !album_has_cover {
                // Es la primera carátula del álbum, la asignamos como principal
                let path = format!("cache/covers/{}.avif", album_hash_id);
                
                self.conn.execute(
                    "UPDATE ALBUMS SET cover_hash = ?1, cover_path = ?2 WHERE hash_id = ?3",
                    params![current_pic_hash, path, album_hash_id],
                )?;
                needs_processing = true;
            } else {
                let album_cover_hash: Option<String> = self.conn.query_row(
                    "SELECT cover_hash FROM ALBUMS WHERE hash_id = ?1",
                    [&album_hash_id],
                    |row| row.get(0)
                )?;

                if album_cover_hash.as_ref() != Some(&current_pic_hash) {
                    // Es una carátula diferente a la del álbum (Override)
                    let song_cover_hash = Self::generate_hash(&format!("{}{}", song.full_file_path, current_pic_hash));
                    final_song_cover_override = Some(song_cover_hash.clone());
                    target_hash_for_processing = song_cover_hash;
                    needs_processing = true; 
                }
            }
        }

        // 2. Inserción de la canción (Con su propio artista de pista)
        self.conn.execute(
            "INSERT INTO SONGS (
                album_id, artist_id, folder_id, file_path, title, track_num, duration, format, 
                bit_depth, sample_rate, channels, size, embedded_cover, cover_override, import_order
            ) VALUES (
                (SELECT id FROM ALBUMS WHERE hash_id = ?1), ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
            ) ON CONFLICT(file_path) DO UPDATE SET 
                title = excluded.title, track_num = excluded.track_num, duration = excluded.duration,
                artist_id = excluded.artist_id, cover_override = excluded.cover_override",
            params![
                album_hash_id, track_artist_id, folder_id, song.full_file_path, song.title, 
                song.track_number,
                song.duration_secs, song.format, song.bit_depth, song.sample_rate, 
                song.channels, song.size, song.embedded_cover, final_song_cover_override, song.import_order
            ],
        )?;

        let song_id: i64 = self.conn.query_row("SELECT id FROM SONGS WHERE file_path = ?1", [&song.full_file_path], |row| row.get(0))?;

        // 3. Guardar metadatos técnicos crudos de todos los formatos presentes
        self.conn.execute("DELETE FROM SONG_TAG_ITEMS WHERE song_id = ?1", [song_id])?;
        for (tag_type, key, value) in raw_tags {
            self.conn.execute(
                "INSERT OR REPLACE INTO SONG_TAG_ITEMS (song_id, tag_type, item_key, raw_value) 
                 VALUES (?1, ?2, ?3, ?4)",
                params![song_id, tag_type, key, value],
            )?;
        }

        // 4. Metadatos extendidos (Vista unificada)
        self.conn.execute(
            "INSERT INTO SONG_METADATA (
                song_id, lyrics, comments, composer, lyricist, publisher, url, copyright, 
                encoded_by, catalog, isrc, key, bpm, track_gain, album_gain
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ON CONFLICT(song_id) DO UPDATE SET lyrics = excluded.lyrics, comments = excluded.comments",
            params![
                song_id, extended.lyrics, extended.comments, extended.composer, extended.lyricist, 
                extended.publisher, extended.url, extended.copyright, extended.encoded_by, 
                extended.catalog, extended.isrc, extended.key, extended.bpm, extended.track_gain, extended.album_gain
            ],
        )?;

        Ok((target_hash_for_processing, needs_processing))
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

    pub fn search_songs(&self, query: &str) -> Result<Vec<Arc<SongData>>> {
        let mut stmt = self.conn.prepare("
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
        ")?;
        
        let rows = stmt.query_map([format!("{}*", query)], |row| {
            let mut song = SongData::default();
            song.id = row.get(0)?;
            song.full_file_path = row.get(1)?;
            song.title = row.get(2)?;
            song.artist = row.get(3)?;
            song.album = row.get(4)?;
            song.compressed_cached_cover_root = row.get::<_, Option<String>>(5)?; 
            song.duration_secs = row.get(6)?;
            song.format = row.get(7)?;
            song.size = row.get(8)?;
            song.track_number = row.get(9)?; // TEXT -> Option<String>
            song.bit_depth = row.get(10)?;
            song.sample_rate = row.get(11)?;
            song.channels = row.get(12)?;
            song.embedded_cover = row.get(13)?;
            song.cover_override = row.get(14)?;
            song.import_order = row.get(15)?;
            song.genre = row.get(16)?;
            song.release_year = row.get(17)?;
            song.album_artist = row.get(18)?;
            Ok(Arc::new(song))
        })?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(s) = r { results.push(s); }
        }
        Ok(results)
    }

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
            ORDER BY ar.name ASC, al.year ASC, al.title ASC, s.track_num ASC
        ")?;

        let rows = stmt.query_map([], |row| {
            let mut song = SongData::default();
            song.id = row.get(0)?;
            song.full_file_path = row.get(1)?;
            song.title = row.get(2)?;
            song.artist = row.get(3)?;
            song.album = row.get(4)?;
            song.album_id = 0; // WIP: Si se necesita el ID real se puede añadir al SELECT
            song.compressed_cached_cover_root = row.get::<_, Option<String>>(5)?; 
            song.duration_secs = row.get(6)?;
            song.format = row.get(7)?;
            song.size = row.get(8)?;
            song.track_number = row.get(9)?; // TEXT -> Option<String>
            song.bit_depth = row.get(10)?;
            song.sample_rate = row.get(11)?;
            song.channels = row.get(12)?;
            song.embedded_cover = row.get(13)?;
            song.cover_override = row.get(14)?;
            song.import_order = row.get(15)?;
            song.genre = row.get(16)?;
            song.release_year = row.get(17)?;
            song.album_artist = row.get(18)?;
            Ok(Arc::new(song))
        })?;

        let mut songs = Vec::new();
        for r in rows {
            if let Ok(s) = r { songs.push(s); }
        }
        Ok(songs)
    }

    pub fn get_all_albums(&self) -> Result<Vec<(String, String, String, String, String, Option<String>)>> {
        let mut stmt = self.conn.prepare("
            SELECT al.hash_id, al.title, ar.name, al.genre, al.year, al.cover_path
            FROM ALBUMS al
            JOIN ARTISTS ar ON al.artist_id = ar.id
            ORDER BY ar.name ASC, al.year ASC, al.title ASC
        ")?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3).unwrap_or_default(), 
                row.get(4).unwrap_or_default(), row.get(5)?
            ))
        })?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(a) = r { results.push(a); }
        }
        Ok(results)
    }

    /// Obtiene álbumes desglosados por artista para la vista Grid (Consistencia con Listas)
    pub fn get_grid_items_by_artist(&self) -> Result<Vec<(String, String, String, String, String, Option<String>)>> {
        let mut stmt = self.conn.prepare("
            SELECT al.hash_id, al.title, ar.name, al.genre, al.year, al.cover_path
            FROM SONGS s
            JOIN ARTISTS ar ON s.artist_id = ar.id
            JOIN ALBUMS al ON s.album_id = al.id
            GROUP BY al.id, ar.id
            ORDER BY ar.name ASC, al.year ASC, al.title ASC
        ")?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3).unwrap_or_default(), 
                row.get(4).unwrap_or_default(), row.get(5)?
            ))
        })?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(a) = r { results.push(a); }
        }
        Ok(results)
    }

    pub fn get_all_folders(&self) -> Result<Vec<(i64, String, String)>> {
        let mut stmt = self.conn.prepare("SELECT id, path, name FROM FOLDERS ORDER BY path ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(f) = r { results.push(f); }
        }
        Ok(results)
    }

    pub fn get_album_stats_by_hash(&self, album_hash: &str) -> Result<(u64, f64, f64)> {
        let mut stmt = self.conn.prepare("
            SELECT COUNT(*), SUM(duration), SUM(size)
            FROM SONGS s
            JOIN ALBUMS al ON s.album_id = al.id
            WHERE al.hash_id = ?1
        ")?;
        
        let stats = stmt.query_row(params![album_hash], |row| {
            Ok((
                row.get::<_, i64>(0)? as u64,
                row.get::<_, f64>(1).unwrap_or(0.0),
                row.get::<_, f64>(2).unwrap_or(0.0) as f64,
            ))
        })?;
        Ok(stats)
    }

    pub fn get_album_stats_by_hash_and_artist(&self, album_hash: &str, artist_name: &str) -> Result<(u64, f64, f64)> {
        let mut stmt = self.conn.prepare("
            SELECT COUNT(*), SUM(s.duration), SUM(s.size)
            FROM SONGS s
            JOIN ALBUMS al ON s.album_id = al.id
            JOIN ARTISTS ar ON s.artist_id = ar.id
            WHERE al.hash_id = ?1 AND ar.name = ?2
        ")?;
        
        let stats = stmt.query_row(params![album_hash, artist_name], |row| {
            Ok((
                row.get::<_, i64>(0)? as u64,
                row.get::<_, f64>(1).unwrap_or(0.0),
                row.get::<_, f64>(2).unwrap_or(0.0) as f64,
            ))
        })?;
        Ok(stats)
    }

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
            ORDER BY ar.name ASC, al.year ASC, al.title ASC, CAST(s.track_num AS INTEGER) ASC, s.track_num ASC
        ")?;

        let rows = stmt.query_map([album_hash_id], |row| {
            let mut song = SongData::default();
            song.id = row.get(0)?;
            song.full_file_path = row.get(1)?;
            song.title = row.get(2)?;
            song.artist = row.get(3)?;
            song.album = row.get(4)?;
            song.compressed_cached_cover_root = row.get::<_, Option<String>>(5)?; 
            song.duration_secs = row.get(6)?;
            song.format = row.get(7)?;
            song.size = row.get(8)?;
            song.track_number = row.get(9)?; // TEXT -> Option<String>
            song.bit_depth = row.get(10)?;
            song.sample_rate = row.get(11)?;
            song.channels = row.get(12)?;
            song.embedded_cover = row.get(13)?;
            song.cover_override = row.get(14)?;
            song.import_order = row.get(15)?;
            song.genre = row.get(16)?;
            song.release_year = row.get(17)?;
            song.album_artist = row.get(18)?;
            Ok(Arc::new(song))
        })?;

        let mut songs = Vec::new();
        for r in rows {
            if let Ok(s) = r { songs.push(s); }
        }
        Ok(songs)
    }

    pub fn get_songs_by_album_and_artist(&self, album_hash_id: &str, artist_name: &str) -> Result<Vec<Arc<SongData>>> {
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
            song.full_file_path = row.get(1)?;
            song.title = row.get(2)?;
            song.artist = row.get(3)?;
            song.album = row.get(4)?;
            song.compressed_cached_cover_root = row.get::<_, Option<String>>(5)?; 
            song.duration_secs = row.get(6)?;
            song.format = row.get(7)?;
            song.size = row.get(8)?;
            song.track_number = row.get(9)?; // TEXT -> Option<String>
            song.bit_depth = row.get(10)?;
            song.sample_rate = row.get(11)?;
            song.channels = row.get(12)?;
            song.embedded_cover = row.get(13)?;
            song.cover_override = row.get(14)?;
            song.import_order = row.get(15)?;
            song.genre = row.get(16)?;
            song.release_year = row.get(17)?;
            song.album_artist = row.get(18)?;
            Ok(Arc::new(song))
        })?;

        let mut songs = Vec::new();
        for r in rows {
            if let Ok(s) = r { songs.push(s); }
        }
        Ok(songs)
    }

    pub fn get_artist_groups_sql(&self, search_query: Option<&str>) -> Result<Vec<(String, usize, usize, f64)>> {
        let mut sql = String::from("
            SELECT ar.name, COUNT(s.id), COUNT(DISTINCT al.id), SUM(s.duration)
            FROM SONGS s
            JOIN ARTISTS ar ON s.artist_id = ar.id
            JOIN ALBUMS al ON s.album_id = al.id
        ");
        
        let mut params_vec = Vec::new();
        if let Some(q) = search_query {
            if !q.is_empty() {
                sql.push_str(" WHERE s.id IN (SELECT rowid FROM SONGS_FTS WHERE SONGS_FTS MATCH ?1)");
                params_vec.push(format!("{}*", q));
            }
        }
        
        sql.push_str(" GROUP BY ar.name ORDER BY ar.name ASC");
        
        let rows = if params_vec.is_empty() {
            let mut stmt = self.conn.prepare(&sql)?;
            let mapped = stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get::<_, i64>(1)? as usize, row.get::<_, i64>(2)? as usize, row.get::<_, f64>(3).unwrap_or(0.0)))
            })?;
            let mut res = Vec::new();
            for r in mapped { if let Ok(v) = r { res.push(v); } }
            res
        } else {
            let mut stmt = self.conn.prepare(&sql)?;
            let mapped = stmt.query_map([&params_vec[0]], |row| {
                Ok((row.get(0)?, row.get::<_, i64>(1)? as usize, row.get::<_, i64>(2)? as usize, row.get::<_, f64>(3).unwrap_or(0.0)))
            })?;
            let mut res = Vec::new();
            for r in mapped { if let Ok(v) = r { res.push(v); } }
            res
        };

        Ok(rows)
    }

    pub fn get_max_import_order(&self) -> Result<i64> {
        let mut stmt = self.conn.prepare("SELECT MAX(import_order) FROM SONGS")?;
        let res = stmt.query_row([], |row| {
             let val: Option<i64> = row.get(0)?;
             Ok(val.unwrap_or(0))
        });
        res
    }

    pub fn get_library_stats(&self) -> Result<(usize, usize, f64, f64, usize)> {
        let songs: i64 = self.conn.query_row("SELECT COUNT(*) FROM SONGS", [], |r| r.get(0))?;
        let albums: i64 = self.conn.query_row("SELECT COUNT(*) FROM ALBUMS", [], |r| r.get(0))?;
        let artists: i64 = self.conn.query_row("SELECT COUNT(*) FROM ARTISTS", [], |r| r.get(0))?;
        let duration: f64 = self.conn.query_row("SELECT SUM(duration) FROM SONGS", [], |r| Ok(r.get::<_, Option<f64>>(0)?.unwrap_or(0.0)))?;
        let size: f64 = self.conn.query_row("SELECT SUM(size) FROM SONGS", [], |r| Ok(r.get::<_, Option<i64>>(0)?.unwrap_or(0) as f64))?;
        
        Ok((songs as usize, albums as usize, duration, size, artists as usize))
    }

    pub fn delete_song(&self, path: &str) -> Result<()> {
        self.conn.execute("DELETE FROM SONGS WHERE full_file_path = ?1", [path])?;
        Ok(())
    }

    pub fn delete_album_group(&self, album_id: &str) -> Result<()> {
        // En el nuevo esquema, album_id es el id entero o el hash en metadata.
        // Aquí asumimos que recibimos el hash que identifica al álbum.
        let id: i32 = self.conn.query_row("SELECT id FROM ALBUMS WHERE album_hash = ?1", [album_id], |r| r.get(0))?;
        self.conn.execute("DELETE FROM SONGS WHERE album_id = ?1", [id])?;
        self.conn.execute("DELETE FROM ALBUMS WHERE id = ?1", [id])?;
        Ok(())
    }
}
