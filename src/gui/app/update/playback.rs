//! Transport and playback message handlers extracted from the update dispatcher.

use crate::audio::AudioError;
use crate::gui::app::{
    Message, fine_seek_enabled, persistence::log_persist, subscription::external_control_to_message,
};
use crate::integrations::control::ExternalControlEvent;
use iced::Task;

/// Salto fijo (segundos) que aplican los eventos `Seek` sin desplazamiento.
/// `SeekBy` lleva su propio desplazamiento y no usa este valor.
pub(in crate::gui::app) const SEEK_STEP_SECS: f64 = 10.0;

/// Calcula el destino de una búsqueda relativa, acotado al rango válido.
///
/// Las posiciones llegan del servicio multimedia del sistema, por lo que son
/// entrada no confiable: una entrada no finita se resuelve a `0.0`, y el
/// resultado nunca sale de `[0, total]` (o de `[0, ∞)` si la duración aún no
/// se conoce).
pub(in crate::gui::app) fn seek_target(current: f64, delta: f64, total: f64) -> f64 {
    let target = current + delta;
    if !target.is_finite() {
        return 0.0;
    }
    if total > 0.0 {
        target.clamp(0.0, total)
    } else {
        target.max(0.0)
    }
}

/// Acota el volumen recibido del sistema a `[0.0, 1.0]`.
///
/// Un valor no finito se resuelve a `0.0` para que nunca se propague al motor
/// de audio ni al reflejo en el panel.
pub(in crate::gui::app) fn clamp_volume(v: f64) -> f32 {
    if !v.is_finite() {
        return 0.0;
    }
    v.clamp(0.0, 1.0) as f32
}

