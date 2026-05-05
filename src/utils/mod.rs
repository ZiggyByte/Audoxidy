// Utilidades comunes y funciones matemáticas de formateo
pub mod covers;
pub mod interner;
pub mod memory_manager;
pub mod memory_tests;
use crate::db::database::SongData;

pub fn song_matches_search(song: &SongData, query_lowercase: &str) -> bool {
    song.title.as_ref().map(|t| t.to_lowercase().contains(query_lowercase)).unwrap_or(false)
    || song.artist.as_ref().map(|a| a.to_lowercase().contains(query_lowercase)).unwrap_or(false)
    || song.album_artist.as_ref().map(|aa| aa.to_lowercase().contains(query_lowercase)).unwrap_or(false)
    || song.album.as_ref().map(|al| al.to_lowercase().contains(query_lowercase)).unwrap_or(false)
}

/// Flag global de modo de bajos recursos (RAM < 8GB o Cores < 4)
/// Se configura una sola vez al iniciar la aplicación.
pub static LOW_RESOURCE_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn is_low_resource() -> bool {
    LOW_RESOURCE_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

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
    AlbumCard,
    AlbumThumbnail, // Fixed 40px column for 30x30 album art in ThumbnailList view
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
            SortColumn::AlbumCard => "Tarjeta de álbum",
            SortColumn::AlbumThumbnail => "",
        }
    }
}

/// Extrae el valor de texto correspondiente a la columna dada desde un SongData.
pub fn format_metadata(song: &SongData, col: &SortColumn) -> String {
    match col {
        SortColumn::TrackNumber => song.track_number.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "-".to_string()),
        SortColumn::Title => song.title.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::Artist => song.artist.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::AlbumArtist => song.album_artist.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::Album => song.album.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::Genre => song.genre.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::Year => song.release_year.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "-".to_string()),
        SortColumn::Duration => {
            let dur_secs = song.duration_secs.unwrap_or(0.0);
            format_duration(dur_secs)
        }
        SortColumn::Format => song.format.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "-".to_string()),
        SortColumn::SampleRate => song.sample_rate.map_or("-".to_string(), |r| format!("{:.1} kHz", r as f64 / 1000.0)),
        SortColumn::Channels => song.channels.map_or("-".to_string(), |c| {
            match c {
                1 => "mono".to_string(),
                2 => "2".to_string(),
                3 => "2.1".to_string(),
                4 => "4.0".to_string(),
                6 => "5.1".to_string(),
                8 => "7.1".to_string(),
                _ => c.to_string(),
            }
        }),
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
        SortColumn::AlbumCard => "".to_string(),
        SortColumn::AlbumThumbnail => "".to_string(),
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

/// Estructura para gestionar la visualización de rutas en la biblioteca de forma inteligente.
pub struct IntelligentPath {
    pub display: String,
    pub is_multi_drive: bool,
}

/// Formatea una ruta física según las reglas de Audoxidy:
/// 1. Prefijo de disco si provienen de diferentes puntos de montaje.
/// 2. Desambiguación de raíces con nombres idénticos (ej. /Descargas/Musica vs /Documentos/Musica).
/// 3. Abreviación "4-atrás" para rutas profundas en paneles estrechos.
pub fn format_intelligent_path(
    full_path: &str, 
    all_roots: &[String], 
    mount_points: &[(String, String)]
) -> String {
    // A. Detectar Disco / Punto de Montaje
    let mut drive_prefix = String::new();
    if mount_points.len() > 1 {
        let mut best_mnt = "/";
        for (mnt, _) in mount_points {
            if full_path.starts_with(mnt) && mnt.len() > best_mnt.len() {
                best_mnt = mnt;
            }
        }
        if best_mnt != "/" {
            let mnt_name = best_mnt.trim_start_matches("/mnt/").trim_start_matches("/media/");
            drive_prefix = format!("[{}] ", if mnt_name.is_empty() { "/" } else { mnt_name });
        } else {
            drive_prefix = "[/] ".to_string();
        }
    }

    // B. Identificar Raíz y Desambiguar
    let mut root_part = String::new();
    let mut selected_root = "";
    for r in all_roots {
        if full_path.starts_with(r) && r.len() > selected_root.len() {
            selected_root = r;
        }
    }

    if !selected_root.is_empty() {
        let root_path = std::path::Path::new(selected_root);
        let root_name = root_path.file_name().and_then(|n| n.to_str()).unwrap_or("Raíz");
        
        // Verificar si este nombre de raíz está duplicado en las rutas añadidas
        let duplicates = all_roots.iter()
            .filter(|r| std::path::Path::new(r).file_name().and_then(|n| n.to_str()) == Some(root_name))
            .count();
            
        if duplicates > 1 {
            // Usar padre + raíz para desambiguar
            let parent_name = root_path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("..");
            root_part = format!("{}/{} ", parent_name, root_name);
        } else {
            root_part = format!("{} ", root_name);
        }
    }

    // C. Algoritmo "4-atrás" para profundidad
    let relative = if !selected_root.is_empty() {
        full_path.strip_prefix(selected_root).unwrap_or(full_path).trim_start_matches('/')
    } else {
        full_path.trim_start_matches('/')
    };

    let components: Vec<&str> = relative.split('/').filter(|s| !s.is_empty()).collect();
    let inner_path = if components.len() > 4 {
        let last_four = components[components.len()-4..].join("/");
        format!("... / {}", last_four)
    } else {
        components.join("/")
    };

    format!("{}{}{}", drive_prefix, root_part, inner_path).trim().to_string()
}

/// Obtiene el nombre del artista que debe usarse para ordenar y agrupar (prioriza Artista del Álbum).
pub fn get_effective_artist(song: &SongData) -> &str {
    song.album_artist.as_deref()
        .or(song.artist.as_deref())
        .unwrap_or("Artista Desconocido")
}

/// Compara dos cadenas de texto ignorando mayúsculas y minúsculas para un ordenamiento musical natural.
pub fn compare_strings_ignore_case(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

/// Función central de ordenamiento canónico para Audoxidy: Artista (Álbum) -> Año -> Álbum -> Número de Pista.
pub fn compare_songs_for_listing(a: &SongData, b: &SongData) -> std::cmp::Ordering {
    let art_a = get_effective_artist(a);
    let art_b = get_effective_artist(b);
    
    compare_strings_ignore_case(art_a, art_b)
        .then(compare_strings_ignore_case(a.release_year.as_deref().unwrap_or(""), b.release_year.as_deref().unwrap_or("")))
        .then(compare_strings_ignore_case(a.album.as_deref().unwrap_or(""), b.album.as_deref().unwrap_or("")))
        .then({
            let tn_a = a.track_number.as_ref().and_then(|t| t.parse::<u32>().ok());
            let tn_b = b.track_number.as_ref().and_then(|t| t.parse::<u32>().ok());
            match (tn_a, tn_b) {
                (Some(na), Some(nb)) => na.cmp(&nb),
                _ => a.track_number.cmp(&b.track_number),
            }
        })
}

/// Función central de ordenamiento para álbumes (Grid/Listas): Artista -> Año -> Título.
pub fn compare_albums_for_listing(
    art_a: &str, year_a: &str, title_a: &str,
    art_b: &str, year_b: &str, title_b: &str
) -> std::cmp::Ordering {
    compare_strings_ignore_case(art_a, art_b)
        .then(compare_strings_ignore_case(year_a, year_b))
        .then(compare_strings_ignore_case(title_a, title_b))
}

