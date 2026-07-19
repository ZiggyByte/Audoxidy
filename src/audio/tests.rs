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
        assert_eq!(state.sample_rate, 44100);
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
        assert_eq!(state.fades_enabled, true);
        assert!((state.fade_in_ms - 1000.0).abs() < f64::EPSILON as f32);
        assert!((state.fade_out_ms - 1000.0).abs() < f64::EPSILON as f32);

        // Volumen y Mezcla — Silence removal (D-14, D-16)
        assert_eq!(state.silence_enabled, true);
        assert!((state.silence_duration_ms - 1000.0).abs() < f64::EPSILON as f32);
        assert!((state.silence_threshold_db - (-50.0)).abs() < f64::EPSILON as f32);

        // Volumen y Mezcla — Normalization (D-21, D-22, D-23)
        assert_eq!(state.normalize_enabled, false);
        assert!((state.normalize_target_db - (-14.0)).abs() < f64::EPSILON as f32);
        assert!((state.normalize_cap_db - 6.0).abs() < f64::EPSILON as f32);

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
        assert_eq!(state.fades_enabled, true);
        assert!((state.fade_in_ms - 1000.0).abs() < f32::EPSILON);
        assert!((state.fade_out_ms - 1000.0).abs() < f32::EPSILON);

        // Silence removal (D-14, D-16)
        assert_eq!(state.silence_enabled, true);
        assert!((state.silence_duration_ms - 1000.0).abs() < f32::EPSILON);
        assert!((state.silence_threshold_db - (-50.0)).abs() < f32::EPSILON);

        // Normalization (D-21, D-22, D-23)
        assert_eq!(state.normalize_enabled, false);
        assert!((state.normalize_target_db - (-14.0)).abs() < f32::EPSILON);
        assert!((state.normalize_cap_db - 6.0).abs() < f32::EPSILON);

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
        let gain = compute_gain_db(
            None, None, true, true, true, 0.0, 0.0, 2.0, true, 0.0,
        );
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
}
