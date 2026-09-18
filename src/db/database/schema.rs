//! Schema creation and seed data for the SQLite library database.

use rusqlite::{Connection, Result, params};

use super::Database;

impl Database {
    pub(in crate::db::database) fn create_schema(conn: &Connection) -> Result<()> {
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

        // Tabla de presets de ecualizador personalizados
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
}
