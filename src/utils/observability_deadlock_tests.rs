//! Deadlock watchdog smoke test.
//!
//! The module is compiled only when the `deadlock-detection` feature is on, which
//! is why its declaration keeps the feature gate: the default build carries no
//! parking_lot deadlock graph to inspect.

use super::spawn_deadlock_watchdog;

#[test]
fn deadlock_detector_smoke() {
    let cycles = parking_lot::deadlock::check_deadlock();
    assert!(
        cycles.is_empty(),
        "unexpected deadlock cycles in a single-threaded test"
    );
    spawn_deadlock_watchdog();
}
