//! Playback service: transport, load-with-gains, EOF, preload, crossfade trigger.
//!
//! All conditions for EOF detection, preload trigger, and crossfade are
//! copied character-for-character from the GUI tick and playback handlers.
//! The GUI tick handler will call these methods instead of inlining the logic.
//!
//! Lock order compliance: every method that touches both DB and AudioManager
//! follows acquire→copy→release→call AudioManager. The DB lock is never held
//! across an audio manager call.

use super::Core;
use super::error::CoreError;

// ── Transport delegation ──────────────────────────────────────────

impl Core {
    /// Toggles between play and pause.
    pub fn toggle_play_pause(&self) {
        self.audio_manager.toggle_play_pause();
    }

    /// Stops playback and resets the engine state.
    pub fn stop(&self) {
        self.audio_manager.stop();
    }

    /// Seeks to a position in seconds.
    pub fn seek(&self, pos_sec: f64) {
        self.audio_manager.seek(pos_sec);
    }

    /// Sets the playback volume (0.0 to 1.0).
    pub fn set_volume(&self, volume: f32) {
        self.audio_manager.set_volume(volume);
    }

    /// Starts playback.
    pub fn play(&self) {
        self.audio_manager.play();
    }

    /// Returns whether the engine is currently playing.
    pub fn is_playing(&self) -> bool {
        self.audio_manager.is_playing()
    }

    /// Sets the playback state.
    pub fn set_playing(&self, playing: bool) {
        self.audio_manager.set_playing(playing);
    }

    /// Purges internal audio buffers and restarts the stream.
    pub fn purge_buffers(&self) -> Result<(), CoreError> {
        self.audio_manager
            .purge_buffers()
            .map_err(|e| CoreError::Audio(e.to_string()))
    }

    /// Releases preload memory without interrupting playback.
    pub fn purge_preload(&self) -> Result<(), CoreError> {
        self.audio_manager
            .purge_preload()
            .map_err(|e| CoreError::Audio(e.to_string()))
    }

    /// Clears the end-of-file flag.
    pub fn clear_eof(&self) {
        self.audio_manager.clear_eof();
    }

