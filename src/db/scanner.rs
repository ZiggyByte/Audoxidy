use std::path::Path;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use lofty::probe::Probe;
use lofty::prelude::{TaggedFileExt, AudioFile, ItemKey};
use walkdir::WalkDir;

use crate::db::database::{Database, SongData, SongMetadataExtended};

pub struct Scanner {
    _db: Arc<Mutex<Database>>,
    pub db_dirty: Arc<AtomicBool>,
}

impl Scanner {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self { _db: db, db_dirty: Arc::new(AtomicBool::new(false)) }
    }
    
    pub fn scan_folder_async(&self, folder_path: String) {
        let db_arc = Arc::clone(&self._db);
        let dirty_flag = Arc::clone(&self.db_dirty);
        std::thread::spawn(move || {
            Self::scan_folder(&db_arc, &folder_path, &dirty_flag);
        });
    }

    fn scan_folder(db_m: &Arc<Mutex<Database>>, root: &str, dirty_flag: &Arc<AtomicBool>) {
        let supported_extensions = [
            "mp3", "flac", "wav", "ogg", "m4a", "aac", "ape", "aiff", "mpc", "opus", "spx", "wv"
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
        
        if let Ok(db) = db_m.lock() {
            let _ = db.begin_transaction();
        }

        for (count, entry) in WalkDir::new(root).sort_by_file_name().into_iter().filter_map(|e| e.ok()).enumerate() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if supported_extensions.contains(&ext.to_lowercase().as_str()) {
                        Self::process_file(db_m, path, root, current_order, &mut enqueued_covers);
                        current_order += 1;
                        
                        if count % 500 == 0 {
                            if let Ok(db) = db_m.lock() {
                                let _ = db.commit_transaction();
                                let _ = db.begin_transaction();
                            }
                            dirty_flag.store(true, Ordering::Relaxed);
                        }
                    }
                }
            }
        }
        
        if let Ok(db) = db_m.lock() {
            let _ = db.commit_transaction();
        }

        crate::utils::covers::clear_all_cover_cache();
        dirty_flag.store(true, Ordering::Relaxed);
    }

    fn process_file(db_m: &Arc<Mutex<Database>>, path: &Path, _root: &str, import_order: i64, enqueued_covers: &mut std::collections::HashSet<String>) {
        let mut song = SongData::default();
        let mut extended = SongMetadataExtended::default();
        
        song.import_order = import_order;
        song.full_file_path = path.to_string_lossy().to_string();
        
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            song.format = Some(ext.to_uppercase());
        }
        
        if let Ok(metadata) = std::fs::metadata(path) {
            song.size = Some(metadata.len() as i64);
        }

        let mut pic_bytes = None;
        let mut raw_tag_items = Vec::new(); // (tag_type, item_key, raw_value)
        
        // Mapa para la fusión (prioridad). Almacenamos el valor de mayor prioridad para cada clave técnica.
        // Prioridad: Id3v2 > VorbisComments > Mp4ilst > Ape > RiffInfo > Id3v1
        let mut fused_map = std::collections::HashMap::new();

        if let Ok(probe) = Probe::open(path) {
            if let Ok(tagged_file) = probe.guess_file_type().unwrap_or(Probe::open(path).unwrap()).read() {
                let props = tagged_file.properties();
                song.sample_rate = props.sample_rate().map(|sr| sr as i64);
                song.channels = props.channels().map(|ch| ch as i64);
                song.duration_secs = Some(props.duration().as_secs_f64());
                song.bit_depth = props.bit_depth().map(|b| b as i64);
                
                // 1. Recolectar TODAS las etiquetas presentes
                for tag in tagged_file.tags() {
                    let tag_type = tag.tag_type();
                    let tag_type_str = format!("{:?}", tag_type);
                    
                    // Nivel de prioridad dependiente del formato (Menor es más prioritario)
                    let file_type = tagged_file.file_type();
                    let priority = match (file_type, tag_type) {
                        // FLAC: VorbisComments tiene la prioridad absoluta (ID3v2 es solo lectura)
                        (lofty::file::FileType::Flac, lofty::tag::TagType::VorbisComments) => 1,
                        
                        // APE/MPC: APE tiene la prioridad absoluta (ID3v2/v1 son secundarios o solo lectura)
                        (lofty::file::FileType::Ape | lofty::file::FileType::Mpc, lofty::tag::TagType::Ape) => 1,
                        
                        // WAV/AIFF: ID3v2 tiene prioridad sobre RIFF INFO/Text Chunks
                        (lofty::file::FileType::Wav | lofty::file::FileType::Aiff, lofty::tag::TagType::Id3v2) => 1,
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
                            // Guardar para SONG_TAG_ITEMS (Crudo total)
                            raw_tag_items.push((tag_type_str.clone(), format!("{:?}", key), val.clone()));
                            
                            // Lógica de Fusión: Solo actualizamos si no existe o si el nuevo tag tiene más prioridad
                            let entry = fused_map.entry(key.clone()).or_insert((priority, val.clone()));
                            if priority < entry.0 {
                                *entry = (priority, val.clone());
                            }
                        }
                    }

                    // Intentar extraer carátula del tag más prioritario que la tenga
                    if pic_bytes.is_none() {
                        if let Some(pic) = tag.pictures().first() {
                            song.embedded_cover = true;
                            pic_bytes = Some(pic.data().to_vec());
                        }
                    }
                }

                // 2. Poblar SongData con los valores fusionados (Alta Fidelidad)
                let get_fused = |key: ItemKey| fused_map.get(&key).map(|(_, v)| v.clone());

                song.title = get_fused(ItemKey::TrackTitle);
                song.artist = get_fused(ItemKey::TrackArtist);
                song.album = get_fused(ItemKey::AlbumTitle);
                song.genre = get_fused(ItemKey::Genre);
                song.track_number = get_fused(ItemKey::TrackNumber);
                song.album_artist = get_fused(ItemKey::AlbumArtist);
                
                // Año con lógica de fallback robusta pero preservando formato
                song.release_year = get_fused(ItemKey::Year)
                    .or_else(|| get_fused(ItemKey::RecordingDate))
                    .or_else(|| get_fused(ItemKey::OriginalReleaseDate))
                    .map(|d| d.chars().filter(|c| c.is_digit(10)).take(4).collect::<String>());

                extended.lyrics = get_fused(ItemKey::UnsyncLyrics).or_else(|| get_fused(ItemKey::Lyrics));
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
        }

        let pic_hash = pic_bytes.as_ref().map(|b| Database::generate_hash(&hex::encode(b)));

        if let Ok(mut db) = db_m.lock() {
            if let Ok((target_hash, needs_processing)) = db.insert_song_full(&song, &extended, pic_hash, raw_tag_items) {
                if needs_processing && !enqueued_covers.contains(&target_hash) {
                    if let Some(data) = pic_bytes {
                        // Solo encolamos si es un archivo AVIF que no existe
                        let expected_path = format!("cache/covers/{}.avif", target_hash);
                        if !std::path::Path::new(&expected_path).exists() {
                            crate::utils::covers::enqueue_cover_job(data, target_hash.clone());
                            enqueued_covers.insert(target_hash);
                        }
                    }
                }
            }
        }
    }
}
