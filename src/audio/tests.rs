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
            &input, frames, in_channels, out_channels, &map,
            (1.0, 1.0, 1.0, 1.0), &mut out_full,
        );

        let mut out_no_lfe = Vec::new();
        crate::audio::engine::AudioEngine::mix_channels_planar(
            &input, frames, in_channels, out_channels, &map,
            (1.0, 0.0, 1.0, 1.0), &mut out_no_lfe,
        );

        assert_eq!(out_full.len(), 2);
        assert_eq!(out_no_lfe.len(), 2);

        // La diferencia debe ser exactamente LFE (1.0 * coeff_diff = 1.0)
        // LFE se suma por igual a L y R: out_full = out_no_lfe + LFE * (1.0 - 0.0) para cada canal
        for ch in 0..2 {
            let diff = (out_full[ch] - out_no_lfe[ch] - 1.0).abs();
            assert!(diff < 1e-6, "LFE contribution should be exactly 1.0 per channel: {}", diff);
        }

        // Con center coeff=0.0, la contribucion del centro debe desaparecer
        let mut out_no_center = Vec::new();
        crate::audio::engine::AudioEngine::mix_channels_planar(
            &input, frames, in_channels, out_channels, &map,
            (0.0, 1.0, 1.0, 1.0), &mut out_no_center,
        );

        for ch in 0..2 {
            let diff = (out_full[ch] - out_no_center[ch] - 0.8).abs();
            assert!(diff < 1e-6, "Center contribution should be exactly 0.8: {}", diff);
        }
    }
}
