//! Escáner de biblioteca musical que recorre directorios, extrae metadatos
//! con `lofty` y almacena la información en la base de datos SQLite.

use lofty::prelude::{AudioFile, ItemKey, TaggedFileExt};
use lofty::probe::Probe;
use std::path::Path;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use walkdir::WalkDir;

use crate::db::database::{Database, SongData, SongMetadataExtended};

/// Escáner de la biblioteca musical.
///
/// Recorre directorios del sistema de archivos, extrae metadatos de audio
/// mediante `lofty` con fusión inteligente de tags (priorizando ID3v2,
/// VorbisComments, APE según el formato), y los almacena en la base de datos.
/// También encola el procesamiento de carátulas embebidas.
pub struct Scanner {
    _db: Arc<Mutex<Database>>,
    pub db_dirty: Arc<AtomicBool>,
    pub is_scanning: Arc<AtomicBool>,
}

impl Scanner {
    /// Crea un nuevo escáner con la referencia a la base de datos.
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self {
            _db: db,
            db_dirty: Arc::new(AtomicBool::new(false)),
            is_scanning: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Escanea un directorio en un hilo separado de forma asíncrona.
    pub fn scan_folder_async(&self, folder_path: String) {
        self.scan_folder_async_with(folder_path, |_| {});
    }

    /// Scans a directory on a separate thread and invokes `on_complete` with
    /// the number of audio files processed when the walk finishes.
    ///
    /// The callback runs on the scanner thread, after the scanning flag is
    /// cleared, so an external observer receives the scan-finished edge without
    /// polling the flag from the UI thread.
    pub fn scan_folder_async_with<C>(&self, folder_path: String, on_complete: C)
    where
        C: FnOnce(usize) + Send + 'static,
    {
        let db_arc = Arc::clone(&self._db);
        let dirty_flag = Arc::clone(&self.db_dirty);
        let is_scanning_flag = Arc::clone(&self.is_scanning);
        std::thread::spawn(move || {
            is_scanning_flag.store(true, Ordering::SeqCst);
            let processed = Self::scan_folder(&db_arc, &folder_path, &dirty_flag);
            is_scanning_flag.store(false, Ordering::SeqCst);
            on_complete(processed);
        });
    }

    fn scan_folder(db_m: &Arc<Mutex<Database>>, root: &str, dirty_flag: &Arc<AtomicBool>) -> usize {
        let supported_extensions = [
            "mp3", "flac", "wav", "ogg", "m4a", "aac", "ape", "aiff", "mpc", "opus", "spx", "wv",
        ];

        let start_order = {
            if let Ok(db) = db_m.lock() {
                db.get_max_import_order().unwrap_or(0) + 1
            } else {
                1
            }
        };

        let mut current_order = start_order;
        let mut enqueued_covers = std::collections::HashSet::new();

        if let Ok(db) = db_m.lock()
            && let Err(e) = db.begin_transaction()
        {
            tracing::warn!("No se pudo iniciar la transacción del escáner: {}", e);
        }

        // Start boundary of the negative cover set: an entry recorded by an
        // earlier scan must not suppress a cover this walk is about to write.
        // The clear belongs here, outside the walk, so a batch that commits new
        // rows cannot re-enable a retry for every cover the grid asked for
        // before the scanner had written it. The matching finish boundary is the
        // post-scan cleanup tick, which clears the set once the scan settles.
        crate::utils::covers::clear_negative_cover_cache();

        for (count, entry) in WalkDir::new(root)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|e| e.ok())
            .enumerate()
        {
            let path = entry.path();
            if path.is_file()
                && let Some(ext) = path.extension().and_then(|e| e.to_str())
                && supported_extensions.contains(&ext.to_lowercase().as_str())
            {
                Self::process_file(db_m, path, root, current_order, &mut enqueued_covers, false);
                current_order += 1;

                if count % 500 == 0 {
                    if let Ok(db) = db_m.lock() {
                        if let Err(e) = db.commit_transaction() {
                            tracing::warn!(
                                "No se pudo confirmar la transacción del escáner: {}",
                                e
                            );
                        }
                        if let Err(e) = db.begin_transaction() {
                            tracing::warn!("No se pudo iniciar la transacción del escáner: {}", e);
                        }
                    }
                    dirty_flag.store(true, Ordering::Relaxed);
                }
            }
        }

        if let Ok(db) = db_m.lock() {
            if let Err(e) = db.commit_transaction() {
                tracing::warn!("No se pudo confirmar la transacción del escáner: {}", e);
            }
            db.flush_prepared_statements();
            // Statistics are refreshed once the bulk writes settle. The mask is
            // required on this connection: its planner never runs the library
            // queries, so an unmasked call would stop analyzing after the first
            // scan.
            if let Err(e) = db.optimize_query_planner(Some(0x10002)) {
                tracing::debug!(
                    "No se pudo optimizar el planificador tras el escaneo: {}",
                    e
                );
            }
        }

        dirty_flag.store(true, Ordering::Relaxed);
        (current_order - start_order).max(0) as usize
    }

