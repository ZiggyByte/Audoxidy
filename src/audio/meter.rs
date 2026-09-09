use std::sync::atomic::{AtomicU32, Ordering};

/// Lock-free atomic meter data for real-time peak and RMS levels.
///
/// Transfer mechanism from decoder thread to GUI: 4 `AtomicU32` fields store
/// f32 dBFS values bit-reinterpreted via `f32::to_bits()` / `f32::from_bits()`.
/// The decoder thread writes with `Ordering::Relaxed` (single-writer);
/// the GUI reads with `Ordering::Relaxed` (single-reader, stale OK at ~30ms).
///
/// No allocations on the hot path — this struct is purely stack-allocated atomic stores.
pub struct MeterData {
    /// Peak level for left channel in dBFS
    peak_l: AtomicU32,
    /// RMS level for left channel in dBFS
    rms_l: AtomicU32,
    /// Peak level for right channel in dBFS
    peak_r: AtomicU32,
    /// RMS level for right channel in dBFS
    rms_r: AtomicU32,
}

impl MeterData {
    /// Creates a new `MeterData` with all fields initialized to silence (`-inf` dBFS).
    pub fn new() -> Self {
        Self {
            peak_l: AtomicU32::new(f32::NEG_INFINITY.to_bits()),
            rms_l: AtomicU32::new(f32::NEG_INFINITY.to_bits()),
            peak_r: AtomicU32::new(f32::NEG_INFINITY.to_bits()),
            rms_r: AtomicU32::new(f32::NEG_INFINITY.to_bits()),
        }
    }

    /// Stores peak and RMS values for both channels (dBFS, f32 via `to_bits`).
    ///
    /// This is the ONLY write method — called by the decoder thread after DSP
    /// processing, once per batch. Uses `Ordering::Relaxed` (no cross-field
    /// consistency needed — best-effort visual data).
    pub fn write_peak_rms(
        &self,
        peak_l_db: f32,
        rms_l_db: f32,
        peak_r_db: f32,
        rms_r_db: f32,
    ) {
        self.peak_l.store(peak_l_db.to_bits(), Ordering::Relaxed);
        self.rms_l.store(rms_l_db.to_bits(), Ordering::Relaxed);
        self.peak_r.store(peak_r_db.to_bits(), Ordering::Relaxed);
        self.rms_r.store(rms_r_db.to_bits(), Ordering::Relaxed);
    }

    /// Reads the peak level for the left channel in dBFS.
    pub fn read_peak_l(&self) -> f32 {
        f32::from_bits(self.peak_l.load(Ordering::Relaxed))
    }

    /// Reads the RMS level for the left channel in dBFS.
    pub fn read_rms_l(&self) -> f32 {
        f32::from_bits(self.rms_l.load(Ordering::Relaxed))
    }

    /// Reads the peak level for the right channel in dBFS.
    pub fn read_peak_r(&self) -> f32 {
        f32::from_bits(self.peak_r.load(Ordering::Relaxed))
    }

    /// Reads the RMS level for the right channel in dBFS.
    pub fn read_rms_r(&self) -> f32 {
        f32::from_bits(self.rms_r.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silence_initial_value() {
        let m = MeterData::new();
        assert_eq!(m.read_peak_l(), f32::NEG_INFINITY);
        assert_eq!(m.read_rms_l(), f32::NEG_INFINITY);
        assert_eq!(m.read_peak_r(), f32::NEG_INFINITY);
        assert_eq!(m.read_rms_r(), f32::NEG_INFINITY);
    }

    #[test]
    fn test_atomic_roundtrip() {
        let m = MeterData::new();
        m.write_peak_rms(
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        );
        assert_eq!(m.read_peak_l(), f32::NEG_INFINITY);
    }

    #[test]
    fn test_positive_value_roundtrip() {
        let m = MeterData::new();
        m.write_peak_rms(0.5, -10.0, -20.0, -30.0);
        assert_eq!(m.read_peak_l(), 0.5);
    }

    #[test]
    fn test_zero_roundtrip() {
        let m = MeterData::new();
        m.write_peak_rms(0.0, 0.0, 0.0, 0.0);
        assert_eq!(m.read_peak_l(), 0.0);
        assert_eq!(m.read_rms_r(), 0.0);
    }

    #[test]
    fn test_clipping_value_roundtrip() {
        let m = MeterData::new();
        m.write_peak_rms(-6.0, -3.0, 0.0, 6.0);
        assert_eq!(m.read_peak_l(), -6.0);
        assert_eq!(m.read_rms_l(), -3.0);
        assert_eq!(m.read_peak_r(), 0.0);
        assert_eq!(m.read_rms_r(), 6.0);
    }
}