impl crate::gui::app::AudoxidyApp {
    pub(in crate::gui::app) fn handle_playback(
        &mut self,
        message: Message,
        state: &crate::audio::engine::AudioState,
    ) -> Task<Message> {
        match message {
            Message::PlayPause => {
                let _ = self.audio_manager.toggle_play_pause();

                // Si acabamos de pausar (no está reproduciendo), purgamos buffers para liberar RAM
                if !self.audio_manager.is_playing() {
                    tracing::debug!("Audoxidy Audio: Cleaning Buffers on Pause (Memory Recovery).");
                    let _ = self.audio_manager.purge_buffers();
                }

                self.persist_playlist_state();
                self.wake_up_controls(false)
            }
            Message::NextTrack => {
                crate::utils::covers::clear_raw_cache();

                // Crossfade manual: si el grupo Crossfade está activo y su sub-función
                // manual está habilitada, se dispara la mezcla con la duración configurada
                // (el decoder mezcla la pista actual con la siguiente pre-cargada).
                let st = self.audio_manager.get_state();
                if st.crossfade_enabled
                    && st.crossfade_manual_enabled
                    && st.crossfade_manual_ms > 0.0
                {
                    // Garantizar que la siguiente canción esté pre-cargada ANTES de
                    // disparar la mezcla (el usuario puede saltar en cualquier momento,
                    // no solo ~15s antes del final). Si ya está pre-cargada, el dedup
                    // del decoder ignora la orden.
                    if let Some(next_song) = self.playlist_manager.get_next_song_ref() {
                        if self.player_ui_state.preloaded_next_path.as_deref()
                            != Some(next_song.file_path.as_ref())
                        {
                            let _ = self.audio_manager.preload_next(
                                &next_song.file_path,
                                next_song.title.to_string(),
                                next_song.artist_name.to_string(),
                                next_song.album_title.to_string(),
                                next_song.cover_path.as_deref().map(str::to_string),
                                None,
                                None,
                            );
                            self.player_ui_state.preloaded_next_path =
                                Some(next_song.file_path.to_string());
                        }
                        let _ = self
                            .audio_manager
                            .crossfade_next(st.crossfade_manual_ms as f64);
                    }
                }

                self.playlist_manager.play_next(&self.audio_manager);
                // Nueva pista activa: restablecer la pre-carga para la siguiente y
                // marcar el fin de la pre-carga (el GC se reactiva 20s después).
                self.player_ui_state.preloaded_next_path = None;
                self.player_ui_state.preload_finished_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .ok();
                self.persist_playlist_state();

                if self.playlist_manager.shuffle_active {
                    if let Some(session) = &self.playlist_manager.shuffle_session {
                        // db-lock: persistencia — el guardado se omite si el lock falla
                        if let Ok(mut db) = self.database.lock() {
                            log_persist(
                                "shuffle_session",
                                db.save_shuffle_session(
                                    self.playlist_manager.active_playlist_id,
                                    session,
                                ),
                            );
                        } else {
                            tracing::warn!("db-lock: lock no disponible");
                        }
                    }
                }

                self.sync_player_art();
                let wake_task = self.wake_up_controls(false);
                if !self.is_mouse_over_playlist {
                    if let Some(idx) = self.playlist_manager.playing_song_idx {
                        let scroll_task = self.execute_playlist_autoscroll(idx);
                        return Task::batch(vec![wake_task, scroll_task]);
                    }
                }
                wake_task
            }
            Message::PreviousTrack => {
                crate::utils::covers::clear_raw_cache();
                self.playlist_manager.play_prev(&self.audio_manager);
                self.persist_playlist_state();

                if self.playlist_manager.shuffle_active {
                    if let Some(session) = &self.playlist_manager.shuffle_session {
                        // db-lock: persistencia — el guardado se omite si el lock falla
                        if let Ok(mut db) = self.database.lock() {
                            log_persist(
                                "shuffle_session",
                                db.save_shuffle_session(
                                    self.playlist_manager.active_playlist_id,
                                    session,
                                ),
                            );
                        } else {
                            tracing::warn!("db-lock: lock no disponible");
                        }
                    }
                }

                self.sync_player_art();
                let wake_task = self.wake_up_controls(false);
                if !self.is_mouse_over_playlist {
                    if let Some(idx) = self.playlist_manager.playing_song_idx {
                        let scroll_task = self.execute_playlist_autoscroll(idx);
                        return Task::batch(vec![wake_task, scroll_task]);
                    }
                }
                wake_task
            }
            Message::Stop => {
                self.audio_manager.stop();
                let _ = self.audio_manager.purge_buffers(); // Purga profunda al detener
                Task::none()
            }
            Message::VolumeChanged(vol) => {
                self.audio_manager.set_volume(vol);
                // Persistir el volumen entre reinicios
                // db-lock: persistencia — el guardado se omite si el lock falla
                if let Ok(db) = self.database.lock() {
                    log_persist(
                        "player_volume",
                        db.set_setting("player_volume", &format!("{:.4}", vol)),
                    );
                } else {
                    tracing::warn!("db-lock: lock no disponible");
                }
                Task::none()
            }
            Message::SeekTo(pos) => {
                // Con la duración indeterminada no hay posición a la que saltar:
                // se ignoran todos los orígenes de búsqueda fina (barra,
                // teclado y controles multimedia externos).
                if !fine_seek_enabled(state.total_duration_sec) {
                    return Task::none();
                }
                self.audio_manager.seek(pos as f64);
                self.persist_playlist_state();
                self.wake_up_controls(false)
            }
            Message::SetPlaying(playing) => {
                self.audio_manager.set_playing(playing);

                // Al pausar, purgar buffers para liberar RAM (mismo criterio que PlayPause).
                if !playing {
                    tracing::debug!("Audoxidy Audio: Cleaning Buffers on Pause (Memory Recovery).");
                    let _ = self.audio_manager.purge_buffers();
                }

                self.persist_playlist_state();
                self.wake_up_controls(false)
            }
            Message::ExternalControl(event) => {
                // El reflejo del volumen hacia el panel solo responde a un
                // evento entrante de volumen (nunca desde `update`), para no
                // realimentar el bucle.
                if let ExternalControlEvent::SetVolume(volume) = event {
                    self.media_controls.set_volume(clamp_volume(volume) as f64);
                }
                let message = external_control_to_message(
                    event,
                    state.current_pos_sec,
                    state.total_duration_sec,
                    state.is_playing,
                );
                self.update(message)
            }
            Message::ToggleRepeat => {
                self.playlist_manager.repeat_mode = (self.playlist_manager.repeat_mode + 1) % 3;
                self.persist_playlist_state();
                Task::none()
            }
            Message::ToggleShuffle => {
                if !self.playlist_manager.shuffle_active {
                    self.playlist_manager.activate_shuffle();
                    if let Some(session) = &self.playlist_manager.shuffle_session {
                        // db-lock: persistencia — el guardado se omite si el lock falla
                        if let Ok(mut db) = self.database.lock() {
                            log_persist(
                                "shuffle_session",
                                db.save_shuffle_session(
                                    self.playlist_manager.active_playlist_id,
                                    session,
                                ),
                            );
                        } else {
                            tracing::warn!("db-lock: lock no disponible");
                        }
                    }
                } else {
                    self.playlist_manager.deactivate_shuffle();
                    // db-lock: persistencia — el guardado se omite si el lock falla
                    if let Ok(db) = self.database.lock() {
                        log_persist(
                            "shuffle_session",
                            db.clear_shuffle_session(self.playlist_manager.active_playlist_id),
                        );
                    } else {
                        tracing::warn!("db-lock: lock no disponible");
                    }
                }
                self.persist_playlist_state();
                Task::none()
            }
            _ => Task::none(),
        }
    }

