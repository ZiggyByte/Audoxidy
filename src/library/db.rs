use rusqlite::{params, Connection, Result};
use crate::library::metadata::LibrarySong;
use std::path::Path;
use parking_lot::Mutex;

pub struct MediaLibrary {
    conn: Mutex<Connection>,
}

impl MediaLibrary {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let library = Self { conn: Mutex::new(conn) };
        library.init_tables()?;
        Ok(library)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS songs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                artist TEXT NOT NULL,
                album TEXT NOT NULL,
                genre TEXT NOT NULL,
                year INTEGER,
                duration_sec INTEGER NOT NULL,
                path TEXT NOT NULL UNIQUE,
                track_number INTEGER,
                disc_number INTEGER
            )",
            [],
        )?;
        Ok(())
    }

    pub fn add_song(&self, song: &LibrarySong) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO songs (title, artist, album, genre, year, duration_sec, path, track_number, disc_number)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                song.title,
                song.artist,
                song.album,
                song.genre,
                song.year,
                song.duration_sec,
                song.path,
                song.track_number,
                song.disc_number,
            ],
        )?;
        Ok(())
    }

    pub fn get_all_songs(&self) -> Result<Vec<LibrarySong>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT id, title, artist, album, genre, year, duration_sec, path, track_number, disc_number FROM songs")?;
        let song_iter = stmt.query_map([], |row| {
            Ok(LibrarySong {
                id: Some(row.get(0)?),
                title: row.get(1)?,
                artist: row.get(2)?,
                album: row.get(3)?,
                genre: row.get(4)?,
                year: row.get(5)?,
                duration_sec: row.get(6)?,
                path: row.get(7)?,
                track_number: row.get(8)?,
                disc_number: row.get(9)?,
            })
        })?;

        let mut songs = Vec::new();
        for song in song_iter {
            songs.push(song?);
        }
        Ok(songs)
    }

    #[allow(dead_code)]
    pub fn clear(&self) -> Result<()> {
        self.conn.lock().execute("DELETE FROM songs", [])?;
        Ok(())
    }
}
