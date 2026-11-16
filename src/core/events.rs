//! Core event stream for edge transitions.
//!
//! Core emits events on a bounded crossbeam channel when significant
//! state changes occur. The GUI consumes these through an iced
//! subscription with a hash-stable recipe (the same pattern used by
//! the MPRIS external-control bridge in `integrations/control.rs`).

/// Edge-transition events that Core emits.
///
/// These represent state changes that the GUI needs to react to.
/// Hot UI reads (playback progress, VU meter) stay synchronous and
/// are not eventized — only edge transitions appear here.
#[derive(Debug, Clone)]
pub enum CoreEvent {
    /// A library scan has started.
    ScanStarted,
    /// Progress update during a scan.
    ///
    /// Reserved and not emitted: the scanner reports progress by writing its
    /// `is_scanning`/`db_dirty` flags, which the scan tick reads, so no
    /// producer sends this variant today and the channel round-trip test is
    /// its only sender. It is retained as the Core integration surface for a
    /// future scanner progress hook and for non-desktop front-ends.
    ScanProgress {
        /// Number of files processed so far.
        files_processed: usize,
    },
    /// A library scan completed.
    ScanCompleted {
        /// Number of new songs added to the database.
        songs_added: usize,
    },
    /// The library data has been modified and the GUI should refresh.
    LibraryDirty,
    /// The currently playing track has changed.
    TrackChanged {
        /// Path of the new track.
        path: String,
    },
    /// The system is under memory pressure.
    MemoryPressure,
}

/// Creates a bounded channel pair for Core events.
///
/// The channel is bounded to 64 entries (matching the audio command
/// channel pattern) so a slow GUI consumer does not cause unbounded
/// memory growth. Returns the sender (owned by Core) and the receiver
/// (owned by the GUI subscription).
pub fn core_event_channel() -> (
    crossbeam::channel::Sender<CoreEvent>,
    crossbeam::channel::Receiver<CoreEvent>,
) {
    crossbeam::channel::bounded(64)
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
