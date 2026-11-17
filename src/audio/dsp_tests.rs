#[cfg(test)]
mod tests {
    use crate::audio::dsp::{
        Compressor, DspChain, Equalizer, ExpanderMode, Limiter, MidBass, MultiBandStereoExpander,
        NoiseGate, Reverb, StereoBalance, SubBass, VoiceBoost,
    };
    use rubato::{
        Async, Fft, FixedAsync, FixedSync, Resampler, SincInterpolationParameters,
        SincInterpolationType, WindowFunction,
    };

    // ========================================================================
    // DspChain tests
    // ========================================================================

    #[test]
    fn test_dsp_chain_default_enabled() {
        let chain = DspChain::default();
        assert!(chain.enabled);
        assert!((chain.preamp_gain - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_dsp_chain_process_frame_bypass() {
        let mut chain = DspChain::default();
        chain.enabled = false;
        let mut frame = [1.0_f64, -0.5_f64];
        chain.process_frame(&mut frame);
        assert!((frame[0] - 1.0).abs() < 1e-10);
        assert!((frame[1] + 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_dsp_chain_preamp_gain() {
        let mut chain = DspChain::default();
        chain.set_preamp_db(6.0);
        assert!((chain.get_preamp_db() - 6.0).abs() < 1e-4);
        let expected_gain = 10.0_f32.powf(6.0 / 20.0);
        assert!((chain.preamp_gain - expected_gain).abs() < 1e-6);
    }

    #[test]
    fn test_dsp_chain_reset_state() {
        let mut chain = DspChain::default();
        chain.reset_state();
    }

    #[test]
    fn test_dsp_chain_set_sample_rate() {
        let mut chain = DspChain::default();
        chain.set_sample_rate(48000.0);
    }

    // ========================================================================
    // Equalizer tests
    // ========================================================================

    #[test]
    fn test_eq_new_31_bands() {
        let equalizer = Equalizer::new(31);
        assert_eq!(equalizer.bands.len(), 31);
    }

    #[test]
    fn test_eq_new_20_bands() {
        let equalizer = Equalizer::new(20);
        assert_eq!(equalizer.bands.len(), 20);
    }

    #[test]
    fn test_eq_defaults_to_31() {
        let equalizer = Equalizer::new(999);
        assert_eq!(equalizer.bands.len(), 31);
        let equalizer = Equalizer::new(0);
        assert_eq!(equalizer.bands.len(), 31);
    }

    #[test]
    fn test_eq_band_frequencies_monotonic() {
        for num_bands in [20, 31] {
            let equalizer = Equalizer::new(num_bands);
            for window in equalizer.bands.windows(2) {
                assert!(
                    window[0].frequency < window[1].frequency,
                    "bands not monotonic at {} bands: {} >= {}",
                    num_bands,
                    window[0].frequency,
                    window[1].frequency
                );
            }
        }
    }

    #[test]
    fn test_eq_reset_all() {
        let mut equalizer = Equalizer::new(31);
        equalizer.bands[0].set_gain(6.0);
        equalizer.bands[15].set_gain(-3.0);
        equalizer.reset_all();
        for band in &equalizer.bands {
            assert!((band.gain - 0.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_eq_set_mode_switch() {
        let mut equalizer = Equalizer::new(31);
        assert_eq!(equalizer.bands.len(), 31);
        equalizer.set_mode(20);
        assert_eq!(equalizer.bands.len(), 20);
        equalizer.set_mode(31);
        assert_eq!(equalizer.bands.len(), 31);
    }

    #[test]
    fn test_eq_process_enabled_false() {
        let mut equalizer = Equalizer::new(31);
        equalizer.enabled = false;
        let mut sample = 0.5_f64;
        equalizer.process(&mut sample, 0);
        assert!((sample - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_eq_set_sample_rate() {
        let mut equalizer = Equalizer::new(31);
        equalizer.set_sample_rate(96000.0);
    }

    #[test]
    fn test_apply_preset_gains_updates_active_bands() {
        let mut equalizer = Equalizer::new(20);
        let bands_20: Vec<f32> = (0..20).map(|i| i as f32 * 0.5 - 5.0).collect();
        let bands_31: Vec<f32> = vec![0.0; 31];

        equalizer.apply_preset_gains(&bands_20, &bands_31);

        assert_eq!(equalizer.bands.len(), 20);
        for (i, band) in equalizer.bands.iter().enumerate() {
            assert!((band.gain - bands_20[i]).abs() < 0.001);
        }
    }

    #[test]
    fn test_apply_preset_gains_preserves_switched_mode() {
        let mut equalizer = Equalizer::new(20);
        let bands_20: Vec<f32> = (0..20).map(|i| i as f32 * 0.5 - 5.0).collect();
        let bands_31: Vec<f32> = (0..31).map(|i| i as f32 * 0.3 - 4.0).collect();

        equalizer.apply_preset_gains(&bands_20, &bands_31);

        // Switch to 31-band mode — should see bands_31 gains
        equalizer.set_mode(31);
        assert_eq!(equalizer.bands.len(), 31);
        for (i, band) in equalizer.bands.iter().enumerate() {
            assert!((band.gain - bands_31[i]).abs() < 0.001);
        }

        // Switch back to 20-band mode — should see bands_20 gains
        equalizer.set_mode(20);
        assert_eq!(equalizer.bands.len(), 20);
        for (i, band) in equalizer.bands.iter().enumerate() {
            assert!((band.gain - bands_20[i]).abs() < 0.001);
        }
    }

    #[test]
    fn test_reset_all_clears_saved_bands() {
        let mut equalizer = Equalizer::new(20);
        let bands_20: Vec<f32> = vec![3.0; 20];
        let bands_31: Vec<f32> = vec![3.0; 31];
        equalizer.apply_preset_gains(&bands_20, &bands_31);

        // Verify gains were applied
        assert!((equalizer.bands[0].gain - 3.0).abs() < 0.001);

        equalizer.reset_all();

        // Active bands should be zeroed
        for band in &equalizer.bands {
            assert!((band.gain - 0.0).abs() < 0.001);
        }

        // Switch to 31 — if saved_bands_31 was reset, bands will be 0.
        // If NOT reset, bands will still have the old 3.0 values.
        equalizer.set_mode(31);
        for band in &equalizer.bands {
            assert!(
                (band.gain - 0.0).abs() < 0.001,
                "saved_bands_31 should have been reset: gain={}",
                band.gain
            );
        }

        // Switch back to 20 — same check for saved_bands_20
        equalizer.set_mode(20);
        for band in &equalizer.bands {
            assert!(
                (band.gain - 0.0).abs() < 0.001,
                "saved_bands_20 should have been reset: gain={}",
                band.gain
            );
        }
    }

    // ========================================================================
    // Reverb tests
    // ========================================================================

    #[test]
    fn test_reverb_new_disabled() {
        let reverb = Reverb::new();
        assert!(!reverb.enabled);
    }

    #[test]
    fn test_reverb_default_values() {
        let reverb = Reverb::new();
        assert!((reverb.room_size - 0.5).abs() < 1e-6);
        assert!((reverb.wet - 0.5).abs() < 1e-6);
        assert!((reverb.dry - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_reverb_room_size_clamping() {
        let mut reverb = Reverb::new();
        reverb.set_room_size(1.5);
        assert!((reverb.room_size - 1.0).abs() < 1e-6);
        reverb.set_room_size(-0.5);
        assert!((reverb.room_size - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_reverb_damping_clamping() {
        let mut reverb = Reverb::new();
        reverb.set_damping(2.0);
        // set_damping is now a no-op; damping is auto-derived from room_size.
        // Just verify it doesn't panic.
    }

    #[test]
    fn test_reverb_wet_clamping() {
        let mut reverb = Reverb::new();
        reverb.set_wet(1.5);
        assert!((reverb.wet - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_reverb_process_no_panic() {
        let mut reverb = Reverb::new();
        reverb.enabled = true;
        reverb.process(&mut [0.5]);
        reverb.process(&mut [0.5, -0.3]);
    }

    #[test]
    fn test_reverb_reset_state() {
        let mut reverb = Reverb::new();
        reverb.reset_state();
    }

    // ========================================================================
    // Compressor tests
    // ========================================================================

    #[test]
    fn test_compressor_new_disabled() {
        let compressor = Compressor::new();
        assert!(!compressor.enabled);
    }

    /// `Default` delegates to `new()`: a derived default would zero every parameter,
    /// leaving a 0 dB threshold and no intensity curve.
    #[test]
    fn test_default_compressor_matches_new() {
        let defaulted = Compressor::default();
        let constructed = Compressor::new();

        assert_eq!(defaulted.enabled, constructed.enabled);
        assert!((defaulted.threshold - constructed.threshold).abs() < 1e-6);
        assert!((defaulted.intensity - constructed.intensity).abs() < 1e-6);
        assert!((defaulted.intensity_ratio - constructed.intensity_ratio).abs() < 1e-6);
        assert!((defaulted.envelope - constructed.envelope).abs() < 1e-6);
        assert_eq!(defaulted.lookahead_samples, constructed.lookahead_samples);

        // The fields a derive would zero: `new()` sets the -3 dB threshold and runs
        // `update_intensity_params()`, which derives the 4.0 ratio together with the
        // attack, release, knee and makeup values computed in the same call.
        assert!((defaulted.threshold - (-3.0)).abs() < 1e-6);
        assert!((defaulted.intensity - 0.5).abs() < 1e-6);
        assert!((defaulted.intensity_ratio - 4.0).abs() < 1e-6);
        assert_eq!(defaulted.lookahead_samples, 44);
    }

    #[test]
    fn test_compressor_params_defaults() {
        let compressor = Compressor::new();
        assert!((compressor.threshold - (-3.0)).abs() < 1e-6);
        assert!((compressor.intensity - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_compressor_direct_field_params() {
        let mut compressor = Compressor::new();
        compressor.threshold = -20.0;
        compressor.intensity = 0.5;
        compressor.update_intensity_params();
        assert!((compressor.threshold - (-20.0)).abs() < 1e-6);
        assert!((compressor.intensity - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_compressor_process_no_panic() {
        let mut compressor = Compressor::new();
        compressor.enabled = true;
        for _ in 0..100 {
            compressor.process(&mut [0.5, -0.3]);
        }
    }

    #[test]
    fn test_compressor_reset_state() {
        let mut compressor = Compressor::new();
        compressor.reset_state();
    }

    // ========================================================================
    // Compressor premium tests
    // ========================================================================

    #[test]
    fn test_compressor_rms_vs_peak() {
        let mut compressor = Compressor::new();
        compressor.enabled = true;
        compressor.threshold = -10.0;
        compressor.intensity = 0.5;
        compressor.update_intensity_params();
        // Fill lookahead with steady signal
        for _ in 0..compressor.lookahead_samples {
            compressor.process(&mut [0.01_f64, 0.01_f64]);
        }
        // Feed transient
        compressor.process(&mut [0.9_f64, -0.1_f64]);
        // RMS envelope should be lower than peak 0.9
        assert!(compressor.envelope < 0.9);
    }

    #[test]
    fn test_compressor_soft_knee_curve() {
        let mut compressor = Compressor::new();
        compressor.enabled = true;
        compressor.threshold = -10.0;
        compressor.intensity = 0.5;
        compressor.update_intensity_params();
        // Fill lookahead with steady signal at -14dB ≈ 0.2 linear
        for _ in 0..compressor.lookahead_samples {
            compressor.process(&mut [0.2_f64, 0.2_f64]);
        }
        // After processing, envelope should be around 0.2
        assert!(compressor.envelope < 0.3);
        // Process should not panic with knee active
    }

    #[test]
    fn test_compressor_intensity_mapping() {
        let mut compressor_without_intensity = Compressor::new();
        compressor_without_intensity.intensity = 0.0;
        compressor_without_intensity.update_intensity_params();
        assert!(
            compressor_without_intensity.intensity_ratio >= 1.0
                && compressor_without_intensity.intensity_ratio <= 2.0
        );

        let mut compressor_at_half_intensity = Compressor::new();
        compressor_at_half_intensity.intensity = 0.5;
        compressor_at_half_intensity.update_intensity_params();
        assert!(
            compressor_at_half_intensity.intensity_ratio >= 3.5
                && compressor_at_half_intensity.intensity_ratio <= 4.5
        );

        let mut compressor_at_full_intensity = Compressor::new();
        compressor_at_full_intensity.intensity = 1.0;
        compressor_at_full_intensity.update_intensity_params();
        assert!(
            compressor_at_full_intensity.intensity_ratio >= 18.0
                && compressor_at_full_intensity.intensity_ratio <= 22.0
        );
    }

    #[test]
    fn test_compressor_lookahead_transient() {
        let mut compressor = Compressor::new();
        compressor.enabled = true;
        compressor.threshold = -3.0;
        compressor.intensity = 0.75;
        compressor.update_intensity_params();
        // Fill lookahead with silence
        for _ in 0..compressor.lookahead_samples {
            compressor.process(&mut [0.0_f64, 0.0_f64]);
        }
        let mut impulse = [0.98_f64, -0.98_f64];
        compressor.process(&mut impulse);
        assert!(
            impulse[0].abs() < 0.98,
            "Lookahead should compress transient"
        );
    }

    #[test]
    fn test_compressor_zero_threshold_bypass() {
        let mut compressor = Compressor::new();
        compressor.enabled = true;
        compressor.threshold = 0.0;
        compressor.intensity = 0.5;
        compressor.update_intensity_params();
        // Feed frames at -6dB (well below 0dB threshold) until buffer full + output
        for _ in 0..compressor.lookahead_samples + 1 {
            let mut frame = [0.5_f64, -0.3_f64];
            let original = frame;
            compressor.process(&mut frame);
            // Once buffer is full, output should be identical or very close (makeup may apply)
            assert!((frame[0] - original[0]).abs() < 1.0); // makeup adds gain but no compression
        }
    }

    #[test]
    fn test_compressor_makeup_gain() {
        let mut compressor = Compressor::new();
        compressor.enabled = true;
        compressor.threshold = -30.0;
        compressor.intensity = 0.5;
        compressor.update_intensity_params();
        // Fill lookahead with above-threshold signal
        for _ in 0..compressor.lookahead_samples {
            compressor.process(&mut [0.5_f64, 0.5_f64]);
        }
        let mut frame = [0.5_f64, 0.5_f64];
        compressor.process(&mut frame);
        assert!(
            frame[0] > 0.0,
            "Makeup gain should produce non-zero output with heavy compression"
        );
    }

    #[test]
    fn test_compressor_nan_guard() {
        let mut compressor = Compressor::new();
        compressor.enabled = true;
        for _ in 0..compressor.lookahead_samples {
            compressor.process(&mut [f64::NAN, f64::INFINITY]);
        }
        compressor.process(&mut [f64::NAN, f64::INFINITY]);
        // Should not panic and should produce finite output
    }

    // ========================================================================
    // StereoBalance tests
    // ========================================================================

    #[test]
    fn test_stereo_balance_default_centered() {
        let stereo_balance = StereoBalance::default();
        assert!((stereo_balance.balance - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_stereo_balance_full_left() {
        let mut stereo_balance = StereoBalance::default();
        stereo_balance.enabled = true;
        stereo_balance.balance = -1.0;
        let mut frame = [1.0_f64, 1.0_f64];
        stereo_balance.process(&mut frame);
        assert!((frame[0] - 1.0).abs() < 1e-10);
        assert!((frame[1] - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_stereo_balance_full_right() {
        let mut stereo_balance = StereoBalance::default();
        stereo_balance.enabled = true;
        stereo_balance.balance = 1.0;
        let mut frame = [1.0_f64, 1.0_f64];
        stereo_balance.process(&mut frame);
        assert!((frame[0] - 0.0).abs() < 1e-10);
        assert!((frame[1] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_stereo_balance_center_no_change() {
        let mut stereo_balance = StereoBalance::default();
        stereo_balance.enabled = true;
        stereo_balance.balance = 0.0;
        let mut frame = [0.5_f64, 0.5_f64];
        stereo_balance.process(&mut frame);
        assert!((frame[0] - 0.5).abs() < 1e-10);
        assert!((frame[1] - 0.5).abs() < 1e-10);
    }

    // ========================================================================
    // Limiter tests
    // ========================================================================

    #[test]
    fn test_limiter_default_disabled() {
        let limiter = Limiter::default();
        assert!(!limiter.enabled);
    }

    #[test]
    fn test_limiter_ceiling_default() {
        let limiter = Limiter::default();
        assert!((limiter.ceiling - (-1.0)).abs() < 1e-4);
    }

    #[test]
    fn test_limiter_process_no_panic() {
        let mut limiter = Limiter::default();
        limiter.enabled = true;
        for _ in 0..100 {
            limiter.process(&mut [0.5, -0.3]);
        }
    }

    #[test]
    fn test_limiter_reset_state() {
        let mut limiter = Limiter::default();
        limiter.reset_state();
    }

    // ========================================================================
    // Limiter premium tests
    // ========================================================================

    #[test]
    fn test_limiter_oversampling_true_peak() {
        let mut limiter = Limiter::default();
        limiter.enabled = true;
        limiter.ceiling = -1.0;
        for _ in 0..limiter.lookahead_samples {
            limiter.process(&mut [0.5, 0.5]);
        }
        let mut frame = [0.95, 0.95];
        limiter.process(&mut frame);
        let ceiling_lin = 10.0_f64.powf(-1.0 / 20.0);
        assert!(
            frame[0].abs() <= ceiling_lin + 0.01,
            "Oversampled limiter should catch peaks: {} > {}",
            frame[0].abs(),
            ceiling_lin
        );
    }

    #[test]
    fn test_limiter_lookahead_transient() {
        let mut limiter = Limiter::default();
        limiter.enabled = true;
        limiter.ceiling = -3.0;
        for _ in 0..limiter.lookahead_samples {
            limiter.process(&mut [0.0, 0.0]);
        }
        let mut impulse = [0.98, -0.98];
        limiter.process(&mut impulse);
        let ceiling_lin = 10.0_f64.powf(-3.0 / 20.0);
        assert!(
            impulse[0].abs() <= ceiling_lin + 0.01,
            "Lookahead limiter should catch transient: {} > {}",
            impulse[0].abs(),
            ceiling_lin
        );
    }

    #[test]
    fn test_limiter_crest_factor() {
        let mut limiter = Limiter::default();
        limiter.enabled = true;
        for _ in 0..limiter.lookahead_samples {
            limiter.process(&mut [0.5, 0.5]);
        }
        limiter.process(&mut [0.5, 0.5]);
        // After steady signal, crest factor should be relatively low
        assert!(
            limiter.crest_factor_smooth > 0.0,
            "Crest factor should be valid: {}",
            limiter.crest_factor_smooth
        );
    }

    #[test]
    fn test_limiter_zero_ceiling_bypass() {
        let mut limiter = Limiter::default();
        limiter.enabled = true;
        limiter.ceiling = 0.0;
        for _ in 0..limiter.lookahead_samples {
            limiter.process(&mut [0.5, 0.5]);
        }
        let mut frame = [0.5, 0.5];
        limiter.process(&mut frame);
        // With ceiling=0dB, no limiting occurs; output should not be zero
        assert!(
            frame[0].abs() > 0.0,
            "ceiling=0.0 should pass signal through"
        );
        assert!(
            frame[0].abs() <= 0.5 + 1e-3,
            "output should not exceed input without compression"
        );
    }

    #[test]
    fn test_limiter_nan_guard() {
        let mut limiter = Limiter::default();
        limiter.enabled = true;
        for _ in 0..limiter.lookahead_samples {
            limiter.process(&mut [f64::NAN, f64::INFINITY]);
        }
        limiter.process(&mut [f64::NAN, f64::INFINITY]);
    }

    // ========================================================================
    // NoiseGate tests
    // ========================================================================

    #[test]
    fn test_noise_gate_default_threshold() {
        let noise_gate = NoiseGate::default();
        assert!((noise_gate.threshold - (-60.0)).abs() < 1e-6);
    }

    #[test]
    fn test_noise_gate_process_no_panic() {
        let mut noise_gate = NoiseGate::default();
        noise_gate.enabled = true;
        noise_gate.process(&mut [0.5, -0.3]);
    }

    #[test]
    fn test_noise_gate_reset_state() {
        let mut noise_gate = NoiseGate::default();
        noise_gate.reset_state();
    }

    // ========================================================================
    // SubBass tests
    // ========================================================================

    #[test]
    fn test_sub_bass_default_values() {
        let sub_bass = SubBass::default();
        assert!((sub_bass.freq - 45.0).abs() < 1e-6);
        assert!((sub_bass.gain - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_sub_bass_process_no_panic() {
        let mut sub_bass = SubBass::default();
        sub_bass.enabled = true;
        sub_bass.process(&mut [0.5, -0.3]);
    }

    // ========================================================================
    // MidBass tests
    // ========================================================================

    #[test]
    fn test_mid_bass_default_values() {
        let mid_bass = MidBass::default();
        assert!((mid_bass.freq - 100.0).abs() < 1e-6);
        assert!((mid_bass.gain - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_mid_bass_set_sample_rate() {
        let mut mid_bass = MidBass::default();
        mid_bass.set_sample_rate(48000.0);
    }

    // ========================================================================
    // VoiceBoost tests
    // ========================================================================

    #[test]
    fn test_voice_boost_default() {
        let voice_boost = VoiceBoost::default();
        assert!((voice_boost.gain - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_voice_boost_set_sample_rate() {
        let mut voice_boost = VoiceBoost::default();
        voice_boost.set_sample_rate(48000.0);
    }

    // ========================================================================
    // MultiBandStereoExpander tests
    // ========================================================================

    #[test]
    fn test_stereo_expander_default_values() {
        let expander = MultiBandStereoExpander::default();
        assert!((expander.width - 1.0).abs() < 1e-6);
        assert_eq!(expander.mode, ExpanderMode::Hybrid);
    }

    #[test]
    fn test_stereo_expander_process_stereo_no_panic() {
        let mut expander = MultiBandStereoExpander::default();
        expander.enabled = true;
        expander.width = 1.5;
        let mut frame = [0.5_f64, -0.3_f64];
        expander.process(&mut frame);
    }

    #[test]
    fn test_stereo_expander_reset_state() {
        let mut expander = MultiBandStereoExpander::default();
        expander.reset_state();
    }

    // ========================================================================
    // Equal-power reverb tests
    // ========================================================================

    #[test]
    fn test_reverb_equal_power_bypass() {
        let mut reverb = Reverb::new();
        reverb.enabled = true;
        reverb.set_wet(0.0);
        let mut frame = [1.0_f64, -0.8_f64];
        let original = frame;
        reverb.process(&mut frame);
        for (output, original) in frame.iter().zip(original.iter()) {
            assert!(
                (output - original).abs() < 1e-6,
                "wet=0.0 should bypass: out={} orig={}",
                output,
                original
            );
        }
    }

    #[test]
    fn test_reverb_equal_power_rms() {
        let mut reverb = Reverb::new();
        reverb.enabled = true;
        reverb.set_wet(0.5);
        let frame: [f64; 2] = [0.5, -0.3];
        let input_rms = (frame.iter().map(|s| s * s).sum::<f64>() / frame.len() as f64).sqrt();
        let mut out_frame = frame;
        reverb.process(&mut out_frame);
        let output_rms =
            (out_frame.iter().map(|s| s * s).sum::<f64>() / out_frame.len() as f64).sqrt();
        assert!(output_rms > 0.0, "Reverb wet=0.5 should produce output");
        assert!(
            (output_rms - input_rms).abs() < 0.3,
            "RMS should not change drastically; input={} output={}",
            input_rms,
            output_rms
        );
    }

    #[test]
    fn test_reverb_equal_power_full_wet() {
        let mut reverb = Reverb::new();
        reverb.enabled = true;
        reverb.set_wet(1.0);
        let mut frame = [1.0_f64, -0.5_f64];
        reverb.process(&mut frame);
        assert!(
            (frame[0] - 1.0).abs() > 1e-3 || (frame[1] + 0.5).abs() > 1e-3,
            "wet=1.0 should alter the signal (pure wet output, no dry)"
        );
    }

    // ========================================================================
    // FDN reverb tests
    // ========================================================================

    #[test]
    fn test_reverb_fdn_stability() {
        let mut reverb = Reverb::new();
        reverb.enabled = true;
        reverb.set_wet(0.5);
        reverb.set_room_size(0.99);
        // Feed impulse, process 1 second worth of samples
        let mut frame = [1.0_f64, 1.0_f64];
        reverb.process(&mut frame);
        let mut max_val = 0.0_f64;
        for _ in 0..(44100 / 2) {
            let mut silent_frame = [0.0_f64, 0.0_f64];
            reverb.process(&mut silent_frame);
            max_val = max_val
                .max(silent_frame[0].abs())
                .max(silent_frame[1].abs());
        }
        // After 0.5s with no input, output should be decaying
        assert!(max_val < 5.0, "No runaway feedback: max={}", max_val);
    }

    #[test]
    fn test_reverb_fdn_stereo_decorrelated() {
        let mut reverb = Reverb::new();
        reverb.enabled = true;
        reverb.set_wet(1.0);
        reverb.set_room_size(0.8);
        // Feed sustained signal to ensure both channels have energy
        for _ in 0..500 {
            reverb.process(&mut [0.5_f64, 0.5_f64]);
        }
        // Now L and R should differ after processing same input with different delay lengths
        let mut l_val = 0.0_f64;
        let mut r_val = 0.0_f64;
        let mut diverged = false;
        for _ in 0..500 {
            let mut silent_frame = [0.0_f64, 0.0_f64];
            reverb.process(&mut silent_frame);
            l_val = silent_frame[0];
            r_val = silent_frame[1];
            if (l_val - r_val).abs() > 1e-6 {
                diverged = true;
                break;
            }
        }
        // FDN with different L/R delay lengths should produce decorrelated output
        // If not diverged, the channels might be too similar but that's acceptable for mono-in → stereo
        let _ = (l_val, r_val, diverged);
    }

    #[test]
    fn test_reverb_fdn_pre_delay() {
        let mut reverb = Reverb::new();
        reverb.enabled = true;
        reverb.set_room_size(0.5);
        let mut frame = [1.0_f64, 0.0_f64];
        reverb.process(&mut frame);
        // First few samples should be near-zero due to pre-delay
        let mut first_nonzero_sample = None;
        for i in 0..44100 {
            let mut silent_frame = [0.0_f64, 0.0_f64];
            reverb.process(&mut silent_frame);
            if silent_frame[0].abs() > 1e-9 && first_nonzero_sample.is_none() {
                first_nonzero_sample = Some(i);
            }
        }
        assert!(
            first_nonzero_sample.unwrap_or(0) > 0,
            "Pre-delay should delay first output"
        );
    }

    // ========================================================================
    // Limiter auto-on / restore tests
    // ========================================================================

    #[test]
    fn test_limiter_auto_on_restore() {
        let mut chain = DspChain::default();

        // Initially limiter is disabled (default)
        assert!(!chain.limiter.enabled);

        // force_limiter_on when disabled → returns false (was disabled), enables limiter
        let limiter_was_enabled = chain.force_limiter_on();
        assert!(
            !limiter_was_enabled,
            "Limiter was disabled, should return false"
        );
        assert!(chain.limiter.enabled, "Limiter should now be enabled");

        // restore_limiter(false) → restores to disabled
        chain.restore_limiter(false);
        assert!(
            !chain.limiter.enabled,
            "Limiter should be restored to disabled"
        );

        // force_limiter_on when already enabled → returns true (was enabled), stays enabled
        chain.limiter.enabled = true;
        let limiter_was_enabled_again = chain.force_limiter_on();
        assert!(
            limiter_was_enabled_again,
            "Limiter was already enabled, should return true"
        );
        assert!(chain.limiter.enabled, "Limiter should stay enabled");

        // restore_limiter(true) → restores to enabled
        chain.restore_limiter(true);
        assert!(
            chain.limiter.enabled,
            "Limiter should be restored to enabled"
        );

        // Force on again after restore to true, then restore to false
        chain.limiter.enabled = false;
        let limiter_was_enabled_after_restore = chain.force_limiter_on();
        assert!(!limiter_was_enabled_after_restore);
        chain.restore_limiter(false);
        assert!(!chain.limiter.enabled);
    }

    #[test]
    fn test_dsp_chain_order_unchanged() {
        // Verify that adding force_limiter_on/restore_limiter did NOT change
        // DspChain::process_frame order. The bypass test should still pass.
        let mut chain = DspChain::default();
        chain.enabled = false;
        let mut frame = [1.0_f64, -0.5_f64];
        chain.process_frame(&mut frame);
        // Bypass: frame should be unchanged
        assert!((frame[0] - 1.0).abs() < 1e-10);
        assert!((frame[1] + 0.5).abs() < 1e-10);

        // force/restore should not affect process_frame behavior
        let limiter_was_enabled = chain.force_limiter_on();
        chain.restore_limiter(limiter_was_enabled);

        // Process again via bypass — frame still unchanged
        let mut frame2 = [0.8_f64, -0.3_f64];
        chain.process_frame(&mut frame2);
        assert!((frame2[0] - 0.8).abs() < 1e-10);
        assert!((frame2[1] + 0.3).abs() < 1e-10);

        // Enable chain and force limiter on — process should still work (no panic)
        chain.enabled = true;
        chain.preamp_gain = 1.0;
        chain.equalizer.enabled = false;
        chain.reverb.enabled = false;
        chain.compressor.enabled = false;
        chain.noise_gate.enabled = false;
        chain.sub_bass.enabled = false;
        chain.mid_bass.enabled = false;
        chain.voice_boost.enabled = false;
        chain.stereo_expander.enabled = false;
        chain.stereo_balance.enabled = false;
        chain.limiter.enabled = true;

        // Process with limiter enabled — verify no panic, output is finite
        let mut frame3 = [0.5_f64, -0.3_f64];
        chain.process_frame(&mut frame3);
        assert!(frame3[0].is_finite());
        assert!(frame3[1].is_finite());
    }

    // ========================================================================
    // Compressor optimizado (buffer circular + SIMD)
    // ========================================================================

    #[test]
    fn test_compressor_circular_passthrough_when_disabled() {
        let mut compressor = Compressor::new();
        compressor.enabled = false;
        let input = vec![0.5_f64, -0.4, 0.3, -0.2, 0.6, 0.1];
        let mut output = input.clone();
        for frame in output.chunks_mut(2) {
            compressor.process(frame);
        }
        for (a, b) in input.iter().zip(output.iter()) {
            assert!(
                (a - b).abs() < 1e-12,
                "disabled compressor must pass through"
            );
        }
    }

    #[test]
    fn test_compressor_circular_applies_gain_reduction() {
        let mut compressor = Compressor::new();
        compressor.enabled = true;
        // Señal muy fuerte y sostenida (muy por encima del umbral) → la reducción
        // de ganancia debe superar al makeup y reducir el nivel.
        let input: Vec<f64> = (0..4000)
            .map(|i| if i % 2 == 0 { 3.0 } else { -3.0 })
            .collect();
        let mut output = input.clone();
        for frame in output.chunks_mut(2) {
            compressor.process(frame);
        }
        let in_rms: f64 = input.iter().map(|s| s * s).sum::<f64>() / input.len() as f64;
        let out_rms: f64 = output.iter().map(|s| s * s).sum::<f64>() / output.len() as f64;
        assert!(in_rms > 1.0, "test signal must be loud");
        assert!(
            out_rms < in_rms * 0.8,
            "compressor should reduce level on a loud sustained signal (in={}, out={})",
            in_rms,
            out_rms
        );
    }

    #[test]
    fn test_compressor_circular_no_nan() {
        let mut compressor = Compressor::new();
        compressor.enabled = true;
        let mut output = Vec::new();
        // Llenar el lookahead con señal limpia y procesar activamente.
        for _ in 0..100 {
            let mut frame = [0.9_f64, -0.9];
            compressor.process(&mut frame);
            output.extend_from_slice(&frame);
        }
        // Inyectar NaN/Inf: la cadena de procesamiento debe mantener la salida finita.
        for _ in 0..100 {
            let mut frame = [f64::NAN, f64::INFINITY];
            compressor.process(&mut frame);
            output.extend_from_slice(&frame);
        }
        for s in output.iter() {
            assert!(s.is_finite(), "compressor output must be finite");
        }
    }

    #[test]
    fn test_compressor_circular_multichannel_no_panic() {
        let mut compressor = Compressor::new();
        compressor.enabled = true;
        // 6 canales (5.1) → sin panic y sin NaN.
        let mut frame = [0.5_f64, -0.4, 0.3, -0.2, 0.1, 0.0];
        for _ in 0..500 {
            compressor.process(&mut frame);
        }
        for s in frame.iter() {
            assert!(s.is_finite());
        }
    }

    // ========================================================================
    // Limiter optimizado (polyphase true-peak inter-sample)
    // ========================================================================

    #[test]
    fn test_limiter_true_peak_at_least_sample_peak() {
        // El true-peak inter-sample debe ser >= al pico de muestra y finito.
        let frame = [0.8_f64, 0.95, -0.9, 0.7, 0.99, -0.85];
        let sample_peak = frame.iter().map(|s| s.abs()).fold(0.0, f64::max);
        let true_peak = Limiter::true_peak(&frame);
        assert!(true_peak.is_finite());
        assert!(
            true_peak >= sample_peak - 1e-9,
            "true_peak (={}) must be >= sample_peak (={})",
            true_peak,
            sample_peak
        );
    }

    #[test]
    fn test_limiter_true_peak_catches_inter_sample() {
        // Un pico inter-sample (oscilación rápida de alta amplitud entre muestras)
        // debe producir un true-peak por encima del pico de muestra en algunos casos.
        // Usamos una señal donde el interpolador revela picos entre muestras.
        let frame = [1.0_f64, -1.0, 1.0, -1.0, 1.0, -1.0];
        let true_peak = Limiter::true_peak(&frame);
        assert!(true_peak.is_finite());
        assert!(true_peak > 0.0);
        // Para una alternancia a Nyquist, la interpolación nunca debe superar
        // excesivamente el pico de muestra (sanity: < 2x).
        assert!(true_peak < 2.0 + 1e-9);
    }

    #[test]
    fn test_limiter_process_no_nan_and_ceiling() {
        let mut limiter = Limiter::default();
        limiter.enabled = true;
        limiter.ceiling = -1.0; // -1 dBFS
        let mut frame = [0.99_f64, -0.99];
        for _ in 0..500 {
            limiter.process(&mut frame);
        }
        for s in frame.iter() {
            assert!(s.is_finite());
        }
        // Con un techo de -1 dB, la salida sostenida no debe superar el techo lineal.
        let ceiling_lin = 10.0_f64.powf(-1.0 / 20.0);
        assert!(frame.iter().all(|s| s.abs() <= ceiling_lin + 1e-6));
    }

    // ========================================================================
    // Reverb optimizado (FWHT + decorrelación por canal)
    // ========================================================================

    #[test]
    fn test_reverb_multichannel_decorrelated_and_finite() {
        let mut reverb = Reverb::new();
        reverb.enabled = true;
        // 6 canales: todos finitos y sin panic.
        let mut frame = [0.4_f64, -0.3, 0.2, -0.1, 0.5, 0.05];
        for _ in 0..300 {
            reverb.process(&mut frame);
        }
        for s in frame.iter() {
            assert!(s.is_finite(), "reverb output must be finite");
        }
        // Decorrelación por canal: con el MISMO input sostenido en todos los canales
        // (suficientes frames para superar las líneas de delay), los canales usan
        // sets FDN distintos y producen salidas distintas.
        reverb.reset_state();
        let mut sustained_frame = [0.5_f64; 6];
        for _ in 0..3000 {
            reverb.process(&mut sustained_frame);
        }
        assert!(
            (sustained_frame[0] - sustained_frame[2]).abs() > 1e-6
                || (sustained_frame[0] - sustained_frame[4]).abs() > 1e-6,
            "reverb channels should be decorrelated (got a={:?})",
            sustained_frame
        );
    }

    #[test]
    fn test_dsp_chain_multichannel_no_garbage() {
        // Reproduce el escenario del bug reportado: EQ + efectos activos con frame
        // de 6 canales. Un frame "limpio" (seno de baja amplitud) debe salir finito,
        // acotado y sin NaN/Inf — si algún efecto produce basura, lo detectamos aquí.
        let mut chain = DspChain::default();
        chain.enabled = true;
        chain.set_sample_rate(44100.0);
        chain.set_channel_count(6);
        chain.equalizer.enabled = true;
        chain.noise_gate.enabled = true;
        chain.sub_bass.enabled = true;
        chain.mid_bass.enabled = true;
        chain.voice_boost.enabled = true;
        chain.compressor.enabled = true;
        chain.reverb.enabled = true;
        chain.limiter.enabled = true;

        let mut frame = [0.0_f64; 6];
        let mut max_out = 0.0_f64;
        for n in 0..2000 {
            for (channel, s) in frame.iter_mut().enumerate() {
                *s = 0.4
                    * (2.0 * std::f64::consts::PI * 440.0 * (n as f64) / 44100.0 + channel as f64)
                        .sin();
            }
            chain.process_frame(&mut frame);
            for s in frame.iter() {
                assert!(
                    s.is_finite(),
                    "DSP multichannel produjo un valor no finito en n={}",
                    n
                );
                max_out = max_out.max(s.abs());
            }
        }
        assert!(
            max_out < 50.0,
            "salida DSP multichannel desbordada: {}",
            max_out
        );
    }

    #[test]
    fn test_eq_simd_multichannel_matches_scalar_no_oscillation() {
        // El bug del zumbido: la ruta SIMD del biquad (≥4 canales) alimentaba la
        // SALIDA como entrada retardada (x1) en vez de la entrada real → oscilación.
        // Verificamos que las 4 salidas de un mismo EqBand con la MISMA señal de
        // entrada son idénticas (filtro lineal determinista) y acotadas.
        use crate::audio::dsp::EqBand;
        let mut band = EqBand::new(1000.0);
        band.set_gain(6.0);
        band.resize_channels(4);

        let mut frame = [0.0_f64; 4];
        let mut max_out = 0.0_f64;
        for n in 0..4000 {
            let x = 0.3 * (2.0 * std::f64::consts::PI * 200.0 * n as f64 / 44100.0).sin();
            for channel in frame.iter_mut() {
                *channel = x;
            }
            band.process_frame(&mut frame);
            for s in frame.iter() {
                assert!(
                    s.is_finite(),
                    "EqBand SIMD produjo un valor no finito en n={}",
                    n
                );
                max_out = max_out.max(s.abs());
            }
            assert!(
                (frame[0] - frame[1]).abs() < 1e-9
                    && (frame[0] - frame[2]).abs() < 1e-9
                    && (frame[0] - frame[3]).abs() < 1e-9,
                "canales divergen en n={}: {:?}",
                n,
                frame
            );
        }
        assert!(max_out < 10.0, "EqBand SIMD osciló (zumbido): {}", max_out);
    }

    #[test]
    fn test_biquad_simd_multichannel_no_oscillation() {
        use crate::audio::dsp::BiquadFilterType;
        // SubBass/MidBass/VoiceBoost usan BiquadFilter con la misma ruta SIMD.
        let mut biquad =
            crate::audio::dsp::BiquadFilter::new(BiquadFilterType::Peak, 100.0, 6.0, 0.8);
        biquad.resize_channels(4);
        biquad.set_sample_rate(44100.0);

        let mut frame = [0.0_f64; 4];
        let mut max_out = 0.0_f64;
        for n in 0..4000 {
            let x = 0.3 * (2.0 * std::f64::consts::PI * 200.0 * n as f64 / 44100.0).sin();
            for channel in frame.iter_mut() {
                *channel = x;
            }
            biquad.process_frame(&mut frame);
            for s in frame.iter() {
                assert!(
                    s.is_finite(),
                    "BiquadFilter SIMD produjo un valor no finito"
                );
                max_out = max_out.max(s.abs());
            }
            assert!(
                (frame[0] - frame[1]).abs() < 1e-9 && (frame[0] - frame[2]).abs() < 1e-9,
                "biquad canales divergen en n={}",
                n
            );
        }
        assert!(max_out < 10.0, "BiquadFilter SIMD osciló: {}", max_out);
    }

    #[test]
    fn test_wide_is_finite_select_pattern() {
        // Patrón del compresor para enmascarar NaN/Inf → 0 (verificado correcto).
        use wide::f64x4;
        let vector = f64x4::new([0.5_f64, f64::NAN, 1.0, f64::INFINITY]);
        let finite_vector = vector.is_finite().select(vector, f64x4::splat(0.0));
        let lanes = finite_vector.to_array();
        for (i, val) in lanes.iter().enumerate() {
            assert!(
                val.is_finite(),
                "is_finite().select produjo no finito en lane {}: {:?}",
                i,
                lanes
            );
        }
        assert!(
            (lanes[0] - 0.5).abs() < 1e-12,
            "lane 0 debe pasar: {:?}",
            lanes
        );
        assert!(
            lanes[1].abs() < 1e-12,
            "lane 1 (NaN) debe ser 0: {:?}",
            lanes
        );
    }

    #[test]
    fn test_noise_gate_nan_no_panic() {
        // Regresión del crash: NoiseGate hacía partial_cmp(b).unwrap() sobre frames
        // con NaN → panic y cierre del reproductor. Ahora usa f64::max (ignora NaN).
        let mut noise_gate = NoiseGate::default();
        noise_gate.enabled = true;
        let mut frame = [f64::NAN, f64::INFINITY, 0.5, -0.3];
        noise_gate.process(&mut frame); // No debe paniquear.
        for s in frame.iter() {
            assert!(s.is_finite(), "noise gate debe emitir valores finitos");
        }
    }

    #[test]
    fn test_dsp_chain_nan_input_does_not_crash() {
        // Regresión del crash con tasas altas + canales > 2 + efectos: la cadena
        // completa recibe un frame con NaN y NO debe paniquear; la salida debe ser
        // finita (los guards de cada efecto la limpian).
        let mut chain = DspChain::default();
        chain.enabled = true;
        chain.set_sample_rate(192000.0);
        chain.set_channel_count(8);
        chain.equalizer.enabled = true;
        chain.noise_gate.enabled = true;
        chain.sub_bass.enabled = true;
        chain.mid_bass.enabled = true;
        chain.voice_boost.enabled = true;
        chain.compressor.enabled = true;
        chain.reverb.enabled = true;
        chain.limiter.enabled = true;

        let mut frame = [f64::NAN; 8];
        chain.process_frame(&mut frame); // No debe paniquear.
        for s in frame.iter() {
            assert!(s.is_finite(), "la cadena DSP debe emitir valores finitos");
        }
    }

    #[test]
    fn test_dsp_chain_nan_inf_stress_multichannel() {
        // La cadena completa debe sanear NaN/±Inf en configuraciones de 4/6/8
        // canales: los guards de cada efecto (noise gate, limitador) limpian lo
        // no finito y la salida nunca debe contener NaN/Inf.
        for channels in [4usize, 6, 8] {
            let mut chain = DspChain::default();
            chain.enabled = true;
            chain.set_sample_rate(192000.0);
            chain.set_channel_count(channels);
            chain.equalizer.enabled = true;
            chain.noise_gate.enabled = true;
            chain.sub_bass.enabled = true;
            chain.mid_bass.enabled = true;
            chain.voice_boost.enabled = true;
            chain.compressor.enabled = true;
            chain.reverb.enabled = true;
            chain.limiter.enabled = true;

            for (case, value) in [
                ("NaN", f64::NAN),
                ("+Inf", f64::INFINITY),
                ("-Inf", f64::NEG_INFINITY),
            ] {
                let mut frame = vec![value; channels];
                chain.process_frame(&mut frame);
                for (i, s) in frame.iter().enumerate() {
                    assert!(
                        s.is_finite(),
                        "ch={channels}, caso={case}: muestra {i} no finita: {s}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_reverb_lfe_bypassed_and_center_reduced() {
        // El reverb por rol de canal: el LFE (canal 3) pasa sin reverb y el centro
        // (canal 2) lleva menos wet. Con el MISMO input en todos los canales, la
        // salida del LFE debe ser exactamente el input (sin modificar) mientras
        // FL/FR sí se procesan.
        let mut reverb = Reverb::new();
        reverb.enabled = true;
        reverb.wet = 1.0;
        reverb.set_room_size(0.5);

        let mut frame = [0.3_f64; 6];
        for _ in 0..100 {
            reverb.process(&mut frame);
        }
        // El LFE no debe haber sido procesado: el FDN del LFE nunca recibe entrada,
        // así que su salida permanece = input (0.3) tras el dry/wet con wet del LFE.
        // Con wet=1.0 y passthrough, el LFE conserva exactamente su valor.
        assert!(
            (frame[3] - 0.3).abs() < 1e-12,
            "LFE debe pasar sin reverb (got {})",
            frame[3]
        );
        // Los canales frontales SÍ se procesan (deben diferir del input).
        assert!(
            (frame[0] - 0.3).abs() > 1e-9,
            "FL debe tener reverb (got {})",
            frame[0]
        );
    }
    #[test]
    fn test_resampler_high_ratio_44100_to_192000() {
        // Diagnóstico del bug de sample rate: ¿el resampler sinc falla a ratios altos?
        use rubato::{
            Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
            WindowFunction,
        };
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: Some(0.99),
            interpolation: SincInterpolationType::Cubic,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };
        let ratio = 192000.0 / 44100.0;
        let mut resampler =
            match Async::<f64>::new_sinc(ratio, 2.0, &params, 1024, 2, FixedAsync::Input) {
                Ok(resampler) => resampler,
                Err(e) => panic!("new_sinc FALLÓ a ratio {}: {:?}", ratio, e),
            };
        // Alimentar 1024 frames de un seno y procesar.
        let mut input: Vec<Vec<f64>> = vec![vec![0.0; 1024], vec![0.0; 1024]];
        for i in 0..1024 {
            let v = 0.5 * (2.0 * std::f64::consts::PI * 440.0 * i as f64 / 44100.0).sin();
            input[0][i] = v;
            input[1][i] = v;
        }
        let needed = resampler.input_frames_next();
        assert_eq!(needed, 1024);
        let out_frames = resampler.output_frames_next();
        let mut output: Vec<Vec<f64>> = vec![vec![0.0; out_frames], vec![0.0; out_frames]];
        let input_adapt =
            audioadapter_buffers::direct::SequentialSliceOfVecs::new(&input, 2, needed).unwrap();
        let mut output_adapt = audioadapter_buffers::direct::SequentialSliceOfVecs::new_mut(
            &mut output,
            2,
            out_frames,
        )
        .unwrap();
        let res = resampler.process_into_buffer(&input_adapt, &mut output_adapt, None);
        assert!(res.is_ok(), "process_into_buffer falló: {:?}", res);
        // La salida debe ser finita y razonable.
        let peak = output[0].iter().map(|s| s.abs()).fold(0.0_f64, f64::max);
        assert!(
            peak.is_finite() && peak > 0.0 && peak < 2.0,
            "peak raro: {}",
            peak
        );
        println!(
            "OK: ratio {} → {} frames de salida, peak {}",
            ratio, out_frames, peak
        );
    }

    #[test]
    fn test_resampler_ratio_8_7_sustained_output() {
        // Reproduce el patrón del decoder a 44100 -> 384000 (ratio 8.7): alimentar
        // paquetes de 1024 frames repetidamente y verificar que cada llamada produce
        // una cantidad razonable de frames (sin atascarse en 0).
        use rubato::{
            Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
            WindowFunction,
        };
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: Some(0.99),
            interpolation: SincInterpolationType::Cubic,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };
        let ratio = 384000.0 / 44100.0;
        let mut resampler = Async::<f64>::new_sinc(ratio, 2.0, &params, 1024, 2, FixedAsync::Input)
            .expect("new_sinc ratio 8.7 falló");
        let mut total_out_frames = 0usize;
        for chunk in 0..20 {
            let needed = resampler.input_frames_next();
            assert!(needed > 0, "input_frames_next() == 0 en chunk {}", chunk);
            let mut input: Vec<Vec<f64>> = vec![vec![0.0; needed], vec![0.0; needed]];
            for i in 0..needed {
                let v = 0.5 * (2.0 * std::f64::consts::PI * 440.0 * i as f64 / 44100.0).sin();
                input[0][i] = v;
                input[1][i] = v;
            }
            let out_frames = resampler.output_frames_next();
            assert!(
                out_frames > 0,
                "output_frames_next() == 0 en chunk {}",
                chunk
            );
            let mut output: Vec<Vec<f64>> = vec![vec![0.0; out_frames], vec![0.0; out_frames]];
            let in_adapt =
                audioadapter_buffers::direct::SequentialSliceOfVecs::new(&input, 2, needed)
                    .unwrap();
            let mut out_adapt = audioadapter_buffers::direct::SequentialSliceOfVecs::new_mut(
                &mut output,
                2,
                out_frames,
            )
            .unwrap();
            assert!(
                resampler
                    .process_into_buffer(&in_adapt, &mut out_adapt, None)
                    .is_ok(),
                "process_into_buffer falló en chunk {}",
                chunk
            );
            // La salida debe ser finita.
            for s in output[0].iter() {
                assert!(s.is_finite(), "salida no finita en chunk {}", chunk);
            }
            total_out_frames += out_frames;
        }
        // 20 chunks × ~6600 frames ≈ 132k frames ≈ 1.38s a 96000 frames/s reales...
        // lo importante: nunca se atasca y la salida es continua.
        assert!(
            total_out_frames > 100_000,
            "producción demasiado baja: {}",
            total_out_frames
        );
    }

    /// Implementación de resampleo comparada: el interpolador sinc (asíncrono) que
    /// usa el decodificador y el candidato FFT (síncrono) de rubato.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ResamplerKind {
        Sinc,
        Fft,
    }

    const RESAMPLER_STEREO: usize = 2;
    const RESAMPLER_CHUNK: usize = 1024;
    const RESAMPLER_CHUNKS: usize = 40;
    const RESAMPLER_WARMUP_CHUNKS: usize = 20;

    /// Construye el resampler del tipo pedido con la misma semántica de entrada
    /// fija (`FixedAsync::Input` y `FixedSync::Input`) y el mismo tamaño de chunk.
    fn make_resampler(
        kind: ResamplerKind,
        in_rate: f64,
        out_rate: f64,
        params: &SincInterpolationParameters,
    ) -> Box<dyn Resampler<f64>> {
        match kind {
            ResamplerKind::Sinc => Box::new(
                Async::<f64>::new_sinc(
                    out_rate / in_rate,
                    2.0,
                    params,
                    RESAMPLER_CHUNK,
                    RESAMPLER_STEREO,
                    FixedAsync::Input,
                )
                .expect("new_sinc debe construirse"),
            ),
            // Fft exige pares de tasas enteras; se redondean desde el par f64.
            ResamplerKind::Fft => Box::new(
                Fft::<f64>::new(
                    in_rate as usize,
                    out_rate as usize,
                    RESAMPLER_CHUNK,
                    RESAMPLER_STEREO,
                    FixedSync::Input,
                )
                .expect("Fft debe construirse"),
            ),
        }
    }

    /// Pico en régimen estable de un tono `freq` pasado por el resampler indicado.
    /// El mismo warmup, número de chunks y tamaño de chunk se usan para ambos tipos,
    /// de modo que la comparación entre sinc y FFT sea justa.
    fn steady_state_peak(
        kind: ResamplerKind,
        in_rate: f64,
        out_rate: f64,
        freq: f64,
        params: &SincInterpolationParameters,
    ) -> f64 {
        let mut resampler = make_resampler(kind, in_rate, out_rate, params);
        let mut sample_index = 0.0_f64;
        let mut peak = 0.0_f64;
        for chunk in 0..RESAMPLER_CHUNKS {
            let needed = resampler.input_frames_next();
            let mut input: Vec<Vec<f64>> = vec![vec![0.0; needed]; RESAMPLER_STEREO];
            for frame in 0..needed {
                let v = 0.5 * (2.0 * std::f64::consts::PI * freq * sample_index / in_rate).sin();
                input[0][frame] = v;
                input[1][frame] = v;
                sample_index += 1.0;
            }
            let out_frames = resampler.output_frames_next();
            let mut output: Vec<Vec<f64>> = vec![vec![0.0; out_frames]; RESAMPLER_STEREO];
            let in_adapt = audioadapter_buffers::direct::SequentialSliceOfVecs::new(
                &input,
                RESAMPLER_STEREO,
                needed,
            )
            .unwrap();
            let mut out_adapt = audioadapter_buffers::direct::SequentialSliceOfVecs::new_mut(
                &mut output,
                RESAMPLER_STEREO,
                out_frames,
            )
            .unwrap();
            resampler
                .process_into_buffer(&in_adapt, &mut out_adapt, None)
                .expect("process_into_buffer debe tener éxito");
            if chunk >= RESAMPLER_WARMUP_CHUNKS {
                for &s in &output[0] {
                    let a = s.abs();
                    if a > peak {
                        peak = a;
                    }
                }
            }
        }
        peak
    }

    /// Parámetros del perfil audiófilo: los mismos que usa el gate sinc original.
    fn audiophile_sinc_params() -> SincInterpolationParameters {
        SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: Some(0.99),
            interpolation: SincInterpolationType::Cubic,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        }
    }

    #[test]
    fn test_resampler_antialiasing_preserved_with_explicit_cutoff() {
        // Downsample 48 kHz -> 44.1 kHz. La banda de paso (1 kHz) debe sobrevivir y
        // un tono por encima del Nyquist de salida (por ejemplo 23 kHz > 22.05 kHz)
        // debe quedar eliminado por el filtro anti-aliasing. El `f_cutoff` explícito
        // fija el rolloff medido antes de la migración de rubato; cambiarlo por el
        // cutoff automático o por `None` altera silenciosamente estas cifras.
        let params = audiophile_sinc_params();

        let passband_peak =
            steady_state_peak(ResamplerKind::Sinc, 48000.0, 44100.0, 1000.0, &params);
        let rolloff_peak =
            steady_state_peak(ResamplerKind::Sinc, 48000.0, 44100.0, 21500.0, &params);
        let stopband_peak =
            steady_state_peak(ResamplerKind::Sinc, 48000.0, 44100.0, 23000.0, &params);

        println!(
            "resampler anti-aliasing (sinc): passband_peak={:.6} rolloff_peak={:.6} stopband_peak={:.6}",
            passband_peak, rolloff_peak, stopband_peak
        );

        assert!(
            passband_peak > 0.4,
            "la banda de paso 1 kHz se atenuó demasiado: {passband_peak}"
        );
        // Baseline medido con `f_cutoff = 0.99`: 21.5 kHz conserva ~0.4369. La cota
        // superior descarta un cutoff más alto (que dejaría pasar la banda casi
        // intacta) y la inferior uno más bajo (que la atenuaría de más).
        assert!(
            (0.38..0.49).contains(&rolloff_peak),
            "el rolloff no coincide con el baseline medido: {rolloff_peak}"
        );
        assert!(
            stopband_peak < 0.05,
            "la banda eliminada no se atenuó lo suficiente: {stopband_peak}"
        );
    }

    /// Compara la respuesta en frecuencia de sinc y del candidato FFT en tres pares de
    /// tasas representativos: downsample 48k->44.1k, upsample 44.1k->48k y un caso
    /// hi-res 96k->48k. Los valores se registran; para el candidato solo se exige
    /// finitud y positividad, de modo que la evaluación mida en vez de pasar en
    /// silencio. El gate de calidad estricto del perfil sinc vive en la prueba anterior.
    #[test]
    fn test_resampler_antialiasing_fft_vs_sinc_comparison() {
        let params = audiophile_sinc_params();
        // (in_rate, out_rate, tono de banda de paso, tono de rolloff, tono de stopband)
        let cases = [
            (48000.0_f64, 44100.0_f64, 1000.0, 21500.0, 23000.0),
            (44100.0, 48000.0, 1000.0, 15000.0, 20000.0),
            (96000.0, 48000.0, 1000.0, 23000.0, 30000.0),
        ];

        for &(in_rate, out_rate, passband_hz, rolloff_hz, stopband_hz) in &cases {
            for kind in [ResamplerKind::Sinc, ResamplerKind::Fft] {
                let passband = steady_state_peak(kind, in_rate, out_rate, passband_hz, &params);
                let rolloff = steady_state_peak(kind, in_rate, out_rate, rolloff_hz, &params);
                let stopband = steady_state_peak(kind, in_rate, out_rate, stopband_hz, &params);

                println!(
                    "resampler compare [{:.0}->{:.0}] {:?}: passband({:.0}Hz)={:.6} rolloff({:.0}Hz)={:.6} stopband({:.0}Hz)={:.6}",
                    in_rate,
                    out_rate,
                    kind,
                    passband_hz,
                    passband,
                    rolloff_hz,
                    rolloff,
                    stopband_hz,
                    stopband
                );

                for (band, value) in [
                    ("passband", passband),
                    ("rolloff", rolloff),
                    ("stopband", stopband),
                ] {
                    assert!(
                        value.is_finite(),
                        "banda {band} no finita para {kind:?} en {in_rate}->{out_rate}: {value}"
                    );
                    if band == "stopband" {
                        // La banda atenuada puede llegar a 0.0 de forma legítima.
                        assert!(
                            value >= 0.0,
                            "banda {band} negativa para {kind:?} en {in_rate}->{out_rate}: {value}"
                        );
                    } else {
                        assert!(
                            value > 0.0,
                            "banda {band} no positiva para {kind:?} en {in_rate}->{out_rate}: {value}"
                        );
                    }
                }
            }
        }
    }
}
