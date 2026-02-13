use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrarySong {
    pub id: Option<i64>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: Option<u32>,
    pub duration_sec: i64,
    pub path: String,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
}

impl Default for LibrarySong {
    fn default() -> Self {
        Self {
            id: None,
            title: "Sin título".to_string(),
            artist: "Artista desconocido".to_string(),
            album: "Álbum desconocido".to_string(),
            genre: "Desconocido".to_string(),
            year: None,
            duration_sec: 0,
            path: String::new(),
            track_number: None,
            disc_number: None,
        }
    }
}
