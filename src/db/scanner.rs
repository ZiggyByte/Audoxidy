use std::path::Path;
use std::sync::{Arc, Mutex};
use lofty::probe::Probe;
use lofty::prelude::{TaggedFileExt, AudioFile, ItemKey};
use lofty::tag::Accessor;
use walkdir::WalkDir;

use crate::db::database::Database;

pub struct Scanner {
    _db: Arc<Mutex<Database>>,
}

impl Scanner {
    #[allow(dead_code)]
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self { _db: db }
    }
    
    // Escaneo asíncrono
    pub fn scan_folder_async(&self, folder_path: String) {
        let db_arc = Arc::clone(&self._db);
        std::thread::spawn(move || {
            Self::scan_folder(&db_arc, &folder_path);
        });
    }

    fn scan_folder(db_m: &Arc<Mutex<Database>>, root: &str) {
        let supported_extensions = ["mp3", "flac", "wav", "ogg", "m4a"];
        
        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if supported_extensions.contains(&ext.to_lowercase().as_str()) {
                        Self::process_file(db_m, path, root);
                    }
                }
            }
        }
    }

    fn process_file(db_m: &Arc<Mutex<Database>>, path: &Path, root: &str) {
        let mut record = crate::db::database::SongRecord::default();
        record.full_file_path = path.to_string_lossy().to_string();
        record.file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        record.root_directory_name = Some(root.to_string());
        record.full_root_directory_path = path.parent().map(|p| p.to_string_lossy().to_string());
        
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            record.format = Some(ext.to_uppercase());
        }
        
        if let Ok(metadata) = std::fs::metadata(path) {
            record.size = Some(metadata.len() as i64);
        }

        if let Ok(probe) = Probe::open(path) {
            if let Ok(tagged_file) = probe.read() {
                let props = tagged_file.properties();
                record.sample_rate = props.sample_rate().map(|sr| sr as i64);
                record.channels = props.channels().map(|ch| ch as i64);
                record.duration_secs = Some(props.duration().as_secs_f64());
                record.bit_depth = props.bit_depth().map(|b| b as i64);
                
                // Contingencia: Si Lofty falla o el codec es abstracto, extraer de Symphonia.
                if record.bit_depth.is_none() {
                    if let Ok(file) = std::fs::File::open(path) {
                        let mss = symphonia::core::io::MediaSourceStream::new(Box::new(file), Default::default());
                        let mut hint = symphonia::core::probe::Hint::new();
                        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                            hint.with_extension(ext);
                        }
                        if let Ok(probed) = symphonia::default::get_probe().format(&hint, mss, &Default::default(), &Default::default()) {
                            if let Some(track) = probed.format.default_track() {
                                if let Some(bps) = track.codec_params.bits_per_sample {
                                    record.bit_depth = Some(bps as i64);
                                }
                            }
                        }
                    }
                }
                
                if let Some(t) = tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) {
                    record.track_number = t.track();
                    record.total_tracks = t.track_total();
                    record.disc_number = t.disk();
                    record.total_discs = t.disk_total();
                    
                    record.title = t.title().as_deref().map(|s| s.to_string());
                    record.artist = t.artist().as_deref().map(|s| s.to_string());
                    record.album = t.album().as_deref().map(|s| s.to_string());
                    record.genre = t.genre().as_deref().map(|s| s.to_string());
                    // release_year in lofty 0.22 comes as optional u32 if mapped directly, or we can use get_string
                    record.release_year = t.year().map(|y: u32| y.to_string()).or_else(|| t.get_string(&ItemKey::Year).map(|s| s.to_string()));
                    
                    record.album_artist = t.get_string(&ItemKey::AlbumArtist).map(|s| s.to_string());
                    record.lyrics = t.get_string(&ItemKey::Lyrics).map(|s| s.to_string());
                    record.comments = t.get_string(&ItemKey::Comment).map(|s| s.to_string());
                    record.composer = t.get_string(&ItemKey::Composer).map(|s| s.to_string());
                    record.publisher = t.get_string(&ItemKey::Publisher).map(|s| s.to_string());
                    record.isrc = t.get_string(&ItemKey::Isrc).map(|s| s.to_string());
                    record.bpm = t.get_string(&ItemKey::Bpm).map(|s| s.to_string());
                    
                    if !t.pictures().is_empty() {
                        record.embedded_cover = true;
                        if let Some(pic) = t.pictures().first() {
                            let ext = match pic.mime_type() {
                                Some(m) if format!("{:?}", m).to_lowercase().contains("png") => "png",
                                Some(m) if format!("{:?}", m).to_lowercase().contains("gif") => "gif",
                                _ => "jpg",
                            };
                            let cover_dir = std::path::PathBuf::from("covers");
                            if !cover_dir.exists() {
                                let _ = std::fs::create_dir_all(&cover_dir); // Directorio principal temporal
                            }
                            
                            let safe_album_name = record.album.as_deref()
                                .unwrap_or("unknown")
                                .replace(&['/', '\\', ':', '*', '?', '"', '<', '>', '|'][..], "_");
                            let filename = format!("{}.{}", safe_album_name, ext);
                            let cover_path = cover_dir.join(&filename);
                            
                            if !cover_path.exists() {
                                let _ = std::fs::write(&cover_path, pic.data());
                            }
                            // Guardar ruta relativa/completa de la db
                            let absolute_cover = std::fs::canonicalize(&cover_path)
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|_| cover_path.to_string_lossy().to_string());
                            record.original_cover_root = Some(absolute_cover);
                        }
                    }
                }
            }
        }
        
        if let Ok(mut db) = db_m.lock() {
            let _ = db.insert_song(&record);
        }
    }
}