    /// Helper centralizado que carga un archivo de audio con sus valores de ReplayGain.
    /// Busca track_gain y album_gain en la base de datos por ruta y los pasa al motor.
    pub(in crate::gui::app) fn load_file_with_gains(
        &self,
        path: &str,
        title: impl Into<String>,
        artist: impl Into<String>,
        album: impl Into<String>,
        cover_path: Option<String>,
    ) -> Result<(), AudioError> {
        // db-lock: solo lectura — refresco best-effort, se omite si el lock falla
        let (track_gain, album_gain) = if let Ok(db) = self.database.lock() {
            match db.get_replay_gain_by_path(path) {
                Ok(gains) => gains,
                Err(e) => {
                    tracing::error!("Error consultando ReplayGain en BD: {}", e);
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        if track_gain.is_none() && album_gain.is_none() {
            tracing::debug!("DB: No se encontró ReplayGain para la ruta: {}", path);
        }

        self.audio_manager.load_file(
            path, title, artist, album, cover_path, track_gain, album_gain,
        )
    }

    /// Reproduce una canción de la playlist por índice. Si es la MISMA canción que
    /// ya se está reproduciendo, la reinicia desde el inicio (seek a 0) en lugar de
    /// re-cargarla — el dedup del decoder ignoraría un Load del mismo path.
    pub(in crate::gui::app) fn play_song_from_playlist(&mut self, idx: usize) {
        let Some(song) = self.playlist_manager.get_song_at_linear_index(idx) else {
            return;
        };
        let path = song.file_path.clone();
        let title = song.title.clone();
        let artist = song.artist_name.to_string();
        let album = song.album_title.to_string();
        let cover_path = song.cover_path.as_deref().map(str::to_string);
        let s_id = song.song_id;

        let current = self.audio_manager.get_state();
        let is_same_song = current.path.as_str() == path.as_ref() && current.is_playing;

        self.sync_player_art();
        if is_same_song {
            // Reiniciar la canción actual desde el principio.
            self.audio_manager.seek(0.0);
            self.audio_manager.play();
        } else if let Err(e) = self.load_file_with_gains(
            &path,
            title.to_string(),
            artist.to_string(),
            album,
            cover_path,
        ) {
            tracing::error!("Error reproduciendo archivo: {}", e);
            return;
        } else {
            self.audio_manager.play();
            self.playlist_manager.playing_song_idx = Some(idx);
            self.playlist_manager.notify_manual_play(s_id);
        }
        self.persist_playlist_state();
    }

    pub(in crate::gui::app) fn sync_player_art(&mut self) {
        let audio_state = self.audio_manager.get_state();

        // 0. No hacer nada si el audio está en transición (path vacío temporalmente)
        if audio_state.path.is_empty() {
            return;
        }

        // 1. Verificar si la pista ha cambiado realmente
        if self.player_ui_state.current_art_id == audio_state.path {
            return;
        }

        // 2. Obtener la ruta AVIF de la canción actual desde la playlist.
        //    La canción debe coincidir con el path del motor. Si el índice aún no
        //    apunta a la canción correcta (p. ej. justo tras la promoción de la
        //    pre-carga en el decoder, antes de que la GUI avance el índice), NO
        //    marcamos current_art_id: un tick posterior reintentará cuando el
        //    índice ya apunte a la canción real.
        let mut new_cover_path = String::new();
        let mut song_found = false;
        if let Some(idx) = self.playlist_manager.playing_song_idx {
            if let Some(song) = self.playlist_manager.get_song_at_linear_index(idx) {
                if song.file_path.as_ref() == audio_state.path {
                    song_found = true;
                    if let Some(ref cp) = song.cover_path {
                        new_cover_path = cp.to_string();
                    }
                }
            }
        }
        if !song_found {
            return;
        }

        // 3. Reusar carátula si es el MISMO álbum (misma ruta de caché)
        if !new_cover_path.is_empty() && new_cover_path == self.player_ui_state.current_cover_path {
            // Mismo álbum — reusar el handle actual, solo actualizar el art_id
            self.player_ui_state.current_art_id = audio_state.path.clone();
            self.player_ui_state.prefetched_next_handle = None;
            return;
        }

        // 4. Usar handle precargado si está disponible
        if let Some(prefetched) = self.player_ui_state.prefetched_next_handle.take() {
            self.player_ui_state.cached_art_handle = Some(prefetched);
            self.player_ui_state.current_art_id = audio_state.path.clone();
            self.player_ui_state.current_cover_path = new_cover_path;
            return;
        }

        // 5. Cargar desde disco (Handle::from_path — sin caché RAM)
        if !new_cover_path.is_empty() {
            if let Some(handle) = crate::utils::covers::load_cover_handle(&new_cover_path) {
                self.player_ui_state.cached_art_handle = Some(handle);
                self.player_ui_state.current_art_id = audio_state.path.clone();
                self.player_ui_state.current_cover_path = new_cover_path;
                return;
            }
        }

        // 6. Fallback final (Sin carátula)
        self.player_ui_state.cached_art_handle = None;
        self.player_ui_state.current_art_id = audio_state.path.clone();
        self.player_ui_state.current_cover_path.clear();
    }
}
