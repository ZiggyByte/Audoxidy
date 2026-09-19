//! Maintenance service: GC policy, memory purge, and playback guards.
//!
//! All methods return plain data or [`CoreIntent`] values — never iced types.
//! The GUI adapter maps [`CoreIntent`] to its own `Task<Message>` at the
//! boundary, preserving the iced-free invariant of `src/core/`.
//!
//! Lock order compliance: every method that touches the database follows
//! acquire→copy→release→return. No lock is held across audio manager calls.

use super::Core;
use super::events::CoreEvent;

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

    /// Evaluates the GC policy against the caller's scan and preload guards.
    ///
    /// `is_scanning` suppresses the whole check while a scan runs.
    /// `preload_blocked` is the caller's preload guard — true while a preload is
    /// active or within its post-finish safety window — and suppresses the check
    /// as well. When neither guard is active, the fixed 60-second timer and the
    /// RAM thresholds are evaluated in the same order the GUI tick used inline.
    /// Either one being due returns [`CoreIntent::PurgeMemory`], so the tick
    /// dispatches a single synchronous purge and the dispatch route matches the
    /// inline behavior for both branches. RAM pressure additionally emits
    /// [`CoreEvent::MemoryPressure`] as an informational edge-transition
    /// notification; it carries no dispatch of its own.
    pub fn evaluate_gc(&self, is_scanning: bool, preload_blocked: bool) -> Option<CoreIntent> {
        if is_scanning || preload_blocked {
            return None;
        }

        let purge_timer =
            crate::utils::memory_manager::MemoryManager::should_run_global_purge(2, is_scanning);
        let ram_over = crate::utils::memory_manager::MemoryManager::should_purge_for_ram();

        if ram_over {
            self.emit(CoreEvent::MemoryPressure);
        }

        if purge_timer || ram_over {
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
    fn test_evaluate_gc_returns_purge_when_timer_due() {
        let core = make_core_for_test();
        // Back-date the periodic timer past its interval so the check is due:
        // the due branch must return the purge intent so the caller dispatches
        // it synchronously.
        crate::utils::memory_manager::MemoryManager::backdate_global_purge_timer_for_tests(120);
        assert_eq!(
            core.evaluate_gc(false, false),
            Some(CoreIntent::PurgeMemory)
        );
    }

    #[test]
    fn test_evaluate_gc_suppressed_during_scan() {
        let core = make_core_for_test();
        // During a scan, the GC should never fire.
        assert!(core.evaluate_gc(true, false).is_none());
    }

    #[test]
    fn test_evaluate_gc_suppressed_with_preload_guard() {
        let core = make_core_for_test();
        // With an active or recently finished preload, the GC is suppressed.
        assert!(core.evaluate_gc(false, true).is_none());
    }

    #[test]
    fn test_evaluate_gc_scan_guard_wins_over_preload() {
        let core = make_core_for_test();
        // Either guard suppresses the check, with no MemoryManager side effects.
        assert!(core.evaluate_gc(true, true).is_none());
    }

    #[test]
    fn test_execute_global_purge_returns_force_free() {
        let intent = Core::execute_global_purge();
        assert_eq!(intent, CoreIntent::ForceFreeToOs);
    }
}
