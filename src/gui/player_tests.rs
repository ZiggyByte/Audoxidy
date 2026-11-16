//! Autosave scheduling for the player settings.
//!
//! The tests read the pure due predicate the bar calls: no audio device is
//! opened and no widget tree is built, so the first run, the configured
//! interval and a clock that moves backwards are asserted from values alone.

use super::*;

#[test]
fn autosave_due_first_run_persists() {
    assert!(autosave_due(1_000, None, 20));
}

#[test]
fn autosave_due_respects_interval() {
    assert!(!autosave_due(1000, Some(1000), 20));
    assert!(!autosave_due(1019, Some(1000), 20));
    assert!(autosave_due(1020, Some(1000), 20));
    assert!(autosave_due(1030, Some(1000), 20));
}

#[test]
fn autosave_due_handles_backwards_clock() {
    assert!(!autosave_due(999, Some(1000), 20));
}
