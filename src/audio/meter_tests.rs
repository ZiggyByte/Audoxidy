//! Atomic round-trip tests for the meter data shared between the decoder and the UI.
//!
//! `MeterData` stores f32 dBFS values bit-reinterpreted into `AtomicU32`, so the tests
//! pin the silence default and the exact bits recovered for positive, zero, negative and
//! clipping values. No thread reads or writes the atomics: the test body is the only
//! writer and the only reader, so no decoder or GUI timer is involved.

use super::*;

#[test]
fn test_silence_initial_value() {
    let meter = MeterData::new();
    assert_eq!(meter.read_peak_l(), f32::NEG_INFINITY);
    assert_eq!(meter.read_rms_l(), f32::NEG_INFINITY);
    assert_eq!(meter.read_peak_r(), f32::NEG_INFINITY);
    assert_eq!(meter.read_rms_r(), f32::NEG_INFINITY);
}

#[test]
fn test_atomic_roundtrip() {
    let meter = MeterData::new();
    meter.write_peak_rms(
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    );
    assert_eq!(meter.read_peak_l(), f32::NEG_INFINITY);
}

#[test]
fn test_positive_value_roundtrip() {
    let meter = MeterData::new();
    meter.write_peak_rms(0.5, -10.0, -20.0, -30.0);
    assert_eq!(meter.read_peak_l(), 0.5);
}

#[test]
fn test_zero_roundtrip() {
    let meter = MeterData::new();
    meter.write_peak_rms(0.0, 0.0, 0.0, 0.0);
    assert_eq!(meter.read_peak_l(), 0.0);
    assert_eq!(meter.read_rms_r(), 0.0);
}

#[test]
fn test_clipping_value_roundtrip() {
    let meter = MeterData::new();
    meter.write_peak_rms(-6.0, -3.0, 0.0, 6.0);
    assert_eq!(meter.read_peak_l(), -6.0);
    assert_eq!(meter.read_rms_l(), -3.0);
    assert_eq!(meter.read_peak_r(), 0.0);
    assert_eq!(meter.read_rms_r(), 6.0);
}
