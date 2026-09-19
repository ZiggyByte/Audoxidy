//! CoreEvent dispatch handler.
//!
//! Maps edge-transition events from the Core domain layer to GUI state
//! updates and iced tasks. Each variant handles one specific transition.

use crate::core::CoreEvent;
use crate::gui::app::Message;
use iced::Task;

impl crate::gui::app::AudoxidyApp {
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
                // Trigger global memory purge through the existing handler.
                Task::done(Message::GlobalMemoryPurge)
            }
        }
    }
}