    /// Procesa un archivo de audio individual: extrae metadatos y lo inserta en la BD.
    ///
    /// Usa `lofty` para leer tags con fusión por prioridad según formato,
    /// extrae carátulas embebidas y encola su procesamiento AVIF.
    pub fn process_file(
        db_m: &Arc<Mutex<Database>>,
        path: &Path,
        _root: &str,
        import_order: i64,
        enqueued_covers: &mut std::collections::HashSet<String>,
        is_external: bool,
    ) {
        let mut song = SongData::default();
        let mut extended = SongMetadataExtended::default();

        song.import_order = import_order;
        song.full_file_path = crate::utils::interner::intern_string(&path.to_string_lossy());

        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            song.format =
                crate::utils::interner::intern_string_opt(Some(ext.to_uppercase().as_str()));
        }

        if let Ok(metadata) = std::fs::metadata(path) {
            song.size = Some(metadata.len() as i64);
        }

        let mut pic_bytes = None;

        // Mapa para la fusión (prioridad). Almacenamos el valor de mayor prioridad para cada clave técnica.
        // Prioridad: Id3v2 > VorbisComments > Mp4ilst > Ape > RiffInfo > Id3v1
        let mut fused_map = std::collections::HashMap::new();

        if let Ok(probe) = Probe::open(path)
            && let Ok(tagged_file) = probe
                .guess_file_type()
                // A failed guess consumes the probe, so the fallback has to reopen.
                // `or_else` keeps that reopen on the failure path only: `unwrap_or`
                // would evaluate it eagerly, opening every scanned file twice, and
                // its `unwrap` would abort the process when a file disappears
                // between the two opens.
                .or_else(|_| Probe::open(path))
                .and_then(|probe| probe.read())
        {
            let props = tagged_file.properties();
            song.sample_rate = props.sample_rate().map(|sample_rate| sample_rate as i64);
            song.channels = props.channels().map(|ch| ch as i64);
            song.duration_secs = Some(props.duration().as_secs_f64());
            song.bit_depth = props.bit_depth().map(|b| b as i64);

            // 1. Recolectar TODAS las etiquetas presentes
            for tag in tagged_file.tags() {
                let tag_type = tag.tag_type();

                // Nivel de prioridad dependiente del formato (Menor es más prioritario)
                let file_type = tagged_file.file_type();
                let priority = match (file_type, tag_type) {
                    // FLAC: VorbisComments tiene la prioridad absoluta (ID3v2 es solo lectura)
                    (lofty::file::FileType::Flac, lofty::tag::TagType::VorbisComments) => 1,

                    // APE/MPC: APE tiene la prioridad absoluta (ID3v2/v1 son secundarios o solo lectura)
                    (
                        lofty::file::FileType::Ape | lofty::file::FileType::Mpc,
                        lofty::tag::TagType::Ape,
                    ) => 1,

                    // WAV/AIFF: ID3v2 tiene prioridad sobre RIFF INFO/Text Chunks
                    (
                        lofty::file::FileType::Wav | lofty::file::FileType::Aiff,
                        lofty::tag::TagType::Id3v2,
                    ) => 1,
                    (lofty::file::FileType::Wav, lofty::tag::TagType::RiffInfo) => 2,

                    // Prioridades globales (Mantenidas para MP3, AAC, etc.)
                    (_, lofty::tag::TagType::Id3v2) => 1,
                    (_, lofty::tag::TagType::VorbisComments) => 2,
                    (_, lofty::tag::TagType::Mp4Ilst) => 3,
                    (_, lofty::tag::TagType::Ape) => 4,
                    (_, lofty::tag::TagType::RiffInfo) => 5,
                    (_, lofty::tag::TagType::Id3v1) => 6,
                    _ => 10,
                };

                for item in tag.items() {
                    let key = item.key();
                    if let lofty::tag::ItemValue::Text(val) = item.value() {
                        // Lógica de Fusión: Solo actualizamos si no existe o si el nuevo tag tiene más prioridad
                        let entry = fused_map.entry(key).or_insert((priority, val.clone()));
                        if priority < entry.0 {
                            *entry = (priority, val.clone());
                        }
                    }
                }

                // Intentar extraer carátula del tag más prioritario que la tenga
                if pic_bytes.is_none()
                    && let Some(pic) = tag.pictures().first()
                {
                    pic_bytes = Some(pic.data().to_vec());
                }
            }

            // 2. Poblar SongData con los valores fusionados (Alta Fidelidad)
            let get_fused = |key: ItemKey| fused_map.get(&key).map(|(_, v)| v.clone());

            song.title = crate::utils::interner::intern_string_opt(
                get_fused(ItemKey::TrackTitle).as_deref(),
            );
            song.artist = crate::utils::interner::intern_string_opt(
                get_fused(ItemKey::TrackArtist).as_deref(),
            );
            song.album = crate::utils::interner::intern_string_opt(
                get_fused(ItemKey::AlbumTitle).as_deref(),
            );
            song.genre =
                crate::utils::interner::intern_string_opt(get_fused(ItemKey::Genre).as_deref());
            song.track_number = crate::utils::interner::intern_string_opt(
                get_fused(ItemKey::TrackNumber).as_deref(),
            );
            song.album_artist = crate::utils::interner::intern_string_opt(
                get_fused(ItemKey::AlbumArtist).as_deref(),
            );

            // Año con lógica de fallback robusta pero preservando formato
            song.release_year = crate::utils::interner::intern_string_opt(
                get_fused(ItemKey::Year)
                    .or_else(|| get_fused(ItemKey::RecordingDate))
                    .or_else(|| get_fused(ItemKey::OriginalReleaseDate))
                    .map(|d| {
                        d.chars()
                            .filter(|c| c.is_ascii_digit())
                            .take(4)
                            .collect::<String>()
                    })
                    .as_deref(),
            );

            extended.lyrics =
                get_fused(ItemKey::UnsyncLyrics).or_else(|| get_fused(ItemKey::Lyrics));
            extended.comments = get_fused(ItemKey::Comment);
            extended.composer = get_fused(ItemKey::Composer);
            extended.lyricist = get_fused(ItemKey::Lyricist);
            extended.publisher = get_fused(ItemKey::Publisher);
            extended.copyright = get_fused(ItemKey::CopyrightMessage);
            extended.encoded_by = get_fused(ItemKey::EncodedBy);
            extended.catalog = get_fused(ItemKey::CatalogNumber);
            extended.isrc = get_fused(ItemKey::Isrc);
            extended.key = get_fused(ItemKey::InitialKey);
            extended.bpm = get_fused(ItemKey::Bpm);

            extended.track_gain = get_fused(ItemKey::ReplayGainTrackGain)
                .and_then(|s| s.replace(" dB", "").parse::<f64>().ok());
            extended.album_gain = get_fused(ItemKey::ReplayGainAlbumGain)
                .and_then(|s| s.replace(" dB", "").parse::<f64>().ok());
        }

