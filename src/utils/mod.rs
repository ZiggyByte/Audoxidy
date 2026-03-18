// Utilidades comunes y funciones matemáticas de formateo
use crate::db::database::SongData;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortColumn {
    TrackNumber,
    Title,
    Artist,
    AlbumArtist,
    Album,
    Genre,
    Year,
    Duration,
    Format,
    SampleRate,
    Channels,
    BitDepth,
    Bitrate,
    Size,
}

impl SortColumn {
    pub fn as_str(&self) -> &'static str {
        match self {
            SortColumn::TrackNumber => "#",
            SortColumn::Title => "Título",
            SortColumn::Artist => "Artista",
            SortColumn::AlbumArtist => "Artista del Álbum",
            SortColumn::Album => "Álbum",
            SortColumn::Genre => "Género",
            SortColumn::Year => "Año",
            SortColumn::Duration => "Duración",
            SortColumn::Format => "Formato",
            SortColumn::SampleRate => "Muestreo",
            SortColumn::Channels => "Canales",
            SortColumn::BitDepth => "Profundidad",
            SortColumn::Bitrate => "Bits",
            SortColumn::Size => "Tamaño",
        }
    }
}

/// Extrae el valor de texto correspondiente a la columna dada desde un SongData.
pub fn format_metadata(song: &SongData, col: &SortColumn) -> String {
    match col {
        SortColumn::TrackNumber => song.track_number.clone().unwrap_or_else(|| "-".to_string()),
        SortColumn::Title => song.title.clone().unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::Artist => song.artist.clone().unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::AlbumArtist => song.album_artist.clone().unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::Album => song.album.clone().unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::Genre => song.genre.clone().unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::Year => song.release_year.clone().unwrap_or_else(|| "-".to_string()),
        SortColumn::Duration => {
            let dur_secs = song.duration_secs.unwrap_or(0.0);
            format_duration(dur_secs)
        }
        SortColumn::Format => song.format.clone().unwrap_or_else(|| "-".to_string()),
        SortColumn::SampleRate => song.sample_rate.map_or("-".to_string(), |r| format!("{:.1} kHz", r as f64 / 1000.0)),
        SortColumn::Channels => song.channels.map_or("-".to_string(), |c| c.to_string()),
        SortColumn::BitDepth => song.bit_depth.map_or("-".to_string(), |b| format!("{} bits", b)),
        SortColumn::Bitrate => {
            if let (Some(s), Some(d)) = (song.size, song.duration_secs) {
                if d > 0.0 {
                    format!("{} kbps", ((s as f64 * 8.0) / (d * 1000.0)) as u32)
                } else {
                    "-".to_string()
                }
            } else {
                "-".to_string()
            }
        }
        SortColumn::Size => song.size.map_or("-".to_string(), |s| format_size(s as i64)),
    }
}

/// Recorta de manera segura un String basado en su conteo de caracteres visuales.
pub fn truncate_text(text: &str, limit: usize) -> String {
    if text.chars().count() > limit {
        format!("{}...", text.chars().take(limit.saturating_sub(3)).collect::<String>())
    } else {
        text.to_string()
    }
}

/// Convierte segundos totales (f64 o u64) en una cadena de tiempo amigable.
/// Muestra "MM:SS", o "HH:MM:SS" si dura más de una hora, e incluso días si es necesario.
pub fn format_duration(seconds: f64) -> String {
    let secs = seconds.max(0.0) as u64;
    let days = secs / 86400;
    let rh = secs % 86400;
    let hours = rh / 3600;
    let rm = rh % 3600;
    let minutes = rm / 60;
    let secs_rem = rm % 60;

    if days > 0 {
        format!("{:02}:{:02}:{:02}:{:02}", days, hours, minutes, secs_rem)
    } else if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs_rem)
    } else {
        format!("{:02}:{:02}", minutes, secs_rem)
    }
}

/// Convierte bytes en formato MB o GB según su peso.
pub fn format_size(bytes: i64) -> String {
    if bytes <= 0 {
        return "0.00 MB".to_string();
    }
    let size_mb = bytes as f64 / 1_048_576.0;
    if size_mb >= 1024.0 {
        format!("{:.2} GB", size_mb / 1024.0)
    } else {
        format!("{:.2} MB", size_mb)
    }
}
