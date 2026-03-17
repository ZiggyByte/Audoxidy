use rusqlite::{params, Connection, Result};

pub struct Database {
    #[allow(dead_code)]
    conn: Connection,
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
        conn.execute(
            "CREATE TABLE IF NOT EXISTS MUSIC_LIBRARY (
                FULL_FILE_PATH TEXT PRIMARY KEY,
                
                -- Identificadores Físicos y Formato
                FILE_NAME TEXT NOT NULL,
                ROOT_DIRECTORY_NAME TEXT,
                FULL_ROOT_DIRECTORY_PATH TEXT,
                FORMAT TEXT,
                SIZE INTEGER,
                SAMPLE_RATE INTEGER,
                CHANNELS INTEGER,
                DURATION_SECS REAL,
                BIT_DEPTH INTEGER,
                
                -- Identificadores Visuales
                EMBEDDED_COVER BOOLEAN DEFAULT 0,
                ORIGINAL_COVER_ROOT TEXT,
                COMPRESSED_CACHED_COVER_ROOT TEXT,
                
                -- Contadores de Tracks
                TRACK_NUMBER INTEGER,
                TOTAL_TRACKS INTEGER,
                DISC_NUMBER INTEGER,
                TOTAL_DISCS INTEGER,
                
                -- Tags Universales Básicos
                TITLE TEXT,
                ARTIST TEXT,
                ALBUM TEXT,
                GENRE TEXT,
                RELEASE_YEAR TEXT,
                ALBUM_ARTIST TEXT,
                LYRICS TEXT,
                
                -- Tags Avanzados/Audiófilo
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
                
                -- Metadatos Analíticos
                PLAY_COUNT INTEGER DEFAULT 0,
                LAST_PLAYED_DATE TEXT,
                LAST_PLAYED_TIME TEXT,
                AUTO_RATING INTEGER DEFAULT 0,
                PERSONAL_RATING INTEGER DEFAULT 0
            )",
            [],
        )?;
        // Intentar agregar columnas si no existen (migración sencilla DB antigua)
        let _ = conn.execute("ALTER TABLE MUSIC_LIBRARY ADD COLUMN DURATION_SECS REAL", []);
        let _ = conn.execute("ALTER TABLE MUSIC_LIBRARY ADD COLUMN BIT_DEPTH INTEGER", []);
        
        Ok(())
    }
    
    // WIP: Funciones de Guardado/Carga para el escáner se implementarán en el siguiente bloque
    pub fn insert_song(&mut self, record: &SongRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO MUSIC_LIBRARY (
                FULL_FILE_PATH, FILE_NAME, ROOT_DIRECTORY_NAME, FULL_ROOT_DIRECTORY_PATH, 
                FORMAT, SIZE, SAMPLE_RATE, CHANNELS, DURATION_SECS, BIT_DEPTH, EMBEDDED_COVER, ORIGINAL_COVER_ROOT, COMPRESSED_CACHED_COVER_ROOT,
                TRACK_NUMBER, TOTAL_TRACKS, DISC_NUMBER, TOTAL_DISCS, TITLE, ARTIST, ALBUM, GENRE, RELEASE_YEAR, 
                ALBUM_ARTIST, LYRICS, TRACK_GAIN, ALBUM_GAIN, COMMENTS, URL, COPYRIGHT, PUBLISHER, COMPOSER, LYRICIST,
                DIRECTOR, ENCODED_BY, CATALOG, ISRC, KEY, BPM
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38
            )
            ON CONFLICT(FULL_FILE_PATH) DO UPDATE SET
                FILE_NAME=excluded.FILE_NAME, ROOT_DIRECTORY_NAME=excluded.ROOT_DIRECTORY_NAME, 
                FULL_ROOT_DIRECTORY_PATH=excluded.FULL_ROOT_DIRECTORY_PATH, FORMAT=excluded.FORMAT, 
                SIZE=excluded.SIZE, SAMPLE_RATE=excluded.SAMPLE_RATE, CHANNELS=excluded.CHANNELS, 
                DURATION_SECS=excluded.DURATION_SECS, BIT_DEPTH=excluded.BIT_DEPTH,
                EMBEDDED_COVER=excluded.EMBEDDED_COVER, ORIGINAL_COVER_ROOT=excluded.ORIGINAL_COVER_ROOT, 
                COMPRESSED_CACHED_COVER_ROOT=excluded.COMPRESSED_CACHED_COVER_ROOT, TRACK_NUMBER=excluded.TRACK_NUMBER, 
                TOTAL_TRACKS=excluded.TOTAL_TRACKS, DISC_NUMBER=excluded.DISC_NUMBER, TOTAL_DISCS=excluded.TOTAL_DISCS, 
                TITLE=excluded.TITLE, ARTIST=excluded.ARTIST, ALBUM=excluded.ALBUM, GENRE=excluded.GENRE, 
                RELEASE_YEAR=excluded.RELEASE_YEAR, ALBUM_ARTIST=excluded.ALBUM_ARTIST, LYRICS=excluded.LYRICS, 
                TRACK_GAIN=excluded.TRACK_GAIN, ALBUM_GAIN=excluded.ALBUM_GAIN, COMMENTS=excluded.COMMENTS, 
                URL=excluded.URL, COPYRIGHT=excluded.COPYRIGHT, PUBLISHER=excluded.PUBLISHER, COMPOSER=excluded.COMPOSER, 
                LYRICIST=excluded.LYRICIST, DIRECTOR=excluded.DIRECTOR, ENCODED_BY=excluded.ENCODED_BY, 
                CATALOG=excluded.CATALOG, ISRC=excluded.ISRC, KEY=excluded.KEY, BPM=excluded.BPM",
            params![
                &record.full_file_path, &record.file_name, &record.root_directory_name, &record.full_root_directory_path,
                &record.format, &record.size, &record.sample_rate, &record.channels, &record.duration_secs, &record.bit_depth, &record.embedded_cover, 
                &record.original_cover_root, &record.compressed_cached_cover_root, &record.track_number, 
                &record.total_tracks, &record.disc_number, &record.total_discs, &record.title, &record.artist, 
                &record.album, &record.genre, &record.release_year, &record.album_artist, &record.lyrics, 
                &record.track_gain, &record.album_gain, &record.comments, &record.url, &record.copyright, 
                &record.publisher, &record.composer, &record.lyricist, &record.director, &record.encoded_by, 
                &record.catalog, &record.isrc, &record.key, &record.bpm,
            ],
        )?;
        Ok(())
    }

    pub fn get_all_albums(&self) -> Result<Vec<(String, String, String, String, Option<String>)>> {
        let mut stmt = self.conn.prepare("SELECT ALBUM, ARTIST, GENRE, RELEASE_YEAR, ORIGINAL_COVER_ROOT FROM MUSIC_LIBRARY WHERE ALBUM IS NOT NULL GROUP BY ALBUM")?;
        let rows = stmt.query_map([], |row| {
            let album: String = row.get(0).unwrap_or_else(|_| "Unknown Album".to_string());
            let artist: String = row.get(1).unwrap_or_else(|_| "Unknown Artist".to_string());
            let genre: Option<String> = row.get(2).unwrap_or(None);
            let year: Option<String> = row.get(3).unwrap_or(None);
            let cover: Option<String> = row.get(4).unwrap_or(None);
            Ok((album, artist, genre.unwrap_or_default(), year.unwrap_or_default(), cover))
        })?;
        let mut albums = Vec::new();
        for r in rows {
            if let Ok(a) = r { albums.push(a); }
        }
        Ok(albums)
    }

    pub fn get_album_stats(&self, album_name: &str) -> Result<(u64, f64, f64)> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*), SUM(DURATION_SECS), SUM(SIZE) FROM MUSIC_LIBRARY WHERE ALBUM = ?1")?;
        let stats = stmt.query_row(params![album_name], |row| {
            Ok((
                row.get::<_, i64>(0).unwrap_or(0) as u64,
                row.get::<_, f64>(1).unwrap_or(0.0),
                row.get::<_, f64>(2).unwrap_or(0.0),
            ))
        })?;
        Ok(stats)
    }

    pub fn get_songs_by_album(&self, album_name: &str) -> Result<Vec<SongRecord>> {
        let mut stmt = self.conn.prepare("
            SELECT FULL_FILE_PATH, TITLE, ARTIST, ALBUM, RELEASE_YEAR, TRACK_NUMBER, FORMAT, SIZE, SAMPLE_RATE, CHANNELS, DURATION_SECS, GENRE, BIT_DEPTH 
            FROM MUSIC_LIBRARY WHERE ALBUM = ?1 ORDER BY TRACK_NUMBER
        ")?;
        let rows = stmt.query_map([album_name], |row| {
            let mut record = SongRecord::default();
            record.full_file_path = row.get(0).unwrap_or_default();
            record.title = row.get(1).ok();
            record.artist = row.get(2).ok();
            record.album = row.get(3).ok();
            record.release_year = row.get(4).ok();
            record.track_number = row.get(5).ok();
            record.format = row.get(6).ok();
            record.size = row.get(7).ok();
            record.sample_rate = row.get(8).ok();
            record.channels = row.get(9).ok();
            record.duration_secs = row.get(10).ok();
            record.genre = row.get(11).ok();
            record.bit_depth = row.get(12).ok();
            Ok(record)
        })?;
        let mut songs = Vec::new();
        for r in rows {
            if let Ok(s) = r { songs.push(s); }
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
    
    pub fn get_library_stats(&self) -> Result<(usize, usize, f64, f64)> {
        let mut stmt = self.conn.prepare("
            SELECT 
                COUNT(FULL_FILE_PATH) as total_songs,
                COUNT(DISTINCT ALBUM) as total_albums,
                SUM(COALESCE(DURATION_SECS, 0.0)) as total_duration,
                SUM(COALESCE(SIZE, 0)) as total_size
            FROM MUSIC_LIBRARY
        ")?;
        
        // SQLite will return integers as i64 and reals as f64 generally in the sums
        let mut total_songs = 0;
        let mut total_albums = 0;
        let mut total_duration = 0.0;
        let mut total_size = 0.0;
        
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            total_songs = row.get::<_, i64>(0).unwrap_or(0) as usize;
            total_albums = row.get::<_, i64>(1).unwrap_or(0) as usize;
            total_duration = row.get::<_, f64>(2).unwrap_or(0.0);
            total_size = row.get::<_, f64>(3).unwrap_or(0.0);
        }
        
        Ok((total_songs, total_albums, total_duration, total_size))
    }
}

#[derive(Debug, Default, Clone)]
pub struct SongRecord {
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
    
    pub track_number: Option<u32>,
    pub total_tracks: Option<u32>,
    pub disc_number: Option<u32>,
    pub total_discs: Option<u32>,
    
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub release_year: Option<String>,
    pub album_artist: Option<String>,
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
}