    /// Loads and plays an audio file with ReplayGain lookup.
    ///
    /// Lock discipline: acquires DB lock for ReplayGain query, copies
    /// the result, releases the lock, then delegates to AudioManager.
    /// The DB lock is never held across the audio manager call.
    pub fn load_and_play(
        &self,
        path: &str,
        title: &str,
        artist: &str,
        album: &str,
        cover_path: Option<String>,
    ) -> Result<(), CoreError> {
        // db-lock: read-only — ReplayGain lookup, best-effort
        let (track_gain, album_gain) = if let Ok(db) = self.database.lock() {
            match db.get_replay_gain_by_path(path) {
                Ok(gains) => gains,
                Err(e) => {
                    tracing::error!("Error querying ReplayGain in DB: {}", e);
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        if track_gain.is_none() && album_gain.is_none() {
            tracing::debug!("DB: No ReplayGain found for path: {}", path);
        }

        self.audio_manager
            .load_file(
                path, title, artist, album, cover_path, track_gain, album_gain,
            )
            .map_err(|e| CoreError::Audio(e.to_string()))
    }

    /// Preloads the next song on the decoder thread for gapless transitions.
    pub fn preload_next(
        &self,
        path: &str,
        title: &str,
        artist: &str,
        album: &str,
        cover_path: Option<String>,
    ) -> Result<(), CoreError> {
        self.audio_manager
            .preload_next(path, title, artist, album, cover_path, None, None)
            .map_err(|e| CoreError::Audio(e.to_string()))
    }

    /// Triggers a crossfade to the next song with the given duration.
    pub fn crossfade_next(&self, duration_ms: f64) -> Result<(), CoreError> {
        self.audio_manager
            .crossfade_next(duration_ms)
            .map_err(|e| CoreError::Audio(e.to_string()))
    }

    /// Persists the current shuffle session to the database.
    ///
    /// Acquires the DB lock, saves the session, releases the lock.
    /// Best-effort: a lock failure is logged and skipped.
    pub fn persist_shuffle_session(
        &self,
        playlist_id: i64,
        session: &crate::db::database::ShuffleSession,
    ) {
        // db-lock: persistence — save is skipped if lock fails
        if let Ok(mut db) = self.database.lock() {
            let result = db.save_shuffle_session(playlist_id, session);
            if let Err(e) = result {
                tracing::warn!(
                    target: "audoxidy::persistence",
                    key = "shuffle_session",
                    error = %e,
                    "Failed to persist shuffle session"
                );
            }
        } else {
            tracing::warn!("db-lock: lock not available");
        }
    }

    /// Persists a player volume setting to the database.
    pub fn persist_volume(&self, volume: f64) {
        // db-lock: persistence — save is skipped if lock fails
        if let Ok(db) = self.database.lock() {
            let result = db.set_setting("player_volume", &format!("{:.4}", volume));
            if let Err(e) = result {
                tracing::warn!(
                    target: "audoxidy::persistence",
                    key = "player_volume",
                    error = %e,
                    "Failed to persist volume"
                );
            }
        } else {
            tracing::warn!("db-lock: lock not available");
        }
    }
}

// ── Verbatim tick conditions ──────────────────────────────────────
//
// The following functions contain the exact conditions from the GUI tick
// and playback handlers, copied character-for-character. They accept
// parameters for GUI-specific state (preloaded_next_path, etc.) and
// return decisions the caller applies to its own UI state.

/// Result of the preload trigger check.
#[derive(Debug, Default, Clone)]
pub struct PreloadDecision {
    /// Whether the next song should be preloaded.
    pub should_preload: bool,
    /// The file path of the song to preload.
    pub next_path: Option<String>,
    /// Updated preloaded_next_path value for the caller's UI state.
    pub new_preloaded_path: Option<String>,
    /// Whether to clear preload_finished_at (preload is now active).
    pub clear_preload_finished: bool,
}

/// Verbatim preload trigger logic from the GUI tick handler.
///
/// Conditions (`remaining <= 15.0`, `preloaded_next_path` dedup) are
/// copied character-for-character. The caller applies the decision to
/// its own UI state and calls `preload_next` if `should_preload` is true.
pub fn check_preload_trigger(
    remaining_sec: f64,
    next_song_file_path: Option<&str>,
    preloaded_next_path: Option<&str>,
) -> PreloadDecision {
    let mut decision = PreloadDecision::default();

    // Prefetch de carátula y pre-carga de la siguiente canción
    // ~15s antes del final (transiciones sin cortes). El dedup
    // por handle/path evita repetir el trabajo en cada tick.
    if remaining_sec <= 15.0
        && let Some(next_path) = next_song_file_path
        && preloaded_next_path != Some(next_path)
    {
        // Pre-cargar la siguiente canción una sola vez por pista.
        decision.should_preload = true;
        decision.next_path = Some(next_path.to_string());
        decision.new_preloaded_path = Some(next_path.to_string());
        decision.clear_preload_finished = true;
    }

    decision
}

/// Verbatim crossfade trigger conditions from the GUI playback handler.
///
/// The conditions (`crossfade_enabled`, `crossfade_manual_enabled`,
/// `crossfade_manual_ms > 0.0`) are copied character-for-character.
pub fn check_crossfade_trigger(
    crossfade_enabled: bool,
    crossfade_manual_enabled: bool,
    crossfade_manual_ms: f32,
) -> bool {
    // Crossfade manual: si el grupo Crossfade está activo y su sub-función
    // manual está habilitada, se dispara la mezcla con la duración configurada
    // (el decoder mezcla la pista actual con la siguiente pre-cargada).
    crossfade_enabled && crossfade_manual_enabled && crossfade_manual_ms > 0.0
}

/// Information about the next song, needed for preload and crossfade.
#[derive(Debug, Default, Clone)]
pub struct NextSongInfo {
    /// File path of the next song.
    pub file_path: Option<String>,
    /// Title of the next song.
    pub title: Option<String>,
    /// Artist of the next song.
    pub artist: Option<String>,
    /// Album of the next song.
    pub album: Option<String>,
    /// Cover path of the next song.
    pub cover_path: Option<String>,
}

/// Verbatim manual next-track logic from the GUI playback handler.
///
/// If crossfade is enabled and manual crossfade is active, ensures the
/// next song is preloaded and triggers the crossfade mix. The preload
/// dedup via `preloaded_next_path` is preserved exactly.
pub fn manual_next_crossfade(
    audio_manager: &crate::audio::AudioManager,
    crossfade_enabled: bool,
    crossfade_manual_enabled: bool,
    crossfade_manual_ms: f32,
    next_song: &NextSongInfo,
    preloaded_next_path: Option<&str>,
) -> bool {
    if !check_crossfade_trigger(
        crossfade_enabled,
        crossfade_manual_enabled,
        crossfade_manual_ms,
    ) {
        return false;
    }

    if let (Some(path), Some(title), Some(artist), Some(album)) = (
        next_song.file_path.as_deref(),
        next_song.title.as_deref(),
        next_song.artist.as_deref(),
        next_song.album.as_deref(),
    ) {
        // Garantizar que la siguiente canción esté pre-cargada ANTES de
        // disparar la mezcla (el usuario puede saltar en cualquier momento,
        // no solo ~15s antes del final). Si ya está pre-cargada, el dedup
        // del decoder ignora la orden.
        if preloaded_next_path != Some(path) {
            let _ = audio_manager.preload_next(
                path,
                title.to_string(),
                artist.to_string(),
                album.to_string(),
                next_song.cover_path.clone(),
                None,
                None,
            );
        }
        let _ = audio_manager.crossfade_next(crossfade_manual_ms as f64);
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn make_core_for_test() -> Core {
        let paths = super::super::types::CorePaths::desktop_default();
        let db = Arc::new(Mutex::new(
            crate::db::Database::new_memory().expect("in-memory db"),
        ));
        let scanner = Arc::new(crate::db::scanner::Scanner::new(db.clone()));
        let audio_manager =
            Arc::new(crate::audio::AudioManager::new().expect("audio manager init"));
        Core::new(audio_manager, db, scanner, paths)
    }

    #[test]
    fn test_transport_delegates_to_audio_manager() {
        let core = make_core_for_test();
        // toggle_play_pause should not panic
        core.toggle_play_pause();
        // set_volume should not panic
        core.set_volume(0.5);
        // is_playing should return a bool
        let _ = core.is_playing();
    }

    #[test]
    fn test_clear_eof_does_not_panic() {
        let core = make_core_for_test();
        core.clear_eof();
    }

    #[test]
    fn test_stop_does_not_panic() {
        let core = make_core_for_test();
        core.stop();
    }

    #[test]
    fn test_preload_trigger_below_15s() {
        let decision = check_preload_trigger(10.0, Some("/path/to/song.flac"), None);
        assert!(decision.should_preload);
        assert_eq!(decision.next_path.as_deref(), Some("/path/to/song.flac"));
        assert!(decision.clear_preload_finished);
    }

    #[test]
    fn test_preload_trigger_no_dedup_when_same_path() {
        let decision =
            check_preload_trigger(10.0, Some("/path/to/song.flac"), Some("/path/to/song.flac"));
        assert!(!decision.should_preload);
    }

    #[test]
    fn test_preload_trigger_above_15s_no_preload() {
        let decision = check_preload_trigger(20.0, Some("/path/to/song.flac"), None);
        assert!(!decision.should_preload);
    }

    #[test]
    fn test_preload_trigger_no_next_song() {
        let decision = check_preload_trigger(10.0, None, None);
        assert!(!decision.should_preload);
    }

    #[test]
    fn test_crossfade_trigger_all_enabled() {
        assert!(check_crossfade_trigger(true, true, 2000.0));
    }

    #[test]
    fn test_crossfade_trigger_disabled() {
        assert!(!check_crossfade_trigger(false, true, 2000.0));
        assert!(!check_crossfade_trigger(true, false, 2000.0));
        assert!(!check_crossfade_trigger(true, true, 0.0));
    }

    #[test]
    fn test_manual_next_crossfade_skips_when_disabled() {
        let core = make_core_for_test();
        let next_song = NextSongInfo {
            file_path: Some("/path/to/song.flac".to_string()),
            title: Some("Title".to_string()),
            artist: Some("Artist".to_string()),
            album: Some("Album".to_string()),
            cover_path: None,
        };
        let triggered =
            manual_next_crossfade(&core.audio_manager, false, true, 2000.0, &next_song, None);
        assert!(!triggered);
    }
}
