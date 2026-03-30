use rusqlite::{params, Connection, Result};
use std::sync::Arc;

pub struct Database {
    #[allow(dead_code)]
    conn: Connection,
}

#[derive(Debug, Default, Clone)]
pub struct SongData {
    pub full_file_path: String,
    pub file_name: String,
    pub root_directory_name: Option<String>,
    pub full_root_directory_path: Option<String>,
    pub format: Option<String>,
    pub size: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub duration_secs: Option<f64>,
    pub bit_depth: Option<i64>,
    
    pub embedded_cover: bool,
    pub original_cover_root: Option<String>,
    pub compressed_cached_cover_root: Option<String>,
    
    pub track_number: Option<String>,
    pub total_tracks: Option<String>,
    pub disc_number: Option<String>,
    pub total_discs: Option<String>,
    
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub release_year: Option<String>,
    pub album_artist: Option<String>,
    pub album_artist_tag_format: Option<String>,
    pub lyrics: Option<String>,
    
    pub track_gain: Option<f64>,
    pub album_gain: Option<f64>,
    pub comments: Option<String>,
    pub url: Option<String>,
    pub copyright: Option<String>,
    pub publisher: Option<String>,
    pub composer: Option<String>,
    pub lyricist: Option<String>,
    pub director: Option<String>,
    pub encoded_by: Option<String>,
    pub catalog: Option<String>,
    pub isrc: Option<String>,
    pub key: Option<String>,
    pub bpm: Option<String>,
    
    pub import_order: i64,
}

impl Default for Database {
    fn default() -> Self {
        Self::new().expect("No se pudo iniciar la DB local SQLite")
    }
}

impl Database {
    pub fn new() -> Result<Self> {
        let db_path = "library.db"; // Local testing
        let conn = Connection::open(db_path)?;
        
        // Inicializar esquema
        Self::create_schema(&conn)?;
        Ok(Self { conn })
    }

    fn create_schema(conn: &Connection) -> Result<()> {
        // Habilitar modo WAL para concurrencia y rapidez
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "cache_size", -64000)?; // 64MB de caché
        conn.pragma_update(None, "mmap_size", 268435456)?; // 256MB de memory mapping

