//! Utilidades comunes para Audoxidy: formateo, ordenamiento, búsqueda,
//! gestión de memoria, interner de cadenas, carátulas y configuración RON.

pub mod config;
pub mod covers;
pub mod interner;
pub mod memory_manager;
pub mod observability;
use crate::db::database::SongData;

/// Busca una canción verificando si el término de búsqueda aparece en
/// título, artista, artista del álbum o álbum.
///
/// `query_lowercase` debe estar en minúsculas; los campos de la canción
/// se convierten internamente para la comparación.
pub fn song_matches_search(song: &SongData, query_lowercase: &str) -> bool {
    song.title
        .as_ref()
        .map(|t| t.to_lowercase().contains(query_lowercase))
        .unwrap_or(false)
        || song
            .artist
            .as_ref()
            .map(|a| a.to_lowercase().contains(query_lowercase))
            .unwrap_or(false)
        || song
            .album_artist
            .as_ref()
            .map(|aa| aa.to_lowercase().contains(query_lowercase))
            .unwrap_or(false)
        || song
            .album
            .as_ref()
            .map(|al| al.to_lowercase().contains(query_lowercase))
            .unwrap_or(false)
}

/// Flag global de modo de bajos recursos (RAM < 8GB o Cores < 4)
/// Se configura una sola vez al iniciar la aplicación.
pub static LOW_RESOURCE_MODE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Devuelve `true` si el modo de bajos recursos está activo.
pub fn is_low_resource() -> bool {
    LOW_RESOURCE_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Columnas de ordenamiento disponibles en la biblioteca y listas.
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
    /// Devuelve la etiqueta en español para la columna.
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
        SortColumn::TrackNumber => song
            .track_number
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "-".to_string()),
        SortColumn::Title => song
            .title
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::Artist => song
            .artist
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::AlbumArtist => song
            .album_artist
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::Album => song
            .album
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::Genre => song
            .genre
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Desconocido".to_string()),
        SortColumn::Year => song
            .release_year
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "-".to_string()),
        SortColumn::Duration => {
            let dur_secs = song.duration_secs.unwrap_or(0.0);
            format_duration(dur_secs)
        }
        SortColumn::Format => song
            .format
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "-".to_string()),
        SortColumn::SampleRate => song
            .sample_rate
            .map_or("-".to_string(), |r| format!("{:.1} kHz", r as f64 / 1000.0)),
        SortColumn::Channels => song.channels.map_or("-".to_string(), |c| match c {
            1 => "mono".to_string(),
            2 => "2".to_string(),
            3 => "2.1".to_string(),
            4 => "4.0".to_string(),
            6 => "5.1".to_string(),
            8 => "7.1".to_string(),
            _ => c.to_string(),
        }),
        SortColumn::BitDepth => song
            .bit_depth
            .map_or("-".to_string(), |b| format!("{} bits", b)),
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

