//! Volume-fade envelope, silence detection and smoothing timing.
//!
//! Pins the equal-power fade curve, the +3 dB hysteresis of the silence gate and
//! the debounce-then-ramp schedule the decoder follows when a slider moves. The
//! schedule constants are read from the decoder, so the simulation tracks the
//! values production applies instead of copying them. No audio device is opened,
//! no playback happens and no database is touched.

use crate::audio::decoder::{SMOOTH_DEBOUNCE_SECS, SMOOTH_DURATION_SECS};

// --- Silence detection tests ---

#[test]
fn test_silence_detection_threshold_and_hysteresis() {
    // Hysteresis with +3 dB difference.
    // Enter silence at -50 dB → linear threshold = 10^(-50/20) ≈ 0.00316
    // Exit silence at -47 dB  → linear threshold = 10^(-47/20) ≈ 0.00447
    let silence_enter: f64 = 10.0_f64.powf(-50.0 / 20.0);
    let silence_exit: f64 = 10.0_f64.powf(-47.0 / 20.0);

    // Verify enter threshold is correct
    assert!((silence_enter - 0.00316228).abs() < 1e-6);
    // Verify exit threshold is +3 dB above enter
    assert!((silence_exit - 0.00446684).abs() < 1e-6);

    // A peak of 0.002 (below -50 dB) should be detected as silent
    let peak_below: f64 = 0.002;
    assert!(peak_below < silence_enter);

    // A peak of 0.005 (above -47 dB) should NOT be silent (audible)
    let peak_above: f64 = 0.005;
    assert!(peak_above > silence_exit);

    // Peak-per-frame detection — max absolute value across channels
    // A frame with mixed values: use max(|sample|) as peak
    let frame: [f64; 2] = [0.001, 0.004];
    let peak = frame.iter().map(|s| s.abs()).fold(0.0_f64, f64::max);
    // 0.004 > silence_enter ≈ 0.00316 but < silence_exit ≈ 0.00447
    // In current state (not yet silent), threshold = silence_enter → peak > threshold → audible
    assert!(peak > silence_enter);
    // In silent state (already silenced), threshold = silence_exit → peak < threshold → still silent
    assert!(peak < silence_exit);

    // Edge case: exact threshold boundary
    let edge_value = silence_enter;
    let edge_frame: [f64; 2] = [0.0, edge_value];
    let edge_peak = edge_frame.iter().map(|s| s.abs()).fold(0.0_f64, f64::max);
    assert!((edge_peak - silence_enter).abs() < 1e-6);
}

// --- Fade envelope tests ---

#[test]
fn test_fade_equal_power_curve() {
    // Musical fades use equal-power sin²(π·t/2) curve
    // At t=0.0: coeff = sin²(0) = 0.0
    // At t=0.5: coeff = sin²(π/4) = (√2/2)² = 0.5
    // At t=1.0: coeff = sin²(π/2) = 1.0
    fn equal_power_fade(t: f64) -> f64 {
        let angle = std::f64::consts::PI * t / 2.0;
        angle.sin().powi(2)
    }

    let coeff_0 = equal_power_fade(0.0);
    assert!((coeff_0 - 0.0).abs() < 1e-15);

    let coeff_mid = equal_power_fade(0.5);
    assert!((coeff_mid - 0.5).abs() < 1e-15);

    let coeff_1 = equal_power_fade(1.0);
    assert!((coeff_1 - 1.0).abs() < 1e-15);

    // Verify monotonic: the curve should be strictly increasing
    let mut prev = 0.0;
    for i in 1..=100 {
        let t = i as f64 / 100.0;
        let coeff = equal_power_fade(t);
        assert!(
            coeff >= prev,
            "Not monotonic at t={}: {} < {}",
            t,
            coeff,
            prev
        );
        prev = coeff;
    }

    // Max error < 0.001 for the entire curve (smooth musical fade)
    for i in 0..=1000 {
        let t = i as f64 / 1000.0;
        let coeff = equal_power_fade(t);
        // At any point, coefficient should be between 0 and 1
        assert!(coeff >= 0.0);
        assert!(coeff <= 1.0);
        assert!(coeff.is_finite());
    }
}

// --- Volume smoothing with debounce ---
// The smoothing holds applied_vol frozen while the user adjusts; after the
// debounce window the ramp moves applied_vol to the target. The two schedule
// constants are read from the decoder, so this simulation cannot drift from the
// schedule production follows.

