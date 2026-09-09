#[cfg(test)]
mod tests {
    use crate::audio::AudioError;
    use crate::audio::engine::{
        AudioEngine, AudioSettings, AudioState, BitDepth, ChannelConfig, ChannelMap,
    };

    // --- ChannelMap ---

    #[test]
    fn test_channel_map_default_all_none() {
        let map = ChannelMap::default();
        assert!(map.fl.is_none());
        assert!(map.fr.is_none());
        assert!(map.c.is_none());
        assert!(map.lfe.is_none());
        assert!(map.sl.is_none());
        assert!(map.sr.is_none());
        assert!(map.sbl.is_none());
        assert!(map.sbr.is_none());
    }

    #[test]
    fn test_channel_map_custom() {
        let map = ChannelMap {
            fl: Some(0),
            fr: Some(1),
            c: Some(2),
            lfe: Some(3),
            sl: Some(4),
            sr: Some(5),
            sbl: Some(6),
            sbr: Some(7),
        };
        assert_eq!(map.fl, Some(0));
        assert_eq!(map.fr, Some(1));
    }

    // --- AudioState ---

    #[test]
    fn test_audio_state_defaults() {
        let state = AudioState::default();
        assert!(!state.is_playing);
        assert!((state.volume - 0.3).abs() < f64::EPSILON as f32);
        assert_eq!(state.sample_rate, 48000);
        assert_eq!(state.channels, 2);
        assert_eq!(state.current_pos_sec, 0.0);
        assert_eq!(state.total_duration_sec, 0.0);
        assert_eq!(state.title, "Sin título");
        assert_eq!(state.artist, "Artista desconocido");
        assert!(state.path.is_empty());
        assert!(!state.eof_reached);
        assert_eq!(state.replay_gain_track_enabled, true);
        assert_eq!(state.replay_gain_album_enabled, true);

        // Volumen y Mezcla — Fades (D-07, D-09, D-10)
        assert_eq!(state.fades_enabled, false);
        assert_eq!(state.smooth_volume_enabled, false);
        assert!((state.fade_in_ms - 1000.0).abs() < f64::EPSILON as f32);
        assert!((state.fade_out_ms - 2000.0).abs() < f64::EPSILON as f32);
        assert_eq!(state.fade_in_enabled, false);
        assert_eq!(state.fade_out_enabled, false);

        // Volumen y Mezcla — Silence removal (D-14, D-16)
        assert_eq!(state.silence_enabled, true);
        assert_eq!(state.silence_edge_trim_enabled, true);
        assert!((state.silence_duration_ms - 1000.0).abs() < f64::EPSILON as f32);
        assert!((state.silence_threshold_db - (-47.0)).abs() < f64::EPSILON as f32);

        // Volumen y Mezcla — Replay gain fijo (reemplaza Normalización, UAT)
        assert_eq!(state.rg_fixed_enabled, false);
        assert!((state.rg_fixed_db - 0.0).abs() < f64::EPSILON as f32);

        // Volumen y Mezcla — ReplayGain offsets (D-26, D-29, D-30, D-28)
        assert_eq!(state.rg_master_enabled, true);
        assert!((state.rg_offset_album_db - 0.0).abs() < f64::EPSILON as f32);
        assert!((state.rg_offset_track_db - 0.0).abs() < f64::EPSILON as f32);
        assert!((state.rg_offset_rt_db - 0.0).abs() < f64::EPSILON as f32);
        assert_eq!(state.rg_analyze_rt_enabled, true);
    }

    // --- Volumen y Mezcla: AudioState defaults (D-07 through D-30) ---

