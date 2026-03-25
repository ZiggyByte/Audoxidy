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
        
        let start_order = {
            if let Ok(db) = db_m.lock() {
                db.get_max_import_order().unwrap_or(0) + 1
            } else {
                1
            }
        };
        
        let mut current_order = start_order;
        
        for entry in WalkDir::new(root).sort_by_file_name().into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if supported_extensions.contains(&ext.to_lowercase().as_str()) {
                        Self::process_file(db_m, path, root, current_order);
                        current_order += 1;
                    }
                }
            }
        }
    }

    fn process_file(db_m: &Arc<Mutex<Database>>, path: &Path, root: &str, import_order: i64) {
        let mut record = crate::db::database::SongData::default();
        record.import_order = import_order;
        record.full_file_path = path.to_string_lossy().to_string();
        record.file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        record.root_directory_name = Some(root.to_string());
        record.full_root_directory_path = path.parent().map(|p| p.to_string_lossy().to_string());
        
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            record.format = Some(ext.to_uppercase()); // Guardar exacto sin convertir a mayúsculas
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
                    // Extraer siempre como String Literal sin conversiones
                    record.track_number = t.get_string(ItemKey::TrackNumber).map(|s| s.to_string())
                                           .or_else(|| t.track().map(|n| n.to_string()));
                    record.total_tracks = t.get_string(ItemKey::TrackTotal).map(|s| s.to_string())
                                           .or_else(|| t.track_total().map(|n| n.to_string()));
                    record.disc_number = t.get_string(ItemKey::DiscNumber).map(|s| s.to_string())
                                          .or_else(|| t.disk().map(|n| n.to_string()));
                    record.total_discs = t.get_string(ItemKey::DiscTotal).map(|s| s.to_string())
                                          .or_else(|| t.disk_total().map(|n| n.to_string()));
                    
                    record.title = t.title().as_deref().map(|s| s.to_string());
                    record.artist = t.artist().as_deref().map(|s| s.to_string());
                    record.album = t.album().as_deref().map(|s| s.to_string());
                    record.genre = t.genre().as_deref().map(|s| s.to_string());
                    // release_year in lofty 0.23: Accessor::year is replaced by date. 
                    record.release_year = t.get_string(ItemKey::Year).map(|s| s.to_string())
                        .or_else(|| t.get_string(ItemKey::RecordingDate).map(|s| s.to_string()))
                        .or_else(|| t.get_string(ItemKey::OriginalReleaseDate).map(|s| s.to_string()))
                        .or_else(|| t.date().map(|d| d.to_string()));
                    
                    record.album_artist = t.get_string(ItemKey::AlbumArtist).map(|s| s.to_string());
                    if record.album_artist.is_some() {
                        // En 0.23 lofty resuelve ALBUM ARTIST por nosotros de manera estándar
                        record.album_artist_tag_format = Some("ALBUMARTIST".to_string());
                    }
                    
                    // Lyrics is no longer supported directly, changed to UnsyncLyrics
                    record.lyrics = t.get_string(ItemKey::UnsyncLyrics).map(|s| s.to_string());
                    record.comments = t.get_string(ItemKey::Comment).map(|s| s.to_string());
                    record.composer = t.get_string(ItemKey::Composer).map(|s| s.to_string());
                    record.publisher = t.get_string(ItemKey::Publisher).map(|s| s.to_string());
                    record.isrc = t.get_string(ItemKey::Isrc).map(|s| s.to_string());
                    record.bpm = t.get_string(ItemKey::Bpm).map(|s| s.to_string());
                    
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
                            
                            let pic_data = pic.data().to_vec();
                            let safe_album_name_clone = safe_album_name.clone();
                            
                            let filename = format!("{}.{}", safe_album_name, ext);
                            let cover_path = cover_dir.join(&filename);
                            
                            if !cover_path.exists() {
                                let _ = std::fs::write(&cover_path, &pic_data);
                            }
                            // Guardar ruta relativa/completa de la original en la db
                            let absolute_cover = std::fs::canonicalize(&cover_path)
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|_| cover_path.to_string_lossy().to_string());
                            record.original_cover_root = Some(absolute_cover);

                            // 2. Ejecutar la creación de la caché en el Pool de Hilos dedicado (512x512, avif, 65%)
                            let expected_cached_path = std::path::PathBuf::from(format!("cache/covers/{}.avif", safe_album_name_clone));
                            let abs_cache = std::env::current_dir().unwrap_or_default().join(&expected_cached_path).to_string_lossy().to_string();
                            record.compressed_cached_cover_root = Some(abs_cache);

                            // Despachar tarea asíncrona sin bloquear el escaneo principal de metadatos
                            crate::utils::covers::get_cover_pool().spawn(move || {
                                if let Err(e) = crate::utils::covers::process_and_save_cover(&pic_data, &safe_album_name_clone) {
                                    // Se podría usar tracing::warn si está importado, de lo contrario lo ignoramos pasivamente
                                    #[cfg(debug_assertions)]
                                    eprintln!("Error al generar la caché de imagen {}: {}", safe_album_name_clone, e);
                                }
                            });
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