        let pic_hash = pic_bytes
            .as_ref()
            .map(|b| Database::generate_hash(&hex::encode(b)));

        // Resolve the cover write under the database lock, then hand it off after
        // releasing it. The gateway is bounded, so a blocked send under the lock
        // would stall every reader of the database — the GUI included — until an
        // AVIF encode finished, and the existence probe is filesystem I/O that has
        // no business inside the lock either.
        let pending_cover = match db_m.lock() {
            Ok(mut db) => match db.insert_song_full(&song, &extended, pic_hash, is_external) {
                Ok((target_file_name, needs_processing))
                    if needs_processing && !enqueued_covers.contains(&target_file_name) =>
                {
                    Some(target_file_name)
                }
                _ => None,
            },
            Err(_) => None,
        };

        if let (Some(target_file_name), Some(data)) = (pending_cover, pic_bytes) {
            // Only enqueue when the AVIF file does not exist yet. The
            // name arrives complete from the insert, so the existence
            // check and the write cannot disagree.
            let expected_path = crate::utils::paths::get()
                .cover_cache_dir
                .join(&target_file_name);
            if !expected_path.exists() {
                crate::utils::covers::enqueue_cover_job(data, target_file_name.clone());
                enqueued_covers.insert(target_file_name);
            }
        }
    }

    /// Importa un archivo M3U/M3U8 como una nueva playlist.
    ///
    /// Las canciones que no existen en la BD se procesan como externas.
    pub fn import_m3u(db_m: &Arc<Mutex<Database>>, m3u_path: &str) -> Option<i64> {
        use std::io::BufRead;
        let path = Path::new(m3u_path);
        let playlist_name = path.file_stem()?.to_string_lossy().to_string();

        let file = std::fs::File::open(path).ok()?;
        let reader = std::io::BufReader::new(file);

        let mut paths = Vec::new();
        for line in reader.lines() {
            let Ok(line) = line else { continue };
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Manejar rutas relativas respecto a la ubicación del .m3u
            let song_path = if Path::new(line).is_absolute() {
                Path::new(line).to_path_buf()
            } else {
                path.parent()?.join(line)
            };

            if song_path.exists() {
                paths.push(song_path);
            }
        }

        if paths.is_empty() {
            return None;
        }

        let playlist_id = {
            let mut db = db_m.lock().ok()?;
            if let Err(e) = db.begin_transaction() {
                tracing::warn!("No se pudo iniciar la transacción del escáner: {}", e);
            }
            let id = db.create_playlist(&playlist_name, false).ok()?;

            let mut enqueued_covers = std::collections::HashSet::new();
            for p in paths.iter() {
                let p_str = p.to_string_lossy().to_string();

                // 1. Verificar si la canción ya existe (para no duplicar)
                let song_id = if let Ok(Some(sid)) = db.get_song_id_by_path(&p_str) {
                    sid
                } else {
                    // 2. Si no existe, procesarla como EXTERNA (is_external = true)
                    // Soltamos el lock momentáneamente si process_file lo requiere (aunque aquí ya lo tenemos)
                    // Pero process_file pide Arc<Mutex<Database>>, así que necesitamos soltarlo o reestructurar.
                    // Para evitar Deadlock, usaremos un método interno que no pida el lock.
                    drop(db);
                    Self::process_file(db_m, p, "", 0, &mut enqueued_covers, true);
                    db = db_m.lock().ok()?;
                    db.get_song_id_by_path(&p_str).ok().flatten()?
                };

                if let Err(e) = db.add_song_to_playlist(id, song_id) {
                    tracing::warn!(
                        "No se pudo añadir la canción {} a la playlist {}: {}",
                        song_id,
                        id,
                        e
                    );
                }
            }

            if let Err(e) = db.commit_transaction() {
                tracing::warn!("No se pudo confirmar la transacción del escáner: {}", e);
            }
            // The import writes in bulk through the scanner connection too, so it
            // refreshes the statistics once its transaction commits.
            if let Err(e) = db.optimize_query_planner(Some(0x10002)) {
                tracing::debug!(
                    "No se pudo optimizar el planificador tras importar el M3U: {}",
                    e
                );
            }
            id
        };

        Some(playlist_id)
    }

    /// Procesa un lote de archivos externos y los añade a una playlist.
    ///
    /// Soporta archivos de audio individuales, directorios (recursivo) y M3U.
    pub fn process_external_batch(
        db_m: &Arc<Mutex<Database>>,
        paths: Vec<std::path::PathBuf>,
        playlist_id: i64,
    ) {
        let mut enqueued_covers = std::collections::HashSet::new();
        let supported_extensions = [
            "mp3", "flac", "wav", "ogg", "m4a", "aac", "ape", "aiff", "mpc", "opus", "spx", "wv",
        ];

        if let Ok(db) = db_m.lock()
            && let Err(e) = db.begin_transaction()
        {
            tracing::warn!("No se pudo iniciar la transacción del escáner: {}", e);
        }

        for path in paths {
            if path.is_file() {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if supported_extensions.contains(&ext.as_str()) {
                    Self::process_file(db_m, &path, "", 0, &mut enqueued_covers, true);

                    // Añadir a la playlist
                    if let Ok(db) = db_m.lock() {
                        let path_str = path.to_string_lossy();
                        if let Ok(Some(song_id)) = db.get_song_id_by_path(&path_str)
                            && let Err(e) = db.add_song_to_playlist(playlist_id, song_id)
                        {
                            tracing::warn!(
                                "No se pudo añadir la canción {} a la playlist {}: {}",
                                song_id,
                                playlist_id,
                                e
                            );
                        }
                    }
                } else if ext == "m3u" || ext == "m3u8" {
                    // El import_m3u ya hace su propia transacción y añade a una NUEVA playlist.
                    // Para "Agregar archivo", el usuario podría querer anexar a la ACTUAL.
                    // Pero por ahora seguiremos la lógica de import_m3u o la adaptaremos.
                    Self::import_m3u(db_m, &path.to_string_lossy());
                }
            } else if path.is_dir() {
                // Si es un directorio, escaneamos recursivamente
                for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file() {
                        let ext = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        if supported_extensions.contains(&ext.as_str()) {
                            Self::process_file(db_m, path, "", 0, &mut enqueued_covers, true);
                            if let Ok(db) = db_m.lock() {
                                let path_str = path.to_string_lossy();
                                if let Ok(Some(song_id)) = db.get_song_id_by_path(&path_str)
                                    && let Err(e) = db.add_song_to_playlist(playlist_id, song_id)
                                {
                                    tracing::warn!(
                                        "No se pudo añadir la canción {} a la playlist {}: {}",
                                        song_id,
                                        playlist_id,
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Ok(db) = db_m.lock() {
            if let Err(e) = db.commit_transaction() {
                tracing::warn!("No se pudo confirmar la transacción del escáner: {}", e);
            }
            db.flush_prepared_statements();
        }
    }
}

#[cfg(test)]
#[path = "scanner_tests.rs"]
mod scanner_tests;