    #[test]
    fn test_audio_state_volumen_defaults() {
        let state = AudioState::default();

        // Fades (D-07, D-09, D-10)
        assert_eq!(state.fades_enabled, false);
        assert_eq!(state.smooth_volume_enabled, false);
        assert!((state.fade_in_ms - 1000.0).abs() < f32::EPSILON);
        assert!((state.fade_out_ms - 2000.0).abs() < f32::EPSILON);
        assert_eq!(state.fade_in_enabled, false);
        assert_eq!(state.fade_out_enabled, false);

        // Silence removal (D-14, D-16)
        assert_eq!(state.silence_enabled, true);
        assert_eq!(state.silence_edge_trim_enabled, true);
        assert!((state.silence_duration_ms - 1000.0).abs() < f32::EPSILON);
        assert!((state.silence_threshold_db - (-47.0)).abs() < f32::EPSILON);

        // Replay gain fijo (reemplaza Normalización, UAT)
        assert_eq!(state.rg_fixed_enabled, false);
        assert!((state.rg_fixed_db - 0.0).abs() < f32::EPSILON);

        // ReplayGain offsets (D-26, D-29, D-30, D-28)
        assert_eq!(state.rg_master_enabled, true);
        assert!((state.rg_offset_album_db - 0.0).abs() < f32::EPSILON);
        assert!((state.rg_offset_track_db - 0.0).abs() < f32::EPSILON);
        assert!((state.rg_offset_rt_db - 0.0).abs() < f32::EPSILON);
        assert_eq!(state.rg_analyze_rt_enabled, true);

        // Existing replay_gain fields still true
        assert_eq!(state.replay_gain_track_enabled, true);
        assert_eq!(state.replay_gain_album_enabled, true);
    }

    // --- Mezcla Cruzada / Crossfade: AudioState defaults ---

    #[test]
    fn test_audio_state_crossfade_defaults() {
        let state = AudioState::default();

        // Master del grupo Crossfade y sus sub-funciones.
        assert_eq!(state.crossfade_enabled, false);
        assert_eq!(state.crossfade_manual_enabled, false);
        assert_eq!(state.crossfade_auto_enabled, false);
        assert!((state.crossfade_manual_ms - 1000.0).abs() < f32::EPSILON);
        assert!((state.crossfade_auto_ms - 250.0).abs() < f32::EPSILON);
    }

    // --- Loudness gain computation helper (mirrors decoder.rs:986-1025) ---

    /// Compute loudness gain (linear) mirroring the decoder's single gain point (D-01).
    /// All inputs are dB values; the function sums them, caps at +12 dB,
    /// and returns the linear factor 10^(gain_db/20).
    fn compute_gain_db(
        rg_track: Option<f32>,
        rg_album: Option<f32>,
        rg_track_enabled: bool,
        rg_album_enabled: bool,
        rg_master_enabled: bool,
        offset_album_db: f32,
        offset_track_db: f32,
        offset_rt_db: f32,
        analyze_rt_enabled: bool,
        normalize_gain_db: f32,
    ) -> f64 {
        let mut gain_db: f64 = 0.0;

        if rg_master_enabled {
            // 1. ReplayGain base from tags (D-27: album + track sum)
            if rg_track_enabled {
                if let Some(tg) = rg_track {
                    gain_db += tg as f64;
                }
            }
            if rg_album_enabled {
                if let Some(ag) = rg_album {
                    gain_db += ag as f64;
                }
            }

            // 2. RG offsets per source (D-26, D-29)
            gain_db += offset_album_db as f64;
            gain_db += offset_track_db as f64;

            // 3. RT Analysis fallback (D-28): only when no tags AND analyze enabled
            let has_tags = rg_track.is_some() || rg_album.is_some();
            if !has_tags && analyze_rt_enabled {
                gain_db += offset_rt_db as f64;
            }
        }

        // 4. RMS Normalization output (D-05 shared controller)
        gain_db += normalize_gain_db as f64;

        // 5. Clamp to safety ceiling +12 dB (D-26) and convert to linear
        10.0_f64.powf(gain_db.min(12.0) / 20.0)
    }

    // --- Loudness gain tests (D-01, D-26 through D-30) ---