        conn.execute(
            "CREATE TABLE IF NOT EXISTS MUSIC_LIBRARY (
                FULL_FILE_PATH TEXT PRIMARY KEY,
                FILE_NAME TEXT NOT NULL,
                ROOT_DIRECTORY_NAME TEXT,
                FULL_ROOT_DIRECTORY_PATH TEXT,
                FORMAT TEXT,
                SIZE INTEGER,
                SAMPLE_RATE INTEGER,
                CHANNELS INTEGER,
                DURATION_SECS REAL,
                BIT_DEPTH INTEGER,
                EMBEDDED_COVER BOOLEAN DEFAULT 0,
                ORIGINAL_COVER_ROOT TEXT,
                COMPRESSED_CACHED_COVER_ROOT TEXT,
                TRACK_NUMBER TEXT,
                TOTAL_TRACKS TEXT,
                DISC_NUMBER TEXT,
                TOTAL_DISCS TEXT,
                TITLE TEXT,
                ARTIST TEXT,
                ALBUM TEXT,
                GENRE TEXT,
                RELEASE_YEAR TEXT,
                ALBUM_ARTIST TEXT,
                ALBUM_ARTIST_TAG_FORMAT TEXT,
                LYRICS TEXT,
                TRACK_GAIN REAL,
                ALBUM_GAIN REAL,
                COMMENTS TEXT,
                URL TEXT,
                COPYRIGHT TEXT,
                PUBLISHER TEXT,
                COMPOSER TEXT,
                LYRICIST TEXT,
                DIRECTOR TEXT,
                ENCODED_BY TEXT,
                CATALOG TEXT,
                ISRC TEXT,
                KEY TEXT,
                BPM TEXT,
                ALBUM_ID_HASH TEXT,
                PLAY_COUNT INTEGER DEFAULT 0,
                LAST_PLAYED_DATE TEXT,
                LAST_PLAYED_TIME TEXT,
                AUTO_RATING INTEGER DEFAULT 0,
                PERSONAL_RATING INTEGER DEFAULT 0,
                IMPORT_ORDER INTEGER DEFAULT 0
            )",
            [],
        )?;

        // Tablas de optimización por Artista y Álbum
        conn.execute(
            "CREATE TABLE IF NOT EXISTS ARTISTS (
                NAME TEXT PRIMARY KEY,
                TOTAL_SONGS INTEGER DEFAULT 0,
                TOTAL_ALBUMS INTEGER DEFAULT 0
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS ALBUMS (
                ARTIST_ALBUM_HASH TEXT PRIMARY KEY,
                TITLE TEXT,
                ARTIST TEXT,
                GENRE TEXT,
                YEAR TEXT,
                COVER_PATH TEXT,
                TOTAL_SONGS INTEGER DEFAULT 0,
                DURATION REAL DEFAULT 0.0
            )",
            [],
        )?;

        // Índices estratégicos para búsqueda y ordenación instantánea
        conn.execute("CREATE INDEX IF NOT EXISTS idx_artist ON MUSIC_LIBRARY (ARTIST)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_album ON MUSIC_LIBRARY (ALBUM)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_album_hash ON MUSIC_LIBRARY (ALBUM_ID_HASH)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_import ON MUSIC_LIBRARY (IMPORT_ORDER)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_artist_album ON MUSIC_LIBRARY (ALBUM_ID_HASH)", [])?;

        Ok(())
    }
    
    // WIP: Funciones de Guardado/Carga para el escáner se implementarán en el siguiente bloque
    pub fn insert_song(&mut self, record: &SongData) -> Result<()> {
        self.conn.execute(
              "INSERT OR REPLACE INTO MUSIC_LIBRARY (
                FULL_FILE_PATH, FILE_NAME, ROOT_DIRECTORY_NAME, FULL_ROOT_DIRECTORY_PATH,
                FORMAT, SIZE, SAMPLE_RATE, CHANNELS, DURATION_SECS, BIT_DEPTH,
                EMBEDDED_COVER, ORIGINAL_COVER_ROOT, COMPRESSED_CACHED_COVER_ROOT,
                TRACK_NUMBER, TOTAL_TRACKS, DISC_NUMBER, TOTAL_DISCS,
                TITLE, ARTIST, ALBUM, GENRE, RELEASE_YEAR, ALBUM_ARTIST, ALBUM_ARTIST_TAG_FORMAT,
                LYRICS, TRACK_GAIN, ALBUM_GAIN, COMMENTS, URL, COPYRIGHT, PUBLISHER,
                COMPOSER, LYRICIST, DIRECTOR, ENCODED_BY, CATALOG, ISRC, KEY, BPM, ALBUM_ID_HASH, IMPORT_ORDER
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, 
                ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, 
                ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41
            )",
            params![
                record.full_file_path, record.file_name, record.root_directory_name, record.full_root_directory_path,
                record.format, record.size, record.sample_rate, record.channels, record.duration_secs, record.bit_depth,
                record.embedded_cover, record.original_cover_root, record.compressed_cached_cover_root,
                record.track_number, record.total_tracks, record.disc_number, record.total_discs,
                record.title, record.artist, record.album, record.genre, record.release_year, record.album_artist, record.album_artist_tag_format,
                record.lyrics, record.track_gain, record.album_gain, record.comments, record.url, record.copyright, record.publisher,
                record.composer, record.lyricist, record.director, record.encoded_by, record.catalog, record.isrc, record.key, record.bpm, 
                crate::utils::covers::generate_album_id(
                    record.album_artist.as_deref().or(record.artist.as_deref()).unwrap_or("Desconocido"),
                    record.album.as_deref().unwrap_or("Desconocido")
                ),
                record.import_order
            ],
         )?;

        // Actualizar tabla de índices de artistas
        let artist_name = record.album_artist.as_ref().or(record.artist.as_ref()).map(|s| s.as_str()).unwrap_or("Desconocido");
        self.upsert_artist(artist_name)?;

        Ok(())
    }

    pub fn upsert_artist(&mut self, name: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO ARTISTS (NAME, TOTAL_SONGS, TOTAL_ALBUMS) 
             VALUES (?1, 1, 0)
             ON CONFLICT(NAME) DO UPDATE SET 
                TOTAL_SONGS = TOTAL_SONGS + 1",
            [name],
        )?;
        Ok(())
    }

    pub fn commit_transaction(&self) -> Result<()> {
        self.conn.execute("COMMIT", [])?;
        Ok(())
    }

    pub fn begin_transaction(&self) -> Result<()> {
        self.conn.execute("BEGIN TRANSACTION", [])?;
        Ok(())
    }

    pub fn upsert_album(&self, hash: &str, title: &str, artist: &str, genre: &str, year: &str, cover: Option<&str>, duration: f64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO ALBUMS (ARTIST_ALBUM_HASH, TITLE, ARTIST, GENRE, YEAR, COVER_PATH, TOTAL_SONGS, DURATION)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)
             ON CONFLICT(ARTIST_ALBUM_HASH) DO UPDATE SET 
                TOTAL_SONGS = TOTAL_SONGS + 1,
                DURATION = DURATION + excluded.DURATION,
                GENRE = CASE WHEN GENRE = '' OR GENRE IS NULL THEN excluded.GENRE ELSE GENRE END,
                COVER_PATH = COALESCE(COVER_PATH, excluded.COVER_PATH)",
            params![hash, title, artist, genre, year, cover, duration],
        )?;
        Ok(())
    }
    
    pub fn get_max_import_order(&self) -> Result<i64> {
        let mut stmt = self.conn.prepare("SELECT MAX(IMPORT_ORDER) FROM MUSIC_LIBRARY")?;
        let res = stmt.query_row([], |row| {
             let val: Option<i64> = row.get(0)?;
             Ok(val.unwrap_or(0))
        });
        res
    }

    /// Retorna: (ClaveAlbum_Path_Unico, Titulo_Album, Artista_Agrupado, Género, Año, Ruta_Portada)
    /// Optimizado usando la tabla de índices ALBUMS para carga instantánea
    pub fn get_all_albums(&self) -> Result<Vec<(String, String, String, String, String, Option<String>)>> {
        let mut stmt = self.conn.prepare("
            SELECT 
                ARTIST_ALBUM_HASH, 
                TITLE, 
                ARTIST, 
                GENRE, 
                YEAR, 
                COVER_PATH 
            FROM ALBUMS 
            ORDER BY ARTIST ASC, TITLE ASC
        ")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get(0).unwrap_or_default(),
                row.get(1).unwrap_or_default(),
                row.get(2).unwrap_or_default(),
                row.get(3).unwrap_or_default(),
                row.get(4).unwrap_or_default(),
                row.get(5).ok()
            ))
        })?;
        let mut albums = Vec::new();
        for r in rows {
            if let Ok(a) = r { albums.push(a); }
        }
        
        // Fallback si la tabla ALBUMS está vacía (por compatibilidad o primera carga)
        if albums.is_empty() {
             let mut stmt = self.conn.prepare("
                SELECT 
                    FULL_FILE_PATH,
                    COALESCE(ALBUM, 'Desconocido'), 
                    COALESCE(ALBUM_ARTIST, ARTIST, 'Desconocido') AS grouped_artist, 
                    GENRE, 
                    RELEASE_YEAR, 
                    COALESCE(COMPRESSED_CACHED_COVER_ROOT, ORIGINAL_COVER_ROOT) AS COVER
                FROM MUSIC_LIBRARY 
                GROUP BY COALESCE(ALBUM, 'Desconocido'), COALESCE(ALBUM_ARTIST, ARTIST, 'Desconocido'), RELEASE_YEAR, FULL_ROOT_DIRECTORY_PATH
                ORDER BY IMPORT_ORDER ASC, grouped_artist ASC, RELEASE_YEAR ASC, COALESCE(ALBUM, 'Desconocido') ASC
            ")?;
            let rows = stmt.query_map([], |row| {
                let id: String = row.get(0).unwrap_or_default();
                let album: String = row.get(1).unwrap_or_default();
                let artist: String = row.get(2).unwrap_or_default();
                let genre: Option<String> = row.get(3).unwrap_or(None);
                let year: Option<String> = row.get(4).unwrap_or(None);
                let cover: Option<String> = row.get(5).unwrap_or(None);
                Ok((id, album, artist, genre.unwrap_or_default(), year.unwrap_or_default(), cover))
            })?;
            for r in rows {
                if let Ok(a) = r { albums.push(a); }
            }
        }
        Ok(albums)
    }

    pub fn get_album_stats(&self, sample_file_path: &str) -> Result<(u64, f64, f64)> {
        let mut stmt = self.conn.prepare("
            SELECT COUNT(*), SUM(DURATION_SECS), SUM(SIZE) 
            FROM MUSIC_LIBRARY 
            WHERE COALESCE(ALBUM, '') = (SELECT COALESCE(ALBUM, '') FROM MUSIC_LIBRARY WHERE FULL_FILE_PATH = ?1 LIMIT 1)
            AND COALESCE(ALBUM_ARTIST, ARTIST, '') = (SELECT COALESCE(ALBUM_ARTIST, ARTIST, '') FROM MUSIC_LIBRARY WHERE FULL_FILE_PATH = ?1 LIMIT 1)
            AND FULL_ROOT_DIRECTORY_PATH = (SELECT FULL_ROOT_DIRECTORY_PATH FROM MUSIC_LIBRARY WHERE FULL_FILE_PATH = ?1 LIMIT 1)
        ")?;
        let stats = stmt.query_row(params![sample_file_path], |row| {
            Ok((
                row.get::<_, i64>(0).unwrap_or(0) as u64,
                row.get::<_, f64>(1).unwrap_or(0.0),
                row.get::<_, f64>(2).unwrap_or(0.0),
            ))
        })?;
        Ok(stats)
    }

    /// Obtiene estadísticas de un álbum usando su hash (ALBUM_ID_HASH) directamente.
    /// Usado por la vista Grid donde selected_album contiene el hash, no un file_path.
    pub fn get_album_stats_by_hash(&self, album_hash: &str) -> Result<(u64, f64, f64)> {
        let mut stmt = self.conn.prepare("
            SELECT COUNT(*), COALESCE(SUM(DURATION_SECS), 0.0), COALESCE(SUM(SIZE), 0.0) 
            FROM MUSIC_LIBRARY 
            WHERE ALBUM_ID_HASH = ?1
        ")?;
        let stats = stmt.query_row(params![album_hash], |row| {
            Ok((
                row.get::<_, i64>(0).unwrap_or(0) as u64,
                row.get::<_, f64>(1).unwrap_or(0.0),
                row.get::<_, f64>(2).unwrap_or(0.0),
            ))
        })?;
        Ok(stats)
    }

    pub fn get_songs_by_album(&self, album_id_hash: &str) -> Result<Vec<Arc<SongData>>> {
        let mut stmt = self.conn.prepare("
            SELECT FULL_FILE_PATH, TITLE, ARTIST, ALBUM, RELEASE_YEAR, TRACK_NUMBER, FORMAT, SIZE, SAMPLE_RATE, CHANNELS, DURATION_SECS, GENRE, BIT_DEPTH, ALBUM_ARTIST, ALBUM_ARTIST_TAG_FORMAT 
            FROM MUSIC_LIBRARY 
            WHERE ALBUM_ID_HASH = ?1
            ORDER BY CAST(TRACK_NUMBER AS INTEGER) ASC, TITLE ASC
        ")?;
        let rows = stmt.query_map([album_id_hash], |row| {
            let mut record = SongData::default();
            record.full_file_path = row.get(0).unwrap_or_default();
            record.title = row.get(1).ok();
            record.artist = row.get(2).ok();
            record.album = row.get(3).ok();
            record.release_year = row.get(4).ok();
            record.track_number = row.get::<_, String>(5).ok();
            record.format = row.get(6).ok();
            record.size = row.get(7).ok();
            record.sample_rate = row.get(8).ok();
            record.channels = row.get(9).ok();
            record.duration_secs = row.get(10).ok();
            record.genre = row.get(11).ok();
            record.bit_depth = row.get(12).ok();
            record.album_artist = row.get(13).ok();
            record.album_artist_tag_format = row.get(14).ok();
            Ok(record)
        })?;
        let mut songs = Vec::new();
        for r in rows {
            if let Ok(s) = r { songs.push(Arc::new(s)); }
        }
        Ok(songs)
    }

    pub fn get_all_songs(&self) -> Result<Vec<Arc<SongData>>> {
        let mut stmt = self.conn.prepare("
            SELECT FULL_FILE_PATH, TITLE, ARTIST, ALBUM, RELEASE_YEAR, TRACK_NUMBER, FORMAT, SIZE, SAMPLE_RATE, CHANNELS, DURATION_SECS, GENRE, BIT_DEPTH, ALBUM_ARTIST, ALBUM_ARTIST_TAG_FORMAT, ORIGINAL_COVER_ROOT, COMPRESSED_CACHED_COVER_ROOT
            FROM MUSIC_LIBRARY 
            ORDER BY COALESCE(ALBUM_ARTIST, ARTIST, 'Desconocido') ASC, RELEASE_YEAR ASC, ALBUM ASC, CAST(TRACK_NUMBER AS INTEGER) ASC
        ")?;
        let rows = stmt.query_map([], |row| {
            let mut record = SongData::default();
            record.full_file_path = row.get(0).unwrap_or_default();
            record.title = row.get(1).ok();
            record.artist = row.get(2).ok();
            record.album = row.get(3).ok();
            record.release_year = row.get(4).ok();
            record.track_number = row.get::<_, String>(5).ok();
            record.format = row.get(6).ok();
            record.size = row.get(7).ok();
            record.sample_rate = row.get(8).ok();
            record.channels = row.get(9).ok();
            record.duration_secs = row.get(10).ok();
            record.genre = row.get(11).ok();
            record.bit_depth = row.get(12).ok();
            record.album_artist = row.get(13).ok();
            record.album_artist_tag_format = row.get(14).ok();
            record.original_cover_root = row.get(15).ok().flatten();
            record.compressed_cached_cover_root = row.get(16).ok().flatten();
            Ok(record)
        })?;
        let mut songs = Vec::new();
        for r in rows {
            if let Ok(s) = r { songs.push(Arc::new(s)); }
        }
        Ok(songs)
    }

    pub fn delete_song(&self, file_path: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM MUSIC_LIBRARY WHERE FULL_FILE_PATH = ?1",
            params![file_path],
        )?;
        Ok(())
    }

    pub fn delete_album(&self, album_name: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM MUSIC_LIBRARY WHERE ALBUM = ?1",
            params![album_name],
        )?;
        Ok(())
    }

    pub fn delete_album_group(&self, sample_file_path: &str) -> Result<()> {
        self.conn.execute("
            DELETE FROM MUSIC_LIBRARY 
            WHERE COALESCE(ALBUM, '') = (SELECT COALESCE(ALBUM, '') FROM MUSIC_LIBRARY WHERE FULL_FILE_PATH = ?1 LIMIT 1)
            AND COALESCE(ALBUM_ARTIST, ARTIST, '') = (SELECT COALESCE(ALBUM_ARTIST, ARTIST, '') FROM MUSIC_LIBRARY WHERE FULL_FILE_PATH = ?1 LIMIT 1)
            AND FULL_ROOT_DIRECTORY_PATH = (SELECT FULL_ROOT_DIRECTORY_PATH FROM MUSIC_LIBRARY WHERE FULL_FILE_PATH = ?1 LIMIT 1)
        ", params![sample_file_path])?;
        Ok(())
    }
    
    pub fn get_library_stats(&self) -> Result<(usize, usize, f64, f64, usize)> {
        let mut stmt = self.conn.prepare("
            SELECT 
                COUNT(FULL_FILE_PATH) as total_songs,
                COUNT(DISTINCT ALBUM) as total_albums,
                SUM(COALESCE(DURATION_SECS, 0.0)) as total_duration,
                SUM(COALESCE(SIZE, 0)) as total_size,
                COUNT(DISTINCT COALESCE(ARTIST, ALBUM_ARTIST)) as total_artists
            FROM MUSIC_LIBRARY
        ")?;
        
        // SQLite will return integers as i64 and reals as f64 generally in the sums
        let mut total_songs = 0;
        let mut total_albums = 0;
        let mut total_duration = 0.0;
        let mut total_size = 0.0;
        
        let mut total_artists = 0;
        
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            total_songs = row.get::<_, i64>(0).unwrap_or(0) as usize;
            total_albums = row.get::<_, i64>(1).unwrap_or(0) as usize;
            total_duration = row.get::<_, f64>(2).unwrap_or(0.0);
            total_size = row.get::<_, f64>(3).unwrap_or(0.0);
            total_artists = row.get::<_, i64>(4).unwrap_or(0) as usize;
        }
        
        Ok((total_songs, total_albums, total_duration, total_size, total_artists))
    }
}

