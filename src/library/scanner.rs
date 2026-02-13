use walkdir::WalkDir;
use lofty::prelude::*;
use lofty::file::TaggedFileExt;
use lofty::tag::Accessor;
use crate::library::metadata::LibrarySong;
use crate::library::db::MediaLibrary;
use std::path::Path;
use std::sync::Arc;

pub struct LibraryScanner {
    library: Arc<MediaLibrary>,
}

impl LibraryScanner {
    pub fn new(library: Arc<MediaLibrary>) -> Self {
        Self { library }
    }

    pub fn scan_directory<P: AsRef<Path>>(&self, path: P) {
        let library = self.library.clone();
        let path = path.as_ref().to_owned();

        std::thread::spawn(move || {
            for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let _path_str = entry.path().to_string_lossy().to_string();
                    let ext = entry.path().extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                    
                    if ["mp3", "flac", "wav", "ogg", "m4a"].contains(&ext.as_str()) {
                        if let Ok(song) = Self::extract_metadata(entry.path()) {
                            let _ = library.add_song(&song);
                        }
                    }
                }
            }
        });
    }

    fn extract_metadata(path: &Path) -> Result<LibrarySong, Box<dyn std::error::Error>> {
        let tagged_file = lofty::read_from_path(path)?;
        let mut song = LibrarySong::default();
        song.path = path.to_string_lossy().to_string();

        if let Some(tag) = tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) {
            song.title = tag.title().as_deref().unwrap_or("Sin título").to_string();
            song.artist = tag.artist().as_deref().unwrap_or("Artista desconocido").to_string();
            song.album = tag.album().as_deref().unwrap_or("Álbum desconocido").to_string();
            song.genre = tag.genre().as_deref().unwrap_or("Desconocido").to_string();
            song.year = tag.year();
            
            // Track and disc numbers
            song.track_number = tag.track();
            song.disc_number = tag.disk();
        }

        let properties = tagged_file.properties();
        song.duration_sec = properties.duration().as_secs() as i64;

        Ok(song)
    }
}
