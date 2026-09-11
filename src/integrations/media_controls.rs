use crate::audio::AudioManager;
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, PlatformConfig};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct SystemMediaControls {
    controls: Arc<Mutex<MediaControls>>,
    /// Últimos metadatos publicados `(title, artist, album, cover_url)`.
    /// Evita reenviar `set_metadata` en cada mensaje: en Windows souvlaki vuelve
    /// a leer el archivo de la carátula en disco en cada llamada.
    last_metadata: Mutex<Option<(String, String, String, Option<String>)>>,
}

impl SystemMediaControls {
    pub fn new(audio_manager: Arc<AudioManager>) -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(target_os = "linux")]
        let hwnd = None;

        #[cfg(target_os = "windows")]
        let hwnd = { None };

        #[cfg(target_os = "macos")]
        let hwnd = None;

        let config = PlatformConfig {
            dbus_name: "audoxidy",
            display_name: "Audoxidy Player",
            hwnd,
        };

        let mut controls = MediaControls::new(config)?;
        let am = audio_manager.clone();

        // Configurar capacidades iniciales
        controls.attach(move |event| match event {
            MediaControlEvent::Play => am.set_playing(true),
            MediaControlEvent::Pause => am.set_playing(false),
            MediaControlEvent::Toggle => {
                let playing = am.get_state().is_playing;
                am.set_playing(!playing);
            }
            MediaControlEvent::Next => tracing::info!("MPRIS: Next (No implementado)"),
            MediaControlEvent::Previous => tracing::info!("MPRIS: Previous (No implementado)"),
            MediaControlEvent::Stop => am.stop(),
            _ => {}
        })?;

        Ok(Self {
            controls: Arc::new(Mutex::new(controls)),
            last_metadata: Mutex::new(None),
        })
    }

    pub fn update(&self, state: &crate::audio::engine::AudioState) {
        // Actualizar estado de reproducción
        let mut controls = self.controls.lock().unwrap();
        let status = if state.is_playing {
            souvlaki::MediaPlayback::Playing {
                progress: Some(souvlaki::MediaPosition(Duration::from_secs_f64(
                    state.current_pos_sec,
                ))),
            }
        } else {
            souvlaki::MediaPlayback::Paused {
                progress: Some(souvlaki::MediaPosition(Duration::from_secs_f64(
                    state.current_pos_sec,
                ))),
            }
        };
        let _ = controls.set_playback(status);

        // Actualizar metadatos solo cuando cambian (título, artista, álbum o
        // carátula). `cover_file_url` solo devuelve URL si el AVIF cacheado existe.
        let cover = state.cover_path.as_deref().and_then(cover_file_url);
        let current = (
            state.title.clone(),
            state.artist.clone(),
            state.album.clone(),
            cover.clone(),
        );

        let mut last = self.last_metadata.lock().unwrap();
        if last.as_ref() != Some(&current) {
            let _ = controls.set_metadata(MediaMetadata {
                title: Some(&state.title),
                artist: Some(&state.artist),
                album: Some(&state.album),
                duration: Some(Duration::from_secs_f64(state.total_duration_sec)),
                cover_url: cover.as_deref(),
                ..MediaMetadata::default()
            });
            *last = Some(current);
        }
    }
}

/// Convierte la ruta relativa del cache AVIF en una URL `file://` válida.
///
/// Devuelve `None` si la ruta está vacía, si el archivo no existe o si no se
/// puede absolutizar. La ruta se canonicaliza antes de `Url::from_file_path`
/// (que exige una ruta absoluta) y la existencia previa garantiza que nunca se
/// publique una URL rota al panel multimedia del sistema.
fn cover_file_url(relative: &str) -> Option<String> {
    if relative.is_empty() {
        return None;
    }
    let path = Path::new(relative);
    if !path.is_file() {
        return None;
    }
    let abs = std::fs::canonicalize(path).ok()?;
    url::Url::from_file_path(abs).ok().map(|u| u.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_file_url() {
        // Se usa `super::` porque la función de test comparte nombre con el
        // helper y lo ocultaría en el ámbito del módulo.
        assert_eq!(super::cover_file_url(""), None);
        assert_eq!(
            super::cover_file_url("cache/covers/does-not-exist.avif"),
            None
        );

        let dir = std::env::temp_dir();
        let file = dir.join(format!("audoxidy_cover_{}.avif", std::process::id()));
        std::fs::write(&file, b"x").unwrap();
        let url = super::cover_file_url(file.to_str().unwrap()).unwrap();
        assert!(url.starts_with("file://"));
        assert!(url.ends_with(".avif"));
        let _ = std::fs::remove_file(&file);
    }
}
