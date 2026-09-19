//! Core struct that owns the shared Arc handles and constructs services.
//!
//! `Core` is the sole owner of orchestration over the audio manager,
//! the database, and the scanner. It owns policy, persistence ordering,
//! engine call sequencing, and shared domain state.
//!
//! The GUI receives an `Arc<Core>` at construction and calls its methods
//! instead of reaching `AudioManager` or `Database` directly.

use std::sync::{Arc, Mutex};

use crate::audio::AudioManager;
use crate::db::Database;
use crate::db::scanner::Scanner;

use super::error::CoreError;
use super::events::{CoreEvent, core_event_channel};
use super::types::CorePaths;

/// The Core domain layer.
///
/// Owns the Arc handles to the audio manager, database, and scanner.
/// Exposes synchronous, non-blocking commands and an event stream.
/// Never returns iced types or constructs iced widgets.
pub struct Core {
    pub(crate) audio_manager: Arc<AudioManager>,
    pub(crate) database: Arc<Mutex<Database>>,
    pub(crate) scanner: Arc<Scanner>,
    pub(crate) event_tx: crossbeam::channel::Sender<CoreEvent>,
    pub(crate) event_rx: crossbeam::channel::Receiver<CoreEvent>,
    pub(crate) paths: CorePaths,
}

impl Core {
    /// Creates a new Core instance.
    ///
    /// The event channel is created internally. The returned `Core`
    /// owns the sender; the receiver is accessible via [`events()`](Self::events).
    pub fn new(
        audio_manager: Arc<AudioManager>,
        database: Arc<Mutex<Database>>,
        scanner: Arc<Scanner>,
        paths: CorePaths,
    ) -> Self {
        let (event_tx, event_rx) = core_event_channel();
        Self {
            audio_manager,
            database,
            scanner,
            event_tx,
            event_rx,
            paths,
        }
    }

    /// Returns a clone of the event-stream receiver for a front-end subscription.
    ///
    /// The channel is multi-consumer: every clone shares the same queue, so a
    /// front-end can subscribe without taking ownership away from Core.
    pub fn event_receiver(&self) -> crossbeam::channel::Receiver<CoreEvent> {
        self.event_rx.clone()
    }

    /// Returns a reference to the injected filesystem paths.
    pub fn paths(&self) -> &CorePaths {
        &self.paths
    }

    /// Returns a reference to the audio manager.
    pub fn audio_manager(&self) -> &Arc<AudioManager> {
        &self.audio_manager
    }

    /// Returns a reference to the database.
    pub fn database(&self) -> &Arc<Mutex<Database>> {
        &self.database
    }

    /// Acquires the database lock, mapping a poisoned lock to a typed error.
    ///
    /// Service methods use this helper so a poisoned lock surfaces as
    /// [`CoreError::LockUnavailable`] instead of panicking.
    pub(crate) fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, Database>, CoreError> {
        self.database.lock().map_err(|_| CoreError::LockUnavailable)
    }

    /// Returns a reference to the scanner.
    pub fn scanner(&self) -> &Arc<Scanner> {
        &self.scanner
    }

    /// Sends an event on the Core event channel.
    ///
    /// If the receiver has been dropped, the event is logged and
    /// discarded — the sender never panics.
    pub fn emit(&self, event: CoreEvent) {
        if let Err(e) = self.event_tx.send(event) {
            tracing::warn!("core event channel disconnected; event discarded: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::CorePaths;

    #[test]
    fn test_core_constructs_with_default_paths() {
        let paths = CorePaths::desktop_default();
        let db = Arc::new(Mutex::new(Database::default()));
        let scanner = Arc::new(Scanner::new(db.clone()));
        let audio_manager = Arc::new(AudioManager::new().expect("audio manager init"));

        let core = Core::new(audio_manager, db, scanner, paths.clone());
        assert_eq!(core.paths().db_path, paths.db_path);
        assert_eq!(core.paths().cover_cache_dir, paths.cover_cache_dir);
        assert_eq!(core.paths().config_path, paths.config_path);
    }
}
