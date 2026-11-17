//! Single loudness gain point of the playback path.
//!
//! Every dB contribution the player applies before the DSP chain is summed here
//! and converted to a linear factor exactly once. `audio::decoder` calls this
//! from its `AudioState` snapshot right before the chain; the audio tests call
//! the same function, so the arithmetic they pin is the arithmetic that ships.
//!
//! The +12 dB ceiling is a safety limit rather than a preference: the tag gains
//! and the per-source offsets can add up well past it, and a linear factor above
//! that level drives the limiter and the output stage into clipping.

/// Snapshot of the values the gain formula reads.
///
/// The tag gains and the fixed gain keep the `f32` precision they have in
/// `AudioState`; the three offsets keep the `f64` precision of the smoothed
/// accumulators the decoder maintains.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LoudnessGainInputs {
    /// Track ReplayGain tag, in dB.
    pub rg_track: Option<f32>,
    /// Album ReplayGain tag, in dB.
    pub rg_album: Option<f32>,
    /// Per-source enables of the two tags above.
    pub rg_track_enabled: bool,
    pub rg_album_enabled: bool,
    /// Master gate of the whole ReplayGain group.
    pub rg_master_enabled: bool,
    /// EMA-smoothed offset of each source, in dB.
    pub offset_album_db: f64,
    pub offset_track_db: f64,
    pub offset_rt_db: f64,
    /// Real-time analysis fallback enable (only acts when no tag exists).
    pub analyze_rt_enabled: bool,
    /// Fixed replay gain and its own enable.
    pub fixed_enabled: bool,
    pub fixed_db: f32,
}

/// Sum the dB contributions and return the linear gain factor.
///
/// The five steps run in this order and under these gates: the tag base and the
/// per-source offsets live inside the master gate, the real-time offset only
/// acts inside it and only when neither tag is present, and the fixed gain is
/// applied outside the master gate under its own enable alone.
pub(crate) fn gain_linear(inputs: &LoudnessGainInputs) -> f64 {
    let mut gain_db: f64 = 0.0;

    if inputs.rg_master_enabled {
        // 1. ReplayGain base from tags (album + track sum).
        if inputs.rg_track_enabled
            && let Some(track_gain) = inputs.rg_track
        {
            gain_db += track_gain as f64;
        }
        if inputs.rg_album_enabled
            && let Some(album_gain) = inputs.rg_album
        {
            gain_db += album_gain as f64;
        }

        // 2. RG offsets per source: the EMA-smoothed values avoid clicks.
        gain_db += inputs.offset_album_db;
        gain_db += inputs.offset_track_db;

        // 3. RT analysis fallback: only when no tags AND analyze enabled.
        let has_tags = inputs.rg_track.is_some() || inputs.rg_album.is_some();
        if !has_tags && inputs.analyze_rt_enabled {
            gain_db += inputs.offset_rt_db;
        }
    }

    // 4. Fixed replay gain: it applies whenever its own enable is set, unlike
    //    the RT-analysis fallback, and it is not gated by the group master.
    if inputs.fixed_enabled {
        gain_db += inputs.fixed_db as f64;
    }

    // 5. Clamp to the safety ceiling (+12 dB) and convert to linear.
    10.0f64.powf(gain_db.min(12.0) / 20.0)
}
