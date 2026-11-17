//! Pinned values of the persisted setting keys.
//!
//! The key strings are the stored format of the `settings` table, not a naming
//! choice: every installation already holds rows under these names, so a
//! changed value orphans the saved preference with no migration path and no
//! error. Each assertion restates the literal a stored row carries, so a silent
//! re-lettering of the catalog fails the suite instead of losing a preference on
//! the next launch. The file imports no database type: it pins the strings
//! alone.

use super::*;

/// Every persisted key beside the stored name it must keep.
///
/// The values were read from the accessor sites the catalog replaced, so this
/// table is the frozen on-disk contract rather than a restatement of the
/// constants.
const STORED_KEYS: [(&str, &str); 48] = [
    (KEY_AUDIO_HOST, "audio_host"),
    (KEY_AUDIO_DEVICE, "audio_device"),
    (KEY_AUDIO_DEVICE_ID, "audio_device_id"),
    (KEY_AUDIO_SAMPLE_RATE, "audio_sample_rate"),
    (KEY_AUDIO_BIT_DEPTH, "audio_bit_depth"),
    (KEY_AUDIO_CHANNELS, "audio_channels"),
    (KEY_AUDIO_BUFFER_SIZE, "audio_buffer_size"),
    (KEY_AUDIO_SYSTEM_RATE, "audio_system_rate"),
    (KEY_AUDIO_SYSTEM_QUANTUM, "audio_system_quantum"),
    (KEY_COMPRESSOR_THRESHOLD, "compressor_threshold"),
    (KEY_COMPRESSOR_RATIO, "compressor_ratio"),
    (KEY_COMPRESSOR_ATTACK_MS, "compressor_attack_ms"),
    (KEY_COMPRESSOR_RELEASE_MS, "compressor_release_ms"),
    (KEY_REVERB_ROOM_SIZE, "reverb_room_size"),
    (KEY_REVERB_DAMPING, "reverb_damping"),
    (KEY_REVERB_WIDTH, "reverb_width"),
    (KEY_REVERB_MIX, "reverb_mix"),
    (KEY_DSP_REVERB_ENABLED, "dsp_reverb_enabled"),
    (KEY_SUB_BASS_GAIN, "sub_bass_gain"),
    (KEY_SUB_BASS_CUTOFF, "sub_bass_cutoff"),
    (KEY_MID_BASS_GAIN, "mid_bass_gain"),
    (KEY_MID_BASS_CUTOFF, "mid_bass_cutoff"),
    (KEY_VOICE_BOOST_GAIN, "voice_boost_gain"),
    (KEY_VOICE_BOOST_CUTOFF, "voice_boost_cutoff"),
    (KEY_PREAMP_GAIN, "preamp_gain"),
    (KEY_EQ_ENABLED, "eq_enabled"),
    (KEY_EQ_PRESET_NAME, "eq_preset_name"),
    (KEY_EQ_SELECTED_PRESET, "eq_selected_preset"),
    (KEY_EQ_BANDS_31, "eq_bands_31"),
    (KEY_EQ_BAND_GAINS_20, "eq_band_gains_20"),
    (KEY_EQ_BAND_GAINS_31, "eq_band_gains_31"),
    (KEY_VOLUME, "volume"),
    (KEY_PLAYER_VOLUME, "player_volume"),
    (KEY_FADE_IN_MS, "fade_in_ms"),
    (KEY_FADE_OUT_MS, "fade_out_ms"),
    (KEY_VOL_FADES_ENABLED, "vol_fades_enabled"),
    (KEY_CROSSFADE_ENABLED, "crossfade_enabled"),
    (KEY_CROSSFADE_MANUAL_ENABLED, "crossfade_manual_enabled"),
    (KEY_CROSSFADE_MANUAL_MS, "crossfade_manual_ms"),
    (KEY_CROSSFADE_AUTO_ENABLED, "crossfade_auto_enabled"),
    (KEY_CROSSFADE_AUTO_MS, "crossfade_auto_ms"),
    (KEY_METER_ENABLED, "meter_enabled"),
    (KEY_METER_HOLD_TIME_MS, "meter_hold_time_ms"),
    (KEY_METER_RMS_WINDOW_MS, "meter_rms_window_ms"),
    (KEY_METER_MS_MODE, "meter_ms_mode"),
    (KEY_METER_INFINITE_HOLD, "meter_infinite_hold"),
    (KEY_LIBRARY_VIEW_MODE, "library_view_mode"),
    (KEY_LAST_ACTIVE_PLAYLIST_ID, "last_active_playlist_id"),
];

#[test]
fn stored_key_names_are_frozen() {
    for (constant, stored) in STORED_KEYS {
        assert_eq!(
            constant, stored,
            "a persisted setting key no longer matches the row name every \
             installation already stores"
        );
    }

    let distinct: std::collections::BTreeSet<&str> =
        STORED_KEYS.iter().map(|(_, stored)| *stored).collect();
    assert_eq!(
        distinct.len(),
        STORED_KEYS.len(),
        "two preferences resolve to the same stored row, so one would overwrite the other"
    );
}
