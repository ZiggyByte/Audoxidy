//! Maintenance service: GC policy, memory purge, and playback guards.
//!
//! All methods return plain data or [`CoreIntent`] values — never iced types.
//! The GUI adapter maps [`CoreIntent`] to its own `Task<Message>` at the
//! boundary, preserving the iced-free invariant of `src/core/`.
//!
//! Lock order compliance: every method that touches the database follows
//! acquire→copy→release→return. No lock is held across audio manager calls.

use super::Core;

/// Intent returned by maintenance checks for the GUI to act on.
///
/// The GUI adapter maps each variant to a `Task<Message>` at the boundary.
/// Core never constructs iced types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreIntent {
    /// The periodic GC or RAM pressure check fired — run the global purge.
    PurgeMemory,
    /// Force the allocator to release free pages to the OS.
    ForceFreeToOs,
}

impl Core {
    /// Checks whether the periodic GC timer has fired.
    ///
    /// Delegates to [`MemoryManager::should_run_global_purge`] which uses a
    /// fixed 60-second interval and respects the scanning guard. Returns
    /// `Some(CoreIntent::PurgeMemory)` when the cycle elapses, `None`
    /// otherwise.
    pub fn check_gc(&self, is_scanning: bool) -> Option<CoreIntent> {
        if crate::utils::memory_manager::MemoryManager::should_run_global_purge(0, is_scanning) {
            Some(CoreIntent::PurgeMemory)
        } else {
            None
        }
    }

    /// Evaluates the GC timer and returns an intent if a purge is due.
    ///
    /// `preload_secs_remaining` is the seconds before the next preload
    /// fires; when non-`None` and positive, the GC is suppressed to avoid
    /// evicting data the preload will need. Returns `Some(PurgeMemory)` when
    /// the 60-second cycle elapses and no preload guard is active, or
    /// `Some(PurgeMemory)` when system RAM exceeds the 75% hard cap.
    pub fn evaluate_gc(
        &self,
        is_scanning: bool,
        preload_secs_remaining: Option<f64>,
    ) -> Option<CoreIntent> {
        // During scans the GC is suppressed entirely.
        if is_scanning {
            return None;
        }

        // If a preload is imminent (≤20 s away), suppress the GC to avoid
        // evicting preloaded data.
        if preload_secs_remaining.is_some_and(|r| r > 0.0 && r <= 20.0) {
            return None;
        }

        // Check the periodic 60-second timer.
        if crate::utils::memory_manager::MemoryManager::should_run_global_purge(0, is_scanning) {
            return Some(CoreIntent::PurgeMemory);
        }

        // Check RAM pressure (system > 75% or app > 500 MB).
        if crate::utils::memory_manager::MemoryManager::should_purge_for_ram() {
            return Some(CoreIntent::PurgeMemory);
        }

        None
    }

    /// Runs the global memory purge and returns the allocator release intent.
    ///
    /// The GUI adapter should call `force_free_to_os()` after handling
    /// `PurgeMemory` to release freed pages back to the OS.
    pub fn execute_global_purge() -> CoreIntent {
        crate::utils::memory_manager::MemoryManager::force_free_to_os();
        CoreIntent::ForceFreeToOs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Core;
    use crate::core::types::CorePaths;
    use crate::db::Database;
    use crate::db::scanner::Scanner;
    use std::sync::{Arc, Mutex};

    fn make_core_for_test() -> Core {
        let paths = CorePaths::desktop_default();
        let db = Arc::new(Mutex::new(Database::new_memory().expect("in-memory db")));
        let scanner = Arc::new(Scanner::new(db.clone()));
        let audio_manager =
            Arc::new(crate::audio::AudioManager::new().expect("audio manager init"));
        Core::new(audio_manager, db, scanner, paths)
    }

    #[test]
    fn test_core_intent_variants() {
        let purge = CoreIntent::PurgeMemory;
        let free = CoreIntent::ForceFreeToOs;
        assert_ne!(purge, free);
        assert_eq!(purge, CoreIntent::PurgeMemory);
        assert_eq!(free, CoreIntent::ForceFreeToOs);
    }

    #[test]
    fn test_evaluate_gc_suppressed_during_scan() {
        let core = make_core_for_test();
        // During a scan, the GC should never fire.
        assert!(core.evaluate_gc(true, None).is_none());
    }

    #[test]
    fn test_evaluate_gc_suppressed_with_preload_guard() {
        let core = make_core_for_test();
        // With a preload 10 seconds away, the GC is suppressed.
        assert!(core.evaluate_gc(false, Some(10.0)).is_none());
    }

    #[test]
    fn test_execute_global_purge_returns_force_free() {
        let intent = Core::execute_global_purge();
        assert_eq!(intent, CoreIntent::ForceFreeToOs);
    }
}
