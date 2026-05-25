#[cfg(test)]
mod tests {
    use crate::audio::dsp::{
        Compressor, DspChain, Equalizer, ExpanderMode, Limiter, MidBass, MultiBandStereoExpander,
        NoiseGate, Reverb, StereoBalance, SubBass, VoiceBoost,
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
        let eq = Equalizer::new(31);
        assert_eq!(eq.bands.len(), 31);
    }

    #[test]
    fn test_eq_new_20_bands() {
        let eq = Equalizer::new(20);
        assert_eq!(eq.bands.len(), 20);
    }

    #[test]
    fn test_eq_defaults_to_31() {
        let eq = Equalizer::new(999);
        assert_eq!(eq.bands.len(), 31);
        let eq = Equalizer::new(0);
        assert_eq!(eq.bands.len(), 31);
    }

    #[test]
    fn test_eq_band_frequencies_monotonic() {
        for num_bands in [20, 31] {
            let eq = Equalizer::new(num_bands);
            for window in eq.bands.windows(2) {
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
        let mut eq = Equalizer::new(31);
        eq.bands[0].set_gain(6.0);
        eq.bands[15].set_gain(-3.0);
        eq.reset_all();
        for band in &eq.bands {
            assert!((band.gain - 0.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_eq_set_mode_switch() {
        let mut eq = Equalizer::new(31);
        assert_eq!(eq.bands.len(), 31);
        eq.set_mode(20);
        assert_eq!(eq.bands.len(), 20);
        eq.set_mode(31);
        assert_eq!(eq.bands.len(), 31);
    }

    #[test]
    fn test_eq_process_enabled_false() {
        let mut eq = Equalizer::new(31);
        eq.enabled = false;
        let mut sample = 0.5_f64;
        eq.process(&mut sample, 0);
        assert!((sample - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_eq_set_sample_rate() {
        let mut eq = Equalizer::new(31);
        eq.set_sample_rate(96000.0);
    }

    // ========================================================================
    // Reverb tests
    // ========================================================================

    #[test]
    fn test_reverb_new_disabled() {
        let r = Reverb::new();
        assert!(!r.enabled);
    }

    #[test]
    fn test_reverb_default_values() {
        let r = Reverb::new();
        assert!((r.room_size - 0.92).abs() < 1e-6);
        assert!((r.damping - 0.35).abs() < 1e-6);
        assert!((r.wet - 0.85).abs() < 1e-6);
        assert!((r.dry - 0.6).abs() < 1e-6);
    }

    #[test]
    fn test_reverb_room_size_clamping() {
        let mut r = Reverb::new();
        r.set_room_size(1.5);
        assert!((r.room_size - 1.0).abs() < 1e-6);
        r.set_room_size(-0.5);
        assert!((r.room_size - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_reverb_damping_clamping() {
        let mut r = Reverb::new();
        r.set_damping(2.0);
        assert!((r.damping - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_reverb_wet_clamping() {
        let mut r = Reverb::new();
        r.set_wet(1.5);
        assert!((r.wet - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_reverb_process_no_panic() {
        let mut r = Reverb::new();
        r.enabled = true;
        r.process(&mut [0.5]);
        r.process(&mut [0.5, -0.3]);
    }

    #[test]
    fn test_reverb_reset_state() {
        let mut r = Reverb::new();
        r.reset_state();
    }

    // ========================================================================
    // Compressor tests
    // ========================================================================

    #[test]
    fn test_compressor_new_disabled() {
        let c = Compressor::new();
        assert!(!c.enabled);
    }

    #[test]
    fn test_compressor_params_defaults() {
        let c = Compressor::new();
        assert!((c.threshold - (-10.0)).abs() < 1e-6);
        assert!((c.ratio - 4.0).abs() < 1e-6);
        assert!((c.attack - 0.005).abs() < 1e-6);
        assert!((c.release - 0.1).abs() < 1e-6);
    }

    #[test]
    fn test_compressor_ratio_min_enforced() {
        let mut c = Compressor::new();
        c.set_params(-10.0, 0.5, 0.01, 0.1);
        assert!(c.ratio >= 1.0);
    }

    #[test]
    fn test_compressor_attack_release_min() {
        let mut c = Compressor::new();
        c.set_params(-10.0, 4.0, 0.0, 0.0);
        assert!(c.attack >= 0.001);
        assert!(c.release >= 0.001);
    }

    #[test]
    fn test_compressor_process_no_panic() {
        let mut c = Compressor::new();
        c.enabled = true;
        c.process(&mut [0.5, -0.3]);
    }

    #[test]
    fn test_compressor_reset_state() {
        let mut c = Compressor::new();
        c.reset_state();
    }

    // ========================================================================
    // StereoBalance tests
    // ========================================================================

    #[test]
    fn test_stereo_balance_default_centered() {
        let sb = StereoBalance::default();
        assert!((sb.balance - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_stereo_balance_full_left() {
        let mut sb = StereoBalance::default();
        sb.enabled = true;
        sb.balance = -1.0;
        let mut frame = [1.0_f64, 1.0_f64];
        sb.process(&mut frame);
        assert!((frame[0] - 1.0).abs() < 1e-10);
        assert!((frame[1] - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_stereo_balance_full_right() {
        let mut sb = StereoBalance::default();
        sb.enabled = true;
        sb.balance = 1.0;
        let mut frame = [1.0_f64, 1.0_f64];
        sb.process(&mut frame);
        assert!((frame[0] - 0.0).abs() < 1e-10);
        assert!((frame[1] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_stereo_balance_center_no_change() {
        let mut sb = StereoBalance::default();
        sb.enabled = true;
        sb.balance = 0.0;
        let mut frame = [0.5_f64, 0.5_f64];
        sb.process(&mut frame);
        assert!((frame[0] - 0.5).abs() < 1e-10);
        assert!((frame[1] - 0.5).abs() < 1e-10);
    }

    // ========================================================================
    // Limiter tests
    // ========================================================================

    #[test]
    fn test_limiter_default_disabled() {
        let l = Limiter::default();
        assert!(!l.enabled);
    }

    #[test]
    fn test_limiter_ceiling_default() {
        let l = Limiter::default();
        assert!((l.ceiling - (-0.1)).abs() < 1e-4);
    }

    #[test]
    fn test_limiter_process_no_panic() {
        let mut l = Limiter::default();
        l.enabled = true;
        l.process(&mut [0.5, -0.3]);
    }

    #[test]
    fn test_limiter_reset_state() {
        let mut l = Limiter::default();
        l.reset_state();
    }

    // ========================================================================
    // NoiseGate tests
    // ========================================================================

    #[test]
    fn test_noise_gate_default_threshold() {
        let ng = NoiseGate::default();
        assert!((ng.threshold - (-60.0)).abs() < 1e-6);
    }

    #[test]
    fn test_noise_gate_process_no_panic() {
        let mut ng = NoiseGate::default();
        ng.enabled = true;
        ng.process(&mut [0.5, -0.3]);
    }

    #[test]
    fn test_noise_gate_reset_state() {
        let mut ng = NoiseGate::default();
        ng.reset_state();
    }

    // ========================================================================
    // SubBass tests
    // ========================================================================

    #[test]
    fn test_sub_bass_default_values() {
        let sb = SubBass::default();
        assert!((sb.freq - 45.0).abs() < 1e-6);
        assert!((sb.gain - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_sub_bass_process_no_panic() {
        let mut sb = SubBass::default();
        sb.enabled = true;
        sb.process(&mut [0.5, -0.3]);
    }

    // ========================================================================
    // MidBass tests
    // ========================================================================

    #[test]
    fn test_mid_bass_default_values() {
        let mb = MidBass::default();
        assert!((mb.freq - 100.0).abs() < 1e-6);
        assert!((mb.gain - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_mid_bass_set_sample_rate() {
        let mut mb = MidBass::default();
        mb.set_sample_rate(48000.0);
    }

    // ========================================================================
    // VoiceBoost tests
    // ========================================================================

    #[test]
    fn test_voice_boost_default() {
        let vb = VoiceBoost::default();
        assert!((vb.gain - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_voice_boost_set_sample_rate() {
        let mut vb = VoiceBoost::default();
        vb.set_sample_rate(48000.0);
    }

    // ========================================================================
    // MultiBandStereoExpander tests
    // ========================================================================

    #[test]
    fn test_stereo_expander_default_values() {
        let exp = MultiBandStereoExpander::default();
        assert!((exp.width - 1.0).abs() < 1e-6);
        assert_eq!(exp.mode, ExpanderMode::Hybrid);
    }

    #[test]
    fn test_stereo_expander_process_stereo_no_panic() {
        let mut exp = MultiBandStereoExpander::default();
        exp.enabled = true;
        exp.width = 1.5;
        let mut frame = [0.5_f64, -0.3_f64];
        exp.process(&mut frame);
    }

    #[test]
    fn test_stereo_expander_reset_state() {
        let mut exp = MultiBandStereoExpander::default();
        exp.reset_state();
    }
}
