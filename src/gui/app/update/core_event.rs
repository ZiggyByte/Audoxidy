//! CoreEvent dispatch handler.
//!
//! Maps edge-transition events from the Core domain layer to GUI state
//! updates and iced tasks. Each variant handles one specific transition.
//! [`CoreEvent::MemoryPressure`] is informational only: the tick's
//! `CoreIntent` is the single purge trigger, so this handler never dispatches
//! a purge of its own.

use crate::core::CoreEvent;
use crate::gui::app::Message;
use iced::Task;

impl crate::gui::app::AudoxidyApp {
    /// Applies one Core edge transition to the GUI state.
    ///
    /// Every variant returns the task the transition requires; a purely
    /// informational transition returns [`Task::none`].
    pub(in crate::gui::app) fn on_core_event(&mut self, event: CoreEvent) -> Task<Message> {
        match event {
            CoreEvent::ScanStarted => {
                self.was_scanning = true;
                Task::none()
            }
            CoreEvent::ScanProgress { files_processed: _ } => {
                // Scan progress indicator is updated via synchronous state reads.
                Task::none()
            }
            CoreEvent::ScanCompleted { songs_added: _ } => {
                self.db_needs_refresh = true;
                Task::none()
            }
            CoreEvent::LibraryDirty => {
                self.db_needs_refresh = true;
                Task::none()
            }
            CoreEvent::TrackChanged { path: _ } => {
                // Player UI already updates via synchronous state reads.
                Task::none()
            }
            CoreEvent::MemoryPressure => {
                // Informational edge transition: the GC purge is dispatched
                // synchronously from the tick when `evaluate_gc` reports it, so
                // reacting here would dispatch the same purge a second time.
                tracing::debug!("Core memory pressure event (informational)");
                Task::none()
            }
        }
    }
}