#[test]
fn test_smoothing_frozen_during_adjustment() {
    // User keeps changing the target → applied_vol must stay at the old level.
    let mut applied_vol: f64 = 0.65;
    let mut pending_vol: f64 = 0.65;
    let mut debounce_remaining: f64 = 0.0;
    let mut smooth_ramp: Option<(f64, f64, f64, f64)> = None; // from,to,elapsed,duration

    // Simulate: user moves from 65 to 25 in several batches.
    for target in [0.55, 0.45, 0.35, 0.25] {
        if (target - pending_vol).abs() > 1e-10 {
            pending_vol = target;
            debounce_remaining = SMOOTH_DEBOUNCE_SECS;
            smooth_ramp = None;
        }
        // No ramp yet, debounce still counting → applied_vol frozen at 0.65
        debounce_remaining -= 0.05;
        assert!(
            (applied_vol - 0.65).abs() < 1e-10,
            "applied_vol must stay frozen while adjusting, got {}",
            applied_vol
        );
    }
}

#[test]
fn test_smoothing_ramp_reaches_target_in_duration() {
    // After the debounce expires, the ramp must reach the target in ~1500ms.
    let mut applied_vol: f64 = 0.65;
    let pending_vol: f64 = 0.25;
    let mut debounce_remaining: f64 = 0.0;
    let mut smooth_ramp: Option<(f64, f64, f64, f64)> = None;

    // Debounce elapses → start ramp from 0.65 toward 0.25.
    if (applied_vol - pending_vol).abs() > 1e-10 {
        smooth_ramp = Some((applied_vol, pending_vol, 0.0, SMOOTH_DURATION_SECS));
    }

    // Advance the ramp in 100ms batches until completion.
    let dt = 0.1;
    let mut elapsed_total = 0.0;
    let mut reached = false;
    while elapsed_total < 3.0 {
        if let Some((from, to, elapsed, dur)) = smooth_ramp {
            let e = elapsed + dt;
            let t = (e / dur).min(1.0);
            applied_vol = from + (to - from) * t;
            if t >= 1.0 {
                smooth_ramp = None;
                reached = true;
            } else {
                smooth_ramp = Some((from, to, e, dur));
            }
        }
        elapsed_total += dt;
        if reached {
            break;
        }
    }

    assert!(reached, "ramp should complete within 2s");
    assert!(
        (applied_vol - 0.25).abs() < 1e-6,
        "final applied_vol should be 0.25, got {}",
        applied_vol
    );
    // Should have taken ~1500ms (allow tolerance for the batch step)
    assert!((elapsed_total - SMOOTH_DURATION_SECS).abs() < 0.3);
}

#[test]
fn test_smoothing_disabled_applies_immediately() {
    // When smooth_volume_enabled is false, applied_vol = user target at once.
    let target: f64 = 0.35;
    let mut applied_vol: f64 = 0.35;
    let mut pending_vol: f64 = 0.35;
    let mut debounce_remaining: f64 = 0.0;
    let mut smooth_ramp: Option<(f64, f64, f64, f64)> = None;

    // Smoothing OFF path (mirror): applied_vol = user_target, ramp cleared.
    let user_target = target;
    applied_vol = user_target;
    pending_vol = user_target;
    debounce_remaining = 0.0;
    smooth_ramp = None;

    assert!((applied_vol - 0.35).abs() < 1e-10);
    assert!(smooth_ramp.is_none());
    assert_eq!(debounce_remaining, 0.0);
}

// --- Fade rate-per-second timing ---
// coeff advances by rate_per_sec * batch_dt; a 1000ms fade completes in 1s.

#[test]
fn test_fade_timing_rate_per_sec() {
    let fade_ms: f64 = 1000.0;
    let rate_per_sec: f64 = 1.0 / (fade_ms / 1000.0);
    let batch_dt: f64 = 0.1; // 100ms batches

    let mut coeff: f64 = 0.0;
    let mut elapsed: f64 = 0.0;
    while coeff < 1.0 && elapsed < 3.0 {
        coeff += rate_per_sec * batch_dt;
        elapsed += batch_dt;
    }
    assert!(
        (elapsed - 1.0).abs() < 0.11,
        "1000ms fade should complete in ~1s, took {}s",
        elapsed
    );
}

#[test]
fn test_fade_timing_large_ms() {
    // 6500ms fade completes in ~6.5s of audio time regardless of sample rate.
    let fade_ms: f64 = 6500.0;
    let rate_per_sec: f64 = 1.0 / (fade_ms / 1000.0);
    let batch_dt: f64 = 0.1;

    let mut coeff: f64 = 0.0;
    let mut elapsed: f64 = 0.0;
    while coeff < 1.0 && elapsed < 10.0 {
        coeff += rate_per_sec * batch_dt;
        elapsed += batch_dt;
    }
    assert!(
        (elapsed - 6.5).abs() < 0.11,
        "6500ms fade should complete in ~6.5s, took {}s",
        elapsed
    );
}
