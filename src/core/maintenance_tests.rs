//! Tests for the GC policy and the memory-purge intents.
//!
//! The timer test drives the process-global purge timer, so it takes the lock
//! that serializes every module touching that atomic and holds it for its whole
//! body; the remaining tests are decided by the scan and preload guards and
//! return before the timer is consulted, so they take no lock. Every test builds
//! its `Core` over an in-memory database, so no test opens the application
//! database.

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
    let audio_manager = Arc::new(crate::audio::AudioManager::new().expect("audio manager init"));
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
    let _timer_guard = crate::utils::memory_manager::PURGE_TIMER_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
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
