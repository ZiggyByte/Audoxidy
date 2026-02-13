use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, PlatformConfig};
use crate::audio::AudioManager;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct SystemMediaControls {
    controls: Arc<Mutex<MediaControls>>,
}

impl SystemMediaControls {
    pub fn new(audio_manager: Arc<AudioManager>) -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(target_os = "linux")]
        let hwnd = None;

        #[cfg(target_os = "windows")]
        let hwnd = {
             None 
        };

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
        controls.attach(move |event| {
            match event {
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
            }
        })?;

        Ok(Self {
            controls: Arc::new(Mutex::new(controls)),
        })
    }

    pub fn update(&self, state: &crate::audio::engine::AudioState) {
        // Actualizar estado de reproducción
        let mut controls = self.controls.lock().unwrap();
        let status = if state.is_playing {
             souvlaki::MediaPlayback::Playing { progress: Some(souvlaki::MediaPosition(Duration::from_secs_f64(state.current_pos_sec))) }
        } else {
             souvlaki::MediaPlayback::Paused { progress: Some(souvlaki::MediaPosition(Duration::from_secs_f64(state.current_pos_sec))) }
        };
        let _ = controls.set_playback(status);

        // Actualizar metadatos
        let _ = controls.set_metadata(MediaMetadata {
            title: Some(&state.title),
            artist: Some(&state.artist),
            album: None, // No tenemos album en state aun
            duration: Some(Duration::from_secs_f64(state.total_duration_sec)),
            cover_url: None, // TODO: Convertir bytes a URL temporal o similar
            ..MediaMetadata::default()
        });
    }
}
