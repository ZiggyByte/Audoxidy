//! Tests for the tick taxonomy and the pure scheduling policy.
//!
//! They pin which cadences run for a given application state, the authorized
//! period of each kind, the message a kind emits, the recipe identity that
//! keeps two kinds with the same period distinct, and the fire counters. No
//! iced runtime is started and no window is opened.

use super::*;

#[test]
fn test_policy_keeps_frame_and_transport_while_playing() {
    let playing = tick_policy(true, false, true, true, false, false, false);
    assert_eq!(
        playing,
        TickSet {
            frame: true,
            ui: true,
            transport: true,
            scan: false,
            idle: false,
        }
    );

    // A scan running alongside playback stays on the transport cadence only.
    let playing_and_scanning = tick_policy(true, true, true, true, false, false, false);
    assert_eq!(playing_and_scanning, playing);
}

#[test]
fn test_policy_activates_scan_only_while_scanning() {
    let scanning = tick_policy(false, true, false, false, false, false, false);
    assert_eq!(
        scanning,
        TickSet {
            frame: false,
            ui: false,
            transport: false,
            scan: true,
            idle: false,
        }
    );
}

#[test]
fn test_policy_suppresses_scan_while_playing() {
    let set = tick_policy(true, true, false, false, false, false, false);
    assert_eq!(
        set,
        TickSet {
            frame: true,
            ui: false,
            transport: true,
            scan: false,
            idle: false,
        }
    );
}

#[test]
fn test_policy_idles_with_ui_activity_when_stopped() {
    let set = tick_policy(false, false, true, false, false, false, false);
    assert_eq!(
        set,
        TickSet {
            frame: false,
            ui: true,
            transport: false,
            scan: false,
            idle: true,
        }
    );
}

#[test]
fn test_policy_marquee_needs_playback() {
    // A long title alone does not wake the UI cadence.
    let stopped = tick_policy(false, false, false, true, false, false, false);
    assert!(!stopped.ui);
    assert!(stopped.idle);

    // Playing with a long title does.
    let playing = tick_policy(true, false, false, true, false, false, false);
    assert!(playing.ui);
}

#[test]
fn test_policy_meter_fade_keeps_frame_alive() {
    let set = tick_policy(false, false, false, false, false, true, false);
    assert!(set.frame);
    assert!(set.idle);
    assert!(!set.transport);
}

#[test]
fn test_policy_drag_keeps_frame_alive() {
    let set = tick_policy(false, false, false, false, true, false, false);
    assert!(set.frame);
    assert!(set.idle);
}

#[test]
fn test_policy_stepper_hold_keeps_frame_alive() {
    let set = tick_policy(false, false, false, false, false, false, true);
    assert!(set.frame);
    assert!(set.idle);
}

#[test]
fn test_cadences_match_authorized_values() {
    assert_eq!(TickKind::Frame.cadence(), Duration::from_millis(33));
    assert_eq!(TickKind::Ui.cadence(), Duration::from_millis(500));
    assert_eq!(TickKind::Transport.cadence(), Duration::from_millis(500));
    assert_eq!(TickKind::Scan.cadence(), Duration::from_millis(500));
    assert_eq!(TickKind::Idle.cadence(), Duration::from_millis(5000));
}

#[test]
fn test_each_kind_maps_to_its_own_message() {
    assert!(matches!(TickKind::Frame.message(), Message::FrameTick));
    assert!(matches!(TickKind::Ui.message(), Message::UiTick));
    assert!(matches!(
        TickKind::Transport.message(),
        Message::TransportTick
    ));
    assert!(matches!(TickKind::Scan.message(), Message::ScanTick));
    assert!(matches!(TickKind::Idle.message(), Message::IdleTick));
}

#[test]
fn test_active_kinds_lists_kinds_in_canonical_order() {
    let set = tick_policy(true, false, true, true, false, false, false);
    assert_eq!(
        set.active_kinds(),
        vec![
            (TickKind::Frame, Duration::from_millis(33)),
            (TickKind::Ui, Duration::from_millis(500)),
            (TickKind::Transport, Duration::from_millis(500)),
        ]
    );
}

#[test]
fn test_active_kinds_is_idle_only_when_nothing_else_is_due() {
    let set = tick_policy(false, false, false, false, false, false, false);
    assert_eq!(
        set.active_kinds(),
        vec![(TickKind::Idle, Duration::from_millis(5000))]
    );
}

#[test]
fn test_tick_identity_distinguishes_kinds_for_equal_periods() {
    let period = Duration::from_millis(500);
    assert_ne!(
        tick_identity(TickKind::Ui, period),
        tick_identity(TickKind::Transport, period)
    );
    assert_ne!(
        tick_identity(TickKind::Transport, period),
        tick_identity(TickKind::Scan, period)
    );
    assert_ne!(
        tick_identity(TickKind::Ui, period),
        tick_identity(TickKind::Ui, Duration::from_millis(5000))
    );
    assert_eq!(
        tick_identity(TickKind::Ui, period),
        tick_identity(TickKind::Ui, period)
    );
}

#[test]
fn test_record_fire_increments_only_its_own_kind() {
    // The counters are process-global, so the assertions compare deltas:
    // they stay valid even when another test records fires first. The
    // whole scenario lives in one test function so parallel test threads
    // cannot interleave counter mutations.
    for kind in TickKind::all_five() {
        let before = tick_fires();
        kind.record_fire();
        let after = tick_fires();

        for probe in TickKind::all_five() {
            let delta = fires_for(after, probe) - fires_for(before, probe);
            let expected = u64::from(probe == kind);
            assert_eq!(
                delta, expected,
                "recording {kind:?} must change only its own counter, not {probe:?}"
            );
        }

        // The aggregate snapshot and the per-kind accessor must agree.
        assert_eq!(fires_for(after, kind), kind.fires());
        assert_eq!(fires_for(after, kind), fires_for(before, kind) + 1);
    }
}

/// Reads one counter out of a snapshot, so the test can iterate the kinds.
fn fires_for(fires: TickFires, kind: TickKind) -> u64 {
    match kind {
        TickKind::Frame => fires.frame,
        TickKind::Ui => fires.ui,
        TickKind::Transport => fires.transport,
        TickKind::Scan => fires.scan,
        TickKind::Idle => fires.idle,
    }
}