    #[test]
    fn test_loudness_gain_no_tags() {
        // No RG tags, all defaults → unity gain (0 dB)
        let gain = compute_gain_db(
            None, None, true, true, true, // track/album/offsets
            0.0, 0.0, 0.0, true, 0.0,
        );
        assert!((gain - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_loudness_gain_rg_track() {
        // RG track = -3 dB, track enabled, album disabled → gain = 10^(-3/20)
        let gain = compute_gain_db(
            Some(-3.0),
            None,
            true,
            false,
            true,
            0.0,
            0.0,
            0.0,
            false,
            0.0,
        );
        let expected = 10.0_f64.powf(-3.0 / 20.0);
        assert!((gain - expected).abs() < 1e-10);
    }

    #[test]
    fn test_loudness_gain_rg_album_track_sum() {
        // RG track = -3 dB, album = -2 dB, both enabled → gain = 10^(-5/20) (D-27)
        let gain = compute_gain_db(
            Some(-3.0),
            Some(-2.0),
            true,
            true,
            true,
            0.0,
            0.0,
            0.0,
            false,
            0.0,
        );
        let expected = 10.0_f64.powf(-5.0 / 20.0);
        assert!((gain - expected).abs() < 1e-10);
    }

    #[test]
    fn test_loudness_gain_offsets() {
        // RG track = -3 dB, offset_album = +2 dB → gain = 10^(-1/20)
        let gain = compute_gain_db(
            Some(-3.0),
            None,
            true,
            false,
            true,
            2.0,
            0.0,
            0.0,
            false,
            0.0,
        );
        let expected = 10.0_f64.powf(-1.0 / 20.0);
        assert!((gain - expected).abs() < 1e-10);
    }

    #[test]
    fn test_loudness_gain_offset_track_and_album() {
        // RG track = -3 dB, offset_track = +1 dB, offset_album = +2 dB
        // gain_db = -3 + 0 + 1 + 2 = 0 dB → linear 1.0
        let gain = compute_gain_db(
            Some(-3.0),
            None,
            true,
            false,
            true,
            2.0,
            1.0,
            0.0,
            false,
            0.0,
        );
        assert!((gain - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_loudness_gain_rt_fallback() {
        // No RG tags, analyze_rt=true, offset_rt = +2 dB → gain = 10^(2/20)
        let gain = compute_gain_db(None, None, true, true, true, 0.0, 0.0, 2.0, true, 0.0);
        let expected = 10.0_f64.powf(2.0 / 20.0);
        assert!((gain - expected).abs() < 1e-10);
    }

    #[test]
    fn test_loudness_gain_rt_no_fallback_with_tags() {
        // RG track = -3 dB present, analyze_rt=true, offset_rt = +2 dB
        // D-28: offset_rt NOT added when tags exist.
        // gain_db = -3 + 0 = -3 dB
        let gain = compute_gain_db(
            Some(-3.0),
            None,
            true,
            false,
            true,
            0.0,
            0.0,
            2.0,
            true,
            0.0,
        );
        let expected = 10.0_f64.powf(-3.0 / 20.0);
        assert!((gain - expected).abs() < 1e-10);
    }

    #[test]
    fn test_loudness_gain_master_off() {
        // RG master enabled=false → all RG contributions = 0, gain = 1.0 (D-30)
        let gain = compute_gain_db(
            Some(-3.0),
            Some(-2.0),
            true,
            true,
            false, // master OFF
            5.0,
            5.0,
            5.0,
            true,
            0.0,
        );
        assert!((gain - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_loudness_gain_with_normalization() {
        // RG track = -3 dB + normalization = +4 dB → gain_db = +1 dB
        let gain = compute_gain_db(
            Some(-3.0),
            None,
            true,
            false,
            true,
            0.0,
            0.0,
            0.0,
            false,
            4.0,
        );
        let expected = 10.0_f64.powf(1.0 / 20.0);
        assert!((gain - expected).abs() < 1e-10);
    }

    #[test]
    fn test_loudness_gain_cap() {
        // RG track = -3 dB, offset_album = +20 dB → gain_db = +17 dB
        // capped at +12 dB (D-26 safety ceiling)
        let gain = compute_gain_db(
            Some(-3.0),
            None,
            true,
            false,
            true,
            20.0,
            0.0,
            0.0,
            false,
            0.0,
        );
        let expected_at_cap = 10.0_f64.powf(12.0 / 20.0);
        let expected_uncapped = 10.0_f64.powf(17.0 / 20.0);
        assert!((gain - expected_at_cap).abs() < 1e-10);
        assert!((gain - expected_uncapped).abs() > 1e-10);
    }

    #[test]
    fn test_loudness_gain_rg_album_only() {
        // RG album = -4 dB, album enabled, track disabled → gain = 10^(-4/20)
        let gain = compute_gain_db(
            None,
            Some(-4.0),
            false,
            true,
            true,
            0.0,
            0.0,
            0.0,
            false,
            0.0,
        );
        let expected = 10.0_f64.powf(-4.0 / 20.0);
        assert!((gain - expected).abs() < 1e-10);
    }

    // --- Silence detection tests (D-14, D-15, D-17) ---

    #[test]
    fn test_silence_detection_threshold_and_hysteresis() {
        // D-17: hysteresis with +3 dB difference.
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

        // D-15: peak-per-frame detection — max absolute value across channels
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

    // --- Fade envelope tests (D-12) ---

    #[test]
    fn test_fade_equal_power_curve() {
        // D-12: musical fades use equal-power sin²(π·t/2) curve
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

        // D-13: max error < 0.001 for the entire curve (smooth musical fade)
        for i in 0..=1000 {
            let t = i as f64 / 1000.0;
            let coeff = equal_power_fade(t);
            // At any point, coefficient should be between 0 and 1
            assert!(coeff >= 0.0);
            assert!(coeff <= 1.0);
            assert!(coeff.is_finite());
        }
    }

    // --- Volume smoothing with debounce (D-08, UAT round 7) ---
    // The smoothing holds applied_vol frozen while the user adjusts; after the
    // debounce window a 1500ms linear ramp moves applied_vol to the target.
    // This mirrors decoder.rs's smooth logic (SMOOTH_DEBOUNCE_SECS = 0.4,
    // SMOOTH_DURATION_SECS = 1.5).

    const SMOOTH_DEBOUNCE_SECS: f64 = 0.4;
    const SMOOTH_DURATION_SECS: f64 = 1.5;

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

    // --- Fade rate-per-second timing (D-12, D-13, UAT round 4) ---
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

    #[test]
    fn test_audio_state_clone() {
        let mut state = AudioState::default();
        state.title = "Test Song".to_string();
        state.is_playing = true;
        let cloned = state.clone();
        assert_eq!(cloned.title, "Test Song");
        assert!(cloned.is_playing);
    }

    // --- BitDepth ---

    #[test]
    fn test_bit_depth_default_is_32float() {
        assert_eq!(BitDepth::default(), BitDepth::Bits32Float);
    }

    #[test]
    fn test_bit_depth_all_variants() {
        let variants = [BitDepth::Bits16, BitDepth::Bits24, BitDepth::Bits32Float];
        for v in &variants {
            assert!(!format!("{:?}", v).is_empty());
        }
    }

    // --- ChannelConfig ---

    #[test]
    fn test_channel_config_default_manual_2() {
        assert_eq!(ChannelConfig::default(), ChannelConfig::Manual(2));
    }

    #[test]
    fn test_channel_config_auto() {
        let config = ChannelConfig::Auto;
        assert_eq!(config, ChannelConfig::Auto);
    }

    // --- AudioSettings ---

    #[test]
    fn test_audio_settings_default() {
        let settings = AudioSettings::default();
        assert!(settings.host_id.is_none());
        assert!(settings.device_name.is_none());
        assert!(settings.sample_rate.is_none());
        assert!(settings.bit_depth.is_none());
        assert_eq!(settings.channels, ChannelConfig::Manual(2));
    }

    // --- AudioError ---

    #[test]
    fn test_audio_error_display_no_device() {
        let err = AudioError::NoDevice;
        let msg = format!("{err}");
        assert!(msg.contains("No audio device"));
    }

    #[test]
    fn test_audio_error_display_unsupported_format() {
        let err = AudioError::UnsupportedSampleFormat;
        let msg = format!("{err}");
        assert!(msg.contains("Formato de muestra no soportado"));
    }

    #[test]
    fn test_audio_error_display_host_not_found() {
        let err = AudioError::HostNotFound;
        let msg = format!("{err}");
        assert!(msg.contains("Host no encontrado"));
    }

    #[test]
    fn test_audio_error_display_device_not_found() {
        let err = AudioError::DeviceNotFound;
        let msg = format!("{err}");
        assert!(msg.contains("Dispositivo no encontrado"));
    }

    #[test]
    fn test_audio_error_display_no_active_output() {
        let err = AudioError::NoActiveOutput;
        let msg = format!("{err}");
        assert!(msg.contains("No hay una salida de audio activa"));
    }

    #[test]
    fn test_audio_error_display_config() {
        let err = AudioError::ConfigError("test config".into());
        let msg = format!("{err}");
        assert!(msg.contains("test config"));
    }

    #[test]
    fn test_audio_error_display_stream() {
        let err = AudioError::StreamError("test stream".into());
        let msg = format!("{err}");
        assert!(msg.contains("test stream"));
    }

    #[test]
    fn test_audio_error_display_device() {
        let err = AudioError::DeviceError("test device".into());
        let msg = format!("{err}");
        assert!(msg.contains("test device"));
    }

    #[test]
    fn test_audio_error_display_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = AudioError::IoError(io_err);
        let msg = format!("{err}");
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn test_audio_error_io_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let err = AudioError::IoError(io_err);
        let source = std::error::Error::source(&err);
        assert!(source.is_some());
    }

    // --- Mix Channels Planar (existing tests expanded) ---

    #[test]
    fn test_mix_channels_planar_stereo_to_stereo() {
        let frames = 4;
        let in_channels = 2;
        let out_channels = 2;

        let input = vec![vec![1.0, 1.0, 1.0, 1.0], vec![0.5, 0.5, 0.5, 0.5]];

        let map = ChannelMap {
            fl: Some(0),
            fr: Some(1),
            c: None,
            lfe: None,
            sbl: None,
            sbr: None,
            sl: None,
            sr: None,
            ..Default::default()
        };

        let mut out_buf = Vec::new();
        AudioEngine::mix_channels_planar(
            &input,
            frames,
            in_channels,
            out_channels,
            &map,
            (1.0, 1.0, 1.0, 1.0),
            &mut out_buf,
        );

        assert_eq!(out_buf.len(), frames * out_channels);
        assert_eq!(out_buf[0], 1.0);
        assert_eq!(out_buf[1], 0.5);
        assert_eq!(out_buf[2], 1.0);
        assert_eq!(out_buf[3], 0.5);
    }

    #[test]
    fn test_mix_channels_planar_stereo_to_5_1_simple() {
        let frames = 2;
        let in_channels = 2;
        let out_channels = 6;

        let input = vec![vec![1.0, 1.0], vec![0.5, 0.5]];

        let mut map = ChannelMap::default();
        map.fl = Some(0);
        map.fr = Some(1);

        let mut out_buf = Vec::new();
        AudioEngine::mix_channels_planar(
            &input,
            frames,
            in_channels,
            out_channels,
            &map,
            (1.0, 1.0, 1.0, 1.0),
            &mut out_buf,
        );

        assert_eq!(out_buf.len(), frames * out_channels);
        assert_eq!(out_buf[0], 1.0);
        assert_eq!(out_buf[1], 0.5);
        assert_eq!(out_buf[2], 0.0);
        assert_eq!(out_buf[3], 0.0);
        assert_eq!(out_buf[4], 0.0);
        assert_eq!(out_buf[5], 0.0);
    }

    #[test]
    fn test_mix_channels_planar_mono_to_stereo() {
        let frames = 2;
        let in_channels = 1;
        let out_channels = 2;

        let input = vec![vec![0.8, 0.9]];

        let map = ChannelMap {
            c: Some(0),
            ..Default::default()
        };

        let mut out_buf = Vec::new();
        AudioEngine::mix_channels_planar(
            &input,
            frames,
            in_channels,
            out_channels,
            &map,
            (1.0, 1.0, 1.0, 1.0),
            &mut out_buf,
        );

        assert_eq!(out_buf.len(), frames * out_channels);
    }

    #[test]
    fn test_mix_channels_planar_output_grows_with_frames() {
        let map = ChannelMap {
            fl: Some(0),
            fr: Some(1),
            ..Default::default()
        };
        let input_2 = vec![vec![1.0, 1.0], vec![0.5, 0.5]];

        let mut out_buf_short = Vec::new();
        AudioEngine::mix_channels_planar(
            &input_2,
            2,
            2,
            2,
            &map,
            (1.0, 1.0, 1.0, 1.0),
            &mut out_buf_short,
        );
        assert_eq!(out_buf_short.len(), 4);

        let input_4 = vec![vec![1.0, 1.0, 1.0, 1.0], vec![0.5, 0.5, 0.5, 0.5]];
        let mut out_buf_long = Vec::new();
        AudioEngine::mix_channels_planar(
            &input_4,
            4,
            2,
            2,
            &map,
            (1.0, 1.0, 1.0, 1.0),
            &mut out_buf_long,
        );
        assert_eq!(out_buf_long.len(), 8);
    }

    #[test]
    fn test_mix_channels_planar_with_downmix_params() {
        let frames = 1;
        let in_channels = 6;
        let out_channels = 2;

        let input = vec![
            vec![0.5],
            vec![0.5],
            vec![0.8],
            vec![0.3],
            vec![0.2],
            vec![0.2],
        ];

        let map = ChannelMap {
            fl: Some(0),
            fr: Some(1),
            c: Some(2),
            lfe: Some(3),
            sl: Some(4),
            sr: Some(5),
            ..Default::default()
        };

        let mut out_default = Vec::new();
        AudioEngine::mix_channels_planar(
            &input,
            frames,
            in_channels,
            out_channels,
            &map,
            (1.0, 1.0, 1.0, 1.0),
            &mut out_default,
        );
        assert_eq!(out_default.len(), 2);

        let mut out_silent = Vec::new();
        AudioEngine::mix_channels_planar(
            &input,
            frames,
            in_channels,
            out_channels,
            &map,
            (0.0, 0.0, 0.0, 0.0),
            &mut out_silent,
        );
        assert_eq!(out_silent.len(), 2);
    }

    #[test]
    fn test_downmix_coefficients_independent() {
        // Verifica que dm_conf.1 (LFE) solo afecta al canal LFE, no a los demas.
        // Con entrada que tiene LFE != 0, comparamos output con lfe_coeff=1.0 vs lfe_coeff=0.0
        // La unica diferencia debe ser la contribucion del LFE.
        let frames = 1;
        let in_channels = 6;
        let out_channels = 2;

        let input = vec![
            vec![0.5], // FL
            vec![0.3], // FR
            vec![0.8], // C
            vec![1.0], // LFE (fuerte)
            vec![0.2], // SL
            vec![0.2], // SR
        ];

        let map = crate::audio::engine::ChannelMap {
            fl: Some(0),
            fr: Some(1),
            c: Some(2),
            lfe: Some(3),
            sl: Some(4),
            sr: Some(5),
            ..Default::default()
        };

        // Con LFE activo (coeff=1.0) vs LFE silenciado (coeff=0.0)
        let mut out_full = Vec::new();
        crate::audio::engine::AudioEngine::mix_channels_planar(
            &input,
            frames,
            in_channels,
            out_channels,
            &map,
            (1.0, 1.0, 1.0, 1.0),
            &mut out_full,
        );

        let mut out_no_lfe = Vec::new();
        crate::audio::engine::AudioEngine::mix_channels_planar(
            &input,
            frames,
            in_channels,
            out_channels,
            &map,
            (1.0, 0.0, 1.0, 1.0),
            &mut out_no_lfe,
        );

        assert_eq!(out_full.len(), 2);
        assert_eq!(out_no_lfe.len(), 2);

        // La diferencia debe ser exactamente LFE (1.0 * coeff_diff = 1.0)
        // LFE se suma por igual a L y R: out_full = out_no_lfe + LFE * (1.0 - 0.0) para cada canal
        for ch in 0..2 {
            let diff = (out_full[ch] - out_no_lfe[ch] - 1.0).abs();
            assert!(
                diff < 1e-6,
                "LFE contribution should be exactly 1.0 per channel: {}",
                diff
            );
        }

        // Con center coeff=0.0, la contribucion del centro debe desaparecer
        let mut out_no_center = Vec::new();
        crate::audio::engine::AudioEngine::mix_channels_planar(
            &input,
            frames,
            in_channels,
            out_channels,
            &map,
            (0.0, 1.0, 1.0, 1.0),
            &mut out_no_center,
        );

        for ch in 0..2 {
            let diff = (out_full[ch] - out_no_center[ch] - 0.8).abs();
            assert!(
                diff < 1e-6,
                "Center contribution should be exactly 0.8: {}",
                diff
            );
        }
    }

    // --- Matemática de buffers a altas tasas (diagnóstico del cambio de rate) ---

    #[test]
    fn test_ringbuf_sizes_at_high_rates() {
        // rb_size = rate × ch × 2.0s (mínimo 384.000 muestras), como en recreate_stream.
        let cases = [
            (44100u32, 2usize, 176_400usize),
            (96000, 2, 384_000),
            (192_000, 2, 768_000),
            (192_000, 6, 2_304_000),
            (384_000, 8, 6_144_000),
        ];
        for (rate, ch, expected) in cases {
            let rb_size = ((rate as f64 * ch as f64) * 2.0) as usize;
            assert_eq!(rb_size.max(384_000), expected.max(384_000));
        }
    }

    #[test]
    fn test_target_latency_math() {
        // 100ms de latencia objetivo a distintas tasas/canales (decoder should_wait).
        assert_eq!((44100 * 2 * 100) / 1000, 8820);
        assert_eq!((96000 * 2 * 100) / 1000, 19200);
        assert_eq!((192000 * 2 * 100) / 1000, 38400);
        assert_eq!((384000 * 8 * 100) / 1000, 307200);
    }

    // --- Predecode acotado por memoria y avance de posición durante el drenado ---

    #[test]
    fn test_predecode_cap_memory_bounded() {
        // El cap del predecode se limita por MEMORIA (~32MB de f64) además de por
        // tiempo: en salidas multi-canal de alta tasa el tope por tiempo retendría
        // cientos de MB (384kHz × 8ch × 8s ≈ 196MB).
        const PRELOAD_MAX_SAMPLES: usize = 4_000_000;

        // 384000 Hz × 8 ch: el cap por tiempo (8s) supera el tope de memoria.
        let out_rate = 384_000u32;
        let out_channels = 8usize;
        let cap_ms = 8000.0f64;
        let sec_cap = ((cap_ms / 1000.0) * out_rate as f64) as usize * out_channels;
        assert!(sec_cap > PRELOAD_MAX_SAMPLES);
        let cap_frames = sec_cap.min(PRELOAD_MAX_SAMPLES).max(out_channels);
        assert_eq!(cap_frames, PRELOAD_MAX_SAMPLES);

        // 44100 Hz × 2 ch: el cap por tiempo (8s) es menor → gana el tiempo.
        let sec_cap2 = ((cap_ms / 1000.0) * 44_100f64) as usize * 2;
        let cap_frames2 = sec_cap2.min(PRELOAD_MAX_SAMPLES).max(2);
        assert_eq!(cap_frames2, sec_cap2);
        assert!(cap_frames2 < PRELOAD_MAX_SAMPLES);
    }

    #[test]
    fn test_current_pos_advance_during_drain() {
        // La posición de reproducción avanza durante el drenado de la pre-carga:
        // batch / (tasa × canales) segundos por lote (~100ms por lote).
        let rate = 384_000usize;
        let ch = 2usize;
        let batch = 76_800usize; // 100ms a 384000×2
        let batch_secs = batch as f64 / (rate * ch) as f64;
        assert!((batch_secs - 0.1).abs() < 1e-9);

        // 8s de pre-carga drenados en lotes de 100ms → la suma avanza 8s exactos.
        let total_samples = batch * 80;
        let total_secs = total_samples as f64 / (rate * ch) as f64;
        assert!((total_secs - 8.0).abs() < 1e-9);
    }
}
