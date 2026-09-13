use crate::integrations::control::{ExternalControlEvent, ExternalControlSender};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPosition, PlatformConfig, SeekDirection,
};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

/// Error al construir o registrar el puente con los controles multimedia del
/// sistema.
///
/// El `Display` reproduce el mensaje original del error de `souvlaki` y la
/// causa queda expuesta mediante `source()`; el llamador actual usa
/// `.expect(…)`, que imprime la representación `Debug` y no el `Display`.
#[derive(Debug, thiserror::Error)]
pub enum MediaControlsError {
    /// Falló la construcción del backend de souvlaki.
    #[error("{0}")]
    Create(#[source] souvlaki::Error),
    /// Falló el registro del manejador de eventos del panel del sistema.
    #[error("{0}")]
    Attach(#[source] souvlaki::Error),
}

pub struct SystemMediaControls {
    controls: Mutex<MediaControls>,
    /// Últimos metadatos publicados `(title, artist, album, cover_url)`.
    /// Evita reenviar `set_metadata` en cada mensaje: en Windows souvlaki vuelve
    /// a leer el archivo de la carátula en disco en cada llamada.
    last_metadata: Mutex<Option<(String, String, String, Option<String>)>>,
}

impl SystemMediaControls {
    pub fn new(sender: ExternalControlSender) -> Result<Self, MediaControlsError> {
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

        let mut controls = MediaControls::new(config).map_err(MediaControlsError::Create)?;

        // Todos los eventos del panel del sistema se reenvían al canal externo;
        // la aplicación es la única que traduce cada evento a sus mensajes y
        // muta el motor de audio, de modo que cada acción se despacha una sola
        // vez y desde el hilo de la interfaz.
        controls
            .attach(move |event| {
                let mapped = match event {
                    MediaControlEvent::Play => Some(ExternalControlEvent::Play),
                    MediaControlEvent::Pause => Some(ExternalControlEvent::Pause),
                    MediaControlEvent::Toggle => Some(ExternalControlEvent::Toggle),
                    MediaControlEvent::Stop => Some(ExternalControlEvent::Stop),
                    MediaControlEvent::Next => Some(ExternalControlEvent::Next),
                    MediaControlEvent::Previous => Some(ExternalControlEvent::Previous),
                    MediaControlEvent::Seek(SeekDirection::Forward) => {
                        Some(ExternalControlEvent::SeekForward)
                    }
                    MediaControlEvent::Seek(SeekDirection::Backward) => {
                        Some(ExternalControlEvent::SeekBackward)
                    }
                    MediaControlEvent::SeekBy(SeekDirection::Forward, delta) => {
                        Some(ExternalControlEvent::SeekRelative(delta.as_secs_f64()))
                    }
                    MediaControlEvent::SeekBy(SeekDirection::Backward, delta) => {
                        Some(ExternalControlEvent::SeekRelative(-delta.as_secs_f64()))
                    }
                    MediaControlEvent::SetPosition(MediaPosition(position)) => {
                        Some(ExternalControlEvent::SeekTo(position.as_secs_f64()))
                    }
                    MediaControlEvent::SetVolume(volume) => {
                        Some(ExternalControlEvent::SetVolume(volume))
                    }
                    // OpenUri / Raise / Quit se ignoran: no mutan estado local.
                    _ => None,
                };
                if let Some(mapped) = mapped {
                    sender.send(mapped);
                }
            })
            .map_err(MediaControlsError::Attach)?;

        Ok(Self {
            controls: Mutex::new(controls),
            last_metadata: Mutex::new(None),
        })
    }

    /// Refleja en el panel del sistema el volumen aplicado por la aplicación.
    ///
    /// Solo el backend MPRIS implementa `MediaControls::set_volume`; en el
    /// resto de plataformas el método no existe, por lo que allí esta función
    /// es un no-op que mantiene el build multiplataforma.
    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    ))]
    pub fn set_volume(&self, volume: f64) {
        if let Ok(mut controls) = self.controls.lock() {
            if let Err(e) = controls.set_volume(volume.clamp(0.0, 1.0)) {
                tracing::debug!("No se pudo reflejar el volumen en los controles multimedia: {e}");
            }
        }
    }

    /// No-op en plataformas cuyo backend de souvlaki no expone `set_volume`.
    #[cfg(not(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    )))]
    pub fn set_volume(&self, volume: f64) {
        let _ = volume;
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
        if let Err(e) = controls.set_playback(status) {
            tracing::debug!(
                "No se pudo reflejar el estado de reproducción en los controles multimedia: {e}"
            );
        }

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
            let result = controls.set_metadata(MediaMetadata {
                title: Some(&state.title),
                artist: Some(&state.artist),
                album: Some(&state.album),
                duration: Some(Duration::from_secs_f64(state.total_duration_sec)),
                cover_url: cover.as_deref(),
                ..MediaMetadata::default()
            });
            if let Err(e) = result {
                tracing::debug!(
                    "No se pudieron reflejar los metadatos en los controles multimedia: {e}"
                );
            }
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
