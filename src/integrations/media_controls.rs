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
    // The playback publish is change-driven, so the D-Bus call never rides the
    // caller's cadence: an unchanged published identity publishes nothing.
    last_playback: Mutex<Option<PlaybackKey>>,
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
            last_playback: Mutex::new(None),
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
        // Build the published identities once, before taking any lock.
        // `cover_file_url` only returns a URL if the cached AVIF exists.
        let cover = state.cover_path.as_deref().and_then(cover_file_url);
        let current = (
            state.title.clone(),
            state.artist.clone(),
            state.album.clone(),
            cover.clone(),
        );
        let playback_key: PlaybackKey = (
            state.is_playing,
            position_bucket_secs(state.current_pos_sec),
            current.0.clone(),
            current.1.clone(),
            current.2.clone(),
            cover.clone(),
        );

        // The playback publish is change-driven. The guard over the last
        // published identity is dropped before the controls lock is taken, so
        // the two are never held at the same time and no lock is held across
        // the D-Bus call.
        let should_publish = {
            let last_playback = self.last_playback.lock().unwrap();
            playback_should_publish(last_playback.as_ref(), &playback_key)
        };

        if should_publish {
            {
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
            }
            // Store the key whether or not `set_playback` failed, so a failing
            // backend is not retried on every subsequent call.
            *self.last_playback.lock().unwrap() = Some(playback_key);
        }

        // Actualizar metadatos solo cuando cambian (título, artista, álbum o
        // carátula).
        let mut controls = self.controls.lock().unwrap();
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

/// Published playback identity `(is_playing, position_bucket_secs, title,
/// artist, album, cover_url)`.
///
/// A change in any component requires one `set_playback` publish; an unchanged
/// key must publish nothing.
type PlaybackKey = (bool, u64, String, String, String, Option<String>);

/// Collapses a position in seconds to its one-second bucket.
///
/// The bucket bounds the publish rate to at most one per second even if a
/// future caller regresses to the frame cadence; MPRIS clients interpolate
/// `Position`/`Rate`, so a per-second snapshot is enough. A non-positive or
/// non-finite position resolves to `0`.
fn position_bucket_secs(current_pos_sec: f64) -> u64 {
    if !current_pos_sec.is_finite() || current_pos_sec <= 0.0 {
        0
    } else {
        current_pos_sec.floor() as u64
    }
}

/// Reports whether the published playback identity changed.
fn playback_should_publish(previous: Option<&PlaybackKey>, current: &PlaybackKey) -> bool {
    previous != Some(current)
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

    #[test]
    fn position_bucket_truncates_seconds() {
        assert_eq!(super::position_bucket_secs(12.99), 12);
        assert_eq!(super::position_bucket_secs(13.0), 13);
    }

    #[test]
    fn position_bucket_clamps_non_positive() {
        assert_eq!(super::position_bucket_secs(-5.0), 0);
        assert_eq!(super::position_bucket_secs(0.0), 0);
    }

    #[test]
    fn position_bucket_handles_non_finite() {
        assert_eq!(super::position_bucket_secs(f64::NAN), 0);
        assert_eq!(super::position_bucket_secs(f64::INFINITY), 0);
    }

    #[test]
    fn playback_publish_is_change_driven() {
        let base: super::PlaybackKey = (
            true,
            12,
            "Title".to_string(),
            "Artist".to_string(),
            "Album".to_string(),
            None,
        );

        assert!(super::playback_should_publish(None, &base));
        assert!(!super::playback_should_publish(Some(&base), &base));

        let mut changed_playing = base.clone();
        changed_playing.0 = false;
        assert!(super::playback_should_publish(
            Some(&base),
            &changed_playing
        ));

        let mut changed_bucket = base.clone();
        changed_bucket.1 = 13;
        assert!(super::playback_should_publish(Some(&base), &changed_bucket));

        let mut changed_track = base.clone();
        changed_track.2 = "Other".to_string();
        assert!(super::playback_should_publish(Some(&base), &changed_track));
    }
}
