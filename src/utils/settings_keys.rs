//! Persisted setting keys of the `settings` table.
//!
//! Every key the `settings` table stores is declared here once, so the Core
//! settings facade, the audio-routing facade, the playback facade, the
//! audio-center state and update handlers, the startup loader and the audio
//! integration test read one declaration instead of repeating the same string.
//!
//! The string values are a persisted format, not a naming choice: each one is
//! already a row in every installation's database, so a changed value silently
//! orphans the saved preference with no migration path. Only the constant names
//! may move; the values are frozen.

/// Output host the playback stream was opened against (`pipewire`, `alsa`, …).
pub const KEY_AUDIO_HOST: &str = "audio_host";
/// Output device name selected for playback.
pub const KEY_AUDIO_DEVICE: &str = "audio_device";
/// Identifier of the selected output device inside its host.
pub const KEY_AUDIO_DEVICE_ID: &str = "audio_device_id";
/// Sample rate requested for the playback stream, in hertz.
pub const KEY_AUDIO_SAMPLE_RATE: &str = "audio_sample_rate";
/// Sample format requested for the playback stream (`16`, `24`, `32`).
pub const KEY_AUDIO_BIT_DEPTH: &str = "audio_bit_depth";
/// Channel count requested for the playback stream.
pub const KEY_AUDIO_CHANNELS: &str = "audio_channels";
/// Buffer size requested for the playback stream, in frames.
pub const KEY_AUDIO_BUFFER_SIZE: &str = "audio_buffer_size";
/// Sample rate of the system mixer the stream was opened against.
pub const KEY_AUDIO_SYSTEM_RATE: &str = "audio_system_rate";
/// Buffer quantum of the system mixer the stream was opened against.
pub const KEY_AUDIO_SYSTEM_QUANTUM: &str = "audio_system_quantum";

/// Compressor threshold, in dBFS.
pub const KEY_COMPRESSOR_THRESHOLD: &str = "compressor_threshold";
/// Compressor ratio applied above the threshold.
pub const KEY_COMPRESSOR_RATIO: &str = "compressor_ratio";
/// Compressor attack time, in milliseconds.
pub const KEY_COMPRESSOR_ATTACK_MS: &str = "compressor_attack_ms";
/// Compressor release time, in milliseconds.
pub const KEY_COMPRESSOR_RELEASE_MS: &str = "compressor_release_ms";

/// Reverb room size.
pub const KEY_REVERB_ROOM_SIZE: &str = "reverb_room_size";
/// Reverb damping of the high frequencies.
pub const KEY_REVERB_DAMPING: &str = "reverb_damping";
/// Reverb stereo width.
pub const KEY_REVERB_WIDTH: &str = "reverb_width";
/// Reverb wet/dry mix.
pub const KEY_REVERB_MIX: &str = "reverb_mix";

/// Whether the reverb stage of the DSP chain is enabled.
pub const KEY_DSP_REVERB_ENABLED: &str = "dsp_reverb_enabled";

/// Sub-bass band gain, in dB.
pub const KEY_SUB_BASS_GAIN: &str = "sub_bass_gain";
/// Sub-bass band cutoff frequency, in hertz.
pub const KEY_SUB_BASS_CUTOFF: &str = "sub_bass_cutoff";

/// Mid-bass band gain, in dB.
pub const KEY_MID_BASS_GAIN: &str = "mid_bass_gain";
/// Mid-bass band cutoff frequency, in hertz.
pub const KEY_MID_BASS_CUTOFF: &str = "mid_bass_cutoff";

/// Voice-boost band gain, in dB.
pub const KEY_VOICE_BOOST_GAIN: &str = "voice_boost_gain";
/// Voice-boost band cutoff frequency, in hertz.
pub const KEY_VOICE_BOOST_CUTOFF: &str = "voice_boost_cutoff";

/// Preamp gain applied ahead of the equalizer bands, in dB.
pub const KEY_PREAMP_GAIN: &str = "preamp_gain";
/// Whether the equalizer stage is enabled.
pub const KEY_EQ_ENABLED: &str = "eq_enabled";
/// Name of the equalizer preset carried by the audio settings snapshot.
pub const KEY_EQ_PRESET_NAME: &str = "eq_preset_name";
/// Name of the equalizer preset currently selected in the audio center.
pub const KEY_EQ_SELECTED_PRESET: &str = "eq_selected_preset";
/// Whether the equalizer runs its 31-band layout instead of its 20-band one.
pub const KEY_EQ_BANDS_31: &str = "eq_bands_31";
/// Per-band gains of the 20-band equalizer layout.
pub const KEY_EQ_BAND_GAINS_20: &str = "eq_band_gains_20";
/// Per-band gains of the 31-band equalizer layout.
pub const KEY_EQ_BAND_GAINS_31: &str = "eq_band_gains_31";

/// Playback volume applied by the audio engine.
pub const KEY_VOLUME: &str = "volume";
/// Playback volume shown and persisted by the player.
pub const KEY_PLAYER_VOLUME: &str = "player_volume";
/// Fade-in duration after playback starts, in milliseconds.
pub const KEY_FADE_IN_MS: &str = "fade_in_ms";
/// Fade-out duration before playback stops, in milliseconds.
pub const KEY_FADE_OUT_MS: &str = "fade_out_ms";
/// Whether the volume fades are enabled.
pub const KEY_VOL_FADES_ENABLED: &str = "vol_fades_enabled";

/// Whether the crossfade group is enabled.
pub const KEY_CROSSFADE_ENABLED: &str = "crossfade_enabled";
/// Whether a manual track change crossfades into the next song.
pub const KEY_CROSSFADE_MANUAL_ENABLED: &str = "crossfade_manual_enabled";
/// Manual crossfade duration, in milliseconds.
pub const KEY_CROSSFADE_MANUAL_MS: &str = "crossfade_manual_ms";
/// Whether the automatic crossfade before the end of a song is enabled.
pub const KEY_CROSSFADE_AUTO_ENABLED: &str = "crossfade_auto_enabled";
/// Automatic crossfade duration, in milliseconds.
pub const KEY_CROSSFADE_AUTO_MS: &str = "crossfade_auto_ms";

/// Whether the VU meter is rendered.
pub const KEY_METER_ENABLED: &str = "meter_enabled";
/// Peak-hold duration of the VU meter, in milliseconds.
pub const KEY_METER_HOLD_TIME_MS: &str = "meter_hold_time_ms";
/// RMS averaging window of the VU meter, in milliseconds.
pub const KEY_METER_RMS_WINDOW_MS: &str = "meter_rms_window_ms";
/// Whether the VU meter measures mid/side instead of stereo.
pub const KEY_METER_MS_MODE: &str = "meter_ms_mode";
/// Whether the VU meter holds its peak until the track changes.
pub const KEY_METER_INFINITE_HOLD: &str = "meter_infinite_hold";

/// Library layout shown on startup (grid or list).
pub const KEY_LIBRARY_VIEW_MODE: &str = "library_view_mode";
/// Playlist that was selected when the application last closed.
pub const KEY_LAST_ACTIVE_PLAYLIST_ID: &str = "last_active_playlist_id";
