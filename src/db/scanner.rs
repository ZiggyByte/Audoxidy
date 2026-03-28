use std::path::Path;
use std::sync::{Arc, Mutex};
use lofty::probe::Probe;
use lofty::prelude::{TaggedFileExt, AudioFile, ItemKey};
use lofty::tag::Accessor;
use walkdir::WalkDir;

use crate::db::database::Database;

use std::sync::atomic::{AtomicBool, Ordering};

pub struct Scanner {
    _db: Arc<Mutex<Database>>,
    is_scanning: AtomicBool,
}

impl Scanner {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self { 
            _db: db,
            is_scanning: AtomicBool::new(false),
        }
    }

    pub fn is_scanning(&self) -> bool {
        self.is_scanning.load(Ordering::SeqCst)
    }
    
    // Escaneo asíncrono
    pub fn scan_folder_async(self: Arc<Self>, folder_path: String) {
        let db_arc = Arc::clone(&self._db);
        let scanner_arc = Arc::clone(&self);
        
        self.is_scanning.store(true, Ordering::SeqCst);
        
        std::thread::spawn(move || {
            Self::scan_folder(&db_arc, &folder_path);
            scanner_arc.is_scanning.store(false, Ordering::SeqCst);
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
        let mut enqueued_covers = std::collections::HashSet::new();
        
        // Iniciar transacción masiva
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
                        
                        // Commit por lotes cada 500 archivos para estabilidad y rendimiento
                        if count % 500 == 0 {
                            if let Ok(db) = db_m.lock() {
                                let _ = db.commit_transaction();
                                let _ = db.begin_transaction();
                            }
                        }
                    }
                }
            }
        }
        
        // Finalizar transacción
        if let Ok(db) = db_m.lock() {
            let _ = db.commit_transaction();
        }

        // Al finalizar el bucle de escaneo de archivos, sugerimos liberar memoria de carátulas
        crate::utils::covers::clear_all_cover_cache();
    }

    fn process_file(db_m: &Arc<Mutex<Database>>, path: &Path, root: &str, import_order: i64, enqueued_covers: &mut std::collections::HashSet<String>) {
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
                            let artist_name = record.album_artist.as_deref().or(record.artist.as_deref()).unwrap_or("Desconocido");
                            let album_name = record.album.as_deref().unwrap_or("Desconocido");
                            
                            // 1. Generar ID único de álbum (Hash Artista + Álbum)
                             let album_hash = crate::utils::covers::generate_album_id(artist_name, album_name);
                             let final_id = album_hash.clone(); // Usamos siempre el hash del álbum para consistencia masiva
                             
                             // 2. Ejecutar la creación de la caché en el Pool de Hilos dedicado (512x512, avif, 80%)
                             let expected_cached_path = std::path::PathBuf::from(format!("cache/covers/{}.avif", final_id));
                             let abs_cache = std::env::current_dir().unwrap_or_default().join(&expected_cached_path).to_string_lossy().to_string();
                             record.compressed_cached_cover_root = Some(abs_cache.clone());

                             // Despachar tarea asíncrona mediante el Gateway Throttled solo si NO existe ya y NO ha sido encolado en esta sesión
                             if !expected_cached_path.exists() && !enqueued_covers.contains(&final_id) {
                                 let pic_data = pic.data().to_vec();
                                 crate::utils::covers::enqueue_cover_job(pic_data, final_id.clone());
                                 enqueued_covers.insert(final_id);
                             }

                            // 3. Registrar en la tabla de ALBUMS para acceso instantáneo
                            if let Ok(db) = db_m.lock() {
                                let _ = db.upsert_album(
                                    &album_hash,
                                    album_name,
                                    artist_name,
                                    record.genre.as_deref().unwrap_or(""),
                                    record.release_year.as_deref().unwrap_or(""),
                                    Some(&abs_cache),
                                    record.duration_secs.unwrap_or(0.0)
                                );
                            }
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