/// Recorta un string basándose en su conteo de caracteres Unicode.
///
/// Si el texto excede el límite, se trunca y se añade "..." al final.
pub fn truncate_text(text: &str, limit: usize) -> String {
    if text.chars().count() > limit {
        format!(
            "{}...",
            text.chars()
                .take(limit.saturating_sub(3))
                .collect::<String>()
        )
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

/// Representación visual de una ruta con soporte multi-disco.
pub struct IntelligentPath {
    /// Ruta formateada para mostrar al usuario.
    pub display: String,
    /// `true` si hay múltiples puntos de montaje activos.
    pub is_multi_drive: bool,
}

/// Formatea una ruta física según las reglas de Audoxidy:
/// 1. Prefijo de disco si provienen de diferentes puntos de montaje.
/// 2. Desambiguación de raíces con nombres idénticos (ej. /Descargas/Musica vs /Documentos/Musica).
/// 3. Abreviación "4-atrás" para rutas profundas en paneles estrechos.
pub fn format_intelligent_path(
    full_path: &str,
    all_roots: &[String],
    mount_points: &[(String, String)],
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
            let mnt_name = best_mnt
                .trim_start_matches("/mnt/")
                .trim_start_matches("/media/");
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
        let root_name = root_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Raíz");

        // Verificar si este nombre de raíz está duplicado en las rutas añadidas
        let duplicates = all_roots
            .iter()
            .filter(|r| {
                std::path::Path::new(r).file_name().and_then(|n| n.to_str()) == Some(root_name)
            })
            .count();

        if duplicates > 1 {
            // Usar padre + raíz para desambiguar
            let parent_name = root_path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("..");
            root_part = format!("{}/{} ", parent_name, root_name);
        } else {
            root_part = format!("{} ", root_name);
        }
    }

    // C. Algoritmo "4-atrás" para profundidad
    let relative = if !selected_root.is_empty() {
        full_path
            .strip_prefix(selected_root)
            .unwrap_or(full_path)
            .trim_start_matches('/')
    } else {
        full_path.trim_start_matches('/')
    };

    let components: Vec<&str> = relative.split('/').filter(|s| !s.is_empty()).collect();
    let inner_path = if components.len() > 4 {
        let last_four = components[components.len() - 4..].join("/");
        format!("... / {}", last_four)
    } else {
        components.join("/")
    };

    format!("{}{}{}", drive_prefix, root_part, inner_path)
        .trim()
        .to_string()
}

/// Obtiene el nombre del artista que debe usarse para ordenar y agrupar (prioriza Artista del Álbum).
pub fn get_effective_artist(song: &SongData) -> &str {
    song.album_artist
        .as_deref()
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
        .then(compare_strings_ignore_case(
            a.release_year.as_deref().unwrap_or(""),
            b.release_year.as_deref().unwrap_or(""),
        ))
        .then(compare_strings_ignore_case(
            a.album.as_deref().unwrap_or(""),
            b.album.as_deref().unwrap_or(""),
        ))
        .then(compare_track_numbers(
            a.track_number.as_deref(),
            b.track_number.as_deref(),
        ))
}

/// Compara dos números de pista de forma natural (Alfanumérica).
/// Maneja casos como "1", "10", "2", "A1", "B2", "1/10", etc.
pub fn compare_track_numbers(a: Option<&str>, b: Option<&str>) -> std::cmp::Ordering {
    let a_str = a.unwrap_or("");
    let b_str = b.unwrap_or("");

    if a_str == b_str {
        return std::cmp::Ordering::Equal;
    }
    if a_str.is_empty() {
        return std::cmp::Ordering::Greater;
    }
    if b_str.is_empty() {
        return std::cmp::Ordering::Less;
    }

    // Intentar parseo numérico puro (99% de los casos)
    let a_num = a_str.parse::<u32>();
    let b_num = b_str.parse::<u32>();

    match (a_num, b_num) {
        (Ok(n1), Ok(n2)) => n1.cmp(&n2),
        // Fallback para alfanuméricos (A1, B2, 1/10)
        _ => {
            // Extraer solo la parte numérica si existe
            let get_num = |s: &str| {
                s.chars()
                    .filter(|c| c.is_numeric())
                    .collect::<String>()
                    .parse::<u32>()
                    .ok()
            };
            let n1 = get_num(a_str);
            let n2 = get_num(b_str);

            match (n1, n2) {
                (Some(v1), Some(v2)) if v1 != v2 => v1.cmp(&v2),
                _ => a_str.to_lowercase().cmp(&b_str.to_lowercase()),
            }
        }
    }
}

/// Función central de ordenamiento para álbumes (Grid/Listas): Artista -> Año -> Título.
pub fn compare_albums_for_listing(
    art_a: &str,
    year_a: &str,
    title_a: &str,
    art_b: &str,
    year_b: &str,
    title_b: &str,
) -> std::cmp::Ordering {
    compare_strings_ignore_case(art_a, art_b)
        .then(compare_strings_ignore_case(year_a, year_b))
        .then(compare_strings_ignore_case(title_a, title_b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_test_song() -> SongData {
        SongData {
            id: 1,
            folder_id: 1,
            artist_id: 1,
            album_id: 1,
            full_file_path: Arc::from("/music/test.flac"),
            title: Some(Arc::from("Test Title")),
            artist: Some(Arc::from("Test Artist")),
            album: Some(Arc::from("Test Album")),
            album_artist: Some(Arc::from("Test Album Artist")),
            genre: Some(Arc::from("Rock")),
            release_year: Some(Arc::from("2024")),
            track_number: Some(Arc::from("1")),
            duration_secs: Some(180.0),
            format: Some(Arc::from("FLAC")),
            bit_depth: Some(24),
            sample_rate: Some(96000),
            size: Some(50_000_000),
            channels: Some(2),
            ..Default::default()
        }
    }

    fn make_test_song2() -> SongData {
        SongData {
            id: 2,
            full_file_path: Arc::from("/music/test2.mp3"),
            title: Some(Arc::from("Another Title")),
            artist: Some(Arc::from("Another Artist")),
            album: Some(Arc::from("Another Album")),
            album_artist: Some(Arc::from("Another Album Artist")),
            release_year: Some(Arc::from("2023")),
            track_number: Some(Arc::from("2")),
            duration_secs: Some(240.0),
            ..Default::default()
        }
    }

    // ── format_duration ──

    #[test]
    fn test_format_duration_zero() {
        assert_eq!(format_duration(0.0), "00:00");
    }

    #[test]
    fn test_format_duration_one_min_five_sec() {
        assert_eq!(format_duration(65.0), "01:05");
    }

    #[test]
    fn test_format_duration_one_hour_one_min_one_sec() {
        assert_eq!(format_duration(3661.0), "01:01:01");
    }

    #[test]
    fn test_format_duration_one_day_one_hour_one_min_one_sec() {
        assert_eq!(format_duration(90061.0), "01:01:01:01");
    }

    #[test]
    fn test_format_duration_negative_clamps_to_zero() {
        assert_eq!(format_duration(-5.0), "00:00");
    }

    #[test]
    fn test_format_duration_one_hour_one_min() {
        assert_eq!(format_duration(3660.0), "01:01:00");
    }

    // ── format_size ──

    #[test]
    fn test_format_size_zero() {
        assert_eq!(format_size(0), "0.00 MB");
    }

    #[test]
    fn test_format_size_negative() {
        assert_eq!(format_size(-1), "0.00 MB");
    }

    #[test]
    fn test_format_size_one_mb() {
        assert_eq!(format_size(1_048_576), "1.00 MB");
    }

    #[test]
    fn test_format_size_one_gb() {
        assert_eq!(format_size(1_073_741_824), "1.00 GB");
    }

    #[test]
    fn test_format_size_512_mb() {
        assert_eq!(format_size(536_870_912), "512.00 MB");
    }

    // ── truncate_text ──

    #[test]
    fn test_truncate_text_within_limit() {
        assert_eq!(truncate_text("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_text_exceeds_limit() {
        assert_eq!(truncate_text("hello world", 8), "hello...");
    }

    #[test]
    fn test_truncate_text_unicode_below_limit() {
        assert_eq!(truncate_text("café", 10), "café");
    }

    #[test]
    fn test_truncate_text_unicode_exceeds_limit() {
        assert_eq!(truncate_text("café", 3), "...");
    }

    #[test]
    fn test_truncate_text_empty() {
        assert_eq!(truncate_text("", 5), "");
    }

    #[test]
    fn test_truncate_text_at_exact_limit() {
        assert_eq!(truncate_text("abc", 3), "abc");
    }

    // ── compare_track_numbers ──

    #[test]
    fn test_compare_track_numbers_both_none() {
        assert_eq!(compare_track_numbers(None, None), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_compare_track_numbers_numeric_less() {
        assert_eq!(
            compare_track_numbers(Some("1"), Some("2")),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_compare_track_numbers_numeric_greater() {
        assert_eq!(
            compare_track_numbers(Some("10"), Some("2")),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_compare_track_numbers_alphanumeric_fallback() {
        assert_eq!(
            compare_track_numbers(Some("A1"), Some("A2")),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_compare_track_numbers_empty_vs_none() {
        // Both unwrap to "", a_str == b_str returns Equal (code behaviour)
        assert_eq!(
            compare_track_numbers(Some(""), None),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn test_compare_track_numbers_fractional() {
        assert_eq!(
            compare_track_numbers(Some("1/10"), Some("2/10")),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_compare_track_numbers_some_vs_none() {
        // Some("1") vs None: None unwraps to "", b_str is empty -> Less (tracks with numbers sort before empties)
        assert_eq!(
            compare_track_numbers(Some("1"), None),
            std::cmp::Ordering::Less
        );
    }

    // ── song_matches_search ──

    #[test]
    fn test_song_matches_search_on_title() {
        let song = make_test_song();
        assert!(song_matches_search(&song, "test title"));
    }

    #[test]
    fn test_song_matches_search_on_artist() {
        let song = make_test_song();
        assert!(song_matches_search(&song, "test artist"));
    }

    #[test]
    fn test_song_matches_search_no_match() {
        let song = make_test_song();
        assert!(!song_matches_search(&song, "zzzznotfound"));
    }

    #[test]
    fn test_song_matches_search_case_insensitive() {
        // query_lowercase must already be lowercased by the caller;
        // song fields (which may be mixed-case) get lowercased inside the function
        let song = make_test_song();
        assert!(song_matches_search(&song, "test title"));
        assert!(song_matches_search(&song, "test artist"));
        assert!(song_matches_search(&song, "test album"));
        assert!(song_matches_search(&song, "test album artist"));
    }

    #[test]
    fn test_song_matches_search_on_album() {
        let song = make_test_song();
        assert!(song_matches_search(&song, "test album"));
    }

    #[test]
    fn test_song_matches_search_on_album_artist() {
        let song = make_test_song();
        assert!(song_matches_search(&song, "test album artist"));
    }

    // ── compare_strings_ignore_case ──

    #[test]
    fn test_compare_strings_ignore_case_less() {
        assert_eq!(
            compare_strings_ignore_case("a", "b"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_compare_strings_ignore_case_greater() {
        assert_eq!(
            compare_strings_ignore_case("B", "a"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_compare_strings_ignore_case_equal() {
        assert_eq!(
            compare_strings_ignore_case("foo", "FOO"),
            std::cmp::Ordering::Equal
        );
    }

    // ── SortColumn::as_str ──

    #[test]
    fn test_sort_column_as_str_non_empty() {
        use SortColumn::*;
        let columns = [
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
        ];
        for col in &columns {
            assert!(
                !col.as_str().is_empty(),
                "SortColumn::{:?} as_str() should not be empty",
                col
            );
        }
    }

    #[test]
    fn test_sort_column_album_thumbnail_as_str_is_empty() {
        assert_eq!(SortColumn::AlbumThumbnail.as_str(), "");
    }

    // ── compare_songs_for_listing ──

    #[test]
    fn test_compare_songs_for_listing_same() {
        let a = make_test_song();
        let b = make_test_song();
        assert_eq!(compare_songs_for_listing(&a, &b), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_compare_songs_for_listing_different_artist() {
        let a = make_test_song();
        let b = make_test_song2();
        // "Test Album Artist" vs "Another Album Artist" -> "test album artist" vs "another album artist"
        // "test album artist" > "another album artist" lexicographically (t > a)
        assert_eq!(
            compare_songs_for_listing(&a, &b),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_compare_songs_for_listing_by_track_number() {
        let mut a = make_test_song();
        let mut b = make_test_song();
        // Same artist/album/year but different track numbers
        a.album_artist = Some(Arc::from("Same Artist"));
        b.album_artist = Some(Arc::from("Same Artist"));
        a.album = Some(Arc::from("Same Album"));
        b.album = Some(Arc::from("Same Album"));
        a.release_year = Some(Arc::from("2024"));
        b.release_year = Some(Arc::from("2024"));
        a.track_number = Some(Arc::from("1"));
        b.track_number = Some(Arc::from("5"));
        assert_eq!(compare_songs_for_listing(&a, &b), std::cmp::Ordering::Less);
    }
}
